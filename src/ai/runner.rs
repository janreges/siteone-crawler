// SiteOne Crawler - AI runner (post-crawl AI phase)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Orchestrates the optional AI phase: resolve config/key, select & rank pages, run the
// requested actions concurrently (own pool + rate limiter), then render results into the
// report (tables + summary). All network I/O happens without holding any Mutex.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::Semaphore;

use super::actions::{custom, extract, llms_txt, seo, typos};
use super::client::AiClient;
use super::config::{AiConfig, resolve_api_key};
use super::page::PageContext;
use super::progress;
use super::provider::Provider;
use super::report::locale::ReportLocale;
use super::report::model::{AiReportModel, AiUsageMeta, CoverageMeta, ReportRow, RulePackMeta, SiteMeta};
use super::report::presets::preset_by_key;
use super::report::schema::{load_schema_file, parse_fields_dsl};
use super::selection::{RankedPage, select_pages};
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::options::core_options::CoreOptions;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const AI_SEO_TABLE: &str = "ai-seo";

// Human-readable analysis-type labels for per-type token accounting (shown in the summary).
const CAT_SEO: &str = "SEO analysis";
const CAT_TYPOS: &str = "Content issues (typos)";
const CAT_CUSTOM: &str = "Custom check";
const CAT_LLMS: &str = "llms.txt summaries";
const CAT_EXTRACT: &str = "AI report (extract)";

// Progress task keys of the per-page actions (see `progress`).
const TASK_SEO: &str = "seo";
const TASK_TYPOS: &str = "typos";
const TASK_CUSTOM: &str = "custom";

/// Entry point for the post-crawl AI phase. Fail-soft: never panics, never aborts the crawl.
pub async fn run_ai(options: &CoreOptions, status: &Arc<Mutex<Status>>, output: &Arc<Mutex<Box<dyn Output>>>) {
    crate::ai::usage::reset();
    crate::ai::client::reset_rate_limiter().await;
    // --- Phase A: select pages and extract their context (under a short read lock) ---
    let (selection_summary, pages, context_failures) = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let include: Vec<String> = options.ai_include.clone();
        let exclude: Vec<String> = options.ai_exclude.clone();
        let sel = select_pages(&st, &include, &exclude, options.ai_max_pages.max(1) as usize);

        let mut pages: Vec<(RankedPage, PageContext)> = Vec::new();
        let mut context_failures: Vec<RankedPage> = Vec::new();
        for rp in &sel.selected {
            if let Some(ctx) = PageContext::build(&st, &rp.uq_id, &rp.url, options) {
                pages.push((rp.clone(), ctx));
            } else {
                context_failures.push(rp.clone());
            }
        }
        let summary = SelectionSummary {
            selected: sel.selected.len(),
            total_html: sel.total_html_pages,
            eligible: sel.total_eligible_before_masks,
            candidates: sel.total_candidates_before_cap,
            excluded: sel.excluded_by_mask,
            context_failed: context_failures.len(),
            capped_dropped: sel.total_candidates_before_cap.saturating_sub(sel.selected.len()),
            initial_call_estimate: 0,
            worst_case_call_budget: 0,
        };
        (summary, pages, context_failures)
    };

    let model = options.ai_model.clone().unwrap_or_default();
    crate::ai::usage::note_model(&model);
    let provider = Provider::parse(&options.ai_provider).unwrap_or(Provider::OpenAiCompatible);

    eprintln!(
        "\n{}",
        utils::get_color_text(
            &format!(
                "AI phase: {} candidate page(s) selected for actions [{}] using {} / {}",
                selection_summary.selected,
                options.ai_actions.join(", "),
                provider.as_str(),
                model
            ),
            "cyan",
            true
        )
    );
    // Each LLM-calling action is one request per page (llms-txt/llms-full share one summary call).
    let calls_per_page = {
        let a = &options.ai_actions;
        (a.iter().any(|x| x == "seo") as usize)
            + (a.iter().any(|x| x == "typos") as usize)
            + (a.iter().any(|x| x == "custom") as usize)
            + (a.iter().any(|x| x == "extract") as usize)
            + (a.iter().any(|x| x == "llms-txt" || x == "llms-full") as usize)
    };
    let summary_calls = if options.ai_actions.iter().any(|action| action == "summary") {
        6
    } else {
        0
    };
    let total_calls = pages.len() * calls_per_page + summary_calls;
    let worst_case_calls = {
        let a = &options.ai_actions;
        let per_page = (a.iter().any(|x| x == "seo") as usize) * 7
            + (a.iter().any(|x| x == "typos") as usize) * 7
            + (a.iter().any(|x| x == "custom") as usize) * 7
            // Three validation attempts can each perform three transport attempts. A native-schema
            // capability rejection may add one prompt-mode fallback request before that budget.
            + (a.iter().any(|x| x == "extract") as usize) * 10
            + (a.iter().any(|x| x == "llms-txt" || x == "llms-full") as usize) * 7;
        pages.len() * per_page + summary_calls * 3
    };
    let selection_summary = SelectionSummary {
        initial_call_estimate: if options.ai_actions.iter().any(|action| action == "extract") {
            pages.len()
        } else {
            0
        },
        worst_case_call_budget: if options.ai_actions.iter().any(|action| action == "extract") {
            pages.len() * 10
        } else {
            0
        },
        ..selection_summary
    };
    eprintln!(
        "  ({} HTML pages crawled, {} eligible, {} excluded by masks, {} candidate(s), capped to --ai-max-pages={}) → {} initial LLM call(s), up to {} with retries",
        selection_summary.total_html,
        selection_summary.eligible,
        selection_summary.excluded,
        selection_summary.candidates,
        options.ai_max_pages,
        total_calls,
        worst_case_calls
    );

    // Rough input-token estimate for the preview (across all selected actions).
    let est_input_tokens: usize = pages
        .iter()
        .map(|(_, c)| (c.content_markdown.chars().count() + 1500) / 4)
        .sum::<usize>()
        * calls_per_page.max(1);

    // --- Dry run: show the plan and stop before any API call ---
    if options.ai_dry_run {
        eprintln!(
            "{}",
            utils::get_color_text(
                &format!(
                    "AI dry-run: would make {} initial LLM call(s) (worst-case retry budget {} requests) over {} page(s), est. input ~{} tokens. No API calls made.",
                    total_calls,
                    worst_case_calls,
                    pages.len(),
                    est_input_tokens
                ),
                "yellow",
                true
            )
        );
        if let Some(input_rate) = options.ai_input_cost_per_million {
            eprintln!(
                "  Estimated input-only cost floor: ${:.6} USD at the supplied input-token rate (output and retries excluded).",
                est_input_tokens as f64 * input_rate / 1_000_000.0
            );
        }
        for (i, (rp, ctx)) in pages.iter().take(20).enumerate() {
            eprintln!("  {:>3}. score {:>5.1}  {}", i + 1, rp.score, ctx.url);
        }
        if let Ok(st) = status.lock() {
            st.add_info_to_summary(
                "ai-dry-run",
                &format!(
                    "AI dry-run: {} page(s) would be analyzed (~{} input tokens)",
                    pages.len(),
                    est_input_tokens
                ),
            );
        }
        return;
    }

    if pages.is_empty() && context_failures.is_empty() {
        if let Ok(st) = status.lock() {
            st.add_notice_to_summary("ai-no-pages", "AI enabled but no pages matched the selection criteria.");
        }
        return;
    }
    if pages.is_empty() && !options.ai_actions.iter().any(|action| action == "extract") {
        if let Ok(st) = status.lock() {
            st.add_warning_to_summary(
                "ai-no-page-content",
                "AI actions were skipped because none of the selected pages had retained content for prompt construction.",
            );
        }
        return;
    }

    // --- Build the client (resolve the API key here, never stored in CoreOptions) ---
    let api_key = if pages.is_empty() {
        // All selected pages failed context extraction. The extract action can still emit honest
        // error rows and coverage metadata without touching a provider or resolving credentials.
        None
    } else {
        match resolve_api_key(
            provider,
            options.ai_api_key.as_ref().map(|s| s.expose()),
            options.ai_api_key_env.as_deref(),
            options.ai_api_key_file.as_deref(),
        ) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", e), "red", true));
                if let Ok(st) = status.lock() {
                    st.add_critical_to_summary("ai-key-error", &format!("AI phase skipped: {}", e));
                }
                return;
            }
        }
    };
    // Hosted providers require a key; openai-compatible (e.g. local vLLM) may not.
    if !pages.is_empty() && api_key.is_none() && provider != Provider::OpenAiCompatible {
        let msg = format!(
            "AI is enabled but no API key resolved for provider '{}'. Set {} or use --ai-api-key-file.",
            provider.as_str(),
            provider.default_key_env()
        );
        eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", msg), "red", true));
        if let Ok(st) = status.lock() {
            st.add_critical_to_summary("ai-key-missing", &format!("AI phase skipped: {}", msg));
        }
        return;
    }

    let endpoint = options
        .ai_endpoint
        .clone()
        .or_else(|| provider.default_endpoint().map(|s| s.to_string()))
        .unwrap_or_default();

    let extra_body = options
        .ai_extra_body
        .as_ref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

    let cache_dir = match options.ai_cache_dir.as_deref() {
        None | Some("") | Some("off") => None,
        Some(d) => Some(utils::get_absolute_path(d)),
    };

    let config = AiConfig {
        provider,
        endpoint,
        model: model.clone(),
        api_key,
        max_tokens: options.ai_max_tokens.clamp(1, 1_000_000) as u32,
        temperature: options.ai_temperature as f32,
        force_completion_tokens: options.ai_use_max_completion_tokens,
        extra_body,
        timeout_secs: options.ai_timeout.clamp(1, 3600) as u64,
        cache_dir,
        max_reqs_per_sec: options.ai_max_reqs_per_sec.filter(|rate| *rate > 0.0),
    };
    let client = Arc::new(AiClient::new(config));

    // --- Run the requested actions across pages (own concurrency pool + rate limiter) ---
    let actions = &options.ai_actions;
    if actions.iter().any(|a| a == "seo") {
        run_seo_action(options, &client, &pages, status, output).await;
    }
    if actions.iter().any(|a| a == "llms-txt" || a == "llms-full") {
        run_llms_action(options, &client, &pages, status).await;
    }
    if actions.iter().any(|a| a == "typos") {
        run_typos_action(options, &client, &pages, status, output).await;
    }
    if actions.iter().any(|a| a == "custom") {
        run_custom_action(options, &client, &pages, status, output).await;
    }
    if actions.iter().any(|a| a == "extract") {
        let usage_before = crate::ai::usage::snapshot();
        run_extract_action(
            options,
            &client,
            &pages,
            &context_failures,
            &selection_summary,
            usage_before,
            status,
        )
        .await;
    }
}

