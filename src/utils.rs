// SiteOne Crawler - Utilities
// (c) Jan Reges <jan.reges@siteone.cz>

use std::sync::RwLock;

use regex::Regex;

use crate::types::ContentTypeId;

static FORCED_COLOR_SETUP: RwLock<Option<bool>> = RwLock::new(None);

/// Check if a string looks like a regex pattern delimited by one of / # ~ %
/// e.g. "/pattern/flags" or "#pattern#i"
pub fn is_regex_pattern(s: &str) -> bool {
    if s.len() < 2 {
        return false;
    }
    let first = s.as_bytes()[0];
    if !matches!(first, b'/' | b'#' | b'~' | b'%') {
        return false;
    }
    // Find the last occurrence of the delimiter
    if let Some(last_delim_pos) = s[1..].rfind(first as char) {
        let last_delim_pos = last_delim_pos + 1; // adjust for the slice offset
        // Everything after the last delimiter should be flags (a-z only)
        let flags = &s[last_delim_pos + 1..];
        flags.chars().all(|c| c.is_ascii_lowercase())
    } else {
        false
    }
}
/// Extract the inner regex pattern from a PCRE-delimited string (e.g., /pattern/flags).
/// If the string is not delimited, returns it as-is.
/// Converts PCRE flags like 'i' to Rust regex inline flags like (?i).
pub fn extract_pcre_regex_pattern(s: &str) -> String {
    if is_regex_pattern(s) {
        let delimiter = s.as_bytes()[0] as char;
        let rest = &s[1..];
        if let Some(end_pos) = rest.rfind(delimiter) {
            let pattern = &rest[..end_pos];
            let flags = &rest[end_pos + 1..];
            let mut regex_pattern = String::new();
            if flags.contains('i') {
                regex_pattern.push_str("(?i)");
            }
            regex_pattern.push_str(pattern);
            return regex_pattern;
        }
    }
    s.to_string()
}

static FORCED_CONSOLE_WIDTH: RwLock<Option<usize>> = RwLock::new(None);

pub const IMG_SRC_TRANSPARENT_1X1_GIF: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw==";

pub fn disable_colors() {
    if let Ok(mut v) = FORCED_COLOR_SETUP.write() {
        *v = Some(false);
    }
}

pub fn force_enabled_colors() {
    if let Ok(mut v) = FORCED_COLOR_SETUP.write() {
        *v = Some(true);
    }
}

pub fn set_forced_console_width(width: usize) {
    if let Ok(mut v) = FORCED_CONSOLE_WIDTH.write() {
        *v = Some(width);
    }
}

pub fn get_formatted_size(bytes: i64, precision: usize) -> String {
    let units = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

    let bytes_f = (bytes.max(0)) as f64;
    let pow = if bytes_f > 0.0 {
        (bytes_f.ln() / 1024_f64.ln()).floor() as usize
    } else {
        0
    };
    let pow = pow.min(units.len() - 1);

    let value = bytes_f / 1024_f64.powi(pow as i32);
    let rounded = format!("{:.prec$}", value, prec = precision);

    format!("{} {}", rounded, units[pow])
}

pub fn get_formatted_duration(duration: f64) -> String {
    if duration < 1.0 {
        let ms = duration * 1000.0;
        format!("{} ms", ms as i64)
    } else if duration < 10.0 {
        let formatted = format!("{:.1}", duration);
        let formatted = formatted.trim_end_matches(".0");
        format!("{} s", formatted)
    } else {
        format!("{} s", duration as i64)
    }
}

pub fn get_formatted_age(age: i64) -> String {
    if age < 60 {
        format!("{} sec(s)", age)
    } else if age < 3600 {
        format!(
            "{} min(s)",
            strip_trailing_dot_zero(&format!("{:.1}", age as f64 / 60.0))
        )
    } else if age < 86400 {
        format!(
            "{} hour(s)",
            strip_trailing_dot_zero(&format!("{:.1}", age as f64 / 3600.0))
        )
    } else {
        format!(
            "{} day(s)",
            strip_trailing_dot_zero(&format!("{:.1}", age as f64 / 86400.0))
        )
    }
}

/// Strip trailing ".0" from formatted numbers.
fn strip_trailing_dot_zero(s: &str) -> String {
    s.strip_suffix(".0").unwrap_or(s).to_string()
}

