// SiteOne Crawler - AI runner (post-crawl AI phase)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Orchestrates the optional AI phase: resolve config/key, select & rank pages, run the
// requested actions concurrently (own pool + rate limiter), then render results into the
// report (tables + summary). All network I/O happens without holding any Mutex.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use super::actions::{custom, llms_txt, seo, typos};
use super::client::AiClient;
use super::config::{AiConfig, resolve_api_key};
use super::page::PageContext;
use super::provider::Provider;
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

/// Entry point for the post-crawl AI phase. Fail-soft: never panics, never aborts the crawl.
pub async fn run_ai(options: &CoreOptions, status: &Arc<Mutex<Status>>, output: &Arc<Mutex<Box<dyn Output>>>) {
    // --- Phase A: select pages and extract their context (under a short read lock) ---
    let (selection_summary, pages) = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let include: Vec<String> = options.ai_include.clone();
        let exclude: Vec<String> = options.ai_exclude.clone();
        let sel = select_pages(&st, &include, &exclude, options.ai_max_pages.max(1) as usize);

        let mut pages: Vec<(RankedPage, PageContext)> = Vec::new();
        for rp in &sel.selected {
            if let Some(ctx) = PageContext::build(&st, &rp.uq_id, &rp.url) {
                pages.push((rp.clone(), ctx));
            }
        }
        let summary = SelectionSummary {
            selected: sel.selected.len(),
            total_html: sel.total_html_pages,
            candidates: sel.total_candidates_before_cap,
            excluded: sel.excluded_by_mask,
        };
        (summary, pages)
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
            + (a.iter().any(|x| x == "llms-txt" || x == "llms-full") as usize)
    };
    let total_calls = pages.len() * calls_per_page;
    eprintln!(
        "  ({} HTML pages crawled, {} excluded by masks, {} candidate(s), capped to --ai-max-pages={}) → ~{} LLM call(s)",
        selection_summary.total_html,
        selection_summary.excluded,
        selection_summary.candidates,
        options.ai_max_pages,
        total_calls
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
                    "AI dry-run: would make ~{} LLM call(s) over {} page(s), est. input ~{} tokens. No API calls made.",
                    total_calls,
                    pages.len(),
                    est_input_tokens
                ),
                "yellow",
                true
            )
        );
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

    if pages.is_empty() {
        if let Ok(st) = status.lock() {
            st.add_notice_to_summary("ai-no-pages", "AI enabled but no pages matched the selection criteria.");
        }
        return;
    }

    // --- Build the client (resolve the API key here, never stored in CoreOptions) ---
    let api_key = match resolve_api_key(
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
    };
    // Hosted providers require a key; openai-compatible (e.g. local vLLM) may not.
    if api_key.is_none() && provider != Provider::OpenAiCompatible {
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
}

/// Shared concurrency settings for an action.
fn action_params(options: &CoreOptions) -> (u32, f32, usize, Option<Duration>) {
    let max_tokens = options.ai_max_tokens.clamp(1, 1_000_000) as u32;
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;
    let delay = options
        .ai_max_reqs_per_sec
        .filter(|r| *r > 0.0)
        .map(|r| Duration::from_secs_f64(1.0 / r));
    (max_tokens, temperature, concurrency, delay)
}

struct SelectionSummary {
    selected: usize,
    total_html: usize,
    candidates: usize,
    excluded: usize,
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
    let delay = options
        .ai_max_reqs_per_sec
        .filter(|r| *r > 0.0)
        .map(|r| Duration::from_secs_f64(1.0 / r));

