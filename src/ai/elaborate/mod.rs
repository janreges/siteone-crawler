// SiteOne Crawler - AI brand elaborate
// (c) Jan Reges <jan.reges@siteone.cz>
//
// `--ai-elaborate`: turn a crawled site into one large, richly structured brand document
// (markdown + JSON + HTML). Pipeline (post-crawl, mirrors `src/ai/summary/`): id-constrained
// LLM page selection over the deterministic candidate universe (mass-entity pages clustered &
// sampled, not enumerated) → per-page typed extraction → deterministic entity merge → prose-only
// synthesis into a fixed per-template skeleton → optional correction pass. Structured lists are
// crawler-assembled from verbatim extracted entities, so contacts/people/facts are
// un-hallucinatable; the LLM only writes connective prose. Output language reuses the global
// `--ai-report-language` + `ReportLocale`.

pub mod brand;
pub mod cluster;
pub mod correct;
pub mod doc;
pub mod extract;
pub mod gapfill;
pub mod merge;
pub mod nav;
pub mod select;
pub mod synthesize;
pub mod templates;

use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::Semaphore;

use crate::ai::client::AiClient;
use crate::ai::config::build_config;
use crate::ai::page::PageContext;
use crate::ai::progress;
use crate::ai::provider::{ChatMessage, ChatRequest};
use crate::ai::report::locale::ReportLocale;
use crate::ai::selection::{Candidate, build_candidates};
use crate::options::core_options::CoreOptions;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

use self::doc::{BrandDoc, ElaborateMeta};
use self::merge::BrandModel;
use self::templates::{Template, prose_ids, skeleton};

const CAT_SELECT: &str = "Brand elaborate (select)";
const CAT_EXTRACT: &str = "Brand elaborate (extract)";
const CAT_SYNTH: &str = "Brand elaborate (synthesis)";
const CAT_CORRECT: &str = "Brand elaborate (correction)";
// Progress tasks of the LLM stages (the gap-fill's is in `gapfill`).
const TASK_SELECT: &str = "elaborate:select";
const TASK_EXTRACT: &str = "elaborate:extract";
const TASK_SYNTH: &str = "elaborate:synthesize";
const SYNTH_LABEL: &str = "Elaborate: synthesis";
/// Label of the `issue` event (kind `ai`) when no document is produced.
const ELABORATE_FAILED: &str = "Brand elaborate failed";
const TASK_CORRECT: &str = "elaborate:correct";
const EXTRACT_ATTEMPTS: u32 = 3;
const SELECT_ATTEMPTS: u32 = 2;
/// How many stored HTML bodies to re-parse for global-nav detection.
const NAV_SAMPLE: usize = 40;
/// How many individual candidate pages to number for the round-1 IA list.
const SHOWN_LIMIT: usize = 200;
/// Assumed model completion cap (tokens) for the single-shot vs sectioned decision.
const MODEL_OUTPUT_CAP_TOKENS: usize = 16_000;
/// Synthesis material budget (KB) — the owner's ≤200 KB constraint.
const MATERIAL_BUDGET_KB: usize = 180;

