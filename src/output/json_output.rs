// SiteOne Crawler - JsonOutput (JSON output)
// (c) Jan Reges <jan.reges@siteone.cz>
//


use std::collections::HashMap;
use std::io::Write;

use serde_json::{Value, json};

use crate::components::summary::summary::Summary;
use crate::components::super_table::SuperTable;
use crate::extra_column::ExtraColumn;
use crate::output::output::{BasicStats, CrawlerInfo, Output};
use crate::output::output_type::OutputType;
use crate::scoring::ci_gate::CiGateResult;
use crate::scoring::quality_score::QualityScores;
use crate::utils;

pub struct JsonOutput {
    crawler_info: CrawlerInfo,
    print_to_output: bool,

    json: serde_json::Map<String, Value>,

    /// Extra columns from options (user-specified)
    extra_columns: Vec<ExtraColumn>,

    /// Serialized options for JSON output
    options_json: Option<Value>,

    /// For progress display on stderr
    hide_progress_bar: bool,
    max_stderr_length: usize,
}

impl JsonOutput {
    pub fn new(
        crawler_info: CrawlerInfo,
        extra_columns: Vec<ExtraColumn>,
        hide_progress_bar: bool,
        print_to_output: bool,
        options_json: Option<Value>,
    ) -> Self {
        Self {
            crawler_info,
            print_to_output,
            json: serde_json::Map::new(),
            extra_columns,
            options_json,
            hide_progress_bar,
            max_stderr_length: 0,
        }
    }

    pub fn get_json(&self) -> String {
        let value = Value::Object(self.json.clone());
        serde_json::to_string_pretty(&value)
            .unwrap_or_else(|e| format!("{{\"error\": \"unable to serialize JSON: {}\"}}", e))
    }
}

impl Output for JsonOutput {
    fn add_banner(&mut self) {
        self.json.insert(
            "crawler".to_string(),
            serde_json::to_value(&self.crawler_info).unwrap_or(Value::Null),
        );
    }

    fn add_used_options(&mut self) {
        if let Some(ref options) = self.options_json {
            self.json.insert("options".to_string(), options.clone());
        }
    }

    fn set_extra_columns_from_analysis(&mut self, extra_columns: Vec<ExtraColumn>) {
        let columns_json: Vec<Value> = extra_columns
            .iter()
            .map(|col| serde_json::to_value(col).unwrap_or(Value::Null))
            .collect();
        self.json
            .insert("extraColumnsFromAnalysis".to_string(), Value::Array(columns_json));
    }

    fn add_table_header(&mut self) {
        self.json.insert("results".to_string(), Value::Array(Vec::new()));
    }

    fn add_table_row(
        &mut self,
        response_headers: &HashMap<String, String>,
        url: &str,
        status: i32,
        elapsed_time: f64,
        size: i64,
        content_type: i32,
        extra_parsed_content: &HashMap<String, String>,
        progress_status: &str,
        cache_type_flags: i32,
        cache_lifetime: Option<i32>,
    ) {
        let status_str = utils::get_http_client_code_with_error_description(status, false);

        // extras: empty array [] when no extra columns, object {} when populated
        let extras_value = if self.extra_columns.is_empty() {
            Value::Array(Vec::new())
        } else {
            let mut extras = serde_json::Map::new();
            for extra_column in &self.extra_columns {
                let header_name = &extra_column.name;
                let value = if let Some(v) = extra_parsed_content.get(header_name) {
                    v.trim().to_string()
                } else if let Some(v) = response_headers.get(&header_name.to_lowercase()) {
                    v.trim().to_string()
                } else {
                    String::new()
                };
                extras.insert(header_name.clone(), Value::String(value));
            }
            Value::Object(extras)
        };

        let row = json!({
            "url": url,
            "status": status_str,
            "elapsedTime": (elapsed_time * 1000.0).round() / 1000.0,
            "size": size,
            "type": content_type,
            "cacheTypeFlags": cache_type_flags,
            "cacheLifetime": cache_lifetime,
            "extras": extras_value,
        });

        if let Some(Value::Array(results)) = self.json.get_mut("results") {
            results.push(row);
        }

        // Print progress to stderr in JSON mode
        if !self.hide_progress_bar && self.print_to_output {
            let parts: Vec<&str> = progress_status.splitn(2, '/').collect();
            let done: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let total: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

            let console_width = utils::get_console_width();
            let text_width_without_url: usize = 65;

            let truncated_url = utils::truncate_in_two_thirds(
                url,
                console_width.saturating_sub(text_width_without_url),
                "\u{2026}",
                None,
            );

            let progress_to_stderr = format!(
                "\rProgress: {:<7} | {} {} | {}",
                progress_status,
                utils::get_progress_bar(done, total, 25),
                utils::get_formatted_duration(elapsed_time),
                truncated_url,
            );

            self.max_stderr_length = self.max_stderr_length.max(progress_to_stderr.len());
            let padded = format!("{:<width$}", progress_to_stderr, width = self.max_stderr_length);

            eprint!("{}", padded);
            let _ = std::io::stderr().flush();
        }
    }

