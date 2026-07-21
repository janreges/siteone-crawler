use std::collections::BTreeMap;

use unicode_normalization::UnicodeNormalization;

use super::locale::ReportLocale;
use super::model::{Finding, ReportCell};

pub const RULE_PACK_VERSION: &str = "eu-credit-fairness-2026-07-21";
pub const CCD2_SOURCE_URL: &str = "https://eur-lex.europa.eu/eli/dir/2023/2225/oj";
pub const UCPD_SOURCE_URL: &str = "https://eur-lex.europa.eu/eli/dir/2005/29/oj";

#[derive(Debug, Clone, Copy)]
pub struct ComplianceRule {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub legal_basis: &'static str,
    pub obligation: &'static str,
    pub legal_status: &'static str,
    pub effective_date: &'static str,
    pub baseline_severity: &'static str,
    pub source_url: &'static str,
    pub description: &'static str,
    pub absence_check: bool,
}

const CCD2_STATUS: &str = "Forward-looking CCD2 readiness; Directive applies from 2026-11-20";
const CCD2_MAY_STATUS: &str = "CCD2 Member-State option; applicability depends on the relevant national implementation";
const CURRENT_UCPD_STATUS: &str = "Current EU unfair-commercial-practices framework";

pub const RULES: &[ComplianceRule] = &[
    ComplianceRule {
        id: "false_availability",
        category: "advertising_claims",
        title: "False expectation of credit availability or approval",
        legal_basis: "CCD2 Article 7",
        obligation: "SHALL",
        legal_status: CCD2_STATUS,
        effective_date: "2026-11-20",
        baseline_severity: "high",
        source_url: CCD2_SOURCE_URL,
        description: "Claims or implies guaranteed, universal, or instant credit availability without a real assessment.",
        absence_check: false,
    },
    ComplianceRule {
        id: "cost_downplay",
        category: "advertising_claims",
        title: "Affirmative downplaying of borrowing cost",
        legal_basis: "UCPD Articles 6-7; CCD2 Article 7 where wording misleads on cost or total payable",
        obligation: "SHALL",
        legal_status: "Current UCPD duty plus forward-looking CCD2 readiness",
        effective_date: "Current; CCD2 applies 2026-11-20",
        baseline_severity: "high",
        source_url: UCPD_SOURCE_URL,
        description: "Affirmatively frames borrowing as free, negligible, or harmless in a way that hides or trivializes material cost. Do not use merely because standard information is omitted.",
        absence_check: false,
    },
    ComplianceRule {
        id: "incomplete_standard_info",
        category: "advertising_claims",
        title: "Incomplete standard information in a numbered credit advertisement",
        legal_basis: "CCD2 Article 8(2)-(4)",
        obligation: "SHALL",
        legal_status: CCD2_STATUS,
        effective_date: "2026-11-20",
        baseline_severity: "medium",
        source_url: CCD2_SOURCE_URL,
        description: "A credit advertisement states an interest rate or cost figure but the inspected advertisement lacks the required representative standard-information set.",
        absence_check: true,
    },
    ComplianceRule {
        id: "missing_warning",
        category: "advertising_claims",
        title: "Missing borrowing-cost warning",
        legal_basis: "CCD2 Article 8(1)",
        obligation: "SHALL",
        legal_status: CCD2_STATUS,
        effective_date: "2026-11-20",
        baseline_severity: "medium",
        source_url: CCD2_SOURCE_URL,
        description: "all advertising concerning credit agreements must include a clear and prominent borrowing-cost warning; quote the positive credit-advertising trigger as evidence.",
        absence_check: true,
    },
    ComplianceRule {
        id: "improves_finances",
        category: "advertising_claims",
        title: "Credit presented as improving finances or living standards",
        legal_basis: "CCD2 Article 8(7)(a) and (c)",
        obligation: "SHALL",
        legal_status: CCD2_STATUS,
        effective_date: "2026-11-20",
        baseline_severity: "high",
        source_url: CCD2_SOURCE_URL,
        description: "Suggests credit improves finances, substitutes for savings, or raises the standard of living.",
        absence_check: false,
    },
    ComplianceRule {
        id: "ignore_register",
        category: "advertising_claims",
        title: "Creditworthiness checks or registers downplayed",
        legal_basis: "CCD2 Article 8(7)(b)",
        obligation: "SHALL",
        legal_status: CCD2_STATUS,
        effective_date: "2026-11-20",
        baseline_severity: "high",
        source_url: CCD2_SOURCE_URL,
        description: "Downplays creditworthiness assessment or implies adverse registers, debt, or enforcement do not matter.",
        absence_check: false,
    },
    ComplianceRule {
        id: "risk_downplay",
        category: "advertising_claims",
        title: "Borrowing risk downplayed",
        legal_basis: "CCD2 Article 7; UCPD Articles 6-7",
        obligation: "SHALL",
        legal_status: "Current UCPD framework plus forward-looking CCD2 Article 7 readiness",
        effective_date: "Current; CCD2 applies 2026-11-20",
        baseline_severity: "medium",
        source_url: UCPD_SOURCE_URL,
        description: "Trivializes repayment, indebtedness, or the financial risk of borrowing.",
        absence_check: false,
    },
    ComplianceRule {
        id: "ease_speed",
        category: "advertising_claims",
        title: "Ease or speed of obtaining credit emphasized",
        legal_basis: "CCD2 Article 8(8)(a)",
        obligation: "MAY",
        legal_status: CCD2_MAY_STATUS,
        effective_date: "Subject to Member-State implementation",
        baseline_severity: "medium",
        source_url: CCD2_SOURCE_URL,
        description: "Highlights how easy or fast it is to obtain credit, such as a few minutes, a few clicks, or no paperwork.",
        absence_check: false,
    },
    ComplianceRule {
        id: "conditional_discount",
        category: "advertising_claims",
        title: "Discount conditional on taking credit",
        legal_basis: "CCD2 Article 8(8)(b)",
        obligation: "MAY",
        legal_status: CCD2_MAY_STATUS,
        effective_date: "Subject to Member-State implementation",
        baseline_severity: "medium",
        source_url: CCD2_SOURCE_URL,
        description: "States that a discount, bonus, or preferential purchase condition depends on taking credit.",
        absence_check: false,
    },
    ComplianceRule {
        id: "grace_period",
        category: "advertising_claims",
        title: "Extended instalment grace period advertised",
        legal_basis: "CCD2 Article 8(8)(c)",
        obligation: "MAY",
        legal_status: CCD2_MAY_STATUS,
        effective_date: "Subject to Member-State implementation",
        baseline_severity: "low",
        source_url: CCD2_SOURCE_URL,
        description: "Offers a grace period longer than three months for instalment repayment.",
        absence_check: false,
    },
    ComplianceRule {
        id: "urgency_scarcity",
        category: "dark_pattern",
        title: "Unsubstantiated urgency or scarcity pressure",
        legal_basis: "UCPD Articles 5-9",
        obligation: "SHALL",
        legal_status: CURRENT_UCPD_STATUS,
        effective_date: "Current",
        baseline_severity: "medium",
        source_url: UCPD_SOURCE_URL,
        description: "Uses textual urgency, fake deadlines, or unsubstantiated scarcity to pressure a decision.",
        absence_check: false,
    },
    ComplianceRule {
        id: "forced_continuity",
        category: "dark_pattern",
        title: "Forced continuity or hidden renewal",
        legal_basis: "UCPD Articles 5-9",
        obligation: "SHALL",
        legal_status: CURRENT_UCPD_STATUS,
        effective_date: "Current",
        baseline_severity: "medium",
        source_url: UCPD_SOURCE_URL,
        description: "Text evidences hidden automatic renewal, continuity, or materially obstructed cancellation.",
        absence_check: false,
    },
    ComplianceRule {
        id: "confirm_shaming",
        category: "dark_pattern",
        title: "Confirm-shaming choice wording",
        legal_basis: "UCPD Articles 5-9",
        obligation: "SHALL",
        legal_status: CURRENT_UCPD_STATUS,
        effective_date: "Current",
        baseline_severity: "low",
        source_url: UCPD_SOURCE_URL,
        description: "Uses guilt, shame, or disparaging choice text to steer the user.",
        absence_check: false,
    },
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ComplianceValidation {
    pub evidence_limited: bool,
    pub indeterminate_rules: Vec<String>,
}

pub fn rule_by_id(id: &str) -> Option<&'static ComplianceRule> {
    RULES.iter().find(|rule| rule.id == id)
}

