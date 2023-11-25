<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Export\Utils\TargetDomainRelation;
use Crawler\ParsedUrl;
use PHPUnit\Framework\TestCase;

class TargetDomainRelationTest extends TestCase
{
    /**
     * @dataProvider getByUrlProvider
     */
    public function testGetByUrl(string $initialUrl, string $baseUrl, string $targetUrl, TargetDomainRelation $expected)
    {
        $result = TargetDomainRelation::getByUrls(
            ParsedUrl::parse($initialUrl),
            ParsedUrl::parse($baseUrl),
            ParsedUrl::parse($targetUrl),
        );
        $this->assertEquals($expected, $result);
    }

    /**
     * @return array[]
     */
    public static function getByUrlProvider(): array
    {
        return [
            // INITIAL_SAME__BASE_SAME
            ['https://www.siteone.io/', 'https://www.siteone.io/', '/', TargetDomainRelation::INITIAL_SAME__BASE_SAME],
            ['https://www.siteone.io/', 'https://www.siteone.io/', 'https://www.siteone.io/', TargetDomainRelation::INITIAL_SAME__BASE_SAME],
            ['https://www.siteone.io/', 'https://www.siteone.io/', '//www.siteone.io/', TargetDomainRelation::INITIAL_SAME__BASE_SAME],

            // INITIAL_SAME__BASE_DIFFERENT (backlink)
            ['https://www.siteone.io/', 'https://nextjs.org/', 'https://www.siteone.io/', TargetDomainRelation::INITIAL_SAME__BASE_DIFFERENT],
            ['https://www.siteone.io/', 'https://nextjs.org/', '//www.siteone.io/', TargetDomainRelation::INITIAL_SAME__BASE_DIFFERENT],

            // INITIAL_DIFFERENT__BASE_SAME
            ['https://www.siteone.io/', 'https://nextjs.org/', '/', TargetDomainRelation::INITIAL_DIFFERENT__BASE_SAME],
            ['https://www.siteone.io/', 'https://nextjs.org/', 'https://nextjs.org/', TargetDomainRelation::INITIAL_DIFFERENT__BASE_SAME],
            ['https://www.siteone.io/', 'https://nextjs.org/', '//nextjs.org', TargetDomainRelation::INITIAL_DIFFERENT__BASE_SAME],

            // INITIAL_DIFFERENT__BASE_DIFFERENT
            ['https://www.siteone.io/', 'https://nextjs.org/', 'https://svelte.dev/', TargetDomainRelation::INITIAL_DIFFERENT__BASE_DIFFERENT],
            ['https://www.siteone.io/', 'https://nextjs.org/', '//svelte.dev', TargetDomainRelation::INITIAL_DIFFERENT__BASE_DIFFERENT],
            ['https://www.siteone.io/', 'https://www.siteone.io/', '//svelte.dev', TargetDomainRelation::INITIAL_DIFFERENT__BASE_DIFFERENT],
        ];
    }
}