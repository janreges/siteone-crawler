// SiteOne Crawler - AI profile pipeline (--ai-profile)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Chapter-based subject-profile generator. See docs/superpowers/specs/2026-07-21-ai-profile-design.md.
// Independent of `src/ai/elaborate/` (which is left untouched).

pub mod budget;
pub mod classify;
pub mod correct;
pub mod describe;
pub mod doc;
pub mod embedded;
pub mod promptpack;
pub mod registry;
pub mod select;
pub mod summarize;
pub mod synthesize;

/// One page in the profile's working universe. `description` is filled by the describe phase (P4);
/// `size_chars` is the char length of the page's cleaned markdown (used to fit selection budgets).
#[derive(Debug, Clone)]
pub struct ProfilePage {
    pub uq_id: String,
    pub url: String,
    pub path: String,
    pub description: String,
    pub size_chars: usize,
}

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use crate::ai::client::AiClient;
use crate::ai::config::build_config;
use crate::ai::page::PageContext;
use crate::ai::provider::{ChatMessage, ChatRequest};
use crate::ai::report::locale::ReportLocale;
use crate::ai::selection::build_candidates;
use crate::options::core_options::CoreOptions;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

use self::budget::ContextBudget;
use self::correct::CorrectionReport;
use self::doc::{ProfileChapter, ProfileDoc, ProfileMeta, ProfilePageInfo};
use self::promptpack::ChapterSpec;

const CAT_SUMMARY: &str = "AI profile (summary)";
const CAT_CLASSIFY: &str = "AI profile (classify)";
const CAT_DESCRIBE: &str = "AI profile (describe)";
const CAT_SELECT: &str = "AI profile (select)";
const CAT_SYNTH: &str = "AI profile (synthesis)";
const CAT_CORRECT: &str = "AI profile (correction)";
const CAT_LOCALIZE: &str = "AI profile (localize)";
const PARSE_ATTEMPTS: u32 = 2;

/// The path portion of a URL, used as a short human-readable page label.
fn path_hint(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .map(|u| {
            let seg = u.path().trim_matches('/').to_string();
            if seg.is_empty() {
                "/".to_string()
            } else {
                format!("/{seg}")
            }
        })
        .unwrap_or_else(|| url.to_string())
}

/// Derive a display subject name from the homepage title (leading segment before a separator), else
/// the host.
fn subject_name_from(title: &str, host: &str) -> String {
    let t = title.trim();
    if t.is_empty() {
        return host.to_string();
    }
    let cut = t.split(['|', '—', '–', '-', '·']).next().unwrap_or(t).trim();
    if cut.is_empty() {
        host.to_string()
    } else {
        cut.to_string()
    }
}

fn report_error(status: &Arc<Mutex<Status>>, msg: &str) {
    eprintln!("{}", utils::get_color_text(&format!("ERROR: {}", msg), "red", true));
    if let Ok(st) = status.lock() {
        st.add_critical_to_summary("ai-profile-error", msg);
    }
}

