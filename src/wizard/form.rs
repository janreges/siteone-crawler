// SiteOne Crawler - Interactive settings form with arrow-key cycling
// (c) Jan Reges <jan.reges@siteone.cz>

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

use super::WizardError;
use super::presets::WizardState;

// ── FormSetting ─────────────────────────────────────────────────────────────

pub struct FormSetting {
    pub label: &'static str,
    pub options: Vec<&'static str>,
    pub current: usize,
}

impl FormSetting {
    fn new(label: &'static str, options: Vec<&'static str>, default: &str) -> Self {
        let current = options.iter().position(|o| *o == default).unwrap_or(0);
        FormSetting {
            label,
            options,
            current,
        }
    }

    pub fn value(&self) -> &str {
        self.options[self.current]
    }

    fn cycle_right(&mut self) {
        self.current = (self.current + 1) % self.options.len();
    }

    fn cycle_left(&mut self) {
        if self.current == 0 {
            self.current = self.options.len() - 1;
        } else {
            self.current -= 1;
        }
    }
}

// ── Setting indices (order in the form) ─────────────────────────────────────

const S_TIMEOUT: usize = 0;
const S_WORKERS: usize = 1;
const S_MAX_RPS: usize = 2;
const S_MAX_URLS: usize = 3;
const S_DEVICE: usize = 4;
const S_JAVASCRIPT: usize = 5;
const S_CSS: usize = 6;
const S_FONTS: usize = 7;
const S_IMAGES: usize = 8;
const S_FILES: usize = 9;
const S_SINGLE_PAGE: usize = 10;
const S_OFFLINE: usize = 11;
const S_MARKDOWN: usize = 12;
const S_SITEMAP: usize = 13;
const S_CACHE: usize = 14;
const S_STORAGE: usize = 15;
const S_ROBOTS: usize = 16;

// ── Build form from WizardState ─────────────────────────────────────────────

pub fn build_form_settings(state: &WizardState) -> Vec<FormSetting> {
    vec![
        // Performance & Limits
        FormSetting::new(
            "Timeout",
            vec!["1s", "2s", "3s", "5s", "10s", "30s", "60s"],
            format_static_timeout(state.timeout),
        ),
        FormSetting::new(
            "Workers",
            vec!["1", "2", "3", "5", "8", "10", "20", "50"],
            format_static_workers(state.workers),
        ),
        FormSetting::new(
            "Max requests/sec",
            vec!["unlimited", "5/s", "10/s", "20/s", "50/s", "100/s", "500/s"],
            format_static_rps(state.max_reqs_per_sec),
        ),
        FormSetting::new(
            "Max visited URLs",
            vec!["unlimited", "100", "500", "1000", "5000", "10000", "50000", "100000"],
            format_static_max_urls(state.max_visited_urls),
        ),
        // Device
        FormSetting::new("Device", vec!["desktop", "mobile", "tablet"], &state.device),
        // Content types
        FormSetting::new(
            "JavaScript",
            vec!["yes", "no"],
            if state.disable_javascript { "no" } else { "yes" },
        ),
        FormSetting::new(
            "CSS stylesheets",
            vec!["yes", "no"],
            if state.disable_styles { "no" } else { "yes" },
        ),
        FormSetting::new(
            "Fonts",
            vec!["yes", "no"],
            if state.disable_fonts { "no" } else { "yes" },
        ),
        FormSetting::new(
            "Images",
            vec!["yes", "no"],
            if state.disable_images { "no" } else { "yes" },
        ),
        FormSetting::new(
            "Files (PDFs, ZIPs..)",
            vec!["yes", "no"],
            if state.disable_files { "no" } else { "yes" },
        ),
        // Scope
        FormSetting::new(
            "Single page only",
            vec!["no", "yes"],
            if state.single_page { "yes" } else { "no" },
        ),
        // Generators
        FormSetting::new(
            "Offline export",
            vec!["disabled", "./tmp/"],
            if state.offline_export_dir.is_some() {
                "./tmp/"
            } else {
                "disabled"
            },
        ),
        FormSetting::new(
            "Markdown export",
            vec!["disabled", "./tmp/"],
            if state.markdown_export_dir.is_some() {
                "./tmp/"
            } else {
                "disabled"
            },
        ),
        FormSetting::new(
            "Sitemap XML",
            vec!["disabled", "./sitemap.xml"],
            if state.sitemap_xml_file.is_some() {
                "./sitemap.xml"
            } else {
                "disabled"
            },
        ),
        // Caching
        FormSetting::new(
            "HTTP caching",
            vec!["enabled", "disabled"],
            if state.http_cache_enabled {
                "enabled"
            } else {
                "disabled"
            },
        ),
        FormSetting::new(
            "Data storage",
            vec!["memory", "file"],
            if state.result_storage_file { "file" } else { "memory" },
        ),
        // Advanced
        FormSetting::new(
            "Ignore robots.txt",
            vec!["no", "yes"],
            if state.ignore_robots_txt { "yes" } else { "no" },
        ),
    ]
}

