// SiteOne Crawler - SSL/TLS protocol-version detection and weak cipher-suite probing (pure Rust)
// (c) Jan Reges <jan.reges@siteone.cz>

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::ProtocolVersion;
use rustls::SupportedProtocolVersion;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::ServerName;

use super::cert_info::InsecureVerifier;
use super::cipher_suites;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Upper bound of the weak cipher-suite enumeration per host (#20): at most this many handshakes …
pub(crate) const MAX_CIPHER_PROBE_HANDSHAKES: usize = 20;
/// … within at most this much time.
pub(crate) const MAX_CIPHER_PROBE_TIME: Duration = Duration::from_secs(10);

/// Largest answer read while waiting for a complete ServerHello (it may span several records of at most
/// 16 KiB each).
const MAX_SERVER_HELLO_BYTES: usize = 64 * 1024;

/// Suites offered by the legacy protocol-version probes: broad classic + ECDHE spread.
const VERSION_PROBE_SUITES: [u16; 10] = [
    0xc014, 0xc013, 0xc00a, 0xc009, // ECDHE (RSA/ECDSA, AES-CBC-SHA)
    0x0035, 0x002f, // RSA AES256/AES128-CBC-SHA
    0x000a, // RSA 3DES-EDE-CBC-SHA
    0xc012, 0xc011, // ECDHE 3DES / RC4
    0x0005, // RSA RC4-128-SHA
];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    Supported,
    NotSupported,
}

/// Build a minimal TLS ClientHello record probing exactly `version`
/// (0x0300 SSLv3, 0x0301 TLS1.0, 0x0302 TLS1.1) with the version-probe suites.
pub(crate) fn build_client_hello(version: u16, hostname: &str) -> Vec<u8> {
    build_client_hello_with_suites(version, hostname, &VERSION_PROBE_SUITES)
}