/// Entry point for `--ai-profile`. Fail-soft: never panics, never aborts the crawl.
pub async fn run(options: &CoreOptions, status: &Arc<Mutex<Status>>, output: &Arc<Mutex<Box<dyn Output>>>) {
    let _ = output;
    let run_start = std::time::Instant::now();
    // Own the usage ledger only when running truly standalone: `run_ai` already reset it if actions
    // ran, and `--ai-elaborate` (dispatched before us) already reset + recorded into it — resetting
    // again here would discard elaborate's tokens from the combined end-of-run cost summary.
    if options.ai_actions.is_empty() && !options.ai_elaborate {
        crate::ai::usage::reset();
    }
    crate::ai::usage::note_model(&options.ai_model.clone().unwrap_or_default());
    crate::ai::client::reset_rate_limiter().await;

    let host = options.get_initial_host(false);
    let locale = ReportLocale::new(&options.ai_report_language);
    let lang = locale.code().to_string();
    let temperature = options.ai_temperature as f32;
    let concurrency = options.ai_max_concurrency.clamp(1, 64) as usize;
    let page_budget = options.ai_max_pages.max(1) as usize;
    let budget = ContextBudget::new(options.ai_context_window, options.ai_max_tokens);
    let out_tokens = budget.out_tokens();

    // --- P1: candidate universe → ordered ProfilePage set with cleaned markdown (one status lock) ---
    let (mut pages, markdowns, meta_fallback, homepage_title) = {
        let st = match status.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let cs = build_candidates(&st, &options.ai_include, &options.ai_exclude);
        let mut pages: Vec<ProfilePage> = Vec::new();
        let mut markdowns: Vec<String> = Vec::new();
        let mut meta_fallback: Vec<(String, String)> = Vec::new();
        let mut homepage_title = String::new();
        for (i, c) in cs.candidates.iter().take(page_budget).enumerate() {
            let Some(ctx) = PageContext::build(&st, &c.uq_id, &c.url, options) else {
                continue;
            };
            if i == 0 {
                homepage_title = ctx.title.clone();
            }
            let md = ctx.content_markdown.clone();
            pages.push(ProfilePage {
                uq_id: c.uq_id.clone(),
                url: c.url.clone(),
                path: path_hint(&c.url),
                description: String::new(),
                size_chars: md.chars().count(),
            });
            meta_fallback.push((ctx.title.clone(), ctx.meta_description.clone()));
            markdowns.push(md);
        }
        (pages, markdowns, meta_fallback, homepage_title)
    };

    if pages.is_empty() {
        if let Ok(st) = status.lock() {
            st.add_notice_to_summary("ai-profile", "AI profile: no eligible pages were crawled.");
        }
        return;
    }
    let subject_name = subject_name_from(&homepage_title, &host);

    // --- Resolve the forced template (if any) up front for dry-run + skip-classify. ---
    let forced = options
        .ai_profile_template
        .as_deref()
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty() && t != "auto");

    // --- Dry run: print the plan, no API calls. ---
    if options.ai_dry_run {
        let type_key = forced
            .clone()
            .unwrap_or_else(|| "auto (detected at run time)".to_string());
        let est_chapters = forced
            .as_deref()
            .and_then(registry::type_by_key)
            .map(|t| t.chapters.len())
            .unwrap_or(12);
        let corr = if options.ai_profile_correct { 1 } else { 0 };
        let classify = if forced.is_some() { 0 } else { 1 };
        // Every non-English language (Czech included) makes one heading-localization call.
        let localize = if locale.code() == "en" { 0 } else { 1 };
        let est_calls = 1 + classify + pages.len() + est_chapters * (2 + corr) + localize + (1 + corr);
        eprintln!(
            "{}",
            utils::get_color_text(
                &format!(
                    "AI profile dry-run: template={}, {} page(s), ctx={} tok → site-summary budget {} KB, chapter budget {} KB; ~{} LLM call(s). No API calls made.",
                    type_key,
                    pages.len(),
                    budget.context_tokens(),
                    budget.site_summary_input() / 1024,
                    budget.chapter_material() / 1024,
                    est_calls
                ),
                "yellow",
                true
            )
        );
        for (i, p) in pages.iter().take(30).enumerate() {
            eprintln!("  {:>3}. {}", i + 1, p.url);
        }
        if let Ok(st) = status.lock() {
            st.add_info_to_summary(
                "ai-profile-dry-run",
                &format!("AI profile dry-run: {} page(s) would be analyzed.", pages.len()),
            );
        }
        return;
    }

    // --- Build the AI client. ---
    let config = match build_config(options) {
        Ok(c) => c,
        Err(e) => {
            report_error(status, &format!("AI profile skipped: {}", e));
            return;
        }
    };
    let provider = config.provider;
    let model_name = config.model.clone();
    let client = Arc::new(AiClient::new(config));
    let mut calls = 0usize;

    eprintln!(
        "\n{}",
        utils::get_color_text(
            &format!(
                "AI profile: {} page(s) using {} / {} (ctx {} tok)",
                pages.len(),
                provider.as_str(),
                model_name,
                budget.context_tokens()
            ),
            "cyan",
            true
        )
    );

    // --- P2: site summary. ---
    let summary_pages: Vec<(String, String)> = pages
        .iter()
        .zip(markdowns.iter())
        .map(|(p, m)| (p.url.clone(), m.clone()))
        .collect();
    let summary_input = summarize::build_input(&summary_pages, budget.site_summary_input());
    let site_description = {
        let req = summarize::build_request(&summary_input, &lang, out_tokens, temperature);
        calls += 1;
        match client.complete(&req, CAT_SUMMARY).await {
            Ok(c) => summarize::parse(&c.text),
            Err(e) => {
                eprintln!(
                    "{}",
                    utils::get_color_text(
                        &format!("  summary failed: {e}; using homepage title/meta"),
                        "yellow",
                        false
                    )
                );
                let (t, m) = meta_fallback.first().cloned().unwrap_or_default();
                describe::fallback(&t, &m)
            }
        }
    };
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI profile: site summary → {} chars", site_description.chars().count()),
            "cyan",
            false
        )
    );

    // --- P3: classify (unless forced). ---
    let (template_key, template_detected) = if let Some(key) = forced.clone() {
        (key, false)
    } else {
        let req = classify::build_request(&host, &site_description, out_tokens);
        calls += 1;
        match client
            .complete_parsed_n(&req, CAT_CLASSIFY, PARSE_ATTEMPTS, classify::parse)
            .await
        {
            Ok((id, _)) => (classify::key_for(id).unwrap_or("general").to_string(), true),
            Err(e) => {
                eprintln!(
                    "{}",
                    utils::get_color_text(
                        &format!("  classification failed: {e}; using 'general'"),
                        "yellow",
                        false
                    )
                );
                if let Ok(st) = status.lock() {
                    st.add_warning_to_summary(
                        "ai-profile-classify",
                        "AI profile: classification failed; used the general template.",
                    );
                }
                ("general".to_string(), true)
            }
        }
    };
    let type_spec = registry::type_by_key(&template_key)
        .unwrap_or_else(|| registry::type_by_key("general").expect("general type always present"));
    let template_name = if locale.is_czech() {
        type_spec.name_cs.clone()
    } else {
        type_spec.name_en.clone()
    };
    eprintln!(
        "{}",
        utils::get_color_text(
            &format!("AI profile: type = {} ({})", type_spec.key, template_name),
            "cyan",
            true
        )
    );

    // --- P4: page descriptions (fan-out). ---
    let sem = Arc::new(Semaphore::new(concurrency));
    let markdowns = Arc::new(markdowns);
    {
        let mut handles = Vec::new();
        for (i, page) in pages.iter().enumerate() {
            let permit_sem = sem.clone();
            let client = client.clone();
            let markdowns = markdowns.clone();
            let url = page.url.clone();
            let lang = lang.clone();
            let cap = budget.describe_input();
            handles.push(tokio::spawn(async move {
                let _permit = permit_sem.acquire_owned().await.ok();
                let md = crate::ai::prompt::truncate_chars(&summarize::strip_md_links(&markdowns[i]), cap);
                let req = describe::build_request(&md, &url, &lang, out_tokens);
                let result = client
                    .complete_parsed_n(&req, CAT_DESCRIBE, PARSE_ATTEMPTS, describe::parse)
                    .await;
                (i, result.ok().map(|(d, _)| d))
            }));
        }
        let mut fallbacks = 0usize;
        for h in handles {
            if let Ok((i, desc)) = h.await {
                calls += 1;
                pages[i].description = desc.unwrap_or_else(|| {
                    fallbacks += 1;
                    let (t, m) = meta_fallback.get(i).cloned().unwrap_or_default();
                    describe::fallback(&t, &m)
                });
            }
        }
        eprintln!(
            "{}",
            utils::get_color_text(
                &format!(
                    "AI profile: page descriptions: {} page(s) ({} fallback)",
                    pages.len(),
                    fallbacks
                ),
                "cyan",
                false
            )
        );
    }

    // --- Heading localization (one batched call for every non-English locale, Czech included). ---
    let localized_headings = localize_headings(&client, type_spec, &lang, &locale, out_tokens, &mut calls).await;

    // --- P5: chapters (parallel sub-pipelines). ---
    let pages_arc = Arc::new(pages);
    let numbered = Arc::new(select::render_numbered(&pages_arc));
    let site_desc_arc = Arc::new(site_description.clone());
    let shared = Arc::new(ChapterShared {
        client: client.clone(),
        sem: sem.clone(),
        pages: pages_arc.clone(),
        markdowns: markdowns.clone(),
        numbered: numbered.clone(),
        site_description: site_desc_arc.clone(),
        lang: lang.clone(),
        out_tokens,
        temperature,
        select_budget: budget.chapter_material(),
        correct_input: budget.correction_input(),
        correct: options.ai_profile_correct,
    });

    let mut handles = Vec::new();
    for chapter in type_spec.chapters.iter().cloned() {
        let shared = shared.clone();
        let heading = localized_headings
            .get(&chapter.id)
            .cloned()
            .unwrap_or_else(|| chapter.heading.clone());
        let page_cap = budget.page_cap(chapter.max_pages, 3);
        handles.push(tokio::spawn(async move {
            build_chapter(&shared, chapter, heading, page_cap).await
        }));
    }
    let mut chapters: Vec<ProfileChapter> = Vec::new();
    for h in handles {
        if let Ok(ch) = h.await {
            chapters.push(ch);
        }
    }
    // Restore template order (join order is nondeterministic).
    let order: BTreeMap<&str, usize> = type_spec
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    chapters.sort_by_key(|c| *order.get(c.id.as_str()).unwrap_or(&usize::MAX));
    // Each non-omitted chapter made a select + synth (+ correction) call; omitted-at-select made one.
    for c in &chapters {
        calls += if c.omitted {
            1
        } else {
            2 + usize::from(c.correction.is_some())
        };
    }

    // --- P6: executive summary from the visible chapters. ---
    let exec_summary = build_exec_summary(
        &client,
        &chapters,
        &site_description,
        &lang,
        out_tokens,
        temperature,
        budget.exec_input(),
        options.ai_profile_correct,
        budget.correction_input(),
    )
    .await;

    // --- Assemble + store. ---
    let visible = chapters.iter().filter(|c| !c.omitted).count();
    let title = format!(
        "{} — {}",
        subject_name,
        if locale.is_czech() {
            "profil subjektu"
        } else {
            "subject profile"
        }
    );
    // Real LLM accounting for this profile, summed from the usage ledger's "AI profile (*)"
    // categories (accurate even when combined with other AI actions in one process).
    let (mut ledger_calls, mut input_tokens, mut output_tokens, mut llm_ms) = (0u64, 0u64, 0u64, 0u64);
    for (name, u) in crate::ai::usage::categories() {
        if name.starts_with("AI profile") {
            ledger_calls += u.calls;
            input_tokens += u.prompt_tokens;
            output_tokens += u.completion_tokens;
            llm_ms += u.network_time_ms;
        }
    }
    let _ = calls; // superseded by the authoritative ledger count below

    let doc = ProfileDoc {
        title,
        subject_name,
        meta: ProfileMeta {
            host: host.clone(),
            url: options.url.clone(),
            crawled_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            provider: provider.as_str().to_string(),
            model: model_name,
            report_language: lang.clone(),
            context_window: budget.context_tokens(),
            template_key: type_spec.key.clone(),
            template_name,
            template_detected,
            pages_total: pages_arc.len(),
            pages_described: pages_arc.len(),
            calls: ledger_calls as usize,
            input_tokens,
            output_tokens,
            llm_ms,
            gen_ms: run_start.elapsed().as_millis() as u64,
            pages_failed: Vec::new(),
        },
        site_description,
        exec_summary,
        pages: pages_arc
            .iter()
            .map(|p| ProfilePageInfo {
                path: p.path.clone(),
                url: p.url.clone(),
                description: p.description.clone(),
                size_chars: p.size_chars,
            })
            .collect(),
        chapters,
    };

    eprintln!(
        "{}",
        utils::get_color_text(
            &format!(
                "AI profile '{}' done: {}/{} chapter(s).",
                type_spec.key,
                visible,
                type_spec.chapters.len()
            ),
            if visible == 0 { "yellow" } else { "green" },
            true
        )
    );
    if let Ok(st) = status.lock() {
        if visible == 0 {
            st.add_critical_to_summary(
                "ai-profile-empty",
                "AI profile: every chapter was empty; no document produced.",
            );
            return;
        }
        st.add_info_to_summary(
            "ai-profile",
            &format!(
                "AI profile '{}' built with {} chapter(s) (MD + JSON + HTML).",
                type_spec.key, visible
            ),
        );
        st.set_ai_profile_doc(doc);
    }
}

