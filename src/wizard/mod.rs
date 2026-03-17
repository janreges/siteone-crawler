// SiteOne Crawler - Interactive wizard for no-args invocation
// (c) Jan Reges <jan.reges@siteone.cz>

mod form;
mod presets;

use colored::Colorize;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use inquire::validator::Validation;
use inquire::{Confirm, InquireError, Select, Text};
use std::io::IsTerminal;

use crate::version;
use presets::{PRESETS, WizardState};

// ── Public API ──────────────────────────────────────────────────────────────

/// Returns true when stdin AND stdout are interactive TTYs.
pub fn is_interactive_tty() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// After an offline/markdown export crawl (wizard mode), offer to immediately
/// serve the exported content via HTTP.
/// Returns `Some((dir_path, "offline"|"markdown"))` if the user confirms, else `None`.
pub fn offer_serve_after_export(crawl_argv: &[String]) -> Option<(String, String)> {
    let (dir, kind) = if let Some(arg) = crawl_argv.iter().find(|a| a.starts_with("--offline-export-dir=")) {
        let raw = arg.trim_start_matches("--offline-export-dir=");
        (raw.trim_matches('\'').to_string(), "offline")
    } else if let Some(arg) = crawl_argv.iter().find(|a| a.starts_with("--markdown-export-dir=")) {
        let raw = arg.trim_start_matches("--markdown-export-dir=");
        (raw.trim_matches('\'').to_string(), "markdown")
    } else {
        return None;
    };

    println!();
    let confirmed = Confirm::new(&format!("Serve the {} export via HTTP?", kind))
        .with_default(true)
        .prompt()
        .unwrap_or(false);

    if confirmed { Some((dir, kind.to_string())) } else { None }
}

/// Block until the user presses Enter. Used after a wizard-launched crawl
/// so the terminal window stays open (especially on Windows double-click).
pub fn press_enter_to_exit() {
    println!();
    println!("{}", "Press Enter to exit...".dimmed());
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}

/// Run the interactive wizard. Returns a synthetic argv `Vec<String>` ready
/// to be fed into `Initiator::new()` / `parse_argv()`.
pub fn run_wizard() -> Result<Vec<String>, WizardError> {
    // Set inquire theme: yellow accents instead of default cyan, gray help text
    let mut render_config = RenderConfig::default_colored();
    render_config.help_message = StyleSheet::new().with_fg(Color::DarkGrey);
    render_config.highlighted_option_prefix = Styled::new("❯").with_fg(Color::DarkYellow);
    render_config.answer = StyleSheet::new().with_fg(Color::DarkYellow);
    inquire::set_global_render_config(render_config);

    print_banner();

    // Step 1: Preset selection (+ dynamic serve items if exports exist)
    let choice = prompt_preset_or_serve()?;
    match choice {
        PresetChoice::Serve(argv) => Ok(argv),
        PresetChoice::Preset(preset_idx) => {
            let preset = &PRESETS[preset_idx];
            let mut state = WizardState::from_preset(preset);

            // Step 2: URL (required)
            state.url = prompt_url()?;

            // Resolve {domain} and {date} placeholders in export paths
            resolve_export_paths(&mut state);

            // Step 3: Interactive settings form (arrow-key navigation + value cycling)
            let mut settings = form::build_form_settings(&state);
            println!();

            // Show warning for Stress Test preset
            if preset.name == "Stress Test" {
                println!(
                    "  {} {}",
                    "WARNING:".yellow().bold(),
                    "Stress testing generates high-concurrency load with cache-busting".yellow()
                );
                println!(
                    "           {}",
                    "random query params. This can overload a server and cause downtime.".yellow()
                );
                println!(
                    "           {}",
                    "Only run this against your own websites or with explicit permission!"
                        .yellow()
                        .bold()
                );
                println!();
            }

            let confirmed = form::run_form(&mut settings, preset.name)?;
            if !confirmed {
                return Err(WizardError::Cancelled);
            }
            form::apply_form_to_state(&settings, &mut state);

            // Re-resolve export paths — apply_form_to_state may have reset them to templates
            resolve_export_paths(&mut state);

            // Step 4: Summary & confirm
            let argv = state.build_argv();
            print_summary(&state, &argv);

            let run = Confirm::new("Start the crawl?").with_default(true).prompt()?;

            if run {
                println!();
                Ok(argv)
            } else {
                Err(WizardError::Cancelled)
            }
        }
    }
}

