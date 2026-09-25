// SiteOne Crawler - XmlProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Extracts URLs from sitemap.xml and sitemap index files.

use std::io::Read;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::content_processor::base_processor::{ProcessorConfig, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_url::{FoundUrl, UrlSource};
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

/// Largest sitemap accepted from a gzip body: the protocol caps a sitemap at 50 MB uncompressed.
const MAX_SITEMAP_BYTES: u64 = 50 * 1024 * 1024;

/// Leading (decompressed) bytes in which a gzip sitemap must name its root element (`<urlset>` or
/// `<sitemapindex>`), so archives such as a linked `.tar.gz` are never inflated in full.
const SITEMAP_ROOT_WINDOW: u64 = 64 * 1024;

pub struct XmlProcessor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl XmlProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Xml],
        }
    }

    fn is_sitemap_xml_index(content: &str) -> bool {
        content.to_lowercase().contains("<sitemapindex")
    }

    fn is_sitemap_xml(content: &str) -> bool {
        content.to_lowercase().contains("<urlset")
    }

    /// Decompress a gzipped sitemap body. Applies to URLs whose path ends in `.gz` and to
    /// responses served as `application/gzip` / `application/x-gzip`, and only when the body is
    /// gzip (magic bytes `1f 8b`) holding a sitemap or sitemap index of at most 50 MB. Anything
    /// else — e.g. a linked `.tar.gz` download — returns `None` and keeps its original bytes.
    pub fn gunzip_sitemap(url_path: &str, content_type: &str, body: &[u8]) -> Option<Vec<u8>> {
        if !Self::is_gzip_candidate(url_path, content_type) || !body.starts_with(&[0x1f, 0x8b]) {
            return None;
        }

        let mut decoder = flate2::read::GzDecoder::new(body).take(MAX_SITEMAP_BYTES + 1);
        let mut xml = Vec::new();
        (&mut decoder).take(SITEMAP_ROOT_WINDOW).read_to_end(&mut xml).ok()?;
        if !Self::starts_with_sitemap_root(&xml) {
            return None;
        }
        decoder.read_to_end(&mut xml).ok()?;
        (xml.len() as u64 <= MAX_SITEMAP_BYTES).then_some(xml)
    }

    /// Whether the response is a gzip sitemap whose body the HTTP client already decoded (a `.gz`
    /// file served with `Content-Encoding: gzip`): a `.gz` path or gzip content type, and a body
    /// that is sitemap XML.
    pub fn is_decoded_gzip_sitemap(url_path: &str, content_type: &str, body: &[u8]) -> bool {
        Self::is_gzip_candidate(url_path, content_type) && Self::starts_with_sitemap_root(body)
    }

    /// A URL path ending in `.gz` or a response served as `application/gzip` / `application/x-gzip`.
    fn is_gzip_candidate(url_path: &str, content_type: &str) -> bool {
        let content_type = content_type.to_ascii_lowercase();
        url_path.to_ascii_lowercase().ends_with(".gz")
            || content_type.contains("application/gzip")
            || content_type.contains("application/x-gzip")
    }

    /// Whether `body` is XML (after an optional BOM and whitespace) that names a sitemap root
    /// element (`<urlset>` / `<sitemapindex>`) within its first 64 KB, past any XML prolog. An
    /// archive that merely contains a sitemap file does not start with markup.
    fn starts_with_sitemap_root(body: &[u8]) -> bool {
        let head = &body[..body.len().min(SITEMAP_ROOT_WINDOW as usize)];
        let head = String::from_utf8_lossy(head);
        head.trim_start_matches('\u{feff}').trim_start().starts_with('<')
            && (Self::is_sitemap_xml(&head) || Self::is_sitemap_xml_index(&head))
    }

    /// Whether a URL path has a sitemap file extension: `.xml`, or `.gz` except a `.tar.gz` archive
    /// (`.tgz` does not end in `.gz`). Case-insensitive.
    pub fn has_sitemap_extension(path: &str) -> bool {
        let path = path.to_ascii_lowercase();
        path.ends_with(".xml") || (path.ends_with(".gz") && !path.ends_with(".tar.gz"))
    }

    /// Parse URLs from a sitemap.xml <urlset> document
    fn get_urls_from_sitemap_xml(content: &str) -> Vec<String> {
        Self::get_locs(content, None)
    }

    /// Parse the URLs of the sitemaps (`.xml`, `.xml.gz`, `.gz`) listed in a sitemap index document
    fn get_urls_from_sitemap_xml_index(content: &str) -> Vec<String> {
        Self::get_locs(content, Some(b"sitemap".as_slice()))
            .into_iter()
            .filter(|url| Self::is_xml_sitemap_url(url))
            .collect()
    }

    /// Whether a sitemap index entry points to a sitemap file (`.xml`, `.xml.gz` or another `.gz`,
    /// not a `.tar.gz` archive). Only the path counts, so a query string such as `?from=1&to=100`
    /// does not hide the extension; a `.gz` body is read only when it holds a sitemap root element.
    fn is_xml_sitemap_url(url: &str) -> bool {
        Self::has_sitemap_extension(url.split(['?', '#']).next().unwrap_or(url))
    }

    /// Text of every `<loc>` element — with `parent`, only of those inside that element. quick-xml
    /// reports `&amp;` and other references as separate events, so the pieces of a URL with a query
    /// string are joined here.
    fn get_locs(content: &str, parent: Option<&[u8]>) -> Vec<String> {
        let mut locs = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut in_parent = parent.is_none();
        let mut in_loc = false;
        let mut loc = String::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    if parent == Some(local_name.as_ref()) {
                        in_parent = true;
                    } else if local_name.as_ref() == b"loc" && in_parent {
                        in_loc = true;
                        loc.clear();
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_loc && let Ok(text) = e.decode() {
                        loc.push_str(&text);
                    }
                }
                Ok(Event::GeneralRef(ref e)) => {
                    if in_loc {
                        if let Ok(Some(ch)) = e.resolve_char_ref() {
                            loc.push(ch);
                        } else if let Ok(name) = e.decode()
                            && let Some(value) = quick_xml::escape::resolve_xml_entity(&name)
                        {
                            loc.push_str(value);
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"loc" && in_loc {
                        in_loc = false;
                        let url = loc.trim();
                        if !url.is_empty() {
                            locs.push(url.to_string());
                        }
                    } else if parent == Some(local_name.as_ref()) {
                        in_parent = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        locs
    }
}

impl ContentProcessor for XmlProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        let source_url_str = source_url.get_full_url(true, false);

        if Self::is_sitemap_xml_index(content) {
            let urls = Self::get_urls_from_sitemap_xml_index(content);
            if urls.is_empty() {
                return None;
            }

            let mut found_urls = FoundUrls::new();
            for url in urls {
                found_urls.add_url(FoundUrl::new(&url, &source_url_str, UrlSource::Sitemap));
            }
            return Some(found_urls);
        }

        if Self::is_sitemap_xml(content) {
            let urls = Self::get_urls_from_sitemap_xml(content);
            if urls.is_empty() {
                return None;
            }

            let mut found_urls = FoundUrls::new();
            for url in urls {
                found_urls.add_url(FoundUrl::new(&url, &source_url_str, UrlSource::Sitemap));
            }
            return Some(found_urls);
        }

        None
    }

    fn apply_content_changes_before_url_parsing(
        &self,
        _content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
    ) {
        // No changes needed before URL parsing in XmlProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        _content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // XML files don't need offline conversion
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "XmlProcessor"
    }

    fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ProcessorConfig {
        ProcessorConfig::new(ParsedUrl::parse("https://example.com/", None))
    }

    #[test]
    fn test_sitemap_xml() {
        let processor = XmlProcessor::new(make_config());
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url><loc>https://example.com/page1</loc></url>
                <url><loc>https://example.com/page2</loc></url>
            </urlset>"#;
        let source = ParsedUrl::parse("https://example.com/sitemap.xml", None);
        let result = processor.find_urls(xml, &source);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get_count(), 2);
    }

    #[test]
    fn test_sitemap_index() {
        let processor = XmlProcessor::new(make_config());
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <sitemap><loc>https://example.com/sitemap1.xml</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap2.xml</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap3.xml.gz</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap.tar.gz</loc></sitemap>
            </sitemapindex>"#;
        let source = ParsedUrl::parse("https://example.com/sitemap.xml", None);
        let result = processor.find_urls(xml, &source);
        assert!(result.is_some());
        // .xml and .xml.gz are included, but not .tar.gz
        assert_eq!(result.unwrap().get_count(), 3);
    }

    #[test]
    fn test_non_sitemap_xml() {
        let processor = XmlProcessor::new(make_config());
        let xml = r#"<?xml version="1.0"?><root><item>test</item></root>"#;
        let source = ParsedUrl::parse("https://example.com/data.xml", None);
        let result = processor.find_urls(xml, &source);
        assert!(result.is_none());
    }

    /// Test the full gzip decompression + XML parsing pipeline.
    /// Simulates what the crawler does when it fetches a .xml.gz sitemap:
    /// gzip-compressed bytes → decompress → parse XML → extract URLs.
    #[test]
    fn test_gzip_compressed_sitemap() {
        use flate2::Compression;
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url><loc>https://example.com/page1</loc></url>
                <url><loc>https://example.com/page2</loc></url>
                <url><loc>https://example.com/page3</loc></url>
            </urlset>"#;

        // Compress the XML (simulates what a .xml.gz file contains)
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify it's actually compressed (smaller or at least different)
        assert_ne!(compressed, xml.as_bytes());

        // Decompress (same logic as in Crawler::process_url for .xml.gz)
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed).unwrap();
        let decompressed_str = String::from_utf8(decompressed).unwrap();

        // Parse the decompressed XML with XmlProcessor
        let processor = XmlProcessor::new(make_config());
        let source = ParsedUrl::parse("https://example.com/sitemap.xml.gz", None);
        let result = processor.find_urls(&decompressed_str, &source);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get_count(), 3);
    }

    /// Same test for gzip-compressed sitemap index.
    #[test]
    fn test_gzip_compressed_sitemap_index() {
        use flate2::Compression;
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <sitemap><loc>https://example.com/sitemap-posts.xml</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap-pages.xml.gz</loc></sitemap>
            </sitemapindex>"#;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed).unwrap();
        let decompressed_str = String::from_utf8(decompressed).unwrap();

        let processor = XmlProcessor::new(make_config());
        let source = ParsedUrl::parse("https://example.com/sitemap-index.xml.gz", None);
        let result = processor.find_urls(&decompressed_str, &source);
        assert!(result.is_some());
        // Both .xml and .xml.gz URLs from the index
        assert_eq!(result.unwrap().get_count(), 2);
    }

    fn gzip(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    const URLSET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://example.com/a</loc></url></urlset>"#;

    #[test]
    fn gzip_sitemap_is_recognised_by_gz_path_or_gzip_content_type() {
        let body = gzip(URLSET.as_bytes());
        for (path, content_type) in [
            ("/sitemap.xml.gz", "application/octet-stream"),
            ("/sitemap.gz", "application/octet-stream"),
            ("/sitemaps/products", "application/gzip"),
            ("/sitemaps/products", "application/x-gzip; charset=binary"),
        ] {
            assert_eq!(
                XmlProcessor::gunzip_sitemap(path, content_type, &body).as_deref(),
                Some(URLSET.as_bytes()),
                "{path} ({content_type})"
            );
        }
    }

    #[test]
    fn gzip_is_left_alone_outside_gzip_sitemaps() {
        let body = gzip(URLSET.as_bytes());
        // A gzip body at an ordinary XML URL served as XML is not a candidate.
        assert_eq!(
            XmlProcessor::gunzip_sitemap("/sitemap.xml", "application/xml", &body),
            None
        );
        // A `.gz` URL whose body the HTTP layer already decoded (Content-Encoding: gzip).
        assert_eq!(
            XmlProcessor::gunzip_sitemap("/sitemap.xml.gz", "application/xml", URLSET.as_bytes()),
            None
        );
        // A gzip archive that is not a sitemap keeps its original bytes.
        assert_eq!(
            XmlProcessor::gunzip_sitemap("/downloads/tool.tar.gz", "application/gzip", &gzip(b"not a sitemap")),
            None
        );
    }

    #[test]
    fn decoded_gzip_sitemap_at_gz_path_is_recognised() {
        // The HTTP client already decoded `Content-Encoding: gzip` of a `.gz` file.
        for body in [URLSET.to_string(), format!("\u{feff}\n  {URLSET}")] {
            assert!(
                XmlProcessor::is_decoded_gzip_sitemap("/sitemap.gz", "application/x-gzip", body.as_bytes()),
                "{body}"
            );
        }
        assert!(XmlProcessor::is_decoded_gzip_sitemap(
            "/sitemaps/products",
            "application/gzip",
            br#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></sitemapindex>"#
        ));
        // Not a gzip sitemap URL, still compressed, or not a sitemap.
        assert!(!XmlProcessor::is_decoded_gzip_sitemap(
            "/sitemap.xml",
            "application/xml",
            URLSET.as_bytes()
        ));
        assert!(!XmlProcessor::is_decoded_gzip_sitemap(
            "/sitemap.gz",
            "application/x-gzip",
            &gzip(URLSET.as_bytes())
        ));
        assert!(!XmlProcessor::is_decoded_gzip_sitemap(
            "/sitemap.gz",
            "application/x-gzip",
            b"<html><body>urlset</body></html>"
        ));
    }

    #[test]
    fn archive_holding_a_sitemap_is_not_a_sitemap() {
        let tar = format!("sitemap.xml\0\0\0\0{URLSET}");
        assert_eq!(
            XmlProcessor::gunzip_sitemap("/downloads/sitemaps.tar.gz", "application/gzip", &gzip(tar.as_bytes())),
            None
        );
        assert!(!XmlProcessor::is_decoded_gzip_sitemap(
            "/downloads/sitemaps.tar.gz",
            "application/gzip",
            tar.as_bytes()
        ));
    }

    #[test]
    fn sitemap_index_entry_with_query_string_is_kept_whole() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <sitemap><loc>https://example.com/sitemap_products_1.xml?from=1&amp;to=100</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap_pages_1.xml.gz?v=2</loc></sitemap>
                <sitemap><loc>https://example.com/feed?format=xml</loc></sitemap>
            </sitemapindex>"#;
        assert_eq!(
            XmlProcessor::get_urls_from_sitemap_xml_index(xml),
            vec![
                "https://example.com/sitemap_products_1.xml?from=1&to=100".to_string(),
                "https://example.com/sitemap_pages_1.xml.gz?v=2".to_string(),
            ]
        );
    }

    #[test]
    fn sitemap_index_accepts_gz_entries_but_not_archives() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <sitemap><loc>https://example.com/sitemap-1.gz</loc></sitemap>
                <sitemap><loc>https://example.com/feeds/products.GZ?v=1</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap-backup.tar.gz</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap-backup.tgz</loc></sitemap>
            </sitemapindex>"#;
        assert_eq!(
            XmlProcessor::get_urls_from_sitemap_xml_index(xml),
            vec![
                "https://example.com/sitemap-1.gz".to_string(),
                "https://example.com/feeds/products.GZ?v=1".to_string(),
            ]
        );
    }

    #[test]
    fn urlset_loc_with_escaped_characters_is_one_url() {
        let xml = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url><loc>https://example.com/search?a=1&amp;b=2&#38;c=3</loc></url>
            </urlset>"#;
        assert_eq!(
            XmlProcessor::get_urls_from_sitemap_xml(xml),
            vec!["https://example.com/search?a=1&b=2&c=3".to_string()]
        );
    }
}
