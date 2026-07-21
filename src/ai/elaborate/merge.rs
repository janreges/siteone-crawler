// SiteOne Crawler - brand elaborate: deterministic entity merge
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Merge per-page `PageEssence` entities into one deduped `BrandModel`, in pure Rust (no LLM). This
// is the anti-hallucination core: 100 pages that each mention the same person collapse to one
// entry, and the crawler renders the structured lists from THIS model — so contacts, people, and
// facts in the document are copied verbatim and cannot be invented by the synthesis LLM. Every
// entity keeps its source URLs for the `## Sources` provenance section.

use serde_json::{Value, json};

use super::cluster::ClusterSummary;
use super::extract::{Channel, PageEssence};

/// A value plus the page URLs it was extracted from.
#[derive(Debug, Clone)]
pub struct Sourced<T> {
    pub value: T,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MergedPerson {
    pub name: String,
    pub role: String,
    pub email: String,
    pub phone: String,
    pub profile_url: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MergedOffering {
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub for_whom: String,
    pub price_or_terms: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MergedLocation {
    pub label: String,
    pub address: String,
    pub phone: String,
    pub email: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BrandModel {
    pub people: Vec<MergedPerson>,
    pub offerings: Vec<MergedOffering>,
    pub locations: Vec<MergedLocation>,
    pub facts: Vec<Sourced<String>>,
    pub channels: Vec<Sourced<Channel>>,
    pub quotes: Vec<Sourced<String>>,
    pub catalogs: Vec<ClusterSummary>,
    /// All page URLs the model draws on (deduped, in first-seen order).
    pub sources: Vec<String>,
}

/// Normalize a free-text value for dedup: lowercase, FOLD DIACRITICS, collapse whitespace, strip
/// surrounding punctuation/quotes. Used only as a KEY — the retained value stays verbatim. Folding
/// diacritics is what lets "Ján Regeš" and "Jan Reges" (or a fact written with vs. without accents)
/// collapse to one entry across pages, on any Latin-script site.
fn norm(s: &str) -> String {
    fold_diacritics(&s.to_lowercase())
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '.' || c == ',' || c == ';' || c == '“' || c == '”')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Map common Latin diacritics to their ASCII base letter (Czech/Slovak/Polish/German/French/
/// Spanish/Nordic/… ). Deliberately conservative and language-agnostic — it only affects dedup keys,
/// never the retained verbatim value. Input is expected already lowercased.
fn fold_diacritics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
            'ç' | 'č' | 'ć' | 'ĉ' | 'ċ' => out.push('c'),
            'ď' | 'đ' => out.push('d'),
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => out.push('e'),
            'ğ' | 'ĝ' | 'ġ' | 'ģ' => out.push('g'),
            'í' | 'ì' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' => out.push('i'),
            'ĺ' | 'ľ' | 'ł' | 'ļ' => out.push('l'),
            'ñ' | 'ń' | 'ň' | 'ņ' => out.push('n'),
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ō' | 'ŏ' | 'ő' | 'ø' => out.push('o'),
            'ŕ' | 'ř' | 'ŗ' => out.push('r'),
            'š' | 'ś' | 'ŝ' | 'ş' | 'ș' => out.push('s'),
            'ť' | 'ţ' | 'ț' => out.push('t'),
            'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => out.push('u'),
            'ý' | 'ÿ' | 'ŷ' => out.push('y'),
            'ž' | 'ź' | 'ż' => out.push('z'),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            _ => out.push(c),
        }
    }
    out
}

/// Significant tokens of an address for containment-based deduplication: diacritic-folded,
/// lowercased, split on non-alphanumerics; word tokens (>=2 chars) and any numeric token (house
/// numbers, ZIP) are kept.
fn address_tokens(s: &str) -> std::collections::BTreeSet<String> {
    fold_diacritics(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && (t.chars().count() >= 2 || t.chars().all(|c| c.is_ascii_digit())))
        .map(|t| t.to_string())
        .collect()
}

/// Keep the longer / non-empty of two candidate values (a later page may carry a fuller version).
fn richer(current: &str, candidate: &str) -> Option<String> {
    let candidate = candidate.trim();
    if !candidate.is_empty() && candidate.len() > current.trim().len() {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn push_source(sources: &mut Vec<String>, url: &str) {
    if !url.is_empty() && !sources.iter().any(|s| s == url) {
        sources.push(url.to_string());
    }
}

pub fn merge(essences: &[(String, PageEssence)], clusters: &[ClusterSummary]) -> BrandModel {
    let mut model = BrandModel {
        catalogs: clusters.to_vec(),
        ..Default::default()
    };

    // Dedup indexes: normalized key → position in the corresponding Vec.
    let mut offering_ix: Vec<(String, usize)> = Vec::new();
    let mut location_ix: Vec<(String, usize)> = Vec::new();
    let mut fact_keys: Vec<String> = Vec::new();
    let mut quote_keys: Vec<String> = Vec::new();
    let mut channel_keys: Vec<String> = Vec::new();

    for (url, e) in essences {
        push_source(&mut model.sources, url);

        merge_people(e, url, &mut model);
        merge_offerings(e, url, &mut model, &mut offering_ix);
        merge_locations(e, url, &mut model, &mut location_ix);

        for f in &e.facts {
            let key = norm(f);
            if key.is_empty() {
                continue;
            }
            if let Some(pos) = fact_keys.iter().position(|k| k == &key) {
                push_source(&mut model.facts[pos].sources, url);
            } else {
                fact_keys.push(key);
                model.facts.push(Sourced {
                    value: f.trim().to_string(),
                    sources: vec![url.clone()],
                });
            }
        }
        for q in &e.quotes {
            let key = norm(q);
            if key.is_empty() {
                continue;
            }
            if let Some(pos) = quote_keys.iter().position(|k| k == &key) {
                push_source(&mut model.quotes[pos].sources, url);
            } else {
                quote_keys.push(key);
                model.quotes.push(Sourced {
                    value: q.trim().to_string(),
                    sources: vec![url.clone()],
                });
            }
        }
        for c in &e.channels {
            let key = norm(&c.value);
            if key.is_empty() {
                continue;
            }
            if let Some(pos) = channel_keys.iter().position(|k| k == &key) {
                push_source(&mut model.channels[pos].sources, url);
            } else {
                channel_keys.push(key);
                model.channels.push(Sourced {
                    value: c.clone(),
                    sources: vec![url.clone()],
                });
            }
        }
    }

    // Second pass: collapse near-duplicate locations that the exact-key dedup missed because the
    // same place is written with a slightly different address (e.g. with/without ZIP, "996/25" vs
    // "25"). Conservative and language-agnostic — see `locations_are_same`.
    consolidate_locations(&mut model);
    model
}

/// Merge locations that describe the same physical place but were written differently across pages.
/// O(n²) over a small list (locations are few). Keeps the richer entry and unions the sources.
fn consolidate_locations(model: &mut BrandModel) {
    let mut i = 0;
    while i < model.locations.len() {
        let mut j = i + 1;
        while j < model.locations.len() {
            if locations_are_same(&model.locations[i], &model.locations[j]) {
                let removed = model.locations.remove(j);
                merge_location_into(&mut model.locations[i], removed);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

/// True when two locations almost certainly denote the same place. Requires the smaller address'
/// significant tokens to be a SUBSET of the larger's, with at least 3 shared tokens and at least one
/// shared numeric token (house number/ZIP) — so two different branches on the same street, or two
/// bare city names, are never merged.
fn locations_are_same(a: &MergedLocation, b: &MergedLocation) -> bool {
    let ta = address_tokens(&a.address);
    let tb = address_tokens(&b.address);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let (small, big) = if ta.len() <= tb.len() { (&ta, &tb) } else { (&tb, &ta) };
    small.len() >= 3 && small.is_subset(big) && small.iter().any(|t| t.chars().all(|c| c.is_ascii_digit()))
}

/// Fold `removed` into `keep`: prefer the richer (longer) address and label, fill blank contacts,
/// union the source URLs.
fn merge_location_into(keep: &mut MergedLocation, removed: MergedLocation) {
    if removed.address.trim().len() > keep.address.trim().len() {
        keep.address = removed.address;
    }
    if removed.label.trim().len() > keep.label.trim().len() {
        keep.label = removed.label;
    }
    if keep.phone.trim().is_empty() && !removed.phone.trim().is_empty() {
        keep.phone = removed.phone;
    }
    if keep.email.trim().is_empty() && !removed.email.trim().is_empty() {
        keep.email = removed.email;
    }
    for s in removed.sources {
        push_source(&mut keep.sources, &s);
    }
}

fn merge_people(e: &PageEssence, url: &str, model: &mut BrandModel) {
    for p in &e.people {
        let name_key = norm(&p.name);
        let email_key = norm(&p.email);
        if name_key.is_empty() && email_key.is_empty() {
            continue;
        }
        // Match an existing person by a shared non-empty email OR the same normalized name — so a
        // page that first lists a person without an email and a later page that adds it still merge.
        let existing = model.people.iter().position(|m| {
            (!email_key.is_empty() && norm(&m.email) == email_key)
                || (!name_key.is_empty() && norm(&m.name) == name_key)
        });
        if let Some(pos) = existing {
            let m = &mut model.people[pos];
            if let Some(v) = richer(&m.name, &p.name) {
                m.name = v;
            }
            if let Some(v) = richer(&m.role, &p.role) {
                m.role = v;
            }
            if m.email.trim().is_empty() && !p.email.trim().is_empty() {
                m.email = p.email.trim().to_string();
            }
            if m.phone.trim().is_empty() && !p.phone.trim().is_empty() {
                m.phone = p.phone.trim().to_string();
            }
            if m.profile_url.trim().is_empty() && !p.profile_url.trim().is_empty() {
                m.profile_url = p.profile_url.trim().to_string();
            }
            push_source(&mut m.sources, url);
        } else {
            model.people.push(MergedPerson {
                name: p.name.trim().to_string(),
                role: p.role.trim().to_string(),
                email: p.email.trim().to_string(),
                phone: p.phone.trim().to_string(),
                profile_url: p.profile_url.trim().to_string(),
                sources: vec![url.to_string()],
            });
        }
    }
}

fn merge_offerings(e: &PageEssence, url: &str, model: &mut BrandModel, ix: &mut Vec<(String, usize)>) {
    for o in &e.offerings {
        if o.name.trim().is_empty() {
            continue;
        }
        let kind = normalize_kind(&o.kind);
        let key = format!("{}:{}", kind, norm(&o.name));
        if let Some((_, pos)) = ix.iter().find(|(k, _)| k == &key) {
            let m = &mut model.offerings[*pos];
            if let Some(v) = richer(&m.summary, &o.summary) {
                m.summary = v;
            }
            if let Some(v) = richer(&m.for_whom, &o.for_whom) {
                m.for_whom = v;
            }
            if let Some(v) = richer(&m.price_or_terms, &o.price_or_terms) {
                m.price_or_terms = v;
            }
            push_source(&mut m.sources, url);
        } else {
            ix.push((key, model.offerings.len()));
            model.offerings.push(MergedOffering {
                kind,
                name: o.name.trim().to_string(),
                summary: o.summary.trim().to_string(),
                for_whom: o.for_whom.trim().to_string(),
                price_or_terms: o.price_or_terms.trim().to_string(),
                sources: vec![url.to_string()],
            });
        }
    }
}

fn merge_locations(e: &PageEssence, url: &str, model: &mut BrandModel, ix: &mut Vec<(String, usize)>) {
    for l in &e.locations {
        let key = norm(&l.address);
        if key.is_empty() && norm(&l.label).is_empty() {
            continue;
        }
        let key = if key.is_empty() {
            format!("label:{}", norm(&l.label))
        } else {
            key
        };
        if let Some((_, pos)) = ix.iter().find(|(k, _)| k == &key) {
            let m = &mut model.locations[*pos];
            if m.phone.trim().is_empty() && !l.phone.trim().is_empty() {
                m.phone = l.phone.trim().to_string();
            }
            if m.email.trim().is_empty() && !l.email.trim().is_empty() {
                m.email = l.email.trim().to_string();
            }
            push_source(&mut m.sources, url);
        } else {
            ix.push((key, model.locations.len()));
            model.locations.push(MergedLocation {
                label: l.label.trim().to_string(),
                address: l.address.trim().to_string(),
                phone: l.phone.trim().to_string(),
                email: l.email.trim().to_string(),
                sources: vec![url.to_string()],
            });
        }
    }
}

fn normalize_kind(kind: &str) -> String {
    match kind.trim().to_lowercase().as_str() {
        "product" | "products" => "product".to_string(),
        _ => "service".to_string(),
    }
}

impl BrandModel {
    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
            && self.offerings.is_empty()
            && self.locations.is_empty()
            && self.facts.is_empty()
            && self.channels.is_empty()
            && self.quotes.is_empty()
            && self.catalogs.is_empty()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "people": self.people.iter().map(|p| json!({
                "name": p.name, "role": p.role, "email": p.email, "phone": p.phone,
                "profileUrl": p.profile_url, "sources": p.sources,
            })).collect::<Vec<_>>(),
            "offerings": self.offerings.iter().map(|o| json!({
                "kind": o.kind, "name": o.name, "summary": o.summary, "forWhom": o.for_whom,
                "priceOrTerms": o.price_or_terms, "sources": o.sources,
            })).collect::<Vec<_>>(),
            "locations": self.locations.iter().map(|l| json!({
                "label": l.label, "address": l.address, "phone": l.phone, "email": l.email, "sources": l.sources,
            })).collect::<Vec<_>>(),
            "facts": self.facts.iter().map(|f| json!({"value": f.value, "sources": f.sources})).collect::<Vec<_>>(),
            "channels": self.channels.iter().map(|c| json!({"kind": c.value.kind, "value": c.value.value, "sources": c.sources})).collect::<Vec<_>>(),
            "quotes": self.quotes.iter().map(|q| json!({"value": q.value, "sources": q.sources})).collect::<Vec<_>>(),
            "catalogs": self.catalogs.iter().map(|c| json!({
                "label": c.label, "template": c.template, "count": c.count, "sampleUrls": c.sample_urls,
            })).collect::<Vec<_>>(),
            "sources": self.sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::extract::{Location, Offering, Person};
    use super::*;

    fn person(name: &str, role: &str, email: &str) -> Person {
        Person {
            name: name.into(),
            role: role.into(),
            email: email.into(),
            ..Default::default()
        }
    }

    #[test]
    fn same_person_across_pages_merges_keeps_email_and_sources() {
        let essences = vec![
            (
                "https://x/tym".to_string(),
                PageEssence {
                    people: vec![person("Jan Novák", "CEO", "")],
                    ..Default::default()
                },
            ),
            (
                "https://x/kontakt".to_string(),
                PageEssence {
                    people: vec![person("Jan Novák", "CEO, zakladatel", "jan@x.cz")],
                    ..Default::default()
                },
            ),
            (
                "https://x/o-nas".to_string(),
                PageEssence {
                    people: vec![person("jan novák", "", "")],
                    ..Default::default()
                },
            ),
        ];
        let m = merge(&essences, &[]);
        assert_eq!(m.people.len(), 1);
        assert_eq!(m.people[0].email, "jan@x.cz");
        assert_eq!(m.people[0].role, "CEO, zakladatel"); // richer role kept
        assert_eq!(m.people[0].sources.len(), 3);
        assert_eq!(m.sources.len(), 3);
    }

    #[test]
    fn people_dedup_is_diacritic_insensitive() {
        // "Ján Regeš" and "Jan Reges" are the same person written with/without accents.
        let essences = vec![
            (
                "https://x/a".to_string(),
                PageEssence {
                    people: vec![person("Ján Regeš", "Author", "")],
                    ..Default::default()
                },
            ),
            (
                "https://x/b".to_string(),
                PageEssence {
                    people: vec![person("Jan Reges", "Main author", "jan@x.cz")],
                    ..Default::default()
                },
            ),
        ];
        let m = merge(&essences, &[]);
        assert_eq!(m.people.len(), 1, "diacritic variants must collapse to one person");
        assert_eq!(m.people[0].email, "jan@x.cz");
        // Different people are NOT merged.
        let two = merge(
            &[(
                "https://x/c".to_string(),
                PageEssence {
                    people: vec![person("Jan Novák", "CEO", ""), person("Jana Nováková", "CFO", "")],
                    ..Default::default()
                },
            )],
            &[],
        );
        assert_eq!(two.people.len(), 2);
    }

    #[test]
    fn locations_consolidate_same_place_but_not_different_branches() {
        fn loc(label: &str, address: &str) -> Location {
            Location {
                label: label.into(),
                address: address.into(),
                ..Default::default()
            }
        }
        // Fictional streets/numbers (cities are generic); not tied to any real entity.
        let essences = vec![(
            "https://x/kontakt".to_string(),
            PageEssence {
                locations: vec![
                    loc("Sídlo", "Květinová 123/7, 602 00 Brno"),
                    loc("Ukázka s.r.o.", "Květinová 7, Brno"), // same HQ, sparser
                    loc("Pobočka Praha", "Lipová 42, 120 00 Praha 2"),
                    loc("Pobočka Ostrava", "Dubová 5/1, 722 00 Ostrava"),
                ],
                ..Default::default()
            },
        )];
        let m = merge(&essences, &[]);
        // The two Brno HQ entries collapse; Praha and Ostrava stay distinct → 3 total.
        assert_eq!(m.locations.len(), 3);
        // The richer Brno address is kept.
        assert!(m.locations.iter().any(|l| l.address.contains("123/7")));
        assert!(m.locations.iter().any(|l| l.address.contains("Lipová")));
        assert!(m.locations.iter().any(|l| l.address.contains("Dubová")));
    }

    #[test]
    fn offerings_dedup_case_insensitive_facts_and_quotes_dedup() {
        let essences = vec![
            (
                "https://x/a".to_string(),
                PageEssence {
                    offerings: vec![Offering {
                        kind: "service".into(),
                        name: "Účetnictví".into(),
                        summary: "krátký".into(),
                        ..Default::default()
                    }],
                    facts: vec!["Založeno 2010.".into()],
                    quotes: vec!["Nejlepší v oboru".into()],
                    ..Default::default()
                },
            ),
            (
                "https://x/b".to_string(),
                PageEssence {
                    offerings: vec![Offering {
                        kind: "service".into(),
                        name: "účetnictví".into(),
                        summary: "delší a podrobnější popis".into(),
                        ..Default::default()
                    }],
                    facts: vec!["Založeno 2010".into()], // dedup ignores trailing period
                    ..Default::default()
                },
            ),
        ];
        let m = merge(&essences, &[]);
        assert_eq!(m.offerings.len(), 1);
        assert_eq!(m.offerings[0].summary, "delší a podrobnější popis"); // richer kept
        assert_eq!(m.facts.len(), 1);
        assert_eq!(m.facts[0].sources.len(), 2);
        assert_eq!(m.quotes.len(), 1);
    }
}
