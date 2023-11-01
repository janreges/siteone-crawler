<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Export\Utils\OfflineUrlConverter;
use Crawler\ParsedUrl;
use PHPUnit\Framework\TestCase;

class OfflineUrlConverterTest extends TestCase
{
    /**
     * @dataProvider getOfflineBaseUrlDepthProvider
     */
    public function testGetOfflineBaseUrlDepthProvider(string $fullUrl, int $expected)
    {
        $result = OfflineUrlConverter::getOfflineBaseUrlDepth(ParsedUrl::parse($fullUrl));
        $this->assertEquals($expected, $result);
    }

    /**
     * @return array[]
     */
    public static function getOfflineBaseUrlDepthProvider(): array
    {
        return [
            // absolute path
            ['/', 0], # because /index.html
            ['/foo', 0], # because /foo.html
            ['/foo/', 1], # because /foo/index.html
            ['/foo/bar', 1], # because /foo/bar.html
            ['/foo/bar/', 2], # because /foo/bar/index.html
            ['/?param=1', 0], # because /index.queryMd5Hash.html
            ['/foo?param=1', 0], # because /foo.queryMd5Hash.html
            ['/foo/?param=1', 1], # because /foo/index.queryMd5Hash.html
            ['/foo/bar?param=1', 1], # because /foo/bar.queryMd5Hash.html
            ['/foo/bar/?param=1', 2], # because /foo/bar/index.queryMd5Hash.html
        ];
    }
}