// SiteOne Crawler - Summary
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};

use crate::components::summary::item::Item;
use crate::components::summary::item_status::ItemStatus;
use crate::utils;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    items: Vec<Item>,
}

impl Summary {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add_item(&mut self, item: Item) {
        self.items.push(item);
    }

    pub fn get_items(&self) -> &[Item] {
        &self.items
    }

    fn sort_items(&mut self) {
        self.items.sort_by_key(|item| item.status.sort_order());
    }

    pub fn get_as_html(&mut self) -> String {
        let mut result = String::from("<ul>\n");
        self.sort_items();
        for item in &self.items {
            result.push_str(&format!("    <li>{}</li>\n", item.get_as_html()));
        }
        result.push_str("</ul>");
        result
    }

    pub fn get_as_console_text(&mut self) -> String {
        let title = "Summary";
        let title_output = format!("{}\n{}\n\n", title, "-".repeat(title.len()));
        let mut result = utils::get_color_text(&title_output, "blue", false);

        self.sort_items();
        for item in &self.items {
            result.push_str(&item.get_as_console_text());
            result.push('\n');
        }
        result
    }

    pub fn get_count_by_item_status(&self, status: ItemStatus) -> usize {
        self.items.iter().filter(|item| item.status == status).count()
    }
}
