// SiteOne Crawler - AI profile: prompt-pack file parser
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Parses the embedded prompt files (types/*.md and chapters/**/*.md) into typed specs and renders
// `{{placeholder}}` tokens in the shared prompt templates. On-disk formats:
//
//   types/<key>.md:
//     ---
//     name_cs: <display name>
//     name_en: <display name>
//     ---
//     <classifier description, one or more paragraphs>
//
//   chapters/<key>/<id>.md:
//     ---
//     heading: <English heading>
//     target_chars: <int>
//     max_pages: <int>
//     ---
//     ## selection_focus
//     <text>
//
//     ## synthesis_instructions
//     <text>

#[derive(Debug, Clone)]
pub struct ChapterSpec {
    pub id: String,
    pub heading: String,
    pub target_chars: usize,
    pub max_pages: usize,
    pub selection_focus: String,
    pub synthesis_instructions: String,
}

#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub key: String,
    pub name_cs: String,
    pub name_en: String,
    pub classifier_description: String,
    pub chapters: Vec<ChapterSpec>,
}

/// Split a `---\n<front matter>\n---\n<body>` document into (front_matter_lines, body). Errors if
/// the leading fence is missing.
fn split_front_matter(raw: &str) -> Result<(Vec<(String, String)>, String), String> {
    let text = raw.replace("\r\n", "\n");
    let text = text
        .strip_prefix("---\n")
        .ok_or("missing leading '---' front matter fence")?;
    let end = text.find("\n---\n").ok_or("missing closing '---' front matter fence")?;
    let (fm, rest) = text.split_at(end);
    let body = rest.strip_prefix("\n---\n").unwrap_or("").to_string();
    let mut pairs = Vec::new();
    for line in fm.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| format!("bad front-matter line: {line}"))?;
        pairs.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok((pairs, body))
}

fn fm_get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

pub fn parse_type(key: &str, raw: &str) -> Result<(String, String, String), String> {
    let (fm, body) = split_front_matter(raw).map_err(|e| format!("type '{key}': {e}"))?;
    let name_cs = fm_get(&fm, "name_cs").ok_or_else(|| format!("type '{key}': missing name_cs"))?;
    let name_en = fm_get(&fm, "name_en").ok_or_else(|| format!("type '{key}': missing name_en"))?;
    let desc = body.trim();
    if desc.is_empty() {
        return Err(format!("type '{key}': empty classifier_description"));
    }
    Ok((name_cs.to_string(), name_en.to_string(), desc.to_string()))
}

/// Extract the body under a `## <name>` heading up to the next `## ` heading or end.
fn section_body(body: &str, name: &str) -> Option<String> {
    let needle = format!("## {name}");
    let start = body.find(&needle)? + needle.len();
    let after = &body[start..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    let end = after.find("\n## ").unwrap_or(after.len());
    Some(after[..end].trim().to_string())
}

pub fn parse_chapter(id: &str, raw: &str) -> Result<ChapterSpec, String> {
    let (fm, body) = split_front_matter(raw).map_err(|e| format!("chapter '{id}': {e}"))?;
    let heading = fm_get(&fm, "heading").ok_or_else(|| format!("chapter '{id}': missing heading"))?;
    let target_chars = fm_get(&fm, "target_chars")
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or_else(|| format!("chapter '{id}': missing/invalid target_chars"))?;
    let max_pages = fm_get(&fm, "max_pages")
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or_else(|| format!("chapter '{id}': missing/invalid max_pages"))?;
    let selection_focus =
        section_body(&body, "selection_focus").ok_or_else(|| format!("chapter '{id}': missing ## selection_focus"))?;
    let synthesis_instructions = section_body(&body, "synthesis_instructions")
        .ok_or_else(|| format!("chapter '{id}': missing ## synthesis_instructions"))?;
    if selection_focus.is_empty() || synthesis_instructions.is_empty() {
        return Err(format!(
            "chapter '{id}': empty selection_focus or synthesis_instructions"
        ));
    }
    Ok(ChapterSpec {
        id: id.to_string(),
        heading: heading.to_string(),
        target_chars: target_chars.clamp(500, 20_000),
        max_pages: max_pages.clamp(1, 60),
        selection_focus,
        synthesis_instructions,
    })
}

/// Replace every `{{key}}` token in `template` with the matching value. Unknown tokens are left
/// as-is (a registry test asserts no `{{` survives in rendered shared prompts).
pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_type_file() {
        let raw = "---\nname_cs: Osobní weby\nname_en: Personal sites\n---\nChoose this type when the subject is one person. NOT this type: a company.\n";
        let (cs, en, desc) = parse_type("personal", raw).unwrap();
        assert_eq!(cs, "Osobní weby");
        assert_eq!(en, "Personal sites");
        assert!(desc.contains("one person"));
    }

    #[test]
    fn parses_a_chapter_file() {
        let raw = "---\nheading: Services & specializations\ntarget_chars: 9000\nmax_pages: 20\n---\n## selection_focus\nSelect every services page.\n\n## synthesis_instructions\nDescribe the services. Omit gracefully when unsupported.\n";
        let c = parse_chapter("03-services", raw).unwrap();
        assert_eq!(c.id, "03-services");
        assert_eq!(c.heading, "Services & specializations");
        assert_eq!(c.target_chars, 9000);
        assert_eq!(c.max_pages, 20);
        assert!(c.selection_focus.starts_with("Select every"));
        assert!(c.synthesis_instructions.contains("Omit gracefully"));
    }

    #[test]
    fn missing_fences_and_fields_error() {
        assert!(parse_type("x", "no front matter").is_err());
        assert!(parse_chapter("x", "---\nheading: H\n---\n## selection_focus\nonly one section\n").is_err());
    }

    #[test]
    fn render_substitutes_known_tokens_only() {
        let out = render(
            "Focus: {{chapter_focus}}; budget {{budget_chars}}. {{unknown}}",
            &[("chapter_focus", "services"), ("budget_chars", "200000")],
        );
        assert_eq!(out, "Focus: services; budget 200000. {{unknown}}");
    }
}
