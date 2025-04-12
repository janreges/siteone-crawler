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
            'max-depth' => $crawl ? 1 : 0,
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
        // Extract pages from results
        $pages = $this->extractPages($crawlerOutput['results'] ?? []);
        
        // Detect SEO issues
        $issues = $this->detectSeoIssues($crawlerOutput['results'] ?? []);
        
        return [
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'pagesWithTitle' => $this->countPagesWithProperty($pages, 'title'),
                'pagesWithDescription' => $this->countPagesWithProperty($pages, 'description'),
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null,
                'nonUniqueMetaData' => [
                    'titles' => $this->summarizeNonUniqueTable($crawlerOutput['tables']['non-unique-titles']['rows'] ?? []),
                    'descriptions' => $this->summarizeNonUniqueTable($crawlerOutput['tables']['non-unique-descriptions']['rows'] ?? [])
                ],
                'headingIssuesCount' => $this->countHeadingIssues($crawlerOutput['tables']['seo-headings']['rows'] ?? [])
            ],
            'pages' => $pages,
            'issues' => $issues,
            'pageMetadata' => $this->transformSeoTable($crawlerOutput['tables']['seo']['rows'] ?? []),
            'openGraph' => $this->transformOpenGraphTable($crawlerOutput['tables']['open-graph']['rows'] ?? []),
            'headings' => $this->transformHeadingsTable($crawlerOutput['tables']['seo-headings']['rows'] ?? [])
        ];
    }
    
    /**
     * Extract pages from crawler results
     * 
     * @param array $results The crawler results
     * @return array The extracted pages
     */
    private function extractPages(array $results): array
    {
        $pages = [];
        
        foreach ($results as $result) {
            // Only include HTML pages
            if (($result['type'] ?? 0) === 1) {
                $pages[] = [
                    'url' => $result['url'] ?? '',
                    'title' => $result['title'] ?? '',
                    'description' => $result['metaDescription'] ?? '',
                    'h1' => $result['h1Text'] ?? '',
                    'h1Count' => $result['h1Count'] ?? 0,
                    'metaTags' => $result['metaTags'] ?? []
                ];
            }
        }
        
        return $pages;
    }
    
    /**
     * Detect SEO issues in crawler results
     * 
     * @param array $results The crawler results
     * @return array The detected issues
     */
    private function detectSeoIssues(array $results): array
    {
        $issues = [];
        
        foreach ($results as $result) {
            // Only analyze HTML pages
            if (($result['type'] ?? 0) !== 1) {
                continue;
            }
            
            $url = $result['url'] ?? '';
            
            // Check for missing title
            if (empty($result['title'] ?? '')) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'missing_title',
                    'severity' => 'critical',
                    'message' => 'Page is missing a title tag'
                ];
            } elseif (mb_strlen($result['title'] ?? '') < 10) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'short_title',
                    'severity' => 'warning',
                    'message' => 'Page title is too short (less than 10 characters)'
                ];
            } elseif (mb_strlen($result['title'] ?? '') > 70) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'long_title',
                    'severity' => 'warning',
                    'message' => 'Page title is too long (more than 70 characters)'
                ];
            }
            
            // Check for missing meta description
            if (empty($result['metaDescription'] ?? '')) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'missing_description',
                    'severity' => 'critical',
                    'message' => 'Page is missing a meta description'
                ];
            } elseif (mb_strlen($result['metaDescription'] ?? '') < 50) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'short_description',
                    'severity' => 'warning',
                    'message' => 'Meta description is too short (less than 50 characters)'
                ];
            } elseif (mb_strlen($result['metaDescription'] ?? '') > 160) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'long_description',
                    'severity' => 'warning',
                    'message' => 'Meta description is too long (more than 160 characters)'
                ];
            }
            
            // Check H1 issues
            if (($result['h1Count'] ?? 0) === 0) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'missing_h1',
                    'severity' => 'critical',
                    'message' => 'Page has no H1 heading'
                ];
            } elseif (($result['h1Count'] ?? 0) > 1) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'multiple_h1',
                    'severity' => 'warning',
                    'message' => 'Page has multiple H1 headings'
                ];
            }
            
            // Check for mismatch between title and H1
            if (!empty($result['title'] ?? '') && !empty($result['h1Text'] ?? '') && 
                $result['title'] !== $result['h1Text']) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'title_h1_mismatch',
                    'severity' => 'notice',
                    'message' => 'Title and H1 heading do not match'
                ];
            }
            
            // Check for OpenGraph issues
            $metaTags = $result['metaTags'] ?? [];
            if (isset($metaTags['og:title']) && $metaTags['og:title'] !== $result['title']) {
                $issues[] = [
                    'url' => $url,
                    'type' => 'og_title_mismatch',
                    'severity' => 'notice',
                    'message' => 'OpenGraph title does not match page title'
                ];
            }
        }
        
        return $issues;
    }
    
    /**
     * Count pages with a non-empty property
     * 
     * @param array $pages The pages
     * @param string $property The property to check
     * @return int The count of pages with the property
     */
    private function countPagesWithProperty(array $pages, string $property): int
    {
        $count = 0;
        
        foreach ($pages as $page) {
            if (!empty($page[$property] ?? '')) {
                $count++;
            }
        }
        
        return $count;
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