// SiteOne Crawler - BaseExporter (shared helpers for all exporters)
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::fs;
use std::path::Path;

use chrono::Local;
use regex::Regex;

use crate::error::{CrawlerError, CrawlerResult};

/// Get the export file path with optional host and timestamp suffixes.
///
/// - If the file has no extension, the given `default_extension` is appended.
/// - If `add_host` is true, the host is inserted before the extension.
/// - If `add_timestamp` is true, a timestamp is inserted before the extension.
pub fn get_export_file_path(
    file: &str,
    default_extension: &str,
    add_host: bool,
    host: Option<&str>,
    add_timestamp: bool,
) -> CrawlerResult<String> {
    let mut file = file.to_string();

    // Add default extension if missing
    let has_extension = Regex::new(r"\.[a-zA-Z0-9]{1,10}$")
        .map(|re| re.is_match(&file))
        .unwrap_or(false);
    if !has_extension {
        file = format!("{}.{}", file, default_extension);
    }

    // Add host before extension
    if add_host
        && let Some(h) = host
        && let Ok(re) = Regex::new(r"\.[a-zA-Z0-9]{1,10}$")
    {
        file = re
            .replace(&file, |caps: &regex::Captures| {
                format!(".{}{}", h, caps.get(0).map_or("", |m| m.as_str()))
            })
            .to_string();
    }

    // Add timestamp before extension
    if add_timestamp {
        let timestamp = Local::now().format("%Y-%m-%d.%H-%M-%S").to_string();
        if let Ok(re) = Regex::new(r"\.[a-zA-Z0-9]{1,10}$") {
            file = re
                .replace(&file, |caps: &regex::Captures| {
                    format!(".{}{}", timestamp, caps.get(0).map_or("", |m| m.as_str()))
                })
                .to_string();
        }
    }

    // Ensure parent directory exists and is writable
    let path = Path::new(&file);
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|e| {
            CrawlerError::Export(format!("Cannot create output directory '{}': {}", parent.display(), e))
        })?;
    }

    Ok(file)
}
