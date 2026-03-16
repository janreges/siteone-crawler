// SiteOne Crawler - Summary ItemStatus
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};

use crate::error::CrawlerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ItemStatus {
    Ok,
    Notice,
    Warning,
    Critical,
    Info,
}

impl ItemStatus {
    pub fn from_range_id(range_id: i32) -> Result<Self, CrawlerError> {
        match range_id {
            0 => Ok(ItemStatus::Ok),
            1 => Ok(ItemStatus::Notice),
            2 => Ok(ItemStatus::Warning),
            3 => Ok(ItemStatus::Critical),
            4 => Ok(ItemStatus::Info),
            _ => Err(CrawlerError::Parse(format!(
                "ItemStatus::from_range_id: Unknown range ID '{}'",
                range_id
            ))),
        }
    }

    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.to_uppercase().as_str() {
            "OK" => Ok(ItemStatus::Ok),
            "NOTICE" => Ok(ItemStatus::Notice),
            "WARNING" => Ok(ItemStatus::Warning),
            "CRITICAL" => Ok(ItemStatus::Critical),
            "INFO" => Ok(ItemStatus::Info),
            _ => Err(CrawlerError::Parse(format!(
                "ItemStatus::from_text: Unknown status '{}'",
                text
            ))),
        }
    }

    pub fn sort_order(&self) -> i32 {
        match self {
            ItemStatus::Critical => 1,
            ItemStatus::Warning => 2,
            ItemStatus::Notice => 3,
            ItemStatus::Ok => 4,
            ItemStatus::Info => 5,
        }
    }
}
