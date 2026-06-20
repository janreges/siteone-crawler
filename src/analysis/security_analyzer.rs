// SiteOne Crawler - SecurityAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

use regex::Regex;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::security_checked_header::{
    SEVERITY_CRITICAL, SEVERITY_NOTICE, SEVERITY_OK, SEVERITY_WARNING,
};
use crate::analysis::result::security_result::SecurityResult;
use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

const SUPER_TABLE_SECURITY: &str = "security";
const ANALYSIS_HEADERS: &str = "Security headers";

const HEADER_ACCESS_CONTROL_ALLOW_ORIGIN: &str = "access-control-allow-origin";
const HEADER_STRICT_TRANSPORT_SECURITY: &str = "strict-transport-security";
const HEADER_X_FRAME_OPTIONS: &str = "x-frame-options";
const HEADER_X_XSS_PROTECTION: &str = "x-xss-protection";
const HEADER_X_CONTENT_TYPE_OPTIONS: &str = "x-content-type-options";
const HEADER_REFERRER_POLICY: &str = "referrer-policy";
const HEADER_CONTENT_SECURITY_POLICY: &str = "content-security-policy";
const HEADER_FEATURE_POLICY: &str = "feature-policy";
const HEADER_PERMISSIONS_POLICY: &str = "permissions-policy";
const HEADER_SERVER: &str = "server";
const HEADER_X_POWERED_BY: &str = "x-powered-by";
const HEADER_SET_COOKIE: &str = "set-cookie";
const HEADER_CROSS_ORIGIN_OPENER_POLICY: &str = "cross-origin-opener-policy";
const HEADER_CROSS_ORIGIN_EMBEDDER_POLICY: &str = "cross-origin-embedder-policy";
const HEADER_CROSS_ORIGIN_RESOURCE_POLICY: &str = "cross-origin-resource-policy";

const CHECKED_HEADERS: &[&str] = &[
    HEADER_ACCESS_CONTROL_ALLOW_ORIGIN,
    HEADER_STRICT_TRANSPORT_SECURITY,
    HEADER_X_FRAME_OPTIONS,
    HEADER_X_XSS_PROTECTION,
    HEADER_X_CONTENT_TYPE_OPTIONS,
    HEADER_REFERRER_POLICY,
    HEADER_CONTENT_SECURITY_POLICY,
    HEADER_FEATURE_POLICY,
    HEADER_PERMISSIONS_POLICY,
    HEADER_SERVER,
    HEADER_X_POWERED_BY,
    HEADER_SET_COOKIE,
    HEADER_CROSS_ORIGIN_OPENER_POLICY,
    HEADER_CROSS_ORIGIN_EMBEDDER_POLICY,
    HEADER_CROSS_ORIGIN_RESOURCE_POLICY,
];

pub struct SecurityAnalyzer {
    base: BaseAnalyzer,
    result: SecurityResult,
    pages_with_critical: usize,
    pages_with_warning: usize,
    pages_with_notice: usize,
}

