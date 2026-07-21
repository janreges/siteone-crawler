// SiteOne Crawler - AI report schema parsing
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Turns a user's field definition (compact `--ai-extract-fields` DSL, or a rich
// `--ai-schema-file`) into `FieldSpec`s, and derives both (a) the human `<output_schema>` block
// injected into the prompt (prompt-mode contract) and (b) a JSON Schema `Value` for native
// API-level enforcement.

use serde_json::{Value, json};

use super::model::{FieldSpec, FieldType};

/// Parse the compact DSL: `name:type[:desc], name:type, ...`. Commas inside `enum(...)` are not
/// field separators. `desc` (optional) is everything after the second colon (may itself contain
/// colons, e.g. a URL in the instruction).
pub fn parse_fields_dsl(dsl: &str) -> Result<Vec<FieldSpec>, String> {
    let parts = split_top_level(dsl);
    let mut fields = Vec::new();
    for raw in parts {
        let seg = raw.trim();
        if seg.is_empty() {
            continue;
        }
        // Split into name / type / desc on the first two top-level colons (colons inside enum(...)
        // parens are protected).
        let (name, rest) =
            split_once_top_level(seg, ':').ok_or_else(|| format!("field '{}' must be 'name:type'", seg))?;
        let (type_str, desc) = match split_once_top_level(rest.trim(), ':') {
            Some((t, d)) => (t.trim().to_string(), d.trim().to_string()),
            None => (rest.trim().to_string(), String::new()),
        };
        let ftype = parse_type(&type_str)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(format!("empty field name in '{}'", seg));
        }
        let mut spec = FieldSpec::new(&name, ftype).with_desc(&desc);
        spec.required = true;
        fields.push(spec);
    }
    if fields.is_empty() {
        return Err("no fields parsed from --ai-extract-fields".to_string());
    }
    validate_field_names(&fields)?;
    Ok(fields)
}

/// A repeated field name would silently overwrite an earlier column and double-list it in the JSON
/// schema `required` set — reject it.
fn validate_field_names(fields: &[FieldSpec]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for f in fields {
        let normalized = f.name.to_lowercase();
        if matches!(normalized.as_str(), "url" | "path" | "_error" | "_evidence") {
            return Err(format!("field name '{}' is reserved by the report format", f.name));
        }
        let mut chars = f.name.chars();
        if !chars.next().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(format!(
                "field name '{}' must be an ASCII identifier starting with a letter or underscore",
                f.name
            ));
        }
        if !seen.insert(normalized) {
            return Err(format!("duplicate field name '{}'", f.name));
        }
        if let FieldType::Enum(values) = &f.ftype {
            let mut enum_values = std::collections::HashSet::new();
            for value in values {
                if value.trim().is_empty() || !enum_values.insert(value.trim().to_lowercase()) {
                    return Err(format!(
                        "field '{}' has empty or duplicate enum values (case-insensitive)",
                        f.name
                    ));
                }
            }
        }
        if (f.min.is_some() || f.max.is_some()) && !f.ftype.is_numeric() {
            return Err(format!("field '{}' uses min/max but is not numeric", f.name));
        }
        if matches!(f.ftype, FieldType::Int | FieldType::Score)
            && [f.min, f.max].into_iter().flatten().any(|value| value.fract() != 0.0)
        {
            return Err(format!("field '{}' requires integer min/max bounds", f.name));
        }
        if matches!(f.ftype, FieldType::Int)
            && [f.min, f.max]
                .into_iter()
                .flatten()
                .any(|value| value.abs() > 9_007_199_254_740_991.0)
        {
            return Err(format!(
                "field '{}' integer bounds exceed the lossless JSON/browser range",
                f.name
            ));
        }
        if matches!(f.ftype, FieldType::Score)
            && [f.min, f.max]
                .into_iter()
                .flatten()
                .any(|value| !(0.0..=100.0).contains(&value))
        {
            return Err(format!("field '{}' score bounds must stay within 0..100", f.name));
        }
    }
    Ok(())
}