// Match default values to the closest available option
fn format_static_timeout(val: u32) -> &'static str {
    match val {
        0..=1 => "1s",
        2 => "2s",
        3..=4 => "3s",
        5..=9 => "5s",
        10..=29 => "10s",
        30..=59 => "30s",
        _ => "60s",
    }
}

fn format_static_workers(val: u32) -> &'static str {
    match val {
        0..=1 => "1",
        2 => "2",
        3..=4 => "3",
        5..=7 => "5",
        8..=9 => "8",
        10..=19 => "10",
        20..=49 => "20",
        _ => "50",
    }
}

fn format_static_rps(val: u32) -> &'static str {
    match val {
        0 => "unlimited",
        1..=7 => "5/s",
        8..=14 => "10/s",
        15..=34 => "20/s",
        35..=74 => "50/s",
        75..=299 => "100/s",
        _ => "500/s",
    }
}

fn format_static_max_urls(val: u32) -> &'static str {
    match val {
        0 => "unlimited",
        1..=299 => "100",
        300..=749 => "500",
        750..=2499 => "1000",
        2500..=7499 => "5000",
        7500..=29999 => "10000",
        30000..=74999 => "50000",
        _ => "100000",
    }
}

// ── Apply form values back to WizardState ───────────────────────────────────

pub fn apply_form_to_state(settings: &[FormSetting], state: &mut WizardState) {
    // Timeout
    state.timeout = parse_timeout(settings[S_TIMEOUT].value());
    // Workers
    state.workers = settings[S_WORKERS].value().parse().unwrap_or(3);
    // Max req/s
    state.max_reqs_per_sec = parse_rps(settings[S_MAX_RPS].value());
    // Max URLs
    state.max_visited_urls = parse_max_urls(settings[S_MAX_URLS].value());
    // Device
    state.device = settings[S_DEVICE].value().to_string();
    // Content types
    state.disable_javascript = settings[S_JAVASCRIPT].value() == "no";
    state.disable_styles = settings[S_CSS].value() == "no";
    state.disable_fonts = settings[S_FONTS].value() == "no";
    state.disable_images = settings[S_IMAGES].value() == "no";
    state.disable_files = settings[S_FILES].value() == "no";
    // Scope
    state.single_page = settings[S_SINGLE_PAGE].value() == "yes";
    // Generators
    state.offline_export_dir = if settings[S_OFFLINE].value() == "disabled" {
        None
    } else {
        Some("./tmp/offline-{domain}-{date}/".to_string())
    };
    state.markdown_export_dir = if settings[S_MARKDOWN].value() == "disabled" {
        None
    } else {
        Some("./tmp/markdown-{domain}-{date}/".to_string())
    };
    state.sitemap_xml_file = if settings[S_SITEMAP].value() == "disabled" {
        None
    } else {
        Some(settings[S_SITEMAP].value().to_string())
    };
    // Caching
    state.http_cache_enabled = settings[S_CACHE].value() == "enabled";
    state.result_storage_file = settings[S_STORAGE].value() == "file";
    // Advanced
    state.ignore_robots_txt = settings[S_ROBOTS].value() == "yes";
}

