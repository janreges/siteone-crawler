// SiteOne Crawler - AI profile: prompt registry
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Owns the embedded shared prompts and the lazily-parsed type/chapter specs. The 13-type taxonomy
// order here is canonical (the classifier catalog is numbered in this order).

use once_cell::sync::Lazy;

use super::embedded::{CHAPTER_FILES, TYPE_FILES};
use super::promptpack::{self, ChapterSpec, TypeSpec};

pub const TYPE_KEYS: [&str; 13] = [
    "corporate",
    "smb-services",
    "product",
    "ecommerce",
    "personal",
    "media-news",
    "expert-content",
    "nonprofit",
    "government",
    "institution",
    "portal-directory",
    "events-culture",
    "general",
];

pub const SITE_SUMMARY: &str = include_str!("../../../prompts/profile/shared/site-summary.md");
pub const CLASSIFY: &str = include_str!("../../../prompts/profile/shared/classify.md");
pub const PAGE_DESCRIBE: &str = include_str!("../../../prompts/profile/shared/page-describe.md");
pub const SELECT_PAGES: &str = include_str!("../../../prompts/profile/shared/select-pages.md");
pub const SYNTHESIZE_CHAPTER: &str = include_str!("../../../prompts/profile/shared/synthesize-chapter.md");
pub const CORRECT: &str = include_str!("../../../prompts/profile/shared/correct.md");
pub const EXEC_SUMMARY: &str = include_str!("../../../prompts/profile/shared/exec-summary.md");
pub const LOCALIZE_HEADINGS: &str = include_str!("../../../prompts/profile/shared/localize-headings.md");

static TYPES: Lazy<Vec<TypeSpec>> = Lazy::new(|| {
    TYPE_KEYS
        .iter()
        .map(|&key| {
            let raw = TYPE_FILES
                .iter()
                .find(|(k, _)| *k == key)
                .unwrap_or_else(|| panic!("embedded type file missing for '{key}'"))
                .1;
            let (name_cs, name_en, desc) =
                promptpack::parse_type(key, raw).unwrap_or_else(|e| panic!("bad embedded type '{key}': {e}"));
            let mut chapters: Vec<ChapterSpec> = CHAPTER_FILES
                .iter()
                .filter(|(k, _, _)| *k == key)
                .map(|(_, id, raw)| {
                    promptpack::parse_chapter(id, raw).unwrap_or_else(|e| panic!("bad embedded chapter: {e}"))
                })
                .collect();
            chapters.sort_by(|a, b| a.id.cmp(&b.id));
            TypeSpec {
                key: key.to_string(),
                name_cs,
                name_en,
                classifier_description: desc,
                chapters,
            }
        })
        .collect()
});

pub fn types() -> &'static [TypeSpec] {
    &TYPES
}

pub fn type_by_key(key: &str) -> Option<&'static TypeSpec> {
    let key = key.trim().to_ascii_lowercase();
    TYPES.iter().find(|t| t.key == key)
}

/// The numbered catalog rendered into the classify prompt (`{{catalog}}`); ids are 1-based and match
/// `TYPE_KEYS` order, so `parse` maps id → `TYPE_KEYS[id-1]`.
pub fn classifier_catalog() -> String {
    let mut out = String::new();
    for (i, t) in TYPES.iter().enumerate() {
        out.push_str(&format!("{}. {} — {}\n", i + 1, t.key, t.classifier_description.trim()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_13_valid_types_with_unique_ordered_chapters() {
        let ts = types();
        assert_eq!(ts.len(), 13);
        for (i, t) in ts.iter().enumerate() {
            assert_eq!(t.key, TYPE_KEYS[i]);
            assert!(!t.classifier_description.is_empty());
            assert!(!t.chapters.is_empty(), "type {} has no chapters", t.key);
            let mut ids: Vec<&str> = t.chapters.iter().map(|c| c.id.as_str()).collect();
            let sorted = ids.clone();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), t.chapters.len(), "duplicate chapter id in {}", t.key);
            // Ids are ordered by their NN- prefix.
            assert!(
                sorted.windows(2).all(|w| w[0] <= w[1]),
                "chapters not ordered in {}",
                t.key
            );
        }
    }

    #[test]
    fn general_type_has_seven_universal_chapters() {
        let g = type_by_key("general").unwrap();
        assert_eq!(g.chapters.len(), 7);
    }

    #[test]
    fn classifier_catalog_numbers_all_types() {
        let cat = classifier_catalog();
        assert!(cat.starts_with("1. corporate"));
        assert!(cat.contains("13. general"));
    }

    #[test]
    fn shared_prompts_only_use_known_placeholders() {
        let allowed = [
            "catalog",
            "site_description",
            "chapter_heading",
            "chapter_focus",
            "max_pages",
            "budget_chars",
            "chapter_instructions",
            "target_chars",
            "language",
        ];
        let prompts = [
            SITE_SUMMARY,
            CLASSIFY,
            PAGE_DESCRIBE,
            SELECT_PAGES,
            SYNTHESIZE_CHAPTER,
            CORRECT,
            EXEC_SUMMARY,
            LOCALIZE_HEADINGS,
        ];
        let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
        for p in prompts {
            for cap in re.captures_iter(p) {
                let name = cap.get(1).unwrap().as_str();
                assert!(allowed.contains(&name), "unknown placeholder {{{{{name}}}}}");
            }
        }
    }
}
