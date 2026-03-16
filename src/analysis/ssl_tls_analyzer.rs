// SiteOne Crawler - SslTlsAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::net::TcpStream;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use rustls::pki_types::ServerName;
use x509_parser::prelude::*;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const SUPER_TABLE_CERTIFICATE_INFO: &str = "certificate-info";

pub struct SslTlsAnalyzer {
    base: BaseAnalyzer,
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
        }
    }

    fn get_tls_certificate_info(&self, hostname: &str, port: u16, status: &Status) -> HashMap<String, String> {
        if !is_hostname_shell_safe(hostname) {
            let mut result = HashMap::new();
            let error = format!("Hostname '{}' contains unsafe characters for shell commands.", hostname);
            status.add_critical_to_summary("ssl-hostname-unsafe", &error);
            result.insert("Errors".to_string(), error);
            return result;
        }

        let mut result = HashMap::new();
        let mut errors: Vec<String> = Vec::new();

        // Build a TLS config that captures the certificate
        let mut root_store = rustls::RootCertStore::empty();

        // Add webpki roots
        for cert in rustls_native_certs::load_native_certs().certs {
            let _ = root_store.add(cert);
        }

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let server_name = match ServerName::try_from(hostname.to_string()) {
            Ok(sn) => sn,
            Err(e) => {
                let error = format!("Invalid hostname '{}': {}", hostname, e);
                status.add_critical_to_summary("ssl-certificate-connect", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        let mut conn = match rustls::ClientConnection::new(Arc::new(config), server_name) {
            Ok(c) => c,
            Err(e) => {
                let error = format!("Unable to create TLS connection to {}:{}: {}", hostname, port, e);
                status.add_critical_to_summary("ssl-certificate-connect", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        let addr = format!("{}:{}", hostname, port);
        let mut sock = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(e) => {
                let error = format!("Unable to connect to {}:{}: {}", hostname, port, e);
                status.add_critical_to_summary("ssl-certificate-connect", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        // Set a short timeout - we only need the TLS handshake, not data
        let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let _ = sock.set_write_timeout(Some(std::time::Duration::from_secs(5)));

        // Complete the TLS handshake
        loop {
            if conn.is_handshaking() {
                match conn.complete_io(&mut sock) {
                    Ok(_) => {}
                    Err(_) => break,
                }
            } else {
                break;
            }
        }

        // Extract peer certificates
        let peer_certs = match conn.peer_certificates() {
            Some(certs) if !certs.is_empty() => certs.to_vec(),
            _ => {
                let error = "No certificate found.".to_string();
                status.add_critical_to_summary("ssl-certificate-missing", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        // Parse the first (leaf) certificate
        let leaf_cert = &peer_certs[0];
        let (_, cert) = match X509Certificate::from_der(leaf_cert.as_ref()) {
            Ok(parsed) => parsed,
            Err(e) => {
                let error = format!("Unable to parse certificate: {}", e);
                status.add_critical_to_summary("ssl-certificate-parse", &error);
                errors.push(error);
                result.insert("Errors".to_string(), errors.join(", "));
                return result;
            }
        };

        // Issuer - add spaces around '='
        let issuer = add_spaces_around_equals(&cert.issuer().to_string());
        result.insert("Issuer".to_string(), issuer.clone());

        // Subject - add spaces around '='
        let subject = add_spaces_around_equals(&cert.subject().to_string());
        result.insert("Subject".to_string(), subject.clone());

        // Valid from
        let not_before = cert.validity().not_before;
        let valid_from_str = format_asn1_time(&not_before);
        let now = chrono::Utc::now();

        if let Some(nb_dt) = asn1_time_to_datetime(&not_before) {
            if now < nb_dt {
                let diff = (nb_dt - now).num_seconds().unsigned_abs() as i64;
                let error = format!(
                    "SSL/TLS certificate is not yet valid, it will be in {}.",
                    utils::get_formatted_age(diff)
                );
                status.add_critical_to_summary("ssl-certificate-valid-from", &error);
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

        // Valid to
        let not_after = cert.validity().not_after;
        let valid_to_str = format_asn1_time(&not_after);
        let valid_to_orig = valid_to_str.clone();

        if let Some(na_dt) = asn1_time_to_datetime(&not_after) {
            if now > na_dt {
                let diff = (now - na_dt).num_seconds().unsigned_abs() as i64;
                let expired_ago = format!("{} ago", utils::get_formatted_age(diff));
                let error = format!("SSL/TLS certificate expired {}.", expired_ago);
                status.add_critical_to_summary("ssl-certificate-valid-to", &error);
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

        // RAW certificate output - get via openssl command
        let certificate_output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "timeout 3s sh -c \"echo | openssl s_client -connect {}:{} -servername {} 2>/dev/null | openssl x509 -text -noout\"",
                hostname, port, hostname
            ))
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if stdout.trim().is_empty() {
                    // Fallback to stderr if stdout is empty
                    String::from_utf8_lossy(&o.stderr).to_string()
                } else {
                    stdout
                }
            })
            .unwrap_or_default();

        if !certificate_output.trim().is_empty() {
            result.insert("RAW certificate output".to_string(), certificate_output);
        }

        // Supported protocols - test each protocol via openssl s_client
        let protocols = [
            ("ssl2", "SSLv2"),
            ("ssl3", "SSLv3"),
            ("tls1", "TLSv1.0"),
            ("tls1_1", "TLSv1.1"),
            ("tls1_2", "TLSv1.2"),
            ("tls1_3", "TLSv1.3"),
        ];
        let unsafe_protocols = ["ssl2", "ssl3", "tls1", "tls1_1"];
        let mut supported_protocols: Vec<String> = Vec::new();
        let mut protocols_output = String::new();

        for (protocol_code, protocol_name) in &protocols {
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "timeout 3s sh -c \"echo 'Q' | openssl s_client -connect {}:{} -servername {} -{} 2>&1\"",
                    hostname, port, hostname, protocol_code
                ))
                .output();

            let output_str = match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr),
                Err(_) => String::new(),
            };

            protocols_output.push_str(&format!("\n=== {} ===\n{}", protocol_code, output_str));

            if output_str.contains("Certificate chain") {
                supported_protocols.push(protocol_name.to_string());
                if unsafe_protocols.contains(protocol_code) {
                    status.add_critical_to_summary(
                        "ssl-protocol-unsafe",
                        &format!("SSL/TLS protocol {} is unsafe.", protocol_name),
                    );
                }
            }
        }

        if !supported_protocols.is_empty() {
            result.insert("Supported protocols".to_string(), supported_protocols.join(", "));
        } else {
            // Fallback to rustls-detected protocol if openssl is not available
            let protocol_version = conn
                .protocol_version()
                .map(|v| {
                    let raw = format!("{:?}", v);
                    raw.replace('_', ".")
                })
                .unwrap_or_else(|| "Unknown".to_string());
            result.insert("Supported protocols".to_string(), protocol_version.clone());
        }

        // Add TLSv1.3 support warning
        let has_tls13 = supported_protocols.iter().any(|p| p.contains("1.3"));
        let has_tls12 = supported_protocols.iter().any(|p| p.contains("1.2"));
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

        if !protocols_output.is_empty() {
            result.insert("RAW protocols output".to_string(), protocols_output);
        }

        // Set summary based on errors
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

        // Extract hostname from URL
        let hostname = match url::Url::parse(&initial_url) {
            Ok(parsed) => parsed.host_str().unwrap_or("").to_string(),
            Err(_) => {
                status.add_critical_to_summary("ssl-tls-analyzer", "SSL/TLS: unable to parse initial URL");
                return;
            }
        };

        if hostname.is_empty() {
            return;
        }

        let s = Instant::now();
        let cert_info = self.get_tls_certificate_info(&hostname, 443, status);
        self.base
            .measure_exec_time("SslTlsAnalyzer", "getTLSandSSLCertificateInfo", s);

        let console_width = utils::get_console_width();

        let mut table_data: Vec<HashMap<String, String>> = Vec::new();
        let display_order = [
            "Issuer",
            "Subject",
            "Valid from",
            "Valid to",
            "Supported protocols",
            "Errors",
            "RAW certificate output",
            "RAW protocols output",
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

/// Validate that a hostname is safe to use in shell commands.
/// Only allows alphanumeric chars, dots, and hyphens to prevent command injection.
fn is_hostname_shell_safe(hostname: &str) -> bool {
    !hostname.is_empty()
        && hostname
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}
