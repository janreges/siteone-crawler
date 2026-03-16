// SiteOne Crawler - SecurityCheckedHeader
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

pub const SEVERITY_OK: i32 = 1;
pub const SEVERITY_NOTICE: i32 = 2;
pub const SEVERITY_WARNING: i32 = 3;
pub const SEVERITY_CRITICAL: i32 = 4;

#[derive(Debug, Clone)]
pub struct SecurityCheckedHeader {
    pub header: String,
    pub highest_severity: Option<i32>,
    /// severity -> count
    pub count_per_severity: HashMap<i32, usize>,
    /// All unique values of this header
    pub values: Vec<String>,
    pub recommendations: Vec<String>,
}

impl SecurityCheckedHeader {
    pub fn new(header: String) -> Self {
        Self {
            header,
            highest_severity: None,
            count_per_severity: HashMap::new(),
            values: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn set_finding(&mut self, value: Option<&str>, severity: i32, recommendation: Option<&str>) {
        if let Some(val) = value
            && !self.values.contains(&val.to_string())
        {
            self.values.push(val.to_string());
        }
        if let Some(rec) = recommendation
            && !self.recommendations.contains(&rec.to_string())
        {
            self.recommendations.push(rec.to_string());
        }
        if self.highest_severity.is_none() || severity > self.highest_severity.unwrap_or(0) {
            self.highest_severity = Some(severity);
        }
        *self.count_per_severity.entry(severity).or_insert(0) += 1;
    }

    pub fn get_formatted_header(&self) -> String {
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

    pub fn get_severity_name(&self) -> &'static str {
        match self.highest_severity {
            Some(SEVERITY_OK) => "ok",
            Some(SEVERITY_NOTICE) => "notice",
            Some(SEVERITY_WARNING) => "warning",
            Some(SEVERITY_CRITICAL) => "critical",
            _ => "unknown",
        }
    }
}
