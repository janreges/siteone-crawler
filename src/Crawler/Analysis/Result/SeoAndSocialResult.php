<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

use DOMDocument;
use DOMNodeList;

class SeoAndSocialResult
{

    const ROBOTS_INDEX = 1;
    const ROBOTS_NOINDEX = 0;
    const ROBOTS_FOLLOW = 1;
    const ROBOTS_NOFOLLOW = 2;

    public readonly string $urlUqId;
    public readonly string $urlPath;

    public ?string $title;
    public ?string $description;
    public ?string $keywords;
    public ?string $h1;

    public ?int $robotsIndex; // see self::ROBOTS_* constants
    public ?int $robotsFollow; // see self::ROBOTS_* constants
    public bool $deniedByRobotsTxt = false;

    public ?string $ogTitle = null;
    public ?string $ogType = null;
    public ?string $ogImage = null;
    public ?string $ogUrl = null;
    public ?string $ogDescription = null;
    public ?string $ogSiteName = null;

    public ?string $twitterCard = null;
    public ?string $twitterSite = null;
    public ?string $twitterCreator = null;
    public ?string $twitterTitle = null;
    public ?string $twitterDescription = null;
    public ?string $twitterImage = null;

    /**
     * @var HeadingTreeItem[]
     */
    public array $headingTreeItems = [];

    public int $headingsCount = 0;
    public int $headingsErrorsCount = 0;

    /**
     * @param string $urlUqId
     * @param string $urlPath
     */
    public function __construct(string $urlUqId, string $urlPath)
    {
        $this->urlUqId = $urlUqId;
        $this->urlPath = $urlPath;
    }

    /**
     * @param string $urlUqId
     * @param string $urlPath
     * @param DOMDocument $dom
     * @param string|null $robotsTxtContent
     * @param int $maxHeadingLevel
     * @return SeoAndSocialResult
     */
    public static function getFromHtml(string $urlUqId, string $urlPath, DOMDocument $dom, ?string $robotsTxtContent, int $maxHeadingLevel): SeoAndSocialResult
    {
        $result = new SeoAndSocialResult($urlUqId, $urlPath);

        $metaTags = $dom->getElementsByTagName('meta');

        $result->title = trim(self::getTitle($dom) ?: '');
        $result->description = trim(self::getMetaTagContent($metaTags, 'description') ?: '');
        $result->keywords = trim(self::getMetaTagContent($metaTags, 'keywords') ?: '');
        $result->h1 = trim(self::getH1($dom) ?: '');

        $result->robotsIndex = self::getRobotsIndex($metaTags);
        $result->robotsFollow = self::getRobotsFollow($metaTags);
        if ($robotsTxtContent) {
            $result->deniedByRobotsTxt = self::isDeniedByRobotsTxt($urlPath, $robotsTxtContent);
        }

        $result->ogTitle = self::getMetaTagContent($metaTags, 'og:title');
        $result->ogType = self::getMetaTagContent($metaTags, 'og:type');
        $result->ogImage = self::getMetaTagContent($metaTags, 'og:image');
        $result->ogUrl = self::getMetaTagContent($metaTags, 'og:url');
        $result->ogDescription = self::getMetaTagContent($metaTags, 'og:description');
        $result->ogSiteName = self::getMetaTagContent($metaTags, 'og:site_name');
        $result->twitterCard = self::getMetaTagContent($metaTags, 'twitter:card');
        $result->twitterSite = self::getMetaTagContent($metaTags, 'twitter:site');
        $result->twitterCreator = self::getMetaTagContent($metaTags, 'twitter:creator');
        $result->twitterTitle = self::getMetaTagContent($metaTags, 'twitter:title');
        $result->twitterDescription = self::getMetaTagContent($metaTags, 'twitter:description');
        $result->twitterImage = self::getMetaTagContent($metaTags, 'twitter:image');

        $result->headingTreeItems = HeadingTreeItem::getHeadingTreeFromHtml($dom, $maxHeadingLevel);
        $result->headingsCount = self::getHeadingsCount($result->headingTreeItems, false);
        $result->headingsErrorsCount = self::getHeadingsCount($result->headingTreeItems, true);


        return $result;
    }

    /**
     * @param HeadingTreeItem[] $headingTreeItems
     * @param bool $onlyErrors
     * @return int
     */
    private static function getHeadingsCount(array $headingTreeItems, bool $onlyErrors): int
    {
        $count = 0;
        foreach ($headingTreeItems as $headingTreeItem) {
            if (!$onlyErrors || $headingTreeItem->level !== $headingTreeItem->realLevel) {
                $count++;
            }
            $count += self::getHeadingsCount($headingTreeItem->children, $onlyErrors);
        }
        return $count;
    }

    private static function getTitle(DOMDocument $dom): ?string
    {
        $titles = $dom->getElementsByTagName('title');
        foreach ($titles as $title) {
            return trim($title->textContent);
        }
        return null;
    }

    private static function getMetaTagContent(DOMNodeList $metaTags, string $name): ?string
    {
        foreach ($metaTags as $metaTag) {
            if ($metaTag->getAttribute('name') === $name) {
                return $metaTag->getAttribute('content');
            } else if ($metaTag->getAttribute('property') === $name) {
                return $metaTag->getAttribute('content');
            }
        }
        return null;
    }

    private static function getH1(DOMDocument $dom): ?string
    {
        $h1s = $dom->getElementsByTagName('h1');
        foreach ($h1s as $h1) {
            return $h1->textContent;
        }
        return null;
    }

    private static function getRobotsIndex(DOMNodeList $metaTags): int
    {
        foreach ($metaTags as $metaTag) {
            if ($metaTag->getAttribute('name') === 'robots') {
                $content = $metaTag->getAttribute('content');
                if (str_contains($content, 'noindex')) {
                    return self::ROBOTS_NOINDEX;
                }
                return self::ROBOTS_INDEX;
            }
        }
        return self::ROBOTS_INDEX;
    }

    private static function getRobotsFollow(DOMNodeList $metaTags): int
    {
        foreach ($metaTags as $metaTag) {
            if ($metaTag->getAttribute('name') === 'robots') {
                $content = $metaTag->getAttribute('content');
                if (str_contains($content, 'nofollow')) {
                    return self::ROBOTS_NOFOLLOW;
                }
                return self::ROBOTS_FOLLOW;
            }
        }
        return self::ROBOTS_FOLLOW;
    }

    private static function isDeniedByRobotsTxt(string $urlPath, string $robotsTxtContent): bool
    {
        if (!$robotsTxtContent) {
            return false;
        }

        $lines = explode("\n", $robotsTxtContent);
        foreach ($lines as $line) {
            $line = trim($line);
            if (str_starts_with($line, 'Disallow:')) {
                $disallowedPath = trim(substr($line, strlen('Disallow:')));
                if (str_starts_with($urlPath, $disallowedPath)) {
                    return true;
                }
            }
        }
        return false;
    }

}