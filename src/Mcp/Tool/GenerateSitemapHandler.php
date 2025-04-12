<?php
/**
 * Generate Sitemap Tool Handler for MCP
 * 
 * This class implements the GenerateSitemap tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class GenerateSitemapHandler implements ToolHandlerInterface
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
        return 'siteone/generateSitemap';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Leverages the crawler\'s sitemap generation feature to create an XML sitemap for the website based on the crawled pages.';
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
                    'description' => 'The URL to generate a sitemap for (required)'
                ],
                'outputFile' => [
                    'type' => 'string',
                    'description' => 'The path where the sitemap file will be saved (required)'
                ],
                'depth' => [
                    'type' => 'integer',
                    'description' => 'Maximum depth to crawl (optional, default 1)',
                    'default' => 1
                ],
                'includeImages' => [
                    'type' => 'boolean',
                    'description' => 'Whether to include image information in the sitemap (optional, default false)',
                    'default' => false
                ],
                'priority' => [
                    'type' => 'number',
                    'description' => 'Base priority for URLs in the sitemap (optional, default 0.5)',
                    'default' => 0.5,
                    'minimum' => 0.0,
                    'maximum' => 1.0
                ],
                'changefreq' => [
                    'type' => 'string',
                    'description' => 'Default change frequency for URLs (optional, default "weekly")',
                    'default' => 'weekly',
                    'enum' => ['always', 'hourly', 'daily', 'weekly', 'monthly', 'yearly', 'never']
                ]
            ],
            'required' => ['url', 'outputFile']
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
        
        if (!isset($parameters['outputFile']) || empty($parameters['outputFile'])) {
            throw new \RuntimeException('Missing required parameter: outputFile');
        }
        
        // Get parameters with defaults
        $url = $parameters['url'];
        $outputFile = $parameters['outputFile'];
        $depth = $parameters['depth'] ?? 1;
        $includeImages = $parameters['includeImages'] ?? false;
        $priority = $parameters['priority'] ?? 0.5;
        $changefreq = $parameters['changefreq'] ?? 'weekly';
        
        // Ensure output directory exists
        $outputDir = dirname($outputFile);
        $this->ensureDirectoryExists($outputDir);
        
        // Execute the crawler with appropriate parameters
        $crawlerParams = [
            'url' => $url,
            'max-depth' => $depth,
            'export-sitemap' => true,
            'sitemap-file' => $outputFile,
            'sitemap-include-images' => $includeImages,
            'sitemap-priority' => $priority,
            'sitemap-changefreq' => $changefreq
        ];
        
        // Run the crawler
        $result = $this->executor->execute($crawlerParams);
        
        // Transform the crawler output into MCP tool result
        return $this->transformOutput($result, $outputFile);
    }
    
    /**
     * Transform the crawler output into a structured MCP tool result
     * 
     * @param array $crawlerOutput The crawler output
     * @param string $outputFile The output file path
     * @return array The structured MCP tool result
     */
    private function transformOutput(array $crawlerOutput, string $outputFile): array
    {
        // Check if the sitemap file was generated
        $sitemapInfo = $this->getSitemapInfo($outputFile);
        
        // Count HTML URLs that would be included in the sitemap
        $htmlUrlCount = $this->countHtmlUrls($crawlerOutput['results'] ?? []);
        
        return [
            'success' => $sitemapInfo['exists'],
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'htmlUrls' => $htmlUrlCount,
                'sitemapFile' => $outputFile,
                'sitemapSize' => $sitemapInfo['size'],
                'sitemapLastModified' => $sitemapInfo['lastModified'],
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'domains' => $this->extractDomains($crawlerOutput['results'] ?? [])
        ];
    }
    
    /**
     * Get information about the generated sitemap file
     * 
     * @param string $outputFile The path to the sitemap file
     * @return array Information about the sitemap file
     */
    private function getSitemapInfo(string $outputFile): array
    {
        $info = [
            'exists' => false,
            'size' => 0,
            'lastModified' => null,
            'urlCount' => 0
        ];
        
        if (file_exists($outputFile) && is_file($outputFile) && is_readable($outputFile)) {
            $info['exists'] = true;
            $info['size'] = filesize($outputFile);
            $info['lastModified'] = filemtime($outputFile);
            
            // Try to count URLs in the sitemap
            $info['urlCount'] = $this->countUrlsInSitemap($outputFile);
        }
        
        return $info;
    }
    
    /**
     * Count URLs in a sitemap file
     * 
     * @param string $sitemapFile The path to the sitemap file
     * @return int The number of URLs in the sitemap
     */
    private function countUrlsInSitemap(string $sitemapFile): int
    {
        try {
            // Load the sitemap XML
            $xml = @simplexml_load_file($sitemapFile);
            
            if ($xml === false) {
                return 0;
            }
            
            // Register the sitemap namespace
            $xml->registerXPathNamespace('s', 'http://www.sitemaps.org/schemas/sitemap/0.9');
            
            // Count <url> elements
            $urls = $xml->xpath('//s:url');
            
            return count($urls);
        } catch (\Exception $e) {
            // If there's any error, return 0
            return 0;
        }
    }
    
    /**
     * Count HTML URLs in the crawler results
     * 
     * @param array $results The crawler results
     * @return int The number of HTML URLs
     */
    private function countHtmlUrls(array $results): int
    {
        $count = 0;
        
        foreach ($results as $result) {
            // HTML type is usually represented as 1
            if (($result['type'] ?? 0) === 1 && ($result['status'] ?? '') === '200') {
                $count++;
            }
        }
        
        return $count;
    }
    
    /**
     * Extract unique domains from crawler results
     * 
     * @param array $results The crawler results
     * @return array The list of domains with URL counts
     */
    private function extractDomains(array $results): array
    {
        $domains = [];
        
        foreach ($results as $result) {
            $url = $result['url'] ?? '';
            $domain = parse_url($url, PHP_URL_HOST);
            
            if (!$domain) {
                continue;
            }
            
            if (!isset($domains[$domain])) {
                $domains[$domain] = [
                    'name' => $domain,
                    'totalUrls' => 0,
                    'htmlUrls' => 0
                ];
            }
            
            $domains[$domain]['totalUrls']++;
            
            // Count HTML URLs
            if (($result['type'] ?? 0) === 1 && ($result['status'] ?? '') === '200') {
                $domains[$domain]['htmlUrls']++;
            }
        }
        
        // Convert to a simple array
        $result = array_values($domains);
        
        // Sort by domain name
        usort($result, function($a, $b) {
            return strcmp($a['name'], $b['name']);
        });
        
        return $result;
    }
    
    /**
     * Ensure that a directory exists and is writable
     * 
     * @param string $dir The directory path
     * @throws \RuntimeException If the directory cannot be created or is not writable
     */
    private function ensureDirectoryExists(string $dir): void
    {
        if (!file_exists($dir)) {
            if (!mkdir($dir, 0755, true) && !is_dir($dir)) {
                throw new \RuntimeException(sprintf('Directory "%s" could not be created', $dir));
            }
        }
        
        if (!is_dir($dir)) {
            throw new \RuntimeException(sprintf('"%s" is not a directory', $dir));
        }
        
        if (!is_writable($dir)) {
            throw new \RuntimeException(sprintf('Directory "%s" is not writable', $dir));
        }
    }
} 