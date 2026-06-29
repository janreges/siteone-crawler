// SiteOne Crawler - AnimationExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Assembles the per-page screenshots
// captured in `--browser --screenshots` mode into an animated GIF and/or MP4 video,
// in crawl order. GIF is encoded with the embedded `image` crate (or ffmpeg when
// available); MP4 requires an external ffmpeg binary (auto-detected or --ffmpeg-path).

use std::path::Path;
use std::process::{Command, Stdio};

use image::codecs::gif::{GifEncoder, Repeat};
use image::imageops::FilterType;
use image::{Delay, DynamicImage, Frame, RgbImage, Rgba, RgbaImage};

use crate::error::CrawlerResult;
use crate::options::core_options::CoreOptions;
use crate::output::output::Output;
use crate::result::status::Status;

/// Removes a staging directory on drop, so the temp frames are cleaned up on every exit path —
/// including an early return or a panic inside the `image`/ffmpeg encode steps.
struct TempDirGuard<'a>(&'a Path);

impl Drop for TempDirGuard<'_> {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = std::fs::remove_dir_all(self.0);
        }
    }
}

/// Output formats the exporter can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFormat {
    Gif,
    Mp4,
}

/// Post-crawl exporter: builds GIF/MP4 animations from page screenshots.
pub struct AnimationExporter {
    formats: Vec<AnimationFormat>,
    frame_duration: f64,
    width: u32,
    ffmpeg_path: Option<String>,
    screenshots_dir: String,
    viewport: (u32, u32),
}

impl AnimationExporter {
    pub fn new(options: &CoreOptions) -> Self {
        Self {
            formats: parse_formats(&options.screenshots_animation),
            frame_duration: options.screenshots_animation_frame_duration.clamp(0.2, 10.0),
            width: options.screenshots_animation_width.clamp(2, 8192) as u32,
            ffmpeg_path: options.ffmpeg_path.clone(),
            screenshots_dir: options
                .screenshots_dir
                .clone()
                .unwrap_or_else(|| "tmp/screenshots".to_string()),
            viewport: parse_viewport(&options.screenshot_viewport),
        }
    }

    /// Collect screenshot file paths in crawl order (IndexMap iteration order),
    /// keeping only pages that actually produced an on-disk screenshot.
    fn collect_screenshot_paths(&self, status: &Status) -> Vec<String> {
        status
            .get_visited_urls()
            .iter()
            .filter_map(|vu| {
                status
                    .get_browser_diagnostics(&vu.uq_id)
                    .and_then(|d| d.screenshot_path)
                    .filter(|p| Path::new(p).is_file())
            })
            .collect()
    }

    /// Resolve a usable ffmpeg binary: explicit `--ffmpeg-path` first, else `ffmpeg`
    /// from PATH. Returns the invocable program string, or None if it does not run.
    fn resolve_ffmpeg(&self) -> Option<String> {
        if let Some(p) = &self.ffmpeg_path {
            return if Self::ffmpeg_works(p) { Some(p.clone()) } else { None };
        }
        let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        if Self::ffmpeg_works(exe) {
            Some(exe.to_string())
        } else {
            None
        }
    }

