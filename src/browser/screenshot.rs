// SiteOne Crawler - Screenshot capture
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Captures a viewport or full-page
// screenshot of a rendered page and writes it to the screenshots directory.

use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, GetLayoutMetricsParams, Viewport as PageViewport,
};
use chromiumoxide::page::ScreenshotParams;
use md5::{Digest, Md5};

use crate::browser::viewport;
use crate::options::core_options::CoreOptions;

/// Chromium compositor capture-surface height limit (~2^14 px). Taller full-page captures
/// are silently truncated by the browser, so we cap + warn instead.
const MAX_FULLPAGE_HEIGHT: f64 = 16384.0;

/// Injected just before capture to settle animations so a screenshot taken mid-effect shows a
/// clean frame instead of a half-played one. Mirrors Playwright's `animations: 'disabled'`
/// strategy, which is deliberately NOT "fast-forward everything":
///
/// - CSS transitions are forced to 0s — they are always finite, so they snap to their end state.
/// - Finite animations (entrance reveals: fade/slide-in) get `finish()` → jump to their end
///   state. This is the exact case the user reported: content stuck mid-reveal.
/// - Infinite animations — and effectively-infinite ones with a very high iteration count
///   (ambient/auto-play hero loops, spinners) — get `pause()` in their current visible frame.
///   Fast-forwarding these to "the end" lands on an empty/transitional keyframe and wipes the
///   hero, so we freeze them where they are instead.
///
/// We intentionally do NOT inject a global `animation-duration:0s !important`: that forces
/// infinite `@keyframes` loops onto their final (often empty) frame — the very regression this
/// split avoids. Not covered: scroll-driven animations (`animation-timeline: scroll()/view()`)
/// whose progress is bound to scroll position, not time — `finish()` can't advance them, so they
/// stay at their scroll-0 state. Must never throw (everything is wrapped in try/catch).
const FREEZE_ANIMATIONS_JS: &str = r#"(function(){
  try{
    var s=document.createElement('style');
    s.setAttribute('data-siteone-freeze','1');
    s.textContent='*,*::before,*::after{transition-duration:0s !important;transition-delay:0s !important;}';
    (document.head||document.documentElement).appendChild(s);
  }catch(e){}
  try{
    if(document.getAnimations){document.getAnimations().forEach(function(a){
      try{
        var t=a.effect&&a.effect.getComputedTiming?a.effect.getComputedTiming():null;
        if(t&&(t.iterations===Infinity||t.iterations>100)){a.pause();}else{a.finish();}
      }catch(e){}
    });}
  }catch(e){}
})();"#;

/// Settle delay after freezing animations, so the compositor paints the final frame before
/// the screenshot samples it.
pub(crate) const FREEZE_SETTLE_MS: u64 = 150;

/// Settle running CSS/Web animations before capture: finite reveals jump to their end,
/// infinite loops freeze in place (fail-soft: a CDP evaluation error just snaps the page as-is).
pub(crate) async fn freeze_animations(page: &Page) {
    let _ = page.evaluate(FREEZE_ANIMATIONS_JS).await;
}

/// Upper bound for one capture (settle, measure, encode, write).
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);

/// Settle delay after resizing the page to a further `--screenshot-viewport` size, so the
/// responsive layout reflows and repaints before the capture.
const VIEWPORT_SETTLE_MS: u64 = 500;

/// Screenshots of one page: the file of the first `--screenshot-viewport` size, the files of the
/// further sizes, and why the series stopped early (if it did).
#[derive(Debug, Default)]
pub struct Screenshots {
    pub first: Option<String>,
    pub extra: Vec<String>,
    pub error: Option<String>,
}

/// Capture the page in every `--screenshot-viewport` size. The first size is the render viewport
/// the page was loaded in; each further size resizes the page (device pixel ratio 1, desktop
/// mode), lets it settle and captures, and the render viewport is restored at the end. With one
/// size the file names stay as they were; with several, every file gets a `_<W>x<H>` suffix.
/// A failed or timed-out capture ends the series and is reported in `error`.
pub async fn capture_all(page: &Page, options: &CoreOptions, url: &str) -> Screenshots {
    let viewports =
        viewport::parse_viewports(&options.screenshot_viewport).unwrap_or_else(|_| vec![viewport::DEFAULT_VIEWPORT]);
    let suffixed = viewports.len() > 1;
    let mut shots = Screenshots::default();

    for (index, &(width, height)) in viewports.iter().enumerate() {
        if index > 0 {
            if let Err(e) = set_viewport(page, width, height).await {
                shots.error = Some(e);
                break;
            }
            tokio::time::sleep(Duration::from_millis(VIEWPORT_SETTLE_MS)).await;
        }
        let name_viewport = suffixed.then_some((width, height));
        match tokio::time::timeout(CAPTURE_TIMEOUT, capture(page, options, url, name_viewport)).await {
            Ok(Ok(path)) if index == 0 => shots.first = Some(path),
            Ok(Ok(path)) => shots.extra.push(path),
            Ok(Err(e)) => {
                shots.error = Some(e);
                break;
            }
            Err(_) => {
                shots.error = Some(format!("screenshot {}x{} timed out", width, height));
                break;
            }
        }
    }

    // Leave the page in its render viewport.
    if suffixed {
        let (width, height) = viewports[0];
        let _ = set_viewport(page, width, height).await;
    }
    shots
}

