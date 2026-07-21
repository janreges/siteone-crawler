// SiteOne Crawler - AI report value coercion
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Validate a parsed LLM JSON object against the declared `FieldSpec`s and then convert it into
// typed `ReportCell`s. Mechanical JSON defects are repaired before this layer; semantic type
// mismatches are deliberately rejected so the caller retries instead of guessing a value.

use std::collections::BTreeMap;

use serde_json::Value;

use super::model::{FieldSpec, FieldType, Finding, ReportCell};

/// Validate and coerce one page's parsed object into typed cells (one per declared field).
pub fn coerce_row(fields: &[FieldSpec], obj: &Value) -> Result<BTreeMap<String, ReportCell>, String> {
    let object = obj
        .as_object()
        .ok_or_else(|| "extract response is not a JSON object".to_string())?;
    if let Some(unexpected) = object
        .keys()
        .find(|key| !fields.iter().any(|field| field.name == **key))
    {
        return Err(format!("unexpected field '{}' in extract response", unexpected));
    }
    let mut cells = BTreeMap::new();
    for f in fields {
        let raw = object
            .get(&f.name)
            .ok_or_else(|| format!("field '{}' is missing; every declared key must be explicit", f.name))?;
        if raw.is_null() {
            if f.required {
                return Err(format!("required field '{}' must not be null", f.name));
            } else {
                cells.insert(f.name.clone(), ReportCell::Null);
                continue;
            }
        }
        let value = coerce_value(f, raw).map_err(|e| format!("field '{}': {}", f.name, e))?;
        cells.insert(f.name.clone(), value);
    }
    Ok(cells)
}

fn coerce_value(f: &FieldSpec, v: &Value) -> Result<ReportCell, String> {
    match &f.ftype {
        FieldType::Str | FieldType::Text => Ok(ReportCell::Str(required_string(v)?)),
        FieldType::Url => {
            let value = required_string(v)?;
            url::Url::parse(&value).map_err(|_| "must be an absolute URL".to_string())?;
            Ok(ReportCell::Str(value))
        }
        FieldType::Path => {
            let value = required_string(v)?;
            if !value.starts_with('/') {
                return Err("must start with '/'".to_string());
            }
            Ok(ReportCell::Str(value))
        }
        FieldType::Date => {
            let value = required_string(v)?;
            let bytes = value.as_bytes();
            let lexical_iso_date = bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes[..4].iter().all(u8::is_ascii_digit)
                && bytes[5..7].iter().all(u8::is_ascii_digit)
                && bytes[8..].iter().all(u8::is_ascii_digit);
            if !lexical_iso_date {
                return Err("must be an ISO date in YYYY-MM-DD form".to_string());
            }
            chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                .map_err(|_| "must be an ISO date in YYYY-MM-DD form".to_string())?;
            Ok(ReportCell::Str(value))
        }
        FieldType::Bool => v
            .as_bool()
            .map(ReportCell::Bool)
            .ok_or_else(|| "must be a JSON boolean".to_string()),
        FieldType::Int => {
            const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
            let integer = v
                .as_i64()
                .ok_or_else(|| "must be a signed 64-bit JSON integer".to_string())?;
            if !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&integer) {
                return Err("must be within the lossless JSON/browser integer range".to_string());
            }
            let n = integer as f64;
            let (lo, hi) = numeric_bounds(f);
            if lo > hi {
                return Err("has invalid numeric bounds".to_string());
            }
            if n < lo || n > hi {
                return Err(format!("value {} is outside the allowed range {}..{}", n, lo, hi));
            }
            Ok(ReportCell::Num(n))
        }
        FieldType::Float | FieldType::Score => {
            let n = v.as_f64().ok_or_else(|| "must be a JSON number".to_string())?;
            if !n.is_finite() {
                return Err("must be a finite number".to_string());
            }
            let (lo, hi) = numeric_bounds(f);
            if lo > hi {
                return Err("has invalid numeric bounds".to_string());
            }
            if n < lo || n > hi {
                return Err(format!("value {} is outside the allowed range {}..{}", n, lo, hi));
            }
            if matches!(f.ftype, FieldType::Score) && n.fract() != 0.0 {
                return Err("must be an integer".to_string());
            }
            Ok(ReportCell::Num(n))
        }
        FieldType::Enum(allowed) => {
            let value = required_string(v)?;
            clamp_enum(&value, allowed)
                .map(ReportCell::Str)
                .ok_or_else(|| format!("value '{}' is not one of [{}]", value, allowed.join(", ")))
        }
        FieldType::StrArray => Ok(ReportCell::List(as_string_list(v)?)),
        FieldType::Findings => Ok(ReportCell::Findings(as_findings(v)?)),
    }
}

