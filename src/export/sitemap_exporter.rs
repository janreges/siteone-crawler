// SiteOne Crawler - SitemapExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Generates sitemap.xml and/or sitemap.txt from crawl results.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::types::ContentTypeId;
use crate::utils;

pub struct SitemapExporter {
    /// Path for XML sitemap output (--sitemap-xml-file)
    pub output_sitemap_xml: Option<String>,
    /// Path for TXT sitemap output (--sitemap-txt-file)
    pub output_sitemap_txt: Option<String>,
    /// Base priority for XML sitemap entries (--sitemap-base-priority)
    pub base_priority: f64,
    /// Priority increase value based on slash count (--sitemap-priority-increase)
    pub priority_increase: f64,
}

impl SitemapExporter {
    pub fn new(
        output_sitemap_xml: Option<String>,
        output_sitemap_txt: Option<String>,
        base_priority: f64,
        priority_increase: f64,
    ) -> Self {
        Self {
            output_sitemap_xml,
            output_sitemap_txt,
            base_priority,
            priority_increase,
        }
    }

    /// Collect URLs eligible for sitemap: internal, HTML, 200 status code.
    /// Sort by slash count ascending, then alphabetically.
    fn collect_sitemap_urls(&self, status: &Status) -> Vec<String> {
        let visited_urls = status.get_visited_urls();
        let mut urls: Vec<String> = visited_urls
            .iter()
            .filter(|vu| !vu.is_external && vu.content_type == ContentTypeId::Html && vu.status_code == 200)
            .map(|vu| vu.url.clone())
            .collect();

        // Sort by slash count ascending, then alphabetically
        urls.sort_by(|a, b| {
            let a_trimmed = a.trim_end_matches('/');
            let b_trimmed = b.trim_end_matches('/');
            let a_slashes = a_trimmed.matches('/').count();
            let b_slashes = b_trimmed.matches('/').count();
            a_slashes.cmp(&b_slashes).then_with(|| a.cmp(b))
        });

        urls
    }

    /// Generate an XML sitemap file.
    fn generate_xml_sitemap(&self, output_file: &str, urls: &[String]) -> CrawlerResult<String> {
        // Ensure .xml extension
        let output_file = if output_file.to_lowercase().ends_with(".xml") {
            output_file.to_string()
        } else {
            let stripped = regex::Regex::new(r"\.xml$")
                .ok()
                .map(|re| re.replace(output_file, "").to_string())
                .unwrap_or_else(|| output_file.to_string());
            format!("{}.xml", stripped)
        };

        // Ensure parent directory exists
        let path = Path::new(&output_file);
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                CrawlerError::Export(format!("Cannot create output directory '{}': {}", parent.display(), e))
            })?;
        }

        // Build XML content manually for proper formatting
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<urlset xmlns=\"https://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        xml.push_str("<!-- Sitemap generated using SiteOne Crawler - https://crawler.siteone.io/features/sitemap-generator/ -->\n");

        for url in urls {
            // Calculate priority based on slash count in path
            let slashes_count = url::Url::parse(url)
                .ok()
                .map(|u| u.path().matches('/').count())
                .unwrap_or(1) as f64;

            let priority = (self.base_priority + (self.priority_increase * (1.0 - slashes_count))).clamp(0.1, 1.0);

            // Escape special XML characters in URL
            let escaped_url = escape_xml(url);

            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escaped_url));
            xml.push_str(&format!("    <priority>{:.1}</priority>\n", priority));
            xml.push_str("  </url>\n");
        }

        xml.push_str("</urlset>\n");

        // Write to file
        let mut file = fs::File::create(&output_file)
            .map_err(|e| CrawlerError::Export(format!("Failed to create XML sitemap file '{}': {}", output_file, e)))?;
        file.write_all(xml.as_bytes())
            .map_err(|e| CrawlerError::Export(format!("Failed to write XML sitemap to '{}': {}", output_file, e)))?;

        Ok(output_file)
    }

    /// Generate a TXT sitemap file (plain list of URLs).
    fn generate_txt_sitemap(&self, output_file: &str, urls: &[String]) -> CrawlerResult<String> {
        // Ensure .txt extension
        let output_file = if output_file.to_lowercase().ends_with(".txt") {
            output_file.to_string()
        } else {
            let stripped = regex::Regex::new(r"\.txt$")
                .ok()
                .map(|re| re.replace(output_file, "").to_string())
                .unwrap_or_else(|| output_file.to_string());
            format!("{}.txt", stripped)
        };

        // Ensure parent directory exists
        let path = Path::new(&output_file);
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                CrawlerError::Export(format!("Cannot create output directory '{}': {}", parent.display(), e))
            })?;
        }

        let content = urls.join("\n");
        fs::write(&output_file, &content)
            .map_err(|e| CrawlerError::Export(format!("Failed to write TXT sitemap to '{}': {}", output_file, e)))?;

        Ok(output_file)
    }
}

impl Exporter for SitemapExporter {
    fn get_name(&self) -> &str {
        "SitemapExporter"
    }

    fn should_be_activated(&self) -> bool {
        self.output_sitemap_xml.is_some() || self.output_sitemap_txt.is_some()
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let urls = self.collect_sitemap_urls(status);

        // Generate XML sitemap
        if let Some(ref output_file) = self.output_sitemap_xml.clone() {
            match self.generate_xml_sitemap(output_file, &urls) {
                Ok(sitemap_file) => {
                    let display_path = utils::get_output_formatted_path(&sitemap_file);
                    status.add_info_to_summary("sitemap-xml", &format!("XML sitemap generated to '{}'", display_path));
                }
                Err(e) => {
                    status.add_critical_to_summary("sitemap-xml", &format!("Sitemap XML ERROR: {}", e));
                }
            }
        }

        // Generate TXT sitemap
        if let Some(ref output_file) = self.output_sitemap_txt.clone() {
            match self.generate_txt_sitemap(output_file, &urls) {
                Ok(sitemap_file) => {
                    let display_path = utils::get_output_formatted_path(&sitemap_file);
                    status.add_info_to_summary("sitemap-txt", &format!("TXT sitemap generated to '{}'", display_path));
                }
                Err(e) => {
                    status.add_critical_to_summary("sitemap-txt", &format!("Sitemap TXT ERROR: {}", e));
                }
            }
        }

        Ok(())
    }
}

/// Escape special XML characters in a string.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
