<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

class SecurityCheckedHeader
{

    const OK = 1;
    const NOTICE = 2;
    const WARNING = 3;
    const CRITICAL = 4;

    public readonly string $header;
    public ?int $highestSeverity = null;

    /**
     * @var int[] [severity => count]
     */
    public array $countPerSeverity = [];

    /**
     * All unique values of this header
     * @var string[]
     */
    public array $values = [];

    /**
     * @var string[]
     */
    public array $recommendations = [];

    /**
     * @param string $header
     */
    public function __construct(string $header)
    {
        $this->header = $header;
    }

    /**
     * @param array|string|null $value
     * @param int $severity
     * @param string|null $recommendation
     * @return void
     */
    public function setFinding(array|string|null $value, int $severity, ?string $recommendation): void
    {
        if ($value !== null) {
            if (is_array($value)) {
                $value = implode(', ', $value);
            }
            $this->values[$value] = $value;
        }
        if ($recommendation !== null) {
            $this->recommendations[$recommendation] = $recommendation;
        }
        if ($severity > $this->highestSeverity || $this->highestSeverity === null) {
            $this->highestSeverity = $severity;
        }

        if (!isset($this->countPerSeverity[$severity])) {
            $this->countPerSeverity[$severity] = 1;
        } else {
            $this->countPerSeverity[$severity]++;
        }
    }

    public function getFormattedHeader(): string
    {
        $words = explode('-', $this->header);
        foreach ($words as &$word) {
            $word = ucfirst($word);
        }
        return str_replace('Xss', 'XSS', implode('-', $words));
    }

    public function getSeverityName(): string
    {
        return match ($this->highestSeverity) {
            self::OK => 'ok',
            self::NOTICE => 'notice',
            self::WARNING => 'warning',
            self::CRITICAL => 'critical',
            default => 'unknown',
        };
    }

}