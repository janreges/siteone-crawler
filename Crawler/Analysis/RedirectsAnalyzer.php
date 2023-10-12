<?php

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Options;
use Crawler\Utils;

class RedirectsAnalyzer extends BaseAnalyzer implements Analyzer
{
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

        $status = $this->status;
        $superTable = new SuperTable(
            'redirects',
            'Redirected URLs',
            'No redirects found.',
            [
                new SuperTableColumn('statusCode', 'Status', 6, function ($value) {
                    return Utils::getColoredStatusCode($value);
                }),
                new SuperTableColumn('url', 'Redirected URL', $urlColumnWidth, function ($value) {
                    return Utils::getUrlWithoutSchemeAndHost($value);
                }, null, true),
                new SuperTableColumn('targetUrl', 'Target URL', $urlColumnWidth, null, function ($row) {
                    return Utils::getUrlWithoutSchemeAndHost($row->extras['Location'] ?? '?');
                }, true),
                new SuperTableColumn('sourceUqId', 'Found at URL', $urlColumnWidth, function ($value) use ($status) {
                    return $value ? Utils::getUrlWithoutSchemeAndHost($status->getUrlByUqId($value)) : '';
                }, null, true),
            ], true, 'url', 'ASC');

        $superTable->setData($urlRedirects);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);

        $this->status->addSummaryItemByRanges(
            'redirects',
            count($urlRedirects),
            [[0, 0], [1, 5], [6, PHP_INT_MAX]],
            ["Redirects OK - no redirects found", "Redirects WARNING - %s redirect(s) found", "Redirects CRITICAL - %s redirects found"]
        );
    }

    public function getOrder(): int
    {
        return 200;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}