// SiteOne Crawler - TargetDomainRelation
// (c) Jan Reges <jan.reges@siteone.cz>

use crate::engine::parsed_url::ParsedUrl;

/// Describes the relationship between initial URL, base URL (page where link was found),
/// and target URL (the link destination).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetDomainRelation {
    /// e.g. initial www.siteone.io, base www.siteone.io, target www.siteone.io
    InitialSameBaseSame,
    /// e.g. initial www.siteone.io, base nextjs.org, target www.siteone.io
    InitialSameBaseDifferent,
    /// e.g. initial www.siteone.io, base nextjs.org, target nextjs.org
    InitialDifferentBaseSame,
    /// e.g. initial www.siteone.io, base nextjs.org, target svelte.dev
    InitialDifferentBaseDifferent,
}

impl TargetDomainRelation {
    /// Determine the domain relation given the hosts of the initial URL, base URL, and target URL.
    /// If `target_host` is None or matches `base_host`, it's considered same as base.
    /// Determine the domain relation given ParsedUrl references.
    pub fn get_by_urls(initial_url: &ParsedUrl, base_url: &ParsedUrl, target_url: &ParsedUrl) -> Self {
        Self::get_by_hosts(
            initial_url.host.as_deref(),
            base_url.host.as_deref(),
            target_url.host.as_deref(),
        )
    }

    /// Determine the domain relation given the hosts of the initial URL, base URL, and target URL.
    /// If `target_host` is None or matches `base_host`, it's considered same as base.
    pub fn get_by_hosts(initial_host: Option<&str>, base_host: Option<&str>, target_host: Option<&str>) -> Self {
        let initial = initial_host.unwrap_or("");
        let base = base_host.unwrap_or("");
        let target = target_host.unwrap_or("");

        if target.is_empty() || target == base {
            // base host is the same as target host
            if base == initial {
                TargetDomainRelation::InitialSameBaseSame
            } else {
                TargetDomainRelation::InitialDifferentBaseSame
            }
        } else {
            // base host is different from target host
            if target == initial {
                TargetDomainRelation::InitialSameBaseDifferent
            } else {
                TargetDomainRelation::InitialDifferentBaseDifferent
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::parsed_url::ParsedUrl;

    // =========================================================================
    // All 12 domain relation cases
    // =========================================================================

    // INITIAL_SAME__BASE_SAME
    #[test]
    fn initial_same_base_same_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://www.siteone.io/", None);
        let target = ParsedUrl::parse("/", Some(&base));
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialSameBaseSame
        );
    }

    #[test]
    fn initial_same_base_same_absolute() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://www.siteone.io/", None);
        let target = ParsedUrl::parse("https://www.siteone.io/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialSameBaseSame
        );
    }

    #[test]
    fn initial_same_base_same_protocol_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://www.siteone.io/", None);
        let target = ParsedUrl::parse("//www.siteone.io/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialSameBaseSame
        );
    }

    // INITIAL_SAME__BASE_DIFFERENT (backlink)
    #[test]
    fn initial_same_base_different_absolute() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("https://www.siteone.io/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialSameBaseDifferent
        );
    }

    #[test]
    fn initial_same_base_different_protocol_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("//www.siteone.io/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialSameBaseDifferent
        );
    }

    // INITIAL_DIFFERENT__BASE_SAME
    #[test]
    fn initial_different_base_same_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("/", Some(&base));
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseSame
        );
    }

    #[test]
    fn initial_different_base_same_absolute() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("https://nextjs.org/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseSame
        );
    }

    #[test]
    fn initial_different_base_same_protocol_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("//nextjs.org", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseSame
        );
    }

    // INITIAL_DIFFERENT__BASE_DIFFERENT
    #[test]
    fn initial_different_base_different_absolute() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("https://svelte.dev/", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseDifferent
        );
    }

    #[test]
    fn initial_different_base_different_protocol_relative() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://nextjs.org/", None);
        let target = ParsedUrl::parse("//svelte.dev", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseDifferent
        );
    }

    #[test]
    fn initial_different_base_different_same_initial_base() {
        let initial = ParsedUrl::parse("https://www.siteone.io/", None);
        let base = ParsedUrl::parse("https://www.siteone.io/", None);
        let target = ParsedUrl::parse("//svelte.dev", None);
        assert_eq!(
            TargetDomainRelation::get_by_urls(&initial, &base, &target),
            TargetDomainRelation::InitialDifferentBaseDifferent
        );
    }

    // Host-level tests (existing, kept)
    #[test]
    fn test_target_empty() {
        let result = TargetDomainRelation::get_by_hosts(Some("www.siteone.io"), Some("www.siteone.io"), None);
        assert_eq!(result, TargetDomainRelation::InitialSameBaseSame);
    }
}
