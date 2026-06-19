// SiteOne Crawler - SSL/TLS protocol-version detection (pure Rust)
// (c) Jan Reges <jan.reges@siteone.cz>

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::ProtocolVersion;
use rustls::SupportedProtocolVersion;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::ServerName;

use super::cert_info::InsecureVerifier;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    Supported,
    NotSupported,
}

/// Build a minimal TLS ClientHello record probing exactly `version`
/// (0x0300 SSLv3, 0x0301 TLS1.0, 0x0302 TLS1.1). SSLv3 carries no extensions;
/// TLS1.0/1.1 include SNI + supported_groups + ec_point_formats so that
/// ECDHE-only servers still negotiate.
pub(crate) fn build_client_hello(version: u16, hostname: &str) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&version.to_be_bytes()); // client_version
    body.extend_from_slice(&[0u8; 32]); // random (deterministic is fine for a probe)
    body.push(0); // session_id length = 0

    // Cipher suites: broad classic + ECDHE spread for legacy servers.
    let suites: [u16; 10] = [
        0xc014, 0xc013, 0xc00a, 0xc009, // ECDHE (RSA/ECDSA, AES-CBC-SHA)
        0x0035, 0x002f, // RSA AES256/AES128-CBC-SHA
        0x000a, // RSA 3DES-EDE-CBC-SHA
        0xc012, 0xc011, // ECDHE 3DES / RC4
        0x0005, // RSA RC4-128-SHA
    ];
    let mut suite_bytes = Vec::with_capacity(suites.len() * 2);
    for s in suites {
        suite_bytes.extend_from_slice(&s.to_be_bytes());
    }
    body.extend_from_slice(&(suite_bytes.len() as u16).to_be_bytes());
    body.extend_from_slice(&suite_bytes);

    // Compression methods: null only.
    body.push(1);
    body.push(0);

    // Extensions are illegal in SSLv3; only emit for TLS 1.0+.
    if version >= 0x0301 {
        let mut ext: Vec<u8> = Vec::new();

        // server_name (SNI), type 0x0000
        if !hostname.is_empty() {
            let host = hostname.as_bytes();
            let mut list = Vec::new();
            list.push(0u8); // name_type = host_name(0)
            list.extend_from_slice(&(host.len() as u16).to_be_bytes());
            list.extend_from_slice(host);

            let mut sni = Vec::new();
            sni.extend_from_slice(&(list.len() as u16).to_be_bytes()); // server_name_list length
            sni.extend_from_slice(&list);

            ext.extend_from_slice(&0x0000u16.to_be_bytes());
            ext.extend_from_slice(&(sni.len() as u16).to_be_bytes());
            ext.extend_from_slice(&sni);
        }

        // supported_groups, type 0x000a: secp256r1(0x0017), x25519(0x001d)
        {
            let groups: [u16; 2] = [0x0017, 0x001d];
            let mut gl = Vec::new();
            for g in groups {
                gl.extend_from_slice(&g.to_be_bytes());
            }
            let mut payload = Vec::new();
            payload.extend_from_slice(&(gl.len() as u16).to_be_bytes());
            payload.extend_from_slice(&gl);

            ext.extend_from_slice(&0x000au16.to_be_bytes());
            ext.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            ext.extend_from_slice(&payload);
        }

        // ec_point_formats, type 0x000b: uncompressed(0)
        {
            let payload: [u8; 2] = [1, 0]; // list length 1, format 0
            ext.extend_from_slice(&0x000bu16.to_be_bytes());
            ext.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            ext.extend_from_slice(&payload);
        }

        body.extend_from_slice(&(ext.len() as u16).to_be_bytes());
        body.extend_from_slice(&ext);
    }

    // Handshake header: ClientHello(0x01) + 3-byte length.
    let mut hs: Vec<u8> = Vec::with_capacity(body.len() + 4);
    hs.push(0x01);
    let blen = body.len();
    hs.push((blen >> 16) as u8);
    hs.push((blen >> 8) as u8);
    hs.push(blen as u8);
    hs.extend_from_slice(&body);

    // Record header: handshake(0x16) + version + 2-byte length.
    let mut rec: Vec<u8> = Vec::with_capacity(hs.len() + 5);
    rec.push(0x16);
    rec.extend_from_slice(&version.to_be_bytes());
    rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
    rec.extend_from_slice(&hs);
    rec
}

/// Interpret the first bytes of the server's response. A probe counts as
/// Supported only when the server replies with a ServerHello echoing exactly
/// the requested version; an Alert / non-handshake / mismatch / truncation is
/// NotSupported.
pub(crate) fn parse_probe_response(buf: &[u8], requested: u16) -> ProbeOutcome {
    // Need: 5-byte record header + 4-byte handshake header + 2-byte version.
    if buf.len() < 11 {
        return ProbeOutcome::NotSupported;
    }
    if buf[0] != 0x16 {
        return ProbeOutcome::NotSupported; // 0x15 alert or anything else
    }
    if buf[5] != 0x02 {
        return ProbeOutcome::NotSupported; // not a ServerHello
    }
    let server_version = u16::from_be_bytes([buf[9], buf[10]]);
    if server_version == requested {
        ProbeOutcome::Supported
    } else {
        ProbeOutcome::NotSupported
    }
}

