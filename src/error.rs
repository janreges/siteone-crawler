// SiteOne Crawler - Error types
// (c) Jan Reges <jan.reges@siteone.cz>

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrawlerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("DNS resolution error: {0}")]
    Dns(String),

    #[error("TLS/SSL error: {0}")]
    Tls(String),

    #[error("Mail error: {0}")]
    Mail(String),

    #[error("Export error: {0}")]
    Export(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("{0}")]
    Other(String),
}

pub type CrawlerResult<T> = std::result::Result<T, CrawlerError>;