fn parse_type(t: &str) -> Result<FieldType, String> {
    let trimmed = t.trim();
    let low = trimmed.to_lowercase();
    // Detect the enum(...) form case-insensitively, but keep the ORIGINAL-cased values.
    if low.starts_with("enum(") && trimmed.ends_with(')') {
        let inner = &trimmed["enum(".len()..trimmed.len() - 1];
        let vals: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
        if vals.is_empty() || vals.iter().any(|value| value.is_empty()) {
            return Err(format!("enum type '{}' contains an empty value", t));
        }
        return Ok(FieldType::Enum(vals));
    }
    match low.as_str() {
        "string" | "str" => Ok(FieldType::Str),
        "text" => Ok(FieldType::Text),
        "int" | "integer" => Ok(FieldType::Int),
        "float" | "number" => Ok(FieldType::Float),
        "bool" | "boolean" => Ok(FieldType::Bool),
        "string[]" | "str[]" | "array" | "list" => Ok(FieldType::StrArray),
        "url" => Ok(FieldType::Url),
        "path" => Ok(FieldType::Path),
        "score" => Ok(FieldType::Score),
        "date" => Ok(FieldType::Date),
        "findings" => Ok(FieldType::Findings),
        _ => Err(format!("unknown field type '{}'", t)),
    }
}

/// Load a rich schema from a JSON file: an array of
/// `{name, type, desc?, required?, min?, max?, enum?}` objects.
pub fn load_schema_file(path: &str) -> Result<Vec<FieldSpec>, String> {
    let data = std::fs::read_to_string(path).map_err(|e| format!("cannot read --ai-schema-file '{}': {}", path, e))?;
    let arr: Value = serde_json::from_str(&data).map_err(|e| format!("invalid JSON in '{}': {}", path, e))?;
    let items = arr
        .as_array()
        .ok_or_else(|| "schema file must be a JSON array of field objects".to_string())?;
    let mut fields = Vec::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("each field needs a string 'name'")?
            .to_string();
        if name.trim().is_empty() {
            return Err("field name must not be empty".to_string());
        }
        let type_str = item
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or("each field needs a string 'type'")?;
        let ftype = if let Some(enum_value) = item.get("enum") {
            if !matches!(type_str.trim().to_ascii_lowercase().as_str(), "enum" | "string" | "str") {
                return Err(format!(
                    "field '{}' supplies enum values but has incompatible type '{}'",
                    name, type_str
                ));
            }
            let enum_vals = enum_value
                .as_array()
                .ok_or_else(|| format!("field '{}' enum must be an array of strings", name))?;
            let vals: Vec<String> = enum_vals
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .ok_or_else(|| format!("field '{}' enum values must be non-empty strings", name))
                })
                .collect::<Result<_, _>>()?;
            if vals.is_empty() {
                return Err(format!(
                    "field '{}' has an empty enum (needs at least one string value)",
                    name
                ));
            }
            FieldType::Enum(vals)
        } else {
            parse_type(type_str)?
        };
        let mut spec = FieldSpec::new(&name, ftype);
        spec.desc = match item.get("desc") {
            Some(value) => value
                .as_str()
                .ok_or_else(|| format!("field '{}' desc must be a string", name))?
                .to_string(),
            None => String::new(),
        };
        spec.required = match item.get("required") {
            Some(value) => value
                .as_bool()
                .ok_or_else(|| format!("field '{}' required must be a boolean", name))?,
            None => true,
        };
        spec.min = optional_finite_number(item.get("min"), &name, "min")?;
        spec.max = optional_finite_number(item.get("max"), &name, "max")?;
        // A min>max range would make `f64::clamp` panic at coercion time — reject it up front.
        if let (Some(lo), Some(hi)) = (spec.min, spec.max)
            && lo > hi
        {
            return Err(format!("field '{}' has min ({}) greater than max ({})", name, lo, hi));
        }
        fields.push(spec);
    }
    if fields.is_empty() {
        return Err("schema file contained no fields".to_string());
    }
    validate_field_names(&fields)?;
    Ok(fields)
}

