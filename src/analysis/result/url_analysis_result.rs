// SiteOne Crawler - UrlAnalysisResult
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::utils;

#[derive(Debug, Clone, Default)]
pub struct UrlAnalysisResult {
    ok: Vec<String>,
    notice: Vec<String>,
    warning: Vec<String>,
    critical: Vec<String>,

    ok_details: HashMap<String, Vec<String>>,
    notice_details: HashMap<String, Vec<String>>,
    warning_details: HashMap<String, Vec<String>>,
    critical_details: HashMap<String, Vec<String>>,

    /// Stats per analysis and severity: analysis_name -> severity -> count
    stats_per_analysis: HashMap<String, HashMap<String, usize>>,
}

impl UrlAnalysisResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_ok(&mut self, message: String, analysis_name: &str, detail: Option<Vec<String>>) {
        self.ok.push(message);
        if let Some(d) = detail {
            self.ok_details.entry(analysis_name.to_string()).or_default().extend(d);
        }
        *self
            .stats_per_analysis
            .entry(analysis_name.to_string())
            .or_default()
            .entry("ok".to_string())
            .or_insert(0) += 1;
    }

    pub fn add_notice(&mut self, message: String, analysis_name: &str, detail: Option<Vec<String>>) {
        self.notice.push(message);
        if let Some(d) = detail {
            self.notice_details
                .entry(analysis_name.to_string())
                .or_default()
                .extend(d);
        }
        *self
            .stats_per_analysis
            .entry(analysis_name.to_string())
            .or_default()
            .entry("notice".to_string())
            .or_insert(0) += 1;
    }

    pub fn add_warning(&mut self, message: String, analysis_name: &str, detail: Option<Vec<String>>) {
        self.warning.push(message);
        if let Some(d) = detail {
            self.warning_details
                .entry(analysis_name.to_string())
                .or_default()
                .extend(d);
        }
        *self
            .stats_per_analysis
            .entry(analysis_name.to_string())
            .or_default()
            .entry("warning".to_string())
            .or_insert(0) += 1;
    }

    pub fn add_critical(&mut self, message: String, analysis_name: &str, detail: Option<Vec<String>>) {
        self.critical.push(message);
        if let Some(d) = detail {
            self.critical_details
                .entry(analysis_name.to_string())
                .or_default()
                .extend(d);
        }
        *self
            .stats_per_analysis
            .entry(analysis_name.to_string())
            .or_default()
            .entry("critical".to_string())
            .or_insert(0) += 1;
    }

    pub fn get_stats_per_analysis(&self) -> &HashMap<String, HashMap<String, usize>> {
        &self.stats_per_analysis
    }

    pub fn get_ok(&self) -> &[String] {
        &self.ok
    }

    pub fn get_notice(&self) -> &[String] {
        &self.notice
    }

    pub fn get_warning(&self) -> &[String] {
        &self.warning
    }

    pub fn get_critical(&self) -> &[String] {
        &self.critical
    }

    pub fn get_ok_details(&self) -> &HashMap<String, Vec<String>> {
        &self.ok_details
    }

    pub fn get_notice_details(&self) -> &HashMap<String, Vec<String>> {
        &self.notice_details
    }

    pub fn get_warning_details(&self) -> &HashMap<String, Vec<String>> {
        &self.warning_details
    }

    pub fn get_critical_details(&self) -> &HashMap<String, Vec<String>> {
        &self.critical_details
    }

    pub fn get_all_count(&self) -> usize {
        self.ok.len() + self.notice.len() + self.warning.len() + self.critical.len()
    }

    pub fn get_details_of_severity_and_analysis_name(&self, severity: &str, analysis_name: &str) -> Vec<String> {
        match severity {
            "ok" => self.ok_details.get(analysis_name).cloned().unwrap_or_default(),
            "notice" => self.notice_details.get(analysis_name).cloned().unwrap_or_default(),
            "warning" => self.warning_details.get(analysis_name).cloned().unwrap_or_default(),
            "critical" => self.critical_details.get(analysis_name).cloned().unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn to_icon_string(&self) -> String {
        let mut result = String::new();

        let count_critical = self.critical.len();
        let count_warning = self.warning.len();
        let count_notice = self.notice.len();
        let count_ok = self.ok.len();

        if count_critical > 0 {
            result.push_str(&format!("{}\u{26d4} ", count_critical));
        }
        if count_warning > 0 {
            result.push_str(&format!("{}\u{26a0} ", count_warning));
        }
        if count_notice > 0 {
            result.push_str(&format!("{}\u{2139}\u{fe0f} ", count_notice));
        }
        if count_ok > 0 {
            result.push_str(&format!("{}\u{2705} ", count_ok));
        }

        result.trim().to_string()
    }

    pub fn to_colorized_string(&self, strip_whitespaces: bool) -> String {
        let mut result = String::new();

        let count_critical = self.critical.len();
        let count_warning = self.warning.len();
        let count_notice = self.notice.len();
        let count_ok = self.ok.len();

        if count_critical > 0 {
            result.push_str(&utils::get_color_text(&count_critical.to_string(), "red", true));
            result.push_str(" / ");
        }
        if count_warning > 0 {
            result.push_str(&utils::get_color_text(&count_warning.to_string(), "magenta", false));
            result.push_str(" / ");
        }
        if count_notice > 0 {
            result.push_str(&utils::get_color_text(&count_notice.to_string(), "blue", false));
            result.push_str(" / ");
        }
        if count_ok > 0 {
            result.push_str(&utils::get_color_text(&count_ok.to_string(), "green", false));
            result.push_str(" / ");
        }

        let trimmed = result.trim_end_matches(" / ").to_string();
        if strip_whitespaces {
            trimmed.replace(' ', "")
        } else {
            trimmed
        }
    }

    pub fn to_not_colorized_string(&self, strip_whitespaces: bool) -> String {
        let mut result = String::new();

        let count_critical = self.critical.len();
        let count_warning = self.warning.len();
        let count_notice = self.notice.len();
        let count_ok = self.ok.len();

        if count_critical > 0 {
            result.push_str(&format!("{} / ", count_critical));
        }
        if count_warning > 0 {
            result.push_str(&format!("{} / ", count_warning));
        }
        if count_notice > 0 {
            result.push_str(&format!("{} / ", count_notice));
        }
        if count_ok > 0 {
            result.push_str(&format!("{} / ", count_ok));
        }

        let trimmed = result.trim_end_matches(" / ").to_string();
        if strip_whitespaces {
            trimmed.replace(' ', "")
        } else {
            trimmed
        }
    }

    pub fn get_all_details_for_analysis(&self, analysis_name: &str) -> HashMap<String, Vec<String>> {
        let mut result = HashMap::new();
        result.insert(
            "ok".to_string(),
            self.ok_details.get(analysis_name).cloned().unwrap_or_default(),
        );
        result.insert(
            "notice".to_string(),
            self.notice_details.get(analysis_name).cloned().unwrap_or_default(),
        );
        result.insert(
            "warning".to_string(),
            self.warning_details.get(analysis_name).cloned().unwrap_or_default(),
        );
        result.insert(
            "critical".to_string(),
            self.critical_details.get(analysis_name).cloned().unwrap_or_default(),
        );
        result
    }
}

impl std::fmt::Display for UrlAnalysisResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_colorized_string(true))
    }
}
