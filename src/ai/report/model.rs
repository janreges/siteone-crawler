// SiteOne Crawler - AI report data model
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The in-memory model that a first-class AI report is built from. One `extract` action
// populates an `AiReportModel` (site meta + typed schema + per-page rows + derived rollups);
// two exporters (JSON, HTML) then render it. Keeping all rows here (not in a `--rows-limit`ed
// SuperTable) is deliberate: the report must never silently drop analyzed pages.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// A per-page field's declared type. Drives both the LLM output schema and HTML auto-rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Str,
    /// Long free text (rendered truncated + expandable).
    Text,
    Int,
    Float,
    Bool,
    /// Fixed set of allowed string values (rendered as a colored chip + distribution).
    Enum(Vec<String>),
    /// Array of strings (rendered as tag chips + tag frequency).
    StrArray,
    Url,
    /// URL path starting with `/` (rendered monospace, never a live link).
    Path,
    /// Integer 0..=100 (rendered with an inline mini-bar + histogram).
    Score,
    Date,
    /// Array of structured findings (severity/category/rule/excerpt/recommendation). Rendered as
    /// expandable severity-colored sub-rows + a severity/category distribution. Used by audit
    /// presets (e.g. `compliance`).
    Findings,
}

impl FieldType {
    /// Stable machine name emitted in the JSON `schema` section.
    pub fn type_name(&self) -> &'static str {
        match self {
            FieldType::Str => "string",
            FieldType::Text => "text",
            FieldType::Int => "int",
            FieldType::Float => "float",
            FieldType::Bool => "bool",
            FieldType::Enum(_) => "enum",
            FieldType::StrArray => "string[]",
            FieldType::Url => "url",
            FieldType::Path => "path",
            FieldType::Score => "score",
            FieldType::Date => "date",
            FieldType::Findings => "findings",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, FieldType::Int | FieldType::Float | FieldType::Score)
    }
}

/// One declared per-page field.
#[derive(Debug, Clone)]
pub struct FieldSpec {
    pub name: String,
    pub ftype: FieldType,
    /// Field-level instruction injected into the prompt (what makes extraction accurate).
    pub desc: String,
    pub required: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl FieldSpec {
    pub fn new(name: &str, ftype: FieldType) -> Self {
        FieldSpec {
            name: name.to_string(),
            ftype,
            desc: String::new(),
            required: true,
            min: None,
            max: None,
        }
    }

    pub fn with_desc(mut self, desc: &str) -> Self {
        self.desc = desc.to_string();
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn to_json(&self) -> Value {
        let mut o = json!({ "name": self.name, "type": self.ftype.type_name(), "required": self.required });
        if !self.desc.is_empty() {
            o["desc"] = json!(self.desc);
        }
        if let FieldType::Enum(ref vals) = self.ftype {
            o["enum"] = json!(vals);
        }
        if let Some(m) = self.min {
            o["min"] = json!(m);
        }
        if let Some(m) = self.max {
            o["max"] = json!(m);
        }
        o
    }
}

/// One structured finding inside a `Findings` cell (audit presets).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Finding {
    /// info | low | medium | high | critical
    pub severity: String,
    pub category: String,
    /// Rule id / short label (e.g. "ease_speed", "missing_warning").
    pub rule: String,
    /// Verbatim page excerpt the finding is grounded in (may be empty).
    pub excerpt: String,
    pub recommendation: String,
    /// Deterministic rule-pack metadata, never authored by the model.
    pub title: String,
    pub legal_basis: String,
    pub obligation: String,
    pub legal_status: String,
    pub effective_date: String,
    pub source_url: String,
    pub source_urls: Vec<String>,
    pub applicability: String,
    pub evidence_scope: String,
}

impl Finding {
    pub fn to_json(&self) -> Value {
        let mut value = json!({
            "severity": self.severity,
            "category": self.category,
            "rule": self.rule,
            "excerpt": self.excerpt,
            "recommendation": self.recommendation,
        });
        for (key, item) in [
            ("title", &self.title),
            ("legalBasis", &self.legal_basis),
            ("obligation", &self.obligation),
            ("legalStatus", &self.legal_status),
            ("effectiveDate", &self.effective_date),
            ("sourceUrl", &self.source_url),
            ("applicability", &self.applicability),
            ("evidenceScope", &self.evidence_scope),
        ] {
            if !item.is_empty() {
                value[key] = json!(item);
            }
        }
        if !self.source_urls.is_empty() {
            value["sourceUrls"] = json!(self.source_urls);
        }
        value
    }
}

/// Canonical severity ordering (most severe first) for deterministic distributions/coloring.
pub const SEVERITY_ORDER: &[&str] = &["critical", "high", "medium", "low", "info"];

/// A single coerced cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportCell {
    Str(String),
    Num(f64),
    Bool(bool),
    List(Vec<String>),
    Findings(Vec<Finding>),
    Null,
}

