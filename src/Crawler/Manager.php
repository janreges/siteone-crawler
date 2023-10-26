<?php

namespace Crawler;

use Crawler\Analysis\AnalysisManager;
use Crawler\Export\Exporter;
use Crawler\Export\FileExporter;
use Crawler\Export\MailerExporter;
use Crawler\Export\OfflineWebsiteExporter;
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
use Crawler\Result\VisitedUrl;
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

    private AnalysisManager $analysisManager;

    /**
     * @param string $version
     * @param float $startTime
     * @param CoreOptions $options
     * @param string $command
     * @param Exporter[] $exporters
     * @param AnalysisManager $analysisManager
     * @param string $baseDir
     * @throws Exception
     */
    public function __construct(string $version, float $startTime, CoreOptions $options, string $command, array $exporters, AnalysisManager $analysisManager, string $baseDir)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->exporters = $exporters;

        $crawlerInfo = new Info(
            'SiteOne Website Crawler',
            $this->version,
            date('Y-m-d H:i:s'),
            Utils::getSafeCommand($this->command),
            gethostname(),
            $options->userAgent ?? 'default'
        );

        $resultStorageDir = $options->resultStorageDir && !str_starts_with($options->resultStorageDir, '/')
            ? ($baseDir . '/' . $options->resultStorageDir)
            : $options->resultStorageDir;

        $this->status = new Status(
            $options->resultStorage === StorageType::MEMORY
                ? new MemoryStorage($options->resultStorageCompression)
                : new FileStorage($resultStorageDir, $options->resultStorageCompression),
            true,
            $crawlerInfo,
            $options,
            $this->startTime
        );

        $this->output = $this->getOutputByOptions($this->status);

        $httpClientCacheDir = $options->httpCacheDir && !str_starts_with($options->httpCacheDir, '/')
            ? ($baseDir . '/' . $options->httpCacheDir)
            : ($options->httpCacheDir ?: null);

        $httpClient = new HttpClient($httpClientCacheDir, $options->httpCacheCompression);
        $this->crawler = new Crawler($options, $httpClient, $this->output, $this->status);

        $this->analysisManager = $analysisManager;
        $this->analysisManager->init($this->crawler, $this->status, $this->output);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function run(): void
    {
        $this->output->addBanner();

        // remove avif and webp support from accept header if offline website export is enabled (because of potential missing support in offline browsers)
        if ($this->hasExporter(OfflineWebsiteExporter::class)) {
            $this->crawler->removeAvifAndWebpSupportFromAcceptHeader();
        }

        $this->crawler->init();
        $this->crawler->run([$this, 'crawlerDoneCallback'], [$this, 'visitedUrlCallback']);

        // for cases when callback is not called
        $this->crawlerDoneCallback();
    }

    public function crawlerDoneCallback(): void
    {
        $this->crawler->terminate();

        static $alreadyDone = false;
        if ($alreadyDone) {
            return;
        }
        $alreadyDone = true;

        $this->analysisManager->runAnalyzers();
        $this->runExporters();

        $this->output->addUsedOptions();
        $this->output->addTotalStats($this->crawler->getVisited());
        $this->output->addSummary($this->status->getSummary());
        $this->output->end();
    }

    /**
     * This callback is called after each URL is crawled (even if it fails) from Crawler class by callback
     * Returns array of analysis results in format [analysisClass => UrlAnalysisResult, ...]
     *
     * @param VisitedUrl $url
     * @param string|null $body
     * @param array|null $headers
     * @return array
     */
    public function visitedUrlCallback(VisitedUrl $url, ?string $body, ?array $headers): array
    {
        // cache for analyzer class to table column
        static $analyzerClassToTableColumn = null;
        if ($analyzerClassToTableColumn === null) {
            $analyzerClassToTableColumn = [];
            foreach ($this->analysisManager->getAnalyzers() as $analyzer) {
                $tableColumn = $analyzer->showAnalyzedVisitedUrlResultAsColumn();
                if ($tableColumn) {
                    $analyzerClassToTableColumn[get_class($analyzer)] = $tableColumn->name;
                }
            }
        }

        $result = [];
        $analysisResult = $this->analysisManager->analyzeVisitedUrl($url, $body, $headers);
        foreach ($analysisResult as $analyzerClass => $analyzerResult) {
            if (isset($analyzerClassToTableColumn[$analyzerClass])) {
                $result[$analyzerClassToTableColumn[$analyzerClass]] = $analyzerResult;
            }
        }
        return $result;
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
                $this->status,
                $this->options,
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
                $status,
                $this->options,
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
            throw new Exception(__METHOD__ . ": Unknown output type");
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
                $this->status->addCriticalToSummary($exporterBasename, "{$exporterBasename} error: " . $e->getMessage());
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

}