/// Whether `--ai-schema-enforce=auto` should enable native schema constraint for this provider.
/// Conservative on purpose: only the providers where `json_schema` is reliably honored
/// (hosted OpenAI, Gemini). OpenAI-compatible endpoints (vLLM/SGLang/etc.) vary widely in their
/// `guided_json`/`json_schema` support and some return only the first field or hang under
/// constrained decoding, so `auto` uses the embedded contract plus generic JSON mode there; a user
/// with a known-good endpoint can force it with `--ai-schema-enforce=on`. Anthropic has no
/// response_format at all.
fn auto_enforce_for_provider(provider: Provider, model: &str) -> bool {
    super::provider::supports_native_schema_auto(provider, model)
}

/// The first-class AI report `extract` action: run the typed per-page schema over all selected
/// pages, coerce the results, and store an `AiReportModel` in Status for the report exporters.
async fn run_extract_action(
    options: &CoreOptions,
    client: &Arc<AiClient>,
    pages: &[(RankedPage, PageContext)],
    context_failures: &[RankedPage],
    selection: &SelectionSummary,
    usage_before: crate::ai::usage::UsageSnapshot,
    status: &Arc<Mutex<Status>>,
) {
    // Resolve the schema + prompt: a preset, or a custom field schema.
    let report_key = options.ai_report.clone().unwrap_or_else(|| "extract".to_string());
    let locale = ReportLocale::new(&options.ai_report_language);
    let (mut fields, system_prompt, _preset_title, hero) = if report_key == "extract" {
        let fields = if let Some(ref dsl) = options.ai_extract_fields {
            match parse_fields_dsl(dsl) {
                Ok(f) => f,
                Err(e) => {
                    report_extract_error(status, &format!("invalid --ai-extract-fields: {}", e));
                    return;
                }
            }
        } else if let Some(ref path) = options.ai_schema_file {
            match load_schema_file(path) {
                Ok(f) => f,
                Err(e) => {
                    report_extract_error(status, &e);
                    return;
                }
            }
        } else {
            report_extract_error(status, "extract report needs --ai-extract-fields or --ai-schema-file");
            return;
        };
        (
            fields,
            CUSTOM_EXTRACT_PROMPT.to_string(),
            "Custom AI Extract".to_string(),
            "distribution".to_string(),
        )
    } else if let Some(preset) = preset_by_key(&report_key) {
        (
            preset.fields,
            preset.system_prompt,
            preset.title.to_string(),
            preset.hero.to_string(),
        )
    } else {
        report_extract_error(status, &format!("unknown --ai-report '{}'", report_key));
        return;
    };
    let title = locale.preset_title(&report_key).to_string();
    let host = options.get_initial_host(false);
    let site_name = compute_common_site_suffix(options, pages);

    // The IA title is deterministic crawler metadata. Keep it in the report schema, but do not ask
    // the model to repeat or rewrite it; this also prevents a weak model's title from causing a
    // needless retry or overriding the source value.
    let extraction_fields = if report_key == "ia" {
        fields
            .iter()
            .filter(|field| field.name != "title")
            .cloned()
            .collect::<Vec<_>>()
    } else {
        fields.clone()
    };
    if report_key != "extract" {
        for field in &mut fields {
            if let Some(description) = locale.field_description(&field.name) {
                field.desc = description.to_string();
            }
        }
    }

    let (max_tokens, temperature, concurrency) = action_params(options);
    let enforce_schema = match options.ai_schema_enforce.as_str() {
        "on" if client.provider() == Provider::Anthropic => {
            report_extract_error(
                status,
                "--ai-schema-enforce=on is not supported for Anthropic; use auto/off embedded-contract mode",
            );
            return;
        }
        "on" => true,
        "off" => false,
        _ => auto_enforce_for_provider(client.provider(), &options.ai_model.clone().unwrap_or_default()),
    };

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI report '{}': extracting {} field(s) from {} page(s) (schema enforcement: {})...",
                report_key,
                extraction_fields.len(),
                pages.len(),
                if enforce_schema { "on" } else { "prompt" }
            ),
            "cyan",
            false
        )
    );

    let fields = Arc::new(fields);
    let extraction_fields = Arc::new(extraction_fields);
    let system_prompt = Arc::new(system_prompt);
    let report_language = Arc::new(locale.code().to_string());
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    let task = format!("report:{}", report_key);
    progress::start(&task, &format!("Report '{}'", report_key), pages.len() as u64);

    for (_rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let ctx = ctx.clone();
        let extraction_fields = extraction_fields.clone();
        let system_prompt = system_prompt.clone();
        let report_language = report_language.clone();
        let report_key_for_task = report_key.clone();
        let task_ctx = ctx.clone();
        let subject = url_path_and_query(&ctx.url);
        handles.push((
            task_ctx,
            tokio::spawn(progress::unit(task.clone(), subject, async move {
                let _permit = permit;
                let req = extract::build_request(
                    &extraction_fields,
                    &system_prompt,
                    &ctx,
                    &report_key_for_task,
                    &report_language,
                    max_tokens,
                    temperature,
                    enforce_schema,
                );
                // Up to 3 model-output attempts after syntax repair and semantic validation. The HTTP
                // layer separately handles safe transport/status retries. If all validation attempts
                // fail, the page is an honest error, never a row with fabricated defaults.
                let result = client
                    .complete_parsed_n(&req, CAT_EXTRACT, EXTRACT_MAX_ATTEMPTS, |t| {
                        let mut cells = extract::parse_and_coerce(&extraction_fields, t)?;
                        if report_key_for_task == "ia" {
                            let description = match cells.get("description") {
                                Some(crate::ai::report::model::ReportCell::Str(value)) => value,
                                _ => return Err("IA description is missing or has the wrong type".to_string()),
                            };
                            let description = super::report::ia::normalize_description(description)?;
                            cells.insert(
                                "description".to_string(),
                                crate::ai::report::model::ReportCell::Str(description),
                            );
                        }
                        let validation = if report_key_for_task == "compliance" {
                            Some(super::report::compliance::validate_cells(
                                &mut cells,
                                &extract::evidence_text(&ctx, &report_key_for_task),
                                extract::input_was_truncated(&ctx, &report_key_for_task),
                                &report_language,
                            )?)
                        } else {
                            None
                        };
                        Ok((cells, validation))
                    })
                    .await;
                (ctx, result)
            })),
        ));
    }

    // Site metadata for the report header.
    let dispatched = pages.len();
    let mut rows: Vec<ReportRow> = context_failures
        .iter()
        .map(|page| {
            ReportRow::errored(
                page.url.clone(),
                url_path_and_query(&page.url),
                "selected page content was unavailable for AI context extraction".to_string(),
            )
        })
        .collect();
    let mut error_count = context_failures.len();
    let mut parse_error_count = 0usize;
    let mut request_error_count = 0usize;
    let mut dropped = 0usize;
    for (task_ctx, h) in handles {
        match h.await {
            Ok((ctx, result)) => {
                let path = url_path_and_query(&ctx.url);
                match result {
                    Ok(((mut cells, validation), _)) => {
                        // IA titles are source metadata, not model-authored prose. This guarantees
                        // consistent site-suffix removal even when the model returned a non-empty title.
                        if report_key == "ia" {
                            let title = super::report::ia::clean_source_title(&ctx.title, &ctx.h1, &site_name, &host)
                                .map(crate::ai::report::model::ReportCell::Str)
                                .unwrap_or(crate::ai::report::model::ReportCell::Null);
                            cells.insert("title".to_string(), title);
                        }
                        let mut row = ReportRow::ok(ctx.url.clone(), path, cells);
                        let input_truncated = extract::input_was_truncated(&ctx, &report_key);
                        if let Some(validation) = validation {
                            row = row.with_evidence(
                                validation.evidence_limited,
                                input_truncated,
                                validation.indeterminate_rules,
                            );
                        } else if input_truncated {
                            row = row.with_evidence(true, true, Vec::new());
                        }
                        rows.push(row);
                    }
                    Err(e) => {
                        error_count += 1;
                        let error = e.to_string();
                        if error.contains("invalid response:") {
                            parse_error_count += 1;
                        } else {
                            request_error_count += 1;
                        }
                        // Per-page visibility so a slow/unhealthy endpoint isn't a silent hang.
                        eprintln!(
                            "{}",
                            utils::get_color_text(
                                &format!("  AI report: {} → error: {}", url_path_and_query(&ctx.url), error),
                                "yellow",
                                false
                            )
                        );
                        let mut row = ReportRow::errored(ctx.url.clone(), path, error);
                        row.input_truncated = extract::input_was_truncated(&ctx, &report_key);
                        if row.input_truncated {
                            row.evidence_status = Some("unavailable".to_string());
                        }
                        rows.push(row);
                    }
                }
            }
            // A JoinError (task panic/cancel): we cannot recover the URL, but we MUST NOT silently
            // under-count — surface it (mirrors run_seo_action's fail accounting).
            Err(_) => {
                dropped += 1;
                error_count += 1;
                let path = url_path_and_query(&task_ctx.url);
                let input_truncated = extract::input_was_truncated(&task_ctx, &report_key);
                let mut row = ReportRow::errored(
                    task_ctx.url,
                    path,
                    "internal AI report task failed or was cancelled".to_string(),
                );
                row.input_truncated = input_truncated;
                if input_truncated {
                    row.evidence_status = Some("unavailable".to_string());
                }
                rows.push(row);
            }
        }
    }
    progress::finish(&task);

    let analyzed = rows.len().saturating_sub(error_count);
    let content_truncated = rows.iter().filter(|row| row.input_truncated).count();
    let indeterminate = rows
        .iter()
        .filter(|row| row.evidence_status.as_deref() == Some("limited"))
        .count();
    let usage = crate::ai::usage::snapshot().delta_since(usage_before);
    let network_completions = usage.calls.saturating_sub(usage.cache_hits);
    let unaccounted_http_attempts = usage.http_attempts.saturating_sub(network_completions);
    let (estimated_cost_usd, cost_estimate_status) = match (
        options.ai_input_cost_per_million,
        options.ai_output_cost_per_million,
    ) {
        (Some(input_rate), Some(output_rate)) => {
            let estimate =
                (usage.prompt_tokens as f64 * input_rate + usage.completion_tokens as f64 * output_rate) / 1_000_000.0;
            let status = if usage.calls_without_usage > 0 || unaccounted_http_attempts > 0 {
                if locale.is_czech() {
                    format!(
                        "Částečný odhad z uživatelem zadaných sazeb; {} dokončených odpovědí a {} dalších HTTP pokusů nemá úplné údaje o tokenech",
                        usage.calls_without_usage, unaccounted_http_attempts
                    )
                } else {
                    format!(
                        "Partial estimate from user-supplied rates; {} completed responses and {} additional HTTP attempts lack complete token usage",
                        usage.calls_without_usage, unaccounted_http_attempts
                    )
                }
            } else if locale.is_czech() {
                "Odhad z uživatelem zadaných sazeb a vykázané spotřeby tokenů".to_string()
            } else {
                "Estimate from user-supplied rates and reported token usage".to_string()
            };
            (Some(estimate), status)
        }
        _ => (
            None,
            if locale.is_czech() {
                "Cena nebyla odhadnuta; nastavte sazby za vstupní a výstupní tokeny".to_string()
            } else {
                "Cost not estimated; set input and output token rates to enable it".to_string()
            },
        ),
    };
    let site = SiteMeta {
        host,
        url: crate::utils::redact_url_userinfo(&options.url),
        crawled_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        pages_analyzed: analyzed,
        provider: client.provider().as_str().to_string(),
        model: options.ai_model.clone().unwrap_or_default(),
        report_language: locale.code().to_string(),
        chrome_language: locale.catalog_language().to_string(),
        coverage: CoverageMeta {
            crawled_html: selection.total_html,
            eligible: selection.eligible,
            excluded: selection.excluded,
            selected: selection.selected,
            capped_dropped: selection.capped_dropped,
            context_failed: selection.context_failed,
            analyzed,
            failed: error_count,
            parse_failed: parse_error_count,
            request_failed: request_error_count,
            task_dropped: dropped,
            content_truncated,
            indeterminate,
            include_masks: options.ai_include.clone(),
            exclude_masks: options.ai_exclude.clone(),
            ranking_method: if locale.is_czech() {
                "skóre důležitosti: domovská stránka/hloubka, počet objevených odkazů, přítomnost v sitemapě a krátkost URL"
                    .to_string()
            } else {
                "importance score: homepage/depth, discovery fanout, sitemap presence, URL shallowness".to_string()
            },
            sampled: selection.selected < selection.total_html
                || error_count > 0
                || content_truncated > 0
                || indeterminate > 0,
        },
        usage: AiUsageMeta {
            calls: usage.calls,
            cache_hits: usage.cache_hits,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            calls_without_usage: usage.calls_without_usage,
            unaccounted_http_attempts,
            http_attempts: usage.http_attempts,
            retries: usage.retries,
            network_time_seconds: usage.network_time_s,
            initial_call_estimate: selection.initial_call_estimate,
            worst_case_call_budget: selection.worst_case_call_budget,
            input_cost_per_million_usd: options.ai_input_cost_per_million,
            output_cost_per_million_usd: options.ai_output_cost_per_million,
            estimated_cost_usd,
            cost_estimate_status,
        },
        rule_pack: (report_key == "compliance").then(|| RulePackMeta {
            version: super::report::compliance::RULE_PACK_VERSION.to_string(),
            sources: vec![
                super::report::compliance::CCD2_SOURCE_URL.to_string(),
                super::report::compliance::UCPD_SOURCE_URL.to_string(),
            ],
            risk_formula: if locale.is_czech() {
                "součet základních závažností modelově klasifikovaných nálezů s ověřeným výňatkem: kritická=40, vysoká=25, střední=12, nízká=5, informativní=2; možnosti členského státu MAY=0; maximum 100"
                    .to_string()
            } else {
                "sum of baseline severities for model-classified findings with excerpt grounding: critical=40, high=25, medium=12, low=5, info=2; Member-State MAY options=0; capped at 100"
                    .to_string()
            },
            applicability: if locale.is_czech() {
                "Profil připravenosti pro reklamu na spotřebitelský úvěr v EU. Crawler neurčuje jurisdikci, působnost produktu ani výjimky podle článku 2 CCD2; ty musí příjemce ověřit. Textové signály UCPD mají širší rozsah, ale stále vyžadují posouzení celé praktiky."
                    .to_string()
            } else {
                "EU consumer-credit advertising readiness profile. The crawler does not determine jurisdiction, product scope, or CCD2 Article 2 exclusions; recipients must verify them. Textual UCPD signals have broader scope but still require assessment of the full practice."
                    .to_string()
            },
            review_status: if locale.is_czech() {
                "Technická poradní taxonomie založená na citovaných primárních zdrojích. Crawler ověřuje strukturu, povolené ID a přítomnost výňatku ve vstupu; přiřazení pravidla k významu textu zůstává modelovým úsudkem. Před právním spoléháním je nutná kontrola kvalifikovaným právníkem. Právní stav byl ověřen při verzování 21. 7. 2026."
                    .to_string()
            } else {
                "Engineering advisory taxonomy based on the cited primary sources. The crawler verifies structure, allowed IDs, and excerpt presence; mapping a rule to the text's meaning remains model judgment. Obtain qualified legal review before relying on it. Legal status was checked when versioned on 2026-07-21."
                    .to_string()
            },
        }),
    };
    let mut report = AiReportModel::new(&report_key, &title, site, (*fields).clone(), &hero);
    report.rows = rows;
    report.narrative = if report_key == "topics" {
        Some(super::report::synthesis::build_topic_analysis(
            &mut report,
            locale.code(),
        ))
    } else {
        None
    };
    report.build_rollups();

    let total = report.rows.len();
    let color = if error_count > 0 { "yellow" } else { "green" };
    let err_note = if error_count > 0 {
        format!(
            " ({} page(s) failed: {} invalid response(s), {} request failure(s), {} missing input(s), {} internal task failure(s) — reported as errors)",
            error_count,
            parse_error_count,
            request_error_count,
            context_failures.len(),
            dropped
        )
    } else {
        String::new()
    };
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI report '{}' done: {} page(s) extracted{}.",
                report_key, analyzed, err_note
            ),
            color,
            true
        )
    );
    if let Ok(st) = status.lock() {
        st.add_info_to_summary(
            "ai-report",
            &format!(
                "AI report '{}' generated for {} of {} page(s) (JSON + HTML).",
                report_key, analyzed, total
            ),
        );
        if parse_error_count > 0 {
            st.add_warning_to_summary(
                "ai-report-parse-errors",
                &format!(
                    "AI report: {} page(s) could not be parsed after {} attempts and are marked as errors (no fabricated values).",
                    parse_error_count, EXTRACT_MAX_ATTEMPTS
                ),
            );
        }
        if request_error_count > 0 {
            st.add_warning_to_summary(
                "ai-report-request-errors",
                &format!(
                    "AI report: {} page(s) failed because the provider request did not complete successfully and are marked as errors.",
                    request_error_count
                ),
            );
        }
        if !context_failures.is_empty() {
            st.add_warning_to_summary(
                "ai-report-context-errors",
                &format!(
                    "AI report: {} selected page(s) had no retained content for AI extraction and are marked as errors.",
                    context_failures.len()
                ),
            );
        }
        // A task that panicked/was cancelled yields no row — surface it rather than silently
        // under-counting (dispatched vs collected).
        if dropped > 0 {
            st.add_warning_to_summary(
                "ai-report-dropped",
                &format!(
                    "AI report: {} of {} dispatched page(s) were dropped due to internal task errors (not analyzed).",
                    dropped, dispatched
                ),
            );
        }
        st.set_ai_report_model(report);
    }
}

