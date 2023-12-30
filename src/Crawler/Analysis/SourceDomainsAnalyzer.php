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

class SourceDomainsAnalyzer extends BaseAnalyzer implements Analyzer
{

    const SUPER_TABLE_SOURCE_DOMAINS = 'source-domains';

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $stats = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if ($visitedUrl->hasErrorStatusCode()) {
                continue;
            }
            $urlHost = parse_url($visitedUrl->url, PHP_URL_HOST);

            // create init stat item for the host
            if (!isset($stats[$urlHost])) {
                $stat = [
                    'domain' => $urlHost,
                ];
                foreach (Crawler::getContentTypeIds() as $contentTypeId) {
                    $stat["{$contentTypeId}.count"] = 0;
                    $stat["{$contentTypeId}.totalSize"] = 0;
                    $stat["{$contentTypeId}.totalExecTime"] = 0;
                }
                $stats[$urlHost] = $stat;
            }

            $hostStat = &$stats[$urlHost];
            $contentTypeId = $visitedUrl->contentType;

            $hostStat["{$contentTypeId}.count"]++;
            $hostStat["{$contentTypeId}.totalSize"] += $visitedUrl->size;
            $hostStat["{$contentTypeId}.totalExecTime"] += $visitedUrl->requestTime;
        }

        // convert $stats to $data in this format ['siteone.io' => ['total' => '15 / 253 kB / 0.84 s', 'html' => '3 / 843 kB / 0.28 s', 'js' => '4 / 56 kB / 0.54 s']]
        $data = [];
        $contentTypeLongestValue = [];
        foreach ($stats as $domain => $stat) {
            $total = [
                'count' => 0,
                'size' => 0,
                'execTime' => 0,
            ];
            $data[$domain] = [
                'domain' => $domain,
                'totals' => $total,
            ];
            foreach (Crawler::getContentTypeIds() as $contentTypeId) {
                $count = $stat["{$contentTypeId}.count"];
                $size = $stat["{$contentTypeId}.totalSize"];
                $execTime = $stat["{$contentTypeId}.totalExecTime"];
                $contentType = Utils::getContentTypeNameById($contentTypeId);

                $total['count'] += $count;
                $total['size'] += $size;
                $total['execTime'] += $execTime;

                if ($count) {
                    $data[$domain][$contentType] = str_replace(' ', '', "$count / " . Utils::getFormattedSize($size, 0) . " / " . Utils::getFormattedDuration($execTime));
                } else {
                    $data[$domain][$contentType] = '';
                }

                $contentTypeLongestValue[$contentType] = max($contentTypeLongestValue[$contentType] ?? 0, strlen($data[$domain][$contentType]));
            }
            $data[$domain]['totals'] = str_replace(' ', '', "{$total['count']} / " . Utils::getFormattedSize($total['size'], 0) . " / " . Utils::getFormattedDuration($total['execTime']));
            $data[$domain]['totalCount'] = $total['count'];
        }

        // unset all content-type columns that have 0 requests in total
        foreach ($data as $domain => $stat) {
            foreach ($contentTypeLongestValue as $contentTypeName => $longestValue) {
                if ($longestValue === 0) {
                    unset($data[$domain][$contentTypeName]);
                }
            }
        }

        // setup supertable
        $delimiter = Utils::getColorText('/', 'dark-gray');
        $statsFormatter = function ($value, $renderInto) use ($delimiter) {
            if ($renderInto === SuperTable::RENDER_INTO_HTML) {
                return str_replace('/', " {$delimiter} ", $value);
            } else {
                return str_replace('/', $delimiter, $value);
            }
        };
        $superTableColumns = [
            new SuperTableColumn('domain', 'Domain'),
            new SuperTableColumn('totals', 'Totals', SuperTableColumn::AUTO_WIDTH, $statsFormatter, null, false, false),
        ];

        // add content-type columns
        foreach ($data as $domain => $stat) {
            foreach ($stat as $contentType => $value) {
                if ($contentType === 'domain' || $contentType === 'totals' || $contentType === 'totalCount') {
                    continue;
                }
                $superTableColumns[] = new SuperTableColumn(
                    $contentType,
                    $contentType,
                    SuperTableColumn::AUTO_WIDTH,
                    $statsFormatter,
                    null,
                    false,
                    false   // formatterWillChangeValueLength = false (because we only add colors)
                );
            }
            break;
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_SOURCE_DOMAINS,
            "Source domains",
            "No source domains found.",
            $superTableColumns, false, 'totalCount', 'DESC'
        );

        $superTable->setData($data);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public function getOrder(): int
    {
        return 205;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}