/// Build a minimal TLS ClientHello record for `version` offering exactly `suites`. SSLv3 carries no
/// extensions; TLS 1.0+ include SNI + supported_groups + ec_point_formats so that ECDHE-only
/// servers still negotiate, and TLS 1.2 adds signature_algorithms.
pub(crate) fn build_client_hello_with_suites(version: u16, hostname: &str, suites: &[u16]) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&version.to_be_bytes()); // client_version
    body.extend_from_slice(&[0u8; 32]); // random (deterministic is fine for a probe)
    body.push(0); // session_id length = 0

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

        // supported_groups, type 0x000a: every elliptic curve a server may be limited to (x25519, x448,
        // secp256r1/384r1/521r1, brainpool, the older secp/sect curves), so that no ECDHE suite is
        // refused for want of a shared group. No FFDHE group (RFC 7919): a server with its own DH
        // parameters would then have to refuse the DHE suites.
        {
            let groups: [u16; 30] = [
                0x001d, 0x0017, 0x001e, 0x0018, 0x0019, 0x001a, 0x001b, 0x001c, 0x0016, 0x0015, 0x0014, 0x0013, 0x0012,
                0x0011, 0x0010, 0x000f, 0x000e, 0x000d, 0x000c, 0x000b, 0x000a, 0x0009, 0x0008, 0x0007, 0x0006, 0x0005,
                0x0004, 0x0003, 0x0002, 0x0001,
            ];
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

        // signature_algorithms, type 0x000d (TLS 1.2 only): every scheme a TLS 1.2 server may sign its key
        // exchange with — RSA PKCS#1, RSA-PSS, ECDSA, EdDSA and DSA with SHA-1 to SHA-512 — so that no
        // suite is refused for want of a shared signature algorithm.
        if version >= 0x0303 {
            let schemes: [u16; 23] = [
                0x0804, 0x0805, 0x0806, 0x0809, 0x080a, 0x080b, 0x0401, 0x0501, 0x0601, 0x0403, 0x0503, 0x0603, 0x0807,
                0x0808, 0x0402, 0x0502, 0x0602, 0x0301, 0x0303, 0x0302, 0x0201, 0x0203, 0x0202,
            ];
            let mut list = Vec::new();
            for scheme in schemes {
                list.extend_from_slice(&scheme.to_be_bytes());
            }
            let mut payload = Vec::new();
            payload.extend_from_slice(&(list.len() as u16).to_be_bytes());
            payload.extend_from_slice(&list);

            ext.extend_from_slice(&0x000du16.to_be_bytes());
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

/// What a server answered to a probing ClientHello.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ServerHelloReply {
    /// A complete ServerHello: the negotiated protocol version and cipher suite.
    Hello { version: u16, cipher_suite: u16 },
    /// Not enough bytes yet.
    Incomplete,
    /// An alert: the offer was refused.
    Refused,
    /// Neither an alert nor a well-formed ServerHello.
    Malformed,
}

/// Parse the start of a server's answer: an alert, or a ServerHello, which may span several handshake
/// records (RFC 5246 §6.2.1).
pub(crate) fn parse_server_hello(buf: &[u8]) -> ServerHelloReply {
    // The handshake bytes of the records read so far; the last record may still be incomplete.
    let mut handshake: Vec<u8> = Vec::new();
    let mut record_at = 0;
    loop {
        if handshake.len() >= 4 {
            if handshake[0] != 0x02 {
                return ServerHelloReply::Malformed; // not a ServerHello
            }
            let hello_len = ((handshake[1] as usize) << 16) | ((handshake[2] as usize) << 8) | handshake[3] as usize;
            if handshake.len() >= 4 + hello_len {
                return parse_server_hello_body(&handshake[4..4 + hello_len]);
            }
        }
        if buf.len() < record_at + 5 {
            return ServerHelloReply::Incomplete;
        }
        match buf[record_at] {
            0x16 => {}
            0x15 if record_at == 0 => return ServerHelloReply::Refused,
            _ => return ServerHelloReply::Malformed,
        }
        let record_end = record_at + 5 + u16::from_be_bytes([buf[record_at + 3], buf[record_at + 4]]) as usize;
        handshake.extend_from_slice(&buf[record_at + 5..record_end.min(buf.len())]);
        record_at = record_end;
    }
}

/// server_version(2) + random(32) + session_id_len(1) + session_id + cipher_suite(2) + compression(1)
fn parse_server_hello_body(body: &[u8]) -> ServerHelloReply {
    if body.len() < 35 {
        return ServerHelloReply::Malformed;
    }
    let suite_at = 35 + body[34] as usize;
    if body.len() < suite_at + 3 {
        return ServerHelloReply::Malformed;
    }
    ServerHelloReply::Hello {
        version: u16::from_be_bytes([body[0], body[1]]),
        cipher_suite: u16::from_be_bytes([body[suite_at], body[suite_at + 1]]),
    }
}

/// Read until the answer to a probing ClientHello is decided (the legacy version probe stops after
/// 11 bytes, which is too early to see the cipher suite). Every read waits at most PROBE_TIMEOUT and
/// never past `deadline`, so a server that trickles its answer cannot stretch the per-host budget.
/// A ServerHello or an alert decides the offer, and so does a connection closed before any answer
/// (servers without a shared suite often just close it). Silence, a cut-off or a malformed answer
/// leave it undecided: Err says why.
fn read_server_hello(sock: &mut TcpStream, deadline: Instant) -> Result<ServerHelloReply, Incomplete> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        match parse_server_hello(&buf) {
            ServerHelloReply::Incomplete if buf.len() < MAX_SERVER_HELLO_BYTES => {}
            ServerHelloReply::Incomplete | ServerHelloReply::Malformed => return Err(Incomplete::Unanswered),
            decided => return Ok(decided),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Incomplete::LimitReached);
        }
        let _ = sock.set_read_timeout(Some(PROBE_TIMEOUT.min(remaining)));
        match sock.read(&mut chunk) {
            Ok(0) if buf.is_empty() => return Ok(ServerHelloReply::Refused),
            Ok(0) => return Err(Incomplete::Unanswered),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) if Instant::now() >= deadline => return Err(Incomplete::LimitReached),
            Err(e) if buf.is_empty() && !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Ok(ServerHelloReply::Refused); // connection reset
            }
            Err(_) => return Err(Incomplete::Unanswered),
        }
    }
}

/// Why a cipher-suite enumeration stopped before it was complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Incomplete {
    /// The per-host budget (handshakes or time) is spent.
    LimitReached,
    /// No address of the host accepted a TCP connection.
    Unreachable,
    /// The server did not answer a probe clearly: it went silent, cut its answer off or sent neither
    /// a ServerHello nor an alert.
    Unanswered,
}