    // A single consistent site name (from the homepage) used in all recommended titles.
    let site_name = compute_site_name(options, pages);

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    let mut next_slot = Instant::now();

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if let Some(d) = delay {
            let now = Instant::now();
            if next_slot > now {
                tokio::time::sleep(next_slot - now).await;
            }
            next_slot = Instant::now() + d;
        }
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let site_name = site_name.clone();
        let is_homepage = url_is_homepage(&ctx.url);
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let req = seo::build_request(&ctx, &site_name, is_homepage, max_tokens, temperature);
            let outcome = match client.complete(&req, CAT_SEO).await {
                Ok(completion) => match seo::parse(&completion.text) {
                    Ok(result) => SeoOutcome::Ok(Box::new(SeoOk {
                        result,
                        prompt_tokens: completion.usage.prompt_tokens,
                        completion_tokens: completion.usage.completion_tokens,
                        from_cache: completion.from_cache,
                    })),
                    Err(e) => SeoOutcome::ParseError(e),
                },
                Err(e) => SeoOutcome::CallError(e.to_string()),
            };
            (rp, ctx, outcome)
        }));
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
                prompt_tokens_total += prompt_tokens as u64;
                completion_tokens_total += completion_tokens as u64;
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
            SeoOutcome::ParseError(e) => {
                fail_count += 1;
                eprintln!(
                    "  {}",
                    utils::get_color_text(&format!("AI SEO parse error for {}: {}", ctx.url, e), "yellow", false)
                );
            }
            SeoOutcome::CallError(e) => {
                fail_count += 1;
                eprintln!(
                    "  {}",
                    utils::get_color_text(&format!("AI SEO call error for {}: {}", ctx.url, e), "yellow", false)
                );
            }
        }
    }

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
    prompt_tokens: u32,
    completion_tokens: u32,
    from_cache: bool,
}

enum SeoOutcome {
    Ok(Box<SeoOk>),
    ParseError(String),
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

fn url_path_and_query(url: &str) -> String {
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
    let delay = options
        .ai_max_reqs_per_sec
        .filter(|r| *r > 0.0)
        .map(|r| Duration::from_secs_f64(1.0 / r));

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
    let mut next_slot = Instant::now();

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if let Some(d) = delay {
            let now = Instant::now();
            if next_slot > now {
                tokio::time::sleep(next_slot - now).await;
            }
            next_slot = Instant::now() + d;
        }
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let req = llms_txt::build_summary_request(&ctx, max_tokens, temperature);
            let summary = match client.complete(&req, CAT_LLMS).await {
                Ok(c) => llms_txt::parse_summary(&c.text).ok(),
                Err(e) => {
                    eprintln!(
                        "  {}",
                        utils::get_color_text(
                            &format!("AI llms summary call error for {}: {}", ctx.url, e),
                            "yellow",
                            false
                        )
                    );
                    None
                }
            };
            (rp, ctx, summary)
        }));
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
    let (max_tokens, temperature, concurrency, delay) = action_params(options);
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
    let mut next_slot = Instant::now();

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if let Some(d) = delay {
            let now = Instant::now();
            if next_slot > now {
                tokio::time::sleep(next_slot - now).await;
            }
            next_slot = Instant::now() + d;
        }
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let lang = forced_lang.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let req = typos::build_request(&ctx, lang.as_deref(), max_tokens, temperature);
            // Surface call/parse failures instead of silently swallowing them (otherwise the
            // report would show "No content issues found" even when every request failed).
            let res = match client.complete(&req, CAT_TYPOS).await {
                Ok(c) => match typos::parse(&c.text) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        eprintln!(
                            "  {}",
                            utils::get_color_text(
                                &format!("AI content check parse error for {}: {}", ctx.url, e),
                                "yellow",
                                false
                            )
                        );
                        None
                    }
                },
                Err(e) => {
                    eprintln!(
                        "  {}",
                        utils::get_color_text(
                            &format!("AI content check call error for {}: {}", ctx.url, e),
                            "yellow",
                            false
                        )
                    );
                    None
                }
            };
            (rp, ctx, res)
        }));
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

    let (max_tokens, temperature, concurrency, delay) = action_params(options);
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
    let mut next_slot = Instant::now();

    for (rp, ctx) in pages.iter() {
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if let Some(d) = delay {
            let now = Instant::now();
            if next_slot > now {
                tokio::time::sleep(next_slot - now).await;
            }
            next_slot = Instant::now() + d;
        }
        let client = client.clone();
        let rp = rp.clone();
        let ctx = ctx.clone();
        let prompt = prompt.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let req = custom::build_request(&prompt, &ctx, max_tokens, temperature);
            let findings = match client.complete(&req, CAT_CUSTOM).await {
                Ok(c) => custom::parse(&c.text),
                Err(e) => vec![custom::CustomFinding {
                    severity: "error".to_string(),
                    label: "call-error".to_string(),
                    message: e.to_string(),
                    location: String::new(),
                }],
            };
            (rp, ctx, findings)
        }));
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