pub fn get_formatted_cache_lifetime(seconds: i64) -> String {
    if seconds < 60 {
        format!("{} s", seconds)
    } else if seconds <= 3600 {
        format!("{} min", seconds / 60)
    } else if seconds <= 86400 {
        format!("{} h", seconds / 3600)
    } else if seconds <= 86400 * 90 {
        format!("{} d", seconds / 86400)
    } else if seconds <= 86400 * 365 * 2 {
        format!("{} mon", (seconds as f64 / 86400.0 / 30.0).round() as i64)
    } else {
        format!("{:.1} y", seconds as f64 / 31536000.0)
    }
}

pub fn get_color_text(text: &str, color: &str, set_background: bool) -> String {
    // Check forced color setup
    let forced = FORCED_COLOR_SETUP.read().ok().and_then(|v| *v);
    match forced {
        Some(false) => return text.to_string(),
        Some(true) => {}
        None => {
            // Check if stdout is a TTY
            if !atty_is_tty() {
                return text.to_string();
            }
        }
    }

    let fg_colors: &[(&str, &str)] = &[
        ("black", "0;30"),
        ("red", "0;31"),
        ("green", "0;32"),
        ("yellow", "0;33"),
        ("blue", "0;34"),
        ("magenta", "0;35"),
        ("cyan", "0;36"),
        ("white", "0;37"),
        ("gray", "38;5;244"),
        ("dark-gray", "38;5;240"),
    ];

    let bg_colors: &[(&str, &str)] = &[
        ("black", "1;40"),
        ("red", "1;41"),
        ("green", "1;42"),
        ("yellow", "1;43"),
        ("blue", "1;44"),
        ("magenta", "1;45"),
        ("cyan", "1;46"),
        ("white", "1;47"),
    ];

    let code = if set_background {
        bg_colors
            .iter()
            .find(|(name, _)| *name == color)
            .map(|(_, code)| *code)
            .unwrap_or("0")
    } else {
        fg_colors
            .iter()
            .find(|(name, _)| *name == color)
            .map(|(_, code)| *code)
            .unwrap_or("0")
    };

    format!("\x1b[{}m{}\x1b[0m", code, text)
}

fn atty_is_tty() -> bool {
    // Simple check using libc isatty
    unsafe { libc_isatty(1) != 0 }
}

unsafe extern "C" {
    fn isatty(fd: i32) -> i32;
}

unsafe fn libc_isatty(fd: i32) -> i32 {
    unsafe { isatty(fd) }
}

