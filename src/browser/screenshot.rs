// SiteOne Crawler - Screenshot capture
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Captures a viewport or full-page
// screenshot of a rendered page and writes it to the screenshots directory.

use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, GetLayoutMetricsParams, Viewport as PageViewport,
};
use chromiumoxide::page::ScreenshotParams;
use md5::{Digest, Md5};

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
const FREEZE_SETTLE_MS: u64 = 150;

/// Settle running CSS/Web animations before capture: finite reveals jump to their end,
/// infinite loops freeze in place (fail-soft: a CDP evaluation error just snaps the page as-is).
async fn freeze_animations(page: &Page) {
    let _ = page.evaluate(FREEZE_ANIMATIONS_JS).await;
}

/// Capture a screenshot of `page` and write it to disk. Returns the saved file path.
pub async fn capture(page: &Page, options: &CoreOptions, url: &str) -> Result<String, String> {
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

    let filename = format!("{}.{}", file_stem(url), ext);
    let path = std::path::Path::new(&dir).join(&filename);
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
