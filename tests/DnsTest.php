<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Analysis\DnsAnalyzer;
use PHPUnit\Framework\TestCase;

class DnsTest extends TestCase
{
    /**
     * @dataProvider parseNslookupProvider
     */
    public function testParseNslookup(string $nslookup, $expected)
    {
        $result = DnsAnalyzer::parseNslookup($nslookup);
        $this->assertEquals($expected, $result->getTxtDescription());
    }

    /**
     * @return array[]
     */
    public static function parseNslookupProvider(): array
    {
        $expected1 = "www.siteone.io\n" .
            "  cname1.siteone.io\n" .
            "    cname2.siteone.io\n" .
            "      IPv4: 1.2.3.4\n" .
            "      IPv4: 5.6.7.8\n" .
            "\nDNS server: dns.siteone.io (10.10.10.10)";

        $expected2 = "www.siteone.io\n" .
            "  IPv4: 77.75.79.222\n" .
            "  IPv4: 77.75.77.222\n" .
            "  IPv6: 2a02:598:a::79:222\n" .
            "  IPv6: 2a02:598:2::1222\n" .
            "\nDNS server: dns.siteone.io (10.10.10.10)";

        return [
            ['Server:  dns.siteone.io
                Address:  10.10.10.10
                
                Non-authoritative answer:
                Name:    cname2.siteone.io
                Addresses:  1.2.3.4
                          5.6.7.8
                Aliases:  www.siteone.io
                          cname1.siteone.io',
                $expected1
            ],
            ['Server:         dns.siteone.io
                Address:        10.10.10.10#53
                
                Non-authoritative answer:
                www.siteone.io       canonical name = cname1.siteone.io.
                cname1.siteone.io   canonical name = cname2.siteone.io.
                Name:   cname2.siteone.io
                Address: 1.2.3.4
                Name:   cname2.siteone.io
                Address: 5.6.7.8',
                $expected1
            ],
            ['Server:  dns.siteone.io
                Address:  10.10.10.10

                Non-authoritative answer:
                Name:    www.siteone.io
                Addresses:  2a02:598:a::79:222
                          2a02:598:2::1222
                          77.75.79.222
                          77.75.77.222',
                $expected2
            ]
        ];
    }
}