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
use Crawler\Options\Options;
use Crawler\Utils;

class RedirectsAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_REDIRECTS = 'redirects';

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $urlRedirects = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->statusCode >= 301 && $visitedUrl->statusCode <= 308;
        });

        $consoleWidth = Utils::getConsoleWidth();
        $urlColumnWidth = intval(($consoleWidth - 20) / 3);
        $initialScheme = $this->status->getOptions()->getInitialScheme();
        $initialHost = $this->status->getOptions()->getInitialHost();

        $status = $this->status;
        $superTable = new SuperTable(
            self::SUPER_TABLE_REDIRECTS,
            'Redirected URLs',
            'No redirects found.',
            [
                new SuperTableColumn('statusCode', 'Status', 6, function ($value) {
                    return Utils::getColoredStatusCode($value);
                }),
                new SuperTableColumn('url', 'Redirected URL', $urlColumnWidth, function ($value) use ($initialHost, $initialScheme) {
                    return Utils::getUrlWithoutSchemeAndHost($value, $initialHost, $initialScheme);
                }, null, true),
                new SuperTableColumn('targetUrl', 'Target URL', $urlColumnWidth, null, function ($row) use ($initialHost, $initialScheme) {
                    return Utils::getUrlWithoutSchemeAndHost($row->extras['Location'] ?? '?', $initialHost, $initialScheme);
                }, true),
                new SuperTableColumn('sourceUqId', 'Found at URL', $urlColumnWidth, function ($value) use ($status, $initialHost, $initialScheme) {
                    $urlByUqId = $value ? $status->getUrlByUqId($value) : null;
                    return $urlByUqId ? Utils::getUrlWithoutSchemeAndHost($urlByUqId, $initialHost, $initialScheme) : '';
                }, null, true),
            ], true, 'url', 'ASC');

        $superTable->setData($urlRedirects);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);

        $this->status->addSummaryItemByRanges(
            'redirects',
            count($urlRedirects),
            [[0, 0], [1, 2], [3, 9], [10, PHP_INT_MAX]],
            [
                "Redirects - no redirects found",
                "Redirects - %s redirect(s) found",
                "Redirects - %s redirects found",
                "Redirects - %s redirects found"
            ]
        );
    }

    public function getOrder(): int
    {
        return 10;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}