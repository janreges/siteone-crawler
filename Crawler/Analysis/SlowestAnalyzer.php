<?php

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\Utils;

class SlowestAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_SLOWEST_ANALYZER = 'slowest-analyzer';

    protected int $slowestTopLimit = 10;
    protected float $slowestMinTime = 0.1;

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $slowUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->requestTime >= $this->slowestMinTime;
        });
        usort($slowUrls, function ($a, $b) {
            return $b->requestTime <=> $a->requestTime;
        });

        $slowUrls = array_slice($slowUrls, 0, $this->slowestTopLimit);

        $consoleWidth = Utils::getConsoleWidth();
        $urlColumnWidth = intval($consoleWidth - 25);

        $superTable = new SuperTable(
            'slowest-urls',
            "TOP {$this->slowestTopLimit} slowest URLs",
            "No slow URLs slowest than {$this->slowestMinTime} second(s) found.",
            [
                new SuperTableColumn('requestTimeFormatted', 'Time(s)', 7, null),
                new SuperTableColumn('statusCode', 'Status', 6, null),
                new SuperTableColumn('url', 'Slow URL', $urlColumnWidth, function ($value) {
                    return Utils::getUrlWithoutSchemeAndHost($value);
                }),
            ], true, 'requestTime', 'DESC');

        $superTable->setData($slowUrls);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public function getOrder(): int
    {
        return 400;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_SLOWEST_ANALYZER,
            'Slowest URL analyzer', [
            new Option('--slowest-urls-top-limit', null, 'slowestTopLimit', Type::INT, false, 'Number of URL addresses in TOP slowest URL addresses.', 10, false, false),
            new Option('--slowest-urls-min-time', null, 'slowestMinTime', Type::FLOAT, false, 'The minimum response time for an URL address to be evaluated as slow.', 0.1, false),
        ]));
        return $options;
    }
}