// SiteOne Crawler - DnsAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::dns_analysis_result::DnsAnalysisResult;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const SUPER_TABLE_DNS: &str = "dns";

pub struct DnsAnalyzer {
    base: BaseAnalyzer,
}

impl Default for DnsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }

    /// Resolve DNS for the given domain using hickory-resolver.
    fn get_dns_info(&self, domain: &str) -> Result<DnsAnalysisResult, String> {
        use hickory_resolver::Resolver;
        use hickory_resolver::proto::rr::RecordType;

        let domain_owned = domain.to_string();

        // Use block_in_place to allow blocking the current thread while running async DNS lookups
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resolver = Resolver::builder_tokio()
                    .map_err(|e| format!("Failed to create DNS resolver: {}", e))?
                    .build();

                let mut resolved_domains = vec![domain_owned.clone()];
                let mut ipv4_addresses = Vec::new();
                let mut ipv6_addresses = Vec::new();

                // Resolve CNAME records
                if let Ok(cname_response) = resolver.lookup(domain_owned.as_str(), RecordType::CNAME).await {
                    for record in cname_response.iter() {
                        let cname_str = record.to_string().trim_end_matches('.').to_string();
                        if !resolved_domains.contains(&cname_str) {
                            resolved_domains.push(cname_str);
                        }
                    }
                }

                // Resolve A records (IPv4)
                if let Ok(ipv4_response) = resolver.lookup(domain_owned.as_str(), RecordType::A).await {
                    for record in ipv4_response.iter() {
                        let ip_str = record.to_string();
                        if !ip_str.is_empty() {
                            ipv4_addresses.push(ip_str);
                        }
                    }
                }

                // Resolve AAAA records (IPv6)
                if let Ok(ipv6_response) = resolver.lookup(domain_owned.as_str(), RecordType::AAAA).await {
                    for record in ipv6_response.iter() {
                        let ip_str = record.to_string();
                        if !ip_str.is_empty() {
                            ipv6_addresses.push(ip_str);
                        }
                    }
                }

                if ipv4_addresses.is_empty() && ipv6_addresses.is_empty() {
                    return Err(format!("Unable to resolve DNS records for {}", domain_owned));
                }

                let dns_server_ip = Self::get_system_dns_server().unwrap_or_else(|| "0.0.0.0".to_string());
                let dns_server_name = dns_server_ip.clone();

                Ok(DnsAnalysisResult::new(
                    dns_server_name,
                    dns_server_ip,
                    resolved_domains,
                    ipv4_addresses,
                    ipv6_addresses,
                ))
            })
        })
    }

    /// Read the first nameserver entry from /etc/resolv.conf to get the system DNS server IP.
    fn get_system_dns_server() -> Option<String> {
        let contents = std::fs::read_to_string("/etc/resolv.conf").ok()?;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("nameserver")
                && let Some(ip) = trimmed.split_whitespace().nth(1)
            {
                return Some(ip.to_string());
            }
        }
        None
    }
}

impl Analyzer for DnsAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let columns = vec![SuperTableColumn::new(
            "info".to_string(),
            "DNS resolving tree".to_string(),
            70,
            Some(Box::new(|value: &str, _render_into: &str| {
                let mut result = value.to_string();
                // Colorize IPv4 addresses
                if let Ok(re) = regex::Regex::new(r"(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})") {
                    result = re
                        .replace_all(&result, |caps: &regex::Captures| {
                            let ip = &caps[1];
                            if ip.parse::<std::net::Ipv4Addr>().is_ok() {
                                utils::get_color_text(ip, "blue", true)
                            } else {
                                ip.to_string()
                            }
                        })
                        .to_string();
                }
                // Colorize IPv6 addresses
                if let Ok(re) = regex::Regex::new(r"([0-9a-f:]+:+)+[0-9a-f]+") {
                    result = re
                        .replace_all(&result, |caps: &regex::Captures| {
                            let ip = &caps[0];
                            if ip.parse::<std::net::Ipv6Addr>().is_ok() {
                                utils::get_color_text(ip, "blue", true)
                            } else {
                                ip.to_string()
                            }
                        })
                        .to_string();
                }
                result
            })),
            None,
            true,
            false,
            true,
            false,
            None,
        )];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_DNS.to_string(),
            "DNS info".to_string(),
            "No DNS info found.".to_string(),
            columns,
            false,
            None,
            "ASC".to_string(),
            None,
            None,
            None,
        );

        let mut data: Vec<HashMap<String, String>> = Vec::new();

        // Extract domain from the first visited URL
        let domain = status
            .get_visited_urls()
            .first()
            .and_then(|u| u.get_host())
            .unwrap_or_else(|| "unknown".to_string());

        match self.get_dns_info(&domain) {
            Ok(dns_info) => {
                for line in dns_info.get_txt_description().lines() {
                    let mut row = HashMap::new();
                    row.insert("info".to_string(), line.to_string());
                    data.push(row);
                }

                let resolved_domain = dns_info
                    .resolved_domains
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                // DNS server suffix — omit when unknown (e.g. on Windows where /etc/resolv.conf doesn't exist)
                let dns_suffix = if dns_info.dns_server_ip_address != "0.0.0.0" {
                    format!(" (DNS server: {})", dns_info.dns_server_name)
                } else {
                    String::new()
                };

                // IPv4 summary
                if !dns_info.ipv4_addresses.is_empty() {
                    status.add_ok_to_summary(
                        "dns-ipv4",
                        &format!(
                            "DNS IPv4 OK: domain {} resolved to {}{}",
                            resolved_domain,
                            dns_info.ipv4_addresses.join(", "),
                            dns_suffix
                        ),
                    );
                } else {
                    status.add_notice_to_summary(
                        "dns-ipv4",
                        &format!(
                            "DNS IPv4: domain {} does not support IPv4{}",
                            resolved_domain, dns_suffix
                        ),
                    );
                }

                // IPv6 summary
                if !dns_info.ipv6_addresses.is_empty() {
                    status.add_ok_to_summary(
                        "dns-ipv6",
                        &format!(
                            "DNS IPv6 OK: domain {} resolved to {}{}",
                            resolved_domain,
                            dns_info.ipv6_addresses.join(", "),
                            dns_suffix
                        ),
                    );
                } else {
                    status.add_notice_to_summary(
                        "dns-ipv6",
                        &format!(
                            "DNS IPv6: domain {} does not support IPv6{}",
                            resolved_domain, dns_suffix
                        ),
                    );
                }

                // CNAME chain summary
                if dns_info.resolved_domains.len() > 1 {
                    status.add_info_to_summary(
                        "dns-aliases",
                        &format!(
                            "DNS Aliases: IP(s) for domain {} were resolved by CNAME chain {}.",
                            resolved_domain,
                            dns_info.resolved_domains.join(" > ")
                        ),
                    );
                }
            }
            Err(e) => {
                let mut row = HashMap::new();
                row.insert("info".to_string(), e.clone());
                data.push(row);
                status.add_critical_to_summary("dns", &format!("Problem with DNS analysis: {}", e));
            }
        }

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        215
    }

    fn get_name(&self) -> &str {
        "DnsAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