struct ChapterShared {
    client: Arc<AiClient>,
    sem: Arc<Semaphore>,
    pages: Arc<Vec<ProfilePage>>,
    markdowns: Arc<Vec<String>>,
    numbered: Arc<String>,
    site_description: Arc<String>,
    lang: String,
    out_tokens: u32,
    temperature: f32,
    select_budget: usize,
    correct_input: usize,
    correct: bool,
}

/// Run one chapter's select → synthesize → correct sub-pipeline. Returns an omitted chapter when the
/// pages hold nothing relevant. Each LLM call is bounded by the shared semaphore.
async fn build_chapter(
    shared: &ChapterShared,
    chapter: ChapterSpec,
    heading: String,
    page_cap: usize,
) -> ProfileChapter {
    let omitted = |reason: &str| ProfileChapter {
        id: chapter.id.clone(),
        heading: heading.clone(),
        markdown: String::new(),
        sources: Vec::new(),
        target_chars: chapter.target_chars,
        correction: None,
        omitted: true,
        omit_reason: reason.to_string(),
    };

    // P5a: select page indices.
    let select_system = promptpack::render(
        registry::SELECT_PAGES,
        &[
            ("site_description", shared.site_description.as_str()),
            ("chapter_heading", &heading),
            ("chapter_focus", &chapter.selection_focus),
            ("max_pages", &page_cap.to_string()),
            ("budget_chars", &shared.select_budget.to_string()),
        ],
    );
    let req = select::build_request(&select_system, &shared.numbered, shared.out_tokens, shared.temperature);
    let ids = {
        let _permit = shared.sem.clone().acquire_owned().await.ok();
        match shared
            .client
            .complete_parsed_n(&req, CAT_SELECT, PARSE_ATTEMPTS, select::parse_ids)
            .await
        {
            Ok((ids, _)) => select::enforce(&ids, &shared.pages, page_cap, shared.select_budget),
            Err(_) => {
                // Deterministic fallback: top-ranked pages that fit the budget.
                let mut idx: Vec<usize> = (0..shared.pages.len()).collect();
                select::enforce(
                    &idx.drain(..).map(|i| i + 1).collect::<Vec<_>>(),
                    &shared.pages,
                    page_cap,
                    shared.select_budget,
                )
            }
        }
    };
    if ids.is_empty() {
        return omitted("no relevant pages selected");
    }

    // P5b: synthesize.
    let blocks: Vec<(String, String)> = ids
        .iter()
        .map(|&i| (shared.pages[i].path.clone(), shared.markdowns[i].clone()))
        .collect();
    let sources: Vec<String> = ids.iter().map(|&i| shared.pages[i].url.clone()).collect();
    let content = synthesize::build_content(&blocks, shared.select_budget);
    let req = synthesize::build_request(
        &chapter,
        &shared.site_description,
        &content,
        &shared.lang,
        shared.out_tokens,
        shared.temperature,
    );
    let body = {
        let _permit = shared.sem.clone().acquire_owned().await.ok();
        match shared.client.complete_with(&req, None, CAT_SYNTH).await {
            Ok(c) => synthesize::parse(&c.text, &heading),
            Err(_) => String::new(),
        }
    };
    if body.trim().is_empty() {
        return omitted("synthesis produced no content");
    }

    // P5c: correction.
    let (body, correction) = if shared.correct {
        let (corrected, report) = run_correction(shared, &chapter.synthesis_instructions, &content, &body).await;
        // The correction deletes spans and can leave debris (empty bold, blank runs, stray
        // punctuation) or a leaked meta-comment — re-run the deterministic cleanup over its output.
        (synthesize::clean_markdown(&corrected), report)
    } else {
        (body, None)
    };
    if body.trim().is_empty() {
        return omitted("chapter empty after correction cleanup");
    }

    ProfileChapter {
        id: chapter.id.clone(),
        heading,
        markdown: body,
        sources,
        target_chars: chapter.target_chars,
        correction,
        omitted: false,
        omit_reason: String::new(),
    }
}