/// Outcome of one probing handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Offer {
    /// The server picked this suite from the offer.
    Accepted(u16),
    /// The server refused every offered suite (alert, closed connection, other version or suite).
    Refused,
    /// The probe cannot go on: the enumeration is incomplete.
    Stopped(Incomplete),
}

/// Open a TCP connection to the first address that accepts one before `deadline`, and move that
/// address to the front so that the following handshakes go straight to it.
fn connect(addrs: &mut [SocketAddr], deadline: Instant) -> Result<TcpStream, Incomplete> {
    for index in 0..addrs.len() {
        let timeout = PROBE_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()));
        if timeout.is_zero() {
            return Err(Incomplete::LimitReached);
        }
        if let Ok(sock) = TcpStream::connect_timeout(&addrs[index], timeout) {
            addrs[..=index].rotate_right(1);
            return Ok(sock);
        }
    }
    if Instant::now() >= deadline {
        Err(Incomplete::LimitReached)
    } else {
        Err(Incomplete::Unreachable)
    }
}

/// One handshake over a fresh TCP connection offering `suites` for `version`, finished by `deadline`.
fn offer_suites(addrs: &mut [SocketAddr], hostname: &str, version: u16, suites: &[u16], deadline: Instant) -> Offer {
    let mut sock = match connect(addrs, deadline) {
        Ok(sock) => sock,
        Err(reason) => return Offer::Stopped(reason),
    };
    let timeout = PROBE_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()));
    if timeout.is_zero() {
        return Offer::Stopped(Incomplete::LimitReached);
    }
    let _ = sock.set_write_timeout(Some(timeout));
    if sock
        .write_all(&build_client_hello_with_suites(version, hostname, suites))
        .is_err()
    {
        return Offer::Stopped(Incomplete::Unanswered);
    }
    match read_server_hello(&mut sock, deadline) {
        Err(reason) => Offer::Stopped(reason),
        Ok(ServerHelloReply::Hello {
            version: negotiated,
            cipher_suite,
        }) if negotiated == version && suites.contains(&cipher_suite) => Offer::Accepted(cipher_suite),
        Ok(_) => Offer::Refused,
    }
}

/// Limits the number and the total time of cipher-suite probing handshakes for one host.
pub(crate) struct ProbeBudget {
    handshakes_left: usize,
    deadline: Instant,
}

impl ProbeBudget {
    pub(crate) fn new(max_handshakes: usize, max_time: Duration) -> Self {
        Self {
            handshakes_left: max_handshakes,
            deadline: Instant::now() + max_time,
        }
    }

    /// Reserve one handshake and return the deadline it must finish by, or None when the budget is spent.
    fn next_handshake(&mut self) -> Option<Instant> {
        if self.handshakes_left == 0 || Instant::now() >= self.deadline {
            return None;
        }
        self.handshakes_left -= 1;
        Some(self.deadline)
    }
}

/// Weak cipher suites a server accepted.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CipherProbeResult {
    /// (protocol version, IANA suite id) pairs in detection order.
    pub accepted: Vec<(u16, u16)>,
    /// Why the enumeration stopped early, so that `accepted` may be incomplete; None when it finished.
    pub incomplete: Option<Incomplete>,
}

/// Enumerate the insecure and non-forward-secret suites `hostname` accepts for each of `versions`
/// (probed in the given order, highest first).
pub(crate) fn probe_weak_cipher_suites(
    hostname: &str,
    port: u16,
    versions: &[u16],
    budget: &mut ProbeBudget,
) -> CipherProbeResult {
    let addrs: Vec<SocketAddr> = format!("{}:{}", hostname, port)
        .to_socket_addrs()
        .map(|addrs| addrs.collect())
        .unwrap_or_default();
    probe_addresses(addrs, hostname, versions, budget)
}

/// The enumeration against the resolved addresses of `hostname`, tried in order like the other probes.
fn probe_addresses(
    mut addrs: Vec<SocketAddr>,
    hostname: &str,
    versions: &[u16],
    budget: &mut ProbeBudget,
) -> CipherProbeResult {
    if addrs.is_empty() {
        return CipherProbeResult {
            accepted: Vec::new(),
            incomplete: Some(Incomplete::Unreachable),
        };
    }
    enumerate_weak_suites(versions, |version, suites| match budget.next_handshake() {
        Some(deadline) => offer_suites(&mut addrs, hostname, version, suites, deadline),
        None => Offer::Stopped(Incomplete::LimitReached),
    })
}

