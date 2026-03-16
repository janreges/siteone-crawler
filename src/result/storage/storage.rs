// SiteOne Crawler - Storage trait
// (c) Jan Reges <jan.reges@siteone.cz>

use crate::error::CrawlerResult;

pub trait Storage: Send + Sync {
    fn save(&mut self, uq_id: &str, content: &[u8]) -> CrawlerResult<()>;

    fn load(&self, uq_id: &str) -> CrawlerResult<Vec<u8>>;

    fn delete(&mut self, uq_id: &str) -> CrawlerResult<()>;

    fn delete_all(&mut self) -> CrawlerResult<()>;
}