/// Total attempts (1 initial + 2 retries) to get a parseable extract response before a page is
/// recorded as an honest error.
const EXTRACT_MAX_ATTEMPTS: u32 = 3;

fn report_extract_error(status: &Arc<Mutex<Status>>, msg: &str) {
    eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", msg), "red", true));
    if let Ok(st) = status.lock() {
        st.add_critical_to_summary("ai-report-error", &format!("AI report skipped: {}", msg));
    }
}

const CUSTOM_EXTRACT_PROMPT: &str = r#"<role>
You extract structured data from a single web page according to the field schema below. Return
factual, concise values grounded in the page content. This is data extraction, not marketing.
</role>

<instructions>
- Fill every field as accurately as the page content allows. Use JSON null only for a field declared
  optional when its value is genuinely unknown; never invent a neutral default.
- Follow each field's description precisely. Write generated prose in the requested report language.
</instructions>"#;

/// Shared concurrency settings for an action.
fn action_params(options: &CoreOptions) -> (u32, f32, usize) {
    let max_tokens = options.ai_max_tokens.clamp(1, 1_000_000) as u32;
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;
    (max_tokens, temperature, concurrency)
}

struct SelectionSummary {
    selected: usize,
    total_html: usize,
    eligible: usize,
    candidates: usize,
    excluded: usize,
    context_failed: usize,
    capped_dropped: usize,
    initial_call_estimate: usize,
    worst_case_call_budget: usize,
}