pub fn convert_bash_colors_in_text_to_html(text: &str) -> String {
    use once_cell::sync::Lazy;
    static RE_BASH_COLORS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[(.*?)m(.*?)\x1b\[0m").unwrap());
    let re = &*RE_BASH_COLORS;

    re.replace_all(text, |caps: &regex::Captures| {
        let styles_str = caps.get(1).map_or("", |m| m.as_str());
        let content = caps.get(2).map_or("", |m| m.as_str());

        let styles: Vec<&str> = styles_str.split(';').collect();
        let mut font_color: Option<&str> = None;
        let mut background_color: Option<&str> = None;

        for style in &styles {
            if ["30", "31", "32", "33", "34", "35", "36", "37"].contains(style) {
                font_color = Some(style);
            } else if ["40", "41", "42", "43", "44", "45", "46", "47"].contains(style) {
                background_color = Some(style);
            }
        }

        let mut css_style = String::new();
        if let Some(fc) = font_color {
            css_style.push_str(&format!("color: {};", get_html_color_by_bash_color(fc)));
        }
        if let Some(bc) = background_color {
            css_style.push_str(&format!("background-color: {};", get_html_color_by_bash_color(bc)));
        }

        if !css_style.is_empty() {
            format!("<span style=\"{}\">{}</span>", css_style.trim_end_matches(';'), content)
        } else {
            content.to_string()
        }
    })
    .to_string()
}

fn get_html_color_by_bash_color(color: &str) -> &'static str {
    match color {
        "30" | "40" => "#000000",
        "31" | "41" => "#e3342f",
        "32" | "42" => "#38c172",
        "33" | "43" => "#ffff00",
        "34" | "44" => "#2563EB",
        "35" | "45" => "#ff00ff",
        "36" | "46" => "#00ffff",
        "37" | "47" => "#ffffff",
        _ => "#000000",
    }
}

pub fn truncate_in_two_thirds(
    text: &str,
    max_length: usize,
    placeholder: &str,
    forced_coloring: Option<bool>,
) -> String {
    let char_count = text.chars().count();
    if char_count <= max_length {
        return text.to_string();
    }

    let placeholder_len = placeholder.chars().count();
    let first_part_length = ((max_length as f64) * (2.0 / 3.0)).ceil() as usize;
    let second_part_length = if max_length > first_part_length + placeholder_len {
        max_length - first_part_length - placeholder_len
    } else {
        0
    };

    let first_part: String = text.chars().take(first_part_length).collect();
    let second_part: String = text
        .chars()
        .rev()
        .take(second_part_length)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let final_placeholder = match forced_coloring {
        Some(true) | None => get_color_text(placeholder, "red", false),
        Some(false) => placeholder.to_string(),
    };

    format!("{}{}{}", first_part.trim(), final_placeholder, second_part.trim())
}

pub fn truncate_url(
    url: &str,
    max_length: usize,
    placeholder: &str,
    strip_hostname: Option<&str>,
    scheme_of_hostname_to_strip: Option<&str>,
    forced_coloring: Option<bool>,
) -> String {
    let mut url = url.to_string();

    if let Some(hostname) = strip_hostname {
        if let Some(scheme) = scheme_of_hostname_to_strip {
            let full = format!("{}://{}", scheme, hostname);
            url = url.replace(&full, "");
        } else {
            let http = format!("http://{}", hostname);
            let https = format!("https://{}", hostname);
            url = url.replace(&http, "").replace(&https, "");
        }
    }

    if url.chars().count() > max_length {
        url = truncate_in_two_thirds(&url, max_length, placeholder, forced_coloring);
    }

    url
}

pub fn get_progress_bar(done: usize, total: usize, segments: usize) -> String {
    let percentage = (done as f64 / total as f64) * 100.0;
    let filled_segments = ((done as f64 / total as f64) * segments as f64).round() as usize;
    let empty_segments = segments.saturating_sub(filled_segments);

    format!(
        "{:>5}|{}{}|",
        format!("{}%", percentage as i64),
        ">".repeat(filled_segments),
        " ".repeat(empty_segments),
    )
}

pub fn remove_ansi_colors(text: &str) -> String {
    use once_cell::sync::Lazy;
    static RE_ANSI: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[\d+(;\d+)*m").unwrap());
    RE_ANSI.replace_all(text, "").to_string()
}

pub fn get_http_client_code_with_error_description(http_code: i32, short_version: bool) -> String {
    match http_code {
        -1 => {
            if short_version {
                "-1:CON".to_string()
            } else {
                "-1:CONN-FAIL".to_string()
            }
        }
        -2 => {
            if short_version {
                "-2:TIM".to_string()
            } else {
                "-2:TIMEOUT".to_string()
            }
        }
        -3 => {
            if short_version {
                "-3:RST".to_string()
            } else {
                "-3:SRV-RESET".to_string()
            }
        }
        -4 => {
            if short_version {
                "-4:SND".to_string()
            } else {
                "-4:SEND-ERROR".to_string()
            }
        }
        -6 => {
            if short_version {
                "-6:SKP".to_string()
            } else {
                "-6:SKIPPED".to_string()
            }
        }
        code => code.to_string(),
    }
}

pub fn get_console_width() -> usize {
    let forced = FORCED_CONSOLE_WIDTH.read().ok().and_then(|v| *v);
    if let Some(w) = forced {
        return w;
    }

    if let Some((terminal_size::Width(w), _)) = terminal_size::terminal_size() {
        return (w as usize).max(100);
    }

    138
}

pub fn get_url_without_scheme_and_host(
    url: &str,
    only_when_host: Option<&str>,
    initial_scheme: Option<&str>,
) -> String {
    if let Some(host) = only_when_host {
        let host_marker = format!("://{}", host);
        if !url.contains(&host_marker) {
            return url.to_string();
        }
    }

    if let Some(scheme) = initial_scheme {
        let prefix = format!("{}://", scheme);
        if !url.starts_with(&prefix) {
            return url.to_string();
        }
    }

    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path();
        if let Some(query) = parsed.query() {
            format!("{}?{}", path, query)
        } else {
            path.to_string()
        }
    } else {
        url.to_string()
    }
}

pub fn get_safe_command(command: &str) -> String {
    let patterns = [
        (r"(pass[a-z]{0,5})=\S+", "$1=***"),
        (r"(keys?)=\S+", "$1=***"),
        (r"(secrets?)=\S+", "$1=***"),
        (r"(auth)=\S+", "$1=***"),
    ];

    let mut result = command.to_string();
    for (pattern, replacement) in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }
    result
}

