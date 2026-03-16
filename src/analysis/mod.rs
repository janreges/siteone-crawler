pub mod analyzer;
pub mod base_analyzer;
pub mod manager;
pub mod result;

// Simple analyzers
pub mod caching_analyzer;
pub mod content_type_analyzer;
pub mod dns_analyzer;
pub mod external_links_analyzer;
pub mod fastest_analyzer;
pub mod headers_analyzer;
pub mod page404_analyzer;
pub mod redirects_analyzer;
pub mod skipped_urls_analyzer;
pub mod slowest_analyzer;
pub mod source_domains_analyzer;

// Complex analyzers (DOM parsing / TLS inspection)
pub mod accessibility_analyzer;
pub mod best_practice_analyzer;
pub mod security_analyzer;
pub mod seo_opengraph_analyzer;
pub mod ssl_tls_analyzer;
