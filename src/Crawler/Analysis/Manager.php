<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\ManagerStats;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use DOMDocument;
use Exception;

class Manager
{

    const SUPER_TABLE_ANALYSIS_STATS = 'analysis-stats';

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

    private ManagerStats $stats;

    /**
     * @param string $crawlerClassDir
     */
    public function __construct(string $crawlerClassDir)
    {
        $this->crawlerClassDir = $crawlerClassDir;
        $this->stats = new ManagerStats();
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
        // check if there are any working URLs
        if ($this->status->getNumberOfWorkingVisitedUrls() === 0) {
            $errorMessage = 'The analysis has been suspended because no working URL could be found. Please check the URL/domain.';
            $this->output->addError($errorMessage);
            $this->status->addCriticalToSummary('analysis-manager-error', $errorMessage);
            return;
        }

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

        // add analysis stats table
        if ($analyzers) {
            $execTimes = $this->getExecTimesFromAnalyzers();
            $execCounts = $this->getExecCountsFromAnalyzers();

            $superTable = $this->stats->getSuperTable(
                self::SUPER_TABLE_ANALYSIS_STATS,
                'Analysis stats',
                'No analysis stats',
                $execTimes,
                $execCounts
            );

            $this->output->addSuperTable($superTable);
            $this->status->addSuperTableAtEnd($superTable);
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

        $dom = null;
        if ($visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $body !== null && trim($body) !== '') {
            $s = microtime(true);
            $encodedBody = htmlentities($body, ENT_QUOTES | ENT_SUBSTITUTE | ENT_HTML5, 'UTF-8');
            if (!$encodedBody) {
                $this->output->addNotice("HTML encoding error for URL {$visitedUrl->url}");
                return $result;
            }
            $dom = new DOMDocument();
            $domParsing = @$dom->loadHTML($encodedBody);
            if (!$domParsing) {
                $this->output->addNotice("HTML parsing error for URL {$visitedUrl->url}");
                $dom = null;
            }

            $this->stats->measureExecTime(__CLASS__, 'parseDOMDocument', $s);
        }

        foreach ($this->analyzers as $analyzer) {
            $analyzerResult = $analyzer->analyzeVisitedUrl($visitedUrl, $body, $dom, $headers);
            if ($analyzerResult) {
                $result[get_class($analyzer)] = $analyzerResult;
                $this->status->addUrlAnalysisResultForVisitedUrl($visitedUrl->uqId, $analyzerResult);
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
     * @return array [analyzer class::method => exec time in seconds]
     */
    public function getExecTimesFromAnalyzers(): array
    {
        $result = [];

        foreach ($this->analyzers as $analyzer) {
            if ($analyzer instanceof BaseAnalyzer) {
                $result = array_merge($result, $analyzer->getExecTimes());
            }
        }

        return $result;
    }

    /**
     * @return array [analyzer class::method => calls count]
     */
    public function getExecCountsFromAnalyzers(): array
    {
        $result = [];

        foreach ($this->analyzers as $analyzer) {
            if ($analyzer instanceof BaseAnalyzer) {
                $result = array_merge($result, $analyzer->getExecCounts());
            }
        }

        return $result;
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