// SiteOne Crawler - SSL/TLS certificate inspection (pure Rust)
// (c) Jan Reges <jan.reges@siteone.cz>

use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error as RustlsError, ProtocolVersion, SignatureScheme};
use x509_parser::objects::{oid_registry, oid2sn};
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// A certificate verifier that accepts every certificate without validation.
/// Used ONLY by the analyzer so it can inspect expired/self-signed/mismatched
/// certificates and probe protocol versions. It never affects crawling.
#[derive(Debug)]
pub(crate) struct InsecureVerifier;

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Certificate chain captured during a (non-validating) TLS handshake.
pub(crate) struct CapturedCert {
    pub chain: Vec<CertificateDer<'static>>,
    pub negotiated: Option<ProtocolVersion>,
}

/// Why a certificate capture failed.
pub(crate) enum CaptureError {
    /// TCP connect or hostname problem — the host could not be reached.
    Connect(String),
    /// TCP connected but the TLS handshake produced no usable certificate —
    /// typically a server that only supports obsolete protocols or ciphers
    /// (SSL 3.0, RC4, 3DES, weak Diffie-Hellman, …) that rustls refuses to speak.
    Handshake(String),
}

/// Connect to `hostname:port`, perform a TLS handshake with the non-validating
/// verifier and return the peer certificate chain. Returns a typed CaptureError
/// distinguishing an unreachable host from a failed (obsolete) TLS handshake.
pub(crate) fn capture_cert(hostname: &str, port: u16) -> Result<CapturedCert, CaptureError> {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
        .with_no_client_auth();

    let server_name = ServerName::try_from(hostname.to_string())
        .map_err(|e| CaptureError::Connect(format!("Invalid hostname '{}': {}", hostname, e)))?;

    let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).map_err(|e| {
        CaptureError::Connect(format!(
            "Unable to create TLS connection to {}:{}: {}",
            hostname, port, e
        ))
    })?;

    let mut sock = TcpStream::connect(format!("{}:{}", hostname, port))
        .map_err(|e| CaptureError::Connect(format!("Unable to connect to {}:{}: {}", hostname, port, e)))?;
    let _ = sock.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let _ = sock.set_write_timeout(Some(HANDSHAKE_TIMEOUT));

    while conn.is_handshaking() {
        if conn.complete_io(&mut sock).is_err() {
            break;
        }
    }

    let chain = match conn.peer_certificates() {
        Some(c) if !c.is_empty() => c.to_vec(),
        _ => {
            return Err(CaptureError::Handshake(
                "TLS handshake produced no certificate".to_string(),
            ));
        }
    };

    Ok(CapturedCert {
        chain,
        negotiated: conn.protocol_version(),
    })
}

/// Result of an in-process trust verification against the system CA store.
pub(crate) enum Trust {
    Trusted,
    Untrusted(String),
}

/// Verify the captured chain against the native CA store using rustls' webpki
/// verifier (checks chain trust, hostname match and validity period at once).
pub(crate) fn verify_trust(chain: &[CertificateDer<'static>], hostname: &str) -> Trust {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }

    let verifier = match rustls::client::WebPkiServerVerifier::builder(Arc::new(roots)).build() {
        Ok(v) => v,
        Err(e) => return Trust::Untrusted(format!("verifier init failed: {:?}", e)),
    };

    let server_name = match ServerName::try_from(hostname.to_string()) {
        Ok(s) => s,
        Err(e) => return Trust::Untrusted(format!("invalid hostname: {}", e)),
    };

    let (end_entity, intermediates) = match chain.split_first() {
        Some((ee, rest)) => (ee, rest),
        None => return Trust::Untrusted("empty certificate chain".to_string()),
    };

    match verifier.verify_server_cert(end_entity, intermediates, &server_name, &[], UnixTime::now()) {
        Ok(_) => Trust::Trusted,
        Err(e) => Trust::Untrusted(trust_reason(&e)),
    }
}

/// Map a rustls verification error to a short, human-readable reason.
/// Avoids rustls' verbose Display, which leaks raw epoch timestamps and would be
/// truncated mid-word in the table.
fn trust_reason(err: &RustlsError) -> String {
    let reason: &str = match err {
        RustlsError::InvalidCertificate(ce) => match ce {
            CertificateError::Expired | CertificateError::ExpiredContext { .. } => "certificate expired",
            CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => "certificate not yet valid",
            CertificateError::UnknownIssuer => "issuer not trusted (self-signed or unknown CA)",
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
                "certificate not valid for this hostname"
            }
            CertificateError::Revoked => "certificate revoked",
            CertificateError::BadSignature => "invalid certificate signature",
            CertificateError::BadEncoding => "malformed certificate",
            _ => "certificate chain not trusted",
        },
        _ => "certificate chain not trusted",
    };
    reason.to_string()
}

