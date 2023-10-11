<?php

namespace Crawler\Components;

class SuperTableColumn
{
    public readonly string $aplCode;
    public readonly string $name;
    public readonly int $width;
    public $formatter;
    public $renderer;

    /**
     * @param string $aplCode
     * @param string $name
     * @param int $width
     * @param callable|null $formatter
     * @param callable|null $renderer
     */
    public function __construct(string $aplCode, string $name, int $width, ?callable $formatter = null, ?callable $renderer = null)
    {
        $this->aplCode = $aplCode;
        $this->name = $name;
        $this->width = $width;
        $this->formatter = $formatter;
        $this->renderer = $renderer;
    }

    public function getWidthPx(): int
    {
        return $this->width * 8;
    }

}