<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\HeaderStats;
use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Options;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;

class HeadersAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_HEADERS = 'headers';
    const SUPER_TABLE_HEADERS_VALUES = 'headers-values';

    const ANALYSIS_HEADERS = 'HTTP headers';

    /**
     * @var HeaderStats[]
     */
    private array $headerStats = [];

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $consoleWidth = Utils::getConsoleWidth();

        // basic info
        $superTable = new SuperTable(
            self::SUPER_TABLE_HEADERS,
            'HTTP headers',
            'No HTTP headers found.',
            [
                new SuperTableColumn('header', 'Header', SuperTableColumn::AUTO_WIDTH, null, function (HeaderStats $header) {
                    return $header->getFormattedHeaderName();
                }),
                new SuperTableColumn('occurrences', 'Occurs', 6),
                new SuperTableColumn('uniqueValues', 'Unique', 6, null, function (HeaderStats $stats) {
                    $count = count($stats->uniqueValues);
                    if ($count === 0) {
                        return '-';
                    } elseif ($stats->uniqueValuesLimitReached) {
                        return "{$count}+";
                    } else {
                        return $count;
                    }
                }, false),
                new SuperTableColumn('valuesPreview', 'Values preview', $consoleWidth - 90, function ($value, $renderInto) {
                    if (is_string($value) && $renderInto === SuperTable::RENDER_INTO_HTML) {
                        return preg_replace('/\[[^\]]+\]/', '<span class="text-muted">$0</span>', $value);
                    }
                    return $value;
                }, null, true, true, false, false),
                new SuperTableColumn('minValue', 'Min value', 10, null, function (HeaderStats $header) {
                    if ($header->header === 'content-length') {
                        return Utils::getFormattedSize($header->minIntValue);
                    } elseif ($header->header === 'age') {
                        return Utils::getFormattedAge($header->minIntValue);
                    }
                    return $header->minIntValue !== null ? $header->minIntValue : ($header->minDateValue ?: '');
                }),
                new SuperTableColumn('maxValue', 'Max value', 10, null, function (HeaderStats $header) {
                    if ($header->header === 'content-length') {
                        return Utils::getFormattedSize($header->maxIntValue);
                    } elseif ($header->header === 'age') {
                        return Utils::getFormattedAge($header->maxIntValue);
                    }
                    return $header->maxIntValue !== null ? $header->maxIntValue : ($header->maxDateValue ?: '');
                })
            ], true, 'header', 'ASC');

        $superTable->setData($this->headerStats);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        $this->status->addSummaryItemByRanges(
            'unique-headers',
            count($this->headerStats),
            [[0, 30], [31, 40], [41, 50], [51, PHP_INT_MAX]],
            [
                "HTTP headers - found %s unique headers",
                "HTTP headers - found %s unique headers",
                "HTTP headers - found %s unique headers (too many)",
                "HTTP headers - found %s unique headers (too many)"
            ]
        );

        // detail info with header values

        $details = [];
        foreach ($this->headerStats as $header) {
            foreach ($header->uniqueValues as $value => $count) {
                $key = $header->header . '-' . $value;
                $details[$key] = [
                    'header' => $header->getFormattedHeaderName(),
                    'occurrences' => $count,
                    'value' => $value,
                ];
            }
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_HEADERS_VALUES,
            'HTTP header values',
            'No HTTP headers found.',
            [
                new SuperTableColumn('header', 'Header'),
                new SuperTableColumn('occurrences', 'Occurs', 6),
                new SuperTableColumn('value', 'Value', $consoleWidth - 56, null, null, true),
            ], true, null);

        // sort by header asc, then by occurrences desc
        usort($details, function ($a, $b) {
            if ($a['header'] === $b['header']) {
                return $b['occurrences'] <=> $a['occurrences'];
            } else {
                return $a['header'] <=> $b['header'];
            }
        });


        $superTable->setData($details);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * Analyze headers of each request for internal URLs
     *
     * @param VisitedUrl $visitedUrl
     * @param string|null $body
     * @param DOMDocument|null $dom
     * @param array|null $headers
     * @return UrlAnalysisResult|null
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?DOMDocument $dom, ?array $headers): ?UrlAnalysisResult
    {
        if (!$headers || !$visitedUrl->isAllowedForCrawling) {
            return null;
        }

        foreach ($headers as $header => $value) {
            if (!isset($this->headerStats[$header])) {
                $this->headerStats[$header] = new HeaderStats($header);
            }
            $this->headerStats[$header]->addValue($value);
        }

        return null;
    }

    public function getOrder(): int
    {
        return 115;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }

    public static function getAnalysisNames(): array
    {
        return [
            self::ANALYSIS_HEADERS,
        ];
    }
}