/// Entry point for `--ai-elaborate`. Fail-soft: never panics, never aborts the crawl.
pub async fn run(options: &CoreOptions, status: &Arc<Mutex<Status>>, output: &Arc<Mutex<Box<dyn Output>>>) {
    let _ = output;
    // An elaborate-only run owns the usage ledger; when combined with --ai-actions, run_ai already
    // reset it, so don't clobber those numbers.
    if options.ai_actions.is_empty() {
        crate::ai::usage::reset();
    }
    crate::ai::usage::note_model(&options.ai_model.clone().unwrap_or_default());
    crate::ai::client::reset_rate_limiter().await;

    let host = options.get_initial_host(false);
    let locale = ReportLocale::new(&options.ai_report_language);
    let report_language = locale.code().to_string();
    let max_tokens = options.ai_max_tokens.clamp(1, 1_000_000) as u32;
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;
    let budget = options.ai_max_pages.max(1) as usize;

    // --- Phase A: candidate universe + clustering + nav sample + brand (short Status lock) ---
    let (mut candidate_set, mut clusters, nav_links, brand_name) = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let cs = build_candidates(&st, &options.ai_include, &options.ai_exclude);
        let (clusters, _unclustered) =
            cluster::cluster_urls(&cs.candidates, options.ai_elaborate_cluster_min.max(2) as usize);
        // Sample stored HTML from the highest-ranked pages (incl. the homepage) for global-nav
        // detection AND brand detection (title / footer / og:site_name / domain, combined).
        let sample: Vec<(String, String)> = cs
            .candidates
            .iter()
            .take(NAV_SAMPLE)
            .filter_map(|c| st.get_url_body_text(&c.uq_id).map(|html| (c.url.clone(), html)))
            .collect();
        let nav_links = nav::detect_global_nav(&sample);
        let brand_name = brand::detect(&sample, &host);
        (cs, clusters, nav_links, brand_name)
    };

    if candidate_set.candidates.is_empty() {
        let msg = "AI elaborate: no eligible pages were crawled.";
        crate::events::emit_ai_issue(ELABORATE_FAILED, msg);
        if let Ok(st) = status.lock() {
            st.add_notice_to_summary("ai-elaborate", msg);
        }
        return;
    }

    // --- Optional gap-fill: fetch important global-nav pages the crawl never visited so they can
    // enter selection (typical under --single-page or a page cap). Skipped in dry-run (no network).
    // Mask-respecting: a nav URL the user excluded is never fetched. ---
    let gap_cap = options.ai_elaborate_gap_fill.clamp(0, 200) as usize;
    if gap_cap > 0 && !options.ai_dry_run {
        let known: std::collections::HashSet<String> = {
            let st = match status.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.get_visited_urls().into_iter().map(|u| u.url).collect()
        };
        let missing: Vec<String> = nav_links
            .iter()
            .map(|n| n.url.clone())
            .filter(|u| !known.contains(u))
            .filter(|u| crate::ai::selection::url_passes_masks(u, &options.ai_include, &options.ai_exclude))
            .collect();
        if !missing.is_empty() {
            let report = gapfill::fetch_missing(&missing, options, status, gap_cap).await;
            if report.fetched > 0 {
                // Rebuild the candidate universe + clusters so the newly-stored pages can be selected.
                if let Ok(st) = status.lock() {
                    candidate_set = build_candidates(&st, &options.ai_include, &options.ai_exclude);
                    let (new_clusters, _) = cluster::cluster_urls(
                        &candidate_set.candidates,
                        options.ai_elaborate_cluster_min.max(2) as usize,
                    );
                    clusters = new_clusters;
                }
                eprintln!(
                    "{}",
                    utils::get_color_text(
                        &format!(
                            "  gap-fill: fetched {} nav page(s) the crawl had missed.",
                            report.fetched
                        ),
                        "cyan",
                        false
                    )
                );
            }
        }
    }

    // Build the numbered candidate list shown to the LLM: top-ranked pages, titled by nav anchor when
    // known, else by path.
    let nav_anchor = |url: &str| nav_links.iter().find(|n| n.url == url).map(|n| n.anchor.clone());
    let shown: Vec<(String, Candidate)> = candidate_set
        .candidates
        .iter()
        .take(SHOWN_LIMIT)
        .map(|c| {
            let title = nav_anchor(&c.url)
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| path_hint(&c.url));
            (title, c.clone())
        })
        .collect();

    // --- Build the AI client ---
    let config = match build_config(options) {
        Ok(c) => c,
        Err(e) => {
            report_error(status, &format!("AI elaborate skipped: {}", e));
            return;
        }
    };
    let provider = config.provider;
    let model_name = config.model.clone();
    let client = Arc::new(AiClient::new(config));

    // --- S3/S4: LLM selection rounds ---
    eprintln!(
        "\n{}",
        utils::get_color_text(
            &format!(
                "AI elaborate: {} candidate page(s), {} cluster(s) — selecting up to {} pages using {} / {}",
                candidate_set.candidates.len(),
                clusters.len(),
                budget,
                provider.as_str(),
                model_name
            ),
            "cyan",
            true
        )
    );

    // Deterministic top-N preview used for small sites AND dry-runs (both make zero LLM calls).
    let deterministic_plan = || select::SelectionPlan {
        selected: candidate_set.candidates.iter().take(budget).cloned().collect(),
        clusters: Vec::new(),
        site_type: String::new(),
        notices: vec!["deterministic selection (no LLM)".to_string()],
    };
    let small_site = candidate_set.candidates.len() <= 40;

    // --- Dry run: show the deterministic preview and stop WITHOUT calling the API ---
    if options.ai_dry_run {
        let plan = deterministic_plan();
        let template = templates::resolve(options.ai_elaborate_template.as_deref().unwrap_or(""), "");
        let extract_calls = plan.selected.len();
        let select_calls = if small_site { 0 } else { 2 };
        let calls = select_calls + extract_calls + 2; // + synthesis + correction
        eprintln!(
            "{}",
            utils::get_color_text(
                &format!(
                    "AI elaborate dry-run: template={}, {} candidate page(s) shown (LLM selection is skipped in dry-run), ~{} LLM call(s) at run time ({} select + {} extract + synthesis + correction). No API calls made.",
                    template.key(),
                    plan.selected.len(),
                    calls,
                    select_calls,
                    extract_calls
                ),
                "yellow",
                true
            )
        );
        for (i, c) in plan.selected.iter().take(30).enumerate() {
            eprintln!("  {:>3}. {}", i + 1, c.url);
        }
        if let Ok(st) = status.lock() {
            st.add_info_to_summary(
                "ai-elaborate-dry-run",
                &format!(
                    "AI elaborate dry-run: up to {} page(s) would be analyzed.",
                    plan.selected.len()
                ),
            );
        }
        return;
    }

    let plan = if small_site {
        // Skip the LLM selection entirely — take every candidate (already <= budget-ish).
        deterministic_plan()
    } else {
        run_selection(
            &client,
            &shown,
            &candidate_set.candidates,
            &clusters,
            max_tokens,
            temperature,
            options.ai_elaborate_cluster_reps.clamp(0, 10) as usize,
            budget,
        )
        .await
    };

    let template = templates::resolve(options.ai_elaborate_template.as_deref().unwrap_or(""), &plan.site_type);

    // --- S5: build PageContext for the selected set (short lock), then extract (no lock) ---
    let contexts: Vec<(String, PageContext)> = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        plan.selected
            .iter()
            .filter_map(|c| PageContext::build(&st, &c.uq_id, &c.url, options).map(|ctx| (c.url.clone(), ctx)))
            .collect()
    };
    if contexts.is_empty() {
        report_error(
            status,
            "AI elaborate: no page content available for the selected pages.",
        );
        return;
    }

    // --- S7: per-page extraction (fan-out; the client rate-limits internally) ---
    let per_essence_cap = (200 * 1024 / contexts.len().max(1)).clamp(800, 3072);
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI elaborate: extracting essence from {} page(s)...", contexts.len()),
            "cyan",
            false
        )
    );
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    progress::start(TASK_EXTRACT, "Elaborate: extract", contexts.len() as u64);
    for (url, ctx) in contexts.iter().cloned() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let lang = report_language.clone();
        let subject = crate::ai::runner::url_path_and_query(&url);
        handles.push(tokio::spawn(progress::unit(TASK_EXTRACT, subject, async move {
            let _permit = permit;
            let req = extract::build_request(&ctx, template, &lang, max_tokens, temperature);
            let result = client
                .complete_parsed_n(&req, CAT_EXTRACT, EXTRACT_ATTEMPTS, extract::parse)
                .await;
            (url, result)
        })));
    }

    let mut essences: Vec<(String, extract::PageEssence)> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    for h in handles {
        match h.await {
            Ok((url, Ok((mut e, _)))) => {
                extract::cap_essence(&mut e, per_essence_cap);
                essences.push((url, e));
            }
            Ok((url, Err(err))) => {
                eprintln!(
                    "{}",
                    utils::get_color_text(
                        &format!("  elaborate: {} → error: {}", path_hint(&url), err),
                        "yellow",
                        false
                    )
                );
                failed.push((url, err.to_string()));
            }
            Err(_) => {} // task join error — counted implicitly by the missing page
        }
    }
    progress::finish(TASK_EXTRACT);

    if essences.is_empty() {
        report_error(
            status,
            "AI elaborate: every page failed extraction; no document produced.",
        );
        return;
    }

    // --- S8: deterministic merge into the un-hallucinatable BrandModel ---
    let brand_model = merge::merge(&essences, &plan.clusters);
    // `brand_name` was detected from the multi-signal Phase-A sample (title/footer/og/domain).

    // --- S9: prose synthesis (single-shot or sectioned) ---
    let prose = synthesize_prose(
        &client,
        &brand_model,
        &essences,
        template,
        &report_language,
        max_tokens,
        MODEL_OUTPUT_CAP_TOKENS,
        options.ai_elaborate_max_output_kb.clamp(10, 200) as usize,
    )
    .await;

    // --- S10: optional correction pass (default ON) ---
    let title = format!(
        "{} — {}",
        brand_name,
        if locale.is_czech() {
            "profil značky"
        } else {
            "brand profile"
        }
    );
    let mut brand_doc = BrandDoc {
        template: template.key().to_string(),
        brand_name,
        title,
        meta: ElaborateMeta {
            host: host.clone(),
            url: crate::utils::redact_url_userinfo(&options.url),
            crawled_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            provider: provider.as_str().to_string(),
            model: model_name,
            report_language: report_language.clone(),
            site_type: plan.site_type.clone(),
            pages_used: essences.len(),
            pages_failed: failed,
        },
        model: brand_model,
        prose,
        correction: None,
    };

    if options.ai_elaborate_correct && !brand_doc.prose.is_empty() {
        let (new_prose, report) = run_correction(
            &client,
            &brand_doc.prose,
            &brand_doc.model,
            &report_language,
            max_tokens,
        )
        .await;
        brand_doc.prose = new_prose;
        brand_doc.correction = Some(report);
    }

    let n = brand_doc.meta.pages_used;
    let failed_n = brand_doc.meta.pages_failed.len();
    let color = if failed_n > 0 { "yellow" } else { "green" };
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI elaborate '{}' done: built from {} page(s){}.",
                template.key(),
                n,
                if failed_n > 0 {
                    format!(" ({} failed)", failed_n)
                } else {
                    String::new()
                }
            ),
            color,
            true
        )
    );
    if let Ok(st) = status.lock() {
        st.add_info_to_summary(
            "ai-elaborate",
            &format!(
                "Brand elaborate '{}' built from {} page(s) (MD + JSON + HTML).",
                template.key(),
                n
            ),
        );
        if failed_n > 0 {
            st.add_warning_to_summary(
                "ai-elaborate-failed",
                &format!(
                    "AI elaborate: {} page(s) failed extraction and are excluded (no fabricated values).",
                    failed_n
                ),
            );
        }
        st.set_ai_elaborate_doc(brand_doc);
    }
}

