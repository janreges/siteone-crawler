<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Components;

use Crawler\Utils;

class SuperTable
{

    const POSITION_BEFORE_URL_TABLE = 'before-url-table';
    const POSITION_AFTER_URL_TABLE = 'after-url-table';

    public readonly string $aplCode;
    public readonly string $title;
    public readonly ?string $description;
    public readonly ?int $maxRows;

    /**
     * @var SuperTableColumn[]
     */
    private array $columns;
    private bool $positionBeforeUrlTable;
    private array $data;
    private string $emptyTableMessage;
    private ?string $currentOrderColumn;
    private string $currentOrderDirection = 'ASC';
    private string $uniqueId;
    private ?string $hostToStripFromUrls = null;
    private ?string $initialUrl = null;

    /**
     * @param string $aplCode
     * @param string $title
     * @param string $emptyTableMessage
     * @param SuperTableColumn[] $columns
     * @param bool $positionBeforeUrlTable
     * @param string|null $currentOrderColumn
     * @param string $currentOrderDirection
     * @param string|null $description
     * @param int|null $maxRows
     */
    public function __construct(string $aplCode, string $title, string $emptyTableMessage, array $columns, bool $positionBeforeUrlTable, ?string $currentOrderColumn = null, string $currentOrderDirection = 'ASC', ?string $description = null, ?int $maxRows = null)
    {
        foreach ($columns as $column) {
            if (!($column instanceof SuperTableColumn)) {
                throw new \InvalidArgumentException('All columns must be instance of SuperTableColumn');
            }
        }

        $this->aplCode = $aplCode;
        $this->title = $title;
        $this->emptyTableMessage = $emptyTableMessage;
        $this->columns = [];
        foreach ($columns as $column) {
            $this->columns[$column->aplCode] = $column;
        }
        $this->positionBeforeUrlTable = $positionBeforeUrlTable;
        $this->currentOrderColumn = $currentOrderColumn;
        $this->currentOrderDirection = $currentOrderDirection;
        $this->description = $description;
        $this->maxRows = $maxRows;
        $this->uniqueId = 't' . substr(md5(strval(rand(1000000, 9999999))), 0, 6);
    }

    /**
     * @param array $data
     * @return void
     */
    public function setData(array $data)
    {
        $this->data = $data;
        if ($this->currentOrderColumn) {
            $this->sortData($this->currentOrderColumn, $this->currentOrderDirection);
        }
    }