    fn ffmpeg_works(program: &str) -> bool {
        Command::new(program)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Stage normalized frames as numbered PNGs into `dir` for ffmpeg to consume. Frames are
    /// consumed lazily (O(1) memory). Returns the number of frames written.
    fn write_temp_frames<I: IntoIterator<Item = RgbImage>>(frames: I, dir: &Path) -> Result<usize, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("create '{}': {}", dir.display(), e))?;
        let mut count = 0usize;
        for (i, f) in frames.into_iter().enumerate() {
            let p = dir.join(format!("frame_{:05}.png", i + 1));
            f.save(&p).map_err(|e| format!("write '{}': {}", p.display(), e))?;
            count += 1;
        }
        Ok(count)
    }

    fn run_ffmpeg(program: &str, args: &[String]) -> Result<(), String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("spawn ffmpeg: {}", e))?;
        if out.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let last = stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("unknown error");
            Err(format!("ffmpeg failed: {}", last))
        }
    }

    /// GIF via ffmpeg's two-pass palettegen/paletteuse (better palette than the embedded path).
    fn encode_gif_ffmpeg(&self, program: &str, frames_dir: &Path, out: &Path) -> Result<(), String> {
        let framerate = format!("{:.6}", 1.0 / self.frame_duration);
        let input = frames_dir.join("frame_%05d.png");
        let palette = frames_dir.join("palette.png");
        Self::run_ffmpeg(
            program,
            &[
                "-y".into(),
                "-framerate".into(),
                framerate.clone(),
                // Frames are staged as frame_00001.png upward; make the start index explicit
                // instead of relying on ffmpeg's default, so a staging change can't drop frame 1.
                "-start_number".into(),
                "1".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-vf".into(),
                "palettegen".into(),
                palette.to_string_lossy().into_owned(),
            ],
        )?;
        Self::run_ffmpeg(
            program,
            &[
                "-y".into(),
                "-framerate".into(),
                framerate,
                "-start_number".into(),
                "1".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-i".into(),
                palette.to_string_lossy().into_owned(),
                "-lavfi".into(),
                "paletteuse".into(),
                "-loop".into(),
                "0".into(),
                out.to_string_lossy().into_owned(),
            ],
        )
    }

    /// MP4 (H.264) via ffmpeg + libx264. Each frame is shown for ~`frame_duration` (set via the
    /// input framerate), re-timed to a constant 25 fps output for smooth, widely-compatible
    /// playback; the per-frame hold is exact only when `25 * frame_duration` is an integer.
    fn encode_mp4_ffmpeg(&self, program: &str, frames_dir: &Path, out: &Path) -> Result<(), String> {
        let framerate = format!("{:.6}", 1.0 / self.frame_duration);
        let input = frames_dir.join("frame_%05d.png");
        Self::run_ffmpeg(
            program,
            &[
                "-y".into(),
                "-framerate".into(),
                framerate,
                "-start_number".into(),
                "1".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-r".into(),
                "25".into(),
                "-c:v".into(),
                "libx264".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-movflags".into(),
                "+faststart".into(),
                out.to_string_lossy().into_owned(),
            ],
        )
    }

    fn report(&self, status: &Status, kind: &str, path: &Path, frames: usize, res: Result<(), String>) {
        let disp = crate::utils::get_output_formatted_path(&path.to_string_lossy());
        match res {
            Ok(()) => {
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if size == 0 {
                    status.add_warning_to_summary(
                        "screenshots-animation",
                        &format!(
                            "{} animation produced an empty file at '{}' (encoder reported success).",
                            kind, disp
                        ),
                    );
                    return;
                }
                status.add_info_to_summary(
                    "screenshots-animation",
                    &format!(
                        "{} animation ({} frames, {:.1} KB) written to '{}'",
                        kind,
                        frames,
                        size as f64 / 1024.0,
                        disp
                    ),
                );
            }
            Err(e) => {
                status.add_warning_to_summary("screenshots-animation", &format!("{} animation failed: {}", kind, e))
            }
        }
    }
}

impl crate::export::exporter::Exporter for AnimationExporter {
    fn get_name(&self) -> &str {
        "AnimationExporter"
    }