/// Correction pass over a produced text (chapter body or exec summary). Fail-soft: on any error the
/// uncorrected text is returned.
async fn run_correction(
    shared: &ChapterShared,
    task_instructions: &str,
    source_material: &str,
    output: &str,
) -> (String, Option<CorrectionReport>) {
    let system = promptpack::render(registry::CORRECT, &[("language", shared.lang.as_str())]);
    let user = format!(
        "<original_task>\n{}\n</original_task>\n<source_pages>\n{}\n</source_pages>\n<output>\n{}\n</output>",
        crate::ai::prompt::truncate_chars(task_instructions, 4_000),
        crate::ai::prompt::truncate_chars(source_material, shared.correct_input),
        crate::ai::prompt::truncate_chars(output, shared.correct_input),
    );
    let req = ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(user)],
        max_tokens: shared.out_tokens,
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    let _permit = shared.sem.clone().acquire_owned().await.ok();
    match shared
        .client
        .complete_parsed_n(&req, CAT_CORRECT, PARSE_ATTEMPTS, correct::parse_edits)
        .await
    {
        Ok((edits, _)) => {
            let (new_text, report) = correct::apply(output, &edits);
            (new_text, Some(report))
        }
        Err(_) => (output.to_string(), None),
    }
}

/// One batched heading-translation call for every non-English report language (including Czech): the
/// chapter headings authored in English are translated into `lang` in a single id-constrained call,
/// with an English fallback on failure. English (`code() == "en"`) skips the call and keeps the
/// English headings.
async fn localize_headings(
    client: &Arc<AiClient>,
    type_spec: &promptpack::TypeSpec,
    lang: &str,
    locale: &ReportLocale,
    out_tokens: u32,
    calls: &mut usize,
) -> BTreeMap<String, String> {
    if locale.code() == "en" {
        return BTreeMap::new();
    }
    let map: BTreeMap<&str, &str> = type_spec
        .chapters
        .iter()
        .map(|c| (c.id.as_str(), c.heading.as_str()))
        .collect();
    let input = serde_json::to_string(&map).unwrap_or_default();
    let system = promptpack::render(registry::LOCALIZE_HEADINGS, &[("language", lang)]);
    let req = ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(input)],
        max_tokens: out_tokens.min(2_000),
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    *calls += 1;
    let parse = |raw: &str| -> Result<BTreeMap<String, String>, String> {
        let normalized = crate::ai::normalize::normalize_json_response(raw);
        serde_json::from_str::<BTreeMap<String, String>>(&normalized)
            .or_else(|_| serde_json::from_str(&crate::ai::normalize::repair_json(raw)))
            .map_err(|e| e.to_string())
    };
    match client
        .complete_parsed_n(&req, CAT_LOCALIZE, PARSE_ATTEMPTS, parse)
        .await
    {
        Ok((m, _)) => m.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect(),
        Err(_) => BTreeMap::new(),
    }
}