    /**
     * @return string
     */
    public function getHtmlOutput(): string
    {
        $output = "<h2>" . htmlspecialchars($this->title) . "</h2>";
        if (!$this->data) {
            $output .= "<p>" . htmlspecialchars($this->emptyTableMessage) . "</p>";
            return $output;
        } elseif ($this->description) {
            $output .= strip_tags($this->description, 'p,b,strong,i,em,ul,li,ol,br,a') . "<br>";
        }

        $output .= "<table id='" . htmlspecialchars($this->uniqueId) . "' border='1' class='table table-bordered table-hover table-sortable' style='border-collapse: collapse;'>";
        $output .= "<thead>";
        foreach ($this->columns as $key => $column) {
            $width = $column->width === SuperTableColumn::AUTO_WIDTH
                ? $column->getAutoWidthByData($this->data)
                : $column->width;
            $widthPx = min($width * 12, 800);
            $direction = ($this->currentOrderColumn === $key && $this->currentOrderDirection === 'ASC') ? 'DESC' : 'ASC';
            $arrow = ($this->currentOrderColumn === $key) ? ($this->currentOrderDirection === 'ASC' ? '&nbsp;🔼' : '&nbsp;🔽') : '';

            if (isset($this->data[0]) && is_array($this->data[0])) {
                $dataType = isset($this->data[0][$key]) && is_numeric($this->data[0][$key]) ? 'number' : 'string';
            } else {
                $dataType = isset($this->data[0]) && isset($this->data[0]->$key) && is_numeric($this->data[0]->$key) ? 'number' : 'string';
            }
            $output .= "<th data-key='{$key}' data-type='{$dataType}' data-direction='" . $direction . "' data-label='" . htmlspecialchars($column->name) . "' style='width:{$widthPx}px' onclick='sortTable(\"" . htmlspecialchars($this->uniqueId) . "\", \"" . htmlspecialchars($key) . "\")'>" . htmlspecialchars($column->name) . "{$arrow}</th>";
        }

        $initialRootUrl = $this->initialUrl ? preg_replace('/^(https?:\/\/[^\/]+).*$/', '$1', $this->initialUrl) : null;

        $output .= "</thead>";
        $output .= "<tbody>";
        $counter = 1;
        $maxRowsReached = false;
        foreach ($this->data as $row) {
            if ($this->maxRows && $counter > $this->maxRows) {
                $maxRowsReached = true;
                break;
            }
            $output .= "<tr>";
            foreach ($this->columns as $key => $column) {
                $value = is_object($row) ? ($row->{$key} ?? '') : ($row[$key] ?? '');
                $formattedValue = $value;

                if ($column->formatter) {
                    $formattedValue = call_user_func($column->formatter, $value);
                } elseif ($column->renderer) {
                    $formattedValue = call_user_func($column->renderer, $row);
                } else {
                    if ($column->nonBreakingSpaces && is_string($formattedValue)) {
                        $formattedValue = str_replace([' ', "\t"], ['&nbsp;', str_repeat('&nbsp;', 4)], $formattedValue);
                    }
                }

                // colored text
                if (is_string($formattedValue) && (str_contains($formattedValue, '[0;') || str_contains($formattedValue, '[1;') || str_contains($formattedValue, '[0m'))) {
                    $formattedValue = Utils::convertBashColorsInTextToHtml($formattedValue);
                }

                // full URL in value
                if (is_string($formattedValue) && is_string($value) && str_starts_with($value, 'http')) {
                    $formattedValue = "<a href='" . htmlspecialchars($value) . "' target='_blank'>" . Utils::truncateUrl($value, 100, '...', $this->hostToStripFromUrls) . "</a>";
                } // full URL in formatted value
                else if (is_string($formattedValue) && is_string($value) && str_starts_with($formattedValue, 'http')) {
                    $formattedValue = "<a href='" . htmlspecialchars($formattedValue) . "' target='_blank'>" . Utils::truncateUrl($formattedValue, 100, '...', $this->hostToStripFromUrls) . "</a>";
                } // relative URL
                elseif ($initialRootUrl && is_string($formattedValue) && str_starts_with($formattedValue, '/') && preg_match('/^\/[a-z0-9\-_.\/?&#+=%@()|]+$/i', $formattedValue)) {
                    $finalUrl = $initialRootUrl . $formattedValue;
                    $formattedValue = "<a href='" . htmlspecialchars($finalUrl) . "' target='_blank'>" . Utils::truncateUrl($formattedValue, 100, '...', $this->hostToStripFromUrls) . "</a>";
                }

                $output .= "<td data-value='" . htmlspecialchars(is_scalar($value) && strlen(strval($value)) < 200 ? strval($value) : 'complex-data') . "'>{$formattedValue}</td>";
            }
            $output .= "</tr>";
            $counter++;
        }
        if (empty($this->data)) {
            $output .= "<tr><td colspan='" . count($this->columns) . "' class='warning'>" . htmlspecialchars($this->emptyTableMessage) . "</td></tr>";
        } else if ($maxRowsReached) {
            $output .= "<tr><td colspan='" . count($this->columns) . "' class='warning'>You have reached the limit of {$this->maxRows} rows as a protection against very large output or exhausted memory.</td></tr>";
        }
        $output .= "</tbody>";
        $output .= "</table>";

        return $output;
    }

