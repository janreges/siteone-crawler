<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Analysis\AccessibilityAnalyzer;
use Crawler\Analysis\AnalysisManager;
use Crawler\Analysis\BestPracticeAnalyzer;
use Crawler\Analysis\ContentTypeAnalyzer;
use Crawler\Analysis\DnsAnalyzer;
use Crawler\Analysis\FastestAnalyzer;
use Crawler\Analysis\Page404Analyzer;
use Crawler\Analysis\RedirectsAnalyzer;
use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Analysis\SlowestAnalyzer;
use Crawler\Analysis\SourceDomainsAnalyzer;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Export\HtmlReport\Badge;
use Crawler\Export\HtmlReport\Tab;
use Crawler\Result\Status;
use Crawler\Result\Summary\ItemStatus;
use Crawler\Utils;
use Crawler\Version;

class HtmlReport
{

    const SUPER_TABLE_VISITED_URLS = 'visited-urls';

    public readonly Status $status;
    public readonly int $maxExampleUrls;
    private readonly array $skippedSuperTables;

    /**
     * @var int[]
     */
    private static $severityOrder = [
        'critical' => 1,
        'warning' => 2,
        'notice' => 3,
        'ok' => 4,
        'info' => 5
    ];

    public function __construct(Status $status, int $maxExampleUrls = 5)
    {
        $this->status = $status;
        $this->maxExampleUrls = $maxExampleUrls;
        $this->skippedSuperTables = [
            AnalysisManager::SUPER_TABLE_ANALYSIS_STATS, // will be in tab Crawler info
        ];
    }

    public function getHtml(): string
    {
        $html = $this->getTemplate();
        $templateVariables = $this->getTemplateVariables();

        foreach ($templateVariables as $variableName => $variableValue) {
            $html = str_replace('{$' . $variableName . '}', $variableValue, $html);
        }

        return $this->finalizeHtml($html);
    }

    private function getTemplate(): string
    {
        return file_get_contents(__DIR__ . '/HtmlReport/template.html');
    }

    private function finalizeHtml(string $html): string
    {
        // add badges to colored spans
        $html = preg_replace('/(<span)\s+(style="background-color:[^"]+">)/i', '$1 class="badge" $2', $html);
        $html = preg_replace('/(<span)\s+(style="color:[^"]+">)/i', '$1 class="badge in-table" $2', $html);
        $html = str_replace('style="background-color: #ffff00"', 'style="background-color: #ffff00; color: #1F2937"', $html);
        $html = str_ireplace("<td data-value='0'>0</td>", '<td data-value="0"><span class="badge">0</span></td>', $html);

        // other changes
        $html = str_replace('color: #ff00ff', 'color: #ff9234', $html); // change magenta to orange
        $html = preg_replace_callback('/(<td[^>]*>)(\s*[a-z0-9. \/]+\/[a-z0-9. \/]+\s*)(<\/td>)/i', function ($matches) {
            $result = $matches[1] . preg_replace('/\s*\/\s*/', ' / ', $matches[2]) . $matches[3];
            return $result;
        }, $html);

        // badges
        $html = str_replace(
            [
                '<span class="badge in-table" style="color: #00ff00">',
                '<span class="badge in-table" style="color: #ff9234">',
                '<span class="badge in-table" style="color: #ff0000">',
                '<span class="badge in-table" style="background-color: #ffff00; color: #1F2937">',
                '<span class="badge" style="background-color: #ffff00; color: #1F2937">',
                '<span class="badge in-table" style="color: #ffff00">',
            ],
            [
                '<span class="badge green">',
                '<span class="badge orange">',
                '<span class="badge red">',
                '<span class="badge yellow">',
                '<span class="badge yellow">',
                '<span class="badge yellow">',
            ],
            $html
        );

        $html = Utils::removeWhitespacesFromHtml($html);

        return $html;
    }

