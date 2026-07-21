// SiteOne Crawler - brand elaborate: id-constrained page selection
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The LLM only ever sees a NUMBERED candidate list and returns integer ids — hallucinated URLs are
// impossible by construction. Round 1 maps the site's information architecture (sections + which
// mass-entity clusters to sample + the site type). Round 2 expands flagged sections with their real
// 2nd-level links. A deterministic merge unions the LLM picks with cluster representatives and a
// safety floor (top-N by score), so a weak LLM round can never drop the obvious pages, then caps to
// the page budget.

use serde::Deserialize;

use crate::ai::normalize::{normalize_json_response, repair_json};
use crate::ai::selection::Candidate;

use super::cluster::{ClusterSummary, UrlCluster};

#[derive(Debug, Clone, Deserialize)]
pub struct IaSection {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub anchor_ids: Vec<usize>,
    #[serde(default)]
    pub expand: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IaCluster {
    #[serde(default, rename = "ref")]
    pub cref: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub sample: bool,
    #[serde(default)]
    pub reps: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IaMap {
    #[serde(default)]
    pub site_type: String,
    #[serde(default)]
    pub primary_language: String,
    #[serde(default)]
    pub sections: Vec<IaSection>,
    #[serde(default)]
    pub must_include_ids: Vec<usize>,
    #[serde(default)]
    pub clusters: Vec<IaCluster>,
}

/// The final selection: the pages to extract + the labeled clusters to characterize.
pub struct SelectionPlan {
    pub selected: Vec<Candidate>,
    pub clusters: Vec<ClusterSummary>,
    pub site_type: String,
    pub notices: Vec<String>,
}

pub const SELECT_IA_SYSTEM_PROMPT: &str = r#"<role>
You map the information architecture of a website to select the most important pages for a brand
profile. You are given a NUMBERED candidate list and a list of mass-entity CLUSTERS.
</role>
<instructions>
- Group the important candidate pages into top-level sections (About/Company, Services, Products,
  People/Team, Contact, Pricing, Careers, Legal, …). Reference pages ONLY by their integer id.
- Set expand:true for a section whose sub-pages should be explored further (e.g. a Services hub).
- For each CLUSTER decide sample:true/false and how many representatives (reps, 0-3) to keep. Do NOT
  try to include every member of a cluster — it is a mass-entity group (products, articles, …).
- Classify the site: site_type = corporate | ecommerce | product | personal | other.
- Never invent ids or URLs. Use ONLY ids from the numbered list and refs from the cluster list.
</instructions>
<output_contract>
Return ONLY one JSON object:
{"site_type":"", "primary_language":"", "sections":[{"name":"","purpose":"","anchor_ids":[],"expand":false}],
 "must_include_ids":[], "clusters":[{"ref":"c0","label":"","sample":true,"reps":2}]}
No prose, no code fences.
</output_contract>"#;

pub const SELECT_EXPAND_SYSTEM_PROMPT: &str = r#"<role>
You pick the important sub-pages of a website section. You are given a NUMBERED candidate list of the
section's own outbound links.
</role>
<instructions>
Return the ids of the pages worth including in a brand profile (skip pagination, duplicates, and
mass-entity leaves). Reference pages ONLY by integer id from the list.
</instructions>
<output_contract>Return ONLY {"ids":[]}. No prose, no code fences.</output_contract>"#;

/// Render a numbered candidate list for the LLM: `id | path | title/anchor | flags`.
pub fn render_candidate_list(shown: &[(String, Candidate)]) -> String {
    let mut out = String::new();
    for (i, (title, c)) in shown.iter().enumerate() {
        let path = url::Url::parse(&c.url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| c.url.clone());
        let mut flags = Vec::new();
        if c.depth <= 1 {
            flags.push("nav");
        }
        if c.in_sitemap {
            flags.push("sitemap");
        }
        out.push_str(&format!(
            "{} | {} | {} | depth={} {}\n",
            i,
            path,
            title.chars().take(80).collect::<String>(),
            c.depth,
            flags.join(",")
        ));
    }
    out
}

/// Render the cluster list for round 1: `refN | template | count | examples`.
pub fn render_cluster_list(clusters: &[UrlCluster]) -> String {
    let mut out = String::new();
    for (i, c) in clusters.iter().enumerate() {
        out.push_str(&format!(
            "c{} | {} | {} pages | e.g. {}\n",
            i,
            c.template,
            c.member_uq_ids.len(),
            c.sample_urls.join(", ")
        ));
    }
    out
}

#[derive(Deserialize)]
struct ExpandIds {
    #[serde(default)]
    ids: Vec<usize>,
}

pub fn parse_ia_map(raw: &str) -> Result<IaMap, String> {
    let normalized = normalize_json_response(raw);
    if let Ok(m) = serde_json::from_str::<IaMap>(&normalized) {
        return Ok(m);
    }
    serde_json::from_str::<IaMap>(&repair_json(raw)).map_err(|e| format!("invalid IA map JSON: {}", e))
}

pub fn parse_expand_ids(raw: &str) -> Result<Vec<usize>, String> {
    let normalized = normalize_json_response(raw);
    if let Ok(w) = serde_json::from_str::<ExpandIds>(&normalized) {
        return Ok(w.ids);
    }
    serde_json::from_str::<ExpandIds>(&repair_json(raw))
        .map(|w| w.ids)
        .map_err(|e| format!("invalid expand ids: {}", e))
}

/// Deterministic merge: union of the LLM's picks + cluster representatives + a top-`floor` safety net
/// from the FULL universe, deduped by uq_id, LLM-picked first, capped to `budget`. Out-of-range ids
/// are dropped (never fetched). `cN` cluster refs map to `clusters[N]`.
#[allow(clippy::too_many_arguments)]
pub fn merge_selection(
    shown: &[(String, Candidate)],
    ia: &IaMap,
    expand: &[usize],
    all_candidates: &[Candidate],
    clusters: &[UrlCluster],
    reps: usize,
    floor: usize,
    budget: usize,
) -> SelectionPlan {
    let mut picked: Vec<String> = Vec::new(); // uq_ids the LLM chose (highest priority)
    let add_id = |id: usize, picked: &mut Vec<String>| {
        if let Some((_, c)) = shown.get(id)
            && !picked.iter().any(|u| u == &c.uq_id)
        {
            picked.push(c.uq_id.clone());
        }
    };
    for &id in &ia.must_include_ids {
        add_id(id, &mut picked);
    }
    for sec in &ia.sections {
        for &id in &sec.anchor_ids {
            add_id(id, &mut picked);
        }
    }
    for &id in expand {
        add_id(id, &mut picked);
    }

    // Cluster representatives + labeled cluster summaries.
    let mut cluster_summaries: Vec<ClusterSummary> = Vec::new();
    let mut rep_uqs: Vec<String> = Vec::new();
    for ic in &ia.clusters {
        let idx = ic.cref.trim_start_matches('c').parse::<usize>().ok();
        let Some(uc) = idx.and_then(|i| clusters.get(i)) else {
            continue;
        };
        if uc.denied {
            continue; // never sample crawl noise, whatever the LLM asked
        }
        let want = if ic.sample { ic.reps.min(reps).max(1) } else { 0 };
        cluster_summaries.push(ClusterSummary {
            label: if ic.label.trim().is_empty() {
                uc.template.clone()
            } else {
                ic.label.clone()
            },
            template: uc.template.clone(),
            count: uc.member_uq_ids.len(),
            sample_urls: uc.sample_urls.clone(),
        });
        for uq in uc.member_uq_ids.iter().take(want) {
            if !rep_uqs.iter().any(|u| u == uq) {
                rep_uqs.push(uq.clone());
            }
        }
    }

    // Deterministic floor: the top-`floor` by score from the full universe (already score-desc).
    let floor_uqs: Vec<String> = all_candidates.iter().take(floor).map(|c| c.uq_id.clone()).collect();

    // Assemble in priority order: LLM picks → cluster reps → floor. Dedup by uq_id.
    let by_uq = |uq: &str| all_candidates.iter().find(|c| c.uq_id == uq).cloned();
    let mut selected: Vec<Candidate> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for uq in picked.iter().chain(rep_uqs.iter()).chain(floor_uqs.iter()) {
        if seen.iter().any(|s| s == uq) {
            continue;
        }
        // Prefer the resolved candidate from the universe; fall back to the shown entry.
        let cand = by_uq(uq).or_else(|| shown.iter().find(|(_, c)| &c.uq_id == uq).map(|(_, c)| c.clone()));
        if let Some(c) = cand {
            seen.push(uq.clone());
            selected.push(c);
        }
    }
    selected.truncate(budget);

    SelectionPlan {
        selected,
        clusters: cluster_summaries,
        site_type: ia.site_type.clone(),
        notices: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(uq: &str, path: &str, score: f64) -> Candidate {
        Candidate {
            uq_id: uq.into(),
            url: format!("https://x{}", path),
            depth: 2,
            in_sitemap: false,
            score,
        }
    }

    #[test]
    fn parse_ia_map_tolerates_fences_and_dirty() {
        let raw = "```json\n{\"site_type\":\"corporate\",\"must_include_ids\":[0,2],\"sections\":[{\"name\":\"About\",\"anchor_ids\":[1],\"expand\":true}],}\n```";
        let m = parse_ia_map(raw).unwrap();
        assert_eq!(m.site_type, "corporate");
        assert_eq!(m.must_include_ids, vec![0, 2]);
        assert!(m.sections[0].expand);
        assert_eq!(parse_expand_ids("{\"ids\":[3,4]}").unwrap(), vec![3, 4]);
    }

    #[test]
    fn merge_selects_llm_picks_floor_rescue_and_dedup_and_cap() {
        // Universe: 5 pages, score-desc.
        let all = vec![
            cand("home", "/", 100.0),
            cand("about", "/o-nas", 66.0),
            cand("svc", "/sluzby", 66.0),
            cand("team", "/tym", 60.0),
            cand("contact", "/kontakt", 55.0),
        ];
        // Shown list (what the LLM saw): first 4.
        let shown: Vec<(String, Candidate)> = all.iter().take(4).map(|c| (c.url.clone(), c.clone())).collect();
        let ia = IaMap {
            site_type: "corporate".into(),
            must_include_ids: vec![1], // about
            sections: vec![IaSection {
                name: "Services".into(),
                anchor_ids: vec![2, 99], // svc + out-of-range (dropped)
                ..Default_ia_section()
            }],
            ..Default::default()
        };
        // floor=1 rescues "home" even though the LLM didn't pick it; budget large.
        let plan = merge_selection(&shown, &ia, &[], &all, &[], 2, 1, 10);
        let uqs: Vec<&str> = plan.selected.iter().map(|c| c.uq_id.as_str()).collect();
        assert!(uqs.contains(&"about")); // LLM pick
        assert!(uqs.contains(&"svc")); // LLM pick
        assert!(uqs.contains(&"home")); // floor rescue
        assert!(!uqs.contains(&"contact")); // not picked, not in floor
        // No duplicates, out-of-range id 99 dropped.
        assert_eq!(
            uqs.len(),
            plan.selected
                .iter()
                .map(|c| &c.uq_id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );

        // Budget cap.
        let capped = merge_selection(&shown, &ia, &[], &all, &[], 2, 5, 2);
        assert_eq!(capped.selected.len(), 2);
    }

    #[test]
    fn cluster_reps_sampled_denied_skipped() {
        let all: Vec<Candidate> = (0..12)
            .map(|i| cand(&format!("p{}", i), &format!("/p/{}", i), 20.0))
            .collect();
        let shown: Vec<(String, Candidate)> = Vec::new();
        let clusters = vec![
            UrlCluster {
                template: "/p/*".into(),
                member_uq_ids: (0..12).map(|i| format!("p{}", i)).collect(),
                sample_urls: vec!["https://x/p/0".into()],
                denied: false,
            },
            UrlCluster {
                template: "/tag/*".into(),
                member_uq_ids: vec!["t0".into(), "t1".into()],
                sample_urls: vec![],
                denied: true,
            },
        ];
        let ia = IaMap {
            clusters: vec![
                IaCluster {
                    cref: "c0".into(),
                    label: "Products".into(),
                    sample: true,
                    reps: 3,
                },
                IaCluster {
                    cref: "c1".into(),
                    label: "Tags".into(),
                    sample: true,
                    reps: 3,
                },
            ],
            ..Default::default()
        };
        let plan = merge_selection(&shown, &ia, &[], &all, &clusters, 2, 0, 100);
        // reps capped at the global `reps`=2 for the product cluster; denied cluster contributes 0.
        assert_eq!(plan.selected.len(), 2);
        assert_eq!(plan.clusters.len(), 1); // only the non-denied cluster is characterized
        assert_eq!(plan.clusters[0].label, "Products");
        assert_eq!(plan.clusters[0].count, 12);
    }

    // Helper: IaSection has no Default derive (serde-only defaults), so build one for tests.
    #[allow(non_snake_case)]
    fn Default_ia_section() -> IaSection {
        IaSection {
            name: String::new(),
            purpose: String::new(),
            anchor_ids: vec![],
            expand: false,
        }
    }
}
