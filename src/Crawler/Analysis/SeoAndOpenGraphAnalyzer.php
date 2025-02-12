<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\HeadingTreeItem;
use Crawler\Analysis\Result\SeoAndOpenGraphResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;

class SeoAndOpenGraphAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_SEO = 'seo';
    const SUPER_TABLE_OPEN_GRAPH = 'open-graph';
    const SUPER_TABLE_SEO_HEADINGS = 'seo-headings';

    const GROUP_SEO_AND_OPENGRAPH_ANALYZER = 'seo-and-opengraph-analyzer';

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
            return $visitedUrl->statusCode === 200 && $visitedUrl->isAllowedForCrawling && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML;
        });

        $urlResults = $this->getSeoAndOpenGraphResults($htmlUrls);

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
        $this->analyzeOpenGraph($urlResults);
        $this->measureExecTime(__CLASS__, 'analyzeOpenGraph', $s);

        $s = microtime(true);
        $this->analyzeHeadings($urlResults);

        $this->measureExecTime(__CLASS__, 'analyzeHeadings', $s);
    }

    /**
     * @param VisitedUrl[] $htmlUrls
     * @return SeoAndOpenGraphResult[]
     */
    private function getSeoAndOpenGraphResults(array $htmlUrls): array
    {
        $results = [];
        $initialScheme = $this->crawler->getInitialParsedUrl()->scheme;
        $initialHost = $this->crawler->getInitialParsedUrl()->host;
        foreach ($htmlUrls as $visitedUrl) {
            $htmlBody = $this->status->getStorage()->load($visitedUrl->uqId);
            $htmlBodyWithDecodedEntities = @mb_convert_encoding($htmlBody, 'HTML-ENTITIES', 'UTF-8');

            $dom = new DOMDocument();

            if ($htmlBodyWithDecodedEntities) {
                @$dom->loadHTML($htmlBodyWithDecodedEntities);
            } elseif ($htmlBody) {
                @$dom->loadHTML($htmlBody);
            } else {
                continue;
            }

            $urlPathAndQuery = Utils::getUrlWithoutSchemeAndHost($visitedUrl->url, $initialHost, $initialScheme);
            $robotsTxtContent = Status::getRobotsTxtContent($visitedUrl->getScheme(), $visitedUrl->getHost(), $visitedUrl->getPort());

            $urlResult = SeoAndOpenGraphResult::getFromHtml($visitedUrl->uqId, $urlPathAndQuery, $dom, $robotsTxtContent, $this->maxHeadingLevel);
            $results[] = $urlResult;
        }
        return $results;
    }

    /**
     * @param SeoAndOpenGraphResult[] $urlResults
     * @return void
     */
    private function analyzeSeo(array $urlResults): void
    {
        $consoleWidth = Utils::getConsoleWidth();
        $urlColWidth = 50;
        $indexingColWidth = 20;
        $commonColCount = 4;
        $spacesAndPipes = 6 * 3;
        $commonColWidth = intval(($consoleWidth - $urlColWidth - $indexingColWidth - $spacesAndPipes) / $commonColCount);

        $superTable = new SuperTable(
            self::SUPER_TABLE_SEO,
            "SEO metadata",
            "No URLs.",
            [
                new SuperTableColumn('urlPathAndQuery', 'URL', $urlColWidth, null, null, true),
                new SuperTableColumn('indexing', 'Indexing', $indexingColWidth, null, function (SeoAndOpenGraphResult $urlResult) {
                    if ($urlResult->deniedByRobotsTxt) {
                        return Utils::getColorText('DENY (robots.txt)', 'magenta');
                    } elseif ($urlResult->robotsIndex === SeoAndOpenGraphResult::ROBOTS_NOINDEX) {
                        return Utils::getColorText('DENY (meta)', 'magenta');
                    } else {
                        return 'Allowed';
                    }
                }, false, false),
                new SuperTableColumn('title', 'Title', $commonColWidth, null, null, true),
                new SuperTableColumn('h1', 'H1', $commonColWidth, function ($value) {
                    if (!$value) {
                        return Utils::getColorText('Missing H1', 'red', true);
                    }
                    return $value;
                }, null, true, false),
                new SuperTableColumn('description', 'Description', $commonColWidth, null, null, true),
                new SuperTableColumn('keywords', 'Keywords', $commonColWidth, null, null, true),
            ], true, 'urlPathAndQuery', 'ASC'
        );

        // hide in console because of too many columns and long values
        $superTable->setVisibilityInConsole(true, 10);

        // set initial URL (required for urlPath column and active link building)
        $superTable->setInitialUrl($this->crawler->getInitialParsedUrl()->url);

        $superTable->setData($urlResults);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @param SeoAndOpenGraphResult[] $urlResults
     * @return void
     */
    private function analyzeOpenGraph(array $urlResults): void
    {
        $consoleWidth = Utils::getConsoleWidth();
        $urlColWidth = 50;
        $imageColWidth = 18;
        $imageColCount = ($this->hasOgTags ? 1 : 0) + ($this->hasTwitterTags ? 1 : 0);
        $commonColCount = ($this->hasOgTags ? 2 : 0) + ($this->hasTwitterTags ? 2 : 0);
        $spacesAndPipes = (1 + $imageColCount + $commonColCount) * 3;
        $commonColWidth = intval(($consoleWidth - $urlColWidth - ($imageColCount * $imageColWidth) - $spacesAndPipes) / max(1, $commonColCount));

        $columns = [
            new SuperTableColumn('urlPathAndQuery', 'URL', $urlColWidth, null, null, true),
        ];

        if ($this->hasOgTags) {
            $columns[] = new SuperTableColumn('ogTitle', 'OG Title', $commonColWidth, null, null, true);
            $columns[] = new SuperTableColumn('ogDescription', 'OG Description', $commonColWidth, null, null, true);
            $columns[] = new SuperTableColumn('ogImage', 'OG Image', $imageColWidth, null, null, true);
        }
        if ($this->hasTwitterTags) {
            $columns[] = new SuperTableColumn('twitterTitle', 'Twitter Title', $commonColWidth, null, null, true);
            $columns[] = new SuperTableColumn('twitterDescription', 'Twitter Description', $commonColWidth, null, null, true);
            $columns[] = new SuperTableColumn('twitterImage', 'Twitter Image', $imageColWidth, null, null, true);
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_OPEN_GRAPH,
            "OpenGraph metadata",
            "No URLs with OpenGraph data (og:* or twitter:* meta tags).",
            $columns,
            true,
            'urlPathAndQuery',
            'ASC'
        );

        // hide in console because of too many columns and long values
        $superTable->setVisibilityInConsole(true, 10);

        $superTableData = [];
        if ($this->hasOgTags || $this->hasTwitterTags) {
            $superTableData = $urlResults;
        }

        // set initial URL (required for urlPath column and active link building)
        $superTable->setInitialUrl($this->status->getOptions()->url);

        $superTable->setData($superTableData);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @param SeoAndOpenGraphResult[] $urlResults
     * @return void
     */
    private function analyzeHeadings(array $urlResults): void
    {
        $consoleWidth = Utils::getConsoleWidth();
        $urlColWidth = 30;
        $headingColWidth = $consoleWidth - $urlColWidth - 24;

        $superTable = new SuperTable(
            self::SUPER_TABLE_SEO_HEADINGS,
            "Heading structure",
            "No URLs to analyze heading structure.",
            [
                new SuperTableColumn('headings', 'Heading structure', $headingColWidth, null, function (SeoAndOpenGraphResult $urlResult, string $renderInfo) {
                    if (!$urlResult->headingTreeItems) {
                        return '';
                    }
                    if ($renderInfo === SuperTable::RENDER_INTO_CONSOLE) {
                        return HeadingTreeItem::getHeadingTreeTxtList($urlResult->headingTreeItems);
                    } else {
                        return HeadingTreeItem::getHeadingTreeUlLiList($urlResult->headingTreeItems);
                    }
                }, true, false, false, false),
                new SuperTableColumn('headingsCount', 'Count', 5),
                new SuperTableColumn('headingsErrorsCount', 'Errors', 6, function ($value) {
                    return Utils::getColorText(strval($value), $value > 0 ? 'red' : 'green', true);
                }, null, false, false),
                new SuperTableColumn('urlPathAndQuery', 'URL', $urlColWidth, null, null, true),
            ], true, 'headingsErrorsCount', 'DESC'
        );

        // hide in console because of too many columns and long values
        $superTable->setVisibilityInConsole(true, 10);

        // set initial URL (required for urlPath column and active link building)
        $superTable->setInitialUrl($this->status->getOptions()->url);

        $superTable->setData($urlResults);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_SEO_AND_OPENGRAPH_ANALYZER,
            'SEO and OpenGraph analyzer', [
            new Option('--max-heading-level', null, 'maxHeadingLevel', Type::INT, false, 'Maximal analyzer heading level from 1 to 6.', 3, false, false, [1, 6]),
        ]));
        return $options;
    }

    public function getOrder(): int
    {
        return 113;
    }

}