    /**
     * @return array<string, string>
     */
    private function getTemplateVariables(): array
    {
        $host = parse_url($this->status->getOptions()->url, PHP_URL_HOST);
        $info = $this->status->getCrawlerInfo();
        $tabs = $this->getTabs();

        return [
            'initialHost' => $host,
            'initialUrl' => $this->status->getOptions()->url,
            'version' => Version::CODE,
            'executedAt' => $info->executedAt,
            'command' => preg_replace('/^\S+\.php\s+/i', '', Utils::getSafeCommand($info->command)),
            'hostname' => $info->hostname,
            'userAgent' => $info->finalUserAgent,
            'tabs' => $this->getTabsHtml($tabs),
            'tabsRadios' => $this->getTabsRadios($tabs),
            'tabsContent' => $this->getTabsContentHtml($tabs),
            'tabsCss' => $this->getTabsCss($tabs),
        ];
    }

    /**
     * @return Tab[]
     */
    private function getTabs(): array
    {
        $tabs = [];
        $tabs[] = $this->getSummaryTab();
        $tabs[] = $this->getVisitedUrlsTab();
        $tabs[] = $this->getCrawlerStatsTab();
        $tabs[] = $this->getCrawlerInfo();

        $hostToStripFromUrls = $this->getInitialHost();
        $initialUrl = $this->status->getOptions()->url;

        $superTables = array_merge($this->status->getSuperTablesAtBeginning(), $this->status->getSuperTablesAtEnd());
        foreach ($superTables as $superTable) {
            if (in_array($superTable->aplCode, $this->skippedSuperTables, true)) {
                continue;
            }

            // set props used for clickable URLs building
            $superTable->setHostToStripFromUrls($hostToStripFromUrls);
            $superTable->setInitialUrl($initialUrl);

            $badges = $this->getSuperTableBadgesByAplCode($superTable);
            if (!$badges) {
                $badges = $this->getSuperTableGenericBadges($superTable);
            }
            $tabs[] = new Tab($superTable->title, null, $this->getTabContentBySuperTable($superTable), false, $badges, $this->getSuperTableOrder($superTable));
        }

        // sort tabs by order
        usort($tabs, function (Tab $a, Tab $b) {
            return $a->getFinalSortOrder() <=> $b->getFinalSortOrder();
        });

        return $tabs;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsRadios(array $tabs): string
    {
        $html = '';

        // hidden radio buttons for tabs - to be able to use :checked selector (HTML+CSS only solution for tabs
        // which works in all browsers and e-
        $isFirst = true;
        foreach ($tabs as $tab) {
            $html .= '<input type="radio" id="' . htmlspecialchars($tab->radioHtmlId) . '" name="tabs" class="tabs__radio"' . ($isFirst ? ' checked' : '') . '>' . "\n";
            if ($isFirst) {
                $isFirst = false;
            }
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsHtml(array $tabs): string
    {
        $html = '';

        foreach ($tabs as $tab) {
            $badges = '';
            foreach ($tab->badges as $badge) {
                $badges .= '<span class="badge ' . $badge->color . '"' . ($badge->title ? ' style="cursor: help" title="' . htmlspecialchars($badge->title) . '"' : '') . '>' . htmlspecialchars($badge->value) . '</span> ';
            }

            $html .= '<label for="' . htmlspecialchars($tab->radioHtmlId) . '" class="tabs__title ' . htmlspecialchars($tab->radioHtmlId) . '">' . htmlspecialchars($tab->name) . ($badges ? " {$badges}" : '') . "</label>\n";
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsContentHtml(array $tabs): string
    {
        $html = '';
        $linePrefix = '                ';

        foreach ($tabs as $tab) {
            $html .= $linePrefix . '<div class="tabs__tab ' . htmlspecialchars($tab->contentHtmlId) . '">' . "\n";
            if ($tab->addHeading) {
                $html .= $linePrefix . '    <h2>' . htmlspecialchars($tab->name) . '</h2>' . "\n";
            }
            $html .= $linePrefix . '    ' . str_replace("\n", "\n" . $linePrefix . '    ', $tab->tabContent) . "\n";
            $html .= $linePrefix . '</div>' . "\n";
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsCss(array $tabs): string
    {
        $linePrefix = '        ';

        $selectors = [];
        foreach ($tabs as $tab) {
            $selectors[] = '#' . $tab->radioHtmlId . ':checked ~ .tabs__content .' . $tab->contentHtmlId;
        }

        $css = implode(', ', $selectors) . " {\n";
        $css .= $linePrefix . "    display: block;\n";
        $css .= $linePrefix . "}\n";

        // active tab title
        $selectors = [];
        foreach ($tabs as $tab) {
            $selectors[] = '#' . $tab->radioHtmlId . ':checked ~ .tabs__navigation .' . $tab->radioHtmlId;
        }

        $css .= implode(', ', $selectors) . " {\n";
        $css .= $linePrefix . "    background-color: var(--color-blue-600);\n";
        $css .= $linePrefix . "    color: var(--color-white);\n";
        $css .= $linePrefix . "}\n";

        return $css;
    }

    private function getSummaryTab(): Tab
    {
        $summary = $this->status->getSummary();
        $colorToCount = [
            Badge::COLOR_RED => $summary->getCountByItemStatus(ItemStatus::CRITICAL),
            Badge::COLOR_ORANGE => $summary->getCountByItemStatus(ItemStatus::WARNING),
            Badge::COLOR_GREEN => $summary->getCountByItemStatus(ItemStatus::OK),
            Badge::COLOR_BLUE => $summary->getCountByItemStatus(ItemStatus::NOTICE),
            Badge::COLOR_NEUTRAL => $summary->getCountByItemStatus(ItemStatus::INFO),
        ];

        $badges = [];
        foreach ($colorToCount as $color => $count) {
            if ($count > 0) {
                $badges[] = new Badge((string)$count, $color);
            }
        }

        return new Tab('Summary', null, $summary->getAsHtml(), true, $badges, -100);
    }

    private function getCrawlerStatsTab(): Tab
    {
        $html = '';

        $stats = $this->status->getBasicStats();
        $badges = [
            new Badge(strval($stats->totalUrls), Badge::COLOR_NEUTRAL, 'Total visited URLs'),
            new Badge($stats->totalSizeFormatted, Badge::COLOR_NEUTRAL, 'Total size of all visited URLs'),
            new Badge(Utils::getFormattedDuration($stats->totalExecutionTime), Badge::COLOR_NEUTRAL, 'Total execution time'),
        ];

        $html .= $stats->getAsHtml();

        $analysisStats = $this->status->getSuperTableByAplCode(AnalysisManager::SUPER_TABLE_ANALYSIS_STATS);
        if ($analysisStats) {
            $html .= '<br/>' . $analysisStats->getHtmlOutput();
        }

        return new Tab('Crawler stats', null, $html, true, $badges, 900);
    }

    private function getCrawlerInfo(): Tab
    {
        $html = '
            <h2>Crawler info</h2>
            <div class="info__wrapper">
                <table style="border-collapse: collapse;">
                    <tr>
                        <th>Version</th>
                        <td>{$version}</td>
                    </tr>
                    <tr>
                        <th>Executed At</th>
                        <td>{$executedAt}</td>
                    </tr>
                    <tr>
                        <th>Command</th>
                        <td>{$command}</td>
                    </tr>
                    <tr>
                        <th>Hostname</th>
                        <td>{$hostname}</td>
                    </tr>
                    <tr>
                        <th>User-Agent</th>
                        <td>{$userAgent}</td>
                    </tr>
                </table>
            </div>';

        $html = str_replace(
            ['{$version}', '{$executedAt}', '{$command}', '{$hostname}', '{$userAgent}'],
            [
                $this->status->getCrawlerInfo()->version,
                $this->status->getCrawlerInfo()->executedAt,
                Utils::getSafeCommand($this->status->getCrawlerInfo()->command),
                $this->status->getCrawlerInfo()->hostname,
                $this->status->getCrawlerInfo()->finalUserAgent
            ], $html
        );

        $badges = [new Badge('v' . Version::CODE, Badge::COLOR_NEUTRAL, 'Crawler version')];
        return new Tab('Crawler info', null, $html, false, $badges, 5000);
    }

    private function getVisitedUrlsTab(): Tab
    {
        $visitedUrlsTable = $this->getVisitedUrlsTable();
        $visitedUrlsTable->setHostToStripFromUrls($this->getInitialHost());
        $badges = $this->getSuperTableBadgesByAplCode($visitedUrlsTable);
        return new Tab($visitedUrlsTable->title, $visitedUrlsTable->description, $visitedUrlsTable->getHtmlOutput(), false, $badges, $this->getSuperTableOrder($visitedUrlsTable));
    }

    private function getVisitedUrlsTable(): SuperTable
    {
        $initialHost = $this->getInitialHost();

        // setup columns
        $columns = [
            new SuperTableColumn('url', 'URL', $this->status->getOptions()->urlColumnSize, null, function ($row) use ($initialHost) {
                return '<a href="' . htmlspecialchars($row['url']) . '" target="_blank">' . Utils::truncateUrl($row['url'], 80, '...', $initialHost) . '</a>';
            }),
            new SuperTableColumn('status', 'Status', 6, function ($value) {
                return Utils::getColoredStatusCode($value);
            }),
            new SuperTableColumn('type', 'Type', 8),
            new SuperTableColumn('time', 'Time (s)', 8, null, function ($row) {
                return Utils::getColoredRequestTime($row['time'], 6);
            }),
            new SuperTableColumn('size', 'Size', 8, null, function ($row) {
                if ($row['size'] > 1024 * 1024) {
                    return Utils::getColorText($row['sizeFormatted'], 'red', true);
                } else {
                    return $row['sizeFormatted'];
                }
            }),
        ];

        foreach ($this->status->getOptions()->extraColumns as $extraColumn) {
            $columns[] = new SuperTableColumn($extraColumn->name, $extraColumn->name, $extraColumn->length ?: SuperTableColumn::AUTO_WIDTH);
        }

        // setup supertable
        $superTable = new SuperTable(self::SUPER_TABLE_VISITED_URLS, 'Visited URLs', 'No visited URLs.', $columns, false);

        // set data
        $data = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            $row = [
                'url' => $visitedUrl->url,
                'status' => $visitedUrl->statusCode,
                'type' => Utils::getContentTypeNameById($visitedUrl->contentType),
                'time' => $visitedUrl->requestTime,
                'size' => $visitedUrl->size,
                'sizeFormatted' => $visitedUrl->sizeFormatted,
            ];

            if ($visitedUrl->extras) {
                $row = array_merge($row, $visitedUrl->extras);
            }

            $data[] = $row;
        }
        $superTable->setData($data);

        return $superTable;
    }

    /**
     * @param SuperTable $superTable
     * @return Badge[]
     */
    private function getSuperTableBadgesByAplCode(SuperTable $superTable): array
    {
        $badges = [];

        switch ($superTable->aplCode) {
            case RedirectsAnalyzer::SUPER_TABLE_REDIRECTS:
                $redirects = $superTable->getTotalRows();
                $color = $redirects > 100 ? Badge::COLOR_RED : ($redirects > 0 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                $badges[] = new Badge((string)$redirects, $color);
                break;
            case Page404Analyzer::SUPER_TABLE_404:
                $notFound = $superTable->getTotalRows();
                $color = $notFound > 10 ? Badge::COLOR_RED : ($notFound > 0 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                $badges[] = new Badge((string)$notFound, $color);
                break;
            case SourceDomainsAnalyzer::SUPER_TABLE_SOURCE_DOMAINS:
                $domains = $superTable->getTotalRows();
                $color = $domains > 10 ? Badge::COLOR_ORANGE : Badge::COLOR_NEUTRAL;
                $badges[] = new Badge((string)$domains, $color);
                break;
            case ContentTypeAnalyzer::SUPER_TABLE_CONTENT_TYPES:
                $contentTypes = $superTable->getTotalRows();
                $badges[] = new Badge((string)$contentTypes, Badge::COLOR_NEUTRAL);
                break;
            case FastestAnalyzer::SUPER_TABLE_FASTEST_URLS:
                $fastestTime = null;
                foreach ($superTable->getData() as $row) {
                    if ($fastestTime === null) {
                        $fastestTime = $row->requestTime;
                    } else {
                        $fastestTime = min($fastestTime, $row->requestTime);
                    }
                }
                $color = $fastestTime < 0.5 ? Badge::COLOR_GREEN : ($fastestTime < 2 ? Badge::COLOR_ORANGE : Badge::COLOR_RED);
                $badges[] = new Badge(Utils::getFormattedDuration($fastestTime ?: 0), $color);
                break;
            case SlowestAnalyzer::SUPER_TABLE_SLOWEST_URLS:
                $slowestTime = null;
                foreach ($superTable->getData() as $row) {
                    if ($slowestTime === null) {
                        $slowestTime = $row->requestTime;
                    } else {
                        $slowestTime = max($slowestTime, $row->requestTime);
                    }
                }
                $color = $slowestTime < 0.5 ? Badge::COLOR_GREEN : ($slowestTime < 2 ? Badge::COLOR_ORANGE : Badge::COLOR_RED);
                $badges[] = new Badge(Utils::getFormattedDuration($slowestTime ?: 0), $color);
                break;
            case DnsAnalyzer::SUPER_TABLE_DNS:
                $ipv4 = 0;
                $ipv6 = 0;
                foreach ($superTable->getData() as $row) {
                    if (stripos($row['info'], 'IPv4') !== false) {
                        $ipv4++;
                    } elseif (stripos($row['info'], 'IPv6') !== false) {
                        $ipv6++;
                    }
                }

                if ($ipv4) {
                    $badges[] = new Badge("{$ipv4}x IPv4", $ipv4 > 1 ? Badge::COLOR_GREEN : Badge::COLOR_NEUTRAL);
                }
                if ($ipv6) {
                    $badges[] = new Badge("{$ipv6}x IPv6", $ipv6 > 1 ? Badge::COLOR_GREEN : Badge::COLOR_NEUTRAL);
                }

                break;
            case self::SUPER_TABLE_VISITED_URLS:
                $red = 0;
                $orange = 0;
                $green = 0;
                foreach ($superTable->getData() as $row) {
                    $statusCode = $row['status'] ?? 0;
                    if ($statusCode <= 0 || $statusCode >= 400) {
                        $red++;
                    } elseif ($statusCode >= 300) {
                        $orange++;
                    } else {
                        $green++;
                    }
                }

                if ($red) {
                    $badges[] = new Badge((string)$red, Badge::COLOR_RED, 'Errors (40x, 50x, timeout, etc.)');
                }
                if ($orange) {
                    $badges[] = new Badge((string)$orange, Badge::COLOR_ORANGE, 'Redirects (30x)');
                }
                if ($green) {
                    $badges[] = new Badge((string)$green, Badge::COLOR_GREEN, 'OK (20x)');
                }
                break;
        }

        return $badges;
    }

    /**
     * @param SuperTable $superTable
     * @return Badge[]
     */
    private function getSuperTableGenericBadges(SuperTable $superTable): array
    {
        $badges = [];

        $red = 0;
        $orange = 0;
        $green = 0;
        $blue = 0;
        $neutral = 0;

        foreach ($superTable->getData() as $row) {
            if (is_object($row)) {
                $row = (array)$row;
            }
            if (isset($row['ok']) && is_numeric($row['ok'])) {
                $green += $row['ok'];
            }
            if (isset($row['notice']) && is_numeric($row['notice'])) {
                $blue += $row['notice'];
            }
            if (isset($row['warning']) && is_numeric($row['warning'])) {
                $orange += $row['warning'];
            }
            if (isset($row['critical']) && is_numeric($row['critical'])) {
                $red += $row['critical'];
            }
            if (isset($row['error']) && is_numeric($row['error'])) {
                $red += $row['error'];
            }
            if (isset($row['info']) && is_numeric($row['info'])) {
                $neutral += $row['info'];
            }
        }

        if ($red > 0) {
            $badges[] = new Badge((string)$red, Badge::COLOR_RED, 'Critical');
        }
        if ($orange > 0) {
            $badges[] = new Badge((string)$orange, Badge::COLOR_ORANGE, 'Warning');
        }
        if ($green > 0) {
            $badges[] = new Badge((string)$green, Badge::COLOR_GREEN, 'OK');
        }
        if ($blue > 0) {
            $badges[] = new Badge((string)$blue, Badge::COLOR_BLUE, 'Notice');
        }
        if ($neutral > 0) {
            $badges[] = new Badge((string)$neutral, Badge::COLOR_NEUTRAL, 'Info');
        }

        return $badges;
    }

    private function getTabContentBySuperTable(SuperTable $superTable): string
    {
        $html = $superTable->getHtmlOutput();
        $superTables = [];

        switch ($superTable->aplCode) {
            case BestPracticeAnalyzer::SUPER_TABLE_BEST_PRACTICES:
                foreach (BestPracticeAnalyzer::getAnalysisNames() as $analysisName) {
                    $superTable = $this->getSuperTableForUrlAnalysis($analysisName);
                    if ($superTable) {
                        $superTables[] = $superTable;
                    }
                }
                break;
            case AccessibilityAnalyzer::SUPER_TABLE_ACCESSIBILITY:
                foreach (AccessibilityAnalyzer::getAnalysisNames() as $analysisName) {
                    $superTable = $this->getSuperTableForUrlAnalysis($analysisName);
                    if ($superTable) {
                        $superTables[] = $superTable;
                    }
                }
                break;
        }

        foreach ($superTables as $superTable) {
            $html .= '<br/>' . $superTable->getHtmlOutput();
        }

        return $html;
    }

    public function getSuperTableOrder(SuperTable $superTable): int
    {
        static $orders = [
            BestPracticeAnalyzer::SUPER_TABLE_BEST_PRACTICES,
            AccessibilityAnalyzer::SUPER_TABLE_ACCESSIBILITY,
            Page404Analyzer::SUPER_TABLE_404,
            RedirectsAnalyzer::SUPER_TABLE_REDIRECTS,
            FastestAnalyzer::SUPER_TABLE_FASTEST_URLS,
            SlowestAnalyzer::SUPER_TABLE_SLOWEST_URLS,
            ContentTypeAnalyzer::SUPER_TABLE_CONTENT_TYPES,
            SourceDomainsAnalyzer::SUPER_TABLE_SOURCE_DOMAINS,
            DnsAnalyzer::SUPER_TABLE_DNS,
            self::SUPER_TABLE_VISITED_URLS,
        ];

        $index = array_search($superTable->aplCode, $orders, true);
        return is_int($index) ? $index : 1000;
    }

    public function getSuperTableForUrlAnalysis(string $analysisName): ?SuperTable
    {
        static $details = null;
        if ($details === null) {
            $details = $this->getDataForSuperTablesWithDetails();
        }

        $initialHost = $this->getInitialHost();

        if (isset($details[$analysisName])) {
            $superTable = new SuperTable($analysisName, $analysisName, 'No details.', [
                new SuperTableColumn('severity', 'Severity', 10, null, function ($row) {
                    return Utils::getColoredSeverity($row['severityFormatted']);
                }),
                new SuperTableColumn('count', 'Occurs', 8, function ($value) {
                    return $value;
                }),
                new SuperTableColumn('detail', 'Detail', 200, function ($detail) {
                    if (is_string($detail) || is_numeric($detail)) {
                        // check if string contains only non-HTML content or HTML tags <svg>
                        $isSvg = preg_match('/<svg/i', $detail) === 1;
                        if (preg_match('/^[\s\w\d.,:;!?()\/\-]*$/i', $detail) || $isSvg) {
                            if ($isSvg) {
                                $detail = str_replace(' display="block', '', $detail);
                                return Utils::getFormattedSize(strlen($detail)) . ' ' . $detail;
                            } else {
                                return $detail;
                            }
                        } else {
                            return nl2br(htmlspecialchars($detail));
                        }
                    } elseif (is_array($detail) || is_object($detail)) {
                        return nl2br(htmlspecialchars(json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES)));
                    } else {
                        return '';
                    }
                }),
                new SuperTableColumn('exampleUrls', 'Affected URLs (max ' . $this->maxExampleUrls . ')', 60, null, function ($row) use ($initialHost) {
                    $result = '';
                    if (isset($row['exampleUrls']) && $row['exampleUrls'] && count($row['exampleUrls']) === 1) {
                        foreach ($row['exampleUrls'] as $exampleUrl) {
                            $result .= '<a href="' . htmlspecialchars($exampleUrl) . '" target="_blank">' . htmlspecialchars(Utils::truncateUrl($exampleUrl, 60, '...', $initialHost)) . '</a><br />';
                        }
                    } elseif (isset($row['exampleUrls']) && $row['exampleUrls']) {
                        $counter = 1;
                        foreach ($row['exampleUrls'] as $exampleUrl) {
                            $result .= '<a href="' . htmlspecialchars($exampleUrl) . '" target="_blank">' . "URL {$counter}</a>, ";
                            $counter++;
                        }
                    }
                    return rtrim($result, ', ');
                }),
            ], false, null, 'ASC', null, 100);// sort primary by severity and secondary by count
            ;
            usort($details[$analysisName], function ($a, $b) {
                if ($a['severity'] === $b['severity']) {
                    return $b['count'] <=> $a['count'];
                } else {
                    return $a['severity'] <=> $b['severity'];
                }
            });

            $superTable->setData($details[$analysisName]);
            return $superTable;
        }

        return null;
    }

    /**
     * @return array<string, array>
     */
    private function getDataForSuperTablesWithDetails(): array
    {
        $data = [];
        foreach ($this->status->getVisitedUrlToAnalysisResult() as $visitedUrlUqId => $analysisResults) {
            $url = $this->status->getUrlByUqId(strval($visitedUrlUqId));
            foreach ($analysisResults as $analysisResult) {
                /* @var UrlAnalysisResult $analysisResult */
                foreach ($analysisResult->getCriticalDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'critical',
                            'severity' => self::$severityOrder['critical'],
                            'detail' => $detail,
                        ];
                    }
                }

                foreach ($analysisResult->getWarningDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'warning',
                            'severity' => self::$severityOrder['warning'],
                            'detail' => $detail,
                        ];
                    }
                }

                foreach ($analysisResult->getNoticeDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'notice',
                            'severity' => self::$severityOrder['notice'],
                            'detail' => $detail,
                        ];
                    }
                }
            }
        }

        $data = $this->getSuperTableDataAggregated($data);

        return $data;
    }

    /**
     * This method solves data aggregation so that the table with findings does not contain tens of thousands
     * of identical findings, on different URLs, but they are meaningfully grouped and "masked" when needed
     *
     * @param array $data
     * @return array
     */
    private function getSuperTableDataAggregated(array $data): array
    {
        $result = [];

        foreach ($data as $analysisName => $rows) {
            if (!isset($result[$analysisName])) {
                $result[$analysisName] = [];
            }
            foreach ($rows as $row) {
                $detail = $this->getAggregatedDetail($row['detail']);
                $key = $this->getAggregatedDetailKey($row['severityFormatted'], $detail);
                if (!isset($result[$analysisName][$key])) {
                    $result[$analysisName][$key] = [
                        'severityFormatted' => $row['severityFormatted'],
                        'severity' => $row['severity'],
                        'detail' => $detail,
                        'count' => 1,
                        'exampleUrls' => [$row['url'] => $row['url']],
                    ];
                } else {
                    $result[$analysisName][$key]['count']++;
                    if (count($result[$analysisName][$key]['exampleUrls']) < $this->maxExampleUrls) {
                        $result[$analysisName][$key]['exampleUrls'][$row['url']] = $row['url'];
                    }
                }
            }
        }

        return $result;
    }

    private function getAggregatedDetail(mixed $detail): string
    {
        $result = null;
        if (is_string($detail)) {
            if (str_starts_with($detail, '<svg') || str_contains($detail, 'x SVG ')) {
                return $detail;
            }
            $result = Utils::removeUnwantedHtmlAttributes($detail, ['id', 'class', 'name']);

            // when detail is HTML tag, get only the first HTML tag
            if (str_starts_with(trim($result, '"\' '), '<')) {
                if (preg_match('/^[\s"\']*(<[^>]+>)/s', $result, $matches) === 1) {
                    $result = $matches[1];
                }
            }

            // replace trailing numbers in attributes to ***, e.g. in <img class="image-215">
            $result = preg_replace('/([0-9]+)(["\'])/s', '***$2', $result);

        } elseif (is_array($detail)) {
            foreach ($detail as $key => $value) {
                $detail[$key] = $this->getAggregatedDetail($value);
            }
            $result = json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } elseif (is_object($detail)) {
            foreach ($detail as $key => $value) {
                $detail->$key = $this->getAggregatedDetail($value);
            }
            $result = json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        }
        return $result ?? '';
    }

    private function getAggregatedDetailKey(string $severity, string $detail): string
    {
        // remove clip-path attribute from SVGs (often is used in dynamic SVGs, but it is not important for comparison)
        if (str_contains($detail, '<svg')) {
            $detail = preg_replace('/<clipPath[^>]+>/i', '', $detail);
            $detail = preg_replace('/clip-path="[^"]+"/i', '', $detail);
        }
        return $severity . ' | ' . md5($detail);
    }

    private function getInitialHost(): string
    {
        return $this->status->getOptions()->getInitialHost();
    }

}