pub fn get_colored_request_time(request_time: f64, str_pad_to: usize) -> String {
    let formatted = get_formatted_duration(request_time);
    let padded = format!("{:<width$}", formatted, width = str_pad_to);

    if request_time >= 2.0 {
        get_color_text(&padded, "red", true)
    } else if request_time >= 1.0 {
        get_color_text(&padded, "magenta", true)
    } else if request_time >= 0.5 {
        get_color_text(&padded, "yellow", false)
    } else {
        get_color_text(&padded, "green", false)
    }
}

pub fn get_colored_status_code(status_code: i32, str_pad_to: usize) -> String {
    if (200..300).contains(&status_code) {
        get_color_text(&format!("{:<width$}", status_code, width = str_pad_to), "green", false)
    } else if (300..400).contains(&status_code) {
        get_color_text(&format!("{:<width$}", status_code, width = str_pad_to), "yellow", true)
    } else if (400..500).contains(&status_code) {
        get_color_text(&format!("{:<width$}", status_code, width = str_pad_to), "magenta", true)
    } else if (500..600).contains(&status_code) {
        get_color_text(&format!("{:<width$}", status_code, width = str_pad_to), "red", true)
    } else {
        get_color_text(
            &format!(
                "{:<width$}",
                get_http_client_code_with_error_description(status_code, true),
                width = str_pad_to
            ),
            "red",
            true,
        )
    }
}

pub fn get_colored_severity(severity: &str) -> String {
    match severity {
        "critical" => get_color_text(severity, "red", true),
        "warning" => get_color_text(severity, "magenta", true),
        "notice" => get_color_text(severity, "blue", false),
        _ => get_color_text(severity, "green", false),
    }
}

pub fn get_colored_criticals(criticals: i32, str_pad_to: usize) -> String {
    if criticals == 0 {
        criticals.to_string()
    } else {
        get_color_text(&format!("{:<width$}", criticals, width = str_pad_to), "red", true)
    }
}

pub fn get_colored_warnings(warnings: i32, str_pad_to: usize) -> String {
    if warnings == 0 {
        warnings.to_string()
    } else {
        get_color_text(&format!("{:<width$}", warnings, width = str_pad_to), "magenta", false)
    }
}

pub fn get_colored_notices(notices: i32, str_pad_to: usize) -> String {
    if notices == 0 {
        notices.to_string()
    } else {
        get_color_text(&format!("{:<width$}", notices, width = str_pad_to), "blue", false)
    }
}

pub fn get_content_type_name_by_id(content_type_id: ContentTypeId) -> &'static str {
    content_type_id.name()
}

pub fn is_href_for_requestable_resource(href: &str) -> bool {
    if href.starts_with('#') {
        return false;
    }
    if href.contains('{') {
        return false;
    }
    if href.contains('<') {
        return false;
    }
    if href.contains("&#") {
        return false;
    }

    // Check if href starts with a scheme that is not http/https
    use once_cell::sync::Lazy;
    static RE_HAS_SCHEME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9]+:").unwrap());
    static RE_IS_HTTP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^https?:/").unwrap());
    let has_scheme = RE_HAS_SCHEME.is_match(href);
    let is_http = RE_IS_HTTP.is_match(href);

    if has_scheme && !is_http {
        return false;
    }

    true
}

pub fn get_absolute_url_by_base_url(base_url: &str, target_url: &str) -> String {
    // Use the url crate for proper resolution
    if let Ok(base) = url::Url::parse(base_url)
        && let Ok(resolved) = base.join(target_url)
    {
        return resolved.to_string();
    }

    // Fallback: return target_url as-is
    target_url.to_string()
}

