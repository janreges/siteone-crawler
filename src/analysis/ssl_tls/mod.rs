// SiteOne Crawler - SslTlsAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

use x509_parser::prelude::*;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

mod cert_info;
mod cipher_suites;
mod tls_probe;

use cert_info::Trust;
use rustls::ProtocolVersion;

const SUPER_TABLE_CERTIFICATE_INFO: &str = "certificate-info";

pub struct SslTlsAnalyzer {
    base: BaseAnalyzer,
    accept_invalid_certs: bool,
}

impl Default for SslTlsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SslTlsAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            accept_invalid_certs: false,
        }
    }

    /// Set configuration from CoreOptions.
    pub fn set_config(&mut self, accept_invalid_certs: bool) {
        self.accept_invalid_certs = accept_invalid_certs;
    }

    /// `report_connect_failure` is false after a crawl without any working URL, which already reported
    /// that the host could not be reached.
    fn get_tls_certificate_info(
        &self,
        hostname: &str,
        port: u16,
        status: &Status,
        report_connect_failure: bool,
    ) -> HashMap<String, String> {
        let mut result: HashMap<String, String> = HashMap::new();
        let mut errors: Vec<String> = Vec::new();

        // Severity helper: with --accept-invalid-certs, demote cert problems to warnings.
        let accept_invalid = self.accept_invalid_certs;
        let add_cert_problem = |code: &str, msg: &str| {
            if accept_invalid {
                status.add_warning_to_summary(code, msg);
            } else {
                status.add_critical_to_summary(code, msg);
            }
        };

        // 1) Capture the certificate via a non-validating handshake so we can
        //    inspect expired / self-signed / mismatched certs too.
        let captured = match cert_info::capture_cert(hostname, port) {
            Ok(c) => c,
            Err(cert_info::CaptureError::Connect(_)) if !report_connect_failure => return result,
            Err(cert_info::CaptureError::Connect(e)) => {
                status.add_critical_to_summary("ssl-certificate-connect", &e);
                errors.push(e);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
            Err(cert_info::CaptureError::Handshake(detail)) => {
                let msg = "TLS handshake failed — the server may only support obsolete protocols or cipher suites (e.g. SSL 3.0, RC4, 3DES, weak Diffie-Hellman) that are no longer considered secure.";
                status.add_critical_to_summary("ssl-tls-handshake-failed", msg);
                errors.push(format!("{} ({})", msg, detail));
                let versions = [0x0303, 0x0302, 0x0301, 0x0300];
                probe_and_report_cipher_suites(hostname, port, &versions, status, &mut result);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        let leaf_der = &captured.chain[0];
        let (_, cert) = match X509Certificate::from_der(leaf_der.as_ref()) {
            Ok(parsed) => parsed,
            Err(e) => {
                let error = format!("Unable to parse certificate: {}", e);
                status.add_critical_to_summary("ssl-certificate-parse", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        // 2) Identity fields.
        let issuer = add_spaces_around_equals(&cert.issuer().to_string());
        result.insert("Issuer".to_string(), issuer.clone());
        let subject = add_spaces_around_equals(&cert.subject().to_string());
        result.insert("Subject".to_string(), subject.clone());

        let san_list = cert_info::sans(&cert);
        if !san_list.is_empty() {
            const MAX_SAN_SHOWN: usize = 10;
            let value = if san_list.len() > MAX_SAN_SHOWN {
                format!(
                    "{}, … (+{} more, {} total)",
                    san_list[..MAX_SAN_SHOWN].join(", "),
                    san_list.len() - MAX_SAN_SHOWN,
                    san_list.len()
                )
            } else {
                san_list.join(", ")
            };
            result.insert("Subject Alternative Names".to_string(), value);
        }

        // 3) Validity period (we evaluate this ourselves; the verifier also checks it).
        let now = chrono::Utc::now();
        let not_before = cert.validity().not_before;
        let valid_from_str = format_asn1_time(&not_before);
        if let Some(nb_dt) = asn1_time_to_datetime(&not_before) {
            if now < nb_dt {
                let diff = (nb_dt - now).num_seconds().unsigned_abs() as i64;
                let error = format!(
                    "SSL/TLS certificate is not yet valid, it will be in {}.",
                    utils::get_formatted_age(diff)
                );
                add_cert_problem("ssl-certificate-valid-from", &error);
                errors.push(error);
                result.insert("Valid from".to_string(), format!("{} (NOT YET VALID)", valid_from_str));
            } else {
                let diff = (now - nb_dt).num_seconds().unsigned_abs() as i64;
                result.insert(
                    "Valid from".to_string(),
                    format!("{} (VALID already {})", valid_from_str, utils::get_formatted_age(diff)),
                );
            }
        } else {
            result.insert("Valid from".to_string(), valid_from_str);
        }

        let not_after = cert.validity().not_after;
        let valid_to_str = format_asn1_time(&not_after);
        let valid_to_orig = valid_to_str.clone();
        if let Some(na_dt) = asn1_time_to_datetime(&not_after) {
            if now > na_dt {
                let diff = (now - na_dt).num_seconds().unsigned_abs() as i64;
                let expired_ago = format!("{} ago", utils::get_formatted_age(diff));
                let error = format!("SSL/TLS certificate expired {}.", expired_ago);
                add_cert_problem("ssl-certificate-valid-to", &error);
                errors.push(error);
                result.insert(
                    "Valid to".to_string(),
                    format!("{} (EXPIRED {})", valid_to_str, expired_ago),
                );
            } else {
                let diff = (na_dt - now).num_seconds().unsigned_abs() as i64;
                if expires_soon(na_dt, now) {
                    status.add_warning_to_summary(
                        "ssl-certificate-expiring-soon",
                        &format!(
                            "SSL/TLS certificate expires in {} ({}). Renew it now.",
                            utils::get_formatted_age(diff),
                            valid_to_str
                        ),
                    );
                    result.insert(
                        "Valid to".to_string(),
                        format!(
                            "{} (EXPIRES SOON, valid still for {})",
                            valid_to_str,
                            utils::get_formatted_age(diff)
                        ),
                    );
                } else {
                    result.insert(
                        "Valid to".to_string(),
                        format!("{} (VALID still for {})", valid_to_str, utils::get_formatted_age(diff)),
                    );
                }
            }
        } else {
            result.insert("Valid to".to_string(), valid_to_str);
        }

        // 4) Structured certificate details (parsed in pure Rust via x509-parser).
        let sig_name = cert_info::signature_algorithm_string(&cert);
        let key_desc = cert_info::public_key_string(&cert);
        result.insert("Serial number".to_string(), cert_info::serial_string(&cert));
        result.insert("Signature algorithm".to_string(), sig_name.clone());
        result.insert("Public key".to_string(), key_desc.clone());
        result.insert(
            "SHA-256 fingerprint".to_string(),
            cert_info::fingerprint_sha256(leaf_der.as_ref()),
        );

        // 4b) Certificate-quality findings (inspired by the BadSSL.com scenarios):
        //     flag weak crypto, accentuate strong crypto, note missing CN/Subject.
        // Chain-aware: catches a weak intermediate (e.g. SHA-256 leaf, SHA-1 intermediate).
        match cert_info::chain_weak_signature(&captured.chain) {
            Some(weak) => add_cert_problem(
                "ssl-weak-signature",
                &format!(
                    "SSL/TLS certificate chain uses a weak signature algorithm ({}). SHA-1/MD5 are deprecated and distrusted.",
                    weak
                ),
            ),
            None => {
                if matches!(cert_info::signature_grade(&sig_name), cert_info::Grade::Strong) {
                    status.add_ok_to_summary(
                        "ssl-signature-strong",
                        &format!("SSL/TLS certificate uses a strong signature algorithm ({}).", sig_name),
                    );
                }
            }
        }
        match cert_info::public_key_grade(&cert) {
            cert_info::Grade::Weak => add_cert_problem(
                "ssl-weak-key",
                &format!(
                    "SSL/TLS certificate uses a weak public key ({}). Use RSA ≥ 2048-bit or ECDSA ≥ 256-bit.",
                    key_desc
                ),
            ),
            cert_info::Grade::Strong => status.add_ok_to_summary(
                "ssl-key-strong",
                &format!("SSL/TLS certificate uses a strong public key ({}).", key_desc),
            ),
            cert_info::Grade::Unknown => {}
        }
        if !cert_info::has_common_name(&cert) {
            status.add_notice_to_summary(
                "ssl-no-common-name",
                "SSL/TLS certificate has no Common Name (CN); modern clients rely on Subject Alternative Names.",
            );
        }
        if cert_info::subject_is_empty(&cert) {
            status.add_notice_to_summary(
                "ssl-no-subject",
                "SSL/TLS certificate has an empty Subject; identity is provided only via Subject Alternative Names.",
            );
        }

        // 5) Trust verdict against the system CA store (chain + hostname + validity).
        match cert_info::verify_trust(&captured.chain, hostname) {
            Trust::Trusted => {
                result.insert("Trust".to_string(), "Trusted by system CA store".to_string());
                status.add_ok_to_summary(
                    "ssl-certificate-trusted",
                    "SSL/TLS certificate chain is trusted by the system CA store.",
                );
            }
            Trust::Untrusted(reason) => {
                result.insert("Trust".to_string(), format!("Untrusted: {}", reason));
                add_cert_problem(
                    "ssl-certificate-untrusted",
                    &format!(
                        "SSL/TLS certificate chain is not trusted by the system CA store: {}.",
                        reason
                    ),
                );
            }
        }

        // 6) Protocol version detection: modern via rustls, legacy via raw probe.
        let mut supported_protocols: Vec<String> = Vec::new();

        // Legacy probes (order: SSLv3, TLS1.0, TLS1.1).
        let legacy = [(0x0300u16, "SSLv3"), (0x0301, "TLSv1.0"), (0x0302, "TLSv1.1")];
        for (code, name) in legacy {
            if tls_probe::probe_legacy_version(hostname, port, code) == Some(true) {
                supported_protocols.push(name.to_string());
                status.add_critical_to_summary("ssl-protocol-unsafe", &format!("SSL/TLS protocol {} is unsafe.", name));
            }
        }

        // Modern probes.
        if tls_probe::detect_modern_version(hostname, port, &rustls::version::TLS12, ProtocolVersion::TLSv1_2) {
            supported_protocols.push("TLSv1.2".to_string());
        }
        if tls_probe::detect_modern_version(hostname, port, &rustls::version::TLS13, ProtocolVersion::TLSv1_3) {
            supported_protocols.push("TLSv1.3".to_string());
        }

        // Defensive fallback: if every probe failed but we DID complete a
        // handshake during capture, at least report the negotiated version.
        if supported_protocols.is_empty()
            && let Some(v) = captured.negotiated
        {
            supported_protocols.push(cert_info::protocol_name(v).to_string());
        }

        result.insert("Supported protocols".to_string(), supported_protocols.join(", "));

        // 7) TLS 1.2/1.3 hint — now over real probe data, so the previous
        //    false positive is gone. Only emit when we actually detected protocols.
        if !supported_protocols.is_empty() {
            let has_tls13 = supported_protocols.iter().any(|p| p == "TLSv1.3");
            let has_tls12 = supported_protocols.iter().any(|p| p == "TLSv1.2");
            if !has_tls13 {
                if !has_tls12 {
                    status.add_critical_to_summary(
                        "ssl-protocol-hint",
                        "SSL/TLS protocol TLSv1.2 is not supported. Ask your admin/provider to add TLSv1.2 support.",
                    );
                } else {
                    status.add_warning_to_summary(
                        "ssl-protocol-hint",
                        "Latest SSL/TLS protocol TLSv1.3 is not supported. Ask your admin/provider to add TLSv1.3 support.",
                    );
                }
            }
        }

        // 7b) Positive protocol findings — accentuate a good configuration.
        if !supported_protocols.is_empty() {
            let has_legacy = supported_protocols
                .iter()
                .any(|p| p == "SSLv3" || p == "TLSv1.0" || p == "TLSv1.1");
            let has_modern = supported_protocols.iter().any(|p| p == "TLSv1.2" || p == "TLSv1.3");
            if has_modern && !has_legacy {
                status.add_ok_to_summary(
                    "ssl-protocols-modern",
                    "Only modern TLS protocols are supported (no SSLv3 / TLS 1.0 / TLS 1.1).",
                );
            }
            if supported_protocols.iter().any(|p| p == "TLSv1.3") {
                status.add_ok_to_summary("ssl-protocol-tls13", "Modern TLS 1.3 is supported.");
            }
        }

        // 7c) Cipher suites (#20): insecure and non-forward-secret suites per protocol up to TLS 1.2.
        //     TLS 1.2 is always probed (a server offering only weak TLS 1.2 suites fails the rustls
        //     detection above); older versions only when the legacy probes found them.
        let mut probe_versions: Vec<u16> = vec![0x0303];
        for (code, name) in [(0x0302u16, "TLSv1.1"), (0x0301, "TLSv1.0"), (0x0300, "SSLv3")] {
            if supported_protocols.iter().any(|p| p == name) {
                probe_versions.push(code);
            }
        }
        probe_and_report_cipher_suites(hostname, port, &probe_versions, status, &mut result);

        // 8) Overall summary.
        if errors.is_empty() && !issuer.is_empty() {
            status.add_ok_to_summary(
                "ssl-certificate-valid",
                &format!(
                    "SSL/TLS certificate is valid until {}. Issued by {}. Subject is {}.",
                    valid_to_orig, issuer, subject
                ),
            );
            status.add_ok_to_summary(
                "certificate-info",
                &format!("SSL/TLS certificate issued by '{}'.", issuer),
            );
        } else if !errors.is_empty() {
            result.insert("Errors".to_string(), errors.join(", "));
        }

        if issuer.is_empty() && errors.is_empty() {
            status.add_critical_to_summary("certificate-info", "SSL/TLS: unable to load certificate info");
        }

        result
    }
}

impl Analyzer for SslTlsAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        // Find the initial URL from visited URLs (the one with SOURCE_INIT_URL source_attr)
        let visited_urls = status.get_visited_urls();
        let initial = visited_urls
            .iter()
            .find(|u| u.source_attr == crate::result::visited_url::SOURCE_INIT_URL)
            .or_else(|| visited_urls.first())
            .map(|u| (u.url.clone(), u.status_code));

        let (initial_url, initial_status_code) = match initial {
            Some(initial) => initial,
            None => return,
        };

        // Without any working URL (#20) only an HTTPS initial URL that failed to connect is examined:
        // when the TLS handshake is what failed, the table says so and lists the insecure suites the
        // server does accept. Other failures were already reported by the crawl.
        let without_crawled_pages = status.get_number_of_working_visited_urls() == 0;
        if without_crawled_pages && (!initial_url.starts_with("https://") || initial_status_code != -1) {
            return;
        }

        if !initial_url.starts_with("https://") {
            status.add_notice_to_summary("ssl-tls-analyzer", "SSL/TLS not supported, analyzer skipped.");
            return;
        }

        // Extract hostname and port from URL (honor non-default HTTPS ports, e.g. :8443)
        let (hostname, port) = match url::Url::parse(&initial_url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_string();
                let port = parsed.port_or_known_default().unwrap_or(443);
                (host, port)
            }
            Err(_) => {
                status.add_critical_to_summary("ssl-tls-analyzer", "SSL/TLS: unable to parse initial URL");
                return;
            }
        };

        if hostname.is_empty() {
            return;
        }

        let s = Instant::now();
        let cert_info = self.get_tls_certificate_info(&hostname, port, status, !without_crawled_pages);
        if cert_info.is_empty() {
            return;
        }
        self.base
            .measure_exec_time("SslTlsAnalyzer", "getTLSandSSLCertificateInfo", s);

        let console_width = utils::get_console_width();
        let value_width = (console_width as i32 - 30).max(20);

        let mut table_data: Vec<HashMap<String, String>> = Vec::new();
        let display_order = [
            "Issuer",
            "Subject",
            "Subject Alternative Names",
            "Valid from",
            "Valid to",
            "Serial number",
            "Signature algorithm",
            "Public key",
            "SHA-256 fingerprint",
            "Supported protocols",
            "Insecure cipher suites",
            "Without forward secrecy",
            "Trust",
            "Errors",
        ];

        for key in &display_order {
            if let Some(value) = cert_info.get(*key)
                && !value.is_empty()
            {
                let mut row = HashMap::new();
                row.insert("info".to_string(), key.to_string());
                row.insert("value".to_string(), value.clone());
                table_data.push(row);
            }
        }

        let columns = vec![
            SuperTableColumn::new(
                "info".to_string(),
                "Info".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "value".to_string(),
                "Text".to_string(),
                value_width,
                Some(Box::new(move |value: &str, render_into: &str| {
                    if render_into == "html" {
                        suite_list_for_display(value, "\n", "\n", usize::MAX)
                            .replace(' ', "&nbsp;")
                            .replace('\n', "<br>")
                    } else {
                        suite_list_for_display(value, " ", ", ", value_width as usize)
                    }
                })),
                None,
                true,
                true,
                false,
                false,
                None,
            ),
        ];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_CERTIFICATE_INFO.to_string(),
            "SSL/TLS info".to_string(),
            "No SSL/TLS info.".to_string(),
            columns,
            true,
            None,
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(table_data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn runs_without_working_urls(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        20
    }

    fn get_name(&self) -> &str {
        "SslTlsAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

fn format_asn1_time(time: &ASN1Time) -> String {
    // ASN1Time implements Display, but we replace "+00:00" with "GMT"
    format!("{}", time).replace("+00:00", "GMT")
}

fn add_spaces_around_equals(s: &str) -> String {
    use once_cell::sync::Lazy;
    static RE_EQUALS: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"(\w)=(\S)").unwrap());
    RE_EQUALS.replace_all(s, "$1 = $2").to_string()
}

fn asn1_time_to_datetime(time: &ASN1Time) -> Option<chrono::DateTime<chrono::Utc>> {
    // ASN1Time has a timestamp() method that gives epoch seconds
    let epoch = time.timestamp();
    chrono::DateTime::from_timestamp(epoch, 0)
}

/// A certificate that expires in less than this many days (but has not expired yet) gets a warning.
const EXPIRY_WARNING_DAYS: i64 = 14;

/// Whether a certificate valid until `not_after` is still valid at `now` but expires within
/// EXPIRY_WARNING_DAYS days.
fn expires_soon(not_after: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> bool {
    (0..EXPIRY_WARNING_DAYS * 86_400).contains(&(not_after - now).num_seconds())
}

/// Probe the weak cipher suites of `versions` (highest first) within the per-host budget and report them.
fn probe_and_report_cipher_suites(
    hostname: &str,
    port: u16,
    versions: &[u16],
    status: &Status,
    result: &mut HashMap<String, String>,
) {
    let mut budget =
        tls_probe::ProbeBudget::new(tls_probe::MAX_CIPHER_PROBE_HANDSHAKES, tls_probe::MAX_CIPHER_PROBE_TIME);
    let probe = tls_probe::probe_weak_cipher_suites(hostname, port, versions, &mut budget);
    report_cipher_suites(&probe, status, result);
}

/// Suites shown per row in the text and HTML table; the rest is summarized as "… (+k more)". The table
/// data (JSON) and the summary keep the full list.
const MAX_SUITES_SHOWN: usize = 5;

/// Display form of a suite-list value (an optional note line ending with ':', then one suite per
/// line): the note, then at most MAX_SUITES_SHOWN suites that fit into `max_chars` — a suite is never
/// cut in the middle — and "… (+k more)" for the rest. Single-line values are returned unchanged.
fn suite_list_for_display(value: &str, note_separator: &str, separator: &str, max_chars: usize) -> String {
    if !value.contains('\n') {
        return value.to_string();
    }
    let mut suites: Vec<&str> = value.lines().collect();
    let mut shown = String::new();
    if suites.first().is_some_and(|line| line.ends_with(':')) {
        shown.push_str(suites.remove(0));
        shown.push_str(note_separator);
    }
    let more = |hidden: usize, first: bool| format!("{}… (+{} more)", if first { "" } else { separator }, hidden);
    let mut count = 0;
    for (index, suite) in suites.iter().enumerate() {
        let separator_len = if count == 0 { 0 } else { separator.chars().count() };
        let hidden_after = suites.len() - index - 1;
        let reserve = if hidden_after > 0 {
            more(hidden_after, false).chars().count()
        } else {
            0
        };
        if count == MAX_SUITES_SHOWN
            || shown.chars().count() + separator_len + suite.chars().count() + reserve > max_chars
        {
            break;
        }
        if count > 0 {
            shown.push_str(separator);
        }
        shown.push_str(suite);
        count += 1;
    }
    if count < suites.len() {
        shown.push_str(&more(suites.len() - count, count == 0));
    }
    shown
}

/// Put a cipher-suite probe into the SSL/TLS table and the summary (#20): insecure suites are
/// critical; suites without forward secrecy are a notice, which does not affect the score.
fn report_cipher_suites(probe: &tls_probe::CipherProbeResult, status: &Status, result: &mut HashMap<String, String>) {
    // An incomplete list starts with a note, so a shortened display never hides it.
    let (none, note) = match probe.incomplete {
        None => ("None", None),
        Some(tls_probe::Incomplete::LimitReached) => (
            "None found (probe limit reached, the list may be incomplete)",
            Some("Probe limit reached, the list may be incomplete:"),
        ),
        Some(tls_probe::Incomplete::Unreachable) => (
            "None found (probe could not connect, the list may be incomplete)",
            Some("Probe could not connect, the list may be incomplete:"),
        ),
        Some(tls_probe::Incomplete::Unanswered) => (
            "None found (probe got no clear answer, the list may be incomplete)",
            Some("Probe got no clear answer, the list may be incomplete:"),
        ),
    };
    // One suite per line, so the HTML report can break the list (the text table joins the lines).
    let table_value = |suites: &[String]| {
        note.into_iter()
            .map(str::to_string)
            .chain(suites.iter().cloned())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let insecure = cipher_suites::describe_accepted(&probe.accepted, cipher_suites::is_insecure);
    if insecure.is_empty() {
        result.insert("Insecure cipher suites".to_string(), none.to_string());
        if probe.incomplete.is_none() {
            status.add_ok_to_summary(
                "ssl-weak-cipher-suites",
                "No insecure cipher suites (NULL, EXPORT, anonymous, RC4, DES, 3DES) are accepted.",
            );
        }
    } else {
        let list = insecure.join(", ");
        result.insert("Insecure cipher suites".to_string(), table_value(&insecure));
        status.add_critical_to_summary(
            "ssl-weak-cipher-suites",
            &format!("Server accepts insecure cipher suites: {}.", list),
        );
    }

    let without_forward_secrecy =
        cipher_suites::describe_accepted(&probe.accepted, cipher_suites::lacks_forward_secrecy);
    if without_forward_secrecy.is_empty() {
        result.insert("Without forward secrecy".to_string(), none.to_string());
    } else {
        let list = without_forward_secrecy.join(", ");
        result.insert(
            "Without forward secrecy".to_string(),
            table_value(&without_forward_secrecy),
        );
        status.add_notice_to_summary(
            "ssl-no-forward-secrecy",
            &format!(
                "Server accepts cipher suites without forward secrecy (static RSA key exchange), such as {}.",
                list
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::summary::item_status::ItemStatus;

    fn at(offset_seconds: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(1_790_000_000 + offset_seconds, 0).expect("valid timestamp")
    }

    fn empty_status() -> Status {
        let info = crate::info::Info::new(
            "SiteOne Crawler".to_string(),
            "test".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            "https://example.com/".to_string(),
        );
        Status::new(
            Box::new(crate::result::storage::memory_storage::MemoryStorage::new(false)),
            false,
            info,
            std::time::Instant::now(),
        )
    }

    fn summary_status(status: &Status, code: &str) -> Option<ItemStatus> {
        status
            .get_summary()
            .get_items()
            .iter()
            .find(|item| item.apl_code == code)
            .map(|item| item.status)
    }

    #[test]
    fn certificate_expiring_within_14_days_is_flagged() {
        let now = at(0);
        let day = 86_400;
        assert!(!expires_soon(at(31 * day + 21 * 3600), now), "31.9 days left is fine");
        assert!(!expires_soon(at(14 * day), now), "exactly 14 days left is fine");
        assert!(expires_soon(at(14 * day - 1), now));
        assert!(expires_soon(at(1), now));
        assert!(expires_soon(at(0), now), "expires this very second");
        assert!(
            !expires_soon(at(-1), now),
            "already expired is reported as expired, not as expiring"
        );
    }

    #[test]
    fn insecure_suites_are_critical_and_static_rsa_is_a_notice() {
        let status = empty_status();
        let mut table = HashMap::new();
        let probe = tls_probe::CipherProbeResult {
            accepted: vec![(0x0303, 0x000A), (0x0301, 0x000A), (0x0303, 0x009C)],
            incomplete: None,
        };
        report_cipher_suites(&probe, &status, &mut table);
        assert_eq!(
            table["Insecure cipher suites"],
            "TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2)"
        );
        assert_eq!(
            table["Without forward secrecy"],
            "TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2)\nTLS_RSA_WITH_AES_128_GCM_SHA256 (TLSv1.2)",
            "one suite per line, so the HTML report can break the list"
        );
        assert_eq!(
            summary_status(&status, "ssl-weak-cipher-suites"),
            Some(ItemStatus::Critical)
        );
        assert_eq!(
            summary_status(&status, "ssl-no-forward-secrecy"),
            Some(ItemStatus::Notice)
        );
        let text = status
            .get_summary()
            .get_items()
            .iter()
            .find(|item| item.apl_code == "ssl-weak-cipher-suites")
            .map(|item| item.text.clone())
            .unwrap();
        assert!(
            text.starts_with("Server accepts insecure cipher suites: TLS_RSA_WITH_3DES_EDE_CBC_SHA"),
            "{text}"
        );
    }

    /// Eight accepted insecure suites, each listed with TLS 1.0 and TLS 1.2.
    fn many_insecure_suites() -> Vec<(u16, u16)> {
        [0x000Au16, 0x0005, 0x0004, 0xC012, 0xC011, 0x0016, 0x0009, 0x0015]
            .into_iter()
            .flat_map(|id| [(0x0303, id), (0x0301, id)])
            .collect()
    }

    #[test]
    fn an_incomplete_list_starts_with_the_note_and_keeps_every_suite() {
        let status = empty_status();
        let mut table = HashMap::new();
        let probe = tls_probe::CipherProbeResult {
            accepted: many_insecure_suites(),
            incomplete: Some(tls_probe::Incomplete::LimitReached),
        };
        report_cipher_suites(&probe, &status, &mut table);
        let lines: Vec<&str> = table["Insecure cipher suites"].lines().collect();
        assert_eq!(lines[0], "Probe limit reached, the list may be incomplete:");
        assert_eq!(
            lines.len(),
            9,
            "the table data (JSON) keeps all eight suites: {lines:?}"
        );
        assert_eq!(lines[1], "TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2)");
    }

    #[test]
    fn long_suite_lists_are_shortened_between_whole_suites() {
        let status = empty_status();
        let mut table = HashMap::new();
        let probe = tls_probe::CipherProbeResult {
            accepted: many_insecure_suites(),
            incomplete: Some(tls_probe::Incomplete::LimitReached),
        };
        report_cipher_suites(&probe, &status, &mut table);
        let value = &table["Insecure cipher suites"];

        // Text: the note first, then whole suites that fit the column, then how many are left out.
        let text = suite_list_for_display(value, " ", ", ", 160);
        assert!(text.chars().count() <= 160, "{text}");
        assert_eq!(
            text,
            "Probe limit reached, the list may be incomplete: TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2), \
             TLS_RSA_WITH_RC4_128_SHA (TLSv1.0, TLSv1.2), … (+6 more)"
        );

        // HTML: one suite per line, at most MAX_SUITES_SHOWN of them.
        let html = suite_list_for_display(value, "\n", "\n", usize::MAX);
        let lines: Vec<&str> = html.lines().collect();
        assert_eq!(lines.len(), 1 + MAX_SUITES_SHOWN + 1, "{html}");
        assert_eq!(lines[0], "Probe limit reached, the list may be incomplete:");
        assert_eq!(lines[MAX_SUITES_SHOWN + 1], "… (+3 more)");

        // Values of the other rows are left alone.
        assert_eq!(
            suite_list_for_display("TLSv1.2, TLSv1.3", " ", ", ", 10),
            "TLSv1.2, TLSv1.3"
        );
    }

    #[test]
    fn clean_probe_is_ok_and_incomplete_probe_says_so() {
        let status = empty_status();
        let mut table = HashMap::new();
        let clean = tls_probe::CipherProbeResult {
            accepted: Vec::new(),
            incomplete: None,
        };
        report_cipher_suites(&clean, &status, &mut table);
        assert_eq!(table["Insecure cipher suites"], "None");
        assert_eq!(table["Without forward secrecy"], "None");
        assert_eq!(summary_status(&status, "ssl-weak-cipher-suites"), Some(ItemStatus::Ok));
        assert_eq!(summary_status(&status, "ssl-no-forward-secrecy"), None);

        let status = empty_status();
        let mut table = HashMap::new();
        let cut_short = tls_probe::CipherProbeResult {
            accepted: Vec::new(),
            incomplete: Some(tls_probe::Incomplete::LimitReached),
        };
        report_cipher_suites(&cut_short, &status, &mut table);
        assert_eq!(
            table["Insecure cipher suites"],
            "None found (probe limit reached, the list may be incomplete)"
        );
        assert_eq!(
            summary_status(&status, "ssl-weak-cipher-suites"),
            None,
            "no OK without a complete probe"
        );

        let status = empty_status();
        let mut table = HashMap::new();
        let unanswered = tls_probe::CipherProbeResult {
            accepted: Vec::new(),
            incomplete: Some(tls_probe::Incomplete::Unanswered),
        };
        report_cipher_suites(&unanswered, &status, &mut table);
        assert_eq!(
            table["Insecure cipher suites"],
            "None found (probe got no clear answer, the list may be incomplete)"
        );
        assert_eq!(summary_status(&status, "ssl-weak-cipher-suites"), None);
    }

    /// Run the SSL/TLS analyzer the way the analysis manager does after a crawl in which the initial
    /// URL `url` failed to connect (status -1), i.e. without any working URL.
    fn analyze_failed_crawl(url: &str) -> Status {
        // main() installs the process-wide rustls provider; tests have to do it themselves.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut status = empty_status();
        status.add_visited_url(
            crate::result::visited_url::VisitedUrl::new(
                "init".to_string(),
                String::new(),
                crate::result::visited_url::SOURCE_INIT_URL,
                url.to_string(),
                -1,
                0.1,
                None,
                crate::types::ContentTypeId::Html,
                None,
                None,
                None,
                false,
                true,
                0,
                None,
            ),
            None,
            None,
        );
        let mut manager = crate::analysis::manager::AnalysisManager::new();
        manager.register_analyzer(Box::new(SslTlsAnalyzer::new()));
        let mut output = crate::output::json_output::JsonOutput::new(
            crate::output::output::CrawlerInfo::default(),
            vec![],
            true,
            false,
            None,
            0,
        );
        manager.run_analyzers(&status, &mut output);
        status
    }

    /// A server whose TLS handshake fails for a modern client (it answers rustls with a
    /// handshake_failure alert) but that accepts TLS_RSA_WITH_3DES_EDE_CBC_SHA whenever it is offered,
    /// with `server_version` or, when None, the version the client asked for.
    fn legacy_only_tls_server(server_version: Option<[u8; 2]>) -> u16 {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("local address").port();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let mut header = [0u8; 5];
                if stream.read_exact(&mut header).is_err() {
                    continue;
                }
                let mut hello = vec![0u8; u16::from_be_bytes([header[3], header[4]]) as usize];
                if stream.read_exact(&mut hello).is_err() || hello.len() < 39 {
                    continue;
                }
                // handshake header(4) + client_version(2) + random(32), then the session id and the suites
                let version = server_version.unwrap_or([hello[4], hello[5]]);
                let suites_at = 39 + hello[38] as usize;
                let offers_3des = hello
                    .get(suites_at + 2..)
                    .and_then(|rest| {
                        let len = u16::from_be_bytes([hello[suites_at], hello[suites_at + 1]]) as usize;
                        rest.get(..len)
                    })
                    .is_some_and(|suites| suites.chunks(2).any(|suite| suite == [0x00, 0x0A]));
                let answer = if offers_3des {
                    let mut server_hello = vec![0x16, version[0], version[1], 0x00, 0x2a, 0x02, 0x00, 0x00, 0x26];
                    server_hello.extend_from_slice(&version);
                    server_hello.extend_from_slice(&[0u8; 32]);
                    server_hello.extend_from_slice(&[0x00, 0x00, 0x0A, 0x00]);
                    server_hello
                } else {
                    vec![0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x28]
                };
                let _ = stream.write_all(&answer);
            }
        });
        port
    }

    #[test]
    fn a_legacy_only_https_server_is_analyzed_although_no_url_could_be_crawled() {
        let port = legacy_only_tls_server(None);
        let status = analyze_failed_crawl(&format!("https://127.0.0.1:{}/", port));
        assert_eq!(
            summary_status(&status, "ssl-tls-handshake-failed"),
            Some(ItemStatus::Critical)
        );
        assert_eq!(
            summary_status(&status, "ssl-weak-cipher-suites"),
            Some(ItemStatus::Critical),
            "the insecure suites the server accepts are listed"
        );
    }

    #[test]
    fn an_sslv3_only_https_server_is_probed_with_sslv3_although_no_url_could_be_crawled() {
        // It answers every ClientHello offering 3DES with an SSLv3 ServerHello.
        let port = legacy_only_tls_server(Some([0x03, 0x00]));
        let status = analyze_failed_crawl(&format!("https://127.0.0.1:{}/", port));
        let insecure = status
            .get_summary()
            .get_items()
            .iter()
            .find(|item| item.apl_code == "ssl-weak-cipher-suites")
            .map(|item| (item.status, item.text.clone()));
        assert_eq!(
            insecure,
            Some((
                ItemStatus::Critical,
                "Server accepts insecure cipher suites: TLS_RSA_WITH_3DES_EDE_CBC_SHA (SSLv3).".to_string()
            ))
        );
    }

    #[test]
    fn an_unreachable_host_without_crawled_pages_adds_no_ssl_findings() {
        let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        for url in [
            format!("https://127.0.0.1:{}/", closed_port),
            "http://127.0.0.1:1/".to_string(),
        ] {
            let status = analyze_failed_crawl(&url);
            let codes: Vec<String> = status
                .get_summary()
                .get_items()
                .iter()
                .map(|item| item.apl_code.clone())
                .collect();
            assert_eq!(codes, vec!["analysis-manager-error".to_string()], "{url}");
        }
    }

    #[test]
    fn an_unreachable_host_is_not_reported_as_a_probe_limit() {
        let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let mut budget =
            tls_probe::ProbeBudget::new(tls_probe::MAX_CIPHER_PROBE_HANDSHAKES, tls_probe::MAX_CIPHER_PROBE_TIME);
        let probe = tls_probe::probe_weak_cipher_suites("127.0.0.1", closed_port, &[0x0303], &mut budget);
        let status = empty_status();
        let mut table = HashMap::new();
        report_cipher_suites(&probe, &status, &mut table);
        assert_eq!(
            table["Insecure cipher suites"],
            "None found (probe could not connect, the list may be incomplete)"
        );
    }
}
