// SiteOne Crawler - Option group for organizing options
// (c) Jan Reges <jan.reges@siteone.cz>
//

use indexmap::IndexMap;

use super::option::CrawlerOption;

#[derive(Debug, Clone)]
pub struct OptionGroup {
    /// Unique application code for the group
    pub apl_code: String,

    /// Readable name for the group
    pub name: String,

    /// Options indexed by property_to_fill name
    pub options: IndexMap<String, CrawlerOption>,
}

impl OptionGroup {
    pub fn new(apl_code: &str, name: &str, options: Vec<CrawlerOption>) -> Self {
        let mut options_map = IndexMap::new();
        for option in options {
            options_map.insert(option.property_to_fill.clone(), option);
        }

        Self {
            apl_code: apl_code.to_string(),
            name: name.to_string(),
            options: options_map,
        }
    }
}
