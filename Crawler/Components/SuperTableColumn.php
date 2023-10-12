<?php

namespace Crawler\Components;

class SuperTableColumn
{
    public readonly string $aplCode;
    public readonly string $name;
    public readonly int $width;
    public $formatter;
    public $renderer;
    public readonly bool $truncateIfLonger;

    /**
     * @param string $aplCode
     * @param string $name
     * @param int $width
     * @param callable|null $formatter
     * @param callable|null $renderer
     * @param bool $truncateIfLonger
     */
    public function __construct(string $aplCode, string $name, int $width, ?callable $formatter = null, ?callable $renderer = null, bool $truncateIfLonger = false)
    {
        $this->aplCode = $aplCode;
        $this->name = $name;
        $this->width = $width;
        $this->formatter = $formatter;
        $this->renderer = $renderer;
        $this->truncateIfLonger = $truncateIfLonger;
    }

    public function getWidthPx(): int
    {
        return $this->width * 8;
    }

}