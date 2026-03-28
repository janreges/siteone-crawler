// SiteOne Crawler - MultiOutput (delegates to multiple outputs)
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::collections::HashMap;

use crate::components::summary::summary::Summary;
use crate::components::super_table::SuperTable;
use crate::extra_column::ExtraColumn;
use crate::output::output::{BasicStats, Output};
use crate::output::output_type::OutputType;
use crate::scoring::ci_gate::CiGateResult;
use crate::scoring::quality_score::QualityScores;

#[derive(Default)]
pub struct MultiOutput {
    outputs: Vec<Box<dyn Output>>,
}

impl MultiOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_output(&mut self, output: Box<dyn Output>) {
        self.outputs.push(output);
    }

    pub fn get_outputs(&self) -> &[Box<dyn Output>] {
        &self.outputs
    }

    pub fn get_outputs_mut(&mut self) -> &mut [Box<dyn Output>] {
        &mut self.outputs
    }

    pub fn get_output_by_type(&self, output_type: OutputType) -> Option<&dyn Output> {
        self.outputs
            .iter()
            .find(|o| o.get_type() == output_type)
            .map(|o| o.as_ref())
    }

    pub fn get_output_by_type_mut(&mut self, output_type: OutputType) -> Option<&mut Box<dyn Output>> {
        self.outputs.iter_mut().find(|o| o.get_type() == output_type)
    }
}

impl Output for MultiOutput {
    fn add_banner(&mut self) {
        for output in &mut self.outputs {
            output.add_banner();
        }
    }

    fn add_used_options(&mut self) {
        for output in &mut self.outputs {
            output.add_used_options();
        }
    }

    fn set_extra_columns_from_analysis(&mut self, extra_columns: Vec<ExtraColumn>) {
        for output in &mut self.outputs {
            output.set_extra_columns_from_analysis(extra_columns.clone());
        }
    }

    fn add_table_header(&mut self) {
        for output in &mut self.outputs {
            output.add_table_header();
        }
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
        for output in &mut self.outputs {
            output.add_table_row(
                response_headers,
                url,
                status,
                elapsed_time,
                size,
                content_type,
                extra_parsed_content,
                progress_status,
                cache_type_flags,
                cache_lifetime,
            );
        }
    }

    fn add_super_table(&mut self, table: &SuperTable) {
        for output in &mut self.outputs {
            output.add_super_table(table);
        }
    }

    fn add_total_stats(&mut self, stats: &BasicStats) {
        for output in &mut self.outputs {
            output.add_total_stats(stats);
        }
    }

    fn add_notice(&mut self, text: &str) {
        for output in &mut self.outputs {
            output.add_notice(text);
        }
    }

    fn add_error(&mut self, text: &str) {
        for output in &mut self.outputs {
            output.add_error(text);
        }
    }

    fn add_quality_scores(&mut self, scores: &QualityScores) {
        for output in &mut self.outputs {
            output.add_quality_scores(scores);
        }
    }

    fn add_ci_gate_result(&mut self, result: &CiGateResult) {
        for output in &mut self.outputs {
            output.add_ci_gate_result(result);
        }
    }

    fn add_summary(&mut self, summary: &mut Summary) {
        for output in &mut self.outputs {
            output.add_summary(summary);
        }
    }

    fn set_export_file_paths(
        &mut self,
        offline_paths: Option<&HashMap<String, String>>,
        markdown_paths: Option<&HashMap<String, String>>,
    ) {
        for output in &mut self.outputs {
            output.set_export_file_paths(offline_paths, markdown_paths);
        }
    }

    fn get_type(&self) -> OutputType {
        OutputType::Multi
    }

    fn end(&mut self) {
        for output in &mut self.outputs {
            output.end();
        }
    }

    fn get_output_text(&self) -> Option<String> {
        for output in &self.outputs {
            if let Some(text) = output.get_output_text() {
                return Some(text);
            }
        }
        None
    }

    fn get_json_content(&self) -> Option<String> {
        for output in &self.outputs {
            if let Some(json) = output.get_json_content() {
                return Some(json);
            }
        }
        None
    }
}
