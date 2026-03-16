// SiteOne Crawler - SuperTable
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

use crate::components::super_table_column::SuperTableColumn;
use crate::utils;

static RE_RELATIVE_URL_PATH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^/[a-z0-9\-_./?\&#+=%%@()|]*$").unwrap());

pub const POSITION_BEFORE_URL_TABLE: &str = "before-url-table";
pub const POSITION_AFTER_URL_TABLE: &str = "after-url-table";

pub const RENDER_INTO_HTML: &str = "html";
pub const RENDER_INTO_CONSOLE: &str = "console";

static HARD_ROWS_LIMIT: RwLock<usize> = RwLock::new(200);

#[derive(Debug, Serialize)]
pub struct SuperTable {
    pub apl_code: String,
    pub title: String,
    pub description: Option<String>,
    pub max_rows: Option<usize>,
    pub forced_tab_label: Option<String>,

    #[serde(skip)]
    visible_in_html: bool,
    #[serde(skip)]
    visible_in_json: bool,
    #[serde(skip)]
    visible_in_console: bool,
    #[serde(skip)]
    visible_in_console_rows_limit: Option<usize>,
    #[serde(skip)]
    show_only_columns_with_values: bool,

    #[serde(skip)]
    columns: Vec<SuperTableColumn>,
    #[serde(skip)]
    position_before_url_table: bool,
    #[serde(skip)]
    data: Vec<HashMap<String, String>>,
    #[serde(skip)]
    empty_table_message: String,
    #[serde(skip)]
    current_order_column: Option<String>,
    #[serde(skip)]
    current_order_direction: String,
    #[serde(skip)]
    unique_id: String,
    #[serde(skip)]
    host_to_strip_from_urls: Option<String>,
    #[serde(skip)]
    scheme_of_host_to_strip_from_urls: Option<String>,
    #[serde(skip)]
    initial_url: Option<String>,
    #[serde(skip)]
    fulltext_enabled: bool,
    #[serde(skip)]
    min_rows_for_fulltext: usize,
    #[serde(skip)]
    ignore_hard_rows_limit: bool,
    #[serde(skip)]
    max_hard_rows_limit_reached: bool,
}

impl SuperTable {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        apl_code: String,
        title: String,
        empty_table_message: String,
        columns: Vec<SuperTableColumn>,
        position_before_url_table: bool,
        current_order_column: Option<String>,
        current_order_direction: String,
        description: Option<String>,
        max_rows: Option<usize>,
        forced_tab_label: Option<String>,
    ) -> Self {
        let unique_id = generate_unique_id();

        Self {
            apl_code,
            title,
            empty_table_message,
            columns,
            position_before_url_table,
            current_order_column,
            current_order_direction,
            description,
            max_rows,
            forced_tab_label,
            unique_id,
            visible_in_html: true,
            visible_in_json: true,
            visible_in_console: true,
            visible_in_console_rows_limit: None,
            show_only_columns_with_values: false,
            data: Vec::new(),
            host_to_strip_from_urls: None,
            scheme_of_host_to_strip_from_urls: None,
            initial_url: None,
            fulltext_enabled: true,
            min_rows_for_fulltext: 10,
            ignore_hard_rows_limit: false,
            max_hard_rows_limit_reached: false,
        }
    }

    pub fn set_data(&mut self, data: Vec<HashMap<String, String>>) {
        self.data = data;
        if let Some(ref col) = self.current_order_column.clone() {
            let dir = self.current_order_direction.clone();
            self.sort_data(col, &dir);
        }
        self.apply_hard_rows_limit();
        self.remove_columns_with_empty_data();
    }

    pub fn get_html_output(&self) -> String {
        if !self.visible_in_html {
            return String::new();
        }

        let mut output = format!("<h2>{}</h2>", html_escape(&self.title));

        if self.data.is_empty() {
            output.push_str(&format!("<p>{}</p>", html_escape(&self.empty_table_message)));
            return output;
        } else if let Some(ref desc) = self.description {
            output.push_str(desc);
            output.push_str("<br>");
        }

        if self.is_fulltext_enabled() {
            output.push_str("<div class=\"fulltext-container\">");
            output.push_str(&format!(
                "    <input type=\"text\" class=\"fulltext\" data-uq-id=\"{}\" style=\"width: 300px;\" placeholder=\"Fulltext search\">",
                html_escape(&self.unique_id)
            ));
            output.push_str(&format!(
                "    <span id=\"foundRows_{}\" class=\"found-rows\">Found {} row(s).</span>",
                html_escape(&self.unique_id),
                self.data.len()
            ));
            output.push_str("</div>");
        }

        let show_more = self.data.len() > 20;

        let mut extra_classes = vec![self.apl_code.clone()];
        if show_more {
            extra_classes.push("table-with-show-more".to_string());
        }

        output.push_str(&format!(
            "<div class='table-container-top{}'>",
            if show_more { " show-more" } else { "" }
        ));
        if show_more {
            output.push_str(&format!(
                "<input id='showMore_{}' name='showMore' class='show-more-checkbox' type='checkbox' />",
                html_escape(&self.unique_id)
            ));
        }
        output.push_str(&format!(
            "<div class='table-container{}'>",
            if show_more { " show-more" } else { "" }
        ));
        output.push_str(&format!(
            "<table id='{}' border='1' class='table table-bordered table-hover table-sortable {}' style='border-collapse: collapse;'>",
            html_escape(&self.unique_id),
            extra_classes.join(" ")
        ));

        // thead
        output.push_str("<thead>");
        for column in &self.columns {
            let direction = if self.current_order_column.as_deref() == Some(&column.apl_code)
                && self.current_order_direction == "ASC"
            {
                "DESC"
            } else {
                "ASC"
            };

            let arrow = if self.current_order_column.as_deref() == Some(&column.apl_code) {
                if self.current_order_direction == "ASC" {
                    "&nbsp;&#128316;"
                } else {
                    "&nbsp;&#128317;"
                }
            } else {
                ""
            };

            let data_type = column.forced_data_type.as_deref().unwrap_or_else(|| {
                if let Some(first_row) = self.data.first()
                    && let Some(val) = first_row.get(&column.apl_code)
                    && val.parse::<f64>().is_ok()
                {
                    return "number";
                }
                "string"
            });

            output.push_str(&format!(
                "<th class='sortable-th' data-key='{}' data-type='{}' data-direction='{}' data-label='{}' data-uq-id='{}'>{}{}</th>",
                column.apl_code,
                data_type,
                direction,
                html_escape(&column.name),
                html_escape(&self.unique_id),
                html_escape(&column.name),
                arrow
            ));
        }

        let initial_root_url = self.initial_url.as_ref().and_then(|url| {
            let re = regex::Regex::new(r"^(https?://[^/]+).*$").ok()?;
            re.captures(url)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str().to_string())
        });

        output.push_str("</thead>");
        output.push_str("<tbody>");

        let mut counter = 1usize;
        let mut max_rows_reached = false;

        for row in &self.data {
            if let Some(max) = self.max_rows
                && counter > max
            {
                max_rows_reached = true;
                break;
            }

            output.push_str("<tr>");
            for column in &self.columns {
                let value = row.get(&column.apl_code).cloned().unwrap_or_default();
                let mut formatted_value = value.clone();

                if let Some(ref fmt) = column.formatter {
                    formatted_value = fmt(&value, RENDER_INTO_HTML);
                } else if let Some(ref rend) = column.renderer {
                    formatted_value = rend(row, RENDER_INTO_HTML);
                }

                if column.escape_output_html {
                    formatted_value = html_escape(&formatted_value);
                }

                if column.non_breaking_spaces {
                    formatted_value = formatted_value
                        .replace(' ', "&nbsp;")
                        .replace('\t', "&nbsp;&nbsp;&nbsp;&nbsp;");
                }

                // colored text
                if formatted_value.contains("[0;") || formatted_value.contains("[1;") || formatted_value.contains("[0m")
                {
                    formatted_value = crate::utils::convert_bash_colors_in_text_to_html(&formatted_value);
                }

                // full URL in value — skip if a renderer/formatter already produced custom HTML
                let has_custom_formatter = column.formatter.is_some() || column.renderer.is_some();
                if !has_custom_formatter && value.starts_with("http") {
                    let truncated = utils::truncate_url(
                        &value,
                        100,
                        "\u{2026}",
                        self.host_to_strip_from_urls.as_deref(),
                        self.scheme_of_host_to_strip_from_urls.as_deref(),
                        Some(false),
                    );
                    formatted_value = format!("<a href='{}' target='_blank'>{}</a>", html_escape(&value), truncated);
                } else if !has_custom_formatter && formatted_value.starts_with("http") {
                    let truncated = utils::truncate_url(
                        &formatted_value,
                        100,
                        "\u{2026}",
                        self.host_to_strip_from_urls.as_deref(),
                        self.scheme_of_host_to_strip_from_urls.as_deref(),
                        Some(false),
                    );
                    formatted_value = format!(
                        "<a href='{}' target='_blank'>{}</a>",
                        html_escape(&formatted_value),
                        truncated
                    );
                } else if !has_custom_formatter
                    && let Some(ref root_url) = initial_root_url
                    && formatted_value.starts_with('/')
                    && RE_RELATIVE_URL_PATH.is_match(&formatted_value)
                {
                    let final_url = format!("{}{}", root_url, formatted_value);
                    let truncated = utils::truncate_url(
                        &formatted_value,
                        100,
                        "\u{2026}",
                        self.host_to_strip_from_urls.as_deref(),
                        self.scheme_of_host_to_strip_from_urls.as_deref(),
                        Some(false),
                    );
                    formatted_value = format!(
                        "<a href='{}' target='_blank'>{}</a>",
                        html_escape(&final_url),
                        truncated
                    );
                }

                let data_value = if column.get_data_value_callback.is_some() {
                    column.get_data_value(row)
                } else if value.len() < 200 {
                    value.clone()
                } else if formatted_value.len() < 50 {
                    formatted_value.clone()
                } else {
                    "complex-data".to_string()
                };

                output.push_str(&format!(
                    "<td data-value='{}' class='{}'>{}</td>",
                    html_escape(&data_value),
                    html_escape(&column.apl_code),
                    formatted_value
                ));
            }
            output.push_str("</tr>");
            counter += 1;
        }

        if self.data.is_empty() {
            output.push_str(&format!(
                "<tr><td colspan='{}' class='warning'>{}</td></tr>",
                self.columns.len(),
                html_escape(&self.empty_table_message)
            ));
        } else if max_rows_reached {
            output.push_str(&format!(
                "<tr><td colspan='{}' class='warning'>You have reached the limit of {} rows as a protection against very large output or exhausted memory.</td></tr>",
                self.columns.len(),
                self.max_rows.unwrap_or(0)
            ));
        } else if self.max_hard_rows_limit_reached {
            let limit = HARD_ROWS_LIMIT.read().map(|v| *v).unwrap_or(200);
            output.push_str(&format!(
                "<tr><td colspan='{}' class='warning'>You have reached the hard limit of {} rows as a protection against very large output or exhausted memory. You can change this with <code>--rows-limit</code>.</td></tr>",
                self.columns.len(),
                limit
            ));
        }

        output.push_str("</tbody>");

        if self.is_fulltext_enabled() {
            output.push_str("<tfoot>");
            output.push_str(&format!(
                "  <tr class='empty-fulltext'><td colspan='{}' class='warning'>No rows found, please edit your search term.</td></tr>",
                self.columns.len()
            ));
            output.push_str("</tfoot>");
        }

        output.push_str("</table></div>");

        if show_more {
            output.push_str(&format!(
                "<label for='showMore_{}' class='show-more-label'>(+) Show entire table</label>",
                html_escape(&self.unique_id)
            ));
        }
        output.push_str("</div>");

        output
    }

    pub fn get_console_output(&self) -> String {
        let title_output = format!("{}\n{}\n\n", self.title, "-".repeat(self.title.chars().count()));
        let mut output = utils::get_color_text(&title_output, "blue", false);

        let data = &self.data;

        if data.is_empty() {
            output.push_str(&utils::get_color_text(&self.empty_table_message, "gray", false));
            output.push_str("\n\n");
            return output;
        } else if !self.visible_in_console {
            output.push_str(&utils::get_color_text(
                "This table contains large data. To see them, use output to HTML using `--output-html-report=tmp/myreport.html`.",
                "yellow",
                false,
            ));
            output.push_str("\n\n");
            return output;
        }

        let display_data: &[HashMap<String, String>] = if let Some(limit) = self.visible_in_console_rows_limit {
            output.push_str(&utils::get_color_text(
                    &format!(
                        "This table contains large data and shows max {} rows. To see them all, use output to HTML using `--output-html-report=tmp/myreport.html`.",
                        limit
                    ),
                    "yellow",
                    false,
                ));
            output.push_str("\n\n");
            &data[..limit.min(data.len())]
        } else {
            data
        };

        // Calculate column widths
        let column_widths: Vec<usize> = self
            .columns
            .iter()
            .map(|col| {
                if col.width == super::super_table_column::AUTO_WIDTH {
                    col.get_auto_width_by_data(&self.data)
                } else {
                    col.width as usize
                }
            })
            .collect();

        // Headers
        let headers: Vec<String> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| utils::mb_str_pad(&col.name, column_widths[i], ' '))
            .collect();
        output.push_str(&utils::get_color_text(&headers.join(" | "), "gray", false));
        output.push('\n');

        // Separator
        let total_width: usize = column_widths.iter().sum::<usize>() + (self.columns.len() * 3) - 1;
        output.push_str(&"-".repeat(total_width));
        output.push('\n');

        // Rows
        for row in display_data {
            let mut row_data = Vec::new();
            for (i, column) in self.columns.iter().enumerate() {
                let value = row.get(&column.apl_code).cloned().unwrap_or_default();
                let col_width = column_widths[i];

                let mut display_value = if let Some(ref fmt) = column.formatter {
                    fmt(&value, RENDER_INTO_CONSOLE)
                } else if let Some(ref rend) = column.renderer {
                    rend(row, RENDER_INTO_CONSOLE)
                } else {
                    value
                };

                // Strip protocol+domain from same-domain URLs in console output
                if display_value.starts_with("http") {
                    display_value = utils::truncate_url(
                        &display_value,
                        col_width,
                        "\u{2026}",
                        self.host_to_strip_from_urls.as_deref(),
                        self.scheme_of_host_to_strip_from_urls.as_deref(),
                        None,
                    );
                }

                if column.truncate_if_longer && display_value.chars().count() > col_width {
                    display_value = utils::truncate_in_two_thirds(&display_value, col_width, "\u{2026}", None);
                }

                // Always use ANSI-aware padding: truncation may add colored "…" to any column
                let stripped_len = utils::remove_ansi_colors(&display_value).chars().count();
                let padding = col_width.saturating_sub(stripped_len);
                row_data.push(format!("{}{}", display_value, " ".repeat(padding)));
            }
            output.push_str(&row_data.join(" | "));
            output.push('\n');
        }
        output.push('\n');

        output
    }

    pub fn get_json_output(&self) -> Option<serde_json::Value> {
        if !self.visible_in_json {
            return None;
        }

        // Build columns as a dict keyed by aplCode
        let mut columns_map = serde_json::Map::new();
        for col in &self.columns {
            let col_json = serde_json::json!({
                "aplCode": col.apl_code,
                "name": col.name,
                "width": col.width,
                "formatter": if col.formatter.is_some() { serde_json::json!({}) } else { serde_json::Value::Null },
                "renderer": if col.renderer.is_some() { serde_json::json!({}) } else { serde_json::Value::Null },
                "truncateIfLonger": col.truncate_if_longer,
                "formatterWillChangeValueLength": col.formatter_will_change_value_length,
                "nonBreakingSpaces": col.non_breaking_spaces,
                "escapeOutputHtml": col.escape_output_html,
                "getDataValueCallback": if col.get_data_value_callback.is_some() { serde_json::json!({}) } else { serde_json::Value::Null },
                "forcedDataType": col.forced_data_type,
            });
            columns_map.insert(col.apl_code.clone(), col_json);
        }

        Some(serde_json::json!({
            "aplCode": self.apl_code,
            "title": self.title,
            "columns": columns_map,
            "rows": self.data,
            "position": if self.position_before_url_table { POSITION_BEFORE_URL_TABLE } else { POSITION_AFTER_URL_TABLE },
        }))
    }

    pub fn is_position_before_url_table(&self) -> bool {
        self.position_before_url_table
    }

    pub fn get_data(&self) -> &[HashMap<String, String>] {
        &self.data
    }

    pub fn get_total_rows(&self) -> usize {
        self.data.len()
    }

    pub fn set_host_to_strip_from_urls(&mut self, host: Option<String>, scheme: Option<String>) {
        self.host_to_strip_from_urls = host;
        self.scheme_of_host_to_strip_from_urls = scheme;
    }

    pub fn set_initial_url(&mut self, url: Option<String>) {
        self.initial_url = url;
    }

    pub fn set_visibility_in_html(&mut self, visible: bool) {
        self.visible_in_html = visible;
    }

    pub fn set_visibility_in_console(&mut self, visible: bool, rows_limit: Option<usize>) {
        self.visible_in_console = visible;
        self.visible_in_console_rows_limit = rows_limit;
    }

    pub fn set_visibility_in_json(&mut self, visible: bool) {
        self.visible_in_json = visible;
    }

    pub fn is_visible_in_html(&self) -> bool {
        self.visible_in_html
    }

    pub fn is_visible_in_console(&self) -> bool {
        self.visible_in_console
    }

    pub fn is_visible_in_json(&self) -> bool {
        self.visible_in_json
    }

    pub fn disable_fulltext(&mut self) {
        self.fulltext_enabled = false;
    }

    pub fn set_show_only_columns_with_values(&mut self, show_only: bool) {
        self.show_only_columns_with_values = show_only;
    }

    pub fn get_columns(&self) -> &[SuperTableColumn] {
        &self.columns
    }

    pub fn set_hard_rows_limit(limit: usize) {
        if let Ok(mut v) = HARD_ROWS_LIMIT.write() {
            *v = limit;
        }
    }

    pub fn set_ignore_hard_rows_limit(&mut self, ignore: bool) {
        self.ignore_hard_rows_limit = ignore;
    }

    fn sort_data(&mut self, column_key: &str, direction: &str) {
        let dir_upper = direction.to_uppercase();
        let key = column_key.to_string();
        self.data.sort_by(|a, b| {
            let a_val = a.get(&key).cloned().unwrap_or_default();
            let b_val = b.get(&key).cloned().unwrap_or_default();

            // Try numeric comparison first
            let cmp = match (a_val.parse::<f64>(), b_val.parse::<f64>()) {
                (Ok(a_num), Ok(b_num)) => a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal),
                _ => a_val.cmp(&b_val),
            };

            if dir_upper == "ASC" { cmp } else { cmp.reverse() }
        });
    }

    fn is_fulltext_enabled(&self) -> bool {
        self.fulltext_enabled && self.data.len() >= self.min_rows_for_fulltext
    }

    fn remove_columns_with_empty_data(&mut self) {
        if !self.show_only_columns_with_values {
            return;
        }

        let columns_to_remove: Vec<String> = self
            .columns
            .iter()
            .filter(|col| {
                !self.data.iter().any(|row| {
                    let value = row.get(&col.apl_code).cloned().unwrap_or_default();
                    let trimmed = value.trim().trim_matches(|c: char| c == '0' || c == '.' || c == ',');
                    !trimmed.is_empty()
                })
            })
            .map(|col| col.apl_code.clone())
            .collect();

        self.columns.retain(|col| !columns_to_remove.contains(&col.apl_code));

        for row in &mut self.data {
            for key in &columns_to_remove {
                row.remove(key);
            }
        }
    }

    fn apply_hard_rows_limit(&mut self) {
        let limit = HARD_ROWS_LIMIT.read().map(|v| *v).unwrap_or(200);
        if limit > 0 && !self.ignore_hard_rows_limit && self.data.len() > limit {
            self.data.truncate(limit);
            self.max_hard_rows_limit_reached = true;
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn generate_unique_id() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(42);

    use ::md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(nanos.to_string().as_bytes());
    let result = hasher.finalize();
    format!("t{}", &format!("{:x}", result)[..6])
}
