#[derive(Debug, Clone)]
pub struct ReportLocale {
    code: String,
    czech: bool,
}

impl ReportLocale {
    pub fn new(value: &str) -> Self {
        let code = canonicalize(value);
        let czech = code == "cs" || code.starts_with("cs-");
        Self { code, czech }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn catalog_language(&self) -> &'static str {
        if self.czech { "cs" } else { "en" }
    }

    pub fn is_czech(&self) -> bool {
        self.czech
    }

    pub fn text(&self, key: &str) -> &'static str {
        if self.czech {
            match key {
                "ai_report" => "AI report",
                "export_csv" => "Exportovat CSV",
                "toggle_theme" => "Přepnout motiv",
                "pages_analyzed" => "Analyzované stránky",
                "fields" => "Pole",
                "parse_errors" => "Chyby zpracování",
                "findings" => "Nálezy",
                "high_critical" => "Vysoké/kritické",
                "pages_flagged" => "Stránky s nálezem",
                "avg_quality" => "Průměrná kvalita",
                "sections" => "Sekce",
                "pages" => "stránek",
                "coverage" => "Rozsah reportu",
                "crawled" => "Procházené HTML",
                "eligible" => "Způsobilé",
                "selected" => "Vybrané",
                "analyzed" => "Analyzované",
                "failed" => "Neúspěšné",
                "parse_failed" => "Neplatné odpovědi",
                "request_failed" => "Chyby požadavků",
                "task_dropped" => "Interní chyby úloh",
                "excluded" => "Vyloučené maskami",
                "capped" => "Mimo limit",
                "context_failed" => "Bez vstupního obsahu",
                "content_truncated" => "Zkrácený vstup",
                "indeterminate" => "Neúplné posouzení",
                "sampled_report" => "Výběrový report",
                "complete_report" => "Úplný report",
                "ranking" => "Způsob výběru",
                "include_masks" => "Zahrnuté masky",
                "exclude_masks" => "Vyloučené masky",
                "report_language" => "Jazyk reportu",
                "search_placeholder" => "Hledat ve stránkách...",
                "path" => "Cesta",
                "error" => "Chyba",
                "unknown" => "neznámé",
                "methodology" => "Metodika",
                "topic_analysis" => "Souhrnná tematická analýza",
                "clusters" => "Tematické klastry",
                "cannibalization" => "Kandidáti na kanibalizaci",
                "internal_gaps" => "Interní mezery v pokrytí",
                "thin_clusters" => "Slabě pokryté klastry",
                "no_candidates" => "Žádní kandidáti v analyzovaném vzorku.",
                "validated_findings" => "Nálezy s ověřeným výňatkem",
                "no_validated_findings" => {
                    "V dostupném textovém vstupu nebyly přijaty žádné nálezy s ověřeným výňatkem."
                }
                "not_legal_advice" => "Tento report je poradní podklad, nikoli právní stanovisko.",
                "results_advisory" => "Výsledky ověřte před přijetím opatření.",
                "ccd2_timing" => {
                    "Požadavky CCD2 jsou v tomto reportu hodnoceny jako příprava na budoucí stav; směrnice se použije od 20. listopadu 2026."
                }
                "may_timing" => {
                    "Položky označené MAY jsou možností členského státu; před označením za porušení ověřte příslušnou vnitrostátní implementaci."
                }
                "rule_pack" => "Sada pravidel",
                "risk_formula" => "Vzorec rizikového skóre",
                "applicability" => "Rozsah použitelnosti",
                "review_status" => "Stav právního ověření",
                "primary_sources" => "Primární zdroje",
                "evidence_limited" => "Omezené důkazy",
                "evidence_complete" => "Úplný zachovaný text",
                "page" => "Stránka",
                "severity" => "Závažnost",
                "category" => "Kategorie",
                "rule" => "Pravidlo",
                "legal_basis" => "Právní základ",
                "obligation" => "Povinnost",
                "status" => "Stav",
                "effective_date" => "Účinnost",
                "source" => "Zdroj",
                "excerpt" => "Důkazní citace",
                "recommendation" => "Doporučení",
                _ => "",
            }
        } else {
            match key {
                "ai_report" => "AI Report",
                "export_csv" => "Export CSV",
                "toggle_theme" => "Toggle theme",
                "pages_analyzed" => "Pages analyzed",
                "fields" => "Fields",
                "parse_errors" => "Processing errors",
                "findings" => "Findings",
                "high_critical" => "High/critical",
                "pages_flagged" => "Pages flagged",
                "avg_quality" => "Avg quality",
                "sections" => "Sections",
                "pages" => "pages",
                "coverage" => "Report coverage",
                "crawled" => "Crawled HTML",
                "eligible" => "Eligible",
                "selected" => "Selected",
                "analyzed" => "Analyzed",
                "failed" => "Failed",
                "parse_failed" => "Invalid responses",
                "request_failed" => "Request failures",
                "task_dropped" => "Internal task failures",
                "excluded" => "Excluded by masks",
                "capped" => "Dropped by cap",
                "context_failed" => "Missing input content",
                "content_truncated" => "Truncated input",
                "indeterminate" => "Indeterminate assessment",
                "sampled_report" => "Sampled report",
                "complete_report" => "Complete report",
                "ranking" => "Selection method",
                "include_masks" => "Include masks",
                "exclude_masks" => "Exclude masks",
                "report_language" => "Report language",
                "search_placeholder" => "Search pages...",
                "path" => "Path",
                "error" => "Error",
                "unknown" => "unknown",
                "methodology" => "Methodology",
                "topic_analysis" => "Site-wide topic analysis",
                "clusters" => "Topic clusters",
                "cannibalization" => "Cannibalization candidates",
                "internal_gaps" => "Internal coverage gaps",
                "thin_clusters" => "Thin clusters",
                "no_candidates" => "No candidates in the analyzed sample.",
                "validated_findings" => "Findings with grounded excerpts",
                "no_validated_findings" => {
                    "No findings with grounded excerpts were accepted from the available text input."
                }
                "not_legal_advice" => "This report is advisory and does not constitute legal advice.",
                "results_advisory" => "Verify results before acting.",
                "ccd2_timing" => {
                    "CCD2 requirements are assessed as forward-looking readiness in this report; the Directive applies from 20 November 2026."
                }
                "may_timing" => {
                    "Items marked MAY are Member-State options; check the relevant national implementation before treating one as a breach."
                }
                "rule_pack" => "Rule pack",
                "risk_formula" => "Risk-score formula",
                "applicability" => "Applicability scope",
                "review_status" => "Legal-review status",
                "primary_sources" => "Primary sources",
                "evidence_limited" => "Limited evidence",
                "evidence_complete" => "Complete retained text",
                "page" => "Page",
                "severity" => "Severity",
                "category" => "Category",
                "rule" => "Rule",
                "legal_basis" => "Legal basis",
                "obligation" => "Obligation",
                "status" => "Status",
                "effective_date" => "Effective",
                "source" => "Source",
                "excerpt" => "Evidence excerpt",
                "recommendation" => "Recommendation",
                _ => "",
            }
        }
    }

    pub fn preset_title(&self, key: &str) -> &'static str {
        match (self.czech, key) {
            (true, "ia") => "Inventura informační architektury",
            (true, "quality") => "Kvalita a čitelnost obsahu",
            (true, "topics") => "Mapa témat a obsahových mezer",
            (true, "compliance") => "Regulatorní audit a manipulativní vzorce",
            (true, "extract") => "Vlastní AI extrakce",
            (false, "ia") => "Information Architecture Inventory",
            (false, "quality") => "Content Quality & Readability",
            (false, "topics") => "Topic & Content-Gap Map",
            (false, "compliance") => "Regulatory & Dark-Pattern Audit",
            _ => "Custom AI Extract",
        }
    }

    pub fn field_label(&self, key: &str) -> String {
        let translated = if self.czech {
            match key {
                "title" => "Název",
                "description" => "Popis",
                "section" => "Sekce",
                "pageType" => "Typ stránky",
                "primaryEntity" => "Hlavní entita",
                "overall" => "Celkem",
                "clarity" => "Srozumitelnost",
                "depth" => "Hloubka",
                "engagement" => "Poutavost",
                "gradeLevel" => "Úroveň čtení",
                "tone" => "Tón",
                "wordCount" => "Počet slov",
                "topIssue" => "Hlavní problém",
                "primaryTopic" => "Hlavní téma",
                "topicCluster" => "Tematický klastr",
                "entities" => "Entity",
                "keywords" => "Klíčová slova",
                "searchIntent" => "Záměr hledání",
                "funnelStage" => "Fáze funnelu",
                "targetAudience" => "Cílové publikum",
                "riskScore" => "Rizikové skóre",
                "findings" => "Nálezy",
                _ => return humanize_key(key),
            }
        } else {
            return humanize_key(key);
        };
        translated.to_string()
    }

    /// Localized explanatory text for built-in report fields. Custom schema descriptions are
    /// user-authored and must remain untouched.
    pub fn field_description(&self, key: &str) -> Option<&'static str> {
        if !self.czech {
            return None;
        }
        match key {
            "title" => Some(
                "Název stránky odvozený crawlerem z title/H1 bez opakujícího se názvu webu; null pouze tehdy, když chybějí oba zdroje.",
            ),
            "description" => Some(
                "Neutrální popis obsahu a role stránky v rozsahu 200-300 znaků, bez marketingových frází a obecných úvodů.",
            ),
            "section" => Some("Nejvýstižnější hlavní sekce, do které stránka patří."),
            "pageType" => Some("Strukturální typ stránky."),
            "primaryEntity" => Some("Hlavní téma, produkt, místo nebo jiná entita stránky; jinak null."),
            "overall" => Some("Celková kvalita obsahu od 0 (nízká) do 100 (výborná)."),
            "clarity" => Some("Srozumitelnost a snadnost porozumění textu (0-100)."),
            "depth" => Some("Hloubka a věcná úplnost obsahu vzhledem k jeho účelu (0-100)."),
            "engagement" => Some("Poutavost textu pro cílové publikum (0-100)."),
            "gradeLevel" => {
                Some("Přibližná úroveň obtížnosti čtení podle amerických školních ročníků; nižší je snazší.")
            }
            "tone" => Some("Jeden až tři stručné popisy tónu textu."),
            "wordCount" => Some("Přibližný počet viditelných slov v hlavním obsahu."),
            "topIssue" => {
                Some("Nejdůležitější konkrétní doporučení v jedné větě; null, pokud je stránka již kvalitní.")
            }
            "primaryTopic" => Some("Jedno hlavní téma stránky vyjádřené krátkým slovním spojením."),
            "topicCluster" => Some("Širší tematický klastr, do kterého stránka patří."),
            "entities" => Some("Nejvýše šest hlavních entit nebo pojmů obsažených na stránce."),
            "keywords" => Some("Nejvýše šest relevantních cílových klíčových slov nebo frází."),
            "searchIntent" => Some("Převládající záměr hledání, kterému stránka slouží."),
            "funnelStage" => Some("Fáze marketingového funnelu: TOFU, MOFU nebo BOFU."),
            "targetAudience" => Some("Hlavní cílové publikum stránky."),
            "riskScore" => Some(
                "Riziko compliance nebo manipulativních vzorců od 0 do 100; crawler výslednou hodnotu vypočítá z ověřených nálezů.",
            ),
            "findings" => Some(
                "Závažné problémy s compliance nebo manipulativními vzorci nalezené na této stránce; čistá stránka má prázdné pole.",
            ),
            _ => None,
        }
    }

    /// Human-facing translations for stable enum/category codes kept unchanged in JSON.
    pub fn value_label(&self, field: &str, value: &str) -> String {
        if !self.czech {
            return value.replace('_', " ");
        }
        let translated = match (field, value) {
            ("section", "Home") => "Domů",
            ("section", "Product") => "Produkt",
            ("section", "Service") => "Služba",
            ("section", "Pricing") => "Ceník",
            ("section", "Docs") => "Dokumentace",
            ("section", "Support") => "Podpora",
            ("section", "About") => "O nás",
            ("section", "Contact") => "Kontakt",
            ("section", "Legal") => "Právní",
            ("section", "Career") => "Kariéra",
            ("section", "Account") => "Účet",
            ("section", "Other") => "Ostatní",
            ("pageType", "landing") => "vstupní stránka",
            ("pageType", "article") => "článek",
            ("pageType", "listing") => "přehled",
            ("pageType", "detail") => "detail",
            ("pageType", "form") => "formulář",
            ("pageType", "legal") => "právní stránka",
            ("pageType", "utility") => "obslužná stránka",
            ("pageType", "error") => "chybová stránka",
            ("searchIntent", "informational") => "informační",
            ("searchIntent", "navigational") => "navigační",
            ("searchIntent", "commercial") => "komerční",
            ("searchIntent", "transactional") => "transakční",
            ("category", "advertising_claims") => "reklamní tvrzení",
            ("category", "dark_pattern") => "manipulativní vzorec",
            _ => return value.replace('_', " "),
        };
        translated.to_string()
    }
}

