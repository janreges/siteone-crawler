// SiteOne Crawler - UploadExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Uploads HTML report to crawler.siteone.io via HTTP POST.

use std::time::Instant;

use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;
use crate::version;

pub struct UploadExporter {
    /// Whether upload is enabled (--upload)
    pub upload_enabled: bool,
    /// Upload endpoint URL (--upload-to)
    pub endpoint: String,
    /// Retention period (--upload-retention)
    pub retention: Option<String>,
    /// Optional password for the online report (--upload-password)
    pub password: Option<String>,
    /// Upload timeout in seconds (--upload-timeout)
    pub upload_timeout: u64,
    /// HTML report content to upload (set before export)
    pub html_report_content: Option<String>,
}

impl UploadExporter {
    pub fn new(
        upload_enabled: bool,
        endpoint: String,
        retention: Option<String>,
        password: Option<String>,
        upload_timeout: u64,
    ) -> Self {
        Self {
            upload_enabled,
            endpoint,
            retention,
            password,
            upload_timeout,
            html_report_content: None,
        }
    }

    /// Set HTML report content to be uploaded.
    pub fn set_html_report_content(&mut self, content: String) {
        self.html_report_content = Some(content);
    }

    /// Upload the HTML report to the configured endpoint.
    /// Returns the URL where the report is available.
    fn upload(&self, html: &str) -> CrawlerResult<String> {
        // Gzip compress the HTML body
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(html.as_bytes())
            .map_err(|e| CrawlerError::Export(format!("Failed to compress HTML for upload: {}", e)))?;
        let compressed_html = encoder
            .finish()
            .map_err(|e| CrawlerError::Export(format!("Failed to finish compression for upload: {}", e)))?;

        // Build form data
        let mut form = vec![
            ("version".to_string(), version::CODE.to_string()),
            ("platform".to_string(), std::env::consts::OS.to_string()),
            ("arch".to_string(), get_arch()),
        ];

        if let Some(ref retention) = self.retention {
            form.push(("retention".to_string(), retention.clone()));
        }
        if let Some(ref password) = self.password {
            let trimmed = password.trim();
            if !trimmed.is_empty() {
                form.push(("password".to_string(), trimmed.to_string()));
            }
        }

        // Send as application/x-www-form-urlencoded.
        // The gzipped binary htmlBody is URL-encoded via percent-encoding.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(self.upload_timeout))
            .build()
            .map_err(|e| CrawlerError::Export(format!("Failed to create HTTP client for upload: {}", e)))?;

        // Build URL-encoded body manually — reqwest's .form() doesn't support binary values
        use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
        let mut parts: Vec<String> = Vec::new();
        let encoded_html = percent_encode(&compressed_html, NON_ALPHANUMERIC).to_string();
        parts.push(format!("htmlBody={}", encoded_html));
        for (key, value) in &form {
            parts.push(format!(
                "{}={}",
                percent_encode(key.as_bytes(), NON_ALPHANUMERIC),
                percent_encode(value.as_bytes(), NON_ALPHANUMERIC)
            ));
        }
        let body = parts.join("&");

        let response = client
            .post(&self.endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .map_err(|e| CrawlerError::Export(format!("Upload request failed: {}", e)))?;

        let status_code = response.status();
        let body = response.text().unwrap_or_default();

        // Try to parse JSON response
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                return Ok(url.to_string());
            }
            if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
                return Err(CrawlerError::Export(format!(
                    "Upload failed: {} ({})",
                    error, status_code
                )));
            }
        }

        Err(CrawlerError::Export(format!(
            "Upload failed: unknown error ({})",
            status_code
        )))
    }
}

impl Exporter for UploadExporter {
    fn get_name(&self) -> &str {
        "UploadExporter"
    }

    fn should_be_activated(&self) -> bool {
        self.upload_enabled
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let html = match &self.html_report_content {
            Some(c) => c.clone(),
            None => {
                return Err(CrawlerError::Export(
                    "HTML report content not available. Set it via set_html_report_content() before export."
                        .to_string(),
                ));
            }
        };

        let start = Instant::now();
        match self.upload(&html) {
            Ok(online_url) => {
                let elapsed = start.elapsed().as_secs_f64();
                status.add_info_to_summary(
                    "upload-done",
                    &format!(
                        "HTML report uploaded to '{}' and took {}",
                        online_url,
                        utils::get_formatted_duration(elapsed)
                    ),
                );
            }
            Err(e) => {
                let elapsed = start.elapsed().as_secs_f64();
                status.add_critical_to_summary(
                    "upload-failed",
                    &format!(
                        "HTML report upload failed: {} and took {}",
                        e,
                        utils::get_formatted_duration(elapsed)
                    ),
                );
            }
        }

        Ok(())
    }
}

/// Detect system architecture.
fn get_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "x64".to_string(),
        "aarch64" => "arm64".to_string(),
        other => other.to_string(),
    }
}
