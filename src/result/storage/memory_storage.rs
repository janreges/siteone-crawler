// SiteOne Crawler - MemoryStorage
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::io::Write;

use crate::error::{CrawlerError, CrawlerResult};
use crate::result::storage::storage::Storage;

pub struct MemoryStorage {
    storage: HashMap<String, Vec<u8>>,
    compress: bool,
}

impl MemoryStorage {
    pub fn new(compress: bool) -> Self {
        Self {
            storage: HashMap::new(),
            compress,
        }
    }
}

impl Storage for MemoryStorage {
    fn save(&mut self, uq_id: &str, content: &[u8]) -> CrawlerResult<()> {
        let data = if self.compress {
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(content).map_err(CrawlerError::Io)?;
            encoder.finish().map_err(CrawlerError::Io)?
        } else {
            content.to_vec()
        };

        self.storage.insert(uq_id.to_string(), data);
        Ok(())
    }

    fn load(&self, uq_id: &str) -> CrawlerResult<Vec<u8>> {
        match self.storage.get(uq_id) {
            Some(data) if !data.is_empty() => {
                if self.compress {
                    let mut decoder = flate2::read::GzDecoder::new(&data[..]);
                    let mut decompressed = Vec::new();
                    std::io::Read::read_to_end(&mut decoder, &mut decompressed).map_err(CrawlerError::Io)?;
                    Ok(decompressed)
                } else {
                    Ok(data.clone())
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    fn delete(&mut self, uq_id: &str) -> CrawlerResult<()> {
        self.storage.remove(uq_id);
        Ok(())
    }

    fn delete_all(&mut self) -> CrawlerResult<()> {
        self.storage.clear();
        Ok(())
    }
}

impl std::fmt::Debug for MemoryStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryStorage")
            .field("entries", &self.storage.len())
            .field("compress", &self.compress)
            .finish()
    }
}