pub fn get_absolute_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    // On Windows, Path::join() correctly handles drive-relative ("C:foo"),
    // root-relative ("\foo"), and UNC paths ("\\server\share").
    // On Unix, it handles paths starting with "/" by returning them as-is.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    cwd.join(p).to_string_lossy().to_string()
}

pub fn get_output_formatted_path(path: &str) -> String {
    path.to_string()
}

pub fn mb_str_pad(input: &str, pad_length: usize, pad_char: char) -> String {
    let char_count = input.chars().count();
    if char_count >= pad_length {
        input.to_string()
    } else {
        let padding = pad_length - char_count;
        format!(
            "{}{}",
            input,
            std::iter::repeat_n(pad_char, padding).collect::<String>()
        )
    }
}

pub fn strip_javascript(html: &str) -> String {
    let mut result = html.to_string();

    // script tags
    if let Ok(re) = Regex::new(r"(?is)<script[^>]*>.*?</script>") {
        result = re.replace_all(&result, "").to_string();
    }

    // link tags by "href" pointing to .js
    if let Ok(re) = Regex::new(r#"(?is)<link[^>]*href=["'][^"']+\.js[^"']*["'][^>]*>"#) {
        result = re.replace_all(&result, "").to_string();
    }

    // link tags by "as=script"
    if let Ok(re) = Regex::new(r#"(?is)<link[^>]*as=["']script["'][^>]*>"#) {
        result = re.replace_all(&result, "").to_string();
    }

    // on* attributes
    if let Ok(re) = Regex::new(r#"(?is)\s+on[a-z]+=("[^"]*"|'[^']*'|[^\s>]*)"#) {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

pub fn strip_styles(html: &str) -> String {
    let mut result = html.to_string();

    if let Ok(re) = Regex::new(r"(?is)<style\b[^>]*>.*?</style>") {
        result = re.replace_all(&result, "").to_string();
    }

    if let Ok(re) = Regex::new(r#"(?is)<link\b[^>]*rel=["']stylesheet["'][^>]*>"#) {
        result = re.replace_all(&result, "").to_string();
    }

    if let Ok(re) = Regex::new(r#"(?is)\s+style=("[^"]*"|'[^']*'|[^\s>]*)"#) {
        result = re.replace_all(&result, " ").to_string();
    }

    result
}

pub fn strip_fonts(html_or_css: &str) -> String {
    let mut result = html_or_css.to_string();

    if let Ok(re) = Regex::new(r#"(?is)<link\b[^>]*href=["'][^"']+\.(eot|ttf|woff2|woff|otf)[^"']*["'][^>]*>"#) {
        result = re.replace_all(&result, "").to_string();
    }

    if let Ok(re) = Regex::new(r"(?is)@font-face\s*\{[^}]*\}\s*") {
        result = re.replace_all(&result, "").to_string();
    }

    if let Ok(re) = Regex::new(r"(?i)\b(font|font-family)\s*:[^;]+;") {
        result = re.replace_all(&result, "").to_string();
    }

    if let Ok(re) = Regex::new(r#"(?i)\s*style=["']\s*["']"#) {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

pub fn strip_images(html_or_css: &str, placeholder_image: Option<&str>) -> String {
    let placeholder = placeholder_image.unwrap_or(IMG_SRC_TRANSPARENT_1X1_GIF);
    let mut result = html_or_css.to_string();

    let patterns_and_replacements: Vec<(&str, String)> = vec![
        (
            r#"(?is)(<img[^>]+)src=['"][^'"]*['"]([^>]*>)"#,
            format!("${{1}}src=\"{}\"${{2}}", placeholder),
        ),
        (
            r#"(?is)(<img[^>]+)srcset=['"][^'"]*['"]([^>]*>)"#,
            format!("${{1}}srcset=\"{}\"${{2}}", placeholder),
        ),
        (
            r#"(?is)(<source[^>]+)srcset=['"][^'"]*['"]([^>]*>)"#,
            format!("${{1}}srcset=\"{}\"${{2}}", placeholder),
        ),
        (
            r#"(?is)(<source[^>]+)src=['"][^'"]*['"]([^>]*>)"#,
            format!("${{1}}src=\"{}\"${{2}}", placeholder),
        ),
        (
            r#"(?is)url\(\s*['"]?(?!data:)([^'")\s]*\.(?:png|jpe?g|gif|webp|svg|bmp))['"]?\s*\)"#,
            format!("url(\"{}\")", placeholder),
        ),
        (r"(?is)<svg[^>]*>.*?</svg>", String::new()),
    ];

    for (pattern, replacement) in &patterns_and_replacements {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, replacement.as_str()).to_string();
        }
    }

    result
}

pub fn get_colored_cache_lifetime(cache_lifetime: i64, str_pad_to: usize) -> String {
    let color = if cache_lifetime <= 0 {
        "red"
    } else if cache_lifetime < 600 {
        "magenta"
    } else if cache_lifetime <= 86400 {
        "yellow"
    } else {
        "green"
    };

    get_color_text(
        &format!(
            "{:<width$}",
            get_formatted_cache_lifetime(cache_lifetime),
            width = str_pad_to
        ),
        color,
        false,
    )
}

pub fn is_asset_by_content_type(content_type: &str) -> bool {
    let non_asset_content_types = [
        "text/html",
        "application/xhtml+xml",
        "application/xml",
        "application/json",
        "application/ld+json",
        "application/rss+xml",
    ];

    let ct_lower = content_type.to_lowercase();
    for non_asset in &non_asset_content_types {
        if ct_lower.contains(non_asset) {
            return false;
        }
    }
    true
}

pub fn add_class_to_html_images(html: &str, class_name: &str) -> String {
    let mut result = html.to_string();
    if let Ok(re) = Regex::new(r#"(?is)(<img\b)([^>]*>)"#) {
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let tag_start = caps.get(1).map_or("", |m| m.as_str());
                let rest = caps.get(2).map_or("", |m| m.as_str());
                if rest.contains("class=") {
                    format!("{}{}", tag_start, rest)
                } else {
                    format!("{} class=\"{}\"{}", tag_start, class_name, rest)
                }
            })
            .to_string();
    }
    result
}

