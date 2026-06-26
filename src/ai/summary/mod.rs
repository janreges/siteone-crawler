// SiteOne Crawler - AI report-summary action ("summary")
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Post-analysis phase: evaluate 5 areas in parallel (each grounded in compact aggregated
// data), then synthesize one executive summary + prioritized recommendations rendered into
// the report's Summary tab. Fixed cost of 6 LLM calls regardless of site size.

pub mod extract;
pub mod prompts;
pub mod render;

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::client::AiClient;
use super::config::build_config;
use super::normalize::normalize_json_response;
use super::prompt::sanitize_for_prompt;
use super::provider::{ChatMessage, ChatRequest};
use crate::options::core_options::CoreOptions;
use crate::output::output::{BasicStats, Output};
use crate::result::status::Status;
use crate::scoring::scorer;
use crate::utils;

// Per-type token-accounting labels for the executive summary's two prompt stages.
const CAT_SUMMARY_AREAS: &str = "Executive summary (area evals)";
const CAT_SUMMARY_SYNTHESIS: &str = "Executive summary (synthesis)";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AreaFinding {
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub recommendation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AreaAssessment {
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub grade: String,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub summary_narrative: String,
    #[serde(default)]
    pub findings: Vec<AreaFinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Recommendation {
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportSummary {
    #[serde(default)]
    pub overall_assessment: String,
    #[serde(default)]
    pub overall_grade: String,
    #[serde(default)]
    pub recommendations: Vec<Recommendation>,
}

/// Entry point for the `summary` AI action. Fail-soft. (`_output` reserved for future
/// inline console rendering; the summary currently surfaces via the HTML box + a summary item.)
pub async fn run(options: &CoreOptions, status: &Arc<Mutex<Status>>, _output: &Arc<Mutex<Box<dyn Output>>>) {
    // --- Phase A: compute scores + build compact area inputs (short lock) ---
    let area_inputs = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let summary = st.get_summary();
        let bs = st.get_basic_stats();
        let output_stats = BasicStats {
            total_urls: bs.total_urls,
            total_size: bs.total_size,
            total_size_formatted: bs.total_size_formatted.clone(),
            total_execution_time: bs.total_execution_time,
            total_requests_times: bs.total_requests_times,
            total_requests_times_avg: bs.total_requests_times_avg,
            total_requests_times_min: bs.total_requests_times_min,
            total_requests_times_max: bs.total_requests_times_max,
            total_requests_times_p90: bs.total_requests_times_p90,
            count_by_status: bs.count_by_status.clone(),
            count_by_content_type: bs.count_by_content_type.clone(),
        };
        let scores = scorer::calculate_scores(&summary, &output_stats);
        extract::build_area_inputs(&st, &scores)
    };

    let config = match build_config(options) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}",
                utils::get_color_text(&format!("AI summary skipped: {}", e), "red", true)
            );
            return;
        }
    };
    let max_tokens = config.max_tokens;
    let temperature = config.temperature;
    super::usage::note_model(&config.model);
    let client = Arc::new(AiClient::new(config));

    let total_input_kb: usize = area_inputs
        .iter()
        .map(|a| serde_json::to_string(&a.json).map(|s| s.len()).unwrap_or(0))
        .sum::<usize>()
        / 1024;
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI summary: evaluating {} areas (~{} KB input) + synthesis...",
                area_inputs.len(),
                total_input_kb
            ),
            "cyan",
            true
        )
    );

    // --- Phase B (1): area evaluations in parallel ---
    let mut handles = Vec::new();
    for input in area_inputs {
        let client = client.clone();
        let area = input.area;
        let system = prompts::area_system_prompt(area);
        let data = format!(
            "<area_data>\n{}\n</area_data>",
            sanitize_for_prompt(&serde_json::to_string_pretty(&input.json).unwrap_or_default())
        );
        handles.push(tokio::spawn(async move {
            let req = ChatRequest {
                system: Some(system),
                messages: vec![ChatMessage::user(data)],
                max_tokens,
                temperature,
                json_mode: true,
            };
            let res = match client.complete(&req, CAT_SUMMARY_AREAS).await {
                Ok(c) => serde_json::from_str::<AreaAssessment>(&normalize_json_response(&c.text)).ok(),
                Err(e) => {
                    eprintln!("  AI summary: area '{}' failed: {}", area, e);
                    None
                }
            };
            res.map(|mut a| {
                if a.area.is_empty() {
                    a.area = area.to_string();
                }
                a
            })
        }));
    }

    let mut assessments: Vec<AreaAssessment> = Vec::new();
    for h in handles {
        if let Ok(Some(a)) = h.await {
            assessments.push(a);
        }
    }

    if assessments.is_empty() {
        eprintln!(
            "{}",
            utils::get_color_text("AI summary: no area assessments produced; skipping.", "yellow", true)
        );
        return;
    }

    // --- Phase B (2): final synthesis (barrier — needs all areas) ---
    let synth_extra: Option<Value> = options
        .ai_synthesis_extra_body
        .as_ref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());

    // Inject each assessment's `area` into every one of its findings so the synthesis can copy
    // the source area verbatim into each recommendation (prevents cross-area mis-tagging).
    let tagged_assessments: Vec<Value> = assessments
        .iter()
        .map(|a| {
            let findings: Vec<Value> = a
                .findings
                .iter()
                .map(|f| {
                    json!({
                        "area": a.area,
                        "severity": f.severity,
                        "title": f.title,
                        "detail": f.detail,
                        "evidence": f.evidence,
                        "recommendation": f.recommendation,
                    })
                })
                .collect();
            json!({
                "area": a.area,
                "grade": a.grade,
                "score": a.score,
                "summary_narrative": a.summary_narrative,
                "findings": findings,
            })
        })
        .collect();
    let assessments_json = serde_json::to_string_pretty(&tagged_assessments).unwrap_or_default();
    // The synthesis is a single call; give it generous output headroom so that enabling
    // thinking/reasoning for it (via --ai-synthesis-extra-body) does not truncate the JSON.
    // max_tokens is only a ceiling — billing is on actual tokens used.
    let synth_max_tokens = max_tokens.max(32_000);
    let synth_req = ChatRequest {
        system: Some(prompts::SYNTHESIS_SYSTEM_PROMPT.to_string()),
        messages: vec![ChatMessage::user(format!(
            "<area_assessments>\n{}\n</area_assessments>",
            sanitize_for_prompt(&assessments_json)
        ))],
        max_tokens: synth_max_tokens,
        temperature,
        json_mode: true,
    };
    let synth_result = if options.ai_synthesis_extra_body.is_some() {
        client
            .complete_with(&synth_req, synth_extra.as_ref(), CAT_SUMMARY_SYNTHESIS)
            .await
    } else {
        client.complete(&synth_req, CAT_SUMMARY_SYNTHESIS).await
    };

    let report = match synth_result {
        Ok(c) => match serde_json::from_str::<ReportSummary>(&normalize_json_response(&c.text)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{}",
                    utils::get_color_text(&format!("AI summary: synthesis JSON invalid: {}", e), "yellow", true)
                );
                return;
            }
        },
        Err(e) => {
            eprintln!(
                "{}",
                utils::get_color_text(&format!("AI summary: synthesis call failed: {}", e), "yellow", true)
            );
            return;
        }
    };

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI summary done: {} recommendation(s) across {} area(s).",
                report.recommendations.len(),
                assessments.len()
            ),
            "green",
            true
        )
    );

    // --- Phase C: render + store (short lock) ---
    let html = render::render_html(&report, &assessments);
    if let Ok(st) = status.lock() {
        st.set_ai_report_summary_html(html);
        st.add_info_to_summary(
            "ai-report-summary",
            &format!(
                "AI executive summary generated with {} recommendation(s).",
                report.recommendations.len()
            ),
        );
    }
}
