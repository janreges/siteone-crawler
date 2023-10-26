<?php

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

    /**
     * @param string $aplCode
     * @param string $name
     * @param int $width
     * @param callable|null $formatter
     * @param callable|null $renderer
     * @param bool $truncateIfLonger
     * @param bool $formatterWillChangeValueLength
     */
    public function __construct(string $aplCode, string $name, int $width = self::AUTO_WIDTH, ?callable $formatter = null, ?callable $renderer = null, bool $truncateIfLonger = false, bool $formatterWillChangeValueLength = true)
    {
        $this->aplCode = $aplCode;
        $this->name = $name;
        $this->width = $width;
        $this->formatter = $formatter;
        $this->renderer = $renderer;
        $this->truncateIfLonger = $truncateIfLonger;
        $this->formatterWillChangeValueLength = $formatterWillChangeValueLength;
    }

    public function getWidthPx(): int
    {
        return $this->width * 8;
    }

    public function getAutoWidthByData(array $data): int
    {
        $maxWidth = 0;
        foreach ($data as $row) {
            $value = is_object($row) ? $row->{$this->aplCode} : $row[$this->aplCode];
            $value = $this->formatter && $this->formatterWillChangeValueLength ? ($this->formatter)($value) : $value;
            $maxWidth = max($maxWidth, mb_strlen($value));
        }
        return $maxWidth;
    }

}