<?php
/**
 * Analyze Website Tool Handler for MCP
 * 
 * This class implements the AnalyzeWebsite tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class AnalyzeWebsiteHandler implements ToolHandlerInterface
{
    /**
     * Crawler executor instance
     */
    private CrawlerExecutor $executor;
    
    /**
     * Constructor
     * 
     * @param CrawlerExecutor $executor The crawler executor
     */
    public function __construct(CrawlerExecutor $executor)
    {
        $this->executor = $executor;
    }
    
    /**
     * {@inheritdoc}
     */
    public function getName(): string
    {
        return 'siteone/analyzeWebsite';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Performs a general crawl and analysis of a website starting from a given URL, returning summary statistics and key issues.';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getParameterSchema(): array
    {
        return [
            'type' => 'object',
            'properties' => [
                'url' => [
                    'type' => 'string',
                    'description' => 'The URL to analyze (required)'
                ],
                'depth' => [
                    'type' => 'integer',
                    'description' => 'Crawl depth (optional, default 1)',
                    'default' => 1
                ]
            ],
            'required' => ['url']
        ];
    }
    
    /**
     * {@inheritdoc}
     */
    public function execute(array $parameters): array
    {
        // Validate required parameters
        if (!isset($parameters['url']) || empty($parameters['url'])) {
            throw new \RuntimeException('Missing required parameter: url');
        }
        
        // Get parameters with defaults
        $url = $parameters['url'];
        $depth = $parameters['depth'] ?? 1;
        
        // Execute the crawler with appropriate parameters
        $crawlerParams = [
            'url' => $url,
            'max-depth' => $depth,
            'analyze' => true
        ];
        
        // Run the crawler
        $result = $this->executor->execute($crawlerParams);
        
        // Transform the crawler output into MCP tool result
        return $this->transformOutput($result);
    }
    
    /**
     * Transform the crawler output into a structured MCP tool result
     * 
     * @param array $crawlerOutput The crawler output
     * @return array The structured MCP tool result
     */
    private function transformOutput(array $crawlerOutput): array
    {
        // Extract useful information from the crawler output
        return [
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'totalSize' => $this->calculateTotalSize($crawlerOutput['results'] ?? []),
                'totalTime' => $this->calculateTotalTime($crawlerOutput['results'] ?? []),
                'errors404' => $this->count404s($crawlerOutput['tables']['404']['rows'] ?? []),
                'redirects' => count($crawlerOutput['tables']['redirects']['rows'] ?? []),
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'performance' => [
                'slowestUrls' => $this->transformSlowUrls($crawlerOutput['tables']['slowest-urls']['rows'] ?? []),
                'fastestUrls' => $this->transformSlowUrls($crawlerOutput['tables']['fastest-urls']['rows'] ?? [])
            ],
            'security' => $this->transformSecurityTable($crawlerOutput['tables']['security']['rows'] ?? []),
            'seo' => [
                'nonUniqueMetaData' => [
                    'titles' => $this->transformNonUniqueTable($crawlerOutput['tables']['non-unique-titles']['rows'] ?? []),
                    'descriptions' => $this->transformNonUniqueTable($crawlerOutput['tables']['non-unique-descriptions']['rows'] ?? [])
                ],
                'metadata' => $this->transformSeoTable($crawlerOutput['tables']['seo']['rows'] ?? [])
            ],
            'contentTypes' => $this->transformContentTypesTable($crawlerOutput['tables']['content-types']['rows'] ?? []),
            'sourceDomains' => $this->transformSourceDomainsTable($crawlerOutput['tables']['source-domains']['rows'] ?? [])
        ];
    }
    
    /**
     * Calculate the total size of all crawled URLs
     * 
     * @param array $results The crawler results
     * @return int The total size in bytes
     */
    private function calculateTotalSize(array $results): int
    {
        $total = 0;
        
        foreach ($results as $result) {
            $total += (int)($result['size'] ?? 0);
        }
        
        return $total;
    }
    
    /**
     * Calculate the total time spent crawling
     * 
     * @param array $results The crawler results
     * @return float The total time in seconds
     */
    private function calculateTotalTime(array $results): float
    {
        $total = 0.0;
        
        foreach ($results as $result) {
            $total += (float)($result['elapsedTime'] ?? 0);
        }
        
        return $total;
    }
    
    /**
     * Count the number of 404 errors
     * 
     * @param array $errors404 The 404 errors table
     * @return int The number of 404 errors
     */
    private function count404s(array $errors404): int
    {
        return count($errors404);
    }
    
    /**
     * Transform the slowest/fastest URLs table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformSlowUrls(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['url'] ?? '',
                'loadTime' => $row['requestTime'] ?? 0,
                'statusCode' => $row['statusCode'] ?? ''
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the security table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformSecurityTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $status = 'ok';
            
            if (($row['critical'] ?? 0) > 0) {
                $status = 'critical';
            } elseif (($row['warning'] ?? 0) > 0) {
                $status = 'warning';
            } elseif (($row['notice'] ?? 0) > 0) {
                $status = 'notice';
            }
            
            $transformed[] = [
                'header' => $row['header'] ?? '',
                'status' => $status,
                'recommendation' => $row['recommendation'] ?? ''
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the non-unique titles/descriptions table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformNonUniqueTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $key = array_key_exists('title', $row) ? 'title' : 'description';
            
            $transformed[] = [
                'value' => $row[$key] ?? '',
                'count' => $row['count'] ?? 0
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the SEO table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformSeoTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['urlPathAndQuery'] ?? '',
                'title' => $row['title'] ?? null,
                'description' => $row['description'] ?? null,
                'h1' => $row['h1'] ?? null,
                'indexing' => [
                    'robotsIndex' => $row['indexing']['robotsIndex'] ?? null,
                    'robotsFollow' => $row['indexing']['robotsFollow'] ?? null,
                    'deniedByRobotsTxt' => $row['indexing']['deniedByRobotsTxt'] ?? null
                ]
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the content types table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformContentTypesTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'type' => $row['contentType'] ?? '',
                'count' => $row['count'] ?? 0,
                'totalSize' => $row['totalSize'] ?? 0,
                'totalTime' => $row['totalTime'] ?? 0,
                'avgTime' => $row['avgTime'] ?? 0
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the source domains table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformSourceDomainsTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'domain' => $row['domain'] ?? '',
                'totalCount' => $row['totalCount'] ?? 0
            ];
        }
        
        return $transformed;
    }
} 