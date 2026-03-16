// SiteOne Crawler - Summary Item
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};

use crate::components::summary::item_status::ItemStatus;
use crate::utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub apl_code: String,
    pub text: String,
    pub status: ItemStatus,
}

impl Item {
    pub fn new(apl_code: String, text: String, status: ItemStatus) -> Self {
        Self { apl_code, text, status }
    }

    pub fn get_as_html(&self) -> String {
        let icon = match self.status {
            ItemStatus::Ok => "\u{2705}",              // checkmark
            ItemStatus::Notice => "\u{23E9}",          // fast forward
            ItemStatus::Warning => "\u{26A0}\u{FE0F}", // warning
            ItemStatus::Critical => "\u{26D4}",        // no entry
            ItemStatus::Info => "\u{1F4CC}",           // pushpin
        };

        let clean_text = utils::remove_ansi_colors(&self.text);
        let escaped = html_escape(&clean_text);
        let trimmed = escaped.trim_end_matches(['.', ' ']);
        format!("{} {}.", icon, trimmed)
    }

    pub fn get_as_console_text(&self) -> String {
        let icon = match self.status {
            ItemStatus::Ok => "\u{2705}",
            ItemStatus::Notice => "\u{23E9}",
            ItemStatus::Warning => "\u{26A0}\u{FE0F}",
            ItemStatus::Critical => "\u{26D4}",
            ItemStatus::Info => "\u{1F4CC}",
        };

        let trimmed = self.text.trim_end_matches(['.', ' ']);
        format!("{} {}.", icon, trimmed)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
