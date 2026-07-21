// SiteOne Crawler - brand elaborate: navigation / footer link detection
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The crawl does not record which DOM region a link was found in, so we recover the site's global
// navigation POST-crawl by re-parsing a small sample of stored HTML bodies: links that appear in
// <nav>/<header>/<footer> across most sampled pages are the site-wide navigation the elaborate must
// prioritize (the owner's "main menu / footer links"). A sample suffices — chrome is near-identical
// across pages — so we never re-parse the whole crawl.

use std::collections::HashMap;

use scraper::{Html, Selector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavRegion {
    Nav,
    Footer,
}

#[derive(Debug, Clone)]
pub struct NavLink {
    pub url: String,
    pub anchor: String,
    pub region: NavRegion,
}

/// Extract internal chrome (nav/header/footer) links from one page's HTML, resolved to absolute
/// URLs on the same host as `base_url`. Fragment-only, `mailto:`/`tel:`, and external links dropped;
/// deduplicated by resolved URL (first region/anchor wins).
pub fn extract_chrome_links(html: &str, base_url: &str) -> Vec<NavLink> {
    let base = match url::Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    let doc = Html::parse_document(html);
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut out = Vec::new();

    for (selector, region) in [
        ("nav a[href], header a[href]", NavRegion::Nav),
        ("footer a[href]", NavRegion::Footer),
    ] {
        let sel = match Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for el in doc.select(&sel) {
            let href = match el.value().attr("href") {
                Some(h) => h.trim(),
                None => continue,
            };
            if href.is_empty() || href.starts_with('#') {
                continue;
            }
            let lower = href.to_ascii_lowercase();
            if lower.starts_with("mailto:") || lower.starts_with("tel:") || lower.starts_with("javascript:") {
                continue;
            }
            let resolved = match base.join(href) {
                Ok(u) => u,
                Err(_) => continue,
            };
            // Internal only (same host); drop the fragment for dedup/importance.
            if resolved.host_str() != base.host_str() {
                continue;
            }
            let mut norm = resolved.clone();
            norm.set_fragment(None);
            let key = norm.as_str().to_string();
            if seen.insert(key.clone(), ()).is_some() {
                continue;
            }
            let anchor = el
                .text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            out.push(NavLink {
                url: key,
                anchor,
                region,
            });
        }
    }
    out
}

/// Global navigation = links present in the chrome of at least 60 % of the sampled pages. Footer
/// classification wins only when a link appears exclusively in footers across the sample.
pub fn detect_global_nav(pages_html: &[(String, String)]) -> Vec<NavLink> {
    if pages_html.is_empty() {
        return Vec::new();
    }
    let threshold = ((pages_html.len() as f64) * 0.6).ceil() as usize;
    let threshold = threshold.max(1);

    // url -> (occurrence_count, anchor, saw_nav, saw_footer)
    let mut agg: HashMap<String, (usize, String, bool, bool)> = HashMap::new();
    for (url, html) in pages_html {
        for link in extract_chrome_links(html, url) {
            let entry = agg
                .entry(link.url.clone())
                .or_insert((0, link.anchor.clone(), false, false));
            entry.0 += 1;
            if entry.1.is_empty() {
                entry.1 = link.anchor.clone();
            }
            match link.region {
                NavRegion::Nav => entry.2 = true,
                NavRegion::Footer => entry.3 = true,
            }
        }
    }

    let mut out: Vec<NavLink> = agg
        .into_iter()
        .filter(|(_, (count, _, _, _))| *count >= threshold)
        .map(|(url, (_, anchor, saw_nav, _saw_footer))| NavLink {
            url,
            anchor,
            region: if saw_nav { NavRegion::Nav } else { NavRegion::Footer },
        })
        .collect();
    out.sort_by(|a, b| a.url.cmp(&b.url));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const P1: &str = r##"<html><body>
        <header><nav><a href="/o-nas">O nás</a><a href="/sluzby">Služby</a><a href="/produkty">Produkty</a></nav></header>
        <main><a href="/unikatni-clanek">jen tady</a> <a href="https://ext.example/x">ext</a> <a href="#top">frag</a></main>
        <footer><a href="/gdpr">GDPR</a><a href="mailto:a@b.cz">mail</a></footer>
    </body></html>"##;
    const P2: &str = r#"<html><body>
        <header><nav><a href="/o-nas">O nás</a><a href="/sluzby">Služby</a><a href="/produkty">Produkty</a></nav></header>
        <footer><a href="/gdpr">GDPR</a></footer>
    </body></html>"#;

    #[test]
    fn extract_chrome_links_internal_only_absolute_dedup() {
        let links = extract_chrome_links(P1, "https://x.test/some/page");
        let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
        assert!(urls.contains(&"https://x.test/o-nas"));
        assert!(urls.contains(&"https://x.test/gdpr"));
        assert!(!urls.iter().any(|u| u.contains("ext.example"))); // external dropped
        assert!(!urls.iter().any(|u| u.contains("mailto"))); // mailto dropped
        assert!(!urls.iter().any(|u| u.contains("unikatni"))); // main-content link not chrome
        assert_eq!(
            links.iter().find(|l| l.url.ends_with("/gdpr")).unwrap().region,
            NavRegion::Footer
        );
    }

    #[test]
    fn detect_global_nav_keeps_recurring_drops_unique() {
        let sample = vec![
            ("https://x.test/a".to_string(), P1.to_string()),
            ("https://x.test/b".to_string(), P2.to_string()),
        ];
        let nav = detect_global_nav(&sample);
        let urls: Vec<&str> = nav.iter().map(|l| l.url.as_str()).collect();
        assert!(urls.contains(&"https://x.test/o-nas"));
        assert!(urls.contains(&"https://x.test/sluzby"));
        assert!(urls.contains(&"https://x.test/produkty"));
        assert!(urls.contains(&"https://x.test/gdpr"));
        // Page-unique main link was never chrome and never recurred.
        assert!(!urls.iter().any(|u| u.contains("unikatni")));
    }
}