// ── Preset or Serve choice ──────────────────────────────────────────────────

enum PresetChoice {
    Preset(usize),
    Serve(Vec<String>),
}

/// Separator label used in the menu to visually separate serve items.
const SERVE_SEPARATOR: &str = "──────────────────────────────────────";

fn prompt_preset_or_serve() -> Result<PresetChoice, WizardError> {
    let mut labels: Vec<String> = PRESETS.iter().map(|p| p.to_string()).collect();

    // Detect existing exports in ./tmp/
    let offline_dirs = find_export_dirs("offline");
    let markdown_dirs = find_export_dirs("markdown");

    let has_serve_items = !offline_dirs.is_empty() || !markdown_dirs.is_empty();
    let serve_offline_label = "Browse offline export     Serve a previously exported offline site via HTTP";
    let serve_markdown_label = "Browse markdown export    Serve a previously exported markdown site via HTTP";

    if has_serve_items {
        labels.push(SERVE_SEPARATOR.to_string());
        if !offline_dirs.is_empty() {
            labels.push(serve_offline_label.to_string());
        }
        if !markdown_dirs.is_empty() {
            labels.push(serve_markdown_label.to_string());
        }
    }

    let choice = Select::new("Choose a crawl mode:", labels.clone())
        .with_page_size(labels.len())
        .prompt()?;

    // Check if it's a serve option
    if choice == serve_offline_label {
        return prompt_serve_export(&offline_dirs, "offline");
    }
    if choice == serve_markdown_label {
        return prompt_serve_export(&markdown_dirs, "markdown");
    }
    if choice == SERVE_SEPARATOR {
        // User selected the separator — re-prompt
        return prompt_preset_or_serve();
    }

    // It's a preset
    let preset_idx = PRESETS.iter().position(|p| choice.starts_with(p.name)).unwrap_or(0);
    Ok(PresetChoice::Preset(preset_idx))
}

/// Prompt the user to select from available exports, then return serve argv.
fn prompt_serve_export(dirs: &[ExportDir], kind: &str) -> Result<PresetChoice, WizardError> {
    let labels: Vec<String> = dirs.iter().map(|d| format!("{:40} {}", d.name, d.date_label)).collect();

    let choice = Select::new(&format!("Select {} export to serve:", kind), labels).prompt()?;

    // Find matching dir
    let selected = dirs.iter().find(|d| choice.starts_with(&d.name)).unwrap();

    let serve_flag = match kind {
        "offline" => format!("--serve-offline={}", selected.path),
        _ => format!("--serve-markdown={}", selected.path),
    };

    Ok(PresetChoice::Serve(vec!["siteone-crawler".to_string(), serve_flag]))
}

// ── Export directory detection ───────────────────────────────────────────────

struct ExportDir {
    name: String,
    path: String,
    date_label: String,
}

/// Find export directories matching `./tmp/{kind}-*/` pattern, sorted newest first.
fn find_export_dirs(kind: &str) -> Vec<ExportDir> {
    let tmp_path = std::path::Path::new("./tmp");
    if !tmp_path.is_dir() {
        return Vec::new();
    }

    let prefix = format!("{}-", kind);
    let mut dirs: Vec<ExportDir> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(tmp_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) {
                continue;
            }

            // Extract a human-readable date from metadata
            let date_label = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M").to_string()
                })
                .unwrap_or_default();

            dirs.push(ExportDir {
                name,
                path: path.to_string_lossy().to_string(),
                date_label,
            });
        }
    }

    // Sort newest first (by name descending — names contain date)
    dirs.sort_by(|a, b| b.name.cmp(&a.name));
    dirs
}

// ── Export path resolution ──────────────────────────────────────────────────

/// Replace `{domain}` and `{date}` placeholders in export dir paths after URL is known.
fn resolve_export_paths(state: &mut WizardState) {
    let url = &state.url;
    if let Some(ref dir) = state.offline_export_dir
        && (dir.contains("{domain}") || dir.contains("{date}"))
    {
        state.offline_export_dir = Some(presets::resolve_export_path(dir, url));
    }
    if let Some(ref dir) = state.markdown_export_dir
        && (dir.contains("{domain}") || dir.contains("{date}"))
    {
        state.markdown_export_dir = Some(presets::resolve_export_path(dir, url));
    }
}

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum WizardError {
    Cancelled,
    IoError(String),
}

impl std::fmt::Display for WizardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WizardError::Cancelled => write!(f, "Wizard cancelled."),
            WizardError::IoError(msg) => write!(f, "Wizard error: {}", msg),
        }
    }
}

