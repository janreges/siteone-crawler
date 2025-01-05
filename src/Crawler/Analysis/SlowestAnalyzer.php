<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\Utils;

class SlowestAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_SLOWEST_ANALYZER = 'slowest-analyzer';
    const SUPER_TABLE_SLOWEST_URLS = 'slowest-urls';

    protected int $slowestTopLimit = 20;
    protected float $slowestMinTime = 0.01;
    protected float $slowestMaxTime = 3;

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $slowUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->isAllowedForCrawling && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $visitedUrl->requestTime >= $this->slowestMinTime;
        });
        usort($slowUrls, function ($a, $b) {
            return $b->requestTime <=> $a->requestTime;
        });

        $slowUrls = array_slice($slowUrls, 0, $this->slowestTopLimit);

        $consoleWidth = Utils::getConsoleWidth();
        $urlColumnWidth = intval($consoleWidth - 25);

        $superTable = new SuperTable(
            self::SUPER_TABLE_SLOWEST_URLS,
            "TOP slowest URLs",
            "No slow URLs slower than {$this->slowestMinTime} second(s) found.",
            [
                new SuperTableColumn('requestTime', 'Time', 6, function ($value) {
                    return Utils::getColoredRequestTime($value, 6);
                }),
                new SuperTableColumn('statusCode', 'Status', 6, function ($value) {
                    return Utils::getColoredStatusCode($value);
                }),
                new SuperTableColumn('url', 'Slow URL', $urlColumnWidth, null, null, true),
            ], true, 'requestTime', 'DESC');

        $superTable->setData($slowUrls);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);

        $verySlowUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $visitedUrl->requestTime >= $this->slowestMaxTime;
        });

        $this->status->addSummaryItemByRanges(
            'slowUrls',
            count($verySlowUrls),
            [[0, 0], [1, 2], [3, 5], [6, PHP_INT_MAX]],
            [
                "Performance OK - all non-media URLs are faster than {$this->slowestMaxTime} seconds",
                "Performance NOTICE - %s slow non-media URL(s) found (slower than {$this->slowestMaxTime} seconds)",
                "Performance WARNING - %s slow non-media URLs found (slower than {$this->slowestMaxTime} seconds)",
                "Performance CRITICAL - %s slow non-media URLs found (slower than {$this->slowestMaxTime} seconds)"
            ]
        );
    }

    public function getOrder(): int
    {
        return 110;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_SLOWEST_ANALYZER,
            'Slowest URL analyzer', [
            new Option('--slowest-urls-top-limit', null, 'slowestTopLimit', Type::INT, false, 'Number of URL addresses in TOP slowest URL addresses.', 20, false, false),
            new Option('--slowest-urls-min-time', null, 'slowestMinTime', Type::FLOAT, false, 'The minimum response time for an URL address to be added to TOP slow selection.', 0.01, false),
            new Option('--slowest-urls-max-time', null, 'slowestMaxTime', Type::FLOAT, false, 'The maximum response time for an URL address to be evaluated as very slow.', 3, false),
        ]));
        return $options;
    }
}