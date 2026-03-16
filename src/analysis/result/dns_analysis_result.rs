// SiteOne Crawler - DnsAnalysisResult
// (c) Jan Reges <jan.reges@siteone.cz>

#[derive(Debug, Clone)]
pub struct DnsAnalysisResult {
    pub dns_server_name: String,
    pub dns_server_ip_address: String,
    /// DNS resolved domain names (aliases) with all CNAMEs.
    /// First is the original domain name and last is the final resolved domain name.
    pub resolved_domains: Vec<String>,
    /// Final resolved IPv4 addresses
    pub ipv4_addresses: Vec<String>,
    /// Final resolved IPv6 addresses (when available)
    pub ipv6_addresses: Vec<String>,
}

impl DnsAnalysisResult {
    pub fn new(
        dns_server_name: String,
        dns_server_ip_address: String,
        resolved_domains: Vec<String>,
        ipv4_addresses: Vec<String>,
        ipv6_addresses: Vec<String>,
    ) -> Self {
        Self {
            dns_server_name,
            dns_server_ip_address,
            resolved_domains,
            ipv4_addresses,
            ipv6_addresses,
        }
    }

    /// Get text description of DNS analysis result in format respecting the
    /// hierarchy of resolved domains/CNAMEs and IPs.
    pub fn get_txt_description(&self) -> String {
        let mut result = String::new();

        for (i, domain) in self.resolved_domains.iter().enumerate() {
            result.push_str(&"  ".repeat(i));
            result.push_str(domain);
            result.push('\n');
        }

        let indent = "  ".repeat(self.resolved_domains.len());
        for ip in &self.ipv4_addresses {
            result.push_str(&indent);
            result.push_str(&format!("IPv4: {}\n", ip));
        }
        for ip in &self.ipv6_addresses {
            result.push_str(&indent);
            result.push_str(&format!("IPv6: {}\n", ip));
        }

        // Add DNS server info if available (0.0.0.0 means unknown, typical for CYGWIN)
        if self.dns_server_ip_address != "0.0.0.0" {
            if self.dns_server_name != self.dns_server_ip_address {
                result.push_str(&format!(
                    "\nDNS server: {} ({})\n",
                    self.dns_server_name, self.dns_server_ip_address
                ));
            } else {
                result.push_str(&format!("\nDNS server: {}\n", self.dns_server_name));
            }
        }

        result.trim().to_string()
    }
}
