// SiteOne Crawler - Option definition and value parsing
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::sync::Mutex;

use regex::Regex;

use crate::error::CrawlerError;
use crate::utils;

use super::option_type::OptionType;

static EXTRAS_DOMAIN: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub enum OptionValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<String>),
}

impl OptionValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            OptionValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            OptionValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            OptionValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            OptionValue::Str(v) => Some(v.as_str()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<String>> {
        match self {
            OptionValue::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, OptionValue::None)
    }
}

#[derive(Debug, Clone)]
pub struct CrawlerOption {
    /// Option name with '--' prefix, for example "--user-agent"
    pub name: String,

    /// Optional alternative (short) name with '-', for example "-ua" for "--user-agent"
    pub alt_name: Option<String>,

    /// Property name to fill in CoreOptions struct
    pub property_to_fill: String,

    /// Option value type
    pub option_type: OptionType,

    /// Is array of comma delimited values
    pub is_array: bool,

    /// Description for help
    pub description: String,

    /// Default value as string representation
    pub default_value: Option<String>,

    /// Whether the value can be null/empty
    pub is_nullable: bool,

    /// Whether the option can be specified multiple times
    pub callable_multiple_times: bool,

    /// Optional extras (e.g. min/max range for numeric types)
    pub extras: Option<Vec<String>>,

    /// Parsed value from argv
    value: Option<OptionValue>,

    /// Whether value has been set from argv
    is_value_set: bool,

    /// Whether the user explicitly provided this option on the command line
    /// (as opposed to using the default value)
    is_explicitly_set: bool,
}