    fn add_super_table(&mut self, table: &SuperTable) {
        if !self.json.contains_key("tables") {
            self.json
                .insert("tables".to_string(), Value::Object(serde_json::Map::new()));
        }

        if let Some(table_json) = table.get_json_output()
            && let Some(Value::Object(tables)) = self.json.get_mut("tables")
        {
            tables.insert(table.apl_code.clone(), table_json);
        }
    }

    fn add_total_stats(&mut self, stats: &BasicStats) {
        if self.print_to_output {
            eprintln!("\n");
        }

        // Build countByStatus as string-keyed object (JSON requires string keys)
        let count_by_status: serde_json::Map<String, Value> = stats
            .count_by_status
            .iter()
            .map(|(k, v)| (k.to_string(), json!(*v)))
            .collect();

        let stats_json = json!({
            "totalUrls": stats.total_urls,
            "totalSize": stats.total_size,
            "totalSizeFormatted": stats.total_size_formatted,
            "totalExecutionTime": stats.total_execution_time,
            "totalRequestsTimes": stats.total_requests_times,
            "totalRequestsTimesAvg": stats.total_requests_times_avg,
            "totalRequestsTimesMin": stats.total_requests_times_min,
            "totalRequestsTimesMax": stats.total_requests_times_max,
            "countByStatus": count_by_status,
        });
        self.json.insert("stats".to_string(), stats_json);
    }

    fn add_notice(&mut self, text: &str) {
        if !self.json.contains_key("notice") {
            self.json.insert("notice".to_string(), Value::Array(Vec::new()));
        }

        let now = chrono::Local::now();
        let timestamped = format!("{} | {}", now.format("%Y-%m-%d %H:%M:%S"), text);

        if let Some(Value::Array(notices)) = self.json.get_mut("notice") {
            notices.push(Value::String(timestamped));
        }
    }

    fn add_error(&mut self, text: &str) {
        if !self.json.contains_key("error") {
            self.json.insert("error".to_string(), Value::Array(Vec::new()));
        }

        let now = chrono::Local::now();
        let timestamped = format!("{} | {}", now.format("%Y-%m-%d %H:%M:%S"), text);

        if let Some(Value::Array(errors)) = self.json.get_mut("error") {
            errors.push(Value::String(timestamped));
        }
    }

    fn add_quality_scores(&mut self, scores: &QualityScores) {
        if let Ok(value) = serde_json::to_value(scores) {
            self.json.insert("qualityScores".to_string(), value);
        }
    }

    fn add_ci_gate_result(&mut self, result: &CiGateResult) {
        if let Ok(value) = serde_json::to_value(result) {
            self.json.insert("ciGate".to_string(), value);
        }
    }

