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
use Crawler\Result\VisitedUrl;
use Crawler\Utils;

class ContentTypeAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_CONTENT_TYPE_ANALYZER = 'content-type-analyzer';
    const SUPER_TABLE_CONTENT_TYPES = 'content-types';
    const SUPER_TABLE_CONTENT_MIME_TYPES = 'content-types-raw';

    protected int $fastestTopLimit = 10;
    protected float $fastestMaxTime = 1;

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $this->addContentTypeSuperTable();
        $this->addContentTypeRawSuperTable();
    }

    private function addContentTypeSuperTable(): void
    {
        $stats = [];
        foreach (Crawler::getContentTypeIds() as $contentTypeId) {
            $stats[$contentTypeId] = [
                'contentTypeId' => $contentTypeId,
                'contentType' => Utils::getContentTypeNameById($contentTypeId),
                'count' => 0,
                'totalSize' => 0,
                'totalTime' => 0,
                'status20x' => 0,
                'status30x' => 0,
                'status40x' => 0,
                'status42x' => 0,
                'status50x' => 0,
                'statusOther' => 0,
            ];
        }

        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if ($visitedUrl->hasErrorStatusCode()) {
                continue;
            }
            $stats[$visitedUrl->contentType]['count']++;
            $stats[$visitedUrl->contentType]['totalSize'] += $visitedUrl->size;
            $stats[$visitedUrl->contentType]['totalTime'] += $visitedUrl->requestTime;

            $statusSuffix = $visitedUrl->statusCode >= 200 ? substr(strval($visitedUrl->statusCode), 0, 2) . 'x' : 'Other';
            $stats[$visitedUrl->contentType]['status' . $statusSuffix]++;
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_CONTENT_TYPES,
            "Content types",
            "No URLs found.",
            [
                new SuperTableColumn('contentType', 'Content type', 12),
                new SuperTableColumn('count', 'URLs', 5),
                new SuperTableColumn('totalSize', 'Total size', 10, function ($value) {
                    if ($value) {
                        return Utils::getFormattedSize($value);
                    } else {
                        return '-';
                    }
                }),
                new SuperTableColumn('totalTime', 'Total time', 10, function ($value) {
                    return Utils::getFormattedDuration($value);
                }),
                new SuperTableColumn('avgTime', 'Avg time', 8, function ($value) {
                    return Utils::getColoredRequestTime($value, 8);
                }),
                new SuperTableColumn('status20x', 'Status 20x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'green') : $value;
                }),
                new SuperTableColumn('status30x', 'Status 30x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'yellow', true) : $value;
                }),
                new SuperTableColumn('status40x', 'Status 40x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'magenta', true) : $value;
                }),
                new SuperTableColumn('status42x', 'Status 42x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'magenta', true) : $value;
                }),
                new SuperTableColumn('status50x', 'Status 50x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'red', true) : $value;
                }),
                new SuperTableColumn('statusOther', 'Status ERR', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'red', true) : $value;
                }),
            ], true, 'count', 'DESC'
        );

        foreach ($stats as $contentTypeId => $stat) {
            if ($stat['count'] === 0) {
                unset($stats[$contentTypeId]);
            } else {
                $stats[$contentTypeId]['avgTime'] = $stats[$contentTypeId]['totalTime'] / $stats[$contentTypeId]['count'];
            }
        }

        $superTable->setShowOnlyColumnsWithValues(true); // show only rows with values != 0
        $superTable->setData($stats);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    private function addContentTypeRawSuperTable(): void
    {
        $stats = [];

        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if ($visitedUrl->hasErrorStatusCode()) {
                continue;
            }
            $key = $visitedUrl->contentTypeHeader ?: 'unknown';
            if (!isset($stats[$key])) {
                $stats[$key] = [
                    'contentType' => $key,
                    'count' => 0,
                    'totalSize' => 0,
                    'totalTime' => 0,
                    'status20x' => 0,
                    'status30x' => 0,
                    'status40x' => 0,
                    'status42x' => 0,
                    'status50x' => 0,
                    'statusOther' => 0,
                ];
            }

            $stats[$key]['count']++;
            $stats[$key]['totalSize'] += $visitedUrl->size;
            $stats[$key]['totalTime'] += $visitedUrl->requestTime;

            $statusSuffix = $visitedUrl->statusCode >= 200 ? substr(strval($visitedUrl->statusCode), 0, 2) . 'x' : 'Other';
            $stats[$key]['status' . $statusSuffix]++;
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_CONTENT_MIME_TYPES,
            "Content types (MIME types)",
            "No MIME types found.",
            [
                new SuperTableColumn('contentType', 'Content type', 26),
                new SuperTableColumn('count', 'URLs', 5),
                new SuperTableColumn('totalSize', 'Total size', 10, function ($value) {
                    if ($value) {
                        return Utils::getFormattedSize($value);
                    } else {
                        return '-';
                    }
                }),
                new SuperTableColumn('totalTime', 'Total time', 10, function ($value) {
                    return Utils::getFormattedDuration($value);
                }),
                new SuperTableColumn('avgTime', 'Avg time', 8, function ($value) {
                    return Utils::getColoredRequestTime($value, 8);
                }),
                new SuperTableColumn('status20x', 'Status 20x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'green') : $value;
                }),
                new SuperTableColumn('status30x', 'Status 30x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'yellow', true) : $value;
                }),
                new SuperTableColumn('status40x', 'Status 40x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'magenta', true) : $value;
                }),
                new SuperTableColumn('status42x', 'Status 42x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'magenta', true) : $value;
                }),
                new SuperTableColumn('status50x', 'Status 50x', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'red', true) : $value;
                }),
                new SuperTableColumn('statusOther', 'Status ERR', 10, function ($value) {
                    return $value > 0 ? Utils::getColorText(str_pad(strval($value), 10), 'red', true) : $value;
                }),
            ], true, 'count', 'DESC'
        );

        foreach ($stats as $key => $stat) {
            $stats[$key]['avgTime'] = $stats[$key]['totalTime'] / $stats[$key]['count'];
        }

        $superTable->setShowOnlyColumnsWithValues(true); // show only rows with values != 0
        $superTable->setData($stats);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public function getOrder(): int
    {
        return 210;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}