/// The enumeration with the handshake injected (so it can be tested without a network): insecure
/// suites are enumerated completely — offer all, record the pick, remove it, repeat — while for
/// static RSA one accepted suite answers the question, so that probe stops at the first hit.
fn enumerate_weak_suites(versions: &[u16], mut offer: impl FnMut(u16, &[u16]) -> Offer) -> CipherProbeResult {
    let mut result = CipherProbeResult {
        accepted: Vec::new(),
        incomplete: None,
    };
    for &version in versions {
        let mut remaining: Vec<u16> = cipher_suites::INSECURE_SUITES.iter().map(|(id, _)| *id).collect();
        while !remaining.is_empty() {
            match offer(version, &remaining) {
                Offer::Accepted(id) => {
                    result.accepted.push((version, id));
                    remaining.retain(|suite| *suite != id);
                }
                Offer::Refused => break,
                Offer::Stopped(reason) => {
                    result.incomplete = Some(reason);
                    return result;
                }
            }
        }
        let static_rsa_known = result
            .accepted
            .iter()
            .any(|(_, id)| cipher_suites::suite_name(*id).is_some_and(cipher_suites::lacks_forward_secrecy));
        if !static_rsa_known {
            let static_rsa: Vec<u16> = cipher_suites::STATIC_RSA_SUITES.iter().map(|(id, _)| *id).collect();
            match offer(version, &static_rsa) {
                Offer::Accepted(id) => result.accepted.push((version, id)),
                Offer::Refused => {}
                Offer::Stopped(reason) => {
                    result.incomplete = Some(reason);
                    return result;
                }
            }
        }
    }
    result
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

    /// ServerHello captured from 3des.badssl.com: TLS 1.2, TLS_RSA_WITH_3DES_EDE_CBC_SHA, 32-byte
    /// session id, one empty extension.
    const BADSSL_3DES_SERVER_HELLO: [u8; 85] = [
        0x16, 0x03, 0x03, 0x00, 0x50, 0x02, 0x00, 0x00, 0x4c, 0x03, 0x03, 0x5b, 0x37, 0x0d, 0x45, 0x03, 0xdf, 0x15,
        0x95, 0x59, 0xd8, 0x34, 0x33, 0x8b, 0x11, 0xdd, 0xc1, 0x6d, 0xca, 0x48, 0x29, 0xd0, 0x51, 0x26, 0x18, 0x4d,
        0x58, 0xa4, 0xa5, 0xaf, 0xb2, 0xd9, 0xfc, 0x20, 0xda, 0x1a, 0x75, 0x92, 0x2f, 0x83, 0xe6, 0x99, 0x49, 0xc6,
        0x22, 0x77, 0xb3, 0x9b, 0x6c, 0x76, 0x78, 0xf9, 0x55, 0x71, 0x8e, 0xa4, 0xc6, 0x06, 0x9e, 0x89, 0x9e, 0xaa,
        0x8e, 0xdf, 0x74, 0xf0, 0x00, 0x0a, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn parses_a_captured_server_hello() {
        assert_eq!(
            parse_server_hello(&BADSSL_3DES_SERVER_HELLO),
            ServerHelloReply::Hello {
                version: 0x0303,
                cipher_suite: 0x000A
            }
        );
    }

    #[test]
    fn parses_a_server_hello_without_session_id() {
        let mut hello = vec![0x16, 0x03, 0x01, 0x00, 0x2a, 0x02, 0x00, 0x00, 0x26, 0x03, 0x01];
        hello.extend_from_slice(&[0u8; 32]);
        hello.extend_from_slice(&[0x00, 0xc0, 0x11, 0x00]);
        assert_eq!(
            parse_server_hello(&hello),
            ServerHelloReply::Hello {
                version: 0x0301,
                cipher_suite: 0xC011
            }
        );
    }

    #[test]
    fn server_hello_needs_the_whole_message() {
        for cut in [0, 4, 8, 40, 84] {
            assert_eq!(
                parse_server_hello(&BADSSL_3DES_SERVER_HELLO[..cut]),
                ServerHelloReply::Incomplete,
                "cut at {cut}"
            );
        }
    }

    /// The captured ServerHello re-framed as two handshake records, split after `at` handshake bytes.
    fn fragmented_server_hello(at: usize) -> Vec<u8> {
        let handshake = &BADSSL_3DES_SERVER_HELLO[5..];
        let record = |part: &[u8]| {
            let mut record = vec![0x16, 0x03, 0x03];
            record.extend_from_slice(&(part.len() as u16).to_be_bytes());
            record.extend_from_slice(part);
            record
        };
        [record(&handshake[..at]), record(&handshake[at..])].concat()
    }

    #[test]
    fn a_server_hello_split_across_records_is_reassembled() {
        // Split inside the handshake header and inside the body (RFC 5246 §6.2.1 allows both).
        for at in [2, 20] {
            let hello = fragmented_server_hello(at);
            assert_eq!(
                parse_server_hello(&hello),
                ServerHelloReply::Hello {
                    version: 0x0303,
                    cipher_suite: 0x000A
                },
                "split at {at}"
            );
            for cut in [5 + at, 5 + at + 5, hello.len() - 1] {
                assert_eq!(
                    parse_server_hello(&hello[..cut]),
                    ServerHelloReply::Incomplete,
                    "split at {at}, cut at {cut}"
                );
            }
        }
    }

    #[test]
    fn an_alert_is_a_refusal() {
        assert_eq!(
            parse_server_hello(&[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x28]),
            ServerHelloReply::Refused
        );
    }

    #[test]
    fn other_answers_are_malformed() {
        let mut certificate = BADSSL_3DES_SERVER_HELLO;
        certificate[5] = 0x0b;
        assert_eq!(parse_server_hello(&certificate), ServerHelloReply::Malformed);
        assert_eq!(
            parse_server_hello(b"HTTP/1.1 400 Bad Request"),
            ServerHelloReply::Malformed
        );
        // An alert after the first bytes of a ServerHello, and a ServerHello too short for a suite.
        let mut cut_off = fragmented_server_hello(20)[..25].to_vec();
        cut_off.extend_from_slice(&[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x28]);
        assert_eq!(parse_server_hello(&cut_off), ServerHelloReply::Malformed);
        let short = [0x16, 0x03, 0x03, 0x00, 0x06, 0x02, 0x00, 0x00, 0x02, 0x03, 0x03];
        assert_eq!(parse_server_hello(&short), ServerHelloReply::Malformed);
    }

    #[test]
    fn tls12_hello_offers_the_given_suites_and_signature_algorithms() {
        let hello = build_client_hello_with_suites(0x0303, "example.com", &[0x000A, 0xC012]);
        // record(5) + handshake(4) + version(2) + random(32) + session id length(1) = 44
        assert_eq!(&hello[44..46], &[0x00, 0x04], "cipher suites length");
        assert_eq!(&hello[46..50], &[0x00, 0x0A, 0xC0, 0x12]);
        // compression (2 bytes) and extensions length (2 bytes) follow; then the extensions.
        let mut types = Vec::new();
        let mut pos = 54;
        while pos + 4 <= hello.len() {
            types.push(u16::from_be_bytes([hello[pos], hello[pos + 1]]));
            pos += 4 + u16::from_be_bytes([hello[pos + 2], hello[pos + 3]]) as usize;
        }
        assert_eq!(types, vec![0x0000, 0x000a, 0x000b, 0x000d]);
        let tls11 = build_client_hello_with_suites(0x0302, "example.com", &[0x000A, 0xC012]);
        assert!(tls11.len() < hello.len(), "signature_algorithms is TLS 1.2 only");
    }

    #[test]
    fn enumeration_collects_every_insecure_suite_the_server_accepts() {
        let server_prefers = [0xC012u16, 0x0016, 0x000A];
        let mut handshakes = 0;
        let result = enumerate_weak_suites(&[0x0303], |version, offered| {
            handshakes += 1;
            assert_eq!(version, 0x0303);
            server_prefers
                .iter()
                .copied()
                .find(|suite| offered.contains(suite))
                .map_or(Offer::Refused, Offer::Accepted)
        });
        assert_eq!(
            result.accepted,
            vec![(0x0303, 0xC012), (0x0303, 0x0016), (0x0303, 0x000A)]
        );
        assert_eq!(result.incomplete, None);
        // 3 accepted + 1 refused; static RSA is already known from TLS_RSA_WITH_3DES_EDE_CBC_SHA.
        assert_eq!(handshakes, 4);
    }

    #[test]
    fn static_rsa_probe_stops_at_the_first_hit() {
        let mut handshakes = 0;
        let result = enumerate_weak_suites(&[0x0303, 0x0302], |_, offered| {
            handshakes += 1;
            if offered.contains(&0x009C) {
                Offer::Accepted(0x009C)
            } else {
                Offer::Refused
            }
        });
        assert_eq!(result.accepted, vec![(0x0303, 0x009C)]);
        // TLS 1.2: insecure offer refused + static-RSA hit; TLS 1.1: insecure offer refused only.
        assert_eq!(handshakes, 3);
        assert_eq!(result.incomplete, None);
    }

    #[test]
    fn static_rsa_is_found_beyond_aes() {
        // CAMELLIA128-SHA, ARIA128-GCM-SHA256, SEED-SHA: static RSA key exchange like the AES suites.
        for suite in [0x0041u16, 0xC050, 0x0096] {
            let result = enumerate_weak_suites(&[0x0303], |_, offered| {
                if offered.contains(&suite) {
                    Offer::Accepted(suite)
                } else {
                    Offer::Refused
                }
            });
            assert_eq!(result.accepted, vec![(0x0303, suite)], "0x{suite:04X}");
            assert!(
                cipher_suites::suite_name(suite).is_some_and(cipher_suites::lacks_forward_secrecy),
                "0x{suite:04X} is reported as a suite without forward secrecy"
            );
        }
    }

    #[test]
    fn exhausted_budget_marks_the_result_incomplete() {
        let mut budget = ProbeBudget::new(2, Duration::from_secs(10));
        let result = enumerate_weak_suites(&[0x0303], |_, offered| match budget.next_handshake() {
            Some(_) => Offer::Accepted(offered[0]),
            None => Offer::Stopped(Incomplete::LimitReached),
        });
        assert_eq!(result.accepted.len(), 2);
        assert_eq!(result.incomplete, Some(Incomplete::LimitReached));
    }

    /// A local port nothing listens on.
    fn closed_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port()
    }

    #[test]
    fn the_probe_tries_every_resolved_address() {
        // A server that refuses every offer with a handshake_failure alert.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let working = listener.local_addr().expect("local address");
        std::thread::spawn(move || {
            for mut stream in listener.incoming().take(2).flatten() {
                let mut client_hello = [0u8; 1024];
                let _ = stream.read(&mut client_hello);
                let _ = stream.write_all(&[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x28]);
            }
        });
        let unreachable: SocketAddr = format!("127.0.0.1:{}", closed_port()).parse().expect("address");
        let mut budget = ProbeBudget::new(MAX_CIPHER_PROBE_HANDSHAKES, MAX_CIPHER_PROBE_TIME);
        let result = probe_addresses(vec![unreachable, working], "localhost", &[0x0303], &mut budget);
        assert!(result.accepted.is_empty() && result.incomplete.is_none(), "{result:?}");
    }

    #[test]
    fn a_trickling_server_cannot_stretch_the_probe_budget() {
        // A server that answers with the header of a 16 KiB ServerHello and then sends one byte every
        // 50 ms: every single read succeeds, so only the per-host deadline can end the handshake.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("local address").port();
        std::thread::spawn(move || {
            let Some(Ok(mut stream)) = listener.incoming().next() else {
                return;
            };
            let mut client_hello = [0u8; 1024];
            let _ = stream.read(&mut client_hello);
            if stream
                .write_all(&[0x16, 0x03, 0x03, 0x40, 0x00, 0x02, 0x00, 0x3f, 0xfc])
                .is_err()
            {
                return;
            }
            for _ in 0..400 {
                std::thread::sleep(Duration::from_millis(50));
                if stream.write_all(&[0]).is_err() {
                    return;
                }
            }
        });

        let (sender, receiver) = std::sync::mpsc::channel();
        let started = Instant::now();
        std::thread::spawn(move || {
            let mut budget = ProbeBudget::new(MAX_CIPHER_PROBE_HANDSHAKES, Duration::from_secs(1));
            let _ = sender.send(probe_weak_cipher_suites("127.0.0.1", port, &[0x0303], &mut budget));
        });
        let result = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("the probe returns once its 1 s budget is spent");
        assert!(
            started.elapsed() < Duration::from_millis(1500),
            "took {:?}",
            started.elapsed()
        );
        assert_eq!(result.incomplete, Some(Incomplete::LimitReached), "{result:?}");
    }

    const HANDSHAKE_FAILURE: [u8; 7] = [0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x28];

    /// A ServerHello record for `version` selecting `suite`, without session id and extensions.
    fn server_hello(version: u16, suite: u16) -> Vec<u8> {
        let [major, minor] = version.to_be_bytes();
        let mut hello = vec![0x16, major, minor, 0x00, 0x2a, 0x02, 0x00, 0x00, 0x26, major, minor];
        hello.extend_from_slice(&[0u8; 32]);
        hello.push(0); // session id length
        hello.extend_from_slice(&suite.to_be_bytes());
        hello.push(0); // compression
        hello
    }

    /// Read one TLS record (header included).
    fn read_record(stream: &mut TcpStream) -> Option<Vec<u8>> {
        let mut header = [0u8; 5];
        stream.read_exact(&mut header).ok()?;
        let mut record = header.to_vec();
        record.resize(5 + u16::from_be_bytes([header[3], header[4]]) as usize, 0);
        stream.read_exact(&mut record[5..]).ok()?;
        Some(record)
    }

    /// The suites and extensions of a probing ClientHello, as a test server sees them.
    struct SeenClientHello {
        suites: Vec<u16>,
        extensions: Vec<(u16, Vec<u8>)>,
    }

    impl SeenClientHello {
        /// Parse a ClientHello record.
        fn parse(record: &[u8]) -> Option<Self> {
            let body = record.get(9..)?;
            let u16_at = |at: usize| body.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
            let list = |bytes: &[u8]| -> Vec<u16> {
                bytes
                    .chunks_exact(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                    .collect()
            };
            // client_version(2) + random(32), then session id, suites, compression methods, extensions
            let mut pos = 35 + *body.get(34)? as usize;
            let suites_len = u16_at(pos)? as usize;
            let suites = list(body.get(pos + 2..pos + 2 + suites_len)?);
            pos += 2 + suites_len;
            pos += 1 + *body.get(pos)? as usize;
            let mut extensions = Vec::new();
            if let Some(extensions_len) = u16_at(pos) {
                let end = pos + 2 + extensions_len as usize;
                pos += 2;
                while pos + 4 <= end {
                    let len = u16_at(pos + 2)? as usize;
                    extensions.push((u16_at(pos)?, body.get(pos + 4..pos + 4 + len)?.to_vec()));
                    pos += 4 + len;
                }
            }
            Some(Self { suites, extensions })
        }

        /// The code points of a list extension (supported_groups, signature_algorithms).
        fn list(&self, extension: u16) -> Vec<u16> {
            self.extensions
                .iter()
                .find(|(kind, _)| *kind == extension)
                .and_then(|(_, data)| data.get(2..))
                .map(|codes| {
                    codes
                        .chunks_exact(2)
                        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    /// A TLS 1.2 server whose only weak suite is TLS_ECDHE_RSA_WITH_NULL_SHA. Like OpenSSL, it picks it
    /// only when the ClientHello also offers what its key exchange and certificate need (`negotiates`);
    /// every other handshake ends with a handshake_failure alert.
    fn weak_ecdhe_server(negotiates: fn(&SeenClientHello) -> bool) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("local address").port();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let answer = match read_record(&mut stream).as_deref().and_then(SeenClientHello::parse) {
                    Some(hello) if hello.suites.contains(&0xC010) && negotiates(&hello) => server_hello(0x0303, 0xC010),
                    _ => HANDSHAKE_FAILURE.to_vec(),
                };
                let _ = stream.write_all(&answer);
            }
        });
        port
    }

    #[test]
    fn servers_limited_to_p384_p521_or_rsa_pss_still_reveal_weak_suites() {
        let servers: [(&str, fn(&SeenClientHello) -> bool); 3] = [
            ("P-384 only", |hello| hello.list(0x000a).contains(&0x0018)),
            ("P-521 only", |hello| hello.list(0x000a).contains(&0x0019)),
            ("RSA-PSS only", |hello| hello.list(0x000d).contains(&0x0804)),
        ];
        for (server, negotiates) in servers {
            let port = weak_ecdhe_server(negotiates);
            let mut budget = ProbeBudget::new(MAX_CIPHER_PROBE_HANDSHAKES, MAX_CIPHER_PROBE_TIME);
            let result = probe_weak_cipher_suites("127.0.0.1", port, &[0x0303], &mut budget);
            assert_eq!(result.accepted, vec![(0x0303, 0xC010)], "{server}");
            assert_eq!(result.incomplete, None, "{server}");
        }
    }

    /// A server that sends `first_answer` to the first handshake, keeps that connection open for `hold`
    /// and then closes it; every later handshake ends with a handshake_failure alert.
    fn server_answering_first(first_answer: Vec<u8>, hold: Duration) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("local address").port();
        std::thread::spawn(move || {
            let mut first_answer = Some(first_answer);
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = read_record(&mut stream);
                match first_answer.take() {
                    Some(answer) => {
                        std::thread::spawn(move || {
                            let _ = stream.write_all(&answer);
                            std::thread::sleep(hold);
                        });
                    }
                    None => {
                        let _ = stream.write_all(&HANDSHAKE_FAILURE);
                    }
                }
            }
        });
        port
    }

    fn probe_local(port: u16) -> CipherProbeResult {
        let mut budget = ProbeBudget::new(MAX_CIPHER_PROBE_HANDSHAKES, MAX_CIPHER_PROBE_TIME);
        probe_weak_cipher_suites("127.0.0.1", port, &[0x0303], &mut budget)
    }

    #[test]
    fn a_server_that_goes_quiet_leaves_the_probe_incomplete() {
        // No answer within PROBE_TIMEOUT, long before the per-host deadline: not a refusal.
        let port = server_answering_first(Vec::new(), PROBE_TIMEOUT + Duration::from_secs(1));
        let result = probe_local(port);
        assert!(result.accepted.is_empty(), "{result:?}");
        assert_eq!(result.incomplete, Some(Incomplete::Unanswered), "{result:?}");
    }

    #[test]
    fn a_cut_off_server_hello_leaves_the_probe_incomplete() {
        // The record header and the first 20 bytes of the ServerHello, then the connection closes.
        let port = server_answering_first(BADSSL_3DES_SERVER_HELLO[..25].to_vec(), Duration::ZERO);
        let result = probe_local(port);
        assert!(result.accepted.is_empty(), "{result:?}");
        assert_eq!(result.incomplete, Some(Incomplete::Unanswered), "{result:?}");
    }

    #[test]
    fn a_connection_closed_without_an_answer_is_a_refusal() {
        // Servers that do not share a suite often just close the connection instead of sending an alert.
        let port = server_answering_first(Vec::new(), Duration::ZERO);
        let result = probe_local(port);
        assert_eq!(
            result,
            CipherProbeResult {
                accepted: Vec::new(),
                incomplete: None
            }
        );
    }

    // Live check against badssl.com: CARGO_PROFILE_DEV_DEBUG=0 cargo test --lib live_badssl -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_badssl_weak_cipher_hosts() {
        let probe = |host: &str| {
            let mut budget = ProbeBudget::new(MAX_CIPHER_PROBE_HANDSHAKES, MAX_CIPHER_PROBE_TIME);
            probe_weak_cipher_suites(host, 443, &[0x0303], &mut budget)
        };
        let rc4 = probe("rc4.badssl.com");
        assert!(
            rc4.accepted.iter().any(|(_, id)| *id == 0x0005 || *id == 0xC011),
            "{rc4:?}"
        );
        let triple_des = probe("3des.badssl.com");
        assert!(
            triple_des.accepted.iter().any(|(_, id)| *id == 0x000A),
            "{triple_des:?}"
        );
        let static_rsa = probe("static-rsa.badssl.com");
        assert!(
            static_rsa
                .accepted
                .iter()
                .any(|(_, id)| cipher_suites::suite_name(*id).is_some_and(cipher_suites::lacks_forward_secrecy)),
            "{static_rsa:?}"
        );
        let modern = probe("mozilla-modern.badssl.com");
        assert!(modern.accepted.is_empty() && modern.incomplete.is_none(), "{modern:?}");
    }
}
