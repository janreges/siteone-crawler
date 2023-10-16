<?php

namespace Crawler;

class FoundUrls
{

    /**
     * @var FoundUrl[]
     */
    private array $foundUrls = [];

    public function addUrl(FoundUrl $foundUrl): void
    {
        $this->foundUrls[md5($foundUrl->url)] = $foundUrl;
    }

    public function addUrlsFromTextArray(array $urls, string $sourceUrl, string $source): void
    {
        foreach ($urls as $url) {
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

}