impl ReportCell {
    /// Native JSON representation (numbers as numbers, arrays as arrays, etc.).
    pub fn to_json(&self) -> Value {
        match self {
            ReportCell::Str(s) => json!(s),
            ReportCell::Num(n) => {
                // Emit whole numbers without a trailing `.0` for a clean JSON.
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    json!(*n as i64)
                } else {
                    json!(n)
                }
            }
            ReportCell::Bool(b) => json!(b),
            ReportCell::List(v) => json!(v),
            ReportCell::Findings(f) => json!(f.iter().map(|x| x.to_json()).collect::<Vec<_>>()),
            ReportCell::Null => Value::Null,
        }
    }

    /// Flat display string (for the HTML table / CSV).
    pub fn to_display(&self) -> String {
        match self {
            ReportCell::Str(s) => s.clone(),
            ReportCell::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            ReportCell::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
            ReportCell::List(v) => v.join(", "),
            ReportCell::Findings(f) => f
                .iter()
                .map(|x| format!("[{}] {}: {}", x.severity, x.rule, x.recommendation))
                .collect::<Vec<_>>()
                .join(" | "),
            ReportCell::Null => String::new(),
        }
    }
}

/// One analyzed page.
#[derive(Debug, Clone)]
pub struct ReportRow {
    pub url: String,
    /// Path starting with `/` (deterministic, not from the LLM).
    pub path: String,
    pub cells: BTreeMap<String, ReportCell>,
    /// Set when the LLM response could not be parsed after all retries. An errored row carries NO
    /// fabricated cell values — it is surfaced honestly as an error in the JSON and HTML, and it is
    /// excluded from all rollups so it never skews the aggregates.
    pub error: Option<String>,
    /// Evidence-limited rows remain visible but do not claim a clean compliance assessment.
    pub evidence_status: Option<String>,
    pub input_truncated: bool,
    pub indeterminate_rules: Vec<String>,
}

impl ReportRow {
    /// A successful row from coerced cells.
    pub fn ok(url: String, path: String, cells: BTreeMap<String, ReportCell>) -> Self {
        ReportRow {
            url,
            path,
            cells,
            error: None,
            evidence_status: None,
            input_truncated: false,
            indeterminate_rules: Vec::new(),
        }
    }

    /// A row that failed to parse — no cell values, just the honest error.
    pub fn errored(url: String, path: String, error: String) -> Self {
        ReportRow {
            url,
            path,
            cells: BTreeMap::new(),
            error: Some(error),
            evidence_status: None,
            input_truncated: false,
            indeterminate_rules: Vec::new(),
        }
    }

    pub fn with_evidence(mut self, limited: bool, input_truncated: bool, indeterminate_rules: Vec<String>) -> Self {
        self.evidence_status = Some(if limited { "limited" } else { "text_complete" }.to_string());
        self.input_truncated = input_truncated;
        self.indeterminate_rules = indeterminate_rules;
        self
    }

