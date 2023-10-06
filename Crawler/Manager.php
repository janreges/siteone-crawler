<?php

namespace Crawler;

use Crawler\Output\FormattedTextOutput;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\Output;
use Crawler\Output\OutputType;
use Exception;

class Manager
{

    private string $version;
    private float $startTime;
    private Options $options;
    private Output $output;
    private string $command;

    /**
     * @param string $version
     * @param float $startTime
     * @param Options $options
     * @param string $command
     * @throws Exception
     */
    public function __construct(string $version, float $startTime, Options $options, string $command)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->output = $this->getOutputByOptions($options);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function run(): void
    {
        $this->output->addBanner();

        $crawler = new Crawler($this->options, $this->output);
        $crawler->run();

        $this->output->addUsedOptions($crawler->getFinalUserAgent());

        try {
            $this->handleOutputFilesAndMailer();
        } catch (\Exception $e) {
            $this->output->addError("SAVE or MAILER ERROR: {$e->getMessage()}");
        }

        $this->output->end();
    }

    /**
     * @return void
     * @throws Exception
     */
    public function handleOutputFilesAndMailer(): void
    {
        if (!$this->options->outputTextFile && !$this->options->outputJsonFile && !$this->options->outputHtmlFile && !$this->options->mailerIsActivated()) {
            return;
        }

        $multiOutput = $this->output;
        /* @var $multiOutput MultiOutput */

        if ($this->options->outputTextFile) {
            $textOutput = $multiOutput->getOutputByType(OutputType::FORMATTED_TEXT);
            /* @var $textOutput FormattedTextOutput */
            $reportFile = $this->getReportFilename($this->options->outputTextFile, 'txt');
            file_put_contents(
                $reportFile,
                Utils::removeAnsiColors($textOutput->getOutputText())
            );

            $this->output->addNotice("Text report saved to '{$reportFile}'.");
        }

        if ($this->options->outputJsonFile) {
            $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
            /* @var $jsonOutput JsonOutput */
            $reportFile = $this->getReportFilename($this->options->outputJsonFile, 'json');
            file_put_contents(
                $reportFile,
                $jsonOutput->getJson()
            );

            $this->output->addNotice("JSON report saved to '{$reportFile}'.");
        }

        if ($this->options->outputHtmlFile || $this->options->mailerIsActivated()) {
            $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
            $htmlReport = HtmlReport::generate($jsonOutput->getJson());
            if ($this->options->outputHtmlFile) {
                $reportFile = $this->getReportFilename($this->options->outputHtmlFile, 'html');
                file_put_contents(
                    $reportFile,
                    $htmlReport
                );
                $this->output->addNotice("HTML report saved to '{$reportFile}'.");
            }

            if ($this->options->mailerIsActivated()) {
                $mailer = new Mailer($this->options);
                $mailer->sendEmail($htmlReport);
                $this->output->addNotice("HTML report sent to " . implode(', ', $this->options->mailTo) . ".");
            }
        }
    }

    /**
     * @param string $file
     * @param string $extension
     * @return string
     */
    public
    function getReportFilename(string $file, string $extension): string
    {
        $hasExtension = preg_match('/\.[a-z0-9]{2,10}$/i', $file) === 1;
        if (!$hasExtension) {
            $file .= ".{$extension}";
        }
        if ($this->options->addHostToOutputFile) {
            $host = ParsedUrl::parse($this->options->url)->host;
            $file = preg_replace('/\.[a-z0-9]{2,10}$/i', '.' . $host . '$0', $file);
        }
        if ($this->options->addTimestampToOutputFile) {
            $file = preg_replace('/\.[a-z0-9]{2,10}$/i', '.' . date('Y-m-d.H-i-s') . '$0', $file);
        }

        if (!is_writable(dirname($file))) {
            throw new Exception("Output {$extension} file {$file} is not writable. Check permissions.");
        }

        return $file;
    }

    /**
     * @param Options $options
     * @return Output
     * @throws Exception
     */
    private function getOutputByOptions(Options $options): Output
    {
        $requiredOutputs = [];
        if ($this->options->outputType == OutputType::FORMATTED_TEXT || $this->options->outputTextFile) {
            $requiredOutputs[] = new FormattedTextOutput(
                $this->version,
                $this->startTime,
                $this->options,
                $this->getSafeCommand(),
                $this->options->outputType == OutputType::FORMATTED_TEXT
            );
        }
        if ($this->options->outputType == OutputType::JSON || $this->options->outputJsonFile || $this->options->outputHtmlFile) {
            $requiredOutputs[] = new JsonOutput(
                $this->version,
                $this->startTime,
                $this->options,
                $this->getSafeCommand(),
                $this->options->outputType == OutputType::JSON
            );
        }

        $multiOutputRequired = count($requiredOutputs) > 1 || $this->options->mailerIsActivated();

        if ($multiOutputRequired) {
            $result = new MultiOutput();
            foreach ($requiredOutputs as $output) {
                $result->addOutput($output);
            }
            return $result;
        } else if ($requiredOutputs) {
            return $requiredOutputs[0];
        } else {
            throw new Exception("Unknown output type {$this->options->outputType}");
        }
    }

    public function getSafeCommand(): string
    {
        return preg_replace(
            ['/(pass[a-z]{0,5})=[^\s]+/i', '/(key[s]?)=[^\s*]+/i', '/(secret[s]?)=[^\s*]+/i'],
            ['$1=***', '$1=***', '$1=***'],
            $this->command
        );
    }

}