/// Resize the page for a further screenshot size: device pixel ratio 1, desktop (not mobile)
/// emulation, bounded so a wedged page cannot hang the crawl.
async fn set_viewport(page: &Page, width: u32, height: u32) -> Result<(), String> {
    match tokio::time::timeout(
        Duration::from_secs(5),
        page.execute(SetDeviceMetricsOverrideParams::new(width, height, 1.0, false)),
    )
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("viewport {}x{} failed: {}", width, height, e)),
        Err(_) => Err(format!("viewport {}x{} timed out", width, height)),
    }
}

/// Capture a screenshot of `page` and write it to disk; `viewport` goes into the file name when
/// the run captures several sizes. Returns the saved file path.
async fn capture(
    page: &Page,
    options: &CoreOptions,
    url: &str,
    viewport: Option<(u32, u32)>,
) -> Result<String, String> {
    let dir = screenshots_dir(options);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create screenshots dir failed: {}", e))?;

    // Settle animations BEFORE measuring layout or sampling, so the snapshot isn't a
    // mid-animation frame (and full-page height is measured on the settled layout).
    // Best-effort; followed by a short settle for the compositor to repaint.
    freeze_animations(page).await;
    tokio::time::sleep(Duration::from_millis(FREEZE_SETTLE_MS)).await;

    let (format, ext) = match options.screenshot_format.to_lowercase().as_str() {
        "jpg" | "jpeg" => (CaptureScreenshotFormat::Jpeg, "jpg"),
        "webp" => (CaptureScreenshotFormat::Webp, "webp"),
        _ => (CaptureScreenshotFormat::Png, "png"),
    };

    let full_page = options.screenshot_mode.eq_ignore_ascii_case("full-page");
    let mut builder = ScreenshotParams::builder().format(format.clone());

    if full_page {
        // Cap full-page height at the compositor limit instead of letting Chromium silently
        // truncate very tall pages.
        match page.execute(GetLayoutMetricsParams::default()).await {
            Ok(metrics) => {
                let size = &metrics.result.css_content_size;
                if size.height > MAX_FULLPAGE_HEIGHT {
                    eprintln!(
                        "⚠️  Full-page screenshot for {} capped at {}px (page is {}px tall; browser limit ~16384px).",
                        url, MAX_FULLPAGE_HEIGHT as i64, size.height as i64
                    );
                    builder = builder
                        .clip(PageViewport {
                            x: 0.0,
                            y: 0.0,
                            width: size.width,
                            height: MAX_FULLPAGE_HEIGHT,
                            scale: 1.0,
                        })
                        .capture_beyond_viewport(true);
                } else {
                    builder = builder.full_page(true).capture_beyond_viewport(true);
                }
            }
            Err(_) => {
                builder = builder.full_page(true).capture_beyond_viewport(true);
            }
        }
    }

    // Quality only applies to lossy formats.
    if !matches!(format, CaptureScreenshotFormat::Png) {
        builder = builder.quality(options.screenshot_quality.clamp(1, 100));
    }

    let bytes = page
        .screenshot(builder.build())
        .await
        .map_err(|e| format!("screenshot failed: {}", e))?;

    let path = std::path::Path::new(&dir).join(file_name(url, viewport, ext));
    std::fs::write(&path, bytes).map_err(|e| format!("write screenshot failed: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

/// Resolve the screenshots output directory (explicit or default).
fn screenshots_dir(options: &CoreOptions) -> String {
    options
        .screenshots_dir
        .clone()
        .unwrap_or_else(|| "tmp/screenshots".to_string())
}

/// Build a filesystem-safe, collision-resistant file stem from a URL:
/// a readable, truncated, sanitized prefix plus a short md5 suffix.
fn file_stem(url: &str) -> String {
    let mut safe: String = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if safe.len() > 80 {
        safe.truncate(80);
    }
    let mut hasher = Md5::new();
    hasher.update(url.as_bytes());
    let hash = crate::utils::to_lower_hex(hasher.finalize());
    format!("{}_{}", safe.trim_matches('_'), &hash[..8])
}

/// Screenshot file name: the URL stem, a `_<W>x<H>` suffix when the run captures several
/// viewport sizes, and the image extension.
fn file_name(url: &str, viewport: Option<(u32, u32)>, ext: &str) -> String {
    match viewport {
        Some((width, height)) => format!("{}_{}x{}.{}", file_stem(url), width, height, ext),
        None => format!("{}.{}", file_stem(url), ext),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_is_unchanged_for_a_single_viewport() {
        assert_eq!(
            file_name("https://example.com/about", None, "png"),
            "example_com_about_c30b28d2.png"
        );
    }

    #[test]
    fn file_name_carries_the_size_when_there_are_several_viewports() {
        assert_eq!(
            file_name("https://example.com/about", Some((390, 844)), "webp"),
            "example_com_about_c30b28d2_390x844.webp"
        );
    }
}
