<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\Crawler;
use Crawler\FoundUrl;
use Crawler\FoundUrls;
use Crawler\ParsedUrl;
use Crawler\Utils;

class XmlProcessor extends BaseProcessor implements ContentProcessor
{
    protected array $relevantContentTypes = [
        Crawler::CONTENT_TYPE_ID_XML
    ];

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        if ($this->isSitemapXmlIndex($content)) {
            $foundUrls = new FoundUrls();
            $urls = $this->getUrlsFromSitemapXmlIndex($content);

            foreach ($urls as $url) {
                $foundUrls->addUrl(new FoundUrl($url, $sourceUrl->getFullUrl(), FoundUrl::SOURCE_SITEMAP));
            }

            return $foundUrls;
        } else if ($this->isSitemapXml($content)) {
            $foundUrls = new FoundUrls();
            $urls = $this->getUrlsFromSitemapXml($content);

            foreach ($urls as $url) {
                $foundUrls->addUrl(new FoundUrl($url, $sourceUrl->getFullUrl(), FoundUrl::SOURCE_SITEMAP));
            }

            return $foundUrls;
        }

        return null;
    }

    private function isSitemapXmlIndex(string $content): bool
    {
        return stripos($content, '<sitemapindex') !== false;
    }

    private function isSitemapXml(string $content): bool
    {
        return stripos($content, '<urlset') !== false;
    }

    /**
     * @param string $content
     * @return string[]
     */
    private function getUrlsFromSitemapXml(string $content): array
    {
        $xml = @simplexml_load_string($content);
        if ($xml === false) {
            $this->crawler->getOutput()->addError('Invalid XML in sitemap. Skipping.');
            return [];
        }

        $urls = [];
        foreach ($xml->url as $url) {
            $urls[] = (string)$url->loc;
        }

        return $urls;
    }

    /**
     * @param string $content
     * @return string[]
     */
    private function getUrlsFromSitemapXmlIndex(string $content): array
    {
        $xml = @simplexml_load_string($content);
        if ($xml === false) {
            $this->crawler->getOutput()->addError('Invalid XML in sitemap index. Skipping.');
            return [];
        }

        $urls = [];
        foreach ($xml->sitemap as $sitemap) {
            if (isset($sitemap->loc)) {
                if (str_ends_with((string)$sitemap->loc, '.xml')) {
                    $urls[] = (string)$sitemap->loc;
                } else {
                    $this->crawler->getOutput()->addNotice('Sitemap index contains non-XML or compressed URL: ' . $sitemap->loc . '. Skipping.');
                }
            }
        }

        return $urls;
    }

}