/// Run the two id-constrained selection rounds and merge into a plan.
#[allow(clippy::too_many_arguments)]
async fn run_selection(
    client: &Arc<AiClient>,
    shown: &[(String, Candidate)],
    all: &[Candidate],
    clusters: &[cluster::UrlCluster],
    max_tokens: u32,
    temperature: f32,
    reps: usize,
    budget: usize,
) -> select::SelectionPlan {
    let floor = (budget / 4).clamp(10, 40);
    // Two rounds; the second only runs when a section of the first asks for more pages.
    progress::start(TASK_SELECT, "Elaborate: select", 2);

    let user = format!(
        "<candidates>\n{}</candidates>\n<clusters>\n{}</clusters>",
        select::render_candidate_list(shown),
        select::render_cluster_list(clusters)
    );
    let ia_req = ChatRequest {
        system: Some(select::SELECT_IA_SYSTEM_PROMPT.to_string()),
        messages: vec![ChatMessage::user(user)],
        max_tokens,
        temperature,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    let ia = match progress::unit(
        TASK_SELECT,
        "round 1",
        client.complete_parsed_n(&ia_req, CAT_SELECT, SELECT_ATTEMPTS, select::parse_ia_map),
    )
    .await
    {
        Ok((ia, _)) => ia,
        Err(_) => {
            // Fall back to deterministic ranking + clustering.
            progress::finish(TASK_SELECT);
            return select::SelectionPlan {
                selected: all.iter().take(budget).cloned().collect(),
                clusters: Vec::new(),
                site_type: String::new(),
                notices: vec!["selection round 1 failed; used deterministic ranking".to_string()],
            };
        }
    };

    // Round 2: if any section wants expanding, present the second tier of candidates. The round
    // counts as done also when it is not needed.
    let expand_ids = progress::unit(TASK_SELECT, "round 2", async {
        if !(ia.sections.iter().any(|s| s.expand) && shown.len() > 40) {
            return Vec::new();
        }
        let second_tier: Vec<(String, Candidate)> = shown.iter().skip(40).take(120).cloned().collect();
        if second_tier.is_empty() {
            return Vec::new();
        }
        let user = format!(
            "<candidates>\n{}</candidates>",
            select::render_candidate_list(&second_tier)
        );
        let req = ChatRequest {
            system: Some(select::SELECT_EXPAND_SYSTEM_PROMPT.to_string()),
            messages: vec![ChatMessage::user(user)],
            max_tokens,
            temperature,
            json_mode: true,
            json_schema: None,
            schema_name: None,
        };
        match client
            .complete_parsed_n(&req, CAT_SELECT, SELECT_ATTEMPTS, select::parse_expand_ids)
            .await
        {
            // Map second-tier ids (offset by 40) back to `shown` indexes.
            Ok((ids, _)) => ids.into_iter().map(|id| id + 40).collect(),
            Err(_) => Vec::new(),
        }
    })
    .await;
    progress::finish(TASK_SELECT);

    select::merge_selection(shown, &ia, &expand_ids, all, clusters, reps, floor, budget)
}

/// Synthesize the prose sections (single-shot or sectioned map-reduce).
#[allow(clippy::too_many_arguments)]
async fn synthesize_prose(
    client: &Arc<AiClient>,
    model: &BrandModel,
    essences: &[(String, extract::PageEssence)],
    template: Template,
    report_language: &str,
    max_tokens: u32,
    model_output_cap: usize,
    max_output_kb: usize,
) -> std::collections::BTreeMap<String, String> {
    let sk = skeleton(template);
    let material = synthesize::build_material(model, essences, MATERIAL_BUDGET_KB);
    let material_kb = material.len() / 1024;
    let mode = synthesize::decide_mode(
        template,
        max_output_kb,
        model_output_cap,
        material_kb,
        Some(report_language),
    );
    let synth_max = max_tokens.max(32_000);

    let mut prose = std::collections::BTreeMap::new();
    match mode {
        synthesize::SynthMode::SingleShot => {
            let ids = prose_ids(template);
            let sys = synthesize::build_system_prompt(sk, &ids, report_language);
            let call = one_synthesis_call(client, &sys, &material, synth_max);
            if let Some(map) = progress::single_unit(TASK_SYNTH, SYNTH_LABEL, "all sections", call).await {
                prose.extend(map);
            }
        }
        synthesize::SynthMode::Sectioned(ids) => {
            eprintln!(
                "{}",
                utils::get_color_text(
                    &format!(
                        "  synthesis: sectioned mode ({} sections, material {} KB)",
                        ids.len(),
                        material_kb
                    ),
                    "cyan",
                    false
                )
            );
            progress::start(TASK_SYNTH, SYNTH_LABEL, ids.len() as u64);
            for id in ids {
                // Route only relevant essences for this section (+ the model).
                let routed: Vec<(String, extract::PageEssence)> = essences
                    .iter()
                    .filter(|(_, e)| synthesize::essence_matches_section(id, &e.page_role))
                    .cloned()
                    .collect();
                let mat = synthesize::build_material(model, &routed, 60);
                let sys = synthesize::build_system_prompt(sk, &[id], report_language);
                let call = one_synthesis_call(client, &sys, &mat, synth_max);
                if let Some(map) = progress::unit(TASK_SYNTH, id, call).await
                    && let Some(text) = map.get(id)
                {
                    prose.insert(id.to_string(), text.clone());
                }
            }
            progress::finish(TASK_SYNTH);
        }
    }
    // Keep only known prose ids (ignore any invented keys).
    let valid: std::collections::HashSet<&str> = prose_ids(template).into_iter().collect();
    prose.retain(|k, _| valid.contains(k.as_str()));
    prose
}

async fn one_synthesis_call(
    client: &Arc<AiClient>,
    system: &str,
    material: &str,
    max_tokens: u32,
) -> Option<std::collections::BTreeMap<String, String>> {
    let req = ChatRequest {
        system: Some(system.to_string()),
        messages: vec![ChatMessage::user(material.to_string())],
        max_tokens,
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    client
        .complete_parsed_n(&req, CAT_SYNTH, 2, synthesize::parse_prose_sections)
        .await
        .ok()
        .map(|(map, _)| map)
}

/// Correction pass over the synthesized PROSE only (never the verbatim structured lists). The model
/// quotes spans of the prose to fix/delete; each edit is applied to the single prose section that
/// uniquely contains it, so md/json/html stay consistent and no verbatim data can be altered.
async fn run_correction(
    client: &Arc<AiClient>,
    prose: &std::collections::BTreeMap<String, String>,
    model: &BrandModel,
    report_language: &str,
    max_tokens: u32,
) -> (std::collections::BTreeMap<String, String>, correct::CorrectionReport) {
    // Readable prose blob the corrector proofreads (section order is irrelevant here).
    let prose_blob = prose
        .values()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let material = serde_json::to_string(&model.to_json()).unwrap_or_default();
    let system = format!(
        r#"<role>
You proofread the connective prose of a finished brand profile and return safe replacement edits.
Fix spelling/grammar, remove leftover AI artifacts, and DELETE any sentence in <elaborate> that is
NOT supported by <source_material>. Never add new claims.
</role>
<rules>
Return a JSON array of edits. Each "from" MUST be a VERBATIM contiguous span of <elaborate>, a full
sentence or >=40 chars, occurring exactly once. "to" is the replacement ("" deletes the span). Write
"note" in report language '{lang}'. Keep verbatim facts, names, emails and quotes unchanged.
</rules>
<output_contract>Return ONLY {{"edits":[{{"from":"","to":"","reason":"typo|artifact|unsupported|grammar","note":""}}]}}. No prose, no code fences.</output_contract>"#,
        lang = report_language
    );
    let user = format!(
        "<source_material>\n{}\n</source_material>\n<elaborate>\n{}\n</elaborate>",
        crate::ai::prompt::truncate_chars(&material, 60_000),
        crate::ai::prompt::truncate_chars(&prose_blob, 60_000)
    );
    let req = ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(user)],
        max_tokens,
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    let call = client.complete_parsed_n(&req, CAT_CORRECT, 2, correct::parse_edits);
    match progress::single_unit(TASK_CORRECT, "Elaborate: correction", "prose", call).await {
        Ok((edits, _)) => {
            let (new_prose, report) = apply_corrections_to_prose(prose, &edits);
            eprintln!(
                "{}",
                utils::get_color_text(
                    &format!(
                        "  correction: {} proposed, {} applied, {} skipped.",
                        report.proposed,
                        report.applied,
                        report.skipped_no_match + report.skipped_ambiguous + report.skipped_overlap
                    ),
                    "cyan",
                    false
                )
            );
            (new_prose, report)
        }
        Err(_) => (prose.clone(), correct::CorrectionReport::default()),
    }
}

/// Apply correction edits to the prose map. An edit is applied only if its `from` span occurs in
/// EXACTLY ONE section and EXACTLY ONCE there (global uniqueness); ambiguous/absent spans are skipped
/// (never a fabricated or replace-all edit). Structured verbatim data is never in `prose`, so it
/// cannot be touched.
fn apply_corrections_to_prose(
    prose: &std::collections::BTreeMap<String, String>,
    edits: &[correct::Edit],
) -> (std::collections::BTreeMap<String, String>, correct::CorrectionReport) {
    let mut report = correct::CorrectionReport {
        proposed: edits.len(),
        ..Default::default()
    };
    let mut per_section: std::collections::BTreeMap<String, Vec<correct::Edit>> = std::collections::BTreeMap::new();
    for e in edits {
        if e.from.trim().is_empty() {
            report.skipped_no_match += 1;
            continue;
        }
        let mut containing: Option<&String> = None;
        let mut total = 0usize;
        for (id, text) in prose {
            let n = text.matches(e.from.as_str()).count();
            if n > 0 {
                total += n;
                containing = Some(id);
            }
        }
        match (containing, total) {
            (Some(id), 1) => per_section.entry(id.clone()).or_default().push(e.clone()),
            (_, 0) => report.skipped_no_match += 1,
            _ => report.skipped_ambiguous += 1,
        }
    }
    let mut out = prose.clone();
    for (id, section_edits) in per_section {
        if let Some(text) = out.get(&id) {
            let (new_text, sub) = correct::apply_edits(text, &section_edits);
            out.insert(id, new_text);
            report.applied += sub.applied;
            report.skipped_overlap += sub.skipped_overlap;
        }
    }
    (out, report)
}

fn path_hint(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .map(|u| {
            let seg = u.path().trim_matches('/').to_string();
            if seg.is_empty() { "/".to_string() } else { seg }
        })
        .unwrap_or_else(|| url.to_string())
}

fn report_error(status: &Arc<Mutex<Status>>, msg: &str) {
    eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", msg), "red", true));
    crate::events::emit_ai_issue(ELABORATE_FAILED, msg);
    if let Ok(st) = status.lock() {
        st.add_critical_to_summary("ai-elaborate-error", msg);
    }
}
