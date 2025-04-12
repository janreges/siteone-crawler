<?php
/**
 * Get Website Performance Tool Handler for MCP
 * 
 * This class implements the GetWebsitePerformance tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class GetWebsitePerformanceHandler implements ToolHandlerInterface
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
        return 'siteone/getWebsitePerformance';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Analyzes website performance by crawling and identifying the slowest and fastest loading pages based on response times.';
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
            'analyze' => true,
            'analyze-performance' => true,
            'record-response-time' => true
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
        // Calculate overall metrics
        $overallMetrics = $this->calculateOverallMetrics($crawlerOutput['results'] ?? []);
        
        // Extract performance data from results
        $performanceData = $this->extractPerformanceData($crawlerOutput['results'] ?? []);
        
        return [
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'averageResponseTime' => $overallMetrics['averageLoadTime'],
                'totalContentSize' => $overallMetrics['totalSize'],
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'slowestPages' => $this->sortPagesByResponseTime($performanceData, 'desc'),
            'fastestPages' => $this->sortPagesByResponseTime($performanceData, 'asc'),
            'largestPages' => $this->sortPagesBySize($performanceData, 'desc'),
            'slowestUrls' => $this->transformSlowUrls($crawlerOutput['tables']['slowest-urls']['rows'] ?? []),
            'fastestUrls' => $this->transformFastUrls($crawlerOutput['tables']['fastest-urls']['rows'] ?? []),
            'performanceByContentType' => $this->transformContentTypePerformance($crawlerOutput['tables']['content-types']['rows'] ?? []),
            'performanceByDomain' => $this->transformDomainPerformance($crawlerOutput['tables']['source-domains']['rows'] ?? [])
        ];
    }
    
    /**
     * Calculate overall performance metrics from the crawler results
     * 
     * @param array $results The crawler results
     * @return array The overall metrics
     */
    private function calculateOverallMetrics(array $results): array
    {
        $totalLoadTime = 0;
        $totalSize = 0;
        $count = count($results);
        
        foreach ($results as $result) {
            $totalLoadTime += (float)($result['time'] ?? 0);
            $totalSize += (int)($result['contentLength'] ?? 0);
        }
        
        $averageLoadTime = $count > 0 ? (int)round($totalLoadTime / $count) : 0;
        $averageSize = $count > 0 ? (int)round($totalSize / $count) : 0;
        
        return [
            'totalLoadTime' => $totalLoadTime,
            'averageLoadTime' => $averageLoadTime,
            'totalSize' => $totalSize,
            'averageSize' => $averageSize
        ];
    }
    
    /**
     * Extract performance data from the crawler results
     * 
     * @param array $results The crawler results
     * @return array The extracted performance data
     */
    private function extractPerformanceData(array $results): array
    {
        $performanceData = [];
        
        foreach ($results as $result) {
            $performanceData[] = [
                'url' => $result['url'] ?? '',
                'responseTime' => (float)($result['time'] ?? 0),
                'contentLength' => (int)($result['contentLength'] ?? 0),
                'contentType' => $result['contentType'] ?? '',
                'statusCode' => $result['status'] ?? ''
            ];
        }
        
        return $performanceData;
    }
    
    /**
     * Sort pages by response time
     * 
     * @param array $pages The pages
     * @param string $direction The sort direction ('asc' or 'desc')
     * @return array The sorted pages
     */
    private function sortPagesByResponseTime(array $pages, string $direction = 'desc'): array
    {
        $sorted = $pages;
        
        usort($sorted, function($a, $b) use ($direction) {
            if ($direction === 'asc') {
                return $a['responseTime'] <=> $b['responseTime'];
            } else {
                return $b['responseTime'] <=> $a['responseTime'];
            }
        });
        
        return $sorted;
    }
    
    /**
     * Sort pages by content size
     * 
     * @param array $pages The pages
     * @param string $direction The sort direction ('asc' or 'desc')
     * @return array The sorted pages
     */
    private function sortPagesBySize(array $pages, string $direction = 'desc'): array
    {
        $sorted = $pages;
        
        usort($sorted, function($a, $b) use ($direction) {
            if ($direction === 'asc') {
                return $a['contentLength'] <=> $b['contentLength'];
            } else {
                return $b['contentLength'] <=> $a['contentLength'];
            }
        });
        
        return $sorted;
    }
    
    /**
     * Transform the slowest URLs table
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
                'loadTime' => (float)($row['requestTime'] ?? 0),
                'statusCode' => $row['statusCode'] ?? '',
                'size' => (int)($row['size'] ?? 0),
                'contentType' => $this->getContentTypeString($row['contentType'] ?? '')
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the fastest URLs table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformFastUrls(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['url'] ?? '',
                'loadTime' => (float)($row['requestTime'] ?? 0),
                'statusCode' => $row['statusCode'] ?? '',
                'size' => (int)($row['size'] ?? 0),
                'contentType' => $this->getContentTypeString($row['contentType'] ?? '')
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the content types performance table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformContentTypePerformance(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'contentType' => $row['contentType'] ?? '',
                'count' => (int)($row['count'] ?? 0),
                'totalSize' => (int)($row['totalSize'] ?? 0),
                'totalLoadTime' => (float)($row['totalTime'] ?? 0),
                'averageLoadTime' => (float)($row['avgTime'] ?? 0),
                'status' => [
                    '2xx' => (int)($row['status20x'] ?? 0),
                    '3xx' => (int)($row['status30x'] ?? 0),
                    '4xx' => (int)($row['status40x'] ?? 0),
                    '5xx' => (int)($row['status50x'] ?? 0)
                ]
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the source domains performance table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformDomainPerformance(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            // Parse domain performance data
            // The totals field typically has format like "67/30MB/6.2s"
            $totalParts = $this->parseTotalsField($row['totals'] ?? '');
            
            $domainData = [
                'domain' => $row['domain'] ?? '',
                'totalCount' => (int)($row['totalCount'] ?? $totalParts['count']),
                'totalSize' => $totalParts['size'],
                'totalLoadTime' => $totalParts['time'],
                'contentTypes' => []
            ];
            
            // Add content type specific data if available
            $contentTypes = ['HTML', 'Image', 'JS', 'CSS', 'Document', 'JSON', 'Other'];
            foreach ($contentTypes as $type) {
                if (isset($row[$type]) && !empty($row[$type])) {
                    $typeParts = $this->parseTotalsField($row[$type]);
                    if ($typeParts['count'] > 0) {
                        $domainData['contentTypes'][$type] = [
                            'count' => $typeParts['count'],
                            'size' => $typeParts['size'],
                            'loadTime' => $typeParts['time']
                        ];
                    }
                }
            }
            
            $transformed[] = $domainData;
        }
        
        return $transformed;
    }
    
    /**
     * Parse the totals field from the source domains table
     * Format is typically like "67/30MB/6.2s"
     * 
     * @param string $totalsField The totals field value
     * @return array The parsed count, size, and time values
     */
    private function parseTotalsField(string $totalsField): array
    {
        $result = [
            'count' => 0,
            'size' => 0,
            'time' => 0
        ];
        
        // If empty, return defaults
        if (empty($totalsField)) {
            return $result;
        }
        
        // Split by slash
        $parts = explode('/', $totalsField);
        
        // Get count
        if (isset($parts[0])) {
            $result['count'] = (int)$parts[0];
        }
        
        // Get size
        if (isset($parts[1])) {
            $result['size'] = $this->parseSizeString($parts[1]);
        }
        
        // Get time
        if (isset($parts[2])) {
            $result['time'] = $this->parseTimeString($parts[2]);
        }
        
        return $result;
    }
    
    /**
     * Parse a size string like "30MB" or "2kB" into bytes
     * 
     * @param string $sizeString The size string
     * @return int The size in bytes
     */
    private function parseSizeString(string $sizeString): int
    {
        $sizeString = trim($sizeString);
        
        // If it's just a number, return it
        if (is_numeric($sizeString)) {
            return (int)$sizeString;
        }
        
        // Extract the number and unit
        if (preg_match('/^([\d.]+)\s*([a-zA-Z]+)$/', $sizeString, $matches)) {
            $number = (float)$matches[1];
            $unit = strtoupper($matches[2]);
            
            // Convert to bytes based on unit
            switch ($unit) {
                case 'KB':
                case 'K':
                    return (int)($number * 1024);
                case 'MB':
                case 'M':
                    return (int)($number * 1024 * 1024);
                case 'GB':
                case 'G':
                    return (int)($number * 1024 * 1024 * 1024);
                case 'TB':
                case 'T':
                    return (int)($number * 1024 * 1024 * 1024 * 1024);
                case 'B':
                default:
                    return (int)$number;
            }
        }
        
        // If we couldn't parse it, return 0
        return 0;
    }
    
    /**
     * Parse a time string like "6.2s" or "500ms" into seconds
     * 
     * @param string $timeString The time string
     * @return float The time in seconds
     */
    private function parseTimeString(string $timeString): float
    {
        $timeString = trim($timeString);
        
        // If it's just a number, assume it's seconds
        if (is_numeric($timeString)) {
            return (float)$timeString;
        }
        
        // Extract the number and unit
        if (preg_match('/^([\d.]+)\s*([a-zA-Z]+)$/', $timeString, $matches)) {
            $number = (float)$matches[1];
            $unit = strtolower($matches[2]);
            
            // Convert to seconds based on unit
            switch ($unit) {
                case 'ms':
                    return $number / 1000;
                case 'm':
                    return $number * 60;
                case 'h':
                    return $number * 3600;
                case 's':
                default:
                    return $number;
            }
        }
        
        // If we couldn't parse it, return 0
        return 0.0;
    }
    
    /**
     * Convert a content type ID to a string representation
     * 
     * @param string|int $contentTypeId The content type ID
     * @return string The content type string
     */
    private function getContentTypeString($contentTypeId): string
    {
        // These mappings should match the crawler's internal content type IDs
        $contentTypes = [
            1 => 'HTML',
            2 => 'JavaScript',
            3 => 'CSS',
            4 => 'Image',
            5 => 'Audio',
            6 => 'Video',
            7 => 'Document',
            8 => 'JSON',
            9 => 'XML',
            10 => 'Font'
        ];
        
        return $contentTypes[(int)$contentTypeId] ?? 'Unknown';
    }
} 