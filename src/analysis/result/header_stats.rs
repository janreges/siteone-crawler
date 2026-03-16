// SiteOne Crawler - HeaderStats
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::utils;

const MAX_UNIQUE_VALUES: usize = 20;

#[derive(Debug, Clone)]
pub struct HeaderStats {
    pub header: String,
    pub occurrences: usize,
    pub unique_values: HashMap<String, usize>,
    pub unique_values_limit_reached: bool,
    pub min_date_value: Option<String>,
    pub max_date_value: Option<String>,
    pub min_int_value: Option<i64>,
    pub max_int_value: Option<i64>,
}

impl HeaderStats {
    pub fn new(header: String) -> Self {
        Self {
            header,
            occurrences: 0,
            unique_values: HashMap::new(),
            unique_values_limit_reached: false,
            min_date_value: None,
            max_date_value: None,
            min_int_value: None,
            max_int_value: None,
        }
    }

    pub fn add_value(&mut self, value: &str) {
        self.occurrences += 1;

        if self.ignore_header_values(&self.header.clone()) {
        } else if self.is_value_for_min_max_date(&self.header.clone()) {
            self.add_value_for_min_max_date(value);
        } else if self.is_value_for_min_max_int(&self.header.clone()) {
            self.add_value_for_min_max_int(value);
        } else {
            if self.unique_values.len() >= MAX_UNIQUE_VALUES {
                self.unique_values_limit_reached = true;
                return;
            }
            *self.unique_values.entry(value.to_string()).or_insert(0) += 1;
        }
    }

    pub fn get_sorted_unique_values(&self) -> Vec<(&String, &usize)> {
        let mut sorted: Vec<_> = self.unique_values.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted
    }

    pub fn get_formatted_header_name(&self) -> String {
        let words: Vec<String> = self
            .header
            .split('-')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect();
        words.join("-").replace("Xss", "XSS")
    }

    pub fn is_value_for_min_max_int(&self, header: &str) -> bool {
        header == "content-length" || header == "age"
    }

    pub fn is_value_for_min_max_date(&self, header: &str) -> bool {
        header == "date" || header == "expires" || header == "last-modified"
    }

    pub fn ignore_header_values(&self, header: &str) -> bool {
        matches!(header, "etag" | "cf-ray" | "set-cookie" | "content-disposition")
    }

    pub fn get_min_value(&self) -> Option<String> {
        self.min_int_value
            .map(|v| v.to_string())
            .or_else(|| self.min_date_value.clone())
    }

    pub fn get_max_value(&self) -> Option<String> {
        self.max_int_value
            .map(|v| v.to_string())
            .or_else(|| self.max_date_value.clone())
    }

    pub fn get_values_preview(&self, max_length: usize) -> String {
        if self.unique_values.len() == 1
            && let Some(first_value) = self.unique_values.keys().next()
        {
            if first_value.chars().count() > max_length {
                return utils::truncate_in_two_thirds(first_value, max_length, "\u{2026}", None);
            }
            return first_value.clone();
        }

        let values_length: usize = self.unique_values.keys().map(|k| k.len()).sum();

        if values_length < max_length.saturating_sub(10) {
            let mut sorted: Vec<_> = self.unique_values.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            let mut result = String::new();
            for (value, count) in sorted {
                result.push_str(&format!("{} ({}) / ", value, count));
            }

            let trimmed = result.trim().trim_end_matches(" /").to_string();
            if trimmed.is_empty() {
                return "[ignored generic values]".to_string();
            }

            return utils::truncate_in_two_thirds(&trimmed, max_length, "\u{2026}", None);
        }

        "[see values below]".to_string()
    }

    fn add_value_for_min_max_int(&mut self, value: &str) {
        if let Ok(int_val) = value.parse::<i64>() {
            match self.min_int_value {
                None => self.min_int_value = Some(int_val),
                Some(min) if int_val < min => self.min_int_value = Some(int_val),
                _ => {}
            }
            match self.max_int_value {
                None => self.max_int_value = Some(int_val),
                Some(max) if int_val > max => self.max_int_value = Some(int_val),
                _ => {}
            }
        }
    }

    fn add_value_for_min_max_date(&mut self, value: &str) {
        // Try to parse HTTP date format into a simple YYYY-MM-DD string
        if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(value) {
            let date = dt.format("%Y-%m-%d").to_string();
            match &self.min_date_value {
                None => self.min_date_value = Some(date.clone()),
                Some(min) if &date < min => self.min_date_value = Some(date.clone()),
                _ => {}
            }
            match &self.max_date_value {
                None => self.max_date_value = Some(date),
                Some(max) if &date > max => self.max_date_value = Some(date),
                _ => {}
            }
        }
    }
}
