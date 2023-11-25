<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

class AnalyzerStats
{

    /**
     * @var array <string, array<string, array>>
     */
    private array $severityCountsPerAnalysis = [];

    /**
     * Add OK result to stats. You can optionally specify subject (e.g. URL or SVG body) to count only unique results.
     *
     * @param string $analysisName
     * @param string|null $subject
     * @return void
     */
    public function addOk(string $analysisName, ?string $subject = null): void
    {
        $this->addResult($analysisName, 'ok', $subject);
    }

    /**
     * Add WARNING result to stats. You can optionally specify subject (e.g. URL or SVG body) to count only unique results.
     *
     * @param string $analysisName
     * @param string|null $subject
     * @return void
     */
    public function addWarning(string $analysisName, ?string $subject = null): void
    {
        $this->addResult($analysisName, 'warning', $subject);
    }

    /**
     * Add CRITICAL result to stats. You can optionally specify subject (e.g. URL or SVG body) to count only unique results.
     *
     * @param string $analysisName
     * @param string|null $subject
     * @return void
     */
    public function addCritical(string $analysisName, ?string $subject = null): void
    {
        $this->addResult($analysisName, 'critical', $subject);
    }

    /**
     * Add NOTICE result to stats. You can optionally specify subject (e.g. URL or SVG body) to count only unique results.
     *
     * @param string $analysisName
     * @param string|null $subject
     * @return void
     */
    public function addNotice(string $analysisName, ?string $subject = null): void
    {
        $this->addResult($analysisName, 'notice', $subject);
    }

    /**
     * @return array <int<0, max>, array<string, int<0, max>|string>>
     */
    public function toTableArray(): array
    {
        $result = [];
        foreach ($this->severityCountsPerAnalysis as $analysisName => $severityCounts) {
            $result[] = [
                'analysisName' => $analysisName,
                'ok' => count($severityCounts['ok']),
                'notice' => count($severityCounts['notice']),
                'warning' => count($severityCounts['warning']),
                'critical' => count($severityCounts['critical']),
            ];
        }
        return $result;
    }

    private function addResult(string $analysisName, string $severity, ?string $subject): void
    {
        if (!isset($this->severityCountsPerAnalysis[$analysisName])) {
            $this->severityCountsPerAnalysis[$analysisName] = [
                'ok' => [],
                'warning' => [],
                'critical' => [],
                'notice' => [],
            ];
        }

        // subject hash - 10 chars should be enough to identify unique subjects (1,099,511,627,776 combinations)
        $subjectHash = $subject !== null ? substr(md5(trim($subject)), 0, 10) : null;

        if ($subjectHash) {
            $this->severityCountsPerAnalysis[$analysisName][$severity][$subjectHash] = true;
        } else {
            $this->severityCountsPerAnalysis[$analysisName][$severity][] = true;
        }
    }

}