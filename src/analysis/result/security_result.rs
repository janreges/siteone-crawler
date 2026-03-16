// SiteOne Crawler - SecurityResult
// (c) Jan Reges <jan.reges@siteone.cz>

use indexmap::IndexMap;

use super::security_checked_header::{SEVERITY_OK, SecurityCheckedHeader};

#[derive(Debug, Clone, Default)]
pub struct SecurityResult {
    pub checked_headers: IndexMap<String, SecurityCheckedHeader>,
}

impl SecurityResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_checked_header(&mut self, header: &str) -> &mut SecurityCheckedHeader {
        self.checked_headers
            .entry(header.to_string())
            .or_insert_with(|| SecurityCheckedHeader::new(header.to_string()))
    }

    pub fn get_highest_severity(&self) -> i32 {
        let mut highest = SEVERITY_OK;
        for item in self.checked_headers.values() {
            if let Some(sev) = item.highest_severity
                && sev > highest
            {
                highest = sev;
            }
        }
        highest
    }
}
