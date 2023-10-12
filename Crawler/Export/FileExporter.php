<?php

namespace Crawler\Export;

use Crawler\HtmlReport;
use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\OutputType;
use Crawler\Output\TextOutput;
use Crawler\ParsedUrl;
use Crawler\Utils;
use Exception;

class FileExporter extends BaseExporter implements Exporter
{
    const GROUP_FILE_EXPORT_SETTINGS = 'file-export-settings';

    protected ?string $outputHtmlFile = null;
    protected ?string $outputJsonFile = null;
    protected ?string $outputTextFile = null;
    protected bool $addTimestampToOutputFile = false;
    protected bool $addHostToOutputFile = false;

    public function shouldBeActivated(): bool
    {
        return $this->outputHtmlFile || $this->outputJsonFile || $this->outputTextFile;
    }

    /**
     * @return void
     * @throws Exception
     */
    public function export(): void
    {
        $multiOutput = $this->crawler->getOutput();
        /* @var $multiOutput MultiOutput */

        // text file
        if ($this->outputTextFile) {
            $textOutput = $multiOutput->getOutputByType(OutputType::TEXT);
            /* @var $textOutput TextOutput */
            $reportFile = $this->getExportFilePath($this->outputTextFile, 'txt');
            file_put_contents(
                $reportFile,
                Utils::removeAnsiColors($textOutput->getOutputText())
            );

            $this->status->addInfoToSummary('export-to-text', "Text report saved to '{$reportFile}'");
        }

        $jsonOutput = null;
        /* @var $jsonOutput JsonOutput */

        // json file
        if ($this->outputJsonFile) {
            $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
            /* @var $jsonOutput JsonOutput */
            $reportFile = $this->getExportFilePath($this->outputJsonFile, 'json');
            file_put_contents(
                $reportFile,
                $jsonOutput->getJson()
            );

            $this->status->addInfoToSummary('export-to-json', "JSON report saved to '{$reportFile}'");
        }

        // html file
        if ($this->outputHtmlFile) {
            $jsonOutput = $jsonOutput ?: $multiOutput->getOutputByType(OutputType::JSON);
            $htmlReport = HtmlReport::generate($jsonOutput->getJson());
            $reportFile = $this->getExportFilePath($this->outputHtmlFile, 'html');
            file_put_contents(
                $reportFile,
                $htmlReport
            );

            $this->status->addInfoToSummary('export-to-html', "HTML report saved to '{$reportFile}'");
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
        $hasExtension = preg_match('/\.[a-z0-9]{2,10}$/i', $file) === 1;
        if (!$hasExtension) {
            $file .= ".{$extension}";
        }
        if ($this->addHostToOutputFile) {
            $host = ParsedUrl::parse($this->crawler->getCoreOptions()->url)->host;
            $file = preg_replace('/\.[a-z0-9]{2,10}$/i', '.' . $host . '$0', $file);
        }
        if ($this->addTimestampToOutputFile) {
            $file = preg_replace('/\.[a-z0-9]{2,10}$/i', '.' . date('Y-m-d.H-i-s') . '$0', $file);
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
            new Option('--output-html-file', null, 'outputHtmlFile', Type::FILE, false, 'Save HTML report. `.html` added if missing.', null, true),
            new Option('--output-json-file', null, 'outputJsonFile', Type::FILE, false, 'Save report as JSON. `.json` added if missing.', null, true),
            new Option('--output-text-file', null, 'outputTextFile', Type::FILE, false, 'Save output as TXT. `.txt` added if missing.', null, true),
            new Option('--add-host-to-output-file', null, 'addHostToOutputFile', Type::BOOL, false, 'Append initial URL host to filename except sitemaps.', false, false),
            new Option('--add-timestamp-to-output-file', null, 'addTimestampToOutputFile', Type::BOOL, false, 'Append timestamp to filename except sitemaps.', false, false),
        ]));
        return $options;
    }


}