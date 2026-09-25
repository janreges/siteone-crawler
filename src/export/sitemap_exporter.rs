// SiteOne Crawler - SitemapExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Generates sitemap.xml and/or sitemap.txt from crawl results.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::types::ContentTypeId;
use crate::utils;

/// A `Last-Modified` at most this many seconds before the response `Date` (or after it) is a
/// dynamic page stamping "now", not the time its content changed, so it is left out of `<lastmod>`.
const LASTMOD_NOW_TOLERANCE_SECS: i64 = 60;

/// Earliest plausible `<lastmod>` (1995-01-01T00:00:00Z): older values are placeholders such as the
/// Unix epoch, sent to defeat caching.
const LASTMOD_MIN_TIMESTAMP: i64 = 788_918_400;

/// Without a response `Date`, a `Last-Modified` more than this far after the crawl is in the future.
const LASTMOD_FUTURE_TOLERANCE_SECS: i64 = 86_400;

pub struct SitemapExporter {
    /// Path for XML sitemap output (--sitemap-xml-file)
    pub output_sitemap_xml: Option<String>,
    /// Path for TXT sitemap output (--sitemap-txt-file)
    pub output_sitemap_txt: Option<String>,
    /// Base priority for XML sitemap entries (--sitemap-base-priority)
    pub base_priority: f64,
    /// Priority increase value based on slash count (--sitemap-priority-increase)
    pub priority_increase: f64,
    /// `<changefreq>` for every URL (--sitemap-changefreq); `None` leaves the element out
    pub changefreq: Option<String>,
}

/// One page of the sitemap: its URL and, when the server reported it reliably, its last
/// modification as a W3C datetime.
struct SitemapEntry {
    url: String,
    lastmod: Option<String>,
}

impl SitemapExporter {
    pub fn new(
        output_sitemap_xml: Option<String>,
        output_sitemap_txt: Option<String>,
        base_priority: f64,
        priority_increase: f64,
        changefreq: Option<String>,
    ) -> Self {
        Self {
            output_sitemap_xml,
            output_sitemap_txt,
            base_priority,
            priority_increase,
            changefreq,
        }
    }

    /// Collect pages eligible for sitemap: internal, HTML, 200 status code.
    /// Sort by slash count ascending, then alphabetically.
    fn collect_sitemap_entries(&self, status: &Status) -> Vec<SitemapEntry> {
        let visited_urls = status.get_visited_urls();
        let crawled_at = chrono::Utc::now();
        let mut entries: Vec<SitemapEntry> = visited_urls
            .iter()
            .filter(|vu| !vu.is_external && vu.content_type == ContentTypeId::Html && vu.status_code == 200)
            .map(|vu| SitemapEntry {
                url: vu.url.clone(),
                lastmod: status
                    .get_url_headers(&vu.uq_id)
                    .and_then(|headers| lastmod_from_headers(&headers, crawled_at)),
            })
            .collect();

        // Sort by slash count ascending, then alphabetically
        entries.sort_by(|a, b| {
            let a_trimmed = a.url.trim_end_matches('/');
            let b_trimmed = b.url.trim_end_matches('/');
            let a_slashes = a_trimmed.matches('/').count();
            let b_slashes = b_trimmed.matches('/').count();
            a_slashes.cmp(&b_slashes).then_with(|| a.url.cmp(&b.url))
        });

        entries
    }

