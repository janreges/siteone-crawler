// SiteOne Crawler - XmlProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Extracts URLs from sitemap.xml and sitemap index files.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::content_processor::base_processor::{ProcessorConfig, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_url::{FoundUrl, UrlSource};
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

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

    /// Parse URLs from a sitemap.xml <urlset> document
    fn get_urls_from_sitemap_xml(content: &str) -> Vec<String> {
        let mut urls = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut in_loc = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"loc" {
                        in_loc = true;
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_loc && let Ok(text) = e.decode() {
                        let url = text.trim().to_string();
                        if !url.is_empty() {
                            urls.push(url);
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"loc" {
                        in_loc = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        urls
    }

    /// Parse URLs from a sitemap index document
    fn get_urls_from_sitemap_xml_index(content: &str) -> Vec<String> {
        let mut urls = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut in_sitemap = false;
        let mut in_loc = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"sitemap" {
                        in_sitemap = true;
                    } else if local_name.as_ref() == b"loc" && in_sitemap {
                        in_loc = true;
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_loc && let Ok(text) = e.decode() {
                        let url = text.trim().to_string();
                        let url_lower = url.to_lowercase();
                        // Include .xml and .xml.gz sitemap URLs
                        if url_lower.ends_with(".xml") || url_lower.ends_with(".xml.gz") {
                            urls.push(url);
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local_name = e.local_name();
                    if local_name.as_ref() == b"loc" {
                        in_loc = false;
                    } else if local_name.as_ref() == b"sitemap" {
                        in_sitemap = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        urls
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
}
