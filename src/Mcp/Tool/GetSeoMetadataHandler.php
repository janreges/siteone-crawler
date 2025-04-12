<?php
/**
 * Get SEO Metadata Tool Handler for MCP
 * 
 * This class implements the GetSeoMetadata tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class GetSeoMetadataHandler implements ToolHandlerInterface
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
        return 'siteone/getSeoMetadata';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Analyzes a specific URL or crawls a site to gather SEO-related metadata (titles, descriptions, OpenGraph tags, headings) and identifies potential issues.';
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
                'crawl' => [
                    'type' => 'boolean',
                    'description' => 'Whether to crawl the entire site from the URL (optional, default false)',
                    'default' => false
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
        $crawl = $parameters['crawl'] ?? false;
        
        // Execute the crawler with appropriate parameters
        $crawlerParams = [
            'url' => $url,
            'max-depth' => $crawl ? 3 : 0,
            'analyze' => true,
            'analyze-seo' => true
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
                'analyzedUrls' => count($crawlerOutput['results'] ?? []),
                'nonUniqueMetaData' => [
                    'titles' => $this->summarizeNonUniqueTable($crawlerOutput['tables']['non-unique-titles']['rows'] ?? []),
                    'descriptions' => $this->summarizeNonUniqueTable($crawlerOutput['tables']['non-unique-descriptions']['rows'] ?? [])
                ],
                'headingIssuesCount' => $this->countHeadingIssues($crawlerOutput['tables']['seo-headings']['rows'] ?? [])
            ],
            'pageMetadata' => $this->transformSeoTable($crawlerOutput['tables']['seo']['rows'] ?? []),
            'openGraph' => $this->transformOpenGraphTable($crawlerOutput['tables']['open-graph']['rows'] ?? []),
            'headings' => $this->transformHeadingsTable($crawlerOutput['tables']['seo-headings']['rows'] ?? [])
        ];
    }
    
    /**
     * Summarize the non-unique titles/descriptions table
     * 
     * @param array $rows The table rows
     * @return array The summary data
     */
    private function summarizeNonUniqueTable(array $rows): array
    {
        $total = 0;
        $maxCount = 0;
        
        foreach ($rows as $row) {
            $count = $row['count'] ?? 0;
            $total += $count;
            
            if ($count > $maxCount) {
                $maxCount = $count;
            }
        }
        
        return [
            'totalNonUnique' => count($rows),
            'totalDuplicates' => $total,
            'maxDuplicates' => $maxCount
        ];
    }
    
    /**
     * Count the number of heading structure issues
     * 
     * @param array $rows The heading structure table rows
     * @return int The number of issues
     */
    private function countHeadingIssues(array $rows): int
    {
        $total = 0;
        
        foreach ($rows as $row) {
            $total += (int)($row['headingsErrorsCount'] ?? 0);
        }
        
        return $total;
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
                'keywords' => $row['keywords'] ?? null,
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
     * Transform the OpenGraph table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformOpenGraphTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['urlPathAndQuery'] ?? '',
                'ogTitle' => $row['ogTitle'] ?? null,
                'ogDescription' => $row['ogDescription'] ?? null,
                'ogImage' => $row['ogImage'] ?? null,
                'ogType' => $row['ogType'] ?? null,
                'ogUrl' => $row['ogUrl'] ?? null,
                'ogSiteName' => $row['ogSiteName'] ?? null,
                'twitter' => [
                    'card' => $row['twitterCard'] ?? null,
                    'title' => $row['twitterTitle'] ?? null,
                    'description' => $row['twitterDescription'] ?? null,
                    'image' => $row['twitterImage'] ?? null,
                    'site' => $row['twitterSite'] ?? null,
                    'creator' => $row['twitterCreator'] ?? null
                ]
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Transform the headings table
     * 
     * @param array $rows The table rows
     * @return array The transformed data
     */
    private function transformHeadingsTable(array $rows): array
    {
        $transformed = [];
        
        foreach ($rows as $row) {
            $transformed[] = [
                'url' => $row['urlPathAndQuery'] ?? '',
                'headingsCount' => $row['headingsCount'] ?? 0,
                'headingsErrorsCount' => $row['headingsErrorsCount'] ?? 0,
                'headingTree' => $row['headingTreeItems'] ?? []
            ];
        }
        
        return $transformed;
    }
} 