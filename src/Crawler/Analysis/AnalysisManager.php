<?php

namespace Crawler\Analysis;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Crawler;
use Crawler\ExtraColumn;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Exception;

class AnalysisManager
{

    /**
     * Absolute path to Crawler class directory where are subfolders Export and Analysis
     * @var string
     */
    private readonly string $crawlerClassDir;

    private Options $options;
    private Crawler $crawler;
    private Status $status;
    private Output $output;

    /**
     * @var Analyzer[]
     */
    private array $analyzers;

    /**
     * @param string $crawlerClassDir
     */
    public function __construct(string $crawlerClassDir)
    {
        $this->crawlerClassDir = $crawlerClassDir;
    }

    /**
     * Set options for analyzers - must be called before init()
     * @param Options $options
     * @return void
     */
    public function setOptions(Options $options): void
    {
        $this->options = $options;
    }

    /**
     * Init and import all analyzers from Crawler/Analysis folder that returns true in shouldBeActivated() method
     *
     * @param Crawler $crawler
     * @param Status $status
     * @param Output $output
     * @return void
     * @throws Exception
     */
    public function init(Crawler $crawler, Status $status, Output $output): void
    {
        if (!isset($this->options)) {
            throw new Exception('Options are not set');
        }

        $this->crawler = $crawler;
        $this->status = $status;
        $this->output = $output;

        $this->importAnalyzers();
    }

    /**
     * Run all analyzers after all URLs are crawled or after CTRL+C (process termination)
     *
     * @return void
     */
    public function runAnalyzers(): void
    {
        // sort analyzers by order for correct output order
        $analyzers = $this->getAnalyzers();
        usort($analyzers, function (Analyzer $a, Analyzer $b) {
            return $a->getOrder() <=> $b->getOrder();
        });

        foreach ($analyzers as $analyzer) {
            $analyzerBasename = basename(str_replace('\\', '/', get_class($analyzer)));
            $analyzer->setCrawler($this->crawler);
            $analyzer->setStatus($this->status);
            $analyzer->setOutput($this->output);
            try {
                $analyzer->analyze();
            } catch (Exception $e) {
                $this->status->addCriticalToSummary($analyzerBasename, "{$analyzerBasename} error: " . $e->getMessage());
            }
        }
    }

    /**
     * Analyze visited URLs by all active analyzers and return results as an array (key = analyzer class name)
     *
     * @param VisitedUrl $visitedUrl
     * @param string|null $body
     * @param array|null $headers
     * @return UrlAnalysisResult[]
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?array $headers): array
    {
        $result = [];
        foreach ($this->analyzers as $analyzer) {
            $analyzerResult = $analyzer->analyzeVisitedUrl($visitedUrl, $body, $headers);
            if ($analyzerResult) {
                $result[get_class($analyzer)] = $analyzerResult;
            }
        }
        return $result;
    }

    /**
     * @return string[]
     */
    public function getClassesOfAnalyzers(): array
    {
        $classes = [];
        foreach (glob($this->crawlerClassDir . '/Analysis/*Analyzer.php') as $file) {
            $classBaseName = basename($file, '.php');
            if ($classBaseName != 'Analyzer' & $classBaseName != 'BaseAnalyzer') {
                $classes[] = 'Crawler\\Analysis\\' . $classBaseName;
            }
        }
        return $classes;
    }

    /**
     * @return Analyzer[]
     */
    public function getAnalyzers(): array
    {
        return $this->analyzers;
    }

    /**
     * Check if analyzer is in active $this->analyzers
     *
     * @param string $analyzerClass
     * @return bool
     */
    public function hasAnalyzer(string $analyzerClass): bool
    {
        foreach ($this->analyzers as $analyzer) {
            if (get_class($analyzer) === $analyzerClass) {
                return true;
            }
        }
        return false;
    }

    /**
     * Import all founded analyzers in Crawler/Analysis folder and set all extra table columns from analyzers
     *
     * @return void
     */
    private function importAnalyzers(): void
    {
        $this->analyzers = [];

        $analyzerFilterRegex = $this->crawler->getCoreOptions()->analyzerFilterRegex;
        $analyzerClasses = $this->getClassesOfAnalyzers();
        $extraTableColumns = [];
        foreach ($analyzerClasses as $analyzerClass) {
            $analyzerBasename = basename(str_replace('\\', '/', $analyzerClass));

            // skip unwanted analyzers
            if ($analyzerFilterRegex && preg_match($analyzerFilterRegex, $analyzerBasename) === 0) {
                continue;
            }

            $analyzer = new $analyzerClass();
            /** @var Analyzer $analyzer */
            $analyzer->setConfig($this->options);
            if ($analyzer->shouldBeActivated()) {
                $this->analyzers[] = $analyzer;
            }

            $extraTableColumn = $analyzer->showAnalyzedVisitedUrlResultAsColumn();
            if ($extraTableColumn) {
                $extraTableColumns[] = $extraTableColumn;
            }
        }

        if ($extraTableColumns) {
            $this->output->setExtraColumnsFromAnalysis($extraTableColumns);
        }
    }
}