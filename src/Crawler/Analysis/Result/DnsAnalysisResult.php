<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

class DnsAnalysisResult
{

    /**
     * @var string
     */
    public readonly string $dnsServerName;

    /**
     * @var string
     */
    public readonly string $dnsServerIpAddress;

    /**
     * DNS resolved domain names (aliases) with all CNAMEs. First is the original domain name and last is the final
     * resolved domain name for the final IP address(es).
     * @var string[]
     */
    public readonly array $resolvedDomains;

    /**
     * Final resolved IPv4 addresses
     * @var string[]
     */
    public readonly array $ipv4Addresses;

    /**
     * Final resolved IPv6 addresses (when available)
     * @var string[]
     */
    public readonly array $ipv6Addresses;

    /**
     * @param string $dnsServerName
     * @param string $dnsServerIpAddress
     * @param string[] $resolvedDomains
     * @param string[] $ipv4Addresses
     * @param string[] $ipv6Addresses
     */
    public function __construct(string $dnsServerName, string $dnsServerIpAddress, array $resolvedDomains, array $ipv4Addresses, array $ipv6Addresses)
    {
        $this->dnsServerName = $dnsServerName;
        $this->dnsServerIpAddress = $dnsServerIpAddress;
        $this->resolvedDomains = $resolvedDomains;
        $this->ipv4Addresses = $ipv4Addresses;
        $this->ipv6Addresses = $ipv6Addresses;
    }

    /**
     * Get text description of DNS analysis result in format respecting the hierarchy of resolved domains/CNAMEs and IPs
     * Example:
     * www.siteone.io
     *   cname1.siteone.io
     *     cname2.siteone.io
     *       IPv4: 1.2.3.4
     *       IPv4: 5.6.7.8
     *       IPv6: 2001:0db8:85a3:0000:0000:8a2e:0370:7334
     *
     * DNS server: mydns.server.com (7.8.9.10)
     *
     * @return string
     */
    public function getTxtDescription(): string
    {
        $result = '';
        for ($i = 0; $i < count($this->resolvedDomains); $i++) {
            $result .= str_repeat('  ', $i) . $this->resolvedDomains[$i] . "\n";
        }

        foreach ($this->ipv4Addresses as $ip) {
            $result .= str_repeat('  ', count($this->resolvedDomains)) . "IPv4: {$ip}\n";
        }
        foreach ($this->ipv6Addresses as $ip) {
            $result .= str_repeat('  ', count($this->resolvedDomains)) . "IPv6: {$ip}\n";
        }

        // Add DNS server info if available (0.0.0.0 means unknown, typical for CYGWIN)
        if ($this->dnsServerIpAddress !== '0.0.0.0') {
            if ($this->dnsServerName !== $this->dnsServerIpAddress) {
                $result .= "\nDNS server: {$this->dnsServerName} ({$this->dnsServerIpAddress})\n";
            } else {
                $result .= "\nDNS server: {$this->dnsServerName}\n";
            }
        }

        return trim($result);
    }

}