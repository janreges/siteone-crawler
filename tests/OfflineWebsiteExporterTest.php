<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Crawler;
use Crawler\HttpClient\HttpClient;
use Crawler\Info;
use Crawler\Initiator;
use Crawler\Output\TextOutput;
use Crawler\Result\Status;
use Crawler\Result\Storage\MemoryStorage;
use PHPUnit\Framework\TestCase;

define('BASE_DIR', dirname($_SERVER['PHP_SELF'], 3));

class OfflineWebsiteExporterTest extends TestCase
{
    protected \Crawler\ContentProcessor\HtmlProcessor $processor;

    /**
     * @throws \Exception
     */
    protected function setUp(): void
    {
        $initiator = new Initiator([
            '--url=https://siteone.io/',
            '--allowed-domain-for-external-files=cdn.siteone.io',
            '--allowed-domain-for-external-files=cdn.webflow.com',
            '--allowed-domain-for-external-files=nextjs.org',
            '--allowed-domain-for-crawling=svelte.dev',
            '--allowed-domain-for-crawling=nextjs.org',
        ], dirname(__DIR__) . '/src/Crawler');
        $initiator->validateAndInit();

        $storage = new MemoryStorage(true);
        $coreOptions = $initiator->getCoreOptions();
        $status = new Status($storage, false, new Info('-', '-', '-', '-', '-', '-'), $coreOptions, microtime(true));
        $output = new TextOutput(
            '1.0.0',
            $status,
            $coreOptions,
            true
        );

        $crawler = new Crawler(
            $coreOptions,
            new HttpClient(null, null, 'tmp/'),
            $output,
            $status
        );

        $this->processor = new \Crawler\ContentProcessor\HtmlProcessor($crawler);
    }

    /**
     * @dataProvider convertUrlToRelativeUrlProvider
     */
    public function testConvertUrlToRelative($baseUrl, $targetUrl, $expected, $attribute = null)
    {
        $result = $this->processor->convertUrlToRelative(\Crawler\ParsedUrl::parse($baseUrl), $targetUrl, $attribute);
        $this->assertEquals($expected, $result);
    }