/// Colon-separated uppercase SHA-256 fingerprint of the DER bytes.
pub(crate) fn fingerprint_sha256(der: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, der);
    digest
        .as_ref()
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":")
}

/// Human-readable signature algorithm (e.g. "sha256WithRSAEncryption").
pub(crate) fn signature_algorithm_string(cert: &X509Certificate) -> String {
    let oid = &cert.signature_algorithm.algorithm;
    oid2sn(oid, oid_registry())
        .map(|s| s.to_string())
        .unwrap_or_else(|_| oid.to_string())
}

/// Public key type + size (e.g. "RSA 2048 bits", "EC 256 bits").
pub(crate) fn public_key_string(cert: &X509Certificate) -> String {
    match cert.public_key().parsed() {
        Ok(PublicKey::RSA(rsa)) => format!("RSA {} bits", rsa.key_size()),
        Ok(PublicKey::EC(ec)) => format!("EC {} bits", ec.key_size()),
        Ok(PublicKey::DSA(y)) => format!("DSA {} bits", y.len() * 8),
        // Ed25519/Ed448 and others: fall back to the algorithm name from its OID.
        _ => {
            let oid = &cert.public_key().algorithm.algorithm;
            oid2sn(oid, oid_registry())
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        }
    }
}

/// Quality grade for a certificate property (signature algorithm, public key).
pub(crate) enum Grade {
    Strong,
    Weak,
    Unknown,
}

/// Grade a signature algorithm by name: SHA-1/MD5/MD2 are weak, the SHA-2
/// family and EdDSA are strong, anything else is unknown.
pub(crate) fn signature_grade(name: &str) -> Grade {
    let n = name.to_ascii_lowercase();
    if n.contains("md2") || n.contains("md5") || n.contains("sha1") || n.contains("sha-1") {
        Grade::Weak
    } else if n.contains("sha256")
        || n.contains("sha384")
        || n.contains("sha512")
        || n.contains("sha-256")
        || n.contains("sha-384")
        || n.contains("sha-512")
        || n.contains("ed25519")
        || n.contains("ed448")
    {
        Grade::Strong
    } else {
        Grade::Unknown
    }
}

/// Grade the public key strength: RSA ≥ 2048-bit and EC ≥ 256-bit (and EdDSA)
/// are strong; smaller RSA/EC and DSA are weak.
pub(crate) fn public_key_grade(cert: &X509Certificate) -> Grade {
    match cert.public_key().parsed() {
        Ok(PublicKey::RSA(rsa)) => {
            if rsa.key_size() >= 2048 {
                Grade::Strong
            } else {
                Grade::Weak
            }
        }
        Ok(PublicKey::EC(ec)) => {
            if ec.key_size() >= 256 {
                Grade::Strong
            } else {
                Grade::Weak
            }
        }
        Ok(PublicKey::DSA(_)) => Grade::Weak,
        _ => {
            let oid = &cert.public_key().algorithm.algorithm;
            let name = oid2sn(oid, oid_registry()).unwrap_or("").to_ascii_lowercase();
            if name.contains("ed25519") || name.contains("ed448") {
                Grade::Strong
            } else {
                Grade::Unknown
            }
        }
    }
}

