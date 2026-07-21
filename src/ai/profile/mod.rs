// SiteOne Crawler - AI profile pipeline (--ai-profile)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Chapter-based subject-profile generator. See docs/superpowers/specs/2026-07-21-ai-profile-design.md.
// Independent of `src/ai/elaborate/` (which is left untouched).

pub mod budget;
pub mod embedded;
pub mod promptpack;
pub mod registry;
pub mod select;

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