    fn should_be_activated(&self) -> bool {
        !self.formats.is_empty()
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        if self.formats.is_empty() {
            return Ok(());
        }

        let paths = self.collect_screenshot_paths(status);
        if paths.is_empty() {
            status.add_warning_to_summary(
                "screenshots-animation",
                "No screenshots available to build an animation.",
            );
            return Ok(());
        }

        let (cw, ch) = canvas_size(self.width, self.viewport.0, self.viewport.1);
        let want_gif = self.formats.contains(&AnimationFormat::Gif);
        let want_mp4 = self.formats.contains(&AnimationFormat::Mp4);
        let ffmpeg = self.resolve_ffmpeg();
        let _ = std::fs::create_dir_all(&self.screenshots_dir);

        // Per-run staging dir, suffixed by PID to reduce — not perfectly prevent, since PIDs are
        // eventually reused — clashes with another run sharing the screenshots dir. We clear it
        // up front so a prior crash leaves no stale frames the ffmpeg `frame_%05d` glob would
        // pick up; the guard removes it again on every exit path (including a panic).
        let temp_dir = Path::new(&self.screenshots_dir).join(format!(".animation-frames-{}", std::process::id()));
        let _temp_guard = TempDirGuard(&temp_dir);

        // With ffmpeg available we stage normalized frames as PNGs (streamed one at a time
        // → O(1) memory) and let ffmpeg encode GIF/MP4 from them.
        let use_ffmpeg = ffmpeg.is_some() && (want_gif || want_mp4);
        let mut staged = 0usize;
        let mut staged_ok = false;
        if use_ffmpeg {
            let _ = std::fs::remove_dir_all(&temp_dir);
            match Self::write_temp_frames(
                paths.iter().filter_map(|p| decode_normalize(p.as_str(), cw, ch)),
                &temp_dir,
            ) {
                Ok(n) if n > 0 => {
                    staged = n;
                    staged_ok = true;
                    warn_skipped(status, paths.len(), n);
                }
                Ok(_) => status.add_warning_to_summary(
                    "screenshots-animation",
                    "No screenshots could be decoded for the animation.",
                ),
                Err(e) => status.add_warning_to_summary(
                    "screenshots-animation",
                    &format!("Failed to stage frames for ffmpeg: {}", e),
                ),
            }
        }

        if want_gif {
            let out = Path::new(&self.screenshots_dir).join("animation.gif");
            match (&ffmpeg, staged_ok) {
                (Some(ff), true) => {
                    let res = self.encode_gif_ffmpeg(ff, &temp_dir, &out);
                    self.report(status, "GIF", &out, staged, res);
                }
                (Some(_), false) => {
                    // ffmpeg present but staging failed — the staging branch already warned.
                }
                (None, _) => {
                    // No ffmpeg: encode the GIF with the embedded encoder, streaming frames.
                    match encode_gif_embedded(
                        paths.iter().filter_map(|p| decode_normalize(p.as_str(), cw, ch)),
                        self.frame_duration,
                        &out,
                    ) {
                        Ok(0) => {
                            let _ = std::fs::remove_file(&out);
                            status.add_warning_to_summary(
                                "screenshots-animation",
                                "No screenshots could be decoded for the animation.",
                            );
                        }
                        Ok(n) => {
                            warn_skipped(status, paths.len(), n);
                            self.report(status, "GIF", &out, n, Ok(()))
                        }
                        Err(e) => self.report(status, "GIF", &out, 0, Err(e)),
                    }
                }
            }
        }

        if want_mp4 {
            let out = Path::new(&self.screenshots_dir).join("animation.mp4");
            match (&ffmpeg, staged_ok) {
                (Some(ff), true) => {
                    let res = self.encode_mp4_ffmpeg(ff, &temp_dir, &out);
                    self.report(status, "MP4", &out, staged, res);
                }
                (Some(_), false) => {
                    // ffmpeg present but staging failed — the staging branch already warned.
                }
                (None, _) => status.add_warning_to_summary(
                    "screenshots-animation",
                    "MP4 animation requires ffmpeg. Install ffmpeg or set --ffmpeg-path; skipping MP4 (GIF is unaffected).",
                ),
            }
        }

        // `_temp_guard` removes the staging dir on the way out (success, error, or panic).
        Ok(())
    }
}

/// Parse a `--screenshots-animation` value (e.g. "gif,mp4") into formats, in the
/// order given, ignoring unknown/empty tokens.
fn parse_formats(s: &str) -> Vec<AnimationFormat> {
    let mut out = Vec::new();
    for token in s.split(',') {
        let f = match token.trim().to_lowercase().as_str() {
            "gif" => Some(AnimationFormat::Gif),
            "mp4" => Some(AnimationFormat::Mp4),
            _ => None,
        };
        if let Some(f) = f
            && !out.contains(&f)
        {
            out.push(f);
        }
    }
    out
}

/// Parse a `WxH` viewport string, falling back to 1920x1080 (matches the crawler default).
fn parse_viewport(s: &str) -> (u32, u32) {
    s.split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))
        .filter(|(w, h)| *w > 0 && *h > 0)
        .unwrap_or((1920, 1080))
}