fn parse_timeout(val: &str) -> u32 {
    val.strip_suffix('s').and_then(|n| n.parse().ok()).unwrap_or(5)
}

fn parse_rps(val: &str) -> u32 {
    if val == "unlimited" {
        0
    } else {
        val.strip_suffix("/s").and_then(|n| n.parse().ok()).unwrap_or(10)
    }
}

fn parse_max_urls(val: &str) -> u32 {
    if val == "unlimited" {
        0
    } else {
        val.parse().unwrap_or(10000)
    }
}

// ── Interactive form loop ───────────────────────────────────────────────────

/// Run the interactive settings form. Returns Ok(true) on confirm, Ok(false) on cancel.
pub fn run_form(settings: &mut [FormSetting], preset_name: &str) -> Result<bool, WizardError> {
    let mut stdout = io::stdout();
    let mut cursor_idx: usize = 0;

    // Get current cursor position for drawing
    let start_row = cursor::position().map(|(_, row)| row).unwrap_or(0);

    terminal::enable_raw_mode().map_err(|e: std::io::Error| WizardError::IoError(e.to_string()))?;
    execute!(stdout, cursor::Hide).ok();

    // Drain any leftover key events (e.g. Enter release from the previous prompt)
    std::thread::sleep(std::time::Duration::from_millis(100));
    while event::poll(std::time::Duration::from_millis(50))
        .map_err(|e: std::io::Error| WizardError::IoError(e.to_string()))?
    {
        let _ = event::read();
    }

    let (result, final_start_row) = form_event_loop(settings, &mut cursor_idx, start_row, &mut stdout, preset_name);

    // Always restore terminal
    execute!(stdout, cursor::Show).ok();
    terminal::disable_raw_mode().ok();

    // Move past the form area using the scroll-adjusted start row
    let total_rows = settings.len() as u16 + 5;
    execute!(stdout, cursor::MoveTo(0, final_start_row + total_rows)).ok();
    println!();

    result
}

fn form_event_loop(
    settings: &mut [FormSetting],
    cursor_idx: &mut usize,
    mut start_row: u16,
    stdout: &mut io::Stdout,
    preset_name: &str,
) -> (Result<bool, WizardError>, u16) {
    render_form(settings, *cursor_idx, &mut start_row, stdout, preset_name);

    // Ignore Enter events that arrive within a short window after form start,
    // to prevent a stale Enter from the previous inquire prompt from confirming immediately.
    let form_start = std::time::Instant::now();
    let debounce = std::time::Duration::from_millis(300);

    loop {
        match event::read().map_err(|e: std::io::Error| WizardError::IoError(e.to_string())) {
            Err(e) => return (Err(e), start_row),
            Ok(Event::Key(key)) => {
                // Only react to Press events; ignore Release and Repeat to avoid double-firing
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if *cursor_idx > 0 {
                            *cursor_idx -= 1;
                        } else {
                            *cursor_idx = settings.len() - 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *cursor_idx = (*cursor_idx + 1) % settings.len();
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        settings[*cursor_idx].cycle_left();
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        settings[*cursor_idx].cycle_right();
                    }
                    KeyCode::Enter => {
                        if form_start.elapsed() >= debounce {
                            return (Ok(true), start_row);
                        }
                        continue; // ignore stale Enter from previous prompt
                    }
                    KeyCode::Esc | KeyCode::Char('q') => return (Ok(false), start_row),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return (Err(WizardError::Cancelled), start_row);
                    }
                    _ => continue, // skip re-render for unknown keys
                }
                render_form(settings, *cursor_idx, &mut start_row, stdout, preset_name);
            }
            Ok(_) => continue, // ignore non-key events
        }
    }
}

