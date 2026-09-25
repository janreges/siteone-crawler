// SiteOne Crawler - SecretString
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A wrapper around a secret String (e.g. an API key) that redacts itself in both
// `Debug` and `serde::Serialize`. This guarantees the secret is never leaked when
// `CoreOptions` is `{:?}`-printed or serialized into the JSON output / config dump.
// `Redactor` masks the credentials of an AI connection in the text the AI client reports.

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

/// Credentials shorter than this are not blanked out of a text; the text is withheld instead.
///
/// Blanking a short credential out of ordinary words mangles them, and where the blanks fall
/// spells it back out: `a` taken out of "invalid API key" reads "inv[redacted]lid API key".
const MIN_BLANKED_CHARS: usize = 8;

/// What is said instead of a text that repeats a short credential.
const WITHHELD: &str = "[withheld: the text repeats a credential of the AI connection]";

/// Masks the credentials of an AI connection in text bound for the console, an event or a file:
/// the API key, the user and the password of the endpoint and the values of its query (each as
/// written and percent-decoded), and the userinfo of any URL the text quotes.
pub struct Redactor {
    /// Longest first, so a credential that contains a shorter one is blanked whole.
    secrets: Vec<String>,
}

impl Redactor {
    pub fn new(api_key: Option<&str>, endpoint: &str) -> Self {
        let mut secrets: Vec<String> = api_key.into_iter().map(str::to_string).collect();
        if let Ok(url) = url::Url::parse(endpoint) {
            let query_values = url
                .query()
                .unwrap_or_default()
                .split('&')
                .map(|pair| pair.split_once('=').map_or(pair, |(_, value)| value));
            for raw in [url.username()].into_iter().chain(url.password()).chain(query_values) {
                secrets.push(raw.to_string());
                secrets.push(
                    percent_encoding::percent_decode_str(raw)
                        .decode_utf8_lossy()
                        .into_owned(),
                );
            }
            secrets.extend(url.query_pairs().map(|(_, value)| value.into_owned()));
        }
        secrets.retain(|secret| !secret.is_empty());
        secrets.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then_with(|| a.cmp(b)));
        secrets.dedup();
        Self { secrets }
    }

    /// `text` without URL userinfo, with the long credentials blanked out — or withheld as a
    /// whole when it still repeats a short one.
    pub fn redact(&self, text: &str) -> String {
        static USERINFO: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
            regex::Regex::new(r#"(?i)([a-z][a-z0-9+.\-]*://)[^\s/?#"'<>]*@"#).expect("a valid pattern")
        });
        let (long, short): (Vec<&String>, Vec<&String>) = self
            .secrets
            .iter()
            .partition(|secret| secret.chars().count() >= MIN_BLANKED_CHARS);
        let blanked = long
            .into_iter()
            .fold(USERINFO.replace_all(text, "$1").into_owned(), |text, secret| {
                text.replace(secret.as_str(), "[redacted]")
            });
        if short.iter().any(|secret| blanked.contains(secret.as_str())) {
            return WITHHELD.to_string();
        }
        blanked
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
