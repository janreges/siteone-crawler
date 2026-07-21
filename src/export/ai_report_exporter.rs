// SiteOne Crawler - AI report exporters (JSON + HTML)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// One paired exporter consumes the `AiReportModel` stored in Status by the `extract` AI action and
// writes structured JSON plus a self-contained HTML report. Their paths are resolved together from
// `--ai-report-dir` and share a collision-safe run ID. It is activated only when a model exists.

use crate::error::CrawlerResult;
use crate::export::ai_report_html;
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Shared config for the paired AI-report artifacts.
pub struct AiReportExporter {
    json_path: PathBuf,
    html_path: PathBuf,
    cdn: bool,
}

impl AiReportExporter {
    pub fn new_pair(json_path: PathBuf, html_path: PathBuf, cdn: bool) -> Self {
        AiReportExporter {
            json_path,
            html_path,
            cdn,
        }
    }

    /// Resolve both artifact names in one place so JSON and HTML always share an output directory,
    /// sanitized components, and the same collision-safe run ID.
    pub fn paired_paths(output_dir: &str, preset: &str, host: Option<&str>, run_id: &str) -> (PathBuf, PathBuf) {
        let host = host.filter(|value| !value.is_empty()).map(sanitize_component);
        let mut components = vec!["ai-report".to_string(), sanitize_component(preset)];
        if let Some(host) = host {
            components.push(host);
        }
        components.push(sanitize_component(run_id));
        let stem = components.join(".");
        let directory = Path::new(output_dir);
        (
            directory.join(format!("{stem}.json")),
            directory.join(format!("{stem}.html")),
        )
    }
}

/// Keep a filename component filesystem-safe (preset keys are already `[a-z]`, but be defensive).
fn sanitize_component(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

impl Exporter for AiReportExporter {
    fn get_name(&self) -> &str {
        "AiReportExporter"
    }

    fn should_be_activated(&self) -> bool {
        // Activation is decided by the caller (a model must be present); default true so the
        // manager only constructs us when appropriate.
        true
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let model = match status.get_ai_report_model() {
            Some(m) => m,
            None => return Ok(()),
        };

        for path in [&self.json_path, &self.html_path] {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| crate::error::CrawlerError::Export(format!("Cannot create AI report dir: {}", e)))?;
            }
        }

        // Render both before writing either. The paired writer then creates both without replacing
        // existing files and removes the JSON if HTML creation fails.
        let json = serde_json::to_string_pretty(&model.to_json())
            .map_err(|e| crate::error::CrawlerError::Export(format!("AI report JSON serialization: {}", e)))?;
        let html = ai_report_html::render(&model, self.cdn);

        write_pair_atomic(&self.json_path, json.as_bytes(), &self.html_path, html.as_bytes()).map_err(|e| {
            crate::error::CrawlerError::Export(format!(
                "Cannot write paired AI report '{}', '{}': {}",
                self.json_path.display(),
                self.html_path.display(),
                e
            ))
        })?;

        eprintln!(
            "{}",
            crate::utils::get_color_text(
                &format!(
                    "AI report saved to: {} and {}",
                    self.json_path.display(),
                    self.html_path.display()
                ),
                "green",
                false
            )
        );
        status.add_info_to_summary(
            "ai-report-files",
            &format!(
                "AI report saved to {} and {}.",
                self.json_path.display(),
                self.html_path.display()
            ),
        );
        Ok(())
    }
}

fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(content).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

fn write_pair_atomic(
    json_path: &Path,
    json_content: &[u8],
    html_path: &Path,
    html_content: &[u8],
) -> std::io::Result<()> {
    write_atomic(json_path, json_content)?;
    if let Err(error) = write_atomic(html_path, html_content) {
        let _ = std::fs::remove_file(json_path);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_paths_share_directory_stem_and_unique_run_id() {
        let (json, html) = AiReportExporter::paired_paths(
            "tmp/reports",
            "compliance",
            Some("loan.example:8443"),
            "2026-07-21.10-11-12.123-42",
        );
        assert_eq!(json.parent(), Some(Path::new("tmp/reports")));
        assert_eq!(html.parent(), Some(Path::new("tmp/reports")));
        assert_eq!(json.file_stem(), html.file_stem());
        assert!(json.to_string_lossy().contains("loan-example-8443"));
        assert!(json.to_string_lossy().contains("2026-07-21-10-11-12-123-42"));
        assert_eq!(json.extension().and_then(|value| value.to_str()), Some("json"));
        assert_eq!(html.extension().and_then(|value| value.to_str()), Some("html"));
    }

    #[test]
    fn artifact_write_never_replaces_an_existing_file() {
        let dir = std::env::temp_dir().join(format!(
            "siteone-ai-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.json");
        std::fs::write(&path, b"old").unwrap();

        assert!(write_atomic(&path, b"new").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"old");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn paired_write_removes_first_artifact_when_second_cannot_be_created() {
        let dir = std::env::temp_dir().join(format!(
            "siteone-ai-pair-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let json_path = dir.join("report.json");
        let html_path = dir.join("report.html");
        std::fs::write(&html_path, b"existing").unwrap();

        assert!(write_pair_atomic(&json_path, b"json", &html_path, b"html").is_err());
        assert!(!json_path.exists());
        assert_eq!(std::fs::read(&html_path).unwrap(), b"existing");

        std::fs::remove_dir_all(dir).unwrap();
    }
}
