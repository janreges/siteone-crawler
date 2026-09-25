// SiteOne Crawler - Screenshot viewports
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Parses `--screenshot-viewport`: one or more comma-separated sizes, each `WxH` or a named preset.
// The first size is the render viewport; screenshots are taken in every size. Always compiled,
// because option validation runs in every build.

/// Named sizes accepted by `--screenshot-viewport`.
pub const VIEWPORT_PRESETS: [(&str, (u32, u32)); 3] = [
    ("desktop", (1920, 1080)),
    ("tablet", (768, 1024)),
    ("mobile", (390, 844)),
];

/// Most sizes one run may capture (each one is another screenshot of every page).
pub const MAX_VIEWPORTS: usize = 5;

/// Largest width or height in px: the browser's capture-surface limit (~2^14 px), the same bound
/// full-page screenshots are capped at.
pub const MAX_VIEWPORT_SIDE: u32 = 16384;

/// Render viewport when `--screenshot-viewport` cannot be parsed (the crawler default).
pub const DEFAULT_VIEWPORT: (u32, u32) = (1920, 1080);

/// Parse a `--screenshot-viewport` value into `(width, height)` sizes, in the given order and
/// without duplicates. Each comma-separated entry is `WxH` or a preset name (case-insensitive);
/// empty entries are ignored. The error names the offending entry.
pub fn parse_viewports(value: &str) -> Result<Vec<(u32, u32)>, String> {
    let mut viewports: Vec<(u32, u32)> = Vec::new();
    for entry in value.split(',').map(str::trim).filter(|entry| !entry.is_empty()) {
        let viewport = parse_entry(entry)?;
        if !viewports.contains(&viewport) {
            viewports.push(viewport);
        }
    }
    if viewports.is_empty() {
        return Err("no viewport given".to_string());
    }
    if viewports.len() > MAX_VIEWPORTS {
        return Err(format!(
            "{} viewports given, at most {} are allowed",
            viewports.len(),
            MAX_VIEWPORTS
        ));
    }
    Ok(viewports)
}

/// The render viewport: the first `--screenshot-viewport` size, or `DEFAULT_VIEWPORT` when the
/// value cannot be parsed (browser-mode option validation rejects such values beforehand).
pub fn first_viewport(value: &str) -> (u32, u32) {
    parse_viewports(value)
        .ok()
        .and_then(|viewports| viewports.first().copied())
        .unwrap_or(DEFAULT_VIEWPORT)
}

/// One entry: a preset name or `WxH` with both sides between 1 and `MAX_VIEWPORT_SIDE`.
fn parse_entry(entry: &str) -> Result<(u32, u32), String> {
    let lower = entry.to_ascii_lowercase();
    if let Some((_, size)) = VIEWPORT_PRESETS.iter().find(|(name, _)| *name == lower) {
        return Ok(*size);
    }
    let (width, height) = lower
        .split_once('x')
        .and_then(
            |(width, height)| match (width.trim().parse::<u32>(), height.trim().parse::<u32>()) {
                (Ok(width), Ok(height)) if width > 0 && height > 0 => Some((width, height)),
                _ => None,
            },
        )
        .ok_or_else(|| format!("'{}' is neither WxH nor one of desktop, tablet, mobile", entry))?;
    if width > MAX_VIEWPORT_SIDE || height > MAX_VIEWPORT_SIDE {
        return Err(format!("'{}' exceeds {} px per side", entry, MAX_VIEWPORT_SIDE));
    }
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_sizes_and_presets_parse() {
        assert_eq!(parse_viewports("1280x720"), Ok(vec![(1280, 720)]));
        assert_eq!(parse_viewports("desktop"), Ok(vec![(1920, 1080)]));
        assert_eq!(parse_viewports("tablet"), Ok(vec![(768, 1024)]));
        assert_eq!(parse_viewports("mobile"), Ok(vec![(390, 844)]));
    }

    #[test]
    fn lists_keep_their_order_ignore_case_and_spaces_and_drop_duplicates() {
        assert_eq!(
            parse_viewports(" Mobile , 1280X720,desktop,1920x1080 "),
            Ok(vec![(390, 844), (1280, 720), (1920, 1080)])
        );
    }

    #[test]
    fn at_most_five_viewports() {
        assert_eq!(parse_viewports("1x1,2x2,3x3,4x4,5x5").map(|v| v.len()), Ok(5));
        assert!(parse_viewports("1x1,2x2,3x3,4x4,5x5,6x6").is_err());
    }

    #[test]
    fn invalid_entries_are_rejected() {
        for value in [
            "",
            " , ",
            "phone",
            "1920",
            "0x1080",
            "1920x0",
            "x1080",
            "1920x",
            "-1x5",
            "desktop,huge",
        ] {
            assert!(parse_viewports(value).is_err(), "{value:?} must be rejected");
        }
    }

    #[test]
    fn each_side_is_capped_at_16384_px() {
        assert_eq!(parse_viewports("16384x16384"), Ok(vec![(16384, 16384)]));
        for value in ["16385x100", "100x16385", "desktop,20000x20000"] {
            let error = parse_viewports(value).expect_err(value);
            let entry = value.rsplit(',').next().unwrap();
            assert!(error.contains(&format!("'{entry}'")), "{value}: {error}");
            assert!(error.contains("16384"), "{value}: {error}");
        }
    }

    #[test]
    fn first_viewport_falls_back_to_the_default() {
        assert_eq!(first_viewport("mobile,desktop"), (390, 844));
        assert_eq!(first_viewport("1280x720"), (1280, 720));
        assert_eq!(first_viewport("nonsense"), DEFAULT_VIEWPORT);
    }
}
