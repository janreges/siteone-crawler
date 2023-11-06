<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\HeadingTreeItem;
use Crawler\Analysis\Result\SeoAndSocialResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;

class SeoAndSocialAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_SEO = 'seo';
    const SUPER_TABLE_SHARING = 'sharing';
    const SUPER_TABLE_SEO_HEADINGS = 'seo-headings';

    protected int $maxHeadingLevel = 3;

    private bool $hasOgTags = false;
    private bool $hasTwitterTags = false;

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $htmlUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->statusCode === 200 && !$visitedUrl->isExternal && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML;
        });

        $urlResults = $this->getSeoAndSocialResults($htmlUrls);

        // check if there are any OG or Twitter tags
        foreach ($urlResults as $urlResult) {
            if ($this->hasOgTags && $this->hasTwitterTags) {
                break;
            }

            if ($urlResult->ogTitle !== null || $urlResult->ogDescription !== null || $urlResult->ogImage !== null) {
                $this->hasOgTags = true;
            }
            if ($urlResult->twitterCard !== null || $urlResult->twitterTitle !== null || $urlResult->twitterDescription !== null || $urlResult->twitterImage !== null) {
                $this->hasTwitterTags = true;
            }
        }

        $s = microtime(true);
        $this->analyzeSeo($urlResults);
        $this->measureExecTime(__CLASS__, 'analyzeSeo', $s);

        $s = microtime(true);
        $this->analyzeSocials($urlResults);
        $this->measureExecTime(__CLASS__, 'analyzeSocials', $s);

        $s = microtime(true);
        $this->analyzeHeadings($urlResults);
        $this->measureExecTime(__CLASS__, 'analyzeHeadings', $s);
    }

    /**
     * @param VisitedUrl[] $htmlUrls
     * @return SeoAndSocialResult[]
     */
    private function getSeoAndSocialResults(array $htmlUrls): array
    {
        $results = [];
        $robotsTxtContent = Status::getRobotsTxtContent();
        foreach ($htmlUrls as $visitedUrl) {
            $htmlBody = $this->status->getStorage()->load($visitedUrl->uqId);

            $dom = new DOMDocument();
            @$dom->loadHTML($htmlBody);
            $urlPath = parse_url($visitedUrl->url, PHP_URL_PATH);

            $urlResult = SeoAndSocialResult::getFromHtml($visitedUrl->uqId, $urlPath, $dom, $robotsTxtContent, $this->maxHeadingLevel);
            $results[] = $urlResult;
        }
        return $results;
    }

    /**
     * @param SeoAndSocialResult[] $urlResults
     * @return void
     */
    private function analyzeSeo(array $urlResults): void
    {
        $superTable = new SuperTable(
            self::SUPER_TABLE_SEO,
            "SEO meta",
            "No URLs.",
            [
                new SuperTableColumn('urlPath', 'URL', 50, null, null, true),
                new SuperTableColumn('indexing', 'Indexing', 17, null, function (SeoAndSocialResult $urlResult) {
                    if ($urlResult->deniedByRobotsTxt) {
                        return Utils::getColorText('DENY (robots.txt)', 'magenta');
                    } elseif ($urlResult->robotsIndex === SeoAndSocialResult::ROBOTS_NOINDEX) {
                        return Utils::getColorText('DENY (meta)', 'magenta');
                    } else {
                        return 'Allowed';
                    }
                }, false, false),
                new SuperTableColumn('title', 'Title', 30, null, null, true),
                new SuperTableColumn('h1', 'H1', 30, function ($value) {
                    if (!$value) {
                        return Utils::getColorText('Missing H1', 'red', true);
                    }
                    return $value;
                }, null, true, false),
                new SuperTableColumn('description', 'Description', 30, null, null, true),
                new SuperTableColumn('keywords', 'Keywords', 30, null, null, true),
            ], true, 'urlPath', 'ASC'
        );

        $superTable->setData($urlResults);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @param SeoAndSocialResult[] $urlResults
     * @return void
     */
    private function analyzeSocials(array $urlResults): void
    {
        $columns = [
            new SuperTableColumn('urlPath', 'URL', 50, null, null, true),
        ];

        if ($this->hasOgTags) {
            $columns[] = new SuperTableColumn('ogTitle', 'OG Title', 32, null, null, true);
            $columns[] = new SuperTableColumn('ogDescription', 'OG Description', 32, null, null, true);
            $columns[] = new SuperTableColumn('ogImage', 'OG Image', 18, null, null, true);
        }
        if ($this->hasTwitterTags) {
            $columns[] = new SuperTableColumn('twitterCard', 'Twitter Card', 18, null, null, true);
            $columns[] = new SuperTableColumn('twitterTitle', 'Twitter Title', 32, null, null, true);
            $columns[] = new SuperTableColumn('twitterDescription', 'Twitter Description', 32, null, null, true);
            $columns[] = new SuperTableColumn('twitterImage', 'Twitter Image', 18, null, null, true);
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_SHARING,
            "Sharing metadata",
            "No URLs with OG or Twitter tags.",
            $columns,
            true,
            'urlPath',
            'ASC'
        );

        $superTableData = [];
        if ($this->hasOgTags || $this->hasTwitterTags) {
            $superTableData = $urlResults;
        }

        $superTable->setData($superTableData);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @param SeoAndSocialResult[] $urlResults
     * @return void
     */
    private function analyzeHeadings(array $urlResults): void
    {
        $superTable = new SuperTable(
            self::SUPER_TABLE_SEO_HEADINGS,
            "SEO headings structure",
            "No URLs.",
            [
                new SuperTableColumn('urlPath', 'URL', 50, null, null, true),
                new SuperTableColumn('headingsCount', 'Count', 5),
                new SuperTableColumn('headingsErrorsCount', 'Errors', 6, function ($value) {
                    if ($value > 0) {
                        return Utils::getColorText(strval($value), 'red', true);
                    }
                    return $value;
                }, null, false, false),
                new SuperTableColumn('headings', 'Headings structure', 80, null, function (SeoAndSocialResult $urlResult, string $renderInfo) {
                    if (!$urlResult->headingTreeItems) {
                        return '';
                    }
                    if ($renderInfo === SuperTable::RENDER_INTO_CONSOLE) {
                        return HeadingTreeItem::getHeadingTreeTxtList($urlResult->headingTreeItems);
                    } else {
                        return HeadingTreeItem::getHeadingTreeUlLiList($urlResult->headingTreeItems);
                    }
                }, true, false),
            ], true, 'urlPath', 'ASC'
        );

        $superTable->setData($urlResults);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public static function getOptions(): Options
    {
        return new Options();
    }

    public function getOrder(): int
    {
        return 113;
    }

}