    /**
     * @return array[]
     */
    public static function convertUrlToRelativeUrlProvider(): array
    {
        // generate a new set of tests for convertUrlToRelative() which will be more and better structured and for
        // different types of the situation (eg. different domains, different paths, different query strings, etc.)

        return [
            // Absolute URLs with different paths, query strings, and fragments
            ["https://siteone.io/", "https://siteone.io", "index.html"],
            ["https://siteone.io/", "https://siteone.io/", "index.html"],
            ["https://siteone.io/", "https://siteone.io/page", "page.html"],
            ["https://siteone.io", "https://siteone.io/page/", "page/index.html"],
            ["https://siteone.io/", "https://siteone.io/page?p=1", "page.cff19eeeeb.html"],
            ["https://siteone.io/t", "https://siteone.io/page?p=1", "page.cff19eeeeb.html"],
            ["https://siteone.io/", "https://nextjs.org/page?p=1", "_nextjs.org_/page.cff19eeeeb.html"],
            ["https://siteone.io/t/", "https://siteone.io/page#fragment", "../page.html#fragment"],
            ["https://siteone.io/t/2/", "https://nextjs.org/page/extra/#fragment", "../../_nextjs.org_/page/extra/index.html#fragment"],
            ["https://nextjs.org/z/3/", "https://svelte.dev/page?p=1#fragment", "../../../_svelte.dev_/page.cff19eeeeb.html#fragment"],
            ["https://siteone.io/", "https://siteone.io/page/?p=1#fragment", "page/index.cff19eeeeb.html#fragment"],
            ["https://siteone.io/path/", "https://siteone.io/path/page", "../path/page.html"],
            ["https://siteone.io/path/", "https://siteone.io/path/page/?p=1", "../path/page/index.cff19eeeeb.html"],
            ["https://siteone.io/path/", "https://siteone.io/file.css?p=1", "../file.cff19eeeeb.css"],

            // Relative URLs with different paths
            ["https://siteone.io/", "/page", "page.html"],
            ["https://siteone.io/", "/page/", "page/index.html"],
            ["https://siteone.io/", "page", "page.html"],
            ["https://siteone.io/", "page/", "page/index.html"],
            ["https://siteone.io/path/", "../page", "../page.html"],
            ["https://siteone.io/path/", "../page/", "../page/index.html"],
            ["https://siteone.io/path/", "../page?p=1", "../page.cff19eeeeb.html"],
            ["https://siteone.io/path/test/", "../../page/#fragment", "../../page/index.html#fragment"],
            ["https://siteone.io/path/", "../page?p=1#fragment", "../page.cff19eeeeb.html#fragment"],
            ["https://siteone.io/path/", "../style.css?p=1", "../style.cff19eeeeb.css"],

            // Absolute URLs from different domain
            ["https://siteone.io/", "https://nextjs.org/", "_nextjs.org_/index.html"],
            ["https://siteone.io/t", "https://svelte.dev/", "_svelte.dev_/index.html"],
            ["https://siteone.io/t/", "https://svelte.dev/x", "../_svelte.dev_/x.html"],
            ["https://siteone.io/t/", "https://svelte.dev/x/file.css", "../_svelte.dev_/x/file.css"],

            // Absolute backlink to initial domain and other domains
            ["https://nextjs.org/", "https://siteone.io/t/", "../t/index.html"],
            ["https://nextjs.org/subpage", "https://siteone.io/", "../index.html"],
            ["https://nextjs.org/subpage/", "https://siteone.io/a", "../../a.html"],
            ["https://nextjs.org/", "https://siteone.io/", "../index.html"],
            ["https://nextjs.org/", "https://svelte.dev/page", "../_svelte.dev_/page.html"],
            ["https://nextjs.org/subpage/", "https://svelte.dev/page/", "../../_svelte.dev_/page/index.html"],
            ["https://nextjs.org/", "/nextpage", "nextpage.html"],
            ["https://nextjs.org/next/", "/next/file.css?p=1", "../next/file.cff19eeeeb.css"],

            // Protocol-relative URLs
            ["https://siteone.io/", "//nextjs.org/", "_nextjs.org_/index.html"],
            ["https://nextjs.org/", "//siteone.io/page", "../page.html"],
            ["https://nextjs.org/", "//svelte.dev/page/", "../_svelte.dev_/page/index.html"],
            ["https://nextjs.org/", "//svelte.dev/file.js", "../_svelte.dev_/file.js"],

            // URLs with query string only
            ["https://siteone.io/", "?p=1", "index.cff19eeeeb.html"],
            ["https://siteone.io/sub/", "/?p=1", "../index.cff19eeeeb.html"],
            ["https://nextjs.org/a", "/?p=1#fragment", "index.cff19eeeeb.html#fragment"],
            ["https://nextjs.org/a/", "/b/?p=1#fragment", "../b/index.cff19eeeeb.html#fragment"],

            // URLs with fragment only
            ["https://siteone.io/", "#fragment2", "#fragment2"],
            ["https://nextjs.org/", "#fragment3", "#fragment3"],
            ["https://nextjs.org/test", "#fragment4", "#fragment4"],

            // Base URL with the query and target with different paths and queries
            ["https://siteone.io/?q=1", "https://siteone.io/page", "page.html"],
            ["https://siteone.io/?q=1", "/page/", "page/index.html"],
            ["https://siteone.io/a/?q=1", "page?p=1", "../a/page.cff19eeeeb.html"],
            ["https://siteone.io/b/?q=1", "/c/page#fragment", "../c/page.html#fragment"],
            ["https://siteone.io/b/?q=1", "/c/page/#fragment", "../c/page/index.html#fragment"],
            ["https://siteone.io/?q=1", "page?p=1#fragment", "page.cff19eeeeb.html#fragment"],

            // More complex relative URLs
            ["https://siteone.io/path/more/", "../../page", "../../page.html"],
            ["https://siteone.io/path/more/", "../../page/", "../../page/index.html"],
            ["https://siteone.io/path/more/", "../../page?p=1", "../../page.cff19eeeeb.html"],
            ["https://siteone.io/path/more/", "../../page#fragment", "../../page.html#fragment"],
            ["https://siteone.io/path/more/", "../../../page/?p=1#fragment", "../../../page/index.cff19eeeeb.html#fragment"],

            // Other special cases - external CSS
            ['https://cdn.siteone.io/siteone.io/css/styles.css', 'https://cdn.webflow.com/a/b1.jpg', '../../../_cdn.webflow.com_/a/b1.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css', 'https://cdn.webflow.com/b2.jpg', '../../../../../_cdn.webflow.com_/b2.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css', 'https://siteone.io/test/image.jpg', '../../../../../test/image.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/styles.css', '/abt.jpg', '../../abt.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/styles.css', '../abz.jpg', '../abz.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css', 'https://cdn.webflow.com/b2d.jpg', '../../../../../_cdn.webflow.com_/b2d.jpg'],
            ['https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css', 'https://cdn.webflow.com/slozka.test/b2d.jpg', '../../../../../_cdn.webflow.com_/slozka.test_/b2d.jpg'],

            // Other special cases - dynamic images with needed extension estimation
            ['https://nextjs.org/', 'https://nextjs.org/_next/image?url=%2F_next%2Fstatic%2Fmedia%2Fpreview-audible.6063405a.png&w=640&q=75&dpl=dpl_4C87ukg3PhFXfiHatxfw16hpDnFr', '_next/image.9580c6e093.png', 'src'],
            ['https://nextjs.org/', 'https://nextjs.org/_next/image?url=%2F_next%2Fstatic%2Fmedia%2Fpreview-audible.6063405a.png&w=640&q=75&dpl=dpl_4C87ukg3PhFXfiHatxfw16hpDnFr#test55', '_next/image.9580c6e093.png#test55', 'src'],
            ['https://nextjs.org/subpage/', 'https://nextjs.org/_next/image?url=%2F_next%2Fstatic%2Fmedia%2Fpreview-audible.6063405a.png&w=640&q=75&dpl=dpl_4C87ukg3PhFXfiHatxfw16hpDnFr#test66', '../_next/image.9580c6e093.png#test66', 'src'],

            // Unknown and not allowed domains
            ['https://siteone.io/', '//unknown.com', 'https://unknown.com/'],
            ['https://siteone.io/', '//unknown.com/', 'https://unknown.com/'],
            ['https://siteone.io/', 'http://unknown.com/page', 'http://unknown.com/page'],
            ['https://siteone.io/', 'https://unknown.com/', 'https://unknown.com/'],
        ];
    }
}