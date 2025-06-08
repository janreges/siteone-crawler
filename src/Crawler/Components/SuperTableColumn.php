<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Components;

use Closure;

class SuperTableColumn
{
    const AUTO_WIDTH = -1;

    public readonly string $aplCode;
    public readonly string $name;
    public readonly int $width;
    public readonly ?Closure $formatter;  // formatter will take column value as argument and return formatted value
    public readonly ?Closure $renderer;   // renderer will take the whole row as argument and return formatted value
    public readonly bool $truncateIfLonger;
    public readonly bool $formatterWillChangeValueLength;
    public readonly bool $nonBreakingSpaces;
    public readonly bool $escapeOutputHtml;
    public readonly ?Closure $getDataValueCallback;
    public ?string $forcedDataType = null;

    /**
     * @param string $aplCode
     * @param string $name
     * @param int $width
     * @param callable|null $formatter
     * @param callable|null $renderer
     * @param bool $truncateIfLonger
     * @param bool $formatterWillChangeValueLength
     * @param bool $nonBreakingSpaces
     * @param bool $escapeOutputHtml
     * @param callable|null $getDataValueCallback
     */
    public function __construct(string $aplCode, string $name, int $width = self::AUTO_WIDTH, ?callable $formatter = null, ?callable $renderer = null, bool $truncateIfLonger = false, bool $formatterWillChangeValueLength = true, bool $nonBreakingSpaces = false, $escapeOutputHtml = true, ?callable $getDataValueCallback = null)
    {
        $this->aplCode = $aplCode;
        $this->name = $name;
        $this->width = $width;
        $this->formatter = $formatter;
        $this->renderer = $renderer;
        $this->truncateIfLonger = $truncateIfLonger;
        $this->formatterWillChangeValueLength = $formatterWillChangeValueLength;
        $this->nonBreakingSpaces = $nonBreakingSpaces;
        $this->escapeOutputHtml = $escapeOutputHtml;
        $this->getDataValueCallback = $getDataValueCallback;
    }

    public function getWidthPx(): int
    {
        return $this->width * 8;
    }

    public function getAutoWidthByData(array $data): int
    {
        $maxWidth = mb_strlen($this->name);
        foreach ($data as $row) {
            $value = is_object($row) ? @$row->{$this->aplCode} : @$row[$this->aplCode];
            if ($value === null || $value === false) {
                continue;
            }

            $value = $this->formatter && $this->formatterWillChangeValueLength ? ($this->formatter)($value) : $value;
            if (is_scalar($value)) {
                $maxWidth = max($maxWidth, mb_strlen(strval($value)));
            } else {
                $maxWidth = 100;
            }
        }
        return min(1000, $maxWidth);
    }

    /**
     * @param array|object $row
     * @return mixed
     */
    public function getDataValue(array|object $row)
    {
        if ($this->getDataValueCallback) {
            return ($this->getDataValueCallback)($row);
        }
        return is_object($row) ? @$row->{$this->aplCode} : @$row[$this->aplCode];
    }

}