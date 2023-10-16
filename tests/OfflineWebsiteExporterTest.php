<?php

namespace tests;

use Crawler\Crawler;
use Crawler\Export\OfflineWebsiteExporter;
use Crawler\HttpClient\HttpClient;
use Crawler\Info;
use Crawler\Initiator;
use Crawler\Output\TextOutput;
use Crawler\Result\Status;
use Crawler\Result\Storage\MemoryStorage;
use PHPUnit\Framework\TestCase;

class OfflineWebsiteExporterTest extends TestCase
{
    protected OfflineWebsiteExporter $exporter;

    /**
     * @throws \Exception
     */
    protected function setUp(): void
    {
        $initiator = new Initiator([
            '--url=https://www.siteone.io/',
            '--allowed-domain-for-external-files=cdn.siteone.io',
            '--allowed-domain-for-external-files=cdn.webflow.com',
        ], dirname(__DIR__) . '/Crawler');
        $initiator->validateAndInit();

        $storage = new MemoryStorage(true);
        $coreOptions = $initiator->getCoreOptions();
        $status = new Status($storage, false, new Info('-', '-', '-', '-', '-', '-'), $coreOptions, microtime(true));
        $output = new TextOutput(
            '1.0.0',
            microtime(true),
            $status,
            $coreOptions,
            '-',
            true
        );

        $crawler = new Crawler(
            $coreOptions,
            new HttpClient('tmp/'),
            $output,
            $status
        );

        $this->exporter = new OfflineWebsiteExporter();
        $this->exporter->initialUrlHost = 'www.siteone.io';
        $this->exporter->setCrawler($crawler);
        $this->exporter->setStatus($status);
        $this->exporter->setOutput($output);
    }

    /**
     * @dataProvider convertUrlToRelativeUrlProvider
     */
    public function testConvertUrlToRelative($baseUrl, $targetUrl, $expected)
    {
        $result = $this->exporter->convertUrlToRelative($baseUrl, $targetUrl);
        $this->assertEquals($expected, $result);
    }

    /**
     * @return array[]
     */
    public static function convertUrlToRelativeUrlProvider(): array
    {
        return [
            // root
            ['https://www.siteone.io/', '/test', 'test/index.html'],
            ['https://www.siteone.io/', '/test/', 'test/index.html'],
            ['https://www.siteone.io/foo/bar', '/test.html', '../../test.html'],
            ['https://www.siteone.io/foo/bar', '/contact/test', '../../contact/test/index.html'],
            ['https://www.siteone.io/foo/bar/', '/contact/test', '../../contact/test/index.html'],
            ['https://www.siteone.io/foo/bar/', '//www.siteone.io/contact/test', '../../contact/test/index.html'],
            ['https://www.siteone.io/foo/bar/index.html', '//www.siteone.io/contact/test', '../../contact/test/index.html'],
            ['https://www.siteone.io/foo/bar/', '//www.siteone.io/contact/test#hello', '../../contact/test/index.html#hello'],
            ['https://www.siteone.io/foo/bar/hello/', 'https://www.siteone.io/contact/test', '../../../contact/test/index.html'],
            ['https://www.siteone.io/foo/bar/hello/', 'https://www.siteone.io/contact/test', '../../../contact/test/index.html'],
            ['https://www.siteone.io/foo', '/test/', '../test/index.html'],
            // relative paths
            ['https://www.siteone.io/foo/bar/', '../style.css', '../../style.css'],
            ['https://www.siteone.io/foo/bar/', '../style.css ', '../../style.css'],
            // external CSS
            ['https://cdn.siteone.io/www.siteone.io/css/styles.css', 'https://cdn.webflow.com/a/b.jpg', '../../../_cdn.webflow.com/a/b.jpg'],
            ['https://cdn.siteone.io/www.siteone.io/css/hello/hi/styles.css', 'https://cdn.webflow.com/b.jpg', '../../../../../_cdn.webflow.com/b.jpg'],
            // specials
            ['https://www.jakpsatweb.cz/html/rejstrik.html', 'rejstrik.html', 'rejstrik.html'],
            ['https://www.siteone.cz/', 'https://www.siteone.cz/case-study/investice-3-tisicileti-2', 'case-study/investice-3-tisicileti-2/index.html'],
        ];
    }
}