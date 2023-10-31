<?php

namespace Crawler\Analysis;

use Crawler\Analysis\Result\DnsAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Options;
use Exception;

class DnsAnalyzer extends BaseAnalyzer implements Analyzer
{
    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $superTable = new SuperTable(
            'dns',
            'DNS info',
            'No DNS info found.', [
            new SuperTableColumn('info', 'Info'),
        ], false);

        $data = [];
        try {
            $dnsInfo = $this->getDnsInfo();
            foreach (explode("\n", $dnsInfo->getTxtDescription()) as $line) {
                $data[] = ['info' => $line];
            }

            $domain = $dnsInfo->resolvedDomains[0];

            // IPv4
            if ($dnsInfo->ipv4Addresses) {
                $this->status->addOkToSummary('dns-ipv4', "DNS IPv4 OK: domain {$domain} resolved to " . implode(', ', $dnsInfo->ipv4Addresses) . " (DNS server: {$dnsInfo->dnsServerName})");
            } else {
                $this->status->addNoticeToSummary('dns-ipv4', "DNS IPv4: domain {$domain} does not support IPv4 (DNS server: {$dnsInfo->dnsServerName})");
            }

            // IPv6
            if ($dnsInfo->ipv6Addresses) {
                $this->status->addOkToSummary('dns-ipv6', "DNS IPv6 OK: domain {$domain} resolved to " . implode(', ', $dnsInfo->ipv6Addresses) . " (DNS server: {$dnsInfo->dnsServerName})");
            } else {
                $this->status->addNoticeToSummary('dns-ipv6', "DNS IPv6: domain {$domain} does not support IPv6 (DNS server: {$dnsInfo->dnsServerName})");
            }
        } catch (Exception $e) {
            $data[] = ['info' => $e->getMessage()];
            $this->status->addCriticalToSummary('dns', "Problem with DNS analysis: {$e->getMessage()}");
        }

        $superTable->setData($data);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @return DnsAnalysisResult
     * @throws Exception
     */
    private function getDnsInfo(): DnsAnalysisResult
    {
        $domain = parse_url($this->crawler->getCoreOptions()->url, PHP_URL_HOST);
        $nslookup = shell_exec("nslookup " . escapeshellarg($domain));

        if (!$nslookup) {
            throw new Exception(__METHOD__ . ': nslookup command failed.');
        }

        return self::parseNslookup($nslookup);
    }

    public function getOrder(): int
    {
        return 215;
    }

    /**
     * @param string $nslookupOutput
     * @return DnsAnalysisResult
     * @throws Exception
     */
    public static function parseNslookup(string $nslookupOutput): DnsAnalysisResult
    {
        $dnsServerName = null;
        $dnsServerIpAddress = null;
        $resolvedDomains = [];
        $ipv4Addresses = [];
        $ipv6Addresses = [];

        // Extract DNS Server and Address
        if (preg_match('/Server:\s*(\S+)/i', $nslookupOutput, $dnsServerMatches)) {
            $dnsServerName = $dnsServerMatches[1];
        }

        if (preg_match('/Address:\s*([0-9a-z.:]+)/i', $nslookupOutput, $dnsAddressMatches)) {
            $dnsServerIpAddress = $dnsAddressMatches[1];
        }

        if (!$dnsServerName || !$dnsServerIpAddress) {
            throw new Exception('DNS Server or Address not found in nslookup output.');
        }

        // Extract only the "Non-authoritative answer" part
        if (preg_match('/Non-authoritative answer:(.*)/is', $nslookupOutput, $answerMatches)) {
            $answerSection = $answerMatches[1];
        } else {
            $answerSection = $nslookupOutput; // fallback to the entire output if not found
        }

        // Extract IP Addresses from the answer section
        if (preg_match_all('/Address(?:es)?:\s*((?:\S+\s*)+)/is', $answerSection, $ipMatches)) {
            $targetIPs = array_map('trim', preg_split("/\s+/", trim($ipMatches[1][0])));
            foreach ($targetIPs as $ip) {
                if (filter_var($ip, FILTER_VALIDATE_IP, FILTER_FLAG_IPV4)) {
                    $ipv4Addresses[] = $ip;
                } else if (filter_var($ip, FILTER_VALIDATE_IP, FILTER_FLAG_IPV6)) {
                    $ipv6Addresses[] = $ip;
                }
            }
        }

        // Extract resolved domains from the answer section

        // Windows specific format
        if (preg_match_all('/Aliases:\s*((?:\S+\s*)+)/', $answerSection, $aliasMatchesWindows)) {
            $aliases = explode("\n", trim($aliasMatchesWindows[1][0]));
            foreach ($aliases as $alias) {
                $alias = trim($alias, "\r\n\t .");
                $resolvedDomains[$alias] = $alias;
            }
        }

        // Linux/macOS specific format
        if (preg_match_all('/(\S+)\s+canonical name\s*=\s*(\S+)/i', $answerSection, $aliasMatchesLinux)) {
            foreach ($aliasMatchesLinux[1] as $index => $from) {
                $to = trim($aliasMatchesLinux[2][$index], "\r\n\t .");
                $resolvedDomains[$from] = $from;
                $resolvedDomains[$to] = $to;
            }
        }

        if (preg_match_all('/\s*Name:\s*(\S+)/i', $answerSection, $nameMatches)) {
            foreach ($nameMatches[1] as $match) {
                $match = trim($match, "\r\n\t .");
                $resolvedDomains[$match] = $match;
            }
        }

        return new DnsAnalysisResult(
            $dnsServerName,
            $dnsServerIpAddress,
            array_unique(array_values($resolvedDomains)),
            $ipv4Addresses,
            $ipv6Addresses
        );
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}