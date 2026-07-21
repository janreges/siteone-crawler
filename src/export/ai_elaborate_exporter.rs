// SiteOne Crawler - AI brand-elaborate exporter (Markdown + JSON + HTML)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Consumes the `BrandDoc` stored in Status by the `--ai-elaborate` pipeline and writes the brand
// profile as three paired artifacts: readable Markdown, structured JSON, and a self-contained HTML
// document. Paths are resolved together from `--ai-report-dir` and share a collision-safe run ID,
// mirroring `AiReportExporter`. Activated only when a document exists.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;

/// Paths for the three paired brand-elaborate artifacts.
pub struct AiElaborateExporter {
    md_path: PathBuf,
    json_path: PathBuf,
    html_path: PathBuf,
}

impl AiElaborateExporter {
    pub fn new_triple(md_path: PathBuf, json_path: PathBuf, html_path: PathBuf) -> Self {
        AiElaborateExporter {
            md_path,
            json_path,
            html_path,
        }
    }

    /// Resolve all three artifact names in one place so they share directory, sanitized components,
    /// and the same collision-safe run ID.
    pub fn tripled_paths(
        output_dir: &str,
        template: &str,
        host: Option<&str>,
        run_id: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let host = host.filter(|value| !value.is_empty()).map(sanitize_component);
        let mut components = vec!["ai-elaborate".to_string(), sanitize_component(template)];
        if let Some(host) = host {
            components.push(host);
        }
        components.push(sanitize_component(run_id));
        let stem = components.join(".");
        let directory = Path::new(output_dir);
        (
            directory.join(format!("{stem}.md")),
            directory.join(format!("{stem}.json")),
            directory.join(format!("{stem}.html")),
        )
    }
}

/// Keep a filename component filesystem-safe (template keys are already `[a-z]`, but be defensive).
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

impl Exporter for AiElaborateExporter {
    fn get_name(&self) -> &str {
        "AiElaborateExporter"
    }

    fn should_be_activated(&self) -> bool {
        // Activation is decided by the caller (a document must be present); the manager only
        // constructs us when appropriate.
        true
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let doc = match status.get_ai_elaborate_doc() {
            Some(d) => d,
            None => return Ok(()),
        };

        for path in [&self.md_path, &self.json_path, &self.html_path] {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CrawlerError::Export(format!("Cannot create AI elaborate dir: {}", e)))?;
            }
        }

        // Render all three before writing any; the atomic writer then creates them without replacing
        // existing files and rolls back earlier artifacts if a later one fails.
        let md = doc.to_markdown();
        let json = serde_json::to_string_pretty(&doc.to_json())
            .map_err(|e| CrawlerError::Export(format!("AI elaborate JSON serialization: {}", e)))?;
        let html = doc.to_html();

        write_triple_atomic(
            &self.md_path,
            md.as_bytes(),
            &self.json_path,
            json.as_bytes(),
            &self.html_path,
            html.as_bytes(),
        )
        .map_err(|e| {
            CrawlerError::Export(format!(
                "Cannot write brand-elaborate artifacts '{}', '{}', '{}': {}",
                self.md_path.display(),
                self.json_path.display(),
                self.html_path.display(),
                e
            ))
        })?;

        eprintln!(
            "{}",
            crate::utils::get_color_text(
                &format!(
                    "Brand elaborate saved to: {}, {} and {}",
                    self.md_path.display(),
                    self.json_path.display(),
                    self.html_path.display()
                ),
                "green",
                false
            )
        );
        status.add_info_to_summary(
            "ai-elaborate-files",
            &format!(
                "Brand elaborate saved to {}, {} and {}.",
                self.md_path.display(),
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

/// Write all three artifacts without overwriting; on any failure, remove the ones already written so
/// the run never leaves a partial, mismatched set.
fn write_triple_atomic(
    md_path: &Path,
    md: &[u8],
    json_path: &Path,
    json: &[u8],
    html_path: &Path,
    html: &[u8],
) -> std::io::Result<()> {
    write_atomic(md_path, md)?;
    if let Err(error) = write_atomic(json_path, json) {
        let _ = std::fs::remove_file(md_path);
        return Err(error);
    }
    if let Err(error) = write_atomic(html_path, html) {
        let _ = std::fs::remove_file(md_path);
        let _ = std::fs::remove_file(json_path);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tripled_paths_share_directory_stem_and_unique_run_id() {
        let (md, json, html) = AiElaborateExporter::tripled_paths(
            "tmp/reports",
            "corporate",
            Some("brand.example:8443"),
            "2026-07-21.10-11-12.123-42",
        );
        assert_eq!(md.parent(), Some(Path::new("tmp/reports")));
        assert_eq!(md.file_stem(), json.file_stem());
        assert_eq!(json.file_stem(), html.file_stem());
        assert!(md.to_string_lossy().contains("brand-example-8443"));
        assert!(md.to_string_lossy().contains("2026-07-21-10-11-12-123-42"));
        assert_eq!(md.extension().and_then(|v| v.to_str()), Some("md"));
        assert_eq!(json.extension().and_then(|v| v.to_str()), Some("json"));
        assert_eq!(html.extension().and_then(|v| v.to_str()), Some("html"));
    }

    #[test]
    fn triple_write_rolls_back_when_a_later_artifact_cannot_be_created() {
        let dir = std::env::temp_dir().join(format!(
            "siteone-elab-triple-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let md_path = dir.join("e.md");
        let json_path = dir.join("e.json");
        let html_path = dir.join("e.html");
        // Pre-create the HTML so the third write fails; the first two must be rolled back.
        std::fs::write(&html_path, b"existing").unwrap();

        assert!(write_triple_atomic(&md_path, b"md", &json_path, b"json", &html_path, b"html").is_err());
        assert!(!md_path.exists());
        assert!(!json_path.exists());
        assert_eq!(std::fs::read(&html_path).unwrap(), b"existing");

        std::fs::remove_dir_all(dir).unwrap();
    }
}
