// SiteOne Crawler - AI profile: classification phase (P3)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Maps (domain + site description) to one of the 13 subject types by its 1-based catalog id. The
// model only ever returns an integer id, so a hallucinated/unknown type is impossible.

use serde::Deserialize;

use crate::ai::provider::{ChatMessage, ChatRequest};

use super::registry::{self, TYPE_KEYS};

#[derive(Debug, Deserialize)]
struct TypePick {
    #[serde(rename = "type")]
    type_id: i64,
}

pub fn build_request(host: &str, site_description: &str, out_tokens: u32) -> ChatRequest {
    let system = super::promptpack::render(registry::CLASSIFY, &[("catalog", &registry::classifier_catalog())]);
    let user = format!("Domain: {}\nDescription: {}", host, site_description);
    ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(user)],
        max_tokens: out_tokens.min(256),
        temperature: 0.0,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    }
}

/// Parse the `{"type": <int>}` response and validate the id is in 1..=13.
pub fn parse(raw: &str) -> Result<usize, String> {
    let normalized = crate::ai::normalize::normalize_json_response(raw);
    let pick: TypePick = serde_json::from_str(&normalized)
        .or_else(|_| serde_json::from_str(&crate::ai::normalize::repair_json(raw)))
        .map_err(|e| format!("classification response is not valid JSON: {e}"))?;
    let id = pick.type_id;
    if (1..=TYPE_KEYS.len() as i64).contains(&id) {
        Ok(id as usize)
    } else {
        Err(format!("classification id {id} out of range 1..={}", TYPE_KEYS.len()))
    }
}

pub fn key_for(id: usize) -> Option<&'static str> {
    TYPE_KEYS.get(id.checked_sub(1)?).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_id_and_maps_to_key() {
        assert_eq!(parse(r#"{"type": 5}"#).unwrap(), 5);
        assert_eq!(key_for(5), Some("personal"));
        assert_eq!(key_for(13), Some("general"));
    }

    #[test]
    fn rejects_out_of_range_and_garbage() {
        assert!(parse(r#"{"type": 0}"#).is_err());
        assert!(parse(r#"{"type": 14}"#).is_err());
        assert!(parse("not json").is_err());
    }

    #[test]
    fn tolerates_code_fences() {
        assert_eq!(parse("```json\n{\"type\": 1}\n```").unwrap(), 1);
    }
}