fn optional_finite_number(value: Option<&Value>, field: &str, key: &str) -> Result<Option<f64>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let number = value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("field '{}' {} must be a finite number", field, key))?;
    Ok(Some(number))
}

/// The human-readable `<output_schema>` block injected into the prompt. This is the ONLY contract
/// in prompt-mode (no native enforcement) and a helpful hint even when enforcing.
pub fn output_schema_block(fields: &[FieldSpec]) -> String {
    let mut lines = String::from("<output_schema>\nReturn ONLY one JSON object with EXACTLY these keys:\n{\n");
    for (i, f) in fields.iter().enumerate() {
        let mut type_hint = match &f.ftype {
            FieldType::Enum(v) => format!("one of [{}]", v.join(", ")),
            FieldType::StrArray => "array of short strings".to_string(),
            FieldType::Int => "integer".to_string(),
            FieldType::Float => "number".to_string(),
            FieldType::Score => "integer 0-100".to_string(),
            FieldType::Bool => "true or false".to_string(),
            FieldType::Text => "string (a few sentences)".to_string(),
            FieldType::Path => "string (URL path starting with /)".to_string(),
            FieldType::Url => "string (absolute URL)".to_string(),
            FieldType::Date => "string (ISO date)".to_string(),
            FieldType::Str => "short string".to_string(),
            FieldType::Findings => {
                "array of {\"severity\":\"info|low|medium|high|critical\",\"category\":\"...\",\"rule\":\"...\",\"excerpt\":\"verbatim page text or empty\",\"recommendation\":\"...\"} (empty array [] if none)".to_string()
            }
        };
        if !f.required {
            type_hint.push_str(" or null");
        }
        let desc = if f.desc.is_empty() {
            String::new()
        } else {
            format!(" — {}", f.desc)
        };
        let comma = if i + 1 < fields.len() { "," } else { "" };
        lines.push_str(&format!("  \"{}\": <{}{}>{}\n", f.name, type_hint, desc, comma));
    }
    lines.push_str("}\nOutput ONLY the JSON object — no prose, no markdown, no code fences.\n</output_schema>");
    lines
}

/// A JSON Schema object for native API-level enforcement.
pub fn json_schema_of(fields: &[FieldSpec]) -> Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for f in fields {
        let mut schema = match &f.ftype {
            FieldType::Str | FieldType::Text => json!({"type": "string"}),
            FieldType::Url => json!({"type": "string", "format": "uri"}),
            FieldType::Path => json!({"type": "string", "pattern": "^/"}),
            FieldType::Date => json!({"type": "string", "format": "date"}),
            FieldType::Int => numeric_schema(
                "integer",
                Some(f.min.unwrap_or(-9_007_199_254_740_991.0)),
                Some(f.max.unwrap_or(9_007_199_254_740_991.0)),
            ),
            FieldType::Float => numeric_schema("number", f.min, f.max),
            FieldType::Score => {
                json!({"type": "integer", "minimum": f.min.unwrap_or(0.0), "maximum": f.max.unwrap_or(100.0)})
            }
            FieldType::Bool => json!({"type": "boolean"}),
            FieldType::Enum(vals) => json!({"type": "string", "enum": vals}),
            FieldType::StrArray => json!({"type": "array", "items": {"type": "string"}}),
            FieldType::Findings => json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "severity": {"type": "string", "enum": ["info", "low", "medium", "high", "critical"]},
                        "category": {"type": "string"},
                        "rule": {"type": "string"},
                        "excerpt": {"type": "string"},
                        "recommendation": {"type": "string"},
                    },
                    "required": ["severity", "category", "rule", "excerpt", "recommendation"],
                    "additionalProperties": false,
                }
            }),
        };
        if !f.required {
            make_nullable(&mut schema);
        }
        props.insert(f.name.clone(), schema);
        // OpenAI strict schemas require every property in `required`. Optional report fields are
        // represented as a required key whose value may be JSON null.
        required.push(Value::String(f.name.clone()));
    }
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

