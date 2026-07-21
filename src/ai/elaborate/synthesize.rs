// SiteOne Crawler - brand elaborate: synthesis planning & prompt
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The synthesis LLM writes ONLY the prose sections of the fixed skeleton (the crawler assembles the
// structured lists from the merged model). Because the model is deduped, the material is small
// enough for a single call on typical brands; when the ESTIMATED OUTPUT would exceed the model's
// completion cap (the real binding constraint — many 128K models cap output at 8-16K tokens) or the
// material is very large, synthesis falls back to a sectioned map-reduce (one call per prose section
// fed only role-relevant essences).

use std::collections::BTreeMap;

use crate::ai::normalize::{normalize_json_response, repair_json};

use super::extract::PageEssence;
use super::merge::BrandModel;
use super::templates::{SectionKind, SectionSpec, prose_ids};

/// Chars-per-token estimate: Czech/Slovak/Polish diacritics tokenize poorly (~2.7), most other
/// languages (English etc.) ~3.4. Deliberately conservative (over-estimates tokens).
pub fn estimate_tokens(text: &str, lang_hint: Option<&str>) -> usize {
    let dense = matches!(
        lang_hint
            .map(|l| l.split('-').next().unwrap_or(l).to_ascii_lowercase())
            .as_deref(),
        Some("cs") | Some("sk") | Some("pl") | Some("cz")
    );
    let per_tok = if dense { 2.7 } else { 3.4 };
    ((text.chars().count() as f64) / per_tok).ceil() as usize
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynthMode {
    /// One call emitting every prose section.
    SingleShot,
    /// One call per prose section id (fed only role-relevant essences), then a stitch.
    Sectioned(Vec<&'static str>),
}

/// Decide single-shot vs sectioned. Sectioned when the estimated OUTPUT would exceed 80 % of the
/// model's completion cap, or the material is very large.
pub fn decide_mode(
    template: super::templates::Template,
    max_output_kb: usize,
    model_output_cap_tokens: usize,
    material_kb: usize,
    lang_hint: Option<&str>,
) -> SynthMode {
    let est_output_tokens = estimate_tokens(&"x".repeat(max_output_kb * 1024), lang_hint);
    let cap_ok = (est_output_tokens as f64) <= 0.8 * (model_output_cap_tokens as f64);
    if cap_ok && material_kb <= 140 {
        SynthMode::SingleShot
    } else {
        SynthMode::Sectioned(prose_ids(template))
    }
}

/// Route a page essence to a prose section by its `page_role`.
pub fn essence_matches_section(section_id: &str, page_role: &str) -> bool {
    let role = page_role.to_ascii_lowercase();
    match section_id {
        "abstract" => true, // the abstract may draw on anything
        "identity_mission" | "identity_bio" | "product_identity" => {
            matches!(role.as_str(), "homepage" | "about" | "" | "other")
        }
        "offer_intro" | "value_prop" | "expertise" | "experience" => {
            matches!(
                role.as_str(),
                "service" | "product" | "pricing" | "homepage" | "about" | "case_study"
            )
        }
        "audiences" => matches!(
            role.as_str(),
            "service" | "product" | "case_study" | "homepage" | "pricing"
        ),
        _ => true,
    }
}

/// Build the compact synthesis material: the merged model JSON + the prose of the given essences
/// (already in rank order), trimmed to `budget_kb`. When over budget, the lowest-ranked essences'
/// prose is dropped first (deterministic).
pub fn build_material(model: &BrandModel, ranked_essences: &[(String, PageEssence)], budget_kb: usize) -> String {
    let budget = budget_kb * 1024;
    let model_json = serde_json::to_string(&model.to_json()).unwrap_or_default();

    let mut out = String::with_capacity(model_json.len() + 4096);
    out.push_str("<brand_model_json>\n");
    out.push_str(&model_json);
    out.push_str("\n</brand_model_json>\n<page_essences>\n");

    for (url, e) in ranked_essences {
        if e.essence.trim().is_empty() {
            continue;
        }
        let block = format!("- [{}] ({}) {}\n", e.page_role, url, e.essence.trim());
        if out.len() + block.len() > budget {
            break; // drop the rest (lowest-ranked prose) to stay within the material budget
        }
        out.push_str(&block);
    }
    out.push_str("</page_essences>");
    out
}

/// Build the synthesis system prompt: the fixed skeleton (prose sections to write, with per-section
/// instructions) + grounding + output-language + the strict JSON output contract.
pub fn build_system_prompt(skeleton: &[SectionSpec], section_ids: &[&str], report_language: &str) -> String {
    let mut sections = String::new();
    for s in skeleton {
        if s.kind == SectionKind::Prose && section_ids.contains(&s.id) {
            sections.push_str(&format!("- \"{}\": {}\n", s.id, s.instruction));
        }
    }
    format!(
        r#"<role>
You write the connective PROSE of a structured brand profile. You are given a compact, deduped
<brand_model_json> (people, offerings, facts, locations, channels, catalogs — all already extracted
from the site) and short per-page essences. Write clear, factual prose for the requested sections.
</role>

<grounding>
Use ONLY the provided material. Do NOT invent people, numbers, products, or claims. Do NOT restate
the structured lists (the crawler renders those). If you have no grounded material for a section,
return an empty string for it — NEVER pad or invent to fill a section.
</grounding>

<output_language>Write ALL prose in report language '{lang}'. Keep proper names, product names,
and verbatim quotes in their original language.</output_language>

<sections>
Write these sections (JSON key = section id):
{sections}</sections>

<output_contract>
Return ONLY one JSON object mapping each section id above to its markdown prose string, e.g.
{{"identity_mission": "…", "audiences": "…"}}. No other keys, no prose outside the JSON, no code fences.
</output_contract>"#,
        lang = report_language,
        sections = sections
    )
}

/// Parse the synthesis response into `section_id -> prose`. Unknown ids are ignored by the caller.
pub fn parse_prose_sections(raw: &str) -> Result<BTreeMap<String, String>, String> {
    let normalized = normalize_json_response(raw);
    if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&normalized) {
        return Ok(map);
    }
    let repaired = repair_json(raw);
    serde_json::from_str::<BTreeMap<String, String>>(&repaired).map_err(|e| format!("invalid synthesis JSON: {}", e))
}

#[cfg(test)]
mod tests {
    use super::super::templates::Template;
    use super::*;

    #[test]
    fn estimate_tokens_monotonic_and_language_split() {
        assert!(estimate_tokens("aaaa", None) < estimate_tokens("aaaaaaaa", None));
        // Czech is denser (more tokens for the same char count).
        let n = 1000;
        assert!(estimate_tokens(&"a".repeat(n), Some("cs")) > estimate_tokens(&"a".repeat(n), Some("en")));
    }

    #[test]
    fn decide_mode_crosses_at_output_cap() {
        // Small output, small material → single shot.
        assert_eq!(
            decide_mode(Template::Corporate, 40, 16_000, 60, Some("en")),
            SynthMode::SingleShot
        );
        // Large output that would exceed 80% of a tiny cap → sectioned.
        assert!(matches!(
            decide_mode(Template::Corporate, 60, 4_000, 60, Some("cs")),
            SynthMode::Sectioned(_)
        ));
        // Huge material forces sectioned even with a fine output.
        assert!(matches!(
            decide_mode(Template::Corporate, 30, 16_000, 200, Some("en")),
            SynthMode::Sectioned(_)
        ));
    }

    #[test]
    fn build_material_respects_budget_dropping_lowest_ranked() {
        let model = BrandModel::default();
        let essences: Vec<(String, PageEssence)> = (0..50)
            .map(|i| {
                (
                    format!("https://x/{}", i),
                    PageEssence {
                        page_role: "service".into(),
                        essence: "x".repeat(500),
                        ..Default::default()
                    },
                )
            })
            .collect();
        let material = build_material(&model, &essences, 4); // 4 KB budget
        assert!(material.len() <= 4 * 1024 + 200);
        // The first (highest-ranked) essence made it in; not all 50 did.
        assert!(material.contains("https://x/0"));
        assert!(!material.contains("https://x/49"));
    }

    #[test]
    fn parse_prose_sections_tolerates_fences() {
        let raw = "```json\n{\"identity_mission\":\"Firma dělá X.\",\"audiences\":\"Pro Y.\"}\n```";
        let map = parse_prose_sections(raw).unwrap();
        assert_eq!(map.get("identity_mission").unwrap(), "Firma dělá X.");
        assert!(parse_prose_sections("not json").is_err());
    }

    #[test]
    fn system_prompt_lists_only_requested_prose_sections() {
        let sk = super::super::templates::skeleton(Template::Corporate);
        let p = build_system_prompt(sk, &["identity_mission", "audiences"], "cs-CZ");
        assert!(p.contains("\"identity_mission\""));
        assert!(p.contains("\"audiences\""));
        assert!(!p.contains("\"services\"")); // a List section, never prose
        assert!(p.contains("cs-CZ"));
    }
}