async fn run_seo_action(
    options: &CoreOptions,
    client: &Arc<AiClient>,
    pages: &[(RankedPage, PageContext)],
    status: &Arc<Mutex<Status>>,
    output: &Arc<Mutex<Box<dyn Output>>>,
) {
    let max_tokens = options.ai_max_tokens.clamp(1, 1_000_000) as u32;
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;

    // A single consistent site name (from the homepage) used in all recommended titles.
    let site_name = compute_site_name(options, pages);

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    progress::start(TASK_SEO, "SEO", pages.len() as u64);

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let site_name = site_name.clone();
        let is_homepage = url_is_homepage(&ctx.url);
        let subject = url_path_and_query(&ctx.url);
        handles.push(tokio::spawn(progress::unit(TASK_SEO, subject, async move {
            let _permit = permit;
            let req = seo::build_request(&ctx, &site_name, is_homepage, max_tokens, temperature);
            // Retries once after a short delay on any failure (network, provider, or a malformed
            // response that fails to parse).
            let outcome = match client.complete_parsed(&req, CAT_SEO, seo::parse).await {
                Ok((result, completion)) => SeoOutcome::Ok(Box::new(SeoOk {
                    result,
                    prompt_tokens: completion.usage.input(),
                    completion_tokens: completion.usage.output(),
                    from_cache: completion.from_cache,
                })),
                Err(e) => SeoOutcome::CallError(e.to_string()),
            };
            (rp, ctx, outcome)
        })));
    }

    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    let mut ok_count = 0usize;
    let mut fail_count = 0usize;
    let mut overall_sum = 0i64;
    let mut prompt_tokens_total = 0u64;
    let mut completion_tokens_total = 0u64;
    let mut cache_hits = 0usize;
    let mut low_pages: Vec<(String, i32)> = Vec::new();

    for h in handles {
        let (rp, ctx, outcome) = match h.await {
            Ok(v) => v,
            Err(_) => {
                fail_count += 1;
                continue;
            }
        };
        match outcome {
            SeoOutcome::Ok(ok) => {
                let SeoOk {
                    result,
                    prompt_tokens,
                    completion_tokens,
                    from_cache,
                } = *ok;
                ok_count += 1;
                overall_sum += result.scores.overall as i64;
                prompt_tokens_total += prompt_tokens;
                completion_tokens_total += completion_tokens;
                if from_cache {
                    cache_hits += 1;
                }
                if result.scores.overall < 50 {
                    low_pages.push((rp.url.clone(), result.scores.overall));
                }
                let mut row = HashMap::new();
                row.insert("urlPathAndQuery".to_string(), url_path_and_query(&ctx.url));
                row.insert("overall".to_string(), format!("{}%", result.scores.overall));
                row.insert("titleScore".to_string(), format!("{}%", result.scores.title));
                row.insert("descScore".to_string(), format!("{}%", result.scores.meta_description));
                row.insert("recommendedTitle".to_string(), result.recommendations.title.clone());
                row.insert(
                    "recommendedDescription".to_string(),
                    result.recommendations.meta_description.clone(),
                );
                rows.push(row);
            }
            SeoOutcome::CallError(e) => {
                fail_count += 1;
                eprintln!(
                    "  {}",
                    utils::get_color_text(
                        &format!("AI SEO failed for {} (after retry): {}", ctx.url, e),
                        "yellow",
                        false
                    )
                );
            }
        }
    }
    progress::finish(TASK_SEO);

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI SEO done: {} ok, {} failed ({} cache hits). Tokens: prompt {}, completion {}.",
                ok_count, fail_count, cache_hits, prompt_tokens_total, completion_tokens_total
            ),
            "green",
            true
        )
    );

    // --- Render results into the report (tables + summary) ---
    let avg = if ok_count > 0 { overall_sum / ok_count as i64 } else { 0 };

    let table = build_seo_table(rows);
    {
        if let (Ok(st), Ok(mut out)) = (status.lock(), output.lock()) {
            let mut table = table;
            st.configure_super_table_url_stripping(&mut table);
            out.add_super_table(&table);
            // Also store in Status so the table appears in the HTML report.
            st.add_super_table_at_end(table);
        }
    }

    if let Ok(st) = status.lock() {
        if ok_count > 0 {
            st.add_info_to_summary(
                "ai-seo-summary",
                &format!("AI SEO analyzed {} page(s); average overall score {}%.", ok_count, avg),
            );
        }
        if fail_count > 0 {
            st.add_notice_to_summary(
                "ai-seo-failures",
                &format!("AI SEO could not analyze {} page(s) (call/parse errors).", fail_count),
            );
        }
        // Advisory by default; only deduct from the score when explicitly opted in.
        for (url, score) in low_pages.iter().take(20) {
            let msg = format!("AI SEO flags weak on-page SEO ({}%) for {}", score, url);
            if options.ai_seo_affects_score {
                st.add_warning_to_summary("seo-ai-low", &msg);
            } else {
                st.add_info_to_summary("ai-seo-low", &msg);
            }
        }
    }
}

