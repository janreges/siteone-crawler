// SiteOne Crawler - OutputType enum
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::CrawlerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    Text,
    Json,
    Multi,
}

impl OutputType {
    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.trim().to_lowercase().as_str() {
            "text" => Ok(OutputType::Text),
            "json" => Ok(OutputType::Json),
            other => Err(CrawlerError::Parse(format!(
                "Unknown output type '{}'. Supported values are: {}",
                other,
                Self::available_text_types().join(", ")
            ))),
        }
    }

    pub fn available_text_types() -> Vec<&'static str> {
        vec!["text", "json"]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OutputType::Text => "text",
            OutputType::Json => "json",
            OutputType::Multi => "multi",
        }
    }
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