    fn add_summary(&mut self, summary: &mut Summary) {
        if let Ok(summary_value) = serde_json::to_value(summary) {
            self.json.insert("summary".to_string(), summary_value);
        }
    }

    fn get_type(&self) -> OutputType {
        OutputType::Json
    }

    fn end(&mut self) {
        if !self.print_to_output {
            return;
        }

        let json = self.get_json();
        println!("{}", json);
    }

    fn get_json_content(&self) -> Option<String> {
        Some(self.get_json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::ci_gate::{CiCheck, CiGateResult};
    use crate::scoring::quality_score::{CategoryScore, QualityScores};

    fn make_json_output() -> JsonOutput {
        JsonOutput::new(CrawlerInfo::default(), vec![], true, false, None)
    }

    fn make_pass_result() -> CiGateResult {
        CiGateResult {
            passed: true,
            exit_code: 0,
            checks: vec![],
        }
    }

    fn make_fail_result() -> CiGateResult {
        CiGateResult {
            passed: false,
            exit_code: 10,
            checks: vec![
                CiCheck {
                    metric: "Overall score".into(),
                    operator: ">=".into(),
                    threshold: 5.0,
                    actual: 3.0,
                    passed: false,
                },
                CiCheck {
                    metric: "404 errors".into(),
                    operator: "<=".into(),
                    threshold: 0.0,
                    actual: 2.0,
                    passed: false,
                },
                CiCheck {
                    metric: "5xx errors".into(),
                    operator: "<=".into(),
                    threshold: 0.0,
                    actual: 0.0,
                    passed: true,
                },
            ],
        }
    }

    fn parse_json(output: &JsonOutput) -> serde_json::Value {
        serde_json::from_str(&output.get_json()).unwrap()
    }

    #[test]
    fn ci_gate_present_when_added() {
        let mut output = make_json_output();
        output.add_ci_gate_result(&make_pass_result());
        let json = parse_json(&output);
        assert!(json.get("ciGate").is_some());
    }

    #[test]
    fn ci_gate_absent_when_not_added() {
        let output = make_json_output();
        let json = parse_json(&output);
        assert!(json.get("ciGate").is_none());
    }

    #[test]
    fn ci_gate_passed_true() {
        let mut output = make_json_output();
        output.add_ci_gate_result(&make_pass_result());
        let json = parse_json(&output);
        let ci_gate = json.get("ciGate").unwrap();
        assert_eq!(ci_gate.get("passed").unwrap().as_bool().unwrap(), true);
        assert_eq!(ci_gate.get("exitCode").unwrap().as_i64().unwrap(), 0);
    }

    #[test]
    fn ci_gate_passed_false() {
        let mut output = make_json_output();
        output.add_ci_gate_result(&make_fail_result());
        let json = parse_json(&output);
        let ci_gate = json.get("ciGate").unwrap();
        assert_eq!(ci_gate.get("passed").unwrap().as_bool().unwrap(), false);
        assert_eq!(ci_gate.get("exitCode").unwrap().as_i64().unwrap(), 10);
    }

    #[test]
    fn ci_gate_checks_array() {
        let mut output = make_json_output();
        output.add_ci_gate_result(&make_fail_result());
        let json = parse_json(&output);
        let checks = json["ciGate"]["checks"].as_array().unwrap();
        assert_eq!(checks.len(), 3);
    }

    #[test]
    fn quality_scores_in_json() {
        let mut output = make_json_output();
        let scores = QualityScores {
            overall: CategoryScore {
                name: "Overall".into(),
                code: "overall".into(),
                score: 8.5,
                label: "Good".into(),
                weight: 1.0,
                deductions: vec![],
            },
            categories: vec![],
        };
        output.add_quality_scores(&scores);
        let json = parse_json(&output);
        assert!(json.get("qualityScores").is_some());
    }
}