impl Default for SecurityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            result: SecurityResult::new(),
            pages_with_critical: 0,
            pages_with_warning: 0,
            pages_with_notice: 0,
        }
    }

    fn check_headers(&mut self, headers: &HashMap<String, String>, is_https: bool, url_result: &mut UrlAnalysisResult) {
        for &header in CHECKED_HEADERS {
            match header {
                HEADER_ACCESS_CONTROL_ALLOW_ORIGIN => {
                    self.check_access_control_allow_origin(headers, url_result);
                }
                HEADER_STRICT_TRANSPORT_SECURITY => {
                    if is_https {
                        self.check_strict_transport_security(headers, url_result);
                    }
                }
                HEADER_X_FRAME_OPTIONS => {
                    self.check_x_frame_options(headers, url_result);
                }
                HEADER_X_XSS_PROTECTION => {
                    self.check_x_xss_protection(headers, url_result);
                }
                HEADER_X_CONTENT_TYPE_OPTIONS => {
                    self.check_x_content_type_options(headers, url_result);
                }
                HEADER_REFERRER_POLICY => {
                    self.check_referrer_policy(headers, url_result);
                }
                HEADER_CONTENT_SECURITY_POLICY => {
                    self.check_content_security_policy(headers, url_result);
                }
                HEADER_FEATURE_POLICY => {
                    self.check_feature_policy(headers, url_result);
                }
                HEADER_PERMISSIONS_POLICY => {
                    self.check_permissions_policy(headers, url_result);
                }
                HEADER_SERVER => {
                    self.check_server(headers, url_result);
                }
                HEADER_X_POWERED_BY => {
                    self.check_x_powered_by(headers, url_result);
                }
                HEADER_SET_COOKIE => {
                    self.check_set_cookie(headers, is_https, url_result);
                }
                HEADER_CROSS_ORIGIN_OPENER_POLICY => {
                    self.check_cross_origin_header(
                        headers,
                        HEADER_CROSS_ORIGIN_OPENER_POLICY,
                        "Cross-Origin-Opener-Policy is not set. 'same-origin' isolates your browsing context from cross-origin popups (mitigates Spectre/XS-Leaks).",
                        url_result,
                    );
                }
                HEADER_CROSS_ORIGIN_EMBEDDER_POLICY => {
                    self.check_cross_origin_header(
                        headers,
                        HEADER_CROSS_ORIGIN_EMBEDDER_POLICY,
                        "Cross-Origin-Embedder-Policy is not set. 'require-corp' enables cross-origin isolation for powerful features.",
                        url_result,
                    );
                }
                HEADER_CROSS_ORIGIN_RESOURCE_POLICY => {
                    self.check_cross_origin_header(
                        headers,
                        HEADER_CROSS_ORIGIN_RESOURCE_POLICY,
                        "Cross-Origin-Resource-Policy is not set. 'same-origin' or 'same-site' blocks other sites from embedding this resource.",
                        url_result,
                    );
                }
                _ => {}
            }
        }
    }

    fn check_html_security(&mut self, html: &str, is_https: bool, url_result: &mut UrlAnalysisResult) {
        if !is_https {
            return;
        }

        use once_cell::sync::Lazy;
        static RE_FORM_HTTP: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?i)<form[^>]*action=["']http://[^"']+["'][^>]*>"#).unwrap());
        static RE_IFRAME_HTTP: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?i)<iframe[^>]*src=["']http://[^"']+["'][^>]*>"#).unwrap());
        // Active mixed content — script/stylesheet loaded over HTTP can fully MITM the page.
        static RE_SCRIPT_HTTP: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?i)<script[^>]*src=["']http://[^"']+["']"#).unwrap());
        static RE_LINK_HTTP: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?i)<link[^>]*href=["']http://[^"']+["']"#).unwrap());
        // Passive mixed content — images/media over HTTP (lower risk but still a warning).
        static RE_MEDIA_HTTP: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?i)<(?:img|audio|video|source)[^>]*src=["']http://[^"']+["']"#).unwrap());

        // Check for form actions over non-secure HTTP
        for mat in RE_FORM_HTTP.find_iter(html) {
            let finding = format!(
                "Form actions that send data over non-secure HTTP detected in {}",
                mat.as_str()
            );
            url_result.add_critical(finding.clone(), ANALYSIS_HEADERS, Some(vec![finding]));
        }

        // Check for iframes with non-secure HTTP
        for mat in RE_IFRAME_HTTP.find_iter(html) {
            let finding = format!("Iframe with non-secure HTTP detected in {}", mat.as_str());
            url_result.add_critical(finding.clone(), ANALYSIS_HEADERS, Some(vec![finding]));
        }

        // Active mixed content (script/stylesheet over HTTP) — critical
        for mat in RE_SCRIPT_HTTP.find_iter(html).chain(RE_LINK_HTTP.find_iter(html)) {
            let finding = format!("Active mixed content loaded over HTTP detected in {}", mat.as_str());
            url_result.add_critical(finding.clone(), ANALYSIS_HEADERS, Some(vec![finding]));
        }

        // Passive mixed content (image/media over HTTP) — warning
        for mat in RE_MEDIA_HTTP.find_iter(html) {
            let finding = format!("Passive mixed content (media over HTTP) detected in {}", mat.as_str());
            url_result.add_warning(finding.clone(), ANALYSIS_HEADERS, Some(vec![finding]));
        }
    }

    fn get_header_value(headers: &HashMap<String, String>, header: &str) -> Option<String> {
        headers.get(header).map(|s| s.to_string())
    }

    fn check_access_control_allow_origin(
        &mut self,
        headers: &HashMap<String, String>,
        url_result: &mut UrlAnalysisResult,
    ) {
        let value = Self::get_header_value(headers, HEADER_ACCESS_CONTROL_ALLOW_ORIGIN);

        let value_ref = value.as_deref();
        match value_ref {
            None => {}
            Some("*") => {
                let rec = "Access-Control-Allow-Origin is set to '*' which allows any origin to access the resource. This can be a security risk.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_ACCESS_CONTROL_ALLOW_ORIGIN)
                    .set_finding(value_ref, SEVERITY_WARNING, Some(rec));
            }
            Some(v) if v != "same-origin" && v != "none" => {
                let rec = format!(
                    "Access-Control-Allow-Origin is set to '{}' which allows this origin to access the resource.",
                    v
                );
                url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result
                    .get_checked_header(HEADER_ACCESS_CONTROL_ALLOW_ORIGIN)
                    .set_finding(value_ref, SEVERITY_NOTICE, Some(&rec));
            }
            _ => {
                self.result
                    .get_checked_header(HEADER_ACCESS_CONTROL_ALLOW_ORIGIN)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
        }
    }

    fn check_strict_transport_security(
        &mut self,
        headers: &HashMap<String, String>,
        url_result: &mut UrlAnalysisResult,
    ) {
        let value = Self::get_header_value(headers, HEADER_STRICT_TRANSPORT_SECURITY);
        let value_ref = value.as_deref();

        match value_ref {
            None => {
                let rec = "Strict-Transport-Security header is not set. It enforces secure connections and protects against MITM attacks.";
                url_result.add_critical(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_STRICT_TRANSPORT_SECURITY)
                    .set_finding(None, SEVERITY_CRITICAL, Some(rec));
            }
            Some(v) if v.contains("max-age=0") => {
                let rec = "Strict-Transport-Security header is set to max-age=0 which disables HSTS. This can be a security risk.";
                url_result.add_critical(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_STRICT_TRANSPORT_SECURITY)
                    .set_finding(value_ref, SEVERITY_CRITICAL, Some(rec));
            }
            Some(v) => {
                use once_cell::sync::Lazy;
                static RE_MAX_AGE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)max-age=([0-9]+)").unwrap());
                if let Some(caps) = RE_MAX_AGE.captures(v)
                    && let Some(age_str) = caps.get(1)
                    && let Ok(age) = age_str.as_str().parse::<i64>()
                    && age < 31 * 24 * 60 * 60
                {
                    let rec = format!(
                        "Strict-Transport-Security header is set to max-age={} which is less than 31 days. This can be a security risk.",
                        age
                    );
                    url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                    self.result
                        .get_checked_header(HEADER_STRICT_TRANSPORT_SECURITY)
                        .set_finding(value_ref, SEVERITY_WARNING, Some(&rec));
                    return;
                }
                self.result
                    .get_checked_header(HEADER_STRICT_TRANSPORT_SECURITY)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
        }
    }

    fn check_x_frame_options(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_X_FRAME_OPTIONS);
        let value_ref = value.as_deref();

        match value_ref {
            None => {
                let rec = "X-Frame-Options header is not set. It prevents clickjacking attacks when set to 'deny' or 'sameorigin.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_X_FRAME_OPTIONS)
                    .set_finding(None, SEVERITY_WARNING, Some(rec));
            }
            Some("DENY") => {
                self.result
                    .get_checked_header(HEADER_X_FRAME_OPTIONS)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
            Some("SAMEORIGIN") => {
                let rec = "X-Frame-Options header is set to SAMEORIGIN which allows this origin to embed the resource in a frame.";
                url_result.add_notice(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result.get_checked_header(HEADER_X_FRAME_OPTIONS).set_finding(
                    value_ref,
                    SEVERITY_NOTICE,
                    Some(rec),
                );
            }
            Some("ALLOW-FROM") => {
                let rec = "X-Frame-Options header is set to ALLOW-FROM which allows this origin to embed the resource in a frame.";
                url_result.add_notice(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result.get_checked_header(HEADER_X_FRAME_OPTIONS).set_finding(
                    value_ref,
                    SEVERITY_NOTICE,
                    Some(rec),
                );
            }
            Some(v) => {
                let rec = format!(
                    "X-Frame-Options header is set to '{}' which allows this origin to embed the resource in a frame. This can be a security risk.",
                    v
                );
                url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result.get_checked_header(HEADER_X_FRAME_OPTIONS).set_finding(
                    value_ref,
                    SEVERITY_WARNING,
                    Some(&rec),
                );
            }
        }
    }

    fn check_x_xss_protection(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_X_XSS_PROTECTION);
        let value_ref = value.as_deref();

        // X-XSS-Protection is deprecated (MDN) and non-standard. Modern browsers have removed
        // XSS auditor support. The recommended approach is to use Content-Security-Policy instead.
        // Not setting this header is the correct modern behavior.
        match value_ref {
            None | Some("0") => {
                // Not set or explicitly disabled — correct modern behavior
                self.result
                    .get_checked_header(HEADER_X_XSS_PROTECTION)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
            Some("1") | Some("1; mode=block") | Some("1;mode=block") => {
                let rec = "X-XSS-Protection header is set but deprecated. Consider removing it and using Content-Security-Policy instead.";
                url_result.add_notice(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result.get_checked_header(HEADER_X_XSS_PROTECTION).set_finding(
                    value_ref,
                    SEVERITY_NOTICE,
                    Some(rec),
                );
            }
            Some(v) => {
                let rec = format!(
                    "X-XSS-Protection header is set to '{}'. This header is deprecated; use Content-Security-Policy instead.",
                    v
                );
                url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result.get_checked_header(HEADER_X_XSS_PROTECTION).set_finding(
                    value_ref,
                    SEVERITY_NOTICE,
                    Some(&rec),
                );
            }
        }
    }

    fn check_x_content_type_options(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_X_CONTENT_TYPE_OPTIONS);
        let value_ref = value.as_deref();

        match value_ref {
            None => {
                let rec = "X-Content-Type-Options header is not set. It stops MIME type sniffing and mitigates content type attacks.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_X_CONTENT_TYPE_OPTIONS)
                    .set_finding(None, SEVERITY_WARNING, Some(rec));
            }
            Some("nosniff") => {
                self.result
                    .get_checked_header(HEADER_X_CONTENT_TYPE_OPTIONS)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
            Some(v) => {
                let rec = format!(
                    "X-Content-Type-Options header is set to '{}'. This can be a security risk.",
                    v
                );
                url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result
                    .get_checked_header(HEADER_X_CONTENT_TYPE_OPTIONS)
                    .set_finding(value_ref, SEVERITY_WARNING, Some(&rec));
            }
        }
    }

    fn check_referrer_policy(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_REFERRER_POLICY);
        let value_ref = value.as_deref();

        let ok_values = [
            "no-referrer",
            "no-referrer-when-downgrade",
            "origin",
            "origin-when-cross-origin",
            "same-origin",
            "strict-origin",
            "strict-origin-when-cross-origin",
            "unsafe-url",
        ];

        match value_ref {
            None => {
                let rec = "Referrer-Policy header is not set. It controls referrer header sharing and enhances privacy and security.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_REFERRER_POLICY)
                    .set_finding(None, SEVERITY_WARNING, Some(rec));
            }
            Some(v) if ok_values.contains(&v) => {
                self.result
                    .get_checked_header(HEADER_REFERRER_POLICY)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
            Some(v) => {
                let rec = format!("Referrer-Policy header is set to '{}'. This can be a security risk.", v);
                url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result.get_checked_header(HEADER_REFERRER_POLICY).set_finding(
                    value_ref,
                    SEVERITY_NOTICE,
                    Some(&rec),
                );
            }
        }
    }

    fn check_content_security_policy(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_CONTENT_SECURITY_POLICY);
        let value_ref = value.as_deref();

        match value_ref {
            None => {
                let rec = "Content-Security-Policy header is not set. It restricts resources the page can load and prevents XSS attacks.";
                url_result.add_critical(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_CONTENT_SECURITY_POLICY)
                    .set_finding(None, SEVERITY_CRITICAL, Some(rec));
            }
            Some(csp) => {
                // A CSP is present, but 'unsafe-inline'/'unsafe-eval' or a wildcard source largely
                // defeats its XSS protection, so it must not score as a perfect policy.
                let weaknesses = csp_weaknesses(csp);
                if weaknesses.is_empty() {
                    self.result
                        .get_checked_header(HEADER_CONTENT_SECURITY_POLICY)
                        .set_finding(value_ref, SEVERITY_OK, None);
                } else {
                    let rec = format!(
                        "Content-Security-Policy is set but weakened by {} which significantly reduces its XSS protection.",
                        weaknesses.join(", ")
                    );
                    url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                    self.result
                        .get_checked_header(HEADER_CONTENT_SECURITY_POLICY)
                        .set_finding(value_ref, SEVERITY_WARNING, Some(&rec));
                }
            }
        }
    }

    /// Generic checker for the Cross-Origin-* isolation headers (COOP/COEP/CORP). Absence is a
    /// notice (these are less universal than CSP/HSTS); any value is treated as OK.
    fn check_cross_origin_header(
        &mut self,
        headers: &HashMap<String, String>,
        header_name: &'static str,
        advice: &str,
        url_result: &mut UrlAnalysisResult,
    ) {
        let value = Self::get_header_value(headers, header_name);
        match value.as_deref() {
            None => {
                url_result.add_notice(advice.to_string(), ANALYSIS_HEADERS, Some(vec![advice.to_string()]));
                self.result
                    .get_checked_header(header_name)
                    .set_finding(None, SEVERITY_NOTICE, Some(advice));
            }
            value_ref => {
                self.result
                    .get_checked_header(header_name)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
        }
    }

    fn check_feature_policy(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_FEATURE_POLICY);
        let value_ref = value.as_deref();

        let has_permissions_policy = Self::get_header_value(headers, HEADER_PERMISSIONS_POLICY).is_some();

        match value_ref {
            None if has_permissions_policy => {
                let rec = "Feature-Policy header is not set but Permissions-Policy is set. That's enough.";
                url_result.add_notice(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_FEATURE_POLICY)
                    .set_finding(None, SEVERITY_NOTICE, Some(rec));
            }
            None => {
                let rec = "Feature-Policy header is not set. It allows enabling/disabling browser APIs and features for security. Not important if Permissions-Policy is set.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result
                    .get_checked_header(HEADER_FEATURE_POLICY)
                    .set_finding(None, SEVERITY_WARNING, Some(rec));
            }
            _ => {
                self.result
                    .get_checked_header(HEADER_FEATURE_POLICY)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
        }
    }

    fn check_permissions_policy(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_PERMISSIONS_POLICY);
        let value_ref = value.as_deref();

        let has_feature_policy = Self::get_header_value(headers, HEADER_FEATURE_POLICY).is_some();

        match value_ref {
            None if has_feature_policy => {
                let rec = "Permissions-Policy header is not set but Feature-Policy is. We recommend transforming it to this newer header.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result.get_checked_header(HEADER_PERMISSIONS_POLICY).set_finding(
                    None,
                    SEVERITY_WARNING,
                    Some(rec),
                );
            }
            None => {
                let rec = "Permissions-Policy header is not set. It allows enabling/disabling browser APIs and features for security.";
                url_result.add_warning(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
                self.result.get_checked_header(HEADER_PERMISSIONS_POLICY).set_finding(
                    None,
                    SEVERITY_WARNING,
                    Some(rec),
                );
            }
            _ => {
                self.result
                    .get_checked_header(HEADER_PERMISSIONS_POLICY)
                    .set_finding(value_ref, SEVERITY_OK, None);
            }
        }
    }

    fn check_server(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_SERVER);
        let value_ref = value.as_deref();

        let known_values = ["Apache", "nginx", "Microsoft-IIS"];

        let check_for_known = |v: &str| -> bool {
            known_values
                .iter()
                .any(|kv| v.to_lowercase().contains(&kv.to_lowercase()))
        };

        let is_empty_or_whitespace = value_ref
            .map(|v| v.trim_matches(|c: char| " /-.~:".contains(c)).is_empty())
            .unwrap_or(true);

        if value_ref.is_none() || is_empty_or_whitespace {
            let rec = "Server header is not set or empty. This is recommended.";
            url_result.add_notice(rec.to_string(), ANALYSIS_HEADERS, Some(vec![rec.to_string()]));
            self.result
                .get_checked_header(HEADER_SERVER)
                .set_finding(value_ref, SEVERITY_OK, Some(rec));
        } else if let Some(v) = value_ref {
            let has_version = v.chars().any(|c| c.is_ascii_digit());

            if has_version {
                let rec = format!(
                    "Server header is set to '{}'. It is better not to reveal the technologies used and especially their versions.",
                    v
                );
                url_result.add_critical(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result
                    .get_checked_header(HEADER_SERVER)
                    .set_finding(value_ref, SEVERITY_CRITICAL, Some(&rec));
            } else if check_for_known(v) {
                let rec = format!(
                    "Server header is set to known '{}'. It is better not to reveal used technologies.",
                    v
                );
                url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result
                    .get_checked_header(HEADER_SERVER)
                    .set_finding(value_ref, SEVERITY_WARNING, Some(&rec));
            } else {
                let rec = format!(
                    "Server header is set to '{}'. It is better not to reveal used technologies.",
                    v
                );
                url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result
                    .get_checked_header(HEADER_SERVER)
                    .set_finding(value_ref, SEVERITY_NOTICE, Some(&rec));
            }
        }
    }

    fn check_x_powered_by(&mut self, headers: &HashMap<String, String>, url_result: &mut UrlAnalysisResult) {
        let value = Self::get_header_value(headers, HEADER_X_POWERED_BY);
        let value_ref = value.as_deref();

        if let Some(v) = value_ref {
            let has_version = v.chars().any(|c| c.is_ascii_digit());

            if has_version {
                let rec = format!(
                    "X-Powered-By header is set to '{}'. It is better not to reveal the technologies used and especially their versions.",
                    v
                );
                url_result.add_critical(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result.get_checked_header(HEADER_X_POWERED_BY).set_finding(
                    value_ref,
                    SEVERITY_CRITICAL,
                    Some(&rec),
                );
            } else {
                let rec = format!(
                    "X-Powered-By header is set to '{}'. It is better not to reveal used technologies.",
                    v
                );
                url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
                self.result.get_checked_header(HEADER_X_POWERED_BY).set_finding(
                    value_ref,
                    SEVERITY_WARNING,
                    Some(&rec),
                );
            }
        }
    }

    fn check_set_cookie(
        &mut self,
        headers: &HashMap<String, String>,
        is_https: bool,
        url_result: &mut UrlAnalysisResult,
    ) {
        let value = match headers.get(HEADER_SET_COOKIE) {
            Some(v) => v,
            None => return,
        };

        // Multiple cookies may be separated by newlines or exist as a single value
        for cookie in value.split('\n') {
            let cookie = cookie.trim();
            if !cookie.is_empty() {
                self.check_set_cookie_value(cookie, is_https, url_result);
            }
        }
    }

    fn check_set_cookie_value(&mut self, set_cookie: &str, is_https: bool, url_result: &mut UrlAnalysisResult) {
        let mut severity = SEVERITY_OK;
        let cookie_name = set_cookie.split('=').next().unwrap_or("unknown");

        let set_cookie_lower = set_cookie.to_lowercase();

        if !set_cookie_lower.contains("samesite") {
            severity = SEVERITY_NOTICE;
            let rec = format!(
                "Set-Cookie header for '{}' does not have 'SameSite' flag. Consider using 'SameSite=Strict' or 'SameSite=Lax'.",
                cookie_name
            );
            url_result.add_notice(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
        }
        if !set_cookie_lower.contains("httponly") {
            severity = SEVERITY_WARNING;
            let rec = format!(
                "Set-Cookie header for '{}' does not have 'HttpOnly' flag. Attacker can steal the cookie using XSS. Consider using 'HttpOnly' when cookie is not used by JavaScript.",
                cookie_name
            );
            url_result.add_warning(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
        }
        if is_https && !set_cookie_lower.contains("secure") {
            severity = SEVERITY_CRITICAL;
            let rec = format!(
                "Set-Cookie header for '{}' does not have 'Secure' flag. Attacker can steal the cookie over HTTP.",
                cookie_name
            );
            url_result.add_critical(rec.clone(), ANALYSIS_HEADERS, Some(vec![rec.clone()]));
        }

        self.result
            .get_checked_header(HEADER_SET_COOKIE)
            .set_finding(Some(cookie_name), severity, None);
    }

    fn set_findings_to_summary(&mut self, status: &Status) {
        self.pages_with_critical = 0;
        self.pages_with_warning = 0;
        self.pages_with_notice = 0;

        for header in self.result.checked_headers.values() {
            self.pages_with_critical += header.count_per_severity.get(&SEVERITY_CRITICAL).copied().unwrap_or(0);
            self.pages_with_warning += header.count_per_severity.get(&SEVERITY_WARNING).copied().unwrap_or(0);
            self.pages_with_notice += header.count_per_severity.get(&SEVERITY_NOTICE).copied().unwrap_or(0);
        }

        if self.pages_with_critical > 0 {
            status.add_critical_to_summary(
                "security",
                &format!(
                    "Security - {} pages(s) with critical finding(s).",
                    self.pages_with_critical
                ),
            );
        } else if self.pages_with_warning > 0 {
            status.add_warning_to_summary(
                "security",
                &format!("Security - {} pages(s) with warning(s).", self.pages_with_warning),
            );
        } else if self.pages_with_notice > 0 {
            status.add_notice_to_summary(
                "security",
                &format!("Security - {} pages(s) with notice(s).", self.pages_with_notice),
            );
        } else {
            status.add_ok_to_summary("security", "Security - no findings.");
        }
    }
}

impl Analyzer for SecurityAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();
        let recommendation_col_width = (console_width as i32 - 70).max(20);

        let columns = vec![
            SuperTableColumn::new(
                "header".to_string(),
                "Header".to_string(),
                26,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "ok".to_string(),
                "OK".to_string(),
                5,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "green", false);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "notice".to_string(),
                "Notice".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "blue", false);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "warning".to_string(),
                "Warning".to_string(),
                7,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "magenta", true);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "critical".to_string(),
                "Critical".to_string(),
                8,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "red", true);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "recommendation".to_string(),
                "Recommendation".to_string(),
                recommendation_col_width,
                None,
                None,
                true,
                true,
                false,
                false,
                None,
            ),
        ];

        let mut data: Vec<HashMap<String, String>> = Vec::new();
        for header in self.result.checked_headers.values() {
            let mut row = HashMap::new();
            row.insert("header".to_string(), header.get_formatted_header());
            row.insert(
                "highestSeverity".to_string(),
                header.highest_severity.unwrap_or(0).to_string(),
            );
            row.insert(
                "ok".to_string(),
                header
                    .count_per_severity
                    .get(&SEVERITY_OK)
                    .copied()
                    .unwrap_or(0)
                    .to_string(),
            );
            row.insert(
                "notice".to_string(),
                header
                    .count_per_severity
                    .get(&SEVERITY_NOTICE)
                    .copied()
                    .unwrap_or(0)
                    .to_string(),
            );
            row.insert(
                "warning".to_string(),
                header
                    .count_per_severity
                    .get(&SEVERITY_WARNING)
                    .copied()
                    .unwrap_or(0)
                    .to_string(),
            );
            row.insert(
                "critical".to_string(),
                header
                    .count_per_severity
                    .get(&SEVERITY_CRITICAL)
                    .copied()
                    .unwrap_or(0)
                    .to_string(),
            );
            row.insert("recommendation".to_string(), header.recommendations.join(". "));
            data.push(row);
        }

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SECURITY.to_string(),
            "Security".to_string(),
            "Nothing to report.".to_string(),
            columns,
            true,
            Some("highestSeverity".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        self.set_findings_to_summary(status);
    }

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        if !visited_url.is_allowed_for_crawling
            || visited_url.content_type != ContentTypeId::Html
            || visited_url.looks_like_static_file_by_url()
        {
            return None;
        }

        let mut result = UrlAnalysisResult::new();

        let start = Instant::now();
        if let Some(hdrs) = headers {
            self.check_headers(hdrs, visited_url.is_https(), &mut result);
        }
        self.base.measure_exec_time("SecurityAnalyzer", "checkHeaders", start);

        if let Some(html) = body
            && !html.trim().is_empty()
        {
            let start2 = Instant::now();
            self.check_html_security(html, visited_url.is_https(), &mut result);
            self.base
                .measure_exec_time("SecurityAnalyzer", "checkHtmlSecurity", start2);
        }

        Some(result)
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        215
    }

    fn get_name(&self) -> &str {
        "SecurityAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

/// Detect well-known CSP weaknesses that largely defeat its XSS protection.
fn csp_weaknesses(csp: &str) -> Vec<&'static str> {
    let lower = csp.to_lowercase();
    let mut weaknesses = Vec::new();
    if lower.contains("unsafe-inline") {
        weaknesses.push("'unsafe-inline'");
    }
    if lower.contains("unsafe-eval") {
        weaknesses.push("'unsafe-eval'");
    }
    // A bare '*' source in a fetch directive allows loading from anywhere.
    let has_wildcard = lower.split(';').any(|directive| {
        let mut parts = directive.split_whitespace();
        let name = parts.next().unwrap_or("");
        matches!(
            name,
            "default-src" | "script-src" | "object-src" | "style-src" | "frame-src" | "connect-src"
        ) && parts.any(|p| p == "*")
    });
    if has_wildcard {
        weaknesses.push("a wildcard '*' source");
    }
    weaknesses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils;

    #[test]
    fn csp_unsafe_inline_is_a_weakness() {
        assert!(csp_weaknesses("default-src 'self'; script-src 'unsafe-inline'").contains(&"'unsafe-inline'"));
    }

    #[test]
    fn csp_wildcard_source_is_a_weakness() {
        assert!(csp_weaknesses("default-src *").iter().any(|w| w.contains("wildcard")));
    }

    #[test]
    fn strong_csp_has_no_weaknesses() {
        assert!(csp_weaknesses("default-src 'self'; object-src 'none'; base-uri 'self'").is_empty());
    }

    #[test]
    fn missing_coop_is_a_notice() {
        let mut analyzer = SecurityAnalyzer::new();
        let headers: HashMap<String, String> = HashMap::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_cross_origin_header(&headers, HEADER_CROSS_ORIGIN_OPENER_POLICY, "advice", &mut result);
        assert!(!result.get_notice().is_empty());
    }

    #[test]
    fn active_mixed_content_is_critical() {
        let mut analyzer = SecurityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_html_security(r#"<script src="http://evil.example/x.js"></script>"#, true, &mut result);
        assert!(!result.get_critical().is_empty(), "got: {:?}", result.get_critical());
    }

    #[test]
    fn passive_mixed_content_is_warning_not_critical() {
        let mut analyzer = SecurityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_html_security(r#"<img src="http://example.test/x.png">"#, true, &mut result);
        assert!(!result.get_warning().is_empty());
        assert!(result.get_critical().is_empty());
    }

    #[test]
    fn set_cookie_multi_cookie_evaluated_independently() {
        // Regression: two Set-Cookie response headers used to be merged with ", " by
        // get_flat_response_headers while check_set_cookie split on '\n', so only the first
        // cookie was ever evaluated. Each cookie must now be checked on its own.
        let mut analyzer = SecurityAnalyzer::new();

        // As collected by the HTTP client: two separate Set-Cookie headers. The first is fully
        // hardened, the second is missing both Secure and HttpOnly.
        let mut raw_headers: HashMap<String, Vec<String>> = HashMap::new();
        raw_headers.insert(
            HEADER_SET_COOKIE.to_string(),
            vec![
                "session=abc; Secure; HttpOnly; SameSite=Lax".to_string(),
                "tracking=xyz".to_string(),
            ],
        );
        let flat = utils::get_flat_response_headers(&raw_headers);

        let mut result = UrlAnalysisResult::new();
        analyzer.check_set_cookie(&flat, true, &mut result);

        let criticals = result.get_critical();
        // The insecure second cookie must trigger a critical (missing Secure on HTTPS).
        assert!(
            criticals.iter().any(|c| c.contains("tracking")),
            "expected a critical for the insecure 'tracking' cookie, got: {:?}",
            criticals
        );
        // The hardened first cookie must not be flagged as insecure.
        assert!(
            !criticals.iter().any(|c| c.contains("session")),
            "the hardened 'session' cookie must not be flagged, got: {:?}",
            criticals
        );
    }

    #[test]
    fn set_cookie_single_cookie_with_comma_in_expires() {
        // A single Set-Cookie whose value contains ", " (Expires date) must not be split apart.
        let mut analyzer = SecurityAnalyzer::new();
        let mut raw_headers: HashMap<String, Vec<String>> = HashMap::new();
        raw_headers.insert(
            HEADER_SET_COOKIE.to_string(),
            vec!["id=1; Expires=Wed, 09 Jun 2027 10:18:14 GMT; Secure; HttpOnly; SameSite=Lax".to_string()],
        );
        let flat = utils::get_flat_response_headers(&raw_headers);

        let mut result = UrlAnalysisResult::new();
        analyzer.check_set_cookie(&flat, true, &mut result);

        // Fully hardened cookie → no critical/warning findings despite the comma in Expires.
        assert!(result.get_critical().is_empty(), "got: {:?}", result.get_critical());
        assert!(result.get_warning().is_empty(), "got: {:?}", result.get_warning());
    }
}
