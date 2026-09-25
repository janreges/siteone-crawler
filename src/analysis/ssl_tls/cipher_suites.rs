// SiteOne Crawler - TLS cipher suites probed by the SSL/TLS analyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

/// Suites a server must never accept: NULL (no encryption), EXPORT (40/56-bit keys), anonymous
/// (no authentication), RC4, DES and 3DES. IANA code point and name.
pub(crate) const INSECURE_SUITES: [(u16, &str); 49] = [
    (0x0001, "TLS_RSA_WITH_NULL_MD5"),
    (0x0002, "TLS_RSA_WITH_NULL_SHA"),
    (0x003B, "TLS_RSA_WITH_NULL_SHA256"),
    (0xC006, "TLS_ECDHE_ECDSA_WITH_NULL_SHA"),
    (0xC010, "TLS_ECDHE_RSA_WITH_NULL_SHA"),
    (0xC001, "TLS_ECDH_ECDSA_WITH_NULL_SHA"),
    (0xC00B, "TLS_ECDH_RSA_WITH_NULL_SHA"),
    (0x0003, "TLS_RSA_EXPORT_WITH_RC4_40_MD5"),
    (0x0006, "TLS_RSA_EXPORT_WITH_RC2_CBC_40_MD5"),
    (0x0008, "TLS_RSA_EXPORT_WITH_DES40_CBC_SHA"),
    (0x0014, "TLS_DHE_RSA_EXPORT_WITH_DES40_CBC_SHA"),
    (0x0011, "TLS_DHE_DSS_EXPORT_WITH_DES40_CBC_SHA"),
    (0x0017, "TLS_DH_anon_EXPORT_WITH_RC4_40_MD5"),
    (0x0019, "TLS_DH_anon_EXPORT_WITH_DES40_CBC_SHA"),
    (0x0018, "TLS_DH_anon_WITH_RC4_128_MD5"),
    (0x001B, "TLS_DH_anon_WITH_3DES_EDE_CBC_SHA"),
    (0x0034, "TLS_DH_anon_WITH_AES_128_CBC_SHA"),
    (0x003A, "TLS_DH_anon_WITH_AES_256_CBC_SHA"),
    (0x00A6, "TLS_DH_anon_WITH_AES_128_GCM_SHA256"),
    (0x00A7, "TLS_DH_anon_WITH_AES_256_GCM_SHA384"),
    (0x006C, "TLS_DH_anon_WITH_AES_128_CBC_SHA256"),
    (0x006D, "TLS_DH_anon_WITH_AES_256_CBC_SHA256"),
    (0x0046, "TLS_DH_anon_WITH_CAMELLIA_128_CBC_SHA"),
    (0x0089, "TLS_DH_anon_WITH_CAMELLIA_256_CBC_SHA"),
    (0x00BF, "TLS_DH_anon_WITH_CAMELLIA_128_CBC_SHA256"),
    (0x00C5, "TLS_DH_anon_WITH_CAMELLIA_256_CBC_SHA256"),
    (0x009B, "TLS_DH_anon_WITH_SEED_CBC_SHA"),
    (0x001A, "TLS_DH_anon_WITH_DES_CBC_SHA"),
    (0xC015, "TLS_ECDH_anon_WITH_NULL_SHA"),
    (0xC016, "TLS_ECDH_anon_WITH_RC4_128_SHA"),
    (0xC017, "TLS_ECDH_anon_WITH_3DES_EDE_CBC_SHA"),
    (0xC018, "TLS_ECDH_anon_WITH_AES_128_CBC_SHA"),
    (0xC019, "TLS_ECDH_anon_WITH_AES_256_CBC_SHA"),
    (0x0004, "TLS_RSA_WITH_RC4_128_MD5"),
    (0x0005, "TLS_RSA_WITH_RC4_128_SHA"),
    (0xC007, "TLS_ECDHE_ECDSA_WITH_RC4_128_SHA"),
    (0xC011, "TLS_ECDHE_RSA_WITH_RC4_128_SHA"),
    (0xC002, "TLS_ECDH_ECDSA_WITH_RC4_128_SHA"),
    (0xC00C, "TLS_ECDH_RSA_WITH_RC4_128_SHA"),
    (0x0009, "TLS_RSA_WITH_DES_CBC_SHA"),
    (0x0015, "TLS_DHE_RSA_WITH_DES_CBC_SHA"),
    (0x0012, "TLS_DHE_DSS_WITH_DES_CBC_SHA"),
    (0x000A, "TLS_RSA_WITH_3DES_EDE_CBC_SHA"),
    (0x0016, "TLS_DHE_RSA_WITH_3DES_EDE_CBC_SHA"),
    (0xC008, "TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA"),
    (0xC012, "TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA"),
    (0x0013, "TLS_DHE_DSS_WITH_3DES_EDE_CBC_SHA"),
    (0xC003, "TLS_ECDH_ECDSA_WITH_3DES_EDE_CBC_SHA"),
    (0xC00D, "TLS_ECDH_RSA_WITH_3DES_EDE_CBC_SHA"),
];

/// Static-RSA key exchange with otherwise modern ciphers: accepted, but without forward secrecy.
pub(crate) const STATIC_RSA_SUITES: [(u16, &str); 6] = [
    (0x002F, "TLS_RSA_WITH_AES_128_CBC_SHA"),
    (0x0035, "TLS_RSA_WITH_AES_256_CBC_SHA"),
    (0x003C, "TLS_RSA_WITH_AES_128_CBC_SHA256"),
    (0x003D, "TLS_RSA_WITH_AES_256_CBC_SHA256"),
    (0x009C, "TLS_RSA_WITH_AES_128_GCM_SHA256"),
    (0x009D, "TLS_RSA_WITH_AES_256_GCM_SHA384"),
];