/// Build the executive summary from the visible chapters, with an optional correction pass.
#[allow(clippy::too_many_arguments)]
async fn build_exec_summary(
    client: &Arc<AiClient>,
    chapters: &[ProfileChapter],
    site_description: &str,
    lang: &str,
    out_tokens: u32,
    temperature: f32,
    input_budget: usize,
    correct: bool,
    correct_input: usize,
) -> String {
    let mut material = String::new();
    for c in chapters.iter().filter(|c| !c.omitted && !c.markdown.trim().is_empty()) {
        let block = format!("## {}\n{}\n\n", c.heading, c.markdown);
        if !material.is_empty() && material.len() + block.len() > input_budget {
            break;
        }
        material.push_str(&block);
    }
    if material.trim().is_empty() {
        return String::new();
    }
    let system = promptpack::render(
        registry::EXEC_SUMMARY,
        &[("language", lang), ("site_description", site_description)],
    );
    let req = ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(material.clone())],
        max_tokens: out_tokens,
        temperature,
        json_mode: false,
        json_schema: None,
        schema_name: None,
    };
    let body = match client.complete_with(&req, None, CAT_SYNTH).await {
        Ok(c) => crate::ai::normalize::strip_think(&c.text).trim().to_string(),
        Err(_) => return String::new(),
    };
    if !correct || body.is_empty() {
        return synthesize::clean_markdown(&body);
    }
    // Reuse the correction prompt with the exec-summary material as source.
    let system = promptpack::render(registry::CORRECT, &[("language", lang)]);
    let user = format!(
        "<original_task>\nWrite an executive summary of the profile.\n</original_task>\n<source_pages>\n{}\n</source_pages>\n<output>\n{}\n</output>",
        crate::ai::prompt::truncate_chars(&material, correct_input),
        crate::ai::prompt::truncate_chars(&body, correct_input),
    );
    let req = ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(user)],
        max_tokens: out_tokens,
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    };
    match client
        .complete_parsed_n(&req, CAT_CORRECT, PARSE_ATTEMPTS, correct::parse_edits)
        .await
    {
        Ok((edits, _)) => synthesize::clean_markdown(&correct::apply(&body, &edits).0),
        Err(_) => synthesize::clean_markdown(&body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_hint_extracts_path() {
        assert_eq!(path_hint("https://x.cz/o-nas/tym"), "/o-nas/tym");
        assert_eq!(path_hint("https://x.cz/"), "/");
    }

    #[test]
    fn subject_name_prefers_title_segment_then_host() {
        assert_eq!(subject_name_from("Acme s.r.o. | Domů", "acme.cz"), "Acme s.r.o.");
        assert_eq!(subject_name_from("", "acme.cz"), "acme.cz");
    }
}