fn numeric_schema(kind: &str, min: Option<f64>, max: Option<f64>) -> Value {
    let mut schema = json!({"type": kind});
    if let Some(value) = min {
        schema["minimum"] = json!(value);
    }
    if let Some(value) = max {
        schema["maximum"] = json!(value);
    }
    schema
}

fn make_nullable(schema: &mut Value) {
    let Some(kind) = schema.get("type").and_then(Value::as_str).map(str::to_string) else {
        return;
    };
    schema["type"] = json!([kind, "null"]);
}

/// Split on top-level commas (ignoring commas inside `(...)`).
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Split once on the first `sep` that is NOT inside `(...)`.
fn split_once_top_level(s: &str, sep: char) -> Option<(String, String)> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if c == sep && depth == 0 => {
                return Some((s[..i].to_string(), s[i + c.len_utf8()..].to_string()));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::report::presets::{PRESET_KEYS, preset_by_key};

    #[test]
    fn parses_dsl_types_and_enum() {
        let f = parse_fields_dsl(
            "title:string, description:text, section:enum(Blog,Docs,Other), n:int, ok:bool, tags:string[], q:score",
        )
        .unwrap();
        assert_eq!(f.len(), 7);
        assert!(
            matches!(&f[2].ftype, FieldType::Enum(v) if v == &vec!["Blog".to_string(),"Docs".to_string(),"Other".to_string()])
        );
        assert!(matches!(f[3].ftype, FieldType::Int));
        assert!(matches!(f[6].ftype, FieldType::Score));
    }

    #[test]
    fn dsl_field_desc_optional_and_with_colon() {
        let f = parse_fields_dsl("title:string:The page H1 title (see https://x/y)").unwrap();
        assert_eq!(f[0].desc, "The page H1 title (see https://x/y)");
    }

    #[test]
    fn dsl_rejects_unknown_type() {
        assert!(parse_fields_dsl("x:blorp").is_err());
    }

    #[test]
    fn dsl_rejects_empty_enum_members() {
        assert!(parse_fields_dsl("section:enum(Blog,,Docs)").is_err());
    }

    #[test]
    fn dsl_rejects_duplicate_field_names() {
        assert!(parse_fields_dsl("score:score, score:int").is_err());
    }

    #[test]
    fn dsl_rejects_reserved_field_names_case_insensitively() {
        for name in ["url", "PATH", "_Error", "_Evidence"] {
            assert!(parse_fields_dsl(&format!("{}:string", name)).is_err());
        }
    }

    #[test]
    fn rejects_unsafe_names_and_invalid_constraints() {
        assert!(parse_fields_dsl("bad key:string").is_err());

        let mut text_with_min = FieldSpec::new("label", FieldType::Str);
        text_with_min.min = Some(1.0);
        assert!(validate_field_names(&[text_with_min]).is_err());

        let mut wide_score = FieldSpec::new("score", FieldType::Score);
        wide_score.max = Some(101.0);
        assert!(validate_field_names(&[wide_score]).is_err());
    }

    #[test]
    fn json_schema_marks_required_and_enum() {
        let f = parse_fields_dsl("section:enum(A,B), n:int").unwrap();
        let s = json_schema_of(&f);
        assert_eq!(s["properties"]["section"]["enum"][0], "A");
        assert_eq!(s["properties"]["n"]["type"], "integer");
        assert_eq!(s["required"][0], "section");
        assert_eq!(s["additionalProperties"], false);
    }

    #[test]
    fn score_schema_has_bounds() {
        let f = parse_fields_dsl("q:score").unwrap();
        let s = json_schema_of(&f);
        assert_eq!(s["properties"]["q"]["minimum"], 0.0);
        assert_eq!(s["properties"]["q"]["maximum"], 100.0);
    }

    #[test]
    fn strict_schema_requires_optional_keys_as_nullable() {
        let mut optional = FieldSpec::new("note", FieldType::Text);
        optional.required = false;
        let s = json_schema_of(&[optional]);
        assert_eq!(s["required"], json!(["note"]));
        assert_eq!(s["properties"]["note"]["type"], json!(["string", "null"]));
    }

    #[test]
    fn findings_schema_is_recursively_strict() {
        let s = json_schema_of(&[FieldSpec::new("findings", FieldType::Findings)]);
        let item = &s["properties"]["findings"]["items"];
        assert_eq!(item["additionalProperties"], false);
        assert_eq!(
            item["required"],
            json!(["severity", "category", "rule", "excerpt", "recommendation"])
        );
    }

    #[test]
    fn every_preset_schema_is_recursively_openai_strict() {
        fn assert_strict(value: &Value, path: &str) {
            if value.get("type").and_then(Value::as_str) == Some("object") {
                assert_eq!(
                    value.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "object at {path} must reject additional properties"
                );
                let properties = value["properties"].as_object().expect("object properties");
                let required = value["required"].as_array().expect("object required list");
                assert_eq!(
                    properties.len(),
                    required.len(),
                    "every property at {path} must be required (optional values are nullable)"
                );
                for key in properties.keys() {
                    assert!(
                        required.iter().any(|item| item.as_str() == Some(key)),
                        "missing {path}.{key}"
                    );
                }
            }
            if let Some(properties) = value.get("properties").and_then(Value::as_object) {
                for (key, child) in properties {
                    assert_strict(child, &format!("{path}.{key}"));
                }
            }
            if let Some(items) = value.get("items") {
                assert_strict(items, &format!("{path}[]"));
            }
        }

        for key in PRESET_KEYS {
            let preset = preset_by_key(key).expect("known preset");
            assert_strict(&json_schema_of(&preset.fields), key);
        }
    }

    #[test]
    fn nominal_types_and_numeric_bounds_reach_native_schema() {
        let mut count = FieldSpec::new("count", FieldType::Int);
        count.min = Some(1.0);
        count.max = Some(10.0);
        let s = json_schema_of(&[
            FieldSpec::new("urlValue", FieldType::Url),
            FieldSpec::new("pathValue", FieldType::Path),
            FieldSpec::new("dateValue", FieldType::Date),
            count,
        ]);
        assert_eq!(s["properties"]["urlValue"]["format"], "uri");
        assert_eq!(s["properties"]["pathValue"]["pattern"], r"^/");
        assert_eq!(s["properties"]["dateValue"]["format"], "date");
        assert_eq!(s["properties"]["count"]["minimum"], 1.0);
        assert_eq!(s["properties"]["count"]["maximum"], 10.0);
    }

    #[test]
    fn output_block_mentions_all_fields() {
        let f = parse_fields_dsl("title:string, q:score").unwrap();
        let b = output_schema_block(&f);
        assert!(b.contains("\"title\""));
        assert!(b.contains("\"q\""));
        assert!(b.contains("integer 0-100"));
    }

    #[test]
    fn rich_schema_rejects_enum_that_masks_an_incompatible_type() {
        let path = std::env::temp_dir().join(format!(
            "siteone-ai-schema-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, r#"[{"name":"kind","type":"int","enum":["A","B"]}]"#).unwrap();
        let result = load_schema_file(path.to_str().unwrap());
        let _ = std::fs::remove_file(path);
        assert!(result.unwrap_err().contains("incompatible type"));
    }
}
