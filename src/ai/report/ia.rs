pub fn clean_source_title(page_title: &str, h1: &str, site_name: &str, host: &str) -> Option<String> {
    let source = if page_title.trim().is_empty() { h1 } else { page_title }.trim();
    if source.is_empty() {
        return None;
    }
    let mut cleaned = source.to_string();
    let mut suffixes = vec![
        site_name.trim().to_string(),
        host.trim().to_string(),
        host.trim_start_matches("www.").to_string(),
    ];
    suffixes.extend(
        host.trim_start_matches("www.")
            .split('.')
            .filter(|label| label.len() >= 2)
            .map(str::to_string),
    );
    suffixes.sort_by_key(|value| std::cmp::Reverse(value.chars().count()));
    suffixes.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    for suffix in suffixes.into_iter().filter(|value| !value.is_empty()) {
        for separator in [" | ", " - ", " — ", " – ", " :: ", " · "] {
            let target = format!("{}{}", separator, suffix);
            let lower = cleaned.to_lowercase();
            let target_lower = target.to_lowercase();
            if lower.ends_with(&target_lower) {
                let keep = cleaned.chars().count().saturating_sub(target.chars().count());
                cleaned = cleaned.chars().take(keep).collect::<String>().trim().to_string();
                break;
            }
        }
    }
    (!cleaned.is_empty()).then_some(cleaned)
}

pub fn normalize_description(description: &str) -> Result<String, String> {
    let trimmed = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = trimmed.chars().count();
    if count < 200 {
        return Err(format!("IA description has {} characters; expected 200-300", count));
    }
    if count <= 300 {
        return Ok(trimmed);
    }

    let chars = trimmed.chars().collect::<Vec<_>>();
    let sentence_end = (200..=300)
        .rev()
        .find(|index| matches!(chars[index - 1], '.' | '!' | '?'));
    let cut = sentence_end.or_else(|| (200..=300).rev().find(|index| chars[*index].is_whitespace()));
    let cut = cut.unwrap_or(300);
    let shortened = chars[..cut].iter().collect::<String>().trim().to_string();
    if shortened.chars().count() < 200 {
        return Err("IA description could not be shortened without falling below 200 characters".to_string());
    }
    Ok(shortened)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_title_removes_known_site_suffix() {
        assert_eq!(
            clean_source_title(
                "Installation guide | SiteOne Crawler",
                "Installation guide",
                "SiteOne Crawler",
                "crawler.siteone.io",
            )
            .as_deref(),
            Some("Installation guide")
        );
        assert_eq!(
            clean_source_title("", "Contact", "Example", "example.test").as_deref(),
            Some("Contact")
        );
        assert_eq!(
            clean_source_title("Apply online | Provident", "Apply online", "Loans", "provident.cz").as_deref(),
            Some("Apply online")
        );
    }

    #[test]
    fn description_rejects_short_and_shortens_long_at_boundary() {
        assert!(normalize_description("Too short.").is_err());
        let long = format!(
            "{} {}",
            "A factual sentence describing the page purpose and its role in the current website structure.".repeat(3),
            "Extra material that must be removed cleanly at a word boundary.".repeat(3)
        );
        let normalized = normalize_description(&long).unwrap();
        assert!((200..=300).contains(&normalized.chars().count()));
        assert!(!normalized.ends_with(' '));
    }
}