    /// Render the XML sitemap. Inside `<url>` the elements follow the protocol order:
    /// loc, lastmod, changefreq, priority.
    fn render_xml(&self, entries: &[SitemapEntry]) -> String {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        xml.push_str("<!-- Sitemap generated using SiteOne Crawler - https://crawler.siteone.io/features/sitemap-generator/ -->\n");

        for entry in entries {
            // Calculate priority based on slash count in path
            let slashes_count = url::Url::parse(&entry.url)
                .ok()
                .map(|u| u.path().matches('/').count())
                .unwrap_or(1) as f64;

            let priority = (self.base_priority + (self.priority_increase * (1.0 - slashes_count))).clamp(0.1, 1.0);

            // Escape special XML characters in URL
            let escaped_url = escape_xml(&entry.url);

            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escaped_url));
            if let Some(ref lastmod) = entry.lastmod {
                xml.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod));
            }
            if let Some(ref changefreq) = self.changefreq {
                xml.push_str(&format!("    <changefreq>{}</changefreq>\n", changefreq));
            }
            xml.push_str(&format!("    <priority>{:.1}</priority>\n", priority));
            xml.push_str("  </url>\n");
        }

        xml.push_str("</urlset>\n");
        xml
    }

    /// Generate an XML sitemap file; a path ending in `.xml.gz` is written gzip-compressed.
    fn generate_xml_sitemap(&self, output_file: &str, entries: &[SitemapEntry]) -> CrawlerResult<String> {
        let (output_file, gzip) = xml_output_path(output_file);

        // Ensure parent directory exists
        let path = Path::new(&output_file);
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                CrawlerError::Export(format!("Cannot create output directory '{}': {}", parent.display(), e))
            })?;
        }

        let xml = self.render_xml(entries);

        // Write to file
        let mut file = fs::File::create(&output_file)
            .map_err(|e| CrawlerError::Export(format!("Failed to create XML sitemap file '{}': {}", output_file, e)))?;
        let written = if gzip {
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder
                .write_all(xml.as_bytes())
                .and_then(|_| encoder.finish().map(|_| ()))
        } else {
            file.write_all(xml.as_bytes())
        };
        written
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
        let entries = self.collect_sitemap_entries(status);

        // Generate XML sitemap
        if let Some(ref output_file) = self.output_sitemap_xml.clone() {
            match self.generate_xml_sitemap(output_file, &entries) {
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
            let urls: Vec<String> = entries.iter().map(|entry| entry.url.clone()).collect();
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

/// `<lastmod>` (W3C datetime in UTC) from a page's `Last-Modified` response header. `None` when
/// the header is missing or unparseable, before 1995 (a placeholder such as the Unix epoch), at
/// most a minute before the response `Date` (or after it), or — without a `Date` — more than a day
/// after `crawled_at`: dynamic pages stamp "now", which says nothing about when the content
/// changed, and search engines ignore a `lastmod` that is not accurate.
fn lastmod_from_headers(
    headers: &HashMap<String, String>,
    crawled_at: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let last_modified = chrono::DateTime::parse_from_rfc2822(headers.get("last-modified")?.trim()).ok()?;
    if last_modified.timestamp() < LASTMOD_MIN_TIMESTAMP {
        return None;
    }
    let too_recent = match headers
        .get("date")
        .and_then(|date| chrono::DateTime::parse_from_rfc2822(date.trim()).ok())
    {
        Some(date) => (date - last_modified).num_seconds() <= LASTMOD_NOW_TOLERANCE_SECS,
        None => (last_modified.with_timezone(&chrono::Utc) - crawled_at).num_seconds() > LASTMOD_FUTURE_TOLERANCE_SECS,
    };
    if too_recent {
        return None;
    }
    Some(
        last_modified
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%S+00:00")
            .to_string(),
    )
}

/// Output path of the XML sitemap and whether to gzip it: a path ending in `.xml.gz` is kept and
/// written compressed, a path ending in `.xml` is kept, any other path gets `.xml` appended.
fn xml_output_path(output_file: &str) -> (String, bool) {
    let lower = output_file.to_lowercase();
    if lower.ends_with(".xml.gz") {
        (output_file.to_string(), true)
    } else if lower.ends_with(".xml") {
        (output_file.to_string(), false)
    } else {
        (format!("{}.xml", output_file), false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::Info;
    use crate::result::storage::memory_storage::MemoryStorage;
    use crate::result::visited_url::VisitedUrl;

    fn exporter(changefreq: Option<&str>) -> SitemapExporter {
        SitemapExporter::new(None, None, 0.5, 0.1, changefreq.map(str::to_string))
    }

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    /// When the tested crawl ran; the `Last-Modified` values below are a few days older.
    fn crawl_time() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-20T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn page(url: &str) -> SitemapEntry {
        SitemapEntry {
            url: url.to_string(),
            lastmod: None,
        }
    }

    #[test]
    fn xml_uses_the_sitemaps_org_namespace() {
        let xml = exporter(None).render_xml(&[page("https://example.com/")]);
        assert!(
            xml.contains(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#),
            "{xml}"
        );
    }

    #[test]
    fn lastmod_is_the_last_modified_header_in_utc() {
        let lastmod = |value: &str| lastmod_from_headers(&headers(&[("last-modified", value)]), crawl_time());
        assert_eq!(
            lastmod("Fri, 17 Jul 2026 17:29:24 GMT").as_deref(),
            Some("2026-07-17T17:29:24+00:00")
        );
        assert_eq!(
            lastmod("Fri, 17 Jul 2026 19:29:24 +0200").as_deref(),
            Some("2026-07-17T17:29:24+00:00")
        );
    }

    #[test]
    fn lastmod_is_omitted_without_a_usable_header() {
        assert_eq!(lastmod_from_headers(&headers(&[]), crawl_time()), None);
        assert_eq!(
            lastmod_from_headers(&headers(&[("last-modified", "yesterday")]), crawl_time()),
            None
        );
    }

    #[test]
    fn lastmod_is_omitted_when_the_page_stamps_the_response_time() {
        let lastmod = |date: &str| {
            lastmod_from_headers(
                &headers(&[("last-modified", "Fri, 17 Jul 2026 17:29:24 GMT"), ("date", date)]),
                crawl_time(),
            )
        };
        assert_eq!(lastmod("Fri, 17 Jul 2026 17:29:24 GMT"), None, "same second");
        assert_eq!(lastmod("Fri, 17 Jul 2026 17:30:24 GMT"), None, "60 s later");
        assert_eq!(
            lastmod("Fri, 17 Jul 2026 17:29:00 GMT"),
            None,
            "Date before Last-Modified"
        );
        assert_eq!(
            lastmod("Fri, 17 Jul 2026 17:30:25 GMT").as_deref(),
            Some("2026-07-17T17:29:24+00:00"),
            "61 s later"
        );
    }

    #[test]
    fn lastmod_is_omitted_when_it_is_implausibly_old_or_in_the_future() {
        let lastmod = |value: &str| lastmod_from_headers(&headers(&[("last-modified", value)]), crawl_time());
        assert_eq!(lastmod("Thu, 01 Jan 1970 00:00:00 GMT"), None, "Unix epoch");
        assert_eq!(lastmod("Sat, 31 Dec 1994 23:59:59 GMT"), None, "before 1995");
        assert_eq!(
            lastmod("Sun, 01 Jan 1995 00:00:00 GMT").as_deref(),
            Some("1995-01-01T00:00:00+00:00")
        );
        // Without a `Date` header, "the future" is relative to the crawl, with a day of tolerance.
        assert_eq!(
            lastmod("Tue, 21 Jul 2026 11:00:00 GMT").as_deref(),
            Some("2026-07-21T11:00:00+00:00"),
            "23 h after the crawl"
        );
        assert_eq!(lastmod("Tue, 21 Jul 2026 13:00:00 GMT"), None, "25 h after the crawl");
    }

    #[test]
    fn url_elements_follow_the_protocol_order() {
        let xml = exporter(Some("weekly")).render_xml(&[SitemapEntry {
            url: "https://example.com/about".to_string(),
            lastmod: Some("2026-07-17T17:29:24+00:00".to_string()),
        }]);
        let loc = xml.find("<loc>https://example.com/about</loc>").expect("loc");
        let lastmod = xml
            .find("<lastmod>2026-07-17T17:29:24+00:00</lastmod>")
            .expect("lastmod");
        let changefreq = xml.find("<changefreq>weekly</changefreq>").expect("changefreq");
        let priority = xml.find("<priority>0.5</priority>").expect("priority");
        assert!(loc < lastmod && lastmod < changefreq && changefreq < priority, "{xml}");
    }

    #[test]
    fn unknown_lastmod_and_unset_changefreq_are_left_out() {
        let xml = exporter(None).render_xml(&[page("https://example.com/")]);
        assert!(!xml.contains("<lastmod>"), "{xml}");
        assert!(!xml.contains("<changefreq>"), "{xml}");
        assert!(xml.contains("<priority>0.5</priority>"), "{xml}");
    }

    #[test]
    fn lastmod_comes_from_the_stored_response_headers() {
        let info = Info::new(
            "SiteOne Crawler".to_string(),
            "test".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            "https://example.com/".to_string(),
        );
        let mut status = Status::new(
            Box::new(MemoryStorage::new(false)),
            true,
            info,
            std::time::Instant::now(),
        );
        for (uq_id, url, last_modified) in [
            (
                "aaaa1111",
                "https://example.com/",
                Some("Fri, 17 Jul 2026 17:29:24 GMT"),
            ),
            ("bbbb2222", "https://example.com/news", None),
        ] {
            let mut response_headers = headers(&[("content-type", "text/html")]);
            if let Some(value) = last_modified {
                response_headers.insert("last-modified".to_string(), value.to_string());
            }
            let visited = VisitedUrl::new(
                uq_id.to_string(),
                String::new(),
                0,
                url.to_string(),
                200,
                0.1,
                Some(100),
                ContentTypeId::Html,
                Some("text/html".to_string()),
                None,
                None,
                false,
                true,
                0,
                None,
            );
            status.add_visited_url(visited, None, Some(&response_headers));
        }

        let entries = exporter(None).collect_sitemap_entries(&status);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://example.com/");
        assert_eq!(entries[0].lastmod.as_deref(), Some("2026-07-17T17:29:24+00:00"));
        assert_eq!(entries[1].url, "https://example.com/news");
        assert_eq!(entries[1].lastmod, None);
    }

    #[test]
    fn xml_output_path_keeps_xml_and_xml_gz_and_adds_xml_otherwise() {
        assert_eq!(
            xml_output_path("/tmp/sitemap.xml"),
            ("/tmp/sitemap.xml".to_string(), false)
        );
        assert_eq!(
            xml_output_path("/tmp/sitemap.XML.GZ"),
            ("/tmp/sitemap.XML.GZ".to_string(), true)
        );
        assert_eq!(xml_output_path("/tmp/sitemap"), ("/tmp/sitemap.xml".to_string(), false));
    }

    #[test]
    fn xml_gz_sitemap_is_written_gzip_compressed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sitemap.xml.gz");

        let written = exporter(None)
            .generate_xml_sitemap(path.to_str().unwrap(), &[page("https://example.com/")])
            .unwrap();

        assert_eq!(written, path.to_str().unwrap());
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0x1f, 0x8b]), "gzip magic bytes");
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(&bytes[..]), &mut xml).unwrap();
        assert!(xml.contains("<loc>https://example.com/</loc>"), "{xml}");
    }
}