/// Allowed severities for structured findings.
const FINDING_SEVERITIES: &[&str] = &["critical", "high", "medium", "low", "info"];

fn as_findings(v: &Value) -> Result<Vec<Finding>, String> {
    let items = v
        .as_array()
        .ok_or_else(|| "must be an array of finding objects".to_string())?;
    let mut findings = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("finding {} must be an object", index + 1))?;
        const FINDING_KEYS: &[&str] = &["severity", "category", "rule", "excerpt", "recommendation"];
        if let Some(key) = obj.keys().find(|key| !FINDING_KEYS.contains(&key.as_str())) {
            return Err(format!("finding {} contains unexpected field '{}'", index + 1, key));
        }
        let get = |key: &str| -> Result<String, String> {
            let value = obj
                .get(key)
                .ok_or_else(|| format!("finding {} is missing '{}'", index + 1, key))?;
            value
                .as_str()
                .map(|s| s.trim().to_string())
                .ok_or_else(|| format!("finding {} field '{}' must be a string", index + 1, key))
        };
        let severity = get("severity")?.to_lowercase();
        if !FINDING_SEVERITIES.contains(&severity.as_str()) {
            return Err(format!("finding {} has unknown severity '{}'", index + 1, severity));
        }
        let category = get("category")?;
        let rule = get("rule")?;
        let excerpt = get("excerpt")?;
        let recommendation = get("recommendation")?;
        if category.is_empty() || rule.is_empty() || recommendation.is_empty() {
            return Err(format!(
                "finding {} must have non-empty category, rule, and recommendation",
                index + 1
            ));
        }
        findings.push(Finding {
            severity,
            category,
            rule,
            excerpt,
            recommendation,
            ..Finding::default()
        });
    }
    Ok(findings)
}

fn numeric_bounds(f: &FieldSpec) -> (f64, f64) {
    match f.ftype {
        FieldType::Score => (f.min.unwrap_or(0.0), f.max.unwrap_or(100.0)),
        _ => (f.min.unwrap_or(f64::MIN), f.max.unwrap_or(f64::MAX)),
    }
}

fn required_string(v: &Value) -> Result<String, String> {
    let value = v
        .as_str()
        .map(str::trim)
        .ok_or_else(|| "must be a string".to_string())?;
    if value.is_empty() {
        return Err("must not be empty; use null for an optional unknown value".to_string());
    }
    Ok(value.to_string())
}

fn as_string_list(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Array(a) => a
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| format!("array item {} must be a non-empty string", index + 1))
            })
            .collect(),
        _ => Err("must be a JSON array of strings".to_string()),
    }
}

