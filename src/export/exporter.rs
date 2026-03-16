// SiteOne Crawler - Exporter trait
// (c) Jan Reges <jan.reges@siteone.cz>
//

use crate::error::CrawlerResult;
use crate::output::output::Output;
use crate::result::status::Status;

/// Trait for all exporters (file, sitemap, upload, mailer, offline, markdown).
/// Each exporter can save crawl results in a different format or send them somewhere.
pub trait Exporter: Send + Sync {
    /// Get the name of this exporter (for logging/debugging).
    fn get_name(&self) -> &str;

    /// Should this exporter be activated based on the provided options?
    fn should_be_activated(&self) -> bool;

    /// Perform the export (save to file, send to server, etc.).
    /// Uses the Output trait to report progress/results to the user.
    fn export(&mut self, status: &Status, output: &dyn Output) -> CrawlerResult<()>;
}
