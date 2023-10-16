<?php

namespace Crawler;

use Crawler\Analysis\Analyzer;
use Crawler\Export\Exporter;
use Crawler\Export\FileExporter;
use Crawler\Export\MailerExporter;
use Crawler\Export\SitemapExporter;
use Crawler\HttpClient\HttpClient;
use Crawler\Output\TextOutput;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\Output;
use Crawler\Output\OutputType;
use Crawler\Result\Status;
use Crawler\Result\Storage\FileStorage;
use Crawler\Result\Storage\MemoryStorage;
use Crawler\Result\Storage\StorageType;
use Exception;

class Manager
{

    private string $version;
    private float $startTime;
    private CoreOptions $options;
    private Output $output;
    private string $command;
    private Crawler $crawler;
    private Status $status;

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
     * @param string $baseDir
     * @throws Exception
     */
    public function __construct(string $version, float $startTime, CoreOptions $options, string $command, array $exporters, array $analyzers, string $baseDir)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->exporters = $exporters;
        $this->analyzers = $analyzers;

        $compression = true; // TODO from options

        $crawlerInfo = new Info(
            'SiteOne Website Crawler',
            $this->version,
            date('Y-m-d H:i:s'),
            Utils::getSafeCommand($this->command),
            gethostname(),
            $this->options->userAgent ?? 'default'
        );

        $this->status = new Status(
            $options->resultStorage === StorageType::MEMORY
                ? new MemoryStorage($compression)
                : new FileStorage($baseDir . '/tmp/result-storage', $compression),
            true, // TODO by options
            $crawlerInfo,
            $this->options,
            $this->startTime
        );

        $this->output = $this->getOutputByOptions($this->status);

        $httpClient = new HttpClient($baseDir. '/tmp/http-client-cache');
        $this->crawler = new Crawler($this->options, $httpClient, $this->output, $this->status);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function run(): void
    {
        $this->output->addBanner();

        $this->crawler->init();
        $this->crawler->run([$this, 'crawlerDoneCallback']);
    }

    public function crawlerDoneCallback(): void
    {
        $this->runAnalyzers();
        $this->runExporters();

        $this->output->addUsedOptions();
        $this->output->addTotalStats($this->crawler->getVisited());
        $this->output->addSummary($this->status->getSummary());
        $this->output->end();

    }

    /**
     * @param Status $status
     * @return Output
     * @throws Exception
     */
    private function getOutputByOptions(Status $status): Output
    {
        $requiredOutputs = [];
        if ($this->options->outputType == OutputType::TEXT || $this->hasExporter(FileExporter::class)) {
            $requiredOutputs[] = new TextOutput(
                $this->version,
                $this->startTime,
                $this->status,
                $this->options,
                Utils::getSafeCommand($this->command),
                $this->options->outputType == OutputType::TEXT
            );
        }

        $jsonOutputNeeded =
            $this->options->outputType == OutputType::JSON
            || $this->hasExporter(FileExporter::class)
            || $this->hasExporter(MailerExporter::class)
            || $this->hasExporter(SitemapExporter::class);

        if ($jsonOutputNeeded) {
            $requiredOutputs[] = new JsonOutput(
                $this->version,
                $this->startTime,
                $status,
                $this->options,
                Utils::getSafeCommand($this->command),
                $this->options->outputType == OutputType::JSON
            );
        }

        $multiOutputRequired = count($requiredOutputs) > 1;

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
            $exporter->setStatus($this->status);
            $exporter->setOutput($this->output);
            try {
                $exporter->export();
            } catch (Exception $e) {
                $exporterBasename = basename(str_replace('\\', '/', get_class($exporter)));
                $this->status->addErrorToSummary($exporterBasename, "{$exporterBasename} error: " . $e->getMessage());
            }
        }
    }

    private function runAnalyzers(): void
    {
        // sort analyzers by order
        usort($this->analyzers, function (Analyzer $a, Analyzer $b) {
            return $a->getOrder() <=> $b->getOrder();
        });

        foreach ($this->analyzers as $analyzer) {
            $analyzer->setCrawler($this->crawler);
            $analyzer->setStatus($this->status);
            $analyzer->setOutput($this->output);
            try {
                $analyzer->analyze();
            } catch (Exception $e) {
                $analyzerBasename = basename(str_replace('\\', '/', get_class($analyzer)));
                $this->status->addErrorToSummary($analyzerBasename, "{$analyzerBasename} error: " . $e->getMessage());
            }
        }
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