/// Map an exact model value to the canonical spelling in the allowed enum set. Case normalization
/// is mechanically safe; partial/whole-word inference is not and would fabricate a category.
fn clamp_enum(v: &str, allowed: &[String]) -> Option<String> {
    let vt = v.trim();
    if vt.is_empty() {
        return None;
    }
    if let Some(m) = allowed.iter().find(|a| a.eq_ignore_ascii_case(vt)) {
        return Some(m.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn f(name: &str, ft: FieldType) -> FieldSpec {
        FieldSpec::new(name, ft)
    }

    #[test]
    fn unknown_or_missing_required_enum_is_an_error() {
        let fields = vec![f(
            "s",
            FieldType::Enum(vec!["landing".into(), "article".into(), "error".into()]),
        )];
        assert!(coerce_row(&fields, &json!({"s": "weird"})).is_err());
        assert!(coerce_row(&fields, &json!({})).is_err());
    }

    #[test]
    fn undeclared_fields_are_rejected() {
        let fields = vec![f("title", FieldType::Str)];
        assert!(coerce_row(&fields, &json!({"title": "Hello", "invented": true})).is_err());
    }

    #[test]
    fn enum_matches_case_insensitively_but_rejects_inferred_values() {
        let fields = vec![f(
            "s",
            FieldType::Enum(vec!["Blog".into(), "Docs".into(), "Home".into()]),
        )];
        assert_eq!(
            coerce_row(&fields, &json!({"s": "docs"})).unwrap()["s"],
            ReportCell::Str("Docs".into())
        );
        assert!(coerce_row(&fields, &json!({"s": "Blog post"})).is_err());
        assert!(coerce_row(&fields, &json!({"s": "homepage"})).is_err());
    }

    #[test]
    fn inverted_bounds_return_error_without_panicking() {
        let mut spec = f("q", FieldType::Score);
        spec.min = Some(90.0);
        spec.max = Some(10.0);
        assert!(coerce_row(&[spec], &json!({"q": 42})).is_err());
    }

    #[test]
    fn numeric_and_boolean_strings_are_rejected() {
        assert!(coerce_row(&[f("n", FieldType::Int)], &json!({"n": "1234"})).is_err());
        assert!(coerce_row(&[f("x", FieldType::Float)], &json!({"x": "3.5"})).is_err());
        assert!(coerce_row(&[f("ok", FieldType::Bool)], &json!({"ok": "true"})).is_err());
    }

    #[test]
    fn dates_and_integers_keep_the_documented_lossless_contract() {
        let dates = vec![f("date", FieldType::Date)];
        assert!(coerce_row(&dates, &json!({"date": "2024-1-2"})).is_err());
        assert!(coerce_row(&dates, &json!({"date": "+2024-01-02"})).is_err());
        assert!(coerce_row(&dates, &json!({"date": "2024-02-30"})).is_err());
        assert!(coerce_row(&dates, &json!({"date": "2024-01-02"})).is_ok());

        let integers = vec![f("value", FieldType::Int)];
        assert!(coerce_row(&integers, &json!({"value": 9_007_199_254_740_992_u64})).is_err());
        assert_eq!(
            coerce_row(&integers, &json!({"value": 9_007_199_254_740_991_i64})).unwrap()["value"],
            ReportCell::Num(9_007_199_254_740_991.0)
        );
    }

    #[test]
    fn score_requires_an_in_range_integer_number() {
        let fields = vec![f("q", FieldType::Score)];
        assert!(coerce_row(&fields, &json!({"q": 150})).is_err());
        assert!(coerce_row(&fields, &json!({"q": "82/100"})).is_err());
        assert!(coerce_row(&fields, &json!({"q": 82.5})).is_err());
        assert_eq!(
            coerce_row(&fields, &json!({"q": 82})).unwrap()["q"],
            ReportCell::Num(82.0)
        );
    }

    #[test]
    fn string_array_requires_json_array() {
        let fields = vec![f("tags", FieldType::StrArray)];
        assert!(coerce_row(&fields, &json!({"tags": "a, b, c"})).is_err());
        let cells = coerce_row(&fields, &json!({"tags": ["a", "b", "c"]})).unwrap();
        assert_eq!(
            cells["tags"],
            ReportCell::List(vec!["a".into(), "b".into(), "c".into()])
        );
    }

    #[test]
    fn optional_fields_require_an_explicit_null() {
        let fields = [FieldType::Str, FieldType::Int, FieldType::Bool, FieldType::StrArray]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let mut field = f(&format!("f{}", index), kind);
                field.required = false;
                field
            })
            .collect::<Vec<_>>();
        assert!(coerce_row(&fields, &json!({})).is_err());
        let value = serde_json::Value::Object(
            fields
                .iter()
                .map(|field| (field.name.clone(), serde_json::Value::Null))
                .collect(),
        );
        let cells = coerce_row(&fields, &value).unwrap();
        assert!(cells.values().all(|value| value == &ReportCell::Null));
    }

    #[test]
    fn findings_are_validated_and_severity_normalized() {
        let fields = vec![f("findings", FieldType::Findings)];
        let raw = json!({"findings":[
            {"severity":"HIGH","category":"advertising_claims","rule":"ease_speed","excerpt":"za pár minut","recommendation":"remove"}
        ]});
        let cells = coerce_row(&fields, &raw).unwrap();
        match &cells["findings"] {
            ReportCell::Findings(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].severity, "high"); // lowercased & valid
                assert_eq!(v[0].rule, "ease_speed");
            }
            other => panic!("expected Findings, got {:?}", other),
        }
        assert!(
            coerce_row(
                &fields,
                &json!({"findings":[{"severity":"bogus","category":"x","rule":"x","excerpt":"x","recommendation":"y"}]})
            )
            .is_err()
        );
    }

    #[test]
    fn empty_findings_array_is_empty() {
        let fields = vec![f("findings", FieldType::Findings)];
        let cells = coerce_row(&fields, &json!({"findings":[]})).unwrap();
        assert_eq!(cells["findings"], ReportCell::Findings(vec![]));
    }
}