impl From<InquireError> for WizardError {
    fn from(err: InquireError) -> Self {
        match err {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => WizardError::Cancelled,
            other => WizardError::IoError(other.to_string()),
        }
    }
}

// ── Banner ──────────────────────────────────────────────────────────────────

fn print_banner() {
    let separator = "=".repeat(60);
    println!();
    println!("{}", separator.dimmed());
    println!(
        "  {} {}",
        "SiteOne Crawler".bold(),
        format!("v{}", version::CODE).dimmed()
    );
    println!(
        "  {}",
        "Website QA toolkit: audit, clone, export, sitemap, CI/CD".dimmed()
    );
    println!("{}", separator.dimmed());
    println!();
}

// ── URL prompt ──────────────────────────────────────────────────────────────

fn prompt_url() -> Result<String, WizardError> {
    let url = Text::new("Enter the website URL to crawl:")
        .with_placeholder("https://example.com")
        .with_help_message("Enter a domain (e.g. example.com) or full URL (https://...)")
        .with_validator(|input: &str| {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                return Ok(Validation::Invalid("URL is required.".into()));
            }
            let url_str = normalize_url_input(trimmed);
            match url::Url::parse(&url_str) {
                Ok(u) if (u.scheme() == "http" || u.scheme() == "https") && u.host().is_some() => Ok(Validation::Valid),
                _ => Ok(Validation::Invalid(
                    "Invalid URL. Enter a domain name or a valid http(s) address.".into(),
                )),
            }
        })
        .prompt()?;

    Ok(normalize_url_input(url.trim()))
}

fn normalize_url_input(input: &str) -> String {
    let trimmed = input.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

// ── Summary ─────────────────────────────────────────────────────────────────

fn print_summary(state: &WizardState, argv: &[String]) {
    println!();
    let separator = "=".repeat(60);
    println!("{}", separator.dimmed());
    println!("  {}", "Configuration Summary".bold());
    println!("{}", separator.dimmed());
    println!();

    let label_width = 22;

    print_row("URL:", &state.url, label_width);
    print_row("Preset:", &state.preset_name, label_width);
    print_row("Workers:", &state.workers.to_string(), label_width);
    print_row("Timeout:", &format!("{}s", state.timeout), label_width);
    let rate_limit = if state.max_reqs_per_sec == 0 {
        "unlimited".to_string()
    } else {
        format!("{}/s", state.max_reqs_per_sec)
    };
    print_row("Rate limit:", &rate_limit, label_width);

    let max_urls = if state.max_visited_urls == 0 {
        "unlimited".to_string()
    } else {
        state.max_visited_urls.to_string()
    };
    print_row("Max URLs:", &max_urls, label_width);
    print_row("Device:", &state.device, label_width);
    print_row("Content types:", &state.content_summary(), label_width);

    if state.single_page {
        print_row("Scope:", "single page", label_width);
    }
    if let Some(ref dir) = state.offline_export_dir {
        print_row("Offline export:", dir, label_width);
    }
    if let Some(ref dir) = state.markdown_export_dir {
        print_row("Markdown export:", dir, label_width);
    }
    if let Some(ref file) = state.sitemap_xml_file {
        print_row("Sitemap XML:", file, label_width);
    }
    if let Some(ref cols) = state.extra_columns {
        print_row("Extra columns:", cols, label_width);
    }
    if !state.http_cache_enabled {
        print_row("HTTP cache:", "disabled", label_width);
    }
    if state.result_storage_file {
        print_row("Storage:", "file", label_width);
    }
    if state.ignore_robots_txt {
        print_row("Robots.txt:", "ignored", label_width);
    }
    if state.http_auth.is_some() {
        print_row("HTTP auth:", "configured", label_width);
    }
    if let Some(ref proxy) = state.proxy {
        print_row("Proxy:", proxy, label_width);
    }

    // Show generated CLI command
    println!();
    println!("  {}", "Equivalent CLI command:".yellow());
    let cmd = argv[1..].join(" \\\n    ");
    println!("  {} {}", "siteone-crawler".yellow(), cmd.yellow());
    println!();
    println!("  {}", "Tip: Copy this command to skip the wizard next time.".dimmed());
    println!();
}

fn print_row(label: &str, value: &str, label_width: usize) {
    println!("  {:<width$} {}", label.dimmed(), value, width = label_width);
}