/// Round down to the nearest even number (libx264 + yuv420p require even dimensions).
fn even(n: u32) -> u32 {
    n - (n % 2)
}

/// Canvas dimensions: width is the (even) target width; height is derived from the
/// viewport aspect ratio and rounded to an even number. Both are at least 2.
fn canvas_size(target_width: u32, vw: u32, vh: u32) -> (u32, u32) {
    let w = even(target_width.max(2)).max(2);
    let raw_h = (w as f64 * vh as f64 / vw.max(1) as f64).round() as u32;
    let h = even(raw_h).max(2);
    (w, h)
}

/// Height of an image scaled to `target_w` while preserving aspect ratio.
fn scaled_height(orig_w: u32, orig_h: u32, target_w: u32) -> u32 {
    if orig_w == 0 {
        return 0;
    }
    (orig_h as f64 * target_w as f64 / orig_w as f64).round() as u32
}

/// Normalize one decoded screenshot to an exact `canvas_w × canvas_h` RGB frame:
/// scale to the canvas width (preserving aspect), anchor at the top, crop overflow at
/// the bottom, pad any remainder with white, and flatten transparency onto white.
fn normalize_frame(img: &DynamicImage, canvas_w: u32, canvas_h: u32) -> image::RgbImage {
    let sh = scaled_height(img.width(), img.height(), canvas_w).max(1);
    let scaled = img.resize_exact(canvas_w, sh, FilterType::Lanczos3).to_rgba8();

    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([255, 255, 255, 255]));
    let copy_h = sh.min(canvas_h);
    let top = image::imageops::crop_imm(&scaled, 0, 0, canvas_w, copy_h).to_image();
    image::imageops::overlay(&mut canvas, &top, 0, 0);

    DynamicImage::ImageRgba8(canvas).to_rgb8()
}

/// Decode one screenshot and normalize it to the target canvas; silently skips on failure
/// (an undecodable image returns `None`). Per-file warnings are intentionally NOT emitted here
/// — the caller reports a single aggregated "skipped X of Y" line via `warn_skipped`, so a
/// directory of broken screenshots can't flood the summary with one warning each.
fn decode_normalize(path: &str, canvas_w: u32, canvas_h: u32) -> Option<RgbImage> {
    image::open(path)
        .ok()
        .map(|img| normalize_frame(&img, canvas_w, canvas_h))
}

/// Emit a single aggregated warning when fewer frames were usable than screenshots collected.
fn warn_skipped(status: &Status, total: usize, used: usize) {
    if used < total {
        status.add_warning_to_summary(
            "screenshots-animation",
            &format!(
                "Skipped {} of {} screenshot(s) that could not be decoded.",
                total - used,
                total
            ),
        );
    }
}