    pub fn to_json(&self) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("url".to_string(), json!(self.url));
        o.insert("path".to_string(), json!(self.path));
        if let Some(ref e) = self.error {
            o.insert("_error".to_string(), json!(e));
        } else {
            for (k, v) in &self.cells {
                o.insert(k.clone(), v.to_json());
            }
        }
        if self.evidence_status.is_some() || self.input_truncated || !self.indeterminate_rules.is_empty() {
            o.insert(
                "_evidence".to_string(),
                json!({
                    "status": self.evidence_status.as_deref().unwrap_or("unavailable"),
                    "inputTruncated": self.input_truncated,
                    "indeterminateRules": self.indeterminate_rules,
                }),
            );
        }
        Value::Object(o)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CoverageMeta {
    pub crawled_html: usize,
    pub eligible: usize,
    pub excluded: usize,
    pub selected: usize,
    pub capped_dropped: usize,
    pub context_failed: usize,
    pub analyzed: usize,
    pub failed: usize,
    pub parse_failed: usize,
    pub request_failed: usize,
    pub task_dropped: usize,
    pub content_truncated: usize,
    pub indeterminate: usize,
    pub include_masks: Vec<String>,
    pub exclude_masks: Vec<String>,
    pub ranking_method: String,
    pub sampled: bool,
}

impl CoverageMeta {
    pub fn to_json(&self) -> Value {
        json!({
            "crawledHtml": self.crawled_html,
            "eligible": self.eligible,
            "excluded": self.excluded,
            "selected": self.selected,
            "cappedDropped": self.capped_dropped,
            "contextFailed": self.context_failed,
            "analyzed": self.analyzed,
            "failed": self.failed,
            "parseFailed": self.parse_failed,
            "requestFailed": self.request_failed,
            "taskDropped": self.task_dropped,
            "contentTruncated": self.content_truncated,
            "indeterminate": self.indeterminate,
            "includeMasks": self.include_masks,
            "excludeMasks": self.exclude_masks,
            "rankingMethod": self.ranking_method,
            "sampled": self.sampled,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct AiUsageMeta {
    pub calls: u64,
    pub cache_hits: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub calls_without_usage: u64,
    pub unaccounted_http_attempts: u64,
    pub http_attempts: u64,
    pub retries: u64,
    pub network_time_seconds: f64,
    pub initial_call_estimate: usize,
    pub worst_case_call_budget: usize,
    pub input_cost_per_million_usd: Option<f64>,
    pub output_cost_per_million_usd: Option<f64>,
    pub estimated_cost_usd: Option<f64>,
    pub cost_estimate_status: String,
}

impl AiUsageMeta {
    pub fn to_json(&self) -> Value {
        json!({
            "calls": self.calls,
            "cacheHits": self.cache_hits,
            "promptTokens": self.prompt_tokens,
            "completionTokens": self.completion_tokens,
            "callsWithoutUsage": self.calls_without_usage,
            "unaccountedHttpAttempts": self.unaccounted_http_attempts,
            "httpAttempts": self.http_attempts,
            "retries": self.retries,
            "networkTimeSeconds": self.network_time_seconds,
            "initialCallEstimate": self.initial_call_estimate,
            "worstCaseCallBudget": self.worst_case_call_budget,
            "inputCostPerMillionUsd": self.input_cost_per_million_usd,
            "outputCostPerMillionUsd": self.output_cost_per_million_usd,
            "estimatedCostUsd": self.estimated_cost_usd,
            "costEstimateStatus": self.cost_estimate_status,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct RulePackMeta {
    pub version: String,
    pub sources: Vec<String>,
    pub risk_formula: String,
    pub applicability: String,
    pub review_status: String,
}

impl RulePackMeta {
    pub fn to_json(&self) -> Value {
        json!({
            "version": self.version,
            "sources": self.sources,
            "riskFormula": self.risk_formula,
            "applicability": self.applicability,
            "reviewStatus": self.review_status,
        })
    }
}

/// Crawl/run metadata surfaced in the report header.
#[derive(Debug, Clone, Default)]
pub struct SiteMeta {
    pub host: String,
    pub url: String,
    pub crawled_at: String,
    pub pages_analyzed: usize,
    pub provider: String,
    pub model: String,
    pub report_language: String,
    pub chrome_language: String,
    pub coverage: CoverageMeta,
    pub usage: AiUsageMeta,
    pub rule_pack: Option<RulePackMeta>,
}

impl SiteMeta {
    pub fn to_json(&self) -> Value {
        let mut value = json!({
            "host": self.host,
            "url": self.url,
            "crawledAt": self.crawled_at,
            "pagesAnalyzed": self.pages_analyzed,
            "provider": self.provider,
            "model": self.model,
            "reportLanguage": self.report_language,
            "chromeLanguage": self.chrome_language,
            "coverage": self.coverage.to_json(),
            "usage": self.usage.to_json(),
            "rulePack": Value::Null,
        });
        if let Some(rule_pack) = &self.rule_pack {
            value["rulePack"] = rule_pack.to_json();
        }
        value
    }
}

/// Derived, deterministic site-wide aggregations used by the charts.
#[derive(Debug, Clone, Default)]
pub struct RollupSet {
    /// field -> ordered [(value, count)] for enum fields.
    pub distributions: BTreeMap<String, Vec<(String, usize)>>,
    /// field -> stats for numeric/score fields.
    pub numeric: BTreeMap<String, NumericStats>,
    /// field -> (true_count, false_count) for bool fields.
    pub bool_coverage: BTreeMap<String, (usize, usize)>,
    /// field -> ordered [(tag, count)] for string[] fields.
    pub tag_freq: BTreeMap<String, Vec<(String, usize)>>,
    /// field -> most frequent values for short free-string fields.
    pub string_freq: BTreeMap<String, Vec<(String, usize)>>,
    /// field -> findings aggregates (severity/category distributions, page/finding counts).
    pub findings: BTreeMap<String, FindingsStats>,
}

/// Aggregates over a `Findings` field across all pages.
#[derive(Debug, Clone, Default)]
pub struct FindingsStats {
    /// Severity → count, in canonical severity order.
    pub by_severity: Vec<(String, usize)>,
    /// Category → count, most frequent first.
    pub by_category: Vec<(String, usize)>,
    pub total_findings: usize,
    pub pages_with_findings: usize,
    pub pages_total: usize,
}

#[derive(Debug, Clone, Default)]
pub struct NumericStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub median: f64,
    /// Fixed 10-bucket histogram over [bucket_min, bucket_max].
    pub histogram: Vec<usize>,
    pub bucket_min: f64,
    pub bucket_max: f64,
}

impl RollupSet {
    pub fn to_json(&self) -> Value {
        let dist: serde_json::Map<String, Value> = self
            .distributions
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!(
                        v.iter()
                            .map(|(n, c)| json!({"value": n, "count": c}))
                            .collect::<Vec<_>>()
                    ),
                )
            })
            .collect();
        let num: serde_json::Map<String, Value> = self
            .numeric
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    json!({
                        "min": s.min, "max": s.max, "avg": s.avg, "median": s.median,
                        "histogram": s.histogram, "bucketMin": s.bucket_min, "bucketMax": s.bucket_max,
                    }),
                )
            })
            .collect();
        let boolc: serde_json::Map<String, Value> = self
            .bool_coverage
            .iter()
            .map(|(k, (t, f))| (k.clone(), json!({"true": t, "false": f})))
            .collect();
        let tags: serde_json::Map<String, Value> = self
            .tag_freq
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!(v.iter().map(|(n, c)| json!({"tag": n, "count": c})).collect::<Vec<_>>()),
                )
            })
            .collect();
        let strings: serde_json::Map<String, Value> = self
            .string_freq
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!(
                        v.iter()
                            .map(|(n, c)| json!({"value": n, "count": c}))
                            .collect::<Vec<_>>()
                    ),
                )
            })
            .collect();
        let findings: serde_json::Map<String, Value> = self
            .findings
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    json!({
                        "bySeverity": s.by_severity.iter().map(|(n, c)| json!({"severity": n, "count": c})).collect::<Vec<_>>(),
                        "byCategory": s.by_category.iter().map(|(n, c)| json!({"category": n, "count": c})).collect::<Vec<_>>(),
                        "totalFindings": s.total_findings,
                        "pagesWithFindings": s.pages_with_findings,
                        "pagesTotal": s.pages_total,
                    }),
                )
            })
            .collect();
        json!({
            "distributions": dist,
            "numeric": num,
            "boolCoverage": boolc,
            "tagFrequency": tags,
            "stringFrequency": strings,
            "findings": findings,
        })
    }
}

