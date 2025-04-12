<?php
/**
 * Find Broken Links Tool Handler for MCP
 * 
 * This class implements the FindBrokenLinks tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class FindBrokenLinksHandler implements ToolHandlerInterface
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
        return 'siteone/findBrokenLinks';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Specifically crawls a website starting from a URL to identify and report broken internal and external links.';
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
            'analyze-broken-links' => true
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
        return [
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'totalBrokenLinks' => $this->count404s($crawlerOutput['tables']['404']['rows'] ?? []),
                'totalRedirects' => count($crawlerOutput['tables']['redirects']['rows'] ?? []),
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'brokenLinks' => $this->transformBrokenLinks($crawlerOutput),
            'redirects' => $this->transformRedirects($crawlerOutput),
            'skippedUrls' => $this->transformSkippedUrls($crawlerOutput['tables']['skipped']['rows'] ?? [])
        ];
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
     * Transform the 404 links table
     * 
     * @param array $crawlerOutput The crawler output
     * @return array The transformed data
     */
    private function transformBrokenLinks(array $crawlerOutput): array
    {
        $rows = $crawlerOutput['tables']['404']['rows'] ?? [];
        $transformed = [];
        
        // Create a flat list of broken links as expected by tests
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['url'] ?? '',
                'statusCode' => $row['statusCode'] ?? 404,
                'foundOn' => [
                    'url' => $row['foundOnUrl'] ?? '',
                    'title' => $row['foundOnTitle'] ?? ''
                ]
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the redirects table
     * 
     * @param array $crawlerOutput The crawler output
     * @return array The transformed data
     */
    private function transformRedirects(array $crawlerOutput): array
    {
        $rows = $crawlerOutput['tables']['redirects']['rows'] ?? [];
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['url'] ?? '',
                'statusCode' => $row['statusCode'] ?? 0,
                'location' => $row['location'] ?? '',
                'foundOn' => [
                    'url' => $row['foundOnUrl'] ?? '',
                    'title' => $row['foundOnTitle'] ?? ''
                ]
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the skipped URLs table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformSkippedUrls(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['url'] ?? '',
                'reason' => $row['reason'] ?? 'unknown'
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Find the URL of a source page by its unique ID
     * 
     * @param string $sourceId The source unique ID
     * @param array $results The crawler results
     * @return string|null The source URL or null if not found
     */
    private function findSourcePageUrl(string $sourceId, array $results): ?string
    {
        foreach ($results as $result) {
            if (isset($result['uqId']) && $result['uqId'] === $sourceId) {
                return $result['url'] ?? null;
            }
        }
        
        return null;
    }
    
    /**
     * Get a textual description for a skip reason code
     * 
     * @param int|string $reasonCode The reason code
     * @return string The reason description
     */
    private function getSkipReasonText($reasonCode): string
    {
        // If the reason is already a string, just return it
        if (is_string($reasonCode) && !is_numeric($reasonCode)) {
            return $reasonCode;
        }
        
        // Convert to integer if it's a numeric string
        if (is_string($reasonCode) && is_numeric($reasonCode)) {
            $reasonCode = (int)$reasonCode;
        }
        
        // These are example mappings, actual codes may vary
        $reasons = [
            1 => 'External domain',
            2 => 'Disallowed by robots.txt',
            3 => 'URL pattern excluded',
            4 => 'Max depth reached',
            5 => 'Max URLs reached',
            6 => 'Invalid URL format',
            7 => 'URL too long',
            8 => 'Previously failed',
            9 => 'File type excluded',
            10 => 'Fragment identifier',
            11 => 'Duplicate URL'
        ];
        
        return $reasons[$reasonCode] ?? "Unknown reason ($reasonCode)";
    }
} 