/// Encode normalized RGB frames to an animated GIF using the embedded `image` crate
/// (no external tooling). Frames are consumed lazily (O(1) memory). Each frame is shown
/// for `duration_secs`; loops forever. Returns the number of frames written.
fn encode_gif_embedded<I: IntoIterator<Item = RgbImage>>(
    frames: I,
    duration_secs: f64,
    out: &Path,
) -> Result<usize, String> {
    let file = std::fs::File::create(out).map_err(|e| format!("create '{}': {}", out.display(), e))?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = GifEncoder::new_with_speed(writer, 10);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|e| format!("gif set_repeat: {}", e))?;

    let ms = (duration_secs * 1000.0).round().max(20.0) as u32;
    let mut count = 0usize;
    for f in frames {
        let rgba = DynamicImage::ImageRgb8(f).to_rgba8();
        let frame = Frame::from_parts(rgba, 0, 0, Delay::from_numer_denom_ms(ms, 1));
        encoder
            .encode_frame(frame)
            .map_err(|e| format!("gif encode_frame: {}", e))?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_formats_handles_both_order_dedup_and_unknown() {
        assert_eq!(
            parse_formats("gif,mp4"),
            vec![AnimationFormat::Gif, AnimationFormat::Mp4]
        );
        assert_eq!(parse_formats(" MP4 "), vec![AnimationFormat::Mp4]);
        assert_eq!(parse_formats("gif,bogus"), vec![AnimationFormat::Gif]);
        assert_eq!(parse_formats("gif,gif"), vec![AnimationFormat::Gif]);
        assert!(parse_formats("").is_empty());
    }

    #[test]
    fn parse_viewport_parses_and_falls_back() {
        assert_eq!(parse_viewport("1280x720"), (1280, 720));
        assert_eq!(parse_viewport("nonsense"), (1920, 1080));
    }

    #[test]
    fn canvas_size_viewport_16_9_is_exact() {
        assert_eq!(canvas_size(1024, 1920, 1080), (1024, 576));
    }

    #[test]
    fn canvas_size_rounds_to_even() {
        assert_eq!(canvas_size(801, 1920, 1080), (800, 450));
    }

    #[test]
    fn scaled_height_for_tall_fullpage() {
        assert_eq!(scaled_height(1920, 16000, 1024), 8533);
    }

    use image::AnimationDecoder;
    use image::Rgb;
    use image::codecs::gif::GifDecoder;

    #[test]
    fn normalize_crops_tall_image_to_canvas() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 1000, Rgb([10, 20, 30])));
        let out = normalize_frame(&img, 50, 30);
        assert_eq!((out.width(), out.height()), (50, 30));
        assert_eq!(*out.get_pixel(0, 0), Rgb([10, 20, 30]));
        assert_eq!(*out.get_pixel(49, 29), Rgb([10, 20, 30]));
    }

    #[test]
    fn normalize_pads_short_image_with_white() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 20, Rgb([10, 20, 30])));
        let out = normalize_frame(&img, 50, 30);
        assert_eq!((out.width(), out.height()), (50, 30));
        assert_eq!(*out.get_pixel(0, 0), Rgb([10, 20, 30]));
        assert_eq!(*out.get_pixel(0, 29), Rgb([255, 255, 255]));
    }

    #[test]
    fn normalize_flattens_transparency_onto_white() {
        let transparent = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        let img = DynamicImage::ImageRgba8(transparent);
        let out = normalize_frame(&img, 10, 10);
        assert_eq!(*out.get_pixel(0, 0), Rgb([255, 255, 255]));
    }

    #[test]
    fn gif_roundtrip_preserves_frames_dims_and_delay() {
        let frames = vec![
            RgbImage::from_pixel(8, 6, Rgb([255, 0, 0])),
            RgbImage::from_pixel(8, 6, Rgb([0, 255, 0])),
            RgbImage::from_pixel(8, 6, Rgb([0, 0, 255])),
        ];
        let dir = std::env::temp_dir().join("soc_anim_gif_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.gif");

        let n = encode_gif_embedded(frames, 0.2, &path).unwrap();
        assert_eq!(n, 3);

        let file = std::fs::File::open(&path).unwrap();
        let decoder = GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        let decoded = decoder.into_frames().collect_frames().unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].buffer().width(), 8);
        assert_eq!(decoded[0].buffer().height(), 6);
        let (num, den) = decoded[0].delay().numer_denom_ms();
        assert_eq!(num / den, 200); // 0.2 s, GIF stores centiseconds → 200 ms

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[ignore] // requires ffmpeg in PATH
    fn mp4_via_ffmpeg_produces_nonempty_file() {
        let dir = std::env::temp_dir().join("soc_anim_mp4_test");
        std::fs::create_dir_all(&dir).unwrap();
        let frames_dir = dir.join(".animation-frames");
        let frames = vec![
            RgbImage::from_pixel(16, 16, Rgb([255, 0, 0])),
            RgbImage::from_pixel(16, 16, Rgb([0, 255, 0])),
        ];
        AnimationExporter::write_temp_frames(frames, &frames_dir).unwrap();

        let exp = AnimationExporter {
            formats: vec![AnimationFormat::Mp4],
            frame_duration: 0.5,
            width: 16,
            ffmpeg_path: None,
            screenshots_dir: dir.to_string_lossy().to_string(),
            viewport: (1920, 1080),
        };
        let program = exp.resolve_ffmpeg().expect("ffmpeg must be installed for this test");
        let out = dir.join("animation.mp4");
        exp.encode_mp4_ffmpeg(&program, &frames_dir, &out).unwrap();
        assert!(out.is_file());
        assert!(std::fs::metadata(&out).unwrap().len() > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