/// Probe a legacy version over a raw TCP socket.
/// Returns None on a TCP connection failure (host unreachable), otherwise
/// Some(true/false) for supported/not-supported.
pub(crate) fn probe_legacy_version(hostname: &str, port: u16, version: u16) -> Option<bool> {
    let mut sock = match TcpStream::connect(format!("{}:{}", hostname, port)) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let _ = sock.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = sock.set_write_timeout(Some(PROBE_TIMEOUT));

    let hello = build_client_hello(version, hostname);
    if sock.write_all(&hello).is_err() {
        return Some(false);
    }

    // Accumulate at least 11 bytes (enough for record + ServerHello version).
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    let mut tmp = [0u8; 512];
    loop {
        match sock.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() >= 11 {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if buf.is_empty() {
        return Some(false);
    }
    Some(parse_probe_response(&buf, version) == ProbeOutcome::Supported)
}

/// Detect a modern version (TLS 1.2 / 1.3) by attempting a version-pinned
/// rustls handshake with the non-validating verifier. Returns true if the
/// handshake negotiated exactly `expected`.
pub(crate) fn detect_modern_version(
    hostname: &str,
    port: u16,
    version: &'static SupportedProtocolVersion,
    expected: ProtocolVersion,
) -> bool {
    let provider = match CryptoProvider::get_default() {
        Some(p) => p.clone(),
        None => Arc::new(rustls::crypto::ring::default_provider()),
    };
    let config = match rustls::ClientConfig::builder_with_provider(provider).with_protocol_versions(&[version]) {
        Ok(b) => b,
        Err(_) => return false,
    }
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
    .with_no_client_auth();

    let server_name = match ServerName::try_from(hostname.to_string()) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mut conn = match rustls::ClientConnection::new(Arc::new(config), server_name) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut sock = match TcpStream::connect(format!("{}:{}", hostname, port)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = sock.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = sock.set_write_timeout(Some(PROBE_TIMEOUT));

    while conn.is_handshaking() {
        if conn.complete_io(&mut sock).is_err() {
            break;
        }
    }

    conn.protocol_version() == Some(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_hello_has_correct_record_and_handshake_framing() {
        let hello = build_client_hello(0x0301, "example.com"); // TLS 1.0
        // Record header: handshake(0x16), version 0x0301, then 2-byte length
        assert_eq!(hello[0], 0x16);
        assert_eq!(&hello[1..3], &[0x03, 0x01]);
        let rec_len = u16::from_be_bytes([hello[3], hello[4]]) as usize;
        assert_eq!(rec_len, hello.len() - 5, "record length must cover the rest");
        // Handshake header: ClientHello(0x01) + 3-byte length
        assert_eq!(hello[5], 0x01);
        let hs_len = ((hello[6] as usize) << 16) | ((hello[7] as usize) << 8) | hello[8] as usize;
        assert_eq!(hs_len, hello.len() - 9, "handshake length must cover the body");
        // client_version inside the body
        assert_eq!(&hello[9..11], &[0x03, 0x01]);
    }

    #[test]
    fn sslv3_hello_carries_no_extensions() {
        // SSLv3 (0x0300) must NOT include the extensions block.
        let v3 = build_client_hello(0x0300, "example.com");
        let v10 = build_client_hello(0x0301, "example.com");
        assert!(v3.len() < v10.len(), "sslv3 hello should be shorter (no extensions)");
    }

    #[test]
    fn parse_supported_when_serverhello_echoes_version() {
        // record: handshake(0x16) ver 0x0301 len 0x0004 | ServerHello(0x02) len.. server_version 0x0301
        let resp = [
            0x16, 0x03, 0x01, 0x00, 0x04, // record header (len value irrelevant to parser)
            0x02, 0x00, 0x00, 0x00, 0x03, 0x01, // handshake: type, 3-byte len, server_version
        ];
        assert_eq!(parse_probe_response(&resp, 0x0301), ProbeOutcome::Supported);
    }

    #[test]
    fn parse_not_supported_on_alert() {
        // Alert record (0x15) => protocol not supported.
        let resp = [0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x46];
        assert_eq!(parse_probe_response(&resp, 0x0301), ProbeOutcome::NotSupported);
    }

    #[test]
    fn parse_not_supported_on_version_mismatch() {
        // ServerHello echoing a different version than requested.
        let resp = [0x16, 0x03, 0x03, 0x00, 0x04, 0x02, 0x00, 0x00, 0x00, 0x03, 0x03];
        assert_eq!(parse_probe_response(&resp, 0x0301), ProbeOutcome::NotSupported);
    }

    #[test]
    fn parse_not_supported_on_truncated() {
        assert_eq!(parse_probe_response(&[0x16, 0x03], 0x0301), ProbeOutcome::NotSupported);
    }
}
