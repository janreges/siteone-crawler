<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class Debugger
{
    const DEBUG = 'debug';
    const INFO = 'info';
    const NOTICE = 'notice';
    const WARNING = 'warning';
    const CRITICAL = 'critical';

    private static bool $debug = false;
    private static bool $debugPrintToOutput = false;
    private static ?string $debugLogFile = null;

    /**
     * When debug mode is enabled, this method will print message to console and log to file
     *
     * @param string $category
     * @param string $message
     * @param array|null $messageParams
     * @param string $severity
     * @param float|null $time
     * @param int|null $size
     * @return void
     */
    public static function debug(string $category, string $message, ?array $messageParams = [], string $severity = 'debug', ?float $time = null, ?int $size = null): void
    {
        if (self::$debug) {
            $finalMessage = sprintf(
                "%s | %s | %s | ",
                date('Y-m-d H:i:s'),
                str_pad($severity, 8),
                str_pad($category, 14)
            );
            if ($time !== null) {
                $finalMessage .= str_pad(Utils::getFormattedDuration($time), 7) . ' | ';
            }
            if ($size !== null) {
                $finalMessage .= str_pad(Utils::getFormattedSize($size), 7) . ' | ';
            }

            $finalMessage .= $messageParams ? sprintf($message, ...$messageParams) : $message;
            self::print($finalMessage);
            self::log($finalMessage);
        }
    }

    public static function consoleArrayDebug(array $rowData, array $colWidths = []): void
    {
        if (!$colWidths) {
            $consoleWidth = Utils::getConsoleWidth();
            $colWidth = floor($consoleWidth / count($rowData));
            $colWidths = array_fill(0, count($rowData), $colWidth);
        }
        $colWidths = array_map(function ($width) {
            return max($width, 10);
        }, $colWidths);

        $row = [];
        foreach ($rowData as $i => $value) {
            $colWidth = $colWidths[$i];
            if (mb_strlen($value) > $colWidth) {
                $value = Utils::truncateInTwoThirds($value, $colWidth, '..');
            }
            $row[] = str_pad($value, $colWidth);
        }

        $message = implode(' | ', $row);
        self::print($message);
        self::log($message);
    }

    public static function forceEnabledDebug(?string $logFile): void
    {
        self::$debug = true;
        self::$debugPrintToOutput = true;
        if ($logFile) {
            self::$debugLogFile = $logFile;
        }
    }

    /**
     * Set debugger configuration - enable/disable debug mode, printing to output and set debug log file
     *
     * @param bool $debug
     * @param string|null $debugLogFile
     * @return void
     */
    public static function setConfig(bool $debug, ?string $debugLogFile): void
    {
        if ($debug) {
            self::$debug = true;
            self::$debugPrintToOutput = true;
            self::$debugLogFile = $debugLogFile;
        } elseif ($debugLogFile) {
            // when debug is disabled but debugLogFile is set, logging to file is enabled but printing to output is not
            self::$debug = true;
            self::$debugPrintToOutput = false;
            self::$debugLogFile = $debugLogFile;
        }
    }

    /**
     * Print message to console if debugPrintToOutput is set
     *
     * @param string $message
     * @return void
     */
    private static function print(string $message): void
    {
        if (self::$debugPrintToOutput) {
            echo $message . PHP_EOL;
        }
    }

    /**
     * Log message to debug log file if debugLogFile is set
     *
     * @param string $message
     * @return void
     */
    private static function log(string $message): void
    {
        if (self::$debugLogFile) {
            file_put_contents(Utils::getAbsolutePath(self::$debugLogFile), $message . "\n", FILE_APPEND);
        }
    }

}