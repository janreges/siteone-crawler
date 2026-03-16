// SiteOne Crawler - FileStorage
// (c) Jan Reges <jan.reges@siteone.cz>

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{CrawlerError, CrawlerResult};
use crate::result::storage::storage::Storage;

pub struct FileStorage {
    cache_dir: PathBuf,
    compress: bool,
}

impl FileStorage {
    pub fn new(tmp_dir: &str, compress: bool, origin_url_domain: &str) -> CrawlerResult<Self> {
        // Sanitize domain name for use as directory name
        let sanitized_domain = match Regex::new(r"[^a-zA-Z0-9.\-_]") {
            Ok(re) => re.replace_all(&origin_url_domain.to_lowercase(), "-").to_string(),
            _ => origin_url_domain.to_lowercase(),
        };

        let cache_dir = PathBuf::from(tmp_dir).join(sanitized_domain);

        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                CrawlerError::Io(std::io::Error::other(format!(
                    "Directory '{}' was not created: {}",
                    cache_dir.display(),
                    e
                )))
            })?;
        }

        Ok(Self { cache_dir, compress })
    }

    fn get_file_extension(&self) -> &str {
        if self.compress { "cache.gz" } else { "cache" }
    }

    fn get_file_path(&self, uq_id: &str) -> PathBuf {
        debug_assert!(
            uq_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "uq_id '{}' contains unsafe characters",
            uq_id
        );
        let subdir = if uq_id.len() >= 2 { &uq_id[..2] } else { uq_id };
        self.cache_dir
            .join(subdir)
            .join(format!("{}.{}", uq_id, self.get_file_extension()))
    }

    fn create_directory_if_needed(&self, path: &Path) -> CrawlerResult<()> {
        if !path.exists() {
            fs::create_dir_all(path).map_err(|e| {
                CrawlerError::Io(std::io::Error::other(format!(
                    "Directory '{}' was not created. Please check permissions: {}",
                    path.display(),
                    e
                )))
            })?;
        }
        Ok(())
    }
}

impl Storage for FileStorage {
    fn save(&mut self, uq_id: &str, content: &[u8]) -> CrawlerResult<()> {
        let data = if self.compress {
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(content).map_err(CrawlerError::Io)?;
            encoder.finish().map_err(CrawlerError::Io)?
        } else {
            content.to_vec()
        };

        let file_path = self.get_file_path(uq_id);
        if let Some(parent) = file_path.parent() {
            self.create_directory_if_needed(parent)?;
        }

        fs::write(&file_path, &data).map_err(CrawlerError::Io)
    }

    fn load(&self, uq_id: &str) -> CrawlerResult<Vec<u8>> {
        let file_path = self.get_file_path(uq_id);

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read(&file_path).map_err(CrawlerError::Io)?;

        if self.compress {
            let mut decoder = flate2::read::GzDecoder::new(&data[..]);
            let mut decompressed = Vec::new();
            std::io::Read::read_to_end(&mut decoder, &mut decompressed).map_err(CrawlerError::Io)?;
            Ok(decompressed)
        } else {
            Ok(data)
        }
    }

    fn delete(&mut self, uq_id: &str) -> CrawlerResult<()> {
        let file_path = self.get_file_path(uq_id);
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(CrawlerError::Io)?;
        }
        Ok(())
    }

    fn delete_all(&mut self) -> CrawlerResult<()> {
        if self.cache_dir.exists() {
            // Remove all files recursively within cache_dir, then recreate
            fs::remove_dir_all(&self.cache_dir).map_err(CrawlerError::Io)?;
            fs::create_dir_all(&self.cache_dir).map_err(CrawlerError::Io)?;
        }
        Ok(())
    }
}

impl Drop for FileStorage {
    fn drop(&mut self) {
        // Clean up cache directory on drop
        let _ = self.delete_all();
    }
}

impl std::fmt::Debug for FileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStorage")
            .field("cache_dir", &self.cache_dir)
            .field("compress", &self.compress)
            .finish()
    }
}
