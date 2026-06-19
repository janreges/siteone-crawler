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

    fn get_tls_certificate_info(&self, hostname: &str, port: u16, status: &Status) -> HashMap<String, String> {
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
                result.insert(
                    "Valid to".to_string(),
                    format!("{} (VALID still for {})", valid_to_str, utils::get_formatted_age(diff)),
                );
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
        let initial_url = visited_urls
            .iter()
            .find(|u| u.source_attr == crate::result::visited_url::SOURCE_INIT_URL)
            .map(|u| u.url.clone())
            .or_else(|| visited_urls.first().map(|u| u.url.clone()));

        let initial_url = match initial_url {
            Some(url) => url,
            None => return,
        };

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
        let cert_info = self.get_tls_certificate_info(&hostname, port, status);
        self.base
            .measure_exec_time("SslTlsAnalyzer", "getTLSandSSLCertificateInfo", s);

        let console_width = utils::get_console_width();

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
                (console_width as i32 - 30).max(20),
                Some(Box::new(|value: &str, render_into: &str| {
                    if render_into == "html" {
                        value.replace(' ', "&nbsp;").replace('\n', "<br>")
                    } else {
                        value.to_string()
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