pub fn rules_prompt() -> String {
    let mut prompt = format!(
        "<rule_pack version=\"{}\">\nOnly the rules below are supported by the supplied text evidence. SHALL is an EU requirement; MAY is a Member-State option and must not be presented as a binding breach without checking national implementation. CCD2 rules apply only to advertising for an in-scope consumer credit agreement; the crawler cannot establish jurisdiction or Article 2 exclusions from page text alone.\n",
        RULE_PACK_VERSION
    );
    for rule in RULES {
        prompt.push_str(&format!(
            "- {} / {} / {} [{}; baseline {}] — {} Basis: {}. Status: {}.\n",
            rule.category,
            rule.id,
            rule.title,
            rule.obligation,
            rule.baseline_severity,
            rule.description,
            rule.legal_basis,
            rule.legal_status
        ));
    }
    prompt.push_str(&format!(
        "Primary sources: CCD2 {} ; UCPD {}.\n</rule_pack>",
        CCD2_SOURCE_URL, UCPD_SOURCE_URL
    ));
    prompt
}

pub fn validate_cells(
    cells: &mut BTreeMap<String, ReportCell>,
    evidence: &str,
    input_truncated: bool,
    report_language: &str,
) -> Result<ComplianceValidation, String> {
    let findings = match cells.get("findings") {
        Some(ReportCell::Findings(items)) => items.clone(),
        _ => return Err("compliance response is missing a structurally valid findings array".to_string()),
    };
    let provisional_score = match cells.get("riskScore") {
        Some(ReportCell::Num(score)) => Some(*score),
        Some(ReportCell::Null) if input_truncated => None,
        _ => {
            return Err(
                "compliance response needs a numeric riskScore (null is allowed only for truncated input)".to_string(),
            );
        }
    };
    if findings.is_empty() && provisional_score.is_some_and(|score| score != 0.0) {
        return Err("a clean compliance response must explicitly return riskScore 0 with findings []".to_string());
    }
    if findings.is_empty() && !input_truncated && provisional_score != Some(0.0) {
        return Err("a clean compliance response must explicitly return riskScore 0 with findings []".to_string());
    }

    let mut outcome = ComplianceValidation {
        evidence_limited: input_truncated,
        indeterminate_rules: if input_truncated {
            RULES
                .iter()
                .filter(|rule| rule.absence_check)
                .map(|rule| rule.id.to_string())
                .collect()
        } else {
            Vec::new()
        },
    };
    let mut ordered = findings;
    let czech = ReportLocale::new(report_language).is_czech();
    ordered.sort_by_key(|finding| overlap_priority(&finding.rule));
    let normalized_evidence = normalize_evidence(evidence);
    let mut seen: Vec<(&str, String)> = Vec::new();
    let mut validated = Vec::new();

    for mut finding in ordered {
        let rule = rule_by_id(&finding.rule).ok_or_else(|| format!("unknown compliance rule '{}'", finding.rule))?;
        if finding.category != rule.category {
            return Err(format!(
                "rule '{}' belongs to category '{}', not '{}'",
                finding.rule, rule.category, finding.category
            ));
        }
        let normalized_excerpt = normalize_evidence(&finding.excerpt);
        if normalized_excerpt.chars().count() < 12 || !normalized_evidence.contains(&normalized_excerpt) {
            return Err(format!(
                "finding '{}' excerpt is not grounded in the supplied page evidence",
                finding.rule
            ));
        }
        if input_truncated && rule.absence_check {
            outcome.indeterminate_rules.push(rule.id.to_string());
            continue;
        }

        let dedupe_rule = if matches!(rule.id, "cost_downplay" | "incomplete_standard_info") {
            "cost_presentation"
        } else {
            rule.id
        };
        if seen.iter().any(|(group, excerpt)| {
            *group == dedupe_rule
                && (excerpt == &normalized_excerpt
                    || excerpt.contains(&normalized_excerpt)
                    || normalized_excerpt.contains(excerpt))
        }) {
            continue;
        }
        seen.push((dedupe_rule, normalized_excerpt));

        finding.title = localized_title(rule, czech).to_string();
        finding.severity = rule.baseline_severity.to_string();
        finding.legal_basis = if czech {
            rule.legal_basis
                .replace("Articles", "články")
                .replace("Article", "článek")
        } else {
            rule.legal_basis.to_string()
        };
        finding.obligation = rule.obligation.to_string();
        finding.legal_status = localized_status(rule, czech).to_string();
        finding.effective_date = localized_effective_date(rule, czech).to_string();
        finding.source_url = rule.source_url.to_string();
        finding.source_urls = source_urls(rule).into_iter().map(str::to_string).collect();
        finding.applicability = localized_applicability(rule, czech).to_string();
        finding.evidence_scope = if input_truncated && czech {
            "Výňatek odpovídá zachovanému textovému vstupu; text stránky byl zkrácen, proto nelze vyhodnotit nepřítomnost povinných prvků. Vizuální významnost, interakce a síťové chování nebyly posuzovány."
                .to_string()
        } else if input_truncated {
            "The excerpt matches retained text input; the page text was truncated, so absence checks are indeterminate. Visual prominence, interactions, and network behavior were not assessed."
                .to_string()
        } else if czech {
            "Výňatek odpovídá úplnému zachovanému textovému vstupu; vizuální významnost, interakce a síťové chování nebyly posuzovány."
                .to_string()
        } else {
            "The excerpt matches the complete retained text input; visual prominence, interactions, and network behavior were not assessed."
                .to_string()
        };
        validated.push(finding);
    }

    outcome.indeterminate_rules.sort();
    outcome.indeterminate_rules.dedup();
    let score = risk_score(&validated);
    cells.insert("findings".to_string(), ReportCell::Findings(validated));
    if input_truncated && score == 0.0 {
        cells.insert("riskScore".to_string(), ReportCell::Null);
    } else {
        cells.insert("riskScore".to_string(), ReportCell::Num(score));
    }
    Ok(outcome)
}

