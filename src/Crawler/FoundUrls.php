<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class FoundUrls
{

    /**
     * @var FoundUrl[]
     */
    private array $foundUrls = [];

    public function addUrl(FoundUrl $foundUrl): void
    {
        $key = md5($foundUrl->url);
        if (!isset($this->foundUrls[$key])) {
            $this->foundUrls[$key] = $foundUrl;
        }
    }

    public function addUrlsFromTextArray(array $urls, string $sourceUrl, int $source): void
    {
        foreach ($urls as $url) {
            if (!self::isUrlValidForCrawling($url)) {
                continue;
            }
            $this->addUrl(new FoundUrl($url, $sourceUrl, $source));
        }
    }

    /**
     * @return FoundUrl[]
     */
    public function getUrls(): array
    {
        return $this->foundUrls;
    }

    public function getCount(): int
    {
        return count($this->foundUrls);
    }

    /**
     * Check if URL is valid for crawling. Ignored are:
     *  - anchor #fragment links
     *  - data:, mailto:, javascript: and other non-http(s) links
     *  - file:// links
     *
     * @param string $url
     * @return bool
     */
    private static function isUrlValidForCrawling(string $url): bool
    {
        $url = trim($url);
        if (str_starts_with($url, '#') || preg_match('/^[a-z]+:[a-z0-9]/i', $url) === 1 || stripos($url, 'file://') === 0) {
            return false;
        }
        return true;
    }

}