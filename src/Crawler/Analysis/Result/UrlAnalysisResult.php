<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

use Crawler\Utils;

class UrlAnalysisResult
{
    /**
     * @var string[]
     */
    private array $ok = [];

    /**
     * @var string[]
     */
    private array $notice = [];

    /**
     * @var string[]
     */
    private array $warning = [];

    /**
     * @var string[]
     */
    private array $critical = [];

    /**
     * @var array[]
     */
    private array $okDetails = [];

    /**
     * @var array[]
     */
    private array $noticeDetails = [];

    /**
     * @var array[]
     */
    private array $warningDetails = [];

    /**
     * @var array[]
     */
    private array $criticalDetails = [];

    /**
     * Stats per analysis and severity
     * @var array[]
     */
    private array $statsPerAnalysis = [];

    public function addOk(string $message, string $analysisName, ?array $detail = null): void
    {
        $this->ok[] = $message;
        if ($detail) {
            $this->okDetails[$analysisName] = array_merge($this->okDetails[$analysisName] ?? [], $detail);;
        }

        $this->statsPerAnalysis[$analysisName]['ok'] = ($this->statsPerAnalysis[$analysisName]['ok'] ?? 0) + 1;
    }

    public function addNotice(string $message, string $analysisName, ?array $detail = null): void
    {
        $this->notice[] = $message;
        if ($detail) {
            $this->noticeDetails[$analysisName] = array_merge($this->noticeDetails[$analysisName] ?? [], $detail);
        }

        $this->statsPerAnalysis[$analysisName]['notice'] = ($this->statsPerAnalysis[$analysisName]['notice'] ?? 0) + 1;
    }

    public function addWarning(string $message, string $analysisName, ?array $detail = null): void
    {
        $this->warning[] = $message;
        if ($detail) {
            $this->warningDetails[$analysisName] = array_merge($this->warningDetails[$analysisName] ?? [], $detail);
        }

        $this->statsPerAnalysis[$analysisName]['warning'] = ($this->statsPerAnalysis[$analysisName]['warning'] ?? 0) + 1;
    }

    public function addCritical(string $message, string $analysisName, ?array $detail = null): void
    {
        $this->critical[] = $message;
        if ($detail) {
            $this->criticalDetails[$analysisName] = array_merge($this->criticalDetails[$analysisName] ?? [], $detail);;
        }

        $this->statsPerAnalysis[$analysisName]['critical'] = ($this->statsPerAnalysis[$analysisName]['critical'] ?? 0) + 1;
    }

    public function getStatsPerAnalysis(): array
    {
        return $this->statsPerAnalysis;
    }

    public function getOk(): array
    {
        return $this->ok;
    }

    public function getNotice(): array
    {
        return $this->notice;
    }

    public function getWarning(): array
    {
        return $this->warning;
    }

    public function getCritical(): array
    {
        return $this->critical;
    }

    public function getOkDetails(): array
    {
        return $this->okDetails;
    }

    public function getNoticeDetails(): array
    {
        return $this->noticeDetails;
    }

    public function getWarningDetails(): array
    {
        return $this->warningDetails;
    }

    public function getCriticalDetails(): array
    {
        return $this->criticalDetails;
    }

    public function getAllCount(): int
    {
        return count($this->ok) + count($this->notice) + count($this->warning) + count($this->critical);
    }

    public function getDetailsOfSeverityAndAnalysisName(string $severity, string $analysisName): array
    {
        switch ($severity) {
            case 'ok':
                return $this->okDetails[$analysisName] ?? [];
            case 'notice':
                return $this->noticeDetails[$analysisName] ?? [];
            case 'warning':
                return $this->warningDetails[$analysisName] ?? [];
            case 'critical':
                return $this->criticalDetails[$analysisName] ?? [];
            default:
                throw new \InvalidArgumentException('Unknown severity: ' . $severity);
        }
    }

    public function toIconString(string $okIcon = '✅', string $noticeIcon = 'ℹ️', string $warningIcon = '⚠', string $criticalIcon = '⛔'): string
    {
        $result = '';

        $countCritical = count($this->critical);
        $countWarning = count($this->warning);
        $countNotice = count($this->notice);
        $countOk = count($this->ok);

        if ($countCritical) {
            $result .= $countCritical . $criticalIcon . ' ';
        }
        if ($countWarning) {
            $result .= $countWarning . $warningIcon . ' ';
        }
        if ($countNotice) {
            $result .= $countNotice . $noticeIcon . ' ';
        }
        if ($countOk) {
            $result .= $countOk . $okIcon . ' ';
        }

        return trim($result);
    }

    public function toColorizedString(bool $stripWhitespaces = true): string
    {
        $result = '';

        $countCritical = count($this->critical);
        $countWarning = count($this->warning);
        $countNotice = count($this->notice);
        $countOk = count($this->ok);

        if ($countCritical) {
            $result .= Utils::getColorText(strval($countCritical), 'red', true) . ' / ';
        }
        if ($countWarning) {
            $result .= Utils::getColorText(strval($countWarning), 'magenta') . ' / ';
        }
        if ($countNotice) {
            $result .= Utils::getColorText(strval($countNotice), 'blue') . ' / ';
        }
        if ($countOk) {
            $result .= Utils::getColorText(strval($countOk), 'green') . ' / ';
        }

        return $stripWhitespaces ? str_replace(' ', '', trim($result, ' /')) : trim($result, ' /');
    }

    public function toNotColorizedString(bool $stripWhitespaces = true): string
    {
        $result = '';

        $countCritical = count($this->critical);
        $countWarning = count($this->warning);
        $countNotice = count($this->notice);
        $countOk = count($this->ok);

        if ($countCritical) {
            $result .= $countCritical . ' / ';
        }
        if ($countWarning) {
            $result .= $countWarning . ' / ';
        }
        if ($countNotice) {
            $result .= $countNotice . ' / ';
        }
        if ($countOk) {
            $result .= $countOk . ' / ';
        }

        return $stripWhitespaces ? str_replace(' ', '', trim($result, ' /')) : trim($result, ' /');
    }

    public function __toString(): string
    {
        return $this->toColorizedString();
    }

    /**
     * @param string $analysisName
     * @return array|array[]
     */
    public function getAllDetailsForAnalysis(string $analysisName): array
    {
        $result = [
            'ok' => [],
            'notice' => [],
            'warning' => [],
            'critical' => [],
        ];

        foreach ($this->criticalDetails[$analysisName] ?? [] as $detail) {
            $result['critical'][] = $detail;
        }
        foreach ($this->warningDetails[$analysisName] ?? [] as $detail) {
            $result['warning'][] = $detail;
        }
        foreach ($this->noticeDetails[$analysisName] ?? [] as $detail) {
            $result['notice'][] = $detail;
        }
        foreach ($this->okDetails[$analysisName] ?? [] as $detail) {
            $result['ok'][] = $detail;
        }

        return $result;
    }

}