/// IANA name of a probed suite.
pub(crate) fn suite_name(id: u16) -> Option<&'static str> {
    INSECURE_SUITES
        .iter()
        .chain(STATIC_RSA_SUITES.iter())
        .find(|(code, _)| *code == id)
        .map(|(_, name)| *name)
}

/// NULL, EXPORT, anonymous, RC4, DES or 3DES, judged by the IANA name.
pub(crate) fn is_insecure(name: &str) -> bool {
    ["_NULL_", "_EXPORT_", "_anon_", "_RC4_", "_DES_", "_3DES_"]
        .iter()
        .any(|marker| name.contains(marker))
}

/// Static RSA key exchange: traffic recorded today can be decrypted later with the server's key.
pub(crate) fn lacks_forward_secrecy(name: &str) -> bool {
    name.starts_with("TLS_RSA_")
}

/// Protocol label of a probed version.
pub(crate) fn protocol_label(version: u16) -> &'static str {
    match version {
        0x0300 => "SSLv3",
        0x0301 => "TLSv1.0",
        0x0302 => "TLSv1.1",
        0x0303 => "TLSv1.2",
        _ => "unknown",
    }
}

/// "NAME (TLSv1.0, TLSv1.2)" for every accepted (protocol version, suite) pair whose suite name
/// passes `filter` — one entry per suite, in detection order.
pub(crate) fn describe_accepted(accepted: &[(u16, u16)], filter: fn(&str) -> bool) -> Vec<String> {
    let mut order: Vec<&'static str> = Vec::new();
    let mut versions: HashMap<&'static str, Vec<u16>> = HashMap::new();
    for &(version, id) in accepted {
        let Some(name) = suite_name(id) else { continue };
        if !filter(name) {
            continue;
        }
        let seen = versions.entry(name).or_default();
        if seen.is_empty() {
            order.push(name);
        }
        if !seen.contains(&version) {
            seen.push(version);
        }
    }
    order
        .into_iter()
        .map(|name| {
            let mut list = versions.remove(name).unwrap_or_default();
            list.sort_unstable();
            let labels: Vec<&str> = list.into_iter().map(protocol_label).collect();
            format!("{} ({})", name, labels.join(", "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_follows_the_iana_names() {
        for (_, name) in INSECURE_SUITES {
            assert!(is_insecure(name), "{name} must be insecure");
        }
        for (_, name) in STATIC_RSA_SUITES {
            assert!(!is_insecure(name), "{name} is not insecure");
            assert!(lacks_forward_secrecy(name), "{name} has no forward secrecy");
        }
        assert!(lacks_forward_secrecy("TLS_RSA_WITH_3DES_EDE_CBC_SHA"));
        assert!(!lacks_forward_secrecy("TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA"));
        assert!(!lacks_forward_secrecy("TLS_DH_anon_WITH_AES_128_CBC_SHA"));
        assert!(!is_insecure("TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"));
    }

    #[test]
    fn code_points_are_unique_and_named() {
        let mut ids: Vec<u16> = INSECURE_SUITES
            .iter()
            .chain(STATIC_RSA_SUITES.iter())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), INSECURE_SUITES.len() + STATIC_RSA_SUITES.len());
        assert_eq!(suite_name(0x000A), Some("TLS_RSA_WITH_3DES_EDE_CBC_SHA"));
        assert_eq!(suite_name(0xC02F), None, "modern suites are never probed");
    }

    #[test]
    fn less_common_null_anonymous_export_des_and_3des_suites_are_offered() {
        // Anonymous (incl. AES-GCM/CBC-SHA256, CAMELLIA, SEED), DHE_DSS DES/3DES/EXPORT and static-ECDH
        // NULL/RC4/3DES: a server enabling aNULL/eNULL may accept only these.
        for id in [
            0x00A7u16, 0x006C, 0x006D, 0x001A, 0x0017, 0x0019, 0xC015, 0xC017, 0x0046, 0x0089, 0x009B, 0x00BF, 0x00C5,
            0x0011, 0x0012, 0x0013, 0xC001, 0xC002, 0xC003, 0xC00B, 0xC00C, 0xC00D,
        ] {
            let offered = INSECURE_SUITES.iter().find(|(code, _)| *code == id);
            let (_, name) = offered.unwrap_or_else(|| panic!("0x{id:04X} is never offered"));
            assert!(is_insecure(name), "{name} must be insecure");
        }
    }

    #[test]
    fn accepted_suites_are_grouped_with_their_protocols() {
        let accepted = [(0x0303, 0xC012), (0x0303, 0x000A), (0x0301, 0x000A), (0x0303, 0x009C)];
        assert_eq!(
            describe_accepted(&accepted, is_insecure),
            vec![
                "TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.2)".to_string(),
                "TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2)".to_string(),
            ]
        );
        assert_eq!(
            describe_accepted(&accepted, lacks_forward_secrecy),
            vec![
                "TLS_RSA_WITH_3DES_EDE_CBC_SHA (TLSv1.0, TLSv1.2)".to_string(),
                "TLS_RSA_WITH_AES_128_GCM_SHA256 (TLSv1.2)".to_string(),
            ]
        );
    }
}
