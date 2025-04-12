<?php
/**
 * Generate Markdown Tool Handler for MCP
 * 
 * This class implements the GenerateMarkdown tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class GenerateMarkdownHandler implements ToolHandlerInterface
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
        return 'siteone/generateMarkdown';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Uses the crawler\'s website-to-markdown conversion feature to export a website or specific page(s) into Markdown format.';
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
                    'description' => 'The URL to convert to Markdown (required)'
                ],
                'outputDir' => [
                    'type' => 'string',
                    'description' => 'The directory where the Markdown files will be saved (required)'
                ],
                'depth' => [
                    'type' => 'integer',
                    'description' => 'Maximum depth to crawl (optional, default 1)',
                    'default' => 1
                ],
                'includeImages' => [
                    'type' => 'boolean',
                    'description' => 'Whether to download and include images (optional, default true)',
                    'default' => true
                ],
                'includeLinks' => [
                    'type' => 'boolean',
                    'description' => 'Whether to preserve links in the markdown (optional, default true)',
                    'default' => true
                ],
                'frontMatter' => [
                    'type' => 'boolean',
                    'description' => 'Whether to include front matter in generated markdown files (optional, default false)',
                    'default' => false
                ],
                'githubFlavor' => [
                    'type' => 'boolean',
                    'description' => 'Whether to use GitHub flavored markdown (optional, default true)',
                    'default' => true
                ],
                'includeAssets' => [
                    'type' => 'boolean',
                    'description' => 'Whether to download and include assets like CSS and JS (optional, default false)',
                    'default' => false
                ],
                'preserveHtml' => [
                    'type' => 'boolean',
                    'description' => 'Whether to preserve HTML that cannot be converted to Markdown (optional, default true)',
                    'default' => true
                ],
                'singleFile' => [
                    'type' => 'boolean',
                    'description' => 'Whether to generate a single Markdown file (optional, default false)',
                    'default' => false
                ]
            ],
            'required' => ['url', 'outputDir']
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
        
        if (!isset($parameters['outputDir']) || empty($parameters['outputDir'])) {
            throw new \RuntimeException('Missing required parameter: outputDir');
        }
        
        // Get parameters with defaults
        $url = $parameters['url'];
        $outputDir = $parameters['outputDir'];
        $depth = $parameters['depth'] ?? 1;
        $includeImages = $parameters['includeImages'] ?? true;
        $includeLinks = $parameters['includeLinks'] ?? true;
        $frontMatter = $parameters['frontMatter'] ?? false;
        $githubFlavor = $parameters['githubFlavor'] ?? true;
        $includeAssets = $parameters['includeAssets'] ?? false;
        $preserveHtml = $parameters['preserveHtml'] ?? true;
        $singleFile = $parameters['singleFile'] ?? false;
        
        // Ensure output directory exists
        $this->ensureDirectoryExists($outputDir);
        
        // Execute the crawler with appropriate parameters
        $crawlerParams = [
            'url' => $url,
            'max-depth' => $depth,
            'export-md' => true,
            'md-dir' => $outputDir,
            'md-include-images' => $includeImages,
            'md-include-links' => $includeLinks,
            'md-front-matter' => $frontMatter,
            'md-github-flavor' => $githubFlavor
        ];
        
        // Run the crawler
        $result = $this->executor->execute($crawlerParams);
        
        // Transform the crawler output into MCP tool result
        return $this->transformOutput($result, $outputDir);
    }
    
    /**
     * Transform the crawler output into a structured MCP tool result
     * 
     * @param array $crawlerOutput The crawler output
     * @param string $outputDir The output directory
     * @return array The structured MCP tool result
     */
    private function transformOutput(array $crawlerOutput, string $outputDir): array
    {
        // Extract markdown export information if available
        $markdownInfo = $this->extractMarkdownInfo($crawlerOutput);
        
        // List the generated markdown files
        $generatedFiles = $this->listGeneratedFiles($outputDir);
        
        // Extract pages from markdown table
        $pages = $this->extractPages($crawlerOutput);
        
        return [
            'success' => true,
            'summary' => [
                'crawledUrls' => count($crawlerOutput['results'] ?? []),
                'convertedPages' => count($pages),
                'totalContentSize' => $this->calculateTotalContentSize($crawlerOutput),
                'outputDirectory' => $outputDir,
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'markdownInfo' => $markdownInfo,
            'generatedFiles' => $generatedFiles,
            'pages' => $pages
        ];
    }
    
    /**
     * Extract markdown export information from crawler output
     * 
     * @param array $crawlerOutput The crawler output
     * @return array The Markdown export information
     */
    private function extractMarkdownInfo(array $crawlerOutput): array
    {
        // Look for any Markdown export information in the tables
        // This will depend on the specific structure of the crawler's output
        $markdownInfo = [
            'processedPages' => 0,
            'skippedPages' => 0,
            'totalSize' => 0
        ];
        
        // Count HTML pages that were likely processed for Markdown
        foreach ($crawlerOutput['results'] ?? [] as $result) {
            if (($result['type'] ?? 0) === 1) { // HTML type
                $markdownInfo['processedPages']++;
                $markdownInfo['totalSize'] += (int)($result['size'] ?? 0);
            }
        }
        
        // Count skipped pages that couldn't be processed
        if (isset($crawlerOutput['tables']['skipped']['rows'])) {
            $markdownInfo['skippedPages'] = count($crawlerOutput['tables']['skipped']['rows']);
        }
        
        return $markdownInfo;
    }
    
    /**
     * List the Markdown files generated in the output directory
     * 
     * @param string $outputDir The output directory
     * @return array The list of generated files
     */
    private function listGeneratedFiles(string $outputDir): array
    {
        $files = [];
        
        // Make sure the directory exists and is readable
        if (!is_dir($outputDir) || !is_readable($outputDir)) {
            return $files;
        }
        
        // List all markdown files in the directory and its subdirectories
        $iterator = new \RecursiveIteratorIterator(
            new \RecursiveDirectoryIterator(
                $outputDir,
                \FilesystemIterator::SKIP_DOTS | \FilesystemIterator::UNIX_PATHS
            )
        );
        
        foreach ($iterator as $file) {
            if ($file->isFile() && $file->getExtension() === 'md') {
                $relativePath = str_replace($outputDir, '', $file->getPathname());
                $relativePath = ltrim($relativePath, '/\\');
                
                $files[] = [
                    'path' => $relativePath,
                    'size' => $file->getSize(),
                    'lastModified' => $file->getMTime()
                ];
            }
        }
        
        // Sort files by path
        usort($files, function($a, $b) {
            return strcmp($a['path'], $b['path']);
        });
        
        return $files;
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
    
    /**
     * Extract pages from markdown table
     * 
     * @param array $crawlerOutput The crawler output
     * @return array The list of pages
     */
    private function extractPages(array $crawlerOutput): array
    {
        $pages = [];
        
        if (isset($crawlerOutput['tables']['markdown']['rows'])) {
            foreach ($crawlerOutput['tables']['markdown']['rows'] as $row) {
                $pages[] = [
                    'url' => $row['url'] ?? '',
                    'file' => $row['outputFile'] ?? '',
                    'title' => $row['title'] ?? '',
                    'contentLength' => $row['contentLength'] ?? 0
                ];
            }
        }
        
        return $pages;
    }
    
    /**
     * Calculate total content size from markdown tables
     * 
     * @param array $crawlerOutput The crawler output
     * @return int The total content size
     */
    private function calculateTotalContentSize(array $crawlerOutput): int
    {
        $totalSize = 0;
        
        if (isset($crawlerOutput['tables']['markdown']['rows'])) {
            foreach ($crawlerOutput['tables']['markdown']['rows'] as $row) {
                $totalSize += (int)($row['contentLength'] ?? 0);
            }
        }
        
        return $totalSize;
    }
} 