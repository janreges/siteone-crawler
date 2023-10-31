<?php

namespace Crawler\Analysis;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;
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
     * Total exec times of analyzer methods
     * @var array [string => int]
     */
    protected array $execTimes = [];

    /**
     * Total exec counts of analyzer methods
     * @var array [string => int]
     */
    protected array $execCounts = [];

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

        // add analysis stats table
        if ($analyzers) {
            $data = [];

            // stats from this class (AnalysisManager)
            foreach ($this->execTimes as $analyzerAndMethod => $execTime) {
                $data[] = [
                    'analyzerAndMethod' => basename(str_replace('\\', '/', $analyzerAndMethod)),
                    'execTime' => $execTime,
                    'execTimeFormatted' => Utils::getFormattedDuration($execTime),
                    'execCount' => $this->execCounts[$analyzerAndMethod] ?? 0,
                ];
            }

            // stats aggregated all analyzers
            $execTimes = $this->getExecTimesFromAnalyzers();
            $execCounts = $this->getExecCountsFromAnalyzers();
            foreach ($execTimes as $analyzerAndMethod => $execTime) {
                $data[] = [
                    'analyzerAndMethod' => basename(str_replace('\\', '/', $analyzerAndMethod)),
                    'execTime' => $execTime,
                    'execTimeFormatted' => Utils::getFormattedDuration($execTime),
                    'execCount' => $execCounts[$analyzerAndMethod] ?? 0,
                ];
            }

            // configure super table and add it to output
            $superTable = new SuperTable('analysis-stats', 'Analysis stats', 'No analysis stats', [
                new SuperTableColumn('analyzerAndMethod', 'Analyzer::method'),
                new SuperTableColumn('execTimeFormatted', 'Exec time'),
                new SuperTableColumn('execCount', 'Exec count'),
            ], false, 'execTime', 'DESC');
            $superTable->setData($data);

            $this->output->addSuperTable($superTable);
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
        if ($visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML) {
            $s = microtime(true);
            $dom = new DOMDocument();
            $domParsing = @$dom->loadHTML(mb_convert_encoding($body, 'HTML-ENTITIES', 'UTF-8'));
            if (!$domParsing) {
                $this->output->addNotice("HTML parsing error for URL {$visitedUrl->url}");
                $dom = null;
            }

            $this->measureExecTime('parseDOMDocument', $s);
        }

        foreach ($this->analyzers as $analyzer) {
            $analyzerResult = $analyzer->analyzeVisitedUrl($visitedUrl, $body, $dom, $headers);
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

    /**
     * Measure and increment exec time and count of analyzer method
     *
     * @param string $method
     * @param float $startTime
     * @return void
     */
    private function measureExecTime(string $method, float $startTime): void
    {
        $endTime = microtime(true);
        $key = __CLASS__ . '::' . $method;

        if (!isset($this->execTimes[$key])) {
            $this->execTimes[$key] = 0;
        }
        if (!isset($this->execCounts[$key])) {
            $this->execCounts[$key] = 0;
        }

        $this->execTimes[$key] += ($endTime - $startTime);
        $this->execCounts[$key]++;
    }
}