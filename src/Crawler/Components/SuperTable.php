<?php

/*
 * This file is part of the SiteOne Crawler.
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

    const RENDER_INTO_HTML = 'html';
    const RENDER_INTO_CONSOLE = 'console';

    public readonly string $aplCode;
    public readonly string $title;
    public readonly ?string $description;
    public readonly ?int $maxRows;
    public readonly ?string $forcedTabLabel;

    private bool $visibleInHtml = true;
    private bool $visibleInJson = true;
    private bool $visibleInConsole = true;
    private ?int $visibleInConsoleRowsLimit = null; // null = no limit, otherwise limit to X rows + message about HTML report with full data
    private bool $showOnlyColumnsWithValues = false;

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
    private ?string $schemeOfHostToStripFromUrls = null;
    private ?string $initialUrl = null;
    private bool $fulltextEnabled = true;
    private int $minRowsForFulltext = 10;
    private bool $ignoreHardRowsLimit = false;
    private bool $maxHardRowsLimitReached = false;

    private static int $hardRowsLimit = 200;

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
     * @param string|null $forcedTabLabel
     */
    public function __construct(string $aplCode, string $title, string $emptyTableMessage, array $columns, bool $positionBeforeUrlTable, ?string $currentOrderColumn = null, string $currentOrderDirection = 'ASC', ?string $description = null, ?int $maxRows = null, ?string $forcedTabLabel = null)
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
        $this->forcedTabLabel = $forcedTabLabel;
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

        $this->applyHardRowsLimit();
        $this->removeColumnsWithEmptyData();
    }

    /**
     * @return string
     */
    public function getHtmlOutput(): string
    {
        if (!$this->visibleInHtml) {
            return '';
        }

        $output = "<h2>" . htmlspecialchars($this->title) . "</h2>";


        if (!$this->data) {
            $output .= "<p>" . htmlspecialchars($this->emptyTableMessage) . "</p>";
            return $output;
        } elseif ($this->description) {
            $output .= strip_tags($this->description, 'p,b,strong,i,em,ul,li,ol,br,a') . "<br>";
        }

        if ($this->isFulltextEnabled()) {
            $output .= '<div class="fulltext-container">';
            $output .= '    <input type="text" class="fulltext" data-uq-id="' . htmlspecialchars($this->uniqueId) . '" style="width: 300px;" placeholder="Fulltext search">';
            $output .= '    <span id="foundRows_' . htmlspecialchars($this->uniqueId) . '" class="found-rows">Found ' . count($this->data) . ' row(s).</span>';
            $output .= '</div>';
        }

        $showMore = count($this->data) > 20;

        $extraClasses = [
            $this->aplCode,
        ];
        if ($showMore) {
            $extraClasses[] = 'table-with-show-more';
        }

        $output .= "<div class='table-container-top" . ($showMore ? ' show-more' : '') . "'>";
        if ($showMore) {
            $output .= "<input id='showMore_" . htmlspecialchars($this->uniqueId) . "' name='showMore' class='show-more-checkbox' type='checkbox' />";
        }
        $output .= "<div class='table-container" . ($showMore ? ' show-more' : '') . "'>";
        $output .= "<table id='" . htmlspecialchars($this->uniqueId) . "' border='1' class='table table-bordered table-hover table-sortable " . implode(' ', $extraClasses) . "' style='border-collapse: collapse;'>";
        $output .= "<thead>";
        foreach ($this->columns as $key => $column) {
            $direction = ($this->currentOrderColumn === $key && $this->currentOrderDirection === 'ASC') ? 'DESC' : 'ASC';
            $arrow = ($this->currentOrderColumn === $key) ? ($this->currentOrderDirection === 'ASC' ? '&nbsp;🔼' : '&nbsp;🔽') : '';

            if ($column->forcedDataType !== null) {
                $dataType = $column->forcedDataType;
            } else if (isset($this->data[0]) && is_array($this->data[0])) {
                $dataType = isset($this->data[0][$key]) && is_numeric($this->data[0][$key]) ? 'number' : 'string';
            } else {
                $dataType = isset($this->data[0]) && isset($this->data[0]->$key) && is_numeric($this->data[0]->$key) ? 'number' : 'string';
            }
            $output .= "<th class='sortable-th' data-key='{$key}' data-type='{$dataType}' data-direction='" . $direction . "' data-label='" . htmlspecialchars($column->name) . "' data-uq-id='" . htmlspecialchars($this->uniqueId) . "'>" . htmlspecialchars($column->name) . "{$arrow}</th>";
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
                    $formattedValue = call_user_func($column->formatter, $value, self::RENDER_INTO_HTML);
                } elseif ($column->renderer) {
                    $formattedValue = call_user_func($column->renderer, $row, self::RENDER_INTO_HTML);
                }

                if ($column->escapeOutputHtml) {
                    $formattedValue = htmlspecialchars(strval($formattedValue));
                }

                if ($column->nonBreakingSpaces && is_string($formattedValue)) {
                    $formattedValue = str_replace([' ', "\t"], ['&nbsp;', str_repeat('&nbsp;', 4)], $formattedValue);
                }

                // colored text
                if (is_string($formattedValue) && (str_contains($formattedValue, '[0;') || str_contains($formattedValue, '[1;') || str_contains($formattedValue, '[0m'))) {
                    $formattedValue = Utils::convertBashColorsInTextToHtml($formattedValue);
                }

                // full URL in value

                if (is_string($formattedValue) && is_string($value) && str_starts_with($value, 'http')) {
                    $formattedValue = "<a href='" . htmlspecialchars($value) . "' target='_blank'>" . Utils::truncateUrl($value, 100, '…', $this->hostToStripFromUrls, $this->schemeOfHostToStripFromUrls, false) . "</a>";
                } // full URL in formatted value
                else if (is_string($formattedValue) && is_string($value) && str_starts_with($formattedValue, 'http')) {
                    $formattedValue = "<a href='" . htmlspecialchars($formattedValue) . "' target='_blank'>" . Utils::truncateUrl($formattedValue, 100, '…', $this->hostToStripFromUrls, $this->schemeOfHostToStripFromUrls, false) . "</a>";
                } // relative URL
                elseif ($initialRootUrl && is_string($formattedValue) && str_starts_with($formattedValue, '/') && preg_match('/^\/[a-z0-9\-_.\/?&#+=%@()|]*$/i', $formattedValue)) {
                    $finalUrl = $initialRootUrl . $formattedValue;
                    $formattedValue = "<a href='" . htmlspecialchars($finalUrl) . "' target='_blank'>" . Utils::truncateUrl($formattedValue, 100, '…', $this->hostToStripFromUrls, $this->schemeOfHostToStripFromUrls, false) . "</a>";
                }

                if ($column->getDataValueCallback !== null) {
                    $dataValue = strval($column->getDataValue($row));
                } else {
                    $dataValue = is_scalar($value) && strlen(strval($value)) < 200 ? strval($value) : (strlen(strval($formattedValue)) < 50 ? strval($formattedValue) : 'complex-data');
                }
                $output .= "<td data-value='" . htmlspecialchars($dataValue) . "' class='" . htmlspecialchars($key) . "'>{$formattedValue}</td>";
            }
            $output .= "</tr>";
            $counter++;
        }
        if (empty($this->data)) {
            $output .= "<tr><td colspan='" . count($this->columns) . "' class='warning'>" . htmlspecialchars($this->emptyTableMessage) . "</td></tr>";
        } else if ($maxRowsReached) {
            $output .= "<tr><td colspan='" . count($this->columns) . "' class='warning'>You have reached the limit of {$this->maxRows} rows as a protection against very large output or exhausted memory.</td></tr>";
        } else if ($this->maxHardRowsLimitReached) {
            $output .= "<tr><td colspan='" . count($this->columns) . "' class='warning'>You have reached the hard limit of " . self::$hardRowsLimit . " rows as a protection against very large output or exhausted memory. You can change this with <code>--rows-limit</code>.</td></tr>";
        }

        $output .= "</tbody>";
        if ($this->isFulltextEnabled()) {
            $output .= "<tfoot>";
            $output .= "  <tr class='empty-fulltext'><td colspan='" . count($this->columns) . "' class='warning'>No rows found, please edit your search term.</td></tr>";
            $output .= "</tfoot>";
        }
        $output .= "</table></div>";

        // unfortunately, we need to add this next to table because tbody ignores "max-height" and "display: block"
        // will cause that tbody disconnect from thead, so columns will not be aligned properly
        if ($showMore) {
            $output .= "<label for='showMore_" . htmlspecialchars($this->uniqueId) . "' class='show-more-label'>(+) Show entire table</label>";
        }
        $output .= "</div>";

        return $output;
    }

    /**
     * @return string
     */
    public function getConsoleOutput(): string
    {
        $titleOutput = $this->title . PHP_EOL . str_repeat('-', mb_strlen($this->title)) . PHP_EOL . PHP_EOL;;
        $output = Utils::getColorText($titleOutput, 'blue');

        $data = $this->data;

        if (!$data) {
            $output .= Utils::getColorText($this->emptyTableMessage, 'gray') . PHP_EOL . PHP_EOL;
            return $output;
        } else if (!$this->visibleInConsole) {
            $output .= Utils::getColorText("This table contains large data. To see them, use output to HTML using `--output-html-report=tmp/myreport.html`.", 'yellow') . PHP_EOL . PHP_EOL;
            return $output;
        } elseif ($this->visibleInConsoleRowsLimit) {
            $output .= Utils::getColorText("This table contains large data and shows max {$this->visibleInConsoleRowsLimit} rows. To see them all, use output to HTML using `--output-html-report=tmp/myreport.html`.", 'yellow') . PHP_EOL . PHP_EOL;
            $data = array_slice($data, 0, $this->visibleInConsoleRowsLimit);
        }

        $columnToWidth = [];
        foreach ($this->columns as $column) {
            $columnToWidth[$column->aplCode] = $column->width === SuperTableColumn::AUTO_WIDTH
                ? $column->getAutoWidthByData($this->data)
                : $column->width;
        }

        $headers = [];
        foreach ($this->columns as $column) {
            $headers[] = Utils::mb_str_pad($column->name, $columnToWidth[$column->aplCode]);
        }
        $output .= Utils::getColorText(implode(' | ', $headers), 'gray') . PHP_EOL;

        $repeat = array_sum(array_map(function ($column) use ($columnToWidth) {
                return $columnToWidth[$column->aplCode];
            }, $this->columns)) + (count($this->columns) * 3) - 1;
        $output .= str_repeat('-', $repeat) . PHP_EOL;

        foreach ($data as $row) {
            $rowData = [];
            foreach ($this->columns as $key => $column) {
                $value = is_object($row) ? ($row->{$key} ?? '') : ($row[$key] ?? '');
                $columnWidth = $columnToWidth[$column->aplCode];
                if (isset($column->formatter)) {
                    $value = call_user_func($column->formatter, $value, self::RENDER_INTO_CONSOLE);
                } else if (isset($column->renderer)) {
                    $value = call_user_func($column->renderer, $row, self::RENDER_INTO_CONSOLE);
                }

                if ($column->truncateIfLonger && $value && mb_strlen(strval($value)) > $columnWidth) {
                    $value = Utils::truncateInTwoThirds($value, $columnWidth);
                }

                $rowData[] = $column->formatterWillChangeValueLength
                    ? Utils::mb_str_pad(strval($value), $columnWidth, ' ')
                    : ($value . (str_repeat(' ', max(0, $columnWidth - mb_strlen(Utils::removeAnsiColors(strval($value)))))));
            }
            $output .= implode(' | ', $rowData) . PHP_EOL;
        }
        $output .= PHP_EOL;

        return $output;
    }

    public function getJsonOutput(): ?array
    {
        if (!$this->visibleInJson) {
            return null;
        }

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

    public function setHostToStripFromUrls(?string $hostToStripFromUrls, ?string $schemeOfHostToStripFromUrls): void
    {
        $this->hostToStripFromUrls = $hostToStripFromUrls;
        $this->schemeOfHostToStripFromUrls = $schemeOfHostToStripFromUrls;
    }

    public function setInitialUrl(?string $initialUrl): void
    {
        $this->initialUrl = $initialUrl;
    }

    public function setVisibilityInHtml(bool $visibleInHtml): void
    {
        $this->visibleInHtml = $visibleInHtml;
    }

    public function setVisibilityInConsole(bool $visibleInConsole, ?int $visibleInConsoleRowsLimit): void
    {
        $this->visibleInConsole = $visibleInConsole;
        $this->visibleInConsoleRowsLimit = $visibleInConsoleRowsLimit;
    }

    public function setVisibilityInJson(bool $visibleInJson): void
    {
        $this->visibleInJson = $visibleInJson;
    }

    public function isVisibleInHtml(): bool
    {
        return $this->visibleInHtml;
    }

    public function isVisibleInConsole(): bool
    {
        return $this->visibleInConsole;
    }

    public function isVisibleInJson(): bool
    {
        return $this->visibleInJson;
    }

    public function disableFulltext(): void
    {
        $this->fulltextEnabled = false;
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

    private function isFulltextEnabled(): bool
    {
        return $this->fulltextEnabled && count($this->data) >= $this->minRowsForFulltext;
    }

    public function setShowOnlyColumnsWithValues(bool $showOnlyColumnsWithValues): void
    {
        $this->showOnlyColumnsWithValues = $showOnlyColumnsWithValues;
    }

    private function removeColumnsWithEmptyData(): void
    {
        if (!$this->showOnlyColumnsWithValues) {
            return;
        }

        $columnsToRemove = [];
        foreach ($this->columns as $column) {
            $columnHasData = false;
            foreach ($this->data as $row) {
                $value = is_object($row) ? ($row->{$column->aplCode} ?? '') : ($row[$column->aplCode] ?? '');
                if (trim((string)$value, ' 0.,') !== '') {
                    $columnHasData = true;
                    break;
                }
            }
            if (!$columnHasData) {
                $columnsToRemove[] = $column->aplCode;
            }
        }

        foreach ($columnsToRemove as $columnToRemove) {
            unset($this->columns[$columnToRemove]);
            foreach ($this->data as &$row) {
                unset($row[$columnToRemove]);
            }
        }
    }

    /**
     * @return SuperTableColumn[]
     */
    public function getColumns(): array
    {
        return $this->columns;
    }

    public static function setHardRowsLimit(int $hardRowsLimit): void
    {
        self::$hardRowsLimit = $hardRowsLimit;
    }

    public function setIgnoreHardRowsLimit(bool $ignoreHardRowsLimit): void
    {
        $this->ignoreHardRowsLimit = $ignoreHardRowsLimit;
    }

    private function applyHardRowsLimit(): void
    {
        if (self::$hardRowsLimit && !$this->ignoreHardRowsLimit && count($this->data) > self::$hardRowsLimit) {
            $this->data = array_slice($this->data, 0, self::$hardRowsLimit);
            $this->maxHardRowsLimitReached = true;
        }
    }
}
