// SiteOne Crawler - AI profile: the ProfileDoc + renderers
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The stored artifact: meta + site description + executive summary + ordered chapters (each a
// synthesized Markdown body). Renders to Markdown (primary), JSON (structured), and self-contained
// HTML (Task 8). Omitted chapters are recorded (with a reason) but not rendered — never fabricated.

use serde_json::{Value, json};

use crate::ai::report::locale::ReportLocale;

use super::correct::CorrectionReport;

#[derive(Debug, Clone, Default)]
pub struct ProfileMeta {
    pub host: String,
    pub url: String,
    pub crawled_at: String,
    pub provider: String,
    pub model: String,
    pub report_language: String,
    pub context_window: i64,
    pub template_key: String,
    pub template_name: String,
    /// True when the type was auto-detected by the classifier; false when forced by CLI.
    pub template_detected: bool,
    pub pages_total: usize,
    pub pages_described: usize,
    pub calls: usize,
    pub pages_failed: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ProfilePageInfo {
    pub path: String,
    pub url: String,
    pub description: String,
    pub size_chars: usize,
}

#[derive(Debug, Clone)]
pub struct ProfileChapter {
    pub id: String,
    pub heading: String,
    pub markdown: String,
    pub sources: Vec<String>,
    pub target_chars: usize,
    pub correction: Option<CorrectionReport>,
    pub omitted: bool,
    pub omit_reason: String,
}

#[derive(Debug, Clone)]
pub struct ProfileDoc {
    pub title: String,
    pub subject_name: String,
    pub meta: ProfileMeta,
    pub site_description: String,
    pub exec_summary: String,
    pub pages: Vec<ProfilePageInfo>,
    pub chapters: Vec<ProfileChapter>,
}

impl ProfileDoc {
    pub(crate) fn locale(&self) -> ReportLocale {
        ReportLocale::new(&self.meta.report_language)
    }

    /// Chapters that were actually written (non-omitted, non-empty body).
    pub(crate) fn visible_chapters(&self) -> impl Iterator<Item = &ProfileChapter> {
        self.chapters
            .iter()
            .filter(|c| !c.omitted && !c.markdown.trim().is_empty())
    }

    // ---- markdown ----

    pub fn to_markdown(&self) -> String {
        let czech = self.locale().is_czech();
        let sources_label = if czech { "Zdroje" } else { "Sources" };
        let summary_label = if czech { "Shrnutí" } else { "Executive summary" };
        let mut out = String::with_capacity(32 * 1024);
        out.push_str(&format!("# {}\n\n", self.title));
        out.push_str(&format!(
            "> {} · {} · {}\n\n",
            self.meta.host,
            if self.meta.crawled_at.is_empty() {
                "—"
            } else {
                &self.meta.crawled_at
            },
            self.meta.template_name
        ));

        if !self.exec_summary.trim().is_empty() {
            out.push_str(&format!("## {}\n\n{}\n\n", summary_label, self.exec_summary.trim()));
        }

        for c in self.visible_chapters() {
            out.push_str(&format!("## {}\n\n{}\n\n", c.heading, c.markdown.trim()));
            if !c.sources.is_empty() {
                out.push_str(&format!("*{}:* ", sources_label));
                out.push_str(&c.sources.join(", "));
                out.push_str("\n\n");
            }
        }

        if !self.meta.pages_failed.is_empty() {
            let note = if czech {
                format!(
                    "*Sestaveno z {} stránek; {} stránek se nepodařilo zpracovat.*",
                    self.meta.pages_described,
                    self.meta.pages_failed.len()
                )
            } else {
                format!(
                    "*Built from {} pages; {} page(s) could not be processed.*",
                    self.meta.pages_described,
                    self.meta.pages_failed.len()
                )
            };
            out.push_str(&format!("---\n\n{}\n", note));
        }
        out
    }

    // ---- json ----

    pub fn to_json(&self) -> Value {
        json!({
            "title": self.title,
            "subjectName": self.subject_name,
            "siteDescription": self.site_description,
            "meta": {
                "host": self.meta.host,
                "url": self.meta.url,
                "crawledAt": self.meta.crawled_at,
                "provider": self.meta.provider,
                "model": self.meta.model,
                "reportLanguage": self.meta.report_language,
                "contextWindow": self.meta.context_window,
                "template": self.meta.template_key,
                "templateName": self.meta.template_name,
                "templateDetected": self.meta.template_detected,
                "pagesTotal": self.meta.pages_total,
                "pagesDescribed": self.meta.pages_described,
                "calls": self.meta.calls,
                "pagesFailed": self.meta.pages_failed.iter()
                    .map(|(u, e)| json!({"url": u, "error": e})).collect::<Vec<_>>(),
            },
            "executiveSummary": self.exec_summary,
            "pages": self.pages.iter().map(|p| json!({
                "path": p.path, "url": p.url, "description": p.description, "sizeChars": p.size_chars,
            })).collect::<Vec<_>>(),
            "chapters": self.chapters.iter().map(|c| json!({
                "id": c.id,
                "heading": c.heading,
                "markdown": if c.omitted { Value::Null } else { json!(c.markdown) },
                "sources": c.sources,
                "targetChars": c.target_chars,
                "omitted": c.omitted,
                "omitReason": c.omit_reason,
                "correction": c.correction.as_ref().map(|r| json!({
                    "proposed": r.proposed, "applied": r.applied,
                    "skippedNoMatch": r.skipped_no_match, "skippedAmbiguous": r.skipped_ambiguous,
                    "skippedOverlap": r.skipped_overlap, "skippedGuard": r.skipped_guard,
                })),
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProfileDoc {
        ProfileDoc {
            title: "Acme — profil značky".into(),
            subject_name: "Acme".into(),
            meta: ProfileMeta {
                host: "acme.cz".into(),
                url: "https://acme.cz/".into(),
                crawled_at: "2026-07-22 10:00".into(),
                report_language: "cs-CZ".into(),
                template_key: "smb-services".into(),
                template_name: "Malé a střední služby".into(),
                template_detected: true,
                pages_described: 42,
                ..Default::default()
            },
            site_description: "Acme je malá firma.".into(),
            exec_summary: "Acme poskytuje služby.".into(),
            pages: vec![],
            chapters: vec![
                ProfileChapter {
                    id: "01-overview".into(),
                    heading: "Přehled".into(),
                    markdown: "Acme dělá **skvělé** věci.".into(),
                    sources: vec!["https://acme.cz/o-nas".into()],
                    target_chars: 3500,
                    correction: None,
                    omitted: false,
                    omit_reason: String::new(),
                },
                ProfileChapter {
                    id: "09-pricing".into(),
                    heading: "Ceník".into(),
                    markdown: String::new(),
                    sources: vec![],
                    target_chars: 4000,
                    correction: None,
                    omitted: true,
                    omit_reason: "no relevant pages".into(),
                },
            ],
        }
    }

    #[test]
    fn markdown_has_title_summary_visible_chapter_and_omits_empty() {
        let md = sample().to_markdown();
        assert!(md.contains("# Acme — profil značky"));
        assert!(md.contains("## Shrnutí"));
        assert!(md.contains("## Přehled"));
        assert!(md.contains("*Zdroje:* https://acme.cz/o-nas"));
        assert!(!md.contains("## Ceník"), "omitted chapter must not render");
    }

    #[test]
    fn json_records_omitted_chapter_with_reason_and_null_markdown() {
        let j = sample().to_json();
        assert_eq!(j["meta"]["template"], "smb-services");
        assert_eq!(j["chapters"][1]["omitted"], true);
        assert_eq!(j["chapters"][1]["omitReason"], "no relevant pages");
        assert!(j["chapters"][1]["markdown"].is_null());
        assert_eq!(j["chapters"][0]["sources"][0], "https://acme.cz/o-nas");
    }
}