fn render_form(
    settings: &[FormSetting],
    cursor_idx: usize,
    start_row: &mut u16,
    stdout: &mut io::Stdout,
    preset_name: &str,
) {
    execute!(stdout, cursor::MoveTo(0, *start_row), Clear(ClearType::FromCursorDown)).ok();

    let label_w = 22;
    let val_w = 18;

    // Header
    write_line(
        stdout,
        &format!("\x1b[1m  Settings\x1b[0m \x1b[90m(preset: {})\x1b[0m", preset_name),
    );
    write_line(
        stdout,
        "  \x1b[33mUp/Down\x1b[90m = navigate  \x1b[33mLeft/Right\x1b[90m = change value  \x1b[33mEnter\x1b[90m = confirm  \x1b[33mEsc\x1b[90m = cancel\x1b[0m",
    );
    write_line(stdout, "");

    for (i, setting) in settings.iter().enumerate() {
        let is_focused = i == cursor_idx;
        let val = setting.value();

        if is_focused {
            // Focused: yellow arrow, bold label, yellow value with < >
            write!(
                stdout,
                "  \x1b[33m>\x1b[0m \x1b[1m{:<lw$}\x1b[0m \x1b[33m<\x1b[0m \x1b[1;33m{:^vw$}\x1b[0m \x1b[33m>\x1b[0m\r\n",
                setting.label,
                val,
                lw = label_w,
                vw = val_w,
            )
            .ok();
        } else {
            // Normal: dimmed value
            write!(
                stdout,
                "    {:<lw$} \x1b[90m{:^vw$}\x1b[0m\r\n",
                setting.label,
                val,
                lw = label_w,
                vw = val_w,
            )
            .ok();
        }
    }

    write_line(stdout, "");

    stdout.flush().ok();

    // Recalculate start_row in case terminal scrolled (e.g. form near bottom of window).
    // Total lines: header + help + blank + settings + trailing blank = settings.len() + 4
    let total_lines = settings.len() as u16 + 4;
    if let Ok((_, current_row)) = cursor::position() {
        *start_row = current_row.saturating_sub(total_lines);
    }
}

fn write_line(stdout: &mut io::Stdout, text: &str) {
    write!(stdout, "{}\r\n", text).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timeout_values() {
        assert_eq!(parse_timeout("1s"), 1);
        assert_eq!(parse_timeout("5s"), 5);
        assert_eq!(parse_timeout("60s"), 60);
    }

    #[test]
    fn parse_rps_values() {
        assert_eq!(parse_rps("unlimited"), 0);
        assert_eq!(parse_rps("10/s"), 10);
        assert_eq!(parse_rps("100/s"), 100);
    }

    #[test]
    fn parse_max_urls_values() {
        assert_eq!(parse_max_urls("unlimited"), 0);
        assert_eq!(parse_max_urls("10000"), 10000);
    }

    #[test]
    fn format_static_timeout_snaps() {
        assert_eq!(format_static_timeout(1), "1s");
        assert_eq!(format_static_timeout(4), "3s");
        assert_eq!(format_static_timeout(5), "5s");
        assert_eq!(format_static_timeout(15), "10s");
        assert_eq!(format_static_timeout(100), "60s");
    }

    #[test]
    fn format_static_workers_snaps() {
        assert_eq!(format_static_workers(1), "1");
        assert_eq!(format_static_workers(3), "3");
        assert_eq!(format_static_workers(5), "5");
        assert_eq!(format_static_workers(10), "10");
    }

    #[test]
    fn cycle_wraps_around() {
        let mut s = FormSetting::new("test", vec!["a", "b", "c"], "a");
        assert_eq!(s.current, 0);
        s.cycle_left();
        assert_eq!(s.current, 2); // wraps to last
        s.cycle_right();
        assert_eq!(s.current, 0); // wraps to first
    }

    #[test]
    fn apply_form_roundtrip() {
        let mut state = WizardState::from_preset(&super::super::presets::PRESETS[0]); // Quick audit
        let settings = build_form_settings(&state);
        // Apply unchanged form back → state should match
        apply_form_to_state(&settings, &mut state);
        assert_eq!(state.workers, 5);
        assert_eq!(state.timeout, 5);
        assert!(!state.disable_javascript);
    }
}