fn canonicalize(value: &str) -> String {
    let raw = value.trim().replace('_', "-");
    if raw.is_empty()
        || raw
            .split('-')
            .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_alphanumeric()))
    {
        return "en".to_string();
    }
    raw.split('-')
        .enumerate()
        .map(|(index, part)| {
            if index > 0 && part.len() == 2 {
                part.to_ascii_uppercase()
            } else {
                part.to_ascii_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

pub fn is_valid_language_tag(value: &str) -> bool {
    let raw = value.trim().replace('_', "-");
    !raw.is_empty()
        && raw.len() <= 35
        && raw
            .split('-')
            .all(|part| !part.is_empty() && part.len() <= 8 && part.chars().all(|ch| ch.is_ascii_alphanumeric()))
        && raw.split('-').next().is_some_and(|language| {
            (2..=8).contains(&language.len()) && language.chars().all(|ch| ch.is_ascii_alphabetic())
        })
}

fn humanize_key(key: &str) -> String {
    let mut out = String::new();
    for (index, ch) in key.chars().enumerate() {
        if index > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        if index == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn czech_catalog_and_bcp47_canonicalization() {
        let locale = ReportLocale::new("cs_cz");
        assert_eq!(locale.code(), "cs-CZ");
        assert_eq!(locale.catalog_language(), "cs");
        assert_eq!(
            locale.preset_title("compliance"),
            "Regulatorní audit a manipulativní vzorce"
        );
        assert_eq!(locale.field_label("riskScore"), "Rizikové skóre");
        assert!(locale.field_description("riskScore").unwrap().contains("crawler"));
        assert_eq!(locale.value_label("section", "Product"), "Produkt");
        assert_eq!(locale.value_label("category", "advertising_claims"), "reklamní tvrzení");
    }

    #[test]
    fn unsupported_chrome_locale_uses_english_catalog_but_keeps_code() {
        let locale = ReportLocale::new("de-DE");
        assert_eq!(locale.code(), "de-DE");
        assert_eq!(locale.catalog_language(), "en");
        assert_eq!(locale.text("coverage"), "Report coverage");
        assert!(is_valid_language_tag("de-DE"));
        assert!(!is_valid_language_tag("not a language"));
    }
}
