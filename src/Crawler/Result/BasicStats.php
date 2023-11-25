<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result;

use Crawler\Utils;

class BasicStats
{
    public readonly float $totalExecutionTime;
    public readonly int $totalUrls;
    public readonly int $totalSize;
    public readonly string $totalSizeFormatted;
    public readonly float $totalRequestsTimes;
    public readonly float $totalRequestsTimesAvg;
    public readonly float $totalRequestsTimesMin;
    public readonly float $totalRequestsTimesMax;
    public readonly array $countByStatus;

    /**
     * @param float $totalExecutionTime
     * @param int $totalUrls
     * @param int $totalSize
     * @param string $totalSizeFormatted
     * @param float $totalRequestsTimes
     * @param float $totalRequestsTimesAvg
     * @param float $totalRequestsTimesMin
     * @param float $totalRequestsTimesMax
     * @param array $countByStatus
     */
    public function __construct(float $totalExecutionTime, int $totalUrls, int $totalSize, string $totalSizeFormatted, float $totalRequestsTimes, float $totalRequestsTimesAvg, float $totalRequestsTimesMin, float $totalRequestsTimesMax, array $countByStatus)
    {
        $this->totalExecutionTime = $totalExecutionTime;
        $this->totalUrls = $totalUrls;
        $this->totalSize = $totalSize;
        $this->totalSizeFormatted = $totalSizeFormatted;
        $this->totalRequestsTimes = $totalRequestsTimes;
        $this->totalRequestsTimesAvg = $totalRequestsTimesAvg;
        $this->totalRequestsTimesMin = $totalRequestsTimesMin;
        $this->totalRequestsTimesMax = $totalRequestsTimesMax;

        ksort($countByStatus);
        $this->countByStatus = $countByStatus;
    }

    public function getAsHtml(): string
    {
        $html = '<table class="table table-bordered table-striped table-hover">';
        $html .= '<tr><th colspan="2">Basic stats</th></tr>';
        $html .= '<tr><td>Total execution time</td><td>' . Utils::getFormattedDuration($this->totalExecutionTime) . '</td></tr>';
        $html .= '<tr><td>Total URLs</td><td>' . $this->totalUrls . '</td></tr>';
        $html .= '<tr><td>Total size</td><td>' . $this->totalSizeFormatted . '</td></tr>';
        $html .= '<tr><td>Requests - total time</td><td>' . Utils::getFormattedDuration($this->totalRequestsTimes) . '</td></tr>';
        $html .= '<tr><td>Requests - avg time</td><td>' . Utils::getFormattedDuration($this->totalRequestsTimesAvg) . '</td></tr>';
        $html .= '<tr><td>Requests - min time</td><td>' . Utils::getFormattedDuration($this->totalRequestsTimesMin) . '</td></tr>';
        $html .= '<tr><td>Requests - max time</td><td>' . Utils::getFormattedDuration($this->totalRequestsTimesMax) . '</td></tr>';
        $html .= '<tr><td>Requests by status</td><td>';
        foreach ($this->countByStatus as $statusCode => $count) {
            $html .= Utils::convertBashColorsInTextToHtml(Utils::getColoredStatusCode($statusCode)) . ': ' . htmlspecialchars(strval($count)) . '<br>';
        }
        $html .= '</td></tr>';
        $html .= '</table>';

        return $html;
    }

    /**
     * @param VisitedUrl[] $visitedUrls
     * @param float $startTime
     * @return BasicStats
     */
    public static function fromVisitedUrls(array $visitedUrls, float $startTime): BasicStats
    {
        $info = [
            'totalUrls' => count($visitedUrls),
            'totalSize' => 0,
            'countByStatus' => [],
            'totalTime' => 0,
            'minTime' => null,
            'maxTime' => null,
        ];

        foreach ($visitedUrls as $url) {
            $info['totalTime'] += $url->requestTime;
            $info['totalSize'] += $url->size;
            $info['countByStatus'][$url->statusCode] = ($info['countByStatus'][$url->statusCode] ?? 0) + 1;
            $info['minTime'] = $info['minTime'] === null ? $url->requestTime : min($url->requestTime, $info['minTime']);
            $info['maxTime'] = $info['maxTime'] === null ? $url->requestTime : max($url->requestTime, $info['maxTime']);
        }

        return new self(
            round(microtime(true) - $startTime, 3),
            $info['totalUrls'],
            $info['totalSize'],
            Utils::getFormattedSize($info['totalSize']),
            round($info['totalTime'], 3),
            round($info['totalTime'] / max($info['totalUrls'], 1), 3),
            round($info['minTime'] ?: 0, 3),
            round($info['maxTime'] ?: 0, 3),
            $info['countByStatus']
        );
    }

}