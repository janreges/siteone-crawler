// SiteOne Crawler - Debugger
// (c) Jan Reges <jan.reges@siteone.cz>

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::RwLock;

use crate::utils;

pub const DEBUG: &str = "debug";
pub const INFO: &str = "info";
pub const NOTICE: &str = "notice";
pub const WARNING: &str = "warning";
pub const CRITICAL: &str = "critical";

static DEBUG_ENABLED: RwLock<bool> = RwLock::new(false);
static DEBUG_PRINT_TO_OUTPUT: RwLock<bool> = RwLock::new(false);
static DEBUG_LOG_FILE: RwLock<Option<String>> = RwLock::new(None);

pub fn debug(category: &str, message: &str, severity: &str, time: Option<f64>, size: Option<i64>) {
    let enabled = DEBUG_ENABLED.read().map(|v| *v).unwrap_or(false);
    if !enabled {
        return;
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut final_message = format!("{} | {:8} | {:14} | ", now, severity, category,);

    if let Some(t) = time {
        final_message.push_str(&format!("{:7} | ", utils::get_formatted_duration(t)));
    }
    if let Some(s) = size {
        final_message.push_str(&format!("{:7} | ", utils::get_formatted_size(s, 0)));
    }

    final_message.push_str(message);

    print_debug(&final_message);
    log_debug(&final_message);
}

pub fn console_array_debug(row_data: &[String], col_widths: &[usize]) {
    let enabled = DEBUG_ENABLED.read().map(|v| *v).unwrap_or(false);
    if !enabled {
        return;
    }

    let console_width = utils::get_console_width();
    let widths: Vec<usize> = if col_widths.is_empty() {
        let col_width = console_width / row_data.len();
        vec![col_width.max(10); row_data.len()]
    } else {
        col_widths.iter().map(|w| (*w).max(10)).collect()
    };

    let mut row = Vec::new();
    for (i, value) in row_data.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(10);
        let val = if value.len() > w {
            utils::truncate_in_two_thirds(value, w, "..", None)
        } else {
            format!("{:<width$}", value, width = w)
        };
        row.push(val);
    }

    let message = row.join(" | ");
    print_debug(&message);
    log_debug(&message);
}

pub fn force_enabled_debug(log_file: Option<&str>) {
    if let Ok(mut d) = DEBUG_ENABLED.write() {
        *d = true;
    }
    if let Ok(mut p) = DEBUG_PRINT_TO_OUTPUT.write() {
        *p = true;
    }
    if let Some(f) = log_file
        && let Ok(mut lf) = DEBUG_LOG_FILE.write()
    {
        *lf = Some(f.to_string());
    }
}

pub fn set_config(debug_enabled: bool, debug_log_file: Option<&str>) {
    if debug_enabled {
        if let Ok(mut d) = DEBUG_ENABLED.write() {
            *d = true;
        }
        if let Ok(mut p) = DEBUG_PRINT_TO_OUTPUT.write() {
            *p = true;
        }
        if let Some(f) = debug_log_file
            && let Ok(mut lf) = DEBUG_LOG_FILE.write()
        {
            *lf = Some(f.to_string());
        }
    } else if debug_log_file.is_some() {
        // when debug is disabled but debugLogFile is set, logging to file is enabled but printing to output is not
        if let Ok(mut d) = DEBUG_ENABLED.write() {
            *d = true;
        }
        if let Ok(mut p) = DEBUG_PRINT_TO_OUTPUT.write() {
            *p = false;
        }
        if let Some(f) = debug_log_file
            && let Ok(mut lf) = DEBUG_LOG_FILE.write()
        {
            *lf = Some(f.to_string());
        }
    }
}

fn print_debug(message: &str) {
    let should_print = DEBUG_PRINT_TO_OUTPUT.read().map(|v| *v).unwrap_or(false);
    if should_print {
        println!("{}", message);
    }
}

fn log_debug(message: &str) {
    let log_file = DEBUG_LOG_FILE.read().ok().and_then(|v| v.clone());
    if let Some(path) = log_file {
        let abs_path = utils::get_absolute_path(&path);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&abs_path) {
            let _ = writeln!(file, "{}", message);
        }
    }
}