fn localized_title(rule: &ComplianceRule, czech: bool) -> &'static str {
    if !czech {
        return rule.title;
    }
    match rule.id {
        "false_availability" => "Falešné očekávání dostupnosti nebo schválení úvěru",
        "cost_downplay" => "Zlehčování nákladů na úvěr",
        "incomplete_standard_info" => "Neúplné standardní informace v číselné reklamě na úvěr",
        "missing_warning" => "Chybějící upozornění na náklady úvěru",
        "improves_finances" => "Úvěr prezentovaný jako zlepšení financí nebo životní úrovně",
        "ignore_register" => "Zlehčování posouzení úvěruschopnosti nebo registrů",
        "risk_downplay" => "Zlehčování rizik půjčky",
        "ease_speed" => "Zdůraznění snadnosti nebo rychlosti získání úvěru",
        "conditional_discount" => "Sleva podmíněná sjednáním úvěru",
        "grace_period" => "Propagace prodlouženého odkladu splátek",
        "urgency_scarcity" => "Nepodložený nátlak naléhavostí nebo nedostatkem",
        "forced_continuity" => "Vynucené pokračování nebo skryté obnovení",
        "confirm_shaming" => "Manipulativní zahanbování volby",
        _ => rule.title,
    }
}

fn localized_status(rule: &ComplianceRule, czech: bool) -> &'static str {
    if !czech {
        return rule.legal_status;
    }
    if rule.obligation == "MAY" {
        "Možnost členského státu podle CCD2; použitelnost závisí na příslušné národní implementaci"
    } else if rule.category == "dark_pattern" {
        "Aktuální rámec EU pro nekalé obchodní praktiky"
    } else if rule.id == "cost_downplay" {
        "Aktuální povinnost podle UCPD a příprava na CCD2"
    } else if rule.id == "risk_downplay" {
        "Aktuální rámec UCPD a příprava na článek 7 CCD2"
    } else {
        "Příprava na CCD2; směrnice se použije od 20. 11. 2026"
    }
}