pub fn get_flat_response_headers(
    headers: &std::collections::HashMap<String, Vec<String>>,
) -> std::collections::HashMap<String, String> {
    headers.iter().map(|(k, v)| (k.clone(), v.join(", "))).collect()
}

/// Returns peak resident memory usage (VmHWM) in bytes by reading /proc/self/status.
/// Returns 0 if the information is not available (e.g., on non-Linux platforms).
pub fn get_peak_memory_usage() -> i64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmHWM:") {
                // Format is "VmHWM:    12345 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2
                    && let Ok(kb) = parts[1].parse::<i64>()
                {
                    return kb * 1024; // convert kB to bytes
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- get_formatted_size --

    #[test]
    fn formatted_size_zero() {
        assert_eq!(get_formatted_size(0, 1), "0.0 B");
    }

    #[test]
    fn formatted_size_bytes() {
        assert_eq!(get_formatted_size(512, 0), "512 B");
    }

    #[test]
    fn formatted_size_kilobytes() {
        assert_eq!(get_formatted_size(1024, 1), "1.0 kB");
    }

    #[test]
    fn formatted_size_megabytes() {
        assert_eq!(get_formatted_size(1_048_576, 1), "1.0 MB");
    }

    #[test]
    fn formatted_size_gigabytes() {
        assert_eq!(get_formatted_size(1_073_741_824, 1), "1.0 GB");
    }

    // -- get_formatted_duration --

    #[test]
    fn formatted_duration_milliseconds() {
        assert_eq!(get_formatted_duration(0.001), "1 ms");
    }

    #[test]
    fn formatted_duration_half_second() {
        assert_eq!(get_formatted_duration(0.5), "500 ms");
    }

    #[test]
    fn formatted_duration_seconds() {
        assert_eq!(get_formatted_duration(1.5), "1.5 s");
    }

    // -- get_formatted_age --

    #[test]
    fn formatted_age_seconds() {
        assert_eq!(get_formatted_age(0), "0 sec(s)");
        assert_eq!(get_formatted_age(59), "59 sec(s)");
    }

    #[test]
    fn formatted_age_minutes() {
        assert_eq!(get_formatted_age(60), "1 min(s)");
    }

    #[test]
    fn formatted_age_hours() {
        assert_eq!(get_formatted_age(3600), "1 hour(s)");
    }

    #[test]
    fn formatted_age_days() {
        assert_eq!(get_formatted_age(86400), "1 day(s)");
    }

    // -- get_formatted_cache_lifetime --

    #[test]
    fn cache_lifetime_seconds() {
        assert_eq!(get_formatted_cache_lifetime(0), "0 s");
    }

    #[test]
    fn cache_lifetime_minutes() {
        assert_eq!(get_formatted_cache_lifetime(60), "1 min");
    }

    #[test]
    fn cache_lifetime_hours() {
        // 3600 is exactly boundary of <= 3600, so still "min"
        assert_eq!(get_formatted_cache_lifetime(3601), "1 h");
    }

    #[test]
    fn cache_lifetime_days() {
        // 86400 is exactly boundary of <= 86400, so still "h"
        assert_eq!(get_formatted_cache_lifetime(86401), "1 d");
    }

    #[test]
    fn cache_lifetime_months() {
        // 86400*90 = 7776000, must exceed that for "mon"
        assert_eq!(get_formatted_cache_lifetime(86400 * 91), "3 mon");
    }

    // -- is_regex_pattern --

    #[test]
    fn regex_pattern_slash_delimited() {
        assert!(is_regex_pattern("/test/i"));
    }

    #[test]
    fn regex_pattern_hash_delimited() {
        assert!(is_regex_pattern("#pat#"));
    }

    #[test]
    fn regex_pattern_plain_text() {
        assert!(!is_regex_pattern("plain"));
    }

    #[test]
    fn regex_pattern_empty() {
        assert!(!is_regex_pattern(""));
    }

    #[test]
    fn regex_pattern_single_slash() {
        assert!(!is_regex_pattern("/"));
    }

    // -- extract_pcre_regex_pattern --

    #[test]
    fn extract_pcre_with_case_insensitive() {
        assert_eq!(extract_pcre_regex_pattern("/hello/i"), "(?i)hello");
    }

    #[test]
    fn extract_pcre_hash_delimiter() {
        assert_eq!(extract_pcre_regex_pattern("#test#"), "test");
    }

    #[test]
    fn extract_pcre_tilde_with_flags() {
        // Only 'i' flag is converted to (?i); other flags are silently ignored
        let result = extract_pcre_regex_pattern("~foo~ms");
        assert_eq!(result, "foo");
    }

    // -- strip_javascript --

    #[test]
    fn strip_javascript_removes_script_tags() {
        let input = "<p>ok</p><script>alert(1)</script>";
        assert_eq!(strip_javascript(input), "<p>ok</p>");
    }

    // -- strip_styles --

    #[test]
    fn strip_styles_removes_style_tags() {
        let input = "<p>ok</p><style>.x{}</style>";
        assert_eq!(strip_styles(input), "<p>ok</p>");
    }

    // -- mb_str_pad --

    #[test]
    fn str_pad_shorter_input() {
        assert_eq!(mb_str_pad("hi", 5, ' '), "hi   ");
    }

    #[test]
    fn str_pad_longer_input() {
        assert_eq!(mb_str_pad("long", 2, ' '), "long");
    }

    // -- is_href_for_requestable_resource --

    #[test]
    fn requestable_http_url() {
        assert!(is_href_for_requestable_resource("https://x.com"));
    }

    #[test]
    fn requestable_javascript_void() {
        assert!(!is_href_for_requestable_resource("javascript:void(0)"));
    }

    #[test]
    fn requestable_mailto() {
        assert!(!is_href_for_requestable_resource("mailto:a@b.c"));
    }

    #[test]
    fn requestable_data_uri() {
        assert!(!is_href_for_requestable_resource("data:text/html"));
    }

    // -- get_absolute_url_by_base_url --

    #[test]
    fn absolute_url_from_root_relative() {
        assert_eq!(
            get_absolute_url_by_base_url("https://x.com/a/", "/b"),
            "https://x.com/b"
        );
    }

    #[test]
    fn absolute_url_from_relative() {
        assert_eq!(
            get_absolute_url_by_base_url("https://x.com/a/", "c"),
            "https://x.com/a/c"
        );
    }
}