/// The complete report model.
#[derive(Debug, Clone)]
pub struct AiReportModel {
    pub preset: String,
    pub title: String,
    pub site: SiteMeta,
    pub schema: Vec<FieldSpec>,
    pub rows: Vec<ReportRow>,
    pub rollups: RollupSet,
    /// Optional executive summary (reuses the existing summary model as raw JSON).
    pub narrative: Option<Value>,
    /// Suggested hero chart id for the HTML report (e.g. "treemap", "score-histogram").
    pub hero: String,
}

impl AiReportModel {
    pub fn new(preset: &str, title: &str, site: SiteMeta, schema: Vec<FieldSpec>, hero: &str) -> Self {
        AiReportModel {
            preset: preset.to_string(),
            title: title.to_string(),
            site,
            schema,
            rows: Vec::new(),
            rollups: RollupSet::default(),
            narrative: None,
            hero: hero.to_string(),
        }
    }

    /// Compute all deterministic rollups from the current rows. Idempotent.
    pub fn build_rollups(&mut self) {
        let mut r = RollupSet::default();
        for f in &self.schema {
            match &f.ftype {
                FieldType::Enum(allowed) => {
                    let mut counts: BTreeMap<String, usize> = allowed.iter().map(|v| (v.clone(), 0)).collect();
                    for row in &self.rows {
                        if let Some(ReportCell::Str(s)) = row.cells.get(&f.name) {
                            *counts.entry(s.clone()).or_insert(0) += 1;
                        }
                    }
                    // Preserve the declared enum order, then any extras alphabetically.
                    let mut ordered: Vec<(String, usize)> = allowed
                        .iter()
                        .map(|v| (v.clone(), *counts.get(v).unwrap_or(&0)))
                        .collect();
                    for (k, c) in &counts {
                        if !allowed.contains(k) {
                            ordered.push((k.clone(), *c));
                        }
                    }
                    r.distributions.insert(f.name.clone(), ordered);
                }
                FieldType::Int | FieldType::Float | FieldType::Score => {
                    let mut vals: Vec<f64> = self
                        .rows
                        .iter()
                        .filter_map(|row| match row.cells.get(&f.name) {
                            Some(ReportCell::Num(n)) => Some(*n),
                            _ => None,
                        })
                        .collect();
                    if !vals.is_empty() {
                        r.numeric
                            .insert(f.name.clone(), numeric_stats(&mut vals, &f.ftype, f.min, f.max));
                    }
                }
                FieldType::Bool => {
                    let (mut t, mut fa) = (0usize, 0usize);
                    for row in &self.rows {
                        if let Some(ReportCell::Bool(b)) = row.cells.get(&f.name) {
                            if *b { t += 1 } else { fa += 1 }
                        }
                    }
                    r.bool_coverage.insert(f.name.clone(), (t, fa));
                }
                FieldType::StrArray => {
                    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                    for row in &self.rows {
                        if let Some(ReportCell::List(v)) = row.cells.get(&f.name) {
                            for tag in v {
                                *counts.entry(tag.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                    let mut ordered: Vec<(String, usize)> = counts.into_iter().collect();
                    ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    ordered.truncate(30);
                    r.tag_freq.insert(f.name.clone(), ordered);
                }
                FieldType::Str => {
                    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                    for row in &self.rows {
                        if let Some(ReportCell::Str(value)) = row.cells.get(&f.name)
                            && !value.trim().is_empty()
                        {
                            *counts.entry(value.clone()).or_default() += 1;
                        }
                    }
                    let mut ordered = counts.into_iter().collect::<Vec<_>>();
                    ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    ordered.truncate(30);
                    r.string_freq.insert(f.name.clone(), ordered);
                }
                FieldType::Findings => {
                    let mut sev: BTreeMap<String, usize> = BTreeMap::new();
                    let mut cat: BTreeMap<String, usize> = BTreeMap::new();
                    let mut total = 0usize;
                    let mut pages_with = 0usize;
                    for row in &self.rows {
                        if let Some(ReportCell::Findings(list)) = row.cells.get(&f.name) {
                            if !list.is_empty() {
                                pages_with += 1;
                            }
                            for fi in list {
                                total += 1;
                                *sev.entry(fi.severity.clone()).or_insert(0) += 1;
                                if !fi.category.is_empty() {
                                    *cat.entry(fi.category.clone()).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                    // Severity in canonical order first, then any unknowns.
                    let mut by_severity: Vec<(String, usize)> = SEVERITY_ORDER
                        .iter()
                        .filter(|s| sev.contains_key(**s))
                        .map(|s| (s.to_string(), sev[*s]))
                        .collect();
                    for (k, c) in &sev {
                        if !SEVERITY_ORDER.contains(&k.as_str()) {
                            by_severity.push((k.clone(), *c));
                        }
                    }
                    let mut by_category: Vec<(String, usize)> = cat.into_iter().collect();
                    by_category.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    r.findings.insert(
                        f.name.clone(),
                        FindingsStats {
                            by_severity,
                            by_category,
                            total_findings: total,
                            pages_with_findings: pages_with,
                            // Evidence-limited pages remain in this analyzed-page denominator. Their
                            // status is disclosed separately, and they are never presented as clean;
                            // positively grounded findings on them remain valid and countable.
                            pages_total: self.rows.iter().filter(|r| r.error.is_none()).count(),
                        },
                    );
                }
                _ => {}
            }
        }
        self.rollups = r;
    }

    /// Number of pages whose LLM response could not be parsed (surfaced as errors, not fabricated).
    pub fn error_count(&self) -> usize {
        self.rows.iter().filter(|r| r.error.is_some()).count()
    }

    /// Number of successfully analyzed pages.
    pub fn analyzed_count(&self) -> usize {
        self.rows.iter().filter(|r| r.error.is_none()).count()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "preset": self.preset,
            "title": self.title,
            "hero": self.hero,
            "site": self.site.to_json(),
            "schema": self.schema.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
            "pages": self.rows.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
            "rollups": self.rollups.to_json(),
            "narrative": self.narrative.clone().unwrap_or(Value::Null),
        })
    }
}

fn numeric_stats(vals: &mut [f64], ftype: &FieldType, min_o: Option<f64>, max_o: Option<f64>) -> NumericStats {
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = vals.len();
    let min = vals[0];
    let max = vals[n - 1];
    let avg = vals.iter().sum::<f64>() / n as f64;
    let median = if n % 2 == 1 {
        vals[n / 2]
    } else {
        (vals[n / 2 - 1] + vals[n / 2]) / 2.0
    };
    // Bucket range: score defaults 0..100, else declared min/max, else data range.
    let (bmin, bmax) = match ftype {
        FieldType::Score => (min_o.unwrap_or(0.0), max_o.unwrap_or(100.0)),
        _ => (min_o.unwrap_or(min), max_o.unwrap_or(max)),
    };
    let span = (bmax - bmin).max(f64::EPSILON);
    let mut histogram = vec![0usize; 10];
    for &v in vals.iter() {
        let mut idx = (((v - bmin) / span) * 10.0).floor() as isize;
        idx = idx.clamp(0, 9);
        histogram[idx as usize] += 1;
    }
    NumericStats {
        min,
        max,
        avg,
        median,
        histogram,
        bucket_min: bmin,
        bucket_max: bmax,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(url: &str, cells: Vec<(&str, ReportCell)>) -> ReportRow {
        ReportRow::ok(
            url.to_string(),
            url.to_string(),
            cells.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        )
    }

    #[test]
    fn cell_num_serializes_whole_as_int() {
        assert_eq!(ReportCell::Num(80.0).to_json(), json!(80));
        assert_eq!(ReportCell::Num(3.5).to_json(), json!(3.5));
    }

    #[test]
    fn site_metadata_exposes_coverage_language_usage_and_rule_pack() {
        let site = SiteMeta {
            report_language: "cs".into(),
            coverage: CoverageMeta {
                crawled_html: 100,
                eligible: 80,
                selected: 25,
                analyzed: 23,
                failed: 2,
                capped_dropped: 55,
                sampled: true,
                ..CoverageMeta::default()
            },
            usage: AiUsageMeta {
                calls: 27,
                cache_hits: 4,
                input_cost_per_million_usd: Some(1.0),
                output_cost_per_million_usd: Some(2.0),
                estimated_cost_usd: Some(0.125),
                cost_estimate_status: "complete".into(),
                ..AiUsageMeta::default()
            },
            rule_pack: Some(RulePackMeta {
                version: "rules-v1".into(),
                sources: vec!["https://example.test/rules".into()],
                risk_formula: "high=25".into(),
                applicability: "consumer credit only".into(),
                review_status: "advisory engineering taxonomy".into(),
            }),
            ..SiteMeta::default()
        };
        let json = site.to_json();
        assert_eq!(json["reportLanguage"], "cs");
        assert_eq!(json["coverage"]["selected"], 25);
        assert_eq!(json["coverage"]["sampled"], true);
        assert_eq!(json["usage"]["cacheHits"], 4);
        assert_eq!(json["usage"]["estimatedCostUsd"], 0.125);
        assert_eq!(json["rulePack"]["version"], "rules-v1");
    }

    #[test]
    fn rollup_enum_distribution_preserves_declared_order() {
        let schema = vec![FieldSpec::new(
            "section",
            FieldType::Enum(vec!["Blog".into(), "Docs".into(), "Other".into()]),
        )];
        let mut m = AiReportModel::new("t", "T", SiteMeta::default(), schema, "");
        m.rows = vec![
            row("/a", vec![("section", ReportCell::Str("Docs".into()))]),
            row("/b", vec![("section", ReportCell::Str("Docs".into()))]),
            row("/c", vec![("section", ReportCell::Str("Blog".into()))]),
        ];
        m.build_rollups();
        let d = &m.rollups.distributions["section"];
        assert_eq!(d[0], ("Blog".to_string(), 1));
        assert_eq!(d[1], ("Docs".to_string(), 2));
        assert_eq!(d[2], ("Other".to_string(), 0));
    }

    #[test]
    fn rollup_numeric_min_median_max_and_histogram() {
        let schema = vec![FieldSpec::new("q", FieldType::Score)];
        let mut m = AiReportModel::new("t", "T", SiteMeta::default(), schema, "");
        m.rows = vec![
            row("/a", vec![("q", ReportCell::Num(0.0))]),
            row("/b", vec![("q", ReportCell::Num(50.0))]),
            row("/c", vec![("q", ReportCell::Num(100.0))]),
        ];
        m.build_rollups();
        let s = &m.rollups.numeric["q"];
        assert_eq!(s.min, 0.0);
        assert_eq!(s.median, 50.0);
        assert_eq!(s.max, 100.0);
        assert_eq!(s.histogram.iter().sum::<usize>(), 3);
        assert_eq!(s.bucket_min, 0.0);
        assert_eq!(s.bucket_max, 100.0);
    }

    #[test]
    fn rollup_bool_and_tags() {
        let schema = vec![
            FieldSpec::new("inNav", FieldType::Bool),
            FieldSpec::new("tags", FieldType::StrArray),
        ];
        let mut m = AiReportModel::new("t", "T", SiteMeta::default(), schema, "");
        m.rows = vec![
            row(
                "/a",
                vec![
                    ("inNav", ReportCell::Bool(true)),
                    ("tags", ReportCell::List(vec!["x".into(), "y".into()])),
                ],
            ),
            row(
                "/b",
                vec![
                    ("inNav", ReportCell::Bool(false)),
                    ("tags", ReportCell::List(vec!["x".into()])),
                ],
            ),
        ];
        m.build_rollups();
        assert_eq!(m.rollups.bool_coverage["inNav"], (1, 1));
        assert_eq!(m.rollups.tag_freq["tags"][0], ("x".to_string(), 2));
    }

    #[test]
    fn errored_row_has_no_fabricated_values_and_is_excluded_from_rollups() {
        let schema = vec![FieldSpec::new(
            "section",
            FieldType::Enum(vec!["Blog".into(), "Docs".into(), "Other".into()]),
        )];
        let mut m = AiReportModel::new("t", "T", SiteMeta::default(), schema, "");
        m.rows = vec![
            row("/a", vec![("section", ReportCell::Str("Docs".into()))]),
            ReportRow::errored("https://x/b".into(), "/b".into(), "invalid JSON".into()),
        ];
        m.build_rollups();
        assert_eq!(m.error_count(), 1);
        assert_eq!(m.analyzed_count(), 1);
        // The errored row JSON carries `_error` and NO section value (no fabrication).
        let ej = m.rows[1].to_json();
        assert_eq!(ej["_error"], "invalid JSON");
        assert!(ej.get("section").is_none());
        // Rollups count only the analyzed row.
        let d = &m.rollups.distributions["section"];
        assert_eq!(d.iter().find(|(k, _)| k == "Docs").unwrap().1, 1);
    }

    #[test]
    fn rollup_findings_severity_category_and_counts() {
        let schema = vec![FieldSpec::new("findings", FieldType::Findings)];
        let mut m = AiReportModel::new("t", "T", SiteMeta::default(), schema, "");
        let mk = |sev: &str, cat: &str| Finding {
            severity: sev.into(),
            category: cat.into(),
            rule: "r".into(),
            excerpt: String::new(),
            recommendation: "fix".into(),
            ..Finding::default()
        };
        m.rows = vec![
            row(
                "/a",
                vec![(
                    "findings",
                    ReportCell::Findings(vec![mk("high", "ads"), mk("low", "cookies")]),
                )],
            ),
            row("/b", vec![("findings", ReportCell::Findings(vec![mk("high", "ads")]))]),
            row("/c", vec![("findings", ReportCell::Findings(vec![]))]),
        ];
        m.build_rollups();
        let fs = &m.rollups.findings["findings"];
        assert_eq!(fs.total_findings, 3);
        assert_eq!(fs.pages_with_findings, 2);
        assert_eq!(fs.pages_total, 3);
        // Severity in canonical order: high before low.
        assert_eq!(fs.by_severity[0], ("high".to_string(), 2));
        assert_eq!(fs.by_severity[1], ("low".to_string(), 1));
        // Category, most frequent first.
        assert_eq!(fs.by_category[0], ("ads".to_string(), 2));
    }
}
