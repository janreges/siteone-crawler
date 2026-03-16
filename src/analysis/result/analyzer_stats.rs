// SiteOne Crawler - AnalyzerStats
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct AnalyzerStats {
    /// analysis_name -> severity -> set of subject hashes (or just counted entries)
    severity_counts_per_analysis: HashMap<String, SeverityCounts>,
}

#[derive(Debug, Clone, Default)]
struct SeverityCounts {
    ok: HashMap<String, bool>,
    notice: HashMap<String, bool>,
    warning: HashMap<String, bool>,
    critical: HashMap<String, bool>,
}

impl AnalyzerStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_ok(&mut self, analysis_name: &str, subject: Option<&str>) {
        self.add_result(analysis_name, "ok", subject);
    }

    pub fn add_warning(&mut self, analysis_name: &str, subject: Option<&str>) {
        self.add_result(analysis_name, "warning", subject);
    }

    pub fn add_critical(&mut self, analysis_name: &str, subject: Option<&str>) {
        self.add_result(analysis_name, "critical", subject);
    }

    pub fn add_notice(&mut self, analysis_name: &str, subject: Option<&str>) {
        self.add_result(analysis_name, "notice", subject);
    }

    pub fn to_table_data(&self) -> Vec<HashMap<String, String>> {
        let mut result = Vec::new();
        for (analysis_name, counts) in &self.severity_counts_per_analysis {
            let mut row = HashMap::new();
            row.insert("analysisName".to_string(), analysis_name.clone());
            row.insert("ok".to_string(), counts.ok.len().to_string());
            row.insert("notice".to_string(), counts.notice.len().to_string());
            row.insert("warning".to_string(), counts.warning.len().to_string());
            row.insert("critical".to_string(), counts.critical.len().to_string());
            result.push(row);
        }
        result
    }

    fn add_result(&mut self, analysis_name: &str, severity: &str, subject: Option<&str>) {
        let counts = self
            .severity_counts_per_analysis
            .entry(analysis_name.to_string())
            .or_default();

        let subject_hash = subject.map(|s| {
            use md5::{Digest, Md5};
            let mut hasher = Md5::new();
            hasher.update(s.trim().as_bytes());
            let result = hasher.finalize();
            format!("{:x}", result)[..10].to_string()
        });

        let map = match severity {
            "ok" => &mut counts.ok,
            "notice" => &mut counts.notice,
            "warning" => &mut counts.warning,
            "critical" => &mut counts.critical,
            _ => return,
        };

        if let Some(hash) = subject_hash {
            map.insert(hash, true);
        } else {
            // Use a unique key based on current count
            let key = format!("_auto_{}", map.len());
            map.insert(key, true);
        }
    }
}