struct SeoOk {
    result: seo::SeoResult,
    prompt_tokens: u64,
    completion_tokens: u64,
    from_cache: bool,
}

enum SeoOutcome {
    Ok(Box<SeoOk>),
    CallError(String),
}

fn build_seo_table(rows: Vec<HashMap<String, String>>) -> SuperTable {
    let console_width = utils::get_console_width() as i32;
    let url_w = 38;
    let score_w = 7;
    let rec_w = ((console_width - url_w - 3 * score_w - 18) / 2).max(20);

    let columns = vec![
        SuperTableColumn::new(
            "urlPathAndQuery".to_string(),
            "URL".to_string(),
            url_w,
            None,
            None,
            true,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "overall".to_string(),
            "AI score".to_string(),
            score_w,
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "titleScore".to_string(),
            "Title".to_string(),
            score_w,
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "descScore".to_string(),
            "Desc".to_string(),
            score_w,
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "recommendedTitle".to_string(),
            "Recommended title".to_string(),
            rec_w,
            None,
            None,
            true,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "recommendedDescription".to_string(),
            "Recommended description".to_string(),
            rec_w,
            None,
            None,
            true,
            false,
            false,
            true,
            None,
        ),
    ];

    let mut table = SuperTable::new(
        AI_SEO_TABLE.to_string(),
        "AI SEO analysis".to_string(),
        "No pages were analyzed by AI.".to_string(),
        columns,
        false,
        Some("overall".to_string()),
        "ASC".to_string(),
        Some("AI-generated SEO assessment and recommended title/description per page.".to_string()),
        None,
        None,
    );
    table.set_visibility_in_console(true, Some(20));
    table.set_data(rows);
    table
}

