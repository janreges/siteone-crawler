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
use Crawler\FoundUrl;
use Crawler\Options\Options;
use Crawler\Utils;

class SkippedUrlsAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_SKIPPED_SUMMARY = 'skipped-summary';
    const SUPER_TABLE_SKIPPED = 'skipped';

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $skippedUrls = [];
        $skippedUrlsSummary = []; // key for $skippedUrlsSummary is domain + reason

        foreach ($this->crawler->getSkippedUrls() as $skippedUrl) {
            $skippedUrls[] = [
                'reason' => $skippedUrl['reason'],
                'url' => $skippedUrl['url'],
                'sourceUqId' => $skippedUrl['sourceUqId'],
                'sourceAttr' => $skippedUrl['sourceAttr'],
            ];

            $domain = parse_url($skippedUrl['url'], PHP_URL_HOST);
            $key = $domain . '_' . $skippedUrl['reason'];
            if (!isset($skippedUrlsSummary[$key])) {
                $skippedUrlsSummary[$key] = [
                    'reason' => $skippedUrl['reason'],
                    'domain' => $domain,
                    'count' => 1,
                ];
            } else {
                $skippedUrlsSummary[$key]['count']++;
            }
        }

        $consoleWidth = Utils::getConsoleWidth();
        $urlColumnWidth = 60;
        $initialScheme = $this->status->getOptions()->getInitialScheme();
        $initialHost = $this->status->getOptions()->getInitialHost();

        // Skipped URLs summary table
        $superTableSummary = new SuperTable(
            self::SUPER_TABLE_SKIPPED_SUMMARY,
            'Skipped URLs Summary',
            'No skipped URLs found.',
            [
                new SuperTableColumn('reason', 'Reason', 18, function ($value) {
                    return Crawler::SKIPPED_REASONS[$value] ?? 'Unknown';
                }),
                new SuperTableColumn('domain', 'Domain'),
                new SuperTableColumn('count', 'Unique URLs', 11),
            ], true, 'count', 'DESC', null, null, 'Skipped URLs');

        $superTableSummary->setData($skippedUrlsSummary);
        $this->status->addSuperTableAtBeginning($superTableSummary);
        $this->output->addSuperTable($superTableSummary);

        // Skipped URLs table
        $status = $this->status;
        $superTable = new SuperTable(
            self::SUPER_TABLE_SKIPPED,
            'Skipped URLs',
            'No skipped URLs found.',
            [
                new SuperTableColumn('reason', 'Reason', 18, function ($value) {
                    return Crawler::SKIPPED_REASONS[$value] ?? 'Unknown';
                }),
                new SuperTableColumn('url', 'Skipped URL', $urlColumnWidth, function ($value) use ($initialHost, $initialScheme) {
                    return Utils::getUrlWithoutSchemeAndHost($value, $initialHost, $initialScheme);
                }, null, true),
                new SuperTableColumn('sourceAttr', 'Source', 19, function ($value) {
                    return FoundUrl::getShortSourceName($value);
                }, null, false),
                new SuperTableColumn('sourceUqId', 'Found at URL', $urlColumnWidth, function ($value) use ($status, $initialHost, $initialScheme) {
                    $urlByUqId = $value ? $status->getUrlByUqId($value) : null;
                    return $urlByUqId ? Utils::getUrlWithoutSchemeAndHost($urlByUqId, $initialHost, $initialScheme) : '';
                }, null, true),
            ], true, 'url', 'ASC');

        $superTable->setData($skippedUrls);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);

        $this->status->addSummaryItemByRanges(
            'skipped',
            count($skippedUrls),
            [[0, 0], [1, 2], [3, 9], [10, PHP_INT_MAX]],
            [
                "Skipped URLs - no skipped URLs found",
                "Skipped URLs - %s skipped URLs found",
                "Skipped URLs - %s skipped URLs found",
                "Skipped URLs - %s skipped URLs found"
            ]
        );
    }

    public function getOrder(): int
    {
        return 6;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}