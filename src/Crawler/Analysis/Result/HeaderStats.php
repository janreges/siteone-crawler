<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

use Crawler\Utils;

class HeaderStats
{

    /**
     * Maximum number of unique values to store to prevent memory overflow
     */
    const MAX_UNIQUE_VALUES = 20;

    public readonly string $header;

    public int $occurrences = 0;

    /**
     * Unique values and their count
     * @var int[] [string => int]
     */
    public array $uniqueValues = [];
    public bool $uniqueValuesLimitReached = false;

    public ?string $minDateValue = null;
    public ?string $maxDateValue = null;

    public ?int $minIntValue = null;
    public ?int $maxIntValue = null;

    public function __construct(string $header)
    {
        $this->header = $header;
    }

    public function addValue(string|array $value): void
    {
        $this->occurrences++;

        if ($this->ignoreHeaderValues($this->header)) {
            return;
        } elseif (is_string($value) && $this->isValueForMinMaxDate($this->header)) {
            $this->addValueForMinMaxDate($value);
        } elseif (is_string($value) && $this->isValueForMinMaxInt($this->header)) {
            $this->addValueForMinMaxInt($value);
        } else {
            if (count($this->uniqueValues) >= self::MAX_UNIQUE_VALUES) {
                $this->uniqueValuesLimitReached = true;
                return;
            }

            if (is_array($value)) {
                $value = json_encode($value);
            }

            if (!isset($this->uniqueValues[$value])) {
                $this->uniqueValues[$value] = 0;
            }
            $this->uniqueValues[$value]++;
        }
    }

    /**
     * @return int[] [string => int]
     */
    public function getSortedUniqueValues(): array
    {
        arsort($this->uniqueValues);
        return $this->uniqueValues;
    }

    public function getFormattedHeaderName(): string
    {
        $words = explode('-', $this->header);
        foreach ($words as &$word) {
            $word = ucfirst($word);
        }
        return str_replace('Xss', 'XSS', implode('-', $words));
    }

    public function isValueForMinMaxInt(string $header): bool
    {
        return $header === 'content-length' || $header === 'age';
    }

    public function isValueForMinMaxDate(string $header): bool
    {
        return $header === 'date' || $header === 'expires' || $header === 'last-modified';
    }

    public function ignoreHeaderValues(string $header): bool
    {
        static $ignoredHeaders = [
            'etag',
            'cf-ray',
            'set-cookie',
            'content-disposition',
        ];
        return in_array($header, $ignoredHeaders, true);
    }

    public function __get($name)
    {
        if ($name === 'minValue') {
            return $this->minIntValue ?? $this->minDateValue;
        } elseif ($name === 'maxValue') {
            return $this->maxIntValue ?? $this->maxDateValue;
        } elseif ($name === 'valuesPreview') {
            return $this->getValuesPreview();
        } else {
            throw new \Exception("Unknown property '{$name}'");
        }
    }

    private function addValueForMinMaxInt(string $value): void
    {
        $int = @intval($value);
        if ($this->minIntValue === null || $int < $this->minIntValue) {
            $this->minIntValue = $int;
        }
        if ($this->maxIntValue === null || $int > $this->maxIntValue) {
            $this->maxIntValue = $int;
        }
    }

    private function addValueForMinMaxDate(string $value): void
    {
        $timestamp = @strtotime($value);
        if (!$timestamp) {
            return;
        }

        $date = @date('Y-m-d', $timestamp);
        if (trim(strval($date)) === '') {
            return;
        }
        if ($this->minDateValue === null || $date < $this->minDateValue) {
            $this->minDateValue = $date;
        }
        if ($this->maxDateValue === null || $date > $this->maxDateValue) {
            $this->maxDateValue = $date;
        }
    }

    public function getValuesPreview(int $maxLength = 120): string
    {
        if (count($this->uniqueValues) === 1) {
            $firstValue = array_key_first($this->uniqueValues);
            if (is_string($firstValue) && mb_strlen($firstValue) > $maxLength) {
                return Utils::truncateInTwoThirds($firstValue, $maxLength);
            }
            return (string)$firstValue;
        }

        $valuesLength = array_reduce(array_keys($this->uniqueValues), function ($carry, $item) {
            return $carry + strlen((string)$item);
        }, 0);

        if ($valuesLength < $maxLength - 10) {
            $result = '';
            arsort($this->uniqueValues);
            foreach ($this->uniqueValues as $value => $count) {
                $result .= $value . ' (' . $count . ') / ';
            }

            if (trim($result) === '') {
                return '[ignored generic values]';
            }

            return Utils::truncateInTwoThirds(rtrim($result, ' /'), $maxLength);
        }

        return '[see values below]';
    }

}