/// The path and query of `url`, e.g. `/blog/post?page=2`; also the subject of a page's AI requests.
pub(crate) fn url_path_and_query(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let mut s = u.path().to_string();
            if let Some(q) = u.query() {
                s.push('?');
                s.push_str(q);
            }
            s
        }
        Err(_) => url.to_string(),
    }
}

/// Generate llms.txt / llms-full.txt from the selected pages.
async fn run_llms_action(
    options: &CoreOptions,
    client: &Arc<AiClient>,
    pages: &[(RankedPage, PageContext)],
    status: &Arc<Mutex<Status>>,
) {
    let max_tokens = options.ai_max_tokens.clamp(1, 1_000_000) as u32;
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI llms: summarizing {} page(s)...", pages.len()),
            "cyan",
            false
        )
    );

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    // llms.txt and llms-full.txt share one summary per page, so one task tracks both.
    let (task, label) = if options.ai_actions.iter().any(|a| a == "llms-txt") {
        ("llms-txt", "llms.txt")
    } else {
        ("llms-full", "llms-full.txt")
    };
    progress::start(task, label, pages.len() as u64);

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let subject = url_path_and_query(&ctx.url);
        handles.push(tokio::spawn(progress::unit(task, subject, async move {
            let _permit = permit;
            let req = llms_txt::build_summary_request(&ctx, max_tokens, temperature);
            // Retry once on any failure (network, provider, or malformed summary JSON).
            let summary = match client.complete_parsed(&req, CAT_LLMS, llms_txt::parse_summary).await {
                Ok((s, _)) => Some(s),
                Err(e) => {
                    eprintln!(
                        "  {}",
                        utils::get_color_text(
                            &format!("AI llms summary failed for {} (after retry): {}", ctx.url, e),
                            "yellow",
                            false
                        )
                    );
                    None
                }
            };
            (rp, ctx, summary)
        })));
    }

    let mut collected: Vec<(RankedPage, PageContext, llms_txt::PageSummary)> = Vec::new();
    let mut failed = 0usize;
    for h in handles {
        if let Ok((rp, ctx, summary)) = h.await {
            let mut s = summary.unwrap_or_default();
            if s.summary.is_empty() {
                failed += 1;
            }
            if s.name.trim().is_empty() {
                s.name = ctx.title.clone();
            }
            collected.push((rp, ctx, s));
        }
    }
    progress::finish(task);
    if collected.is_empty() {
        return;
    }
    // Restore rank order (tasks complete out of order).
    collected.sort_by(|a, b| b.0.score.partial_cmp(&a.0.score).unwrap_or(std::cmp::Ordering::Equal));

    let (_top_rp, top_ctx, top_summary) = &collected[0];
    let site_name = clean_site_name(&top_ctx.title, &options.get_initial_host(false));
    let site_summary = top_summary.summary.clone();

    let out_dir = llms_output_dir(options);
    if std::fs::create_dir_all(&out_dir).is_err() {
        if let Ok(st) = status.lock() {
            st.add_warning_to_summary("ai-llms-error", &format!("Could not create output dir '{}'", out_dir));
        }
        return;
    }
    let domain = sanitize_domain(&options.get_initial_host(false));
    let base = format!("{}/{}", out_dir.trim_end_matches('/'), domain);

    let actions = &options.ai_actions;
    if actions.iter().any(|a| a == "llms-txt") {
        let entries: Vec<llms_txt::LlmsEntry> = collected.iter().map(|(_, ctx, s)| make_entry(ctx, s)).collect();
        let content = llms_txt::build_llms_txt(&site_name, &site_summary, &entries);
        write_and_report(&format!("{}.llms.txt", base), &content, status, "llms.txt");
    }
    if actions.iter().any(|a| a == "llms-full") {
        let full: Vec<(llms_txt::LlmsEntry, String)> = collected
            .iter()
            .map(|(_, ctx, s)| (make_entry(ctx, s), ctx.content_markdown.clone()))
            .collect();
        let content = llms_txt::build_llms_full(&site_name, &site_summary, &full);
        write_and_report(&format!("{}.llms-full.txt", base), &content, status, "llms-full.txt");
    }

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI llms.txt done: {} page(s) summarized ({} without summary).",
                collected.len(),
                failed
            ),
            "green",
            true
        )
    );
    if failed > 0
        && let Ok(st) = status.lock()
    {
        st.add_notice_to_summary(
            "ai-llms-failures",
            &format!(
                "AI llms.txt: {} page(s) had no summary (call/parse errors); their entries are degraded.",
                failed
            ),
        );
    }
}

/// Derive a concise site name for llms.txt: the brand part of a "Brand - tagline" style
/// homepage title, falling back to the full title, then the host.
fn clean_site_name(title: &str, host: &str) -> String {
    let t = title.trim();
    if t.is_empty() {
        return host.to_string();
    }
    for sep in [" — ", " – ", " - ", " | ", " :: "] {
        if let Some(idx) = t.find(sep) {
            let brand = t[..idx].trim();
            if brand.chars().count() >= 2 {
                return brand.to_string();
            }
        }
    }
    t.to_string()
}

