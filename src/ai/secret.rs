// SiteOne Crawler - SecretString
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A wrapper around a secret String (e.g. an API key) that redacts itself in both
// `Debug` and `serde::Serialize`. This guarantees the secret is never leaked when
// `CoreOptions` is `{:?}`-printed or serialized into the JSON output / config dump.

use std::fmt;

/// A string whose value is hidden from Debug and Serialize output.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Reveal the underlying secret. Use only at the point of actually building the request.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never print the real value.
        f.write_str("\"***\"")
    }
}

impl serde::Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Serialize a redaction marker, never the real value.
        serializer.serialize_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = SecretString::new("sk-super-secret-value");
        assert_eq!(format!("{:?}", s), "\"***\"");
        assert!(!format!("{:?}", s).contains("secret"));
    }

    #[test]
    fn serialize_is_redacted() {
        let s = SecretString::new("sk-super-secret-value");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"***\"");
        assert!(!json.contains("secret"));
    }

    #[test]
    fn expose_returns_real_value() {
        let s = SecretString::new("real");
        assert_eq!(s.expose(), "real");
    }
}
