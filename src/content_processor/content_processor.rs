// SiteOne Crawler - ContentProcessor trait
// (c) Jan Reges <jan.reges@siteone.cz>

use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

/// Trait for content processors that extract URLs and modify content
/// for offline versions.
pub trait ContentProcessor: Send + Sync {
    /// Parse and find framework specific URLs in HTML/CSS/JS
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls>;

    /// Apply content changes for HTML/CSS/JS before URL parsing,
    /// directly modifying the content string.
    /// Called by manager only if is_content_type_relevant() returns true.
    fn apply_content_changes_before_url_parsing(
        &self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
    );

    /// Apply content changes for offline version of the file,
    /// directly modifying the content (HTML/CSS/JS) string.
    /// Called by manager only if is_content_type_relevant() returns true.
    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        remove_unwanted_code: bool,
    );

    /// Apply content changes for offline version with a content loader callback.
    /// The loader takes a URL string and returns its body text if available.
    /// Default implementation delegates to apply_content_changes_for_offline_version.
    /// Only AstroProcessor overrides this to inline modules from storage.
    fn apply_content_changes_for_offline_version_with_loader(
        &self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        remove_unwanted_code: bool,
        _content_loader: &dyn Fn(&str) -> Option<String>,
    ) {
        self.apply_content_changes_for_offline_version(content, content_type, url, remove_unwanted_code);
    }

    /// Check if this ContentProcessor is relevant for given content type
    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool;

    /// Get the name of this processor (used for stats/logging)
    fn get_name(&self) -> &str;

    /// Enable/disable debug mode
    fn set_debug_mode(&mut self, debug_mode: bool);
}
