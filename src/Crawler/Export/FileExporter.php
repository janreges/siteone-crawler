<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\OutputType;
use Crawler\Output\TextOutput;
use Crawler\Utils;
use Exception;

class FileExporter extends BaseExporter implements Exporter
{
    const GROUP_FILE_EXPORT_SETTINGS = 'file-export-settings';

    protected ?string $outputHtmlReport = null;
    protected ?string $htmlReportOptions = null;
    protected ?string $outputJsonFile = null;
    protected ?string $outputTextFile = null;
    protected bool $addTimestampToOutputFile = false;
    protected bool $addHostToOutputFile = false;

    public function shouldBeActivated(): bool
    {
        return $this->outputHtmlReport || $this->outputJsonFile || $this->outputTextFile;
    }

    /**
     * @return void
     * @throws Exception
     */
    public function export(): void
    {
        $multiOutput = $this->crawler->getOutput();
        if (!($multiOutput instanceof MultiOutput)) {
            throw new Exception(__METHOD__ . ': MultiOutput expected');
        }

        /* @var $multiOutput MultiOutput */

        // text file
        if ($this->outputTextFile) {
            $s = microtime(true);
            $textOutput = $multiOutput->getOutputByType(OutputType::TEXT);
            if (!($textOutput instanceof TextOutput)) {
                throw new Exception(__METHOD__ . ': TextOutput expected');
            }

            /* @var $textOutput TextOutput */
            $reportFile = $this->getExportFilePath($this->outputTextFile, 'txt');
            file_put_contents(
                $reportFile,
                Utils::removeAnsiColors($textOutput->getOutputText())
            );

            $reportFileForOutput = Utils::getOutputFormattedPath($reportFile);
            $this->status->addInfoToSummary('export-to-text', "Text report saved to '{$reportFileForOutput}' and took " . Utils::getFormattedDuration(microtime(true) - $s));
        }

        $jsonOutput = null;
        /* @var $jsonOutput JsonOutput */

        // json file
        if ($this->outputJsonFile) {
            $s = microtime(true);
            $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
            if (!($jsonOutput instanceof JsonOutput)) {
                throw new Exception(__METHOD__ . ': JsonOutput expected');
            }

            /* @var $jsonOutput JsonOutput */
            $reportFile = $this->getExportFilePath($this->outputJsonFile, 'json');
            file_put_contents(
                $reportFile,
                $jsonOutput->getJson()
            );

            $reportFileForOutput = Utils::getOutputFormattedPath($reportFile);
            $this->status->addInfoToSummary('export-to-json', "JSON report saved to '{$reportFileForOutput}' and took " . Utils::getFormattedDuration(microtime(true) - $s));
        }

        // html file
        if ($this->outputHtmlReport) {
            $s = microtime(true);
            $htmlReport = new HtmlReport($this->status, 5, $this->htmlReportOptions);
            $htmlReportBody = $htmlReport->getHtml();
            $reportFile = $this->getExportFilePath($this->outputHtmlReport, 'html');
            file_put_contents(
                $reportFile,
                $htmlReportBody
            );

            $reportFileForOutput = Utils::getOutputFormattedPath($reportFile);
            $this->status->addInfoToSummary('export-to-html', "HTML report saved to '{$reportFileForOutput}' and took " . Utils::getFormattedDuration(microtime(true) - $s));
        }
    }

    /**
     * @param string $file
     * @param string $extension
     * @return string
     * @throws Exception
     */
    private function getExportFilePath(string $file, string $extension): string
    {
        $hasExtension = preg_match('/\.[a-z0-9]{1,10}$/i', $file) === 1;
        if (!$hasExtension) {
            $file .= ".{$extension}";
        }
        if ($this->addHostToOutputFile) {
            $host = $this->crawler->getInitialParsedUrl()->host;
            $file = preg_replace('/\.[a-z0-9]{1,10}$/i', '.' . $host . '$0', $file);
        }
        if ($this->addTimestampToOutputFile) {
            $file = preg_replace('/\.[a-z0-9]{1,10}$/i', '.' . date('Y-m-d.H-i-s') . '$0', $file);
        }

        if (!is_writable(dirname($file)) && !is_writable($file)) {
            throw new Exception("Output {$extension} file {$file} is not writable. Check permissions.");
        }

        return $file;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_FILE_EXPORT_SETTINGS,
            'File export settings', [
            new Option('--output-html-report', null, 'outputHtmlReport', Type::FILE, false, "Save HTML report into that file. Set to empty '' to disable HTML report.", 'tmp/%domain%.report.%datetime%.html', true),
            new Option('--html-report-options', null, 'htmlReportOptions', Type::STRING, false, "Comma-separated list of sections to include in HTML report. Available sections: summary, seo-opengraph, image-gallery, video-gallery, visited-urls, dns-ssl, crawler-stats, crawler-info, headers, content-types, skipped-urls, caching, best-practices, accessibility, security, redirects, 404-pages, slowest-urls, fastest-urls, source-domains. Default: all sections.", null, true),
            new Option('--output-json-file', null, 'outputJsonFile', Type::FILE, false, "Save report as JSON. Set to empty '' to disable JSON report.", 'tmp/%domain%.output.%datetime%.json', true),
            new Option('--output-text-file', null, 'outputTextFile', Type::FILE, false, "Save output as TXT. Set to empty '' to disable TXT report.", 'tmp/%domain%.output.%datetime%.txt', true),
            new Option('--add-host-to-output-file', null, 'addHostToOutputFile', Type::BOOL, false, 'Append initial URL host to filename except sitemaps.', false, false),
            new Option('--add-timestamp-to-output-file', null, 'addTimestampToOutputFile', Type::BOOL, false, 'Append timestamp to filename except sitemaps.', false, false),
        ]));
        return $options;
    }


}