<?php

namespace Crawler;

use Crawler\Analysis\Analyzer;
use Crawler\Export\Exporter;
use Crawler\Export\FileExporter;
use Crawler\Export\MailerExporter;
use Crawler\Export\SitemapExporter;
use Crawler\Output\TextOutput;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\Output;
use Crawler\Output\OutputType;
use Exception;

class Manager
{

    private string $version;
    private float $startTime;
    private CoreOptions $options;
    private Output $output;
    private string $command;
    private Crawler $crawler;

    /**
     * @var Exporter[]
     */
    private array $exporters;

    /**
     * @var Analyzer[]
     */
    private array $analyzers;

    /**
     * @param string $version
     * @param float $startTime
     * @param CoreOptions $options
     * @param string $command
     * @param Exporter[] $exporters
     * @param Analyzer[] $analyzers
     * @throws Exception
     */
    public function __construct(string $version, float $startTime, CoreOptions $options, string $command, array $exporters, array $analyzers)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->exporters = $exporters;
        $this->analyzers = $analyzers;
        $this->output = $this->getOutputByOptions();

        $this->crawler = new Crawler($this->options, $this->output);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function run(): void
    {
        $this->output->addBanner();

        $this->crawler->run();
        $this->output->addUsedOptions($this->crawler->getFinalUserAgent());

        $this->runAnalyzers();
        $this->runExporters();

        $this->output->end();
    }

    /**
     * @return Output
     * @throws Exception
     */
    private function getOutputByOptions(): Output
    {
        $requiredOutputs = [];
        if ($this->options->outputType == OutputType::TEXT || $this->hasExporter(FileExporter::class)) {
            $requiredOutputs[] = new TextOutput(
                $this->version,
                $this->startTime,
                $this->options,
                $this->getSafeCommand(),
                $this->options->outputType == OutputType::TEXT
            );
        }

        $jsonOutputNeeded =
            $this->options->outputType == OutputType::JSON
            || $this->hasExporter(FileExporter::class)
            || $this->hasExporter(SitemapExporter::class);

        if ($jsonOutputNeeded) {
            $requiredOutputs[] = new JsonOutput(
                $this->version,
                $this->startTime,
                $this->options,
                $this->getSafeCommand(),
                $this->options->outputType == OutputType::JSON
            );
        }

        $multiOutputRequired = count($requiredOutputs) > 1 || $this->hasExporter(MailerExporter::class);

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

    private function runExporters(): void
    {
        foreach ($this->exporters as $exporter) {
            $exporter->setCrawler($this->crawler);
            $exporter->setOutput($this->output);
            try {
                $exporter->export();
            } catch (Exception $e) {
                $exporterBasename = basename(str_replace('\\', '/', get_class($exporter)));
                $this->output->addError("{$exporterBasename} error: " . $e->getMessage());
            }
        }
    }

    private function runAnalyzers(): void
    {
        foreach ($this->analyzers as $analyzer) {
            $analyzer->setCrawler($this->crawler);
            $analyzer->setOutput($this->output);
            try {
                $analyzer->analyze();
            } catch (Exception $e) {
                $analyzerBasename = basename(str_replace('\\', '/', get_class($analyzer)));
                $this->output->addError("{$analyzerBasename} error: " . $e->getMessage());
            }
        }
    }

    public function getSafeCommand(): string
    {
        return preg_replace(
            ['/(pass[a-z]{0,5})=\S+/i', '/(keys?)=\S+/i', '/(secrets?)=\S+/i'],
            ['$1=***', '$1=***', '$1=***'],
            $this->command
        );
    }

    private function hasExporter(string $exporterClass): bool
    {
        foreach ($this->exporters as $exporter) {
            if (get_class($exporter) === $exporterClass) {
                return true;
            }
        }
        return false;
    }

    private function hasAnalyzer(string $analyzerClass): bool
    {
        foreach ($this->analyzers as $analyzer) {
            if (get_class($analyzer) === $analyzerClass) {
                return true;
            }
        }
        return false;
    }

}