/// True if the URL is the site root (homepage).
fn url_is_homepage(url: &str) -> bool {
    match url::Url::parse(url) {
        Ok(u) => (u.path() == "/" || u.path().is_empty()) && u.query().is_none(),
        Err(_) => false,
    }
}

/// Derive one consistent site name (from the homepage title) for all recommended titles.
fn compute_site_name(options: &CoreOptions, pages: &[(RankedPage, PageContext)]) -> String {
    let host = options.get_initial_host(false);
    let home_title = pages
        .iter()
        .find(|(_, c)| url_is_homepage(&c.url))
        .or_else(|| pages.first())
        .map(|(_, c)| c.title.clone())
        .unwrap_or_default();
    clean_site_name(&home_title, &host)
}

/// Determine the repetitive trailing brand used by page titles. This is deliberately separate from
/// `compute_site_name`, whose prefix-oriented behavior is useful for SEO/llms.txt branding but is
/// wrong for a common `Page | Brand` IA suffix.
fn compute_common_site_suffix(options: &CoreOptions, pages: &[(RankedPage, PageContext)]) -> String {
    let mut counts: std::collections::BTreeMap<String, (String, usize)> = std::collections::BTreeMap::new();
    for (_, context) in pages {
        let title = context.title.trim();
        let candidate = [" | ", " - ", " — ", " – ", " :: ", " · "]
            .iter()
            .filter_map(|separator| {
                title
                    .rfind(separator)
                    .map(|index| (index, title[index + separator.len()..].trim()))
            })
            .filter(|(_, suffix)| suffix.chars().count() >= 2)
            .max_by_key(|(index, _)| *index)
            .map(|(_, suffix)| suffix);
        if let Some(candidate) = candidate {
            let key = candidate.to_lowercase();
            let entry = counts.entry(key).or_insert_with(|| (candidate.to_string(), 0));
            entry.1 += 1;
        }
    }
    // Repetition alone is not enough: a topical qualifier such as "Windows" can legitimately
    // recur across many page titles. Trust a suffix from the actual homepage, or one tied to the
    // host below, instead of deleting an arbitrary repeated segment.
    if let Some((_, homepage)) = pages.iter().find(|(_, context)| url_is_homepage(&context.url)) {
        for separator in [" | ", " - ", " — ", " – ", " :: ", " · "] {
            if let Some(index) = homepage.title.rfind(separator) {
                let suffix = homepage.title[index + separator.len()..].trim();
                if let Some((display, _)) = counts.get(&suffix.to_lowercase()) {
                    return display.clone();
                }
            }
        }
    }

    // A one-page sample cannot establish repetition. It can still identify a multi-word brand
    // suffix when its compact form contains a meaningful host label (e.g. `ABC Finance` on
    // `abc-finance.cz`) without blindly stripping an arbitrary trailing tagline.
    let host_labels = options
        .get_initial_host(false)
        .split('.')
        .map(compact_brand_token)
        .filter(|label| label.len() >= 4 && !matches!(label.as_str(), "info" | "name" | "site" | "online" | "shop"))
        .collect::<Vec<_>>();
    if let Some((display, _)) = counts
        .values()
        .filter(|(display, _)| {
            let compact = compact_brand_token(display);
            host_labels.iter().any(|label| compact.contains(label))
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.len().cmp(&b.0.len())))
    {
        return display.clone();
    }
    compute_site_name(options, pages)
}

fn compact_brand_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn make_entry(ctx: &PageContext, s: &llms_txt::PageSummary) -> llms_txt::LlmsEntry {
    llms_txt::LlmsEntry {
        url: ctx.url.clone(),
        name: s.name.clone(),
        summary: s.summary.clone(),
        section: llms_txt::section_for_url(&ctx.url),
    }
}

fn llms_output_dir(options: &CoreOptions) -> String {
    if let Some(d) = &options.markdown_export_dir
        && !d.is_empty()
    {
        return d.clone();
    }
    if let Some(d) = &options.offline_export_dir
        && !d.is_empty()
    {
        return d.clone();
    }
    "tmp".to_string()
}

fn sanitize_domain(host: &str) -> String {
    let s: String = host
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "site".to_string() } else { s }
}

fn write_and_report(path: &str, content: &str, status: &Arc<Mutex<Status>>, label: &str) {
    match std::fs::write(path, content) {
        Ok(_) => {
            let abs = crate::utils::get_absolute_path(path);
            eprintln!(
                "{}",
                utils::get_color_text(&format!("AI: wrote {} → {}", label, abs), "green", true)
            );
            if let Ok(st) = status.lock() {
                st.add_info_to_summary("ai-llms", &format!("AI generated {} ({})", label, abs));
            }
        }
        Err(e) => {
            eprintln!(
                "{}",
                utils::get_color_text(&format!("AI: failed to write {}: {}", label, e), "red", false)
            );
            if let Ok(st) = status.lock() {
                st.add_warning_to_summary("ai-llms-error", &format!("Failed to write {}: {}", label, e));
            }
        }
    }
}

fn make_text_column(apl_code: &str, name: &str, width: i32, truncate: bool) -> SuperTableColumn {
    SuperTableColumn::new(
        apl_code.to_string(),
        name.to_string(),
        width,
        None,
        None,
        truncate,
        false,
        false,
        true,
        None,
    )
}

async fn run_typos_action(
    options: &CoreOptions,
    client: &Arc<AiClient>,
    pages: &[(RankedPage, PageContext)],
    status: &Arc<Mutex<Status>>,
    output: &Arc<Mutex<Box<dyn Output>>>,
) {
    let (max_tokens, temperature, concurrency) = action_params(options);
    let forced_lang = options.ai_language.clone();

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI typos/grammar: checking {} page(s)...", pages.len()),
            "cyan",
            false
        )
    );

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    progress::start(TASK_TYPOS, "Typos", pages.len() as u64);

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let lang = forced_lang.clone();
        let subject = url_path_and_query(&ctx.url);
        handles.push(tokio::spawn(progress::unit(TASK_TYPOS, subject, async move {
            let _permit = permit;
            let req = typos::build_request(&ctx, lang.as_deref(), max_tokens, temperature);
            // Retry once on any failure; surface a persistent failure instead of silently
            // swallowing it (otherwise the report would show "No content issues found" even when
            // every request failed).
            let res = match client.complete_parsed(&req, CAT_TYPOS, typos::parse).await {
                Ok((r, _)) => Some(r),
                Err(e) => {
                    eprintln!(
                        "  {}",
                        utils::get_color_text(
                            &format!("AI content check failed for {} (after retry): {}", ctx.url, e),
                            "yellow",
                            false
                        )
                    );
                    None
                }
            };
            (rp, ctx, res)
        })));
    }

    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    let mut pages_with_issues = 0usize;
    let mut total_issues = 0usize;
    let mut fail_count = 0usize;
    for h in handles {
        match h.await {
            Ok((_rp, ctx, Some(result))) => {
                if !result.issues.is_empty() {
                    pages_with_issues += 1;
                    for issue in result.issues.iter().take(50) {
                        total_issues += 1;
                        let mut row = HashMap::new();
                        row.insert("urlPathAndQuery".to_string(), url_path_and_query(&ctx.url));
                        row.insert("severity".to_string(), issue.severity.clone());
                        row.insert("kind".to_string(), issue.kind.clone());
                        row.insert("excerpt".to_string(), issue.excerpt.clone());
                        row.insert("suggestion".to_string(), issue.suggestion.clone());
                        rows.push(row);
                    }
                }
            }
            Ok((_rp, _ctx, None)) => fail_count += 1,
            Err(_) => fail_count += 1,
        }
    }
    progress::finish(TASK_TYPOS);

    let fail_note = if fail_count > 0 {
        format!(" ({} page(s) failed)", fail_count)
    } else {
        String::new()
    };
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI content issues done: {} issue(s) across {} page(s){}.",
                total_issues, pages_with_issues, fail_note
            ),
            "green",
            true
        )
    );

    let columns = vec![
        make_text_column("urlPathAndQuery", "URL", 34, true),
        make_text_column("severity", "Severity", 9, false),
        make_text_column("kind", "Type", 10, false),
        make_text_column("excerpt", "Original", 40, true),
        make_text_column("suggestion", "Suggestion", 40, true),
    ];
    let mut table = SuperTable::new(
        "ai-content-issues".to_string(),
        "AI content issues".to_string(),
        "No content issues found by AI.".to_string(),
        columns,
        false,
        None,
        "ASC".to_string(),
        Some("AI-detected spelling, grammar and weak-copy issues (advisory).".to_string()),
        None,
        None,
    );
    table.set_visibility_in_console(true, Some(25));
    table.set_data(rows);
    if let (Ok(st), Ok(mut out)) = (status.lock(), output.lock()) {
        st.configure_super_table_url_stripping(&mut table);
        out.add_super_table(&table);
        st.add_super_table_at_end(table);
    }
    if let Ok(st) = status.lock() {
        if total_issues > 0 {
            st.add_info_to_summary(
                "ai-content-issues",
                &format!(
                    "AI found {} content issue(s) across {} page(s) (advisory).",
                    total_issues, pages_with_issues
                ),
            );
        }
        if fail_count > 0 {
            st.add_notice_to_summary(
                "ai-content-issues-failures",
                &format!(
                    "AI content check could not analyze {} page(s) (call/parse errors) — results may be incomplete.",
                    fail_count
                ),
            );
        }
    }
}

async fn run_custom_action(
    options: &CoreOptions,
    client: &Arc<AiClient>,
    pages: &[(RankedPage, PageContext)],
    status: &Arc<Mutex<Status>>,
    output: &Arc<Mutex<Box<dyn Output>>>,
) {
    // Resolve the user's prompt (inline or file).
    let user_prompt = match resolve_custom_prompt(options) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", e), "red", true));
            if let Ok(st) = status.lock() {
                st.add_critical_to_summary("ai-custom-error", &format!("AI custom action skipped: {}", e));
            }
            return;
        }
    };

    let (max_tokens, temperature, concurrency) = action_params(options);
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI custom check: running on {} page(s)...", pages.len()),
            "cyan",
            false
        )
    );

    let prompt = Arc::new(user_prompt);
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    progress::start(TASK_CUSTOM, "Custom check", pages.len() as u64);

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let prompt = prompt.clone();
        let subject = url_path_and_query(&ctx.url);
        handles.push(tokio::spawn(progress::unit(TASK_CUSTOM, subject, async move {
            let _permit = permit;
            let req = custom::build_request(&prompt, &ctx, max_tokens, temperature);
            // custom::parse is infallible, so this retries once only on a transport/provider error.
            let findings = match client
                .complete_parsed(&req, CAT_CUSTOM, |t| Ok::<_, String>(custom::parse(t)))
                .await
            {
                Ok((findings, _)) => findings,
                Err(e) => vec![custom::CustomFinding {
                    severity: "error".to_string(),
                    label: "call-error".to_string(),
                    message: e.to_string(),
                    location: String::new(),
                }],
            };
            (rp, ctx, findings)
        })));
    }

    let mut rows: Vec<HashMap<String, String>> = Vec::new();
    let mut total = 0usize;
    let mut pages_with_findings = 0usize;
    for h in handles {
        if let Ok((_rp, ctx, findings)) = h.await
            && !findings.is_empty()
        {
            pages_with_findings += 1;
            for f in findings.iter().take(50) {
                total += 1;
                let mut row = HashMap::new();
                row.insert("urlPathAndQuery".to_string(), url_path_and_query(&ctx.url));
                row.insert("severity".to_string(), f.severity.clone());
                row.insert("label".to_string(), f.label.clone());
                row.insert("message".to_string(), f.message.clone());
                row.insert("location".to_string(), f.location.clone());
                rows.push(row);
            }
        }
    }
    progress::finish(TASK_CUSTOM);

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI custom check done: {} finding(s) across {} page(s).",
                total, pages_with_findings
            ),
            "green",
            true
        )
    );

    let columns = vec![
        make_text_column("urlPathAndQuery", "URL", 30, true),
        make_text_column("severity", "Severity", 9, false),
        make_text_column("label", "Label", 16, true),
        make_text_column("message", "Finding", 50, true),
        make_text_column("location", "Location", 24, true),
    ];
    let mut table = SuperTable::new(
        "ai-custom".to_string(),
        "AI custom check".to_string(),
        "No findings from the AI custom check.".to_string(),
        columns,
        false,
        None,
        "ASC".to_string(),
        Some("Findings from your custom AI prompt (advisory).".to_string()),
        None,
        None,
    );
    table.set_visibility_in_console(true, Some(25));
    table.set_data(rows);
    if let (Ok(st), Ok(mut out)) = (status.lock(), output.lock()) {
        st.configure_super_table_url_stripping(&mut table);
        out.add_super_table(&table);
        st.add_super_table_at_end(table);
    }
    if let Ok(st) = status.lock()
        && total > 0
    {
        st.add_info_to_summary(
            "ai-custom",
            &format!(
                "AI custom check produced {} finding(s) across {} page(s).",
                total, pages_with_findings
            ),
        );
    }
}

fn resolve_custom_prompt(options: &CoreOptions) -> Result<String, String> {
    if let Some(ref inline) = options.ai_prompt
        && !inline.trim().is_empty()
    {
        return Ok(inline.clone());
    }
    if let Some(ref path) = options.ai_prompt_file {
        return std::fs::read_to_string(path).map_err(|e| format!("cannot read --ai-prompt-file '{}': {}", path, e));
    }
    Err("custom action requires --ai-prompt or --ai-prompt-file".to_string())
}