fn localized_applicability(rule: &ComplianceRule, czech: bool) -> &'static str {
    if rule.category == "dark_pattern" {
        if czech {
            "Textový signál podle rámce UCPD; právní závěr závisí na kontextu celé praktiky."
        } else {
            "Textual signal under the UCPD framework; a legal conclusion depends on the full practice context."
        }
    } else if czech {
        "Pouze reklama na spotřebitelský úvěr spadající do působnosti CCD2; crawler neurčuje jurisdikci ani výjimky podle článku 2."
    } else {
        "Only advertising for consumer credit within CCD2 scope; the crawler does not determine jurisdiction or Article 2 exclusions."
    }
}

fn source_urls(rule: &ComplianceRule) -> Vec<&'static str> {
    if matches!(rule.id, "cost_downplay" | "risk_downplay") {
        vec![UCPD_SOURCE_URL, CCD2_SOURCE_URL]
    } else {
        vec![rule.source_url]
    }
}

fn localized_effective_date(rule: &ComplianceRule, czech: bool) -> &'static str {
    if !czech {
        return rule.effective_date;
    }
    match rule.effective_date {
        "Subject to Member-State implementation" => "Podle implementace členského státu",
        "Current" => "Platné nyní",
        "Current; CCD2 applies 2026-11-20" => "Platné nyní; CCD2 se použije od 20. 11. 2026",
        value => value,
    }
}