    /**
     * @return string
     */
    public function getConsoleOutput(): string
    {
        $titleOutput = $this->title . PHP_EOL . str_repeat('-', mb_strlen($this->title)) . PHP_EOL . PHP_EOL;;
        $output = Utils::getColorText($titleOutput, 'blue');

        if (!$this->data) {
            $output .= Utils::getColorText($this->emptyTableMessage, 'gray') . PHP_EOL . PHP_EOL;
            return $output;
        }

        $columnToWidth = [];
        foreach ($this->columns as $column) {
            $columnToWidth[$column->aplCode] = $column->width === SuperTableColumn::AUTO_WIDTH
                ? $column->getAutoWidthByData($this->data)
                : $column->width;
        }

        $headers = [];
        foreach ($this->columns as $column) {
            $headers[] = str_pad($column->name, $columnToWidth[$column->aplCode]);
        }
        $output .= Utils::getColorText(implode(' | ', $headers), 'gray') . PHP_EOL;

        $repeat = array_sum(array_map(function ($column) use ($columnToWidth) {
                return $columnToWidth[$column->aplCode];
            }, $this->columns)) + (count($this->columns) * 3) - 1;
        $output .= str_repeat('-', $repeat) . PHP_EOL;

        foreach ($this->data as $row) {
            $rowData = [];
            foreach ($this->columns as $key => $column) {
                $value = is_object($row) ? ($row->{$key} ?? '') : ($row[$key] ?? '');
                $valueLength = mb_strlen(strval($value));
                $columnWidth = $columnToWidth[$column->aplCode];
                if (isset($column->formatter)) {
                    $value = call_user_func($column->formatter, $value);
                } else if (isset($column->renderer)) {
                    $value = call_user_func($column->renderer, $row);
                }

                if ($column->truncateIfLonger && $value && mb_strlen($value) > $columnWidth) {
                    $value = Utils::truncateInTwoThirds($value, $columnWidth);
                }

                $rowData[] = $column->formatterWillChangeValueLength
                    ? str_pad(strval($value), $columnWidth)
                    : ($value . (str_repeat(' ', max(0, $columnWidth - $valueLength))));
            }
            $output .= implode(' | ', $rowData) . PHP_EOL;
        }
        $output .= PHP_EOL;

        return $output;
    }

    public function getJsonOutput(): array
    {
        return [
            'aplCode' => $this->aplCode,
            'title' => $this->title,
            'columns' => $this->columns,
            'rows' => $this->data,
            'position' => $this->positionBeforeUrlTable ? self::POSITION_BEFORE_URL_TABLE : self::POSITION_AFTER_URL_TABLE,
        ];
    }

    public function isPositionBeforeUrlTable(): bool
    {
        return $this->positionBeforeUrlTable;
    }

    /**
     * @return mixed[]
     */
    public function getData(): array
    {
        return $this->data;
    }

    public function getTotalRows(): int
    {
        return count($this->data);
    }

    public function setHostToStripFromUrls(?string $hostToStripFromUrls): void
    {
        $this->hostToStripFromUrls = $hostToStripFromUrls;
    }

    public function setInitialUrl(?string $initialUrl): void
    {
        $this->initialUrl = $initialUrl;
    }

    private function sortData(string $columnKey, string $direction): void
    {
        $direction = strtoupper($direction);
        usort($this->data, function ($a, $b) use ($columnKey, $direction) {
            $aValue = is_object($a) ? ($a->{$columnKey} ?? '') : ($a[$columnKey] ?? '');
            $bValue = is_object($b) ? ($b->{$columnKey} ?? '') : ($b[$columnKey] ?? '');

            if ($direction === 'ASC') {
                return $aValue > $bValue ? 1 : ($aValue < $bValue ? -1 : 0);
            } else {  // DESC
                return $aValue < $bValue ? 1 : ($aValue > $bValue ? -1 : 0);
            }
        });
    }
}
