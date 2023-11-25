<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Utils;

class ManagerStats
{

    /**
     * Total exec times of analyzer methods
     * @var array [string => int]
     */
    protected array $execTimes = [];

    /**
     * Total exec counts of analyzer methods
     * @var array [string => int]
     */
    protected array $execCounts = [];

    /**
     * Measure and increment exec time and count of analyzer method
     *
     * @param string $class
     * @param string $method
     * @param float $startTime
     * @return void
     */
    public function measureExecTime(string $class, string $method, float $startTime): void
    {
        $endTime = microtime(true);
        $key = $class . '::' . $method;

        if (!isset($this->execTimes[$key])) {
            $this->execTimes[$key] = 0;
        }
        if (!isset($this->execCounts[$key])) {
            $this->execCounts[$key] = 0;
        }

        $this->execTimes[$key] += ($endTime - $startTime);
        $this->execCounts[$key]++;
    }

    /**
     * @param string $aplCode
     * @param string $title
     * @param string $emptyTableMessage
     * @param float[]|null $externalTimes
     * @param int[]|null $externalCounts
     * @return SuperTable
     */
    public function getSuperTable(string $aplCode, string $title, string $emptyTableMessage, ?array $externalTimes = null, ?array $externalCounts = null): SuperTable
    {
        $data = [];

        // internal stats
        foreach ($this->execTimes as $classAndMethod => $execTime) {
            $data[] = [
                'classAndMethod' => basename(str_replace('\\', '/', $classAndMethod)),
                'execTime' => $execTime,
                'execTimeFormatted' => Utils::getFormattedDuration($execTime),
                'execCount' => $this->execCounts[$classAndMethod] ?? 0,
            ];
        }

        // external stats (if any)
        foreach ($externalTimes ?: [] as $classAndMethod => $execTime) {
            $data[] = [
                'classAndMethod' => basename(str_replace('\\', '/', $classAndMethod)),
                'execTime' => $execTime,
                'execTimeFormatted' => Utils::getFormattedDuration($execTime),
                'execCount' => $externalCounts[$classAndMethod] ?? 0,
            ];
        }

        // configure super table
        $superTable = new SuperTable($aplCode, $title, $emptyTableMessage, [
            new SuperTableColumn('classAndMethod', 'Class::method'),
            new SuperTableColumn('execTime', 'Exec time', 9, function ($value) {
                return Utils::getColoredRequestTime($value, 9);
            }, null, false, false),
            new SuperTableColumn('execCount', 'Exec count'),
        ], false, 'execTime', 'DESC');

        $superTable->setData($data);
        return $superTable;
    }

    public function getExecTimes(): array
    {
        return $this->execTimes;
    }

    public function getExecCounts(): array
    {
        return $this->execCounts;
    }

}