fn overlap_priority(rule: &str) -> u8 {
    match rule {
        "cost_downplay" => 0,
        "incomplete_standard_info" => 1,
        _ => 0,
    }
}

fn risk_score(findings: &[Finding]) -> f64 {
    findings
        .iter()
        // A Member-State option remains visible as a readiness observation, but must not inflate
        // a score that recipients may otherwise read as binding legal risk.
        .filter(|finding| finding.obligation != "MAY")
        .map(|finding| match finding.severity.as_str() {
            "critical" => 40.0,
            "high" => 25.0,
            "medium" => 12.0,
            "low" => 5.0,
            _ => 2.0,
        })
        .sum::<f64>()
        .min(100.0)
}

fn normalize_evidence(value: &str) -> String {
    let decoded = value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let mut normalized = String::with_capacity(decoded.len());
    let mut pending_space = false;
    for ch in decoded.nfc() {
        if ch.is_whitespace() || ch == '\u{00a0}' {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        let mapped = match ch {
            '‘' | '’' => '\'',
            '“' | '”' => '"',
            '–' | '—' => '-',
            other => other,
        };
        normalized.extend(mapped.to_lowercase());
    }
    normalized.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(finding: Finding) -> BTreeMap<String, ReportCell> {
        BTreeMap::from([
            ("riskScore".to_string(), ReportCell::Num(99.0)),
            ("findings".to_string(), ReportCell::Findings(vec![finding])),
        ])
    }

    fn finding(rule: &str, category: &str, excerpt: &str, severity: &str) -> Finding {
        Finding {
            severity: severity.to_string(),
            category: category.to_string(),
            rule: rule.to_string(),
            excerpt: excerpt.to_string(),
            recommendation: "Remove or qualify the claim.".to_string(),
            ..Finding::default()
        }
    }

    #[test]
    fn grounded_finding_is_enriched_and_score_is_derived() {
        let mut values = cells(finding(
            "ease_speed",
            "advertising_claims",
            "sjednáte snadno za pár minut",
            "low",
        ));
        let outcome = validate_cells(
            &mut values,
            "Půjčku sjednáte snadno za pár minut bez papírování.",
            false,
            "en",
        )
        .unwrap();
        assert!(!outcome.evidence_limited);
        assert_eq!(values["riskScore"], ReportCell::Num(0.0));
        let ReportCell::Findings(items) = &values["findings"] else {
            panic!("findings cell expected")
        };
        assert!(items[0].legal_basis.contains("Article 8(8)(a)"));
        assert_eq!(items[0].severity, "medium");
        assert_eq!(items[0].obligation, "MAY");
        assert!(!items[0].source_url.is_empty());
        assert!(!items[0].applicability.is_empty());
    }

    #[test]
    fn unknown_rule_or_ungrounded_excerpt_is_rejected() {
        let mut unknown = cells(finding("invented", "advertising_claims", "real text", "high"));
        assert!(validate_cells(&mut unknown, "real text", false, "en").is_err());

        let mut hallucinated = cells(finding(
            "false_availability",
            "advertising_claims",
            "100% approval for everyone",
            "high",
        ));
        assert!(validate_cells(&mut hallucinated, "Responsible lending only.", false, "en").is_err());
    }

    #[test]
    fn clean_result_requires_explicit_zero_score_and_empty_findings() {
        let mut contradictory = BTreeMap::from([
            ("riskScore".to_string(), ReportCell::Num(42.0)),
            ("findings".to_string(), ReportCell::Findings(vec![])),
        ]);
        assert!(validate_cells(&mut contradictory, "Ordinary page", false, "en").is_err());

        let mut clean = BTreeMap::from([
            ("riskScore".to_string(), ReportCell::Num(0.0)),
            ("findings".to_string(), ReportCell::Findings(vec![])),
        ]);
        validate_cells(&mut clean, "Ordinary page", false, "en").unwrap();
        assert_eq!(clean["riskScore"], ReportCell::Num(0.0));

        let mut truncated = BTreeMap::from([
            ("riskScore".to_string(), ReportCell::Null),
            ("findings".to_string(), ReportCell::Findings(vec![])),
        ]);
        let outcome = validate_cells(&mut truncated, "Partial ordinary page", true, "en").unwrap();
        assert!(outcome.evidence_limited);
        assert_eq!(truncated["riskScore"], ReportCell::Null);
    }

    #[test]
    fn overlapping_cost_rules_are_deduplicated_even_with_nested_quotes() {
        let findings = vec![
            finding(
                "incomplete_standard_info",
                "advertising_claims",
                "Úroková sazba od 5 %",
                "medium",
            ),
            finding("cost_downplay", "advertising_claims", "sazba od 5 %", "high"),
        ];
        let mut values = BTreeMap::from([
            ("riskScore".to_string(), ReportCell::Num(50.0)),
            ("findings".to_string(), ReportCell::Findings(findings)),
        ]);
        validate_cells(
            &mut values,
            "Výhodná půjčka, úroková sazba od 5 % bez starostí.",
            false,
            "cs",
        )
        .unwrap();
        let ReportCell::Findings(items) = &values["findings"] else {
            panic!("findings expected")
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rule, "cost_downplay");
        assert_eq!(values["riskScore"], ReportCell::Num(25.0));
    }

    #[test]
    fn truncated_scope_quarantines_absence_findings() {
        let mut values = cells(finding(
            "missing_warning",
            "advertising_claims",
            "Spotřebitelský úvěr pro každého",
            "medium",
        ));
        let outcome = validate_cells(&mut values, "Spotřebitelský úvěr pro každého", true, "en").unwrap();
        assert!(outcome.evidence_limited);
        assert_eq!(
            outcome.indeterminate_rules,
            vec!["incomplete_standard_info", "missing_warning"]
        );
        assert_eq!(values["riskScore"], ReportCell::Null);
        assert_eq!(values["findings"], ReportCell::Findings(vec![]));
    }

    #[test]
    fn rule_pack_does_not_claim_unsupported_consent_evidence() {
        let prompt = rules_prompt();
        assert!(prompt.contains("missing_warning"));
        assert!(prompt.contains("all advertising concerning credit agreements"));
        assert!(!prompt.contains("tracking_before_consent"));
        assert!(!prompt.contains("reject_not_equal"));
        assert!(!prompt.contains("preticked_consent"));
    }

    #[test]
    fn deterministic_finding_metadata_uses_report_language() {
        let mut values = cells(finding(
            "ease_speed",
            "advertising_claims",
            "sjednáte snadno za pár minut",
            "medium",
        ));
        validate_cells(&mut values, "Půjčku sjednáte snadno za pár minut.", false, "cs-CZ").unwrap();
        let ReportCell::Findings(items) = &values["findings"] else {
            panic!("findings cell expected")
        };
        assert!(items[0].title.starts_with("Zdůraznění"));
        assert!(items[0].legal_basis.contains("článek"));
        assert!(items[0].legal_status.contains("členského státu"));
    }

    #[test]
    fn rules_with_current_ucpd_basis_do_not_present_2026_as_their_only_effective_date() {
        for id in ["cost_downplay", "risk_downplay"] {
            let rule = rule_by_id(id).unwrap();
            assert_eq!(rule.source_url, UCPD_SOURCE_URL);
            assert!(rule.effective_date.starts_with("Current"));
            assert!(rule.effective_date.contains("2026-11-20"));
        }
    }

    #[test]
    fn evidence_grounding_normalizes_canonically_equivalent_unicode() {
        let mut values = cells(finding(
            "false_availability",
            "advertising_claims",
            "Úvěr pro každého",
            "high",
        ));
        validate_cells(&mut values, "U\u{301}věr pro každého", false, "cs").unwrap();
    }
}