/// Whether the certificate subject contains a Common Name (CN) attribute.
pub(crate) fn has_common_name(cert: &X509Certificate) -> bool {
    cert.subject().iter_common_name().next().is_some()
}

/// Whether the certificate subject is entirely empty (no RDNs at all).
pub(crate) fn subject_is_empty(cert: &X509Certificate) -> bool {
    cert.subject().iter().next().is_none()
}

/// If any non-root certificate in the presented chain uses a weak signature
/// algorithm (SHA-1/MD5), return its name. Catches weak intermediates too
/// (e.g. a SHA-256 leaf signed by a SHA-1 intermediate). Self-signed roots are
/// skipped because a root's signature is irrelevant (trust is by identity).
pub(crate) fn chain_weak_signature(chain: &[CertificateDer<'static>]) -> Option<String> {
    for der in chain {
        if let Ok((_, c)) = X509Certificate::from_der(der.as_ref()) {
            if c.issuer().to_string() == c.subject().to_string() {
                continue;
            }
            let name = signature_algorithm_string(&c);
            if matches!(signature_grade(&name), Grade::Weak) {
                return Some(name);
            }
        }
    }
    None
}

/// DNS Subject Alternative Names.
pub(crate) fn sans(cert: &X509Certificate) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(Some(ext)) = cert.subject_alternative_name() {
        for gn in &ext.value.general_names {
            if let GeneralName::DNSName(dns) = gn {
                out.push(dns.to_string());
            }
        }
    }
    out
}

/// Certificate serial number as a colon-separated hex string.
pub(crate) fn serial_string(cert: &X509Certificate) -> String {
    cert.tbs_certificate.raw_serial_as_string()
}

/// Friendly name for a negotiated/probed protocol version.
pub(crate) fn protocol_name(v: ProtocolVersion) -> &'static str {
    match v {
        ProtocolVersion::TLSv1_3 => "TLSv1.3",
        ProtocolVersion::TLSv1_2 => "TLSv1.2",
        ProtocolVersion::TLSv1_1 => "TLSv1.1",
        ProtocolVersion::TLSv1_0 => "TLSv1.0",
        ProtocolVersion::SSLv3 => "SSLv3",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_matches_known_sha256_vector() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let got = fingerprint_sha256(b"abc");
        assert_eq!(
            got,
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
        );
    }

    #[test]
    fn trust_reason_maps_common_errors_to_clean_text() {
        assert_eq!(
            trust_reason(&RustlsError::InvalidCertificate(CertificateError::Expired)),
            "certificate expired"
        );
        assert_eq!(
            trust_reason(&RustlsError::InvalidCertificate(CertificateError::NotValidYet)),
            "certificate not yet valid"
        );
        assert_eq!(
            trust_reason(&RustlsError::InvalidCertificate(CertificateError::UnknownIssuer)),
            "issuer not trusted (self-signed or unknown CA)"
        );
        assert_eq!(
            trust_reason(&RustlsError::InvalidCertificate(CertificateError::NotValidForName)),
            "certificate not valid for this hostname"
        );
    }

    #[test]
    fn signature_grade_classifies_common_algorithms() {
        assert!(matches!(signature_grade("sha256WithRSAEncryption"), Grade::Strong));
        assert!(matches!(signature_grade("ecdsa-with-SHA384"), Grade::Strong));
        assert!(matches!(signature_grade("ED25519"), Grade::Strong));
        assert!(matches!(signature_grade("sha1WithRSAEncryption"), Grade::Weak));
        assert!(matches!(signature_grade("md5WithRSAEncryption"), Grade::Weak));
        assert!(matches!(signature_grade("totally-unknown-alg"), Grade::Unknown));
    }
}
