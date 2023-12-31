<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

use Crawler\Utils;
use DOMDocument;
use DOMNode;
use DOMNodeList;

class SeoAndOpenGraphResult
{

    const ROBOTS_INDEX = 1;
    const ROBOTS_NOINDEX = 0;
    const ROBOTS_FOLLOW = 1;
    const ROBOTS_NOFOLLOW = 2;

    public readonly string $urlUqId;
    public readonly string $urlPathAndQuery;

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
     * @param string $urlPathAndQuery
     */
    public function __construct(string $urlUqId, string $urlPathAndQuery)
    {
        $this->urlUqId = $urlUqId;
        $this->urlPathAndQuery = $urlPathAndQuery;
    }

    /**
     * @param string $urlUqId
     * @param string $urlPathAndQuery
     * @param DOMDocument $dom
     * @param string|null $robotsTxtContent
     * @param int $maxHeadingLevel
     * @return SeoAndOpenGraphResult
     */
    public static function getFromHtml(string $urlUqId, string $urlPathAndQuery, DOMDocument $dom, ?string $robotsTxtContent, int $maxHeadingLevel): SeoAndOpenGraphResult
    {
        $result = new SeoAndOpenGraphResult($urlUqId, $urlPathAndQuery);

        $metaTags = $dom->getElementsByTagName('meta');

        $result->title = trim(self::getTitle($dom) ?: '');
        $result->description = trim(self::getMetaTagContent($metaTags, 'description') ?: '');
        $result->keywords = trim(self::getMetaTagContent($metaTags, 'keywords') ?: '');

        $h1Content = self::getH1($dom);
        // strip tags from h1 content if it contains html tags
        if ($h1Content && str_contains($h1Content, "<")) {
            $h1Content = strip_tags(Utils::stripJavaScript($h1Content));
        }
        $result->h1 = $h1Content ? (trim($h1Content) != '' ? trim(preg_replace('/\s+/', ' ', $h1Content)) : '') : null;

        $result->robotsIndex = self::getRobotsIndex($metaTags);
        $result->robotsFollow = self::getRobotsFollow($metaTags);
        if ($robotsTxtContent) {
            $result->deniedByRobotsTxt = self::isDeniedByRobotsTxt($urlPathAndQuery, $robotsTxtContent);
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
        $result->headingsCount = self::getHeadingsCount($result->headingTreeItems);
        $result->headingsErrorsCount = self::getHeadingsWithErrorCount($result->headingTreeItems);

        return $result;
    }

    /**
     * @param HeadingTreeItem[] $headingTreeItems
     * @return int
     */
    private static function getHeadingsCount(array $headingTreeItems): int
    {
        $count = 0;
        foreach ($headingTreeItems as $headingTreeItem) {
            $count++;
            $count += self::getHeadingsCount($headingTreeItem->children);
        }
        return $count;
    }

    /**
     * @param HeadingTreeItem[] $headingTreeItems
     * @return int
     */
    private static function getHeadingsWithErrorCount(array $headingTreeItems): int
    {
        $count = 0;
        foreach ($headingTreeItems as $headingTreeItem) {
            if ($headingTreeItem->hasError()) {
                $count++;
            }
            $count += self::getHeadingsWithErrorCount($headingTreeItem->children);
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
            /* @var $h1 DOMNode */
            // WARNING: textContent is not working properly in cases, where website uses other HTML elements inside H1,
            // including <script> so JS code is included in the textContent
            // return $h1->textContent;
            return strip_tags(Utils::stripJavaScript($h1->ownerDocument->saveHTML($h1)));
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

    private static function isDeniedByRobotsTxt(string $urlPathAndQuery, string $robotsTxtContent): bool
    {
        if (!$robotsTxtContent) {
            return false;
        }

        // remove query string from URL
        $urlPath = preg_replace('/\?.*/', '', $urlPathAndQuery);

        // remove scheme and host from URL
        if (str_contains($urlPath, '://')) {
            $urlPath = preg_replace('/^https?:\/\/[^\/]+/', '', $urlPath);
        }

        $lines = explode("\n", $robotsTxtContent);
        foreach ($lines as $line) {
            $line = trim($line);
            if (str_starts_with($line, 'Disallow:')) {
                $disallowedPath = trim(substr($line, strlen('Disallow:')));
                if ($disallowedPath !== '' && str_starts_with($urlPath, $disallowedPath)) {
                    return true;
                }
            }
        }
        return false;
    }

}