// SiteOne Crawler - Type definitions
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::CrawlerError;

// ---------------------------------------------------------------------------
// DeviceType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Mobile,
    Tablet,
}

impl DeviceType {
    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.trim().to_lowercase().as_str() {
            "desktop" => Ok(DeviceType::Desktop),
            "mobile" => Ok(DeviceType::Mobile),
            "tablet" => Ok(DeviceType::Tablet),
            other => Err(CrawlerError::Config(format!(
                "Unknown device type '{}'. Supported values are: {}",
                other,
                Self::available_text_types().join(", ")
            ))),
        }
    }

    pub fn available_text_types() -> Vec<&'static str> {
        vec!["desktop", "mobile", "tablet"]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceType::Desktop => "desktop",
            DeviceType::Mobile => "mobile",
            DeviceType::Tablet => "tablet",
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AssetType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Fonts,
    Images,
    Styles,
    Scripts,
    Files,
}

impl AssetType {
    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.trim().to_lowercase().as_str() {
            "fonts" => Ok(AssetType::Fonts),
            "images" => Ok(AssetType::Images),
            "styles" => Ok(AssetType::Styles),
            "scripts" => Ok(AssetType::Scripts),
            "files" => Ok(AssetType::Files),
            other => Err(CrawlerError::Config(format!(
                "Unknown asset type '{}'. Supported values are: {}",
                other,
                Self::available_text_types().join(", ")
            ))),
        }
    }

    pub fn available_text_types() -> Vec<&'static str> {
        vec!["fonts", "images", "styles", "scripts", "files"]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AssetType::Fonts => "fonts",
            AssetType::Images => "images",
            AssetType::Styles => "styles",
            AssetType::Scripts => "scripts",
            AssetType::Files => "files",
        }
    }
}

impl fmt::Display for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ContentTypeId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum ContentTypeId {
    Html = 1,
    Script = 2,
    Stylesheet = 3,
    Image = 4,
    Video = 5,
    Font = 6,
    Document = 7,
    Json = 8,
    Redirect = 9,
    Other = 10,
    Audio = 11,
    Xml = 12,
}

impl ContentTypeId {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(ContentTypeId::Html),
            2 => Some(ContentTypeId::Script),
            3 => Some(ContentTypeId::Stylesheet),
            4 => Some(ContentTypeId::Image),
            5 => Some(ContentTypeId::Video),
            6 => Some(ContentTypeId::Font),
            7 => Some(ContentTypeId::Document),
            8 => Some(ContentTypeId::Json),
            9 => Some(ContentTypeId::Redirect),
            10 => Some(ContentTypeId::Other),
            11 => Some(ContentTypeId::Audio),
            12 => Some(ContentTypeId::Xml),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ContentTypeId::Html => "HTML",
            ContentTypeId::Script => "JS",
            ContentTypeId::Stylesheet => "CSS",
            ContentTypeId::Image => "Image",
            ContentTypeId::Audio => "Audio",
            ContentTypeId::Video => "Video",
            ContentTypeId::Font => "Font",
            ContentTypeId::Document => "Document",
            ContentTypeId::Json => "JSON",
            ContentTypeId::Xml => "XML",
            ContentTypeId::Redirect => "Redirect",
            ContentTypeId::Other => "Other",
        }
    }
}

impl fmt::Display for ContentTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// SkippedReason
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum SkippedReason {
    NotAllowedHost = 1,
    RobotsTxt = 2,
    ExceedsMaxDepth = 3,
}

impl SkippedReason {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(SkippedReason::NotAllowedHost),
            2 => Some(SkippedReason::RobotsTxt),
            3 => Some(SkippedReason::ExceedsMaxDepth),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SkippedReason::NotAllowedHost => "Not allowed host",
            SkippedReason::RobotsTxt => "Robots.txt",
            SkippedReason::ExceedsMaxDepth => "Exceeds max depth",
        }
    }
}

impl fmt::Display for SkippedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

// ---------------------------------------------------------------------------
// OutputType
// ---------------------------------------------------------------------------

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
            other => Err(CrawlerError::Config(format!(
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
