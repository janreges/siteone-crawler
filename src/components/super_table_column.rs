// SiteOne Crawler - SuperTableColumn
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::Serialize;
use std::collections::HashMap;

pub const AUTO_WIDTH: i32 = -1;

pub type FormatterFn = Box<dyn Fn(&str, &str) -> String + Send + Sync>;
pub type RendererFn = Box<dyn Fn(&HashMap<String, String>, &str) -> String + Send + Sync>;
pub type DataValueCallbackFn = Box<dyn Fn(&HashMap<String, String>) -> String + Send + Sync>;

#[derive(Serialize)]
pub struct SuperTableColumn {
    pub apl_code: String,
    pub name: String,
    pub width: i32,
    #[serde(skip)]
    pub formatter: Option<FormatterFn>,
    #[serde(skip)]
    pub renderer: Option<RendererFn>,
    pub truncate_if_longer: bool,
    pub formatter_will_change_value_length: bool,
    pub non_breaking_spaces: bool,
    pub escape_output_html: bool,
    #[serde(skip)]
    pub get_data_value_callback: Option<DataValueCallbackFn>,
    pub forced_data_type: Option<String>,
}

impl std::fmt::Debug for SuperTableColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SuperTableColumn")
            .field("apl_code", &self.apl_code)
            .field("name", &self.name)
            .field("width", &self.width)
            .field("truncate_if_longer", &self.truncate_if_longer)
            .field(
                "formatter_will_change_value_length",
                &self.formatter_will_change_value_length,
            )
            .field("non_breaking_spaces", &self.non_breaking_spaces)
            .field("escape_output_html", &self.escape_output_html)
            .field("forced_data_type", &self.forced_data_type)
            .finish()
    }
}

impl SuperTableColumn {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        apl_code: String,
        name: String,
        width: i32,
        formatter: Option<FormatterFn>,
        renderer: Option<RendererFn>,
        truncate_if_longer: bool,
        formatter_will_change_value_length: bool,
        non_breaking_spaces: bool,
        escape_output_html: bool,
        get_data_value_callback: Option<DataValueCallbackFn>,
    ) -> Self {
        Self {
            apl_code,
            name,
            width,
            formatter,
            renderer,
            truncate_if_longer,
            formatter_will_change_value_length,
            non_breaking_spaces,
            escape_output_html,
            get_data_value_callback,
            forced_data_type: None,
        }
    }

    pub fn get_width_px(&self) -> i32 {
        self.width * 8
    }

    pub fn get_auto_width_by_data(&self, data: &[HashMap<String, String>]) -> usize {
        let mut max_width = self.name.chars().count();

        for row in data {
            let value = row.get(&self.apl_code);
            match value {
                None => continue,
                Some(v) if v.is_empty() => continue,
                Some(v) => {
                    if self.formatter.is_some() && self.formatter_will_change_value_length {
                        if let Some(ref fmt) = self.formatter {
                            let formatted = fmt(v, "console");
                            max_width = max_width.max(formatted.chars().count());
                        }
                    } else {
                        max_width = max_width.max(v.chars().count());
                    }
                }
            }
        }

        max_width.min(1000)
    }

    pub fn get_data_value(&self, row: &HashMap<String, String>) -> String {
        if let Some(ref callback) = self.get_data_value_callback {
            return callback(row);
        }
        row.get(&self.apl_code).cloned().unwrap_or_default()
    }
}