impl CrawlerOption {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        alt_name: Option<&str>,
        property_to_fill: &str,
        option_type: OptionType,
        is_array: bool,
        description: &str,
        default_value: Option<&str>,
        is_nullable: bool,
        callable_multiple_times: bool,
        extras: Option<Vec<String>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            alt_name: alt_name.map(|s| s.to_string()),
            property_to_fill: property_to_fill.to_string(),
            option_type,
            is_array,
            description: description.to_string(),
            default_value: default_value.map(|s| s.to_string()),
            is_nullable,
            callable_multiple_times,
            extras,
            value: None,
            is_value_set: false,
            is_explicitly_set: false,
        }
    }

    pub fn set_value_from_argv(&mut self, argv: &[String]) -> Result<(), CrawlerError> {
        if self.is_value_set {
            return Err(CrawlerError::Config(format!(
                "Value for option {} is already set. Did you call set_value_from_argv() twice?",
                self.name
            )));
        }

        let mut value: Option<String> = self.default_value.clone();
        let mut array_values: Vec<String> = if self.is_array {
            if let Some(ref dv) = self.default_value {
                if dv.is_empty() { Vec::new() } else { vec![dv.clone()] }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let mut has_default_been_replaced = false;
        let mut defined_by_alt_name = false;

        // Find value in arguments
        let mut i = 0;
        while i < argv.len() {
            let arg = &argv[i];
            let mut arg_value: Option<String> = None;

            if arg == &self.name || self.alt_name.as_deref() == Some(arg.as_str()) {
                if self.option_type == OptionType::Bool {
                    // Flag-style: --debug or -d (no value, implies true)
                    arg_value = Some("true".to_string());
                } else {
                    // Non-bool option without '=': look for value in next argument
                    if i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                        i += 1;
                        arg_value = Some(argv[i].clone());
                    } else {
                        // No value provided — set to empty so validation catches it
                        arg_value = Some(String::new());
                    }
                }
            } else if let Some(rest) = arg.strip_prefix(&format!("{}=", self.name)) {
                arg_value = Some(rest.to_string());
            } else if let Some(ref alt) = self.alt_name
                && let Some(rest) = arg.strip_prefix(&format!("{}=", alt))
            {
                arg_value = Some(rest.to_string());
                defined_by_alt_name = true;
            }

            if let Some(ref mut av) = arg_value {
                self.is_explicitly_set = true;
                unquote_value(av);

                if self.is_array {
                    if !has_default_been_replaced {
                        // First user-provided value replaces the default
                        array_values.clear();
                        has_default_been_replaced = true;
                    }
                    if av.contains(',') {
                        let parts: Vec<String> = av
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .map(|mut s| {
                                unquote_value(&mut s);
                                s
                            })
                            .collect();
                        array_values.extend(parts);
                    } else {
                        array_values.push(av.clone());
                    }
                } else {
                    value = Some(av.clone());
                }
            }
            i += 1;
        }

        // Handle array default from string
        if self.is_array
            && let Some(ref v) = value
            && !v.is_empty()
            && array_values.is_empty()
        {
            let mut unquoted = v.clone();
            unquote_value(&mut unquoted);
            let parts: Vec<String> = unquoted
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|mut s| {
                    unquote_value(&mut s);
                    s
                })
                .collect();
            array_values = parts;
        }

        // Validate and correct types
        if self.is_array {
            for item in &array_values {
                self.validate_value(Some(item), defined_by_alt_name)?;
            }
            // Filter out empty strings
            let filtered: Vec<String> = array_values.into_iter().filter(|s| !s.trim().is_empty()).collect();
            self.value = Some(OptionValue::Array(filtered));
        } else {
            self.validate_value(value.as_deref(), defined_by_alt_name)?;
            self.value = Some(self.correct_value_type(value.as_deref())?);
        }

        self.is_value_set = true;
        Ok(())
    }

    pub fn is_explicitly_set(&self) -> bool {
        self.is_explicitly_set
    }

    pub fn get_value(&self) -> Result<&OptionValue, CrawlerError> {
        if !self.is_value_set {
            return Err(CrawlerError::Config(format!(
                "Value for option {} is not set. Did you call set_value_from_argv()?",
                self.name
            )));
        }
        match &self.value {
            Some(v) => Ok(v),
            None => Err(CrawlerError::Config(format!(
                "Value for option {} is not set",
                self.name
            ))),
        }
    }

    fn validate_value(&self, value: Option<&str>, _defined_by_alt_name: bool) -> Result<(), CrawlerError> {
        // Always use the long name for error messages
        let display_name = &self.name;

        // Handle nullable
        if self.is_nullable && (value.is_none() || value == Some("")) {
            return Ok(());
        }

        let val = match value {
            Some(v) => v,
            None => {
                if !self.is_nullable {
                    // URL type gives specific error, not generic "is required"
                    if self.option_type == OptionType::Url {
                        return Err(CrawlerError::Config(format!(
                            "Option {} must be valid URL (starting with http:// or https://)",
                            display_name
                        )));
                    }
                    return Err(CrawlerError::Config(format!("Option {} is required", display_name)));
                }
                return Ok(());
            }
        };

        match self.option_type {
            OptionType::Int => {
                let parsed: Result<i64, _> = val.parse();
                match parsed {
                    Ok(n) if n < 0 => {
                        return Err(CrawlerError::Config(format!(
                            "Option {} ({}) must be positive integer",
                            display_name, val
                        )));
                    }
                    Err(_) => {
                        return Err(CrawlerError::Config(format!(
                            "Option {} ({}) must be positive integer",
                            display_name, val
                        )));
                    }
                    _ => {}
                }
            }
            OptionType::Float => {
                if val.parse::<f64>().is_err() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be float",
                        display_name, val
                    )));
                }
            }
            OptionType::Bool => {
                if !["1", "0", "yes", "no", "true", "false"].contains(&val) {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be boolean (1/0, yes/no, true/false)",
                        display_name, val
                    )));
                }
            }
            OptionType::String => {
                // Strings are always valid
            }
            OptionType::SizeMG => {
                let re = Regex::new(r"^\d+(\.\d+)?[MG]$").map_err(|e| CrawlerError::Config(e.to_string()))?;
                if !re.is_match(val) {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be string with M/G suffix (for example 512M or 1.5G)",
                        display_name, val
                    )));
                }
            }
            OptionType::Regex => {
                if fancy_regex::Regex::new(val).is_err() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be valid PCRE regular expression",
                        display_name, val
                    )));
                }
            }
            OptionType::Url => {
                let corrected = correct_url(val);
                if corrected.is_empty() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} must be valid URL (starting with http:// or https://)",
                        display_name
                    )));
                }
                if url::Url::parse(&corrected).is_err() {
                    // Try with URL-encoded version for international characters
                    let encoded: String = corrected
                        .chars()
                        .map(|c| {
                            if c.is_ascii_graphic() || c == ' ' {
                                c.to_string()
                            } else {
                                percent_encoding::utf8_percent_encode(
                                    &c.to_string(),
                                    percent_encoding::NON_ALPHANUMERIC,
                                )
                                .to_string()
                            }
                        })
                        .collect();
                    if url::Url::parse(&encoded).is_err() {
                        return Err(CrawlerError::Config(format!(
                            "Option {} ({}) must be valid URL",
                            display_name, val
                        )));
                    }
                }
            }
            OptionType::Email => {
                // Simple email validation
                if !val.contains('@') || !val.contains('.') {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be valid email '{}'",
                        display_name, val, val
                    )));
                }
            }
            OptionType::File => {
                // File path validation - just ensure it's a non-empty string.
                // Writability is checked at export time.
            }
            OptionType::Dir => {
                if val == "off" || val.is_empty() {
                    return Ok(());
                }
                let mut path = val.to_string();
                replace_placeholders(&mut path);
                let abs_path = utils::get_absolute_path(&path);
                if abs_path.trim().is_empty() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be string",
                        display_name, val
                    )));
                }
                let dir_path = std::path::Path::new(&abs_path);
                if !dir_path.exists() && std::fs::create_dir_all(dir_path).is_err() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be valid and writable directory. Check permissions.",
                        display_name, abs_path
                    )));
                }
            }
            OptionType::HostAndPort => {
                let re = Regex::new(r"^[a-zA-Z0-9\-.:]{1,100}:[0-9]{1,5}$")
                    .map_err(|e| CrawlerError::Config(e.to_string()))?;
                if !re.is_match(val) {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be in format host:port",
                        display_name, val
                    )));
                }
            }
            OptionType::ReplaceContent => {
                let re = Regex::new(r"^.+->").map_err(|e| CrawlerError::Config(e.to_string()))?;
                if !re.is_match(val) {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be in format `foo -> bar` or `/preg-regexp/ -> bar`)",
                        display_name, val
                    )));
                }

                let parts: Vec<&str> = val.splitn(2, "->").collect();
                let replace_from = parts[0].trim();
                let is_regex = crate::utils::is_regex_pattern(replace_from);

                if is_regex && Regex::new(replace_from).is_err() {
                    return Err(CrawlerError::Config(format!(
                        "Option {} and its first part ({}) must be valid PCRE regular expression",
                        display_name, replace_from
                    )));
                }
            }
            OptionType::Resolve => {
                // --resolve is in the same format as curl --resolve (ipv4 and ipv6 supported)
                let re = Regex::new(r"^[a-zA-Z0-9\-.]{1,200}:[0-9]{1,5}:[a-fA-F0-9\-.:]{1,100}$")
                    .map_err(|e| CrawlerError::Config(e.to_string()))?;
                if !re.is_match(val) {
                    return Err(CrawlerError::Config(format!(
                        "Option {} ({}) must be in format `domain:port:ip`",
                        display_name, val
                    )));
                }
            }
        }

        // Extra validations for numeric range
        if (self.option_type == OptionType::Int || self.option_type == OptionType::Float)
            && self.extras.as_ref().map(|e| e.len()) == Some(2)
            && let Ok(num) = val.parse::<f64>()
        {
            let extras = self.extras.as_ref().map(|e| {
                let min = e[0].parse::<f64>().unwrap_or(f64::MIN);
                let max = e[1].parse::<f64>().unwrap_or(f64::MAX);
                (min, max)
            });
            if let Some((min, max)) = extras
                && (num < min || num > max)
            {
                return Err(CrawlerError::Config(format!(
                    "Option {} ({}) must be in range {}-{}",
                    display_name, val, min, max
                )));
            }
        }

        Ok(())
    }

    fn correct_value_type(&self, value: Option<&str>) -> Result<OptionValue, CrawlerError> {
        if self.is_nullable && (value.is_none() || value == Some("")) {
            return Ok(OptionValue::None);
        }

        let val = match value {
            Some(v) => v,
            None => return Ok(OptionValue::None),
        };

        match self.option_type {
            OptionType::Int => {
                let n = val
                    .parse::<i64>()
                    .map_err(|_| CrawlerError::Config(format!("Cannot parse '{}' as integer", val)))?;
                Ok(OptionValue::Int(n))
            }
            OptionType::Float => {
                let n = val
                    .parse::<f64>()
                    .map_err(|_| CrawlerError::Config(format!("Cannot parse '{}' as float", val)))?;
                Ok(OptionValue::Float(n))
            }
            OptionType::Bool => {
                let b = ["1", "yes", "true"].contains(&val);
                Ok(OptionValue::Bool(b))
            }
            OptionType::String
            | OptionType::SizeMG
            | OptionType::Regex
            | OptionType::Email
            | OptionType::HostAndPort
            | OptionType::ReplaceContent
            | OptionType::Resolve => Ok(OptionValue::Str(val.to_string())),
            OptionType::Url => {
                let corrected = correct_url(val);
                Ok(OptionValue::Str(corrected))
            }
            OptionType::File => {
                let mut path = val.to_string();
                replace_placeholders(&mut path);
                Ok(OptionValue::Str(utils::get_absolute_path(&path)))
            }
            OptionType::Dir => {
                if val == "off" || val.is_empty() {
                    return Ok(OptionValue::Str(val.to_string()));
                }
                let mut path = val.to_string();
                replace_placeholders(&mut path);
                Ok(OptionValue::Str(utils::get_absolute_path(&path)))
            }
        }
    }

    pub fn set_extras_domain(domain: Option<&str>) {
        if let Ok(mut d) = EXTRAS_DOMAIN.lock() {
            *d = domain.map(|s| s.to_string());
        }
    }
}

/// Correct URL to valid URL, e.g. crawler.siteone.io => https://crawler.siteone.io,
/// or localhost to http://localhost
fn correct_url(url: &str) -> String {
    if !url.starts_with("http") {
        let re = Regex::new(r"^[a-zA-Z0-9\-.:]{1,100}$").ok();
        if re.map(|r| r.is_match(url)).unwrap_or(false) {
            let default_protocol = if url.contains('.') { "https" } else { "http" };
            return format!("{}://{}", default_protocol, url.trim_start_matches('/'));
        }
    }
    url.to_string()
}

/// Remove quotes from given string - as a quote we consider chars " ' `
fn unquote_value(value: &mut String) {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') || (first == b'`' && last == b'`') {
            *value = value[1..value.len() - 1].to_string();
        }
    }
}

/// Replace placeholders like %domain%, %date%, %datetime% in file/dir paths
fn replace_placeholders(value: &mut String) {
    let domain = EXTRAS_DOMAIN.lock().ok().and_then(|d| d.clone()).unwrap_or_default();

    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let datetime = now.format("%Y%m%d-%H%M%S").to_string();

    *value = value
        .replace("%domain%", &domain)
        .replace("%date%", &date)
        .replace("%datetime%", &datetime);
}
