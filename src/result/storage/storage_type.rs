// SiteOne Crawler - StorageType
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};

use crate::error::CrawlerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Memory,
    File,
}

impl StorageType {
    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.trim().to_lowercase().as_str() {
            "memory" => Ok(StorageType::Memory),
            "file" => Ok(StorageType::File),
            other => Err(CrawlerError::Parse(format!(
                "Unknown storage type '{}'. Supported values are: {}",
                other,
                Self::available_text_types().join(", ")
            ))),
        }
    }

    pub fn available_text_types() -> Vec<&'static str> {
        vec!["memory", "file"]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::Memory => "memory",
            StorageType::File => "file",
        }
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
