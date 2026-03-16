// SiteOne Crawler - Options registry
// (c) Jan Reges <jan.reges@siteone.cz>
//

use indexmap::IndexMap;

use super::group::OptionGroup;

#[derive(Debug, Clone)]
pub struct Options {
    groups: IndexMap<String, OptionGroup>,
}

impl Options {
    pub fn new() -> Self {
        Self {
            groups: IndexMap::new(),
        }
    }

    pub fn add_group(&mut self, group: OptionGroup) {
        self.groups.insert(group.apl_code.clone(), group);
    }

    pub fn get_groups(&self) -> &IndexMap<String, OptionGroup> {
        &self.groups
    }

    pub fn get_groups_mut(&mut self) -> &mut IndexMap<String, OptionGroup> {
        &mut self.groups
    }

    pub fn get_group(&self, apl_code: &str) -> Option<&OptionGroup> {
        self.groups.get(apl_code)
    }

    pub fn get_group_mut(&mut self, apl_code: &str) -> Option<&mut OptionGroup> {
        self.groups.get_mut(apl_code)
    }

    /// Check if a specific option was explicitly provided on the command line
    /// (as opposed to using its default value). `property` is the camelCase property name.
    pub fn is_explicitly_set(&self, property: &str) -> bool {
        self.groups
            .values()
            .any(|g| g.options.get(property).is_some_and(|o| o.is_explicitly_set()))
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}
