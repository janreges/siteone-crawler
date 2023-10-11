<?php

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\Utils;

class FastestAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_FASTEST_ANALYZER = 'fastest-analyzer';

    protected int $fastestTopLimit = 10;
    protected float $fastestMaxTime = 1;

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $fastUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->requestTime <= $this->fastestMaxTime;
        });
        usort($fastUrls, function ($a, $b) {
            return $a->requestTime <=> $b->requestTime;
        });

        $fastUrls = array_slice($fastUrls, 0, $this->fastestTopLimit);

        $consoleWidth = Utils::getConsoleWidth();
        $urlColumnWidth = intval($consoleWidth - 25);

        $superTable = new SuperTable(
            'fastest-urls',
            "TOP {$this->fastestTopLimit} fastest URLs",
            "No fast URLs fastest than {$this->fastestMaxTime} second(s) found.",
            [
                new SuperTableColumn('requestTimeFormatted', 'Time(s)', 7, null),
                new SuperTableColumn('statusCode', 'Status', 6, null),
                new SuperTableColumn('url', 'Fast URL', $urlColumnWidth, function ($value) {
                    return Utils::getUrlWithoutSchemeAndHost($value);
                }),
            ], true, 'requestTime', 'ASC'
        );

        $superTable->setData($fastUrls);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public function getOrder(): int
    {
        return 300;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_FASTEST_ANALYZER,
            'Fastest URL analyzer', [
            new Option('--fastest-urls-top-limit', null, 'fastestTopLimit', Type::INT, false, 'Number of URL addresses in TOP fastest URL addresses.', 10, false, false),
            new Option('--fastest-urls-max-time', null, 'fastestMaxTime', Type::FLOAT, false, 'The maximum response time for an URL address to be evaluated as fast.', 1, false),
        ]));
        return $options;
    }
}