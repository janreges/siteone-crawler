<?php
/**
 * Check Security Headers Tool Handler for MCP
 * 
 * This class implements the CheckSecurityHeaders tool for MCP.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

use SiteOne\Mcp\CrawlerExecutor;

class CheckSecurityHeadersHandler implements ToolHandlerInterface
{
    /**
     * Crawler executor instance
     */
    private CrawlerExecutor $executor;
    
    /**
     * Security header descriptions
     */
    private array $headerDescriptions = [
        'Strict-Transport-Security' => 'Forces browsers to use HTTPS for future visits to protect against protocol downgrade attacks',
        'Content-Security-Policy' => 'Helps prevent XSS, clickjacking, and other code injection attacks by restricting resource loading',
        'X-Frame-Options' => 'Prevents clickjacking attacks by controlling whether a page can be rendered in a frame',
        'X-Content-Type-Options' => 'Prevents MIME type sniffing attacks by ensuring browser respects declared content types',
        'X-XSS-Protection' => 'Enables browser\'s built-in XSS filtering capabilities',
        'Referrer-Policy' => 'Controls how much referrer information should be included with requests',
        'Feature-Policy' => 'Allows a site to control which features and APIs can be used in the browser',
        'Permissions-Policy' => 'Modern replacement for Feature-Policy that provides more fine-grained control'
    ];
    
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
        return 'siteone/checkSecurityHeaders';
    }
    
    /**
     * {@inheritdoc}
     */
    public function getDescription(): string
    {
        return 'Checks for the presence and configuration of important security-related HTTP headers for a given URL or site.';
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
            'max-depth' => $crawl ? 2 : 0,
            'analyze' => true,
            'analyze-security' => true
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
        // Transform the security table
        $securityHeaders = $this->transformSecurityTable($crawlerOutput['tables']['security']['rows'] ?? []);
        
        // Get HTTP headers data
        $headersData = $this->extractHeadersData($crawlerOutput['tables']['headers']['rows'] ?? []);
        
        // Get HTTP header values
        $headerValues = $this->extractHeaderValues(
            $crawlerOutput['tables']['headers-values']['rows'] ?? [],
            array_keys($securityHeaders)
        );
        
        // Count domains with security headers
        $domains = $this->extractDomains($crawlerOutput['results'] ?? []);
        
        return [
            'summary' => [
                'analyzedUrls' => count($crawlerOutput['results'] ?? []),
                'analyzedDomains' => count($domains),
                'securityScore' => $this->calculateSecurityScore($securityHeaders),
                'crawlDate' => $crawlerOutput['crawler']['executedAt'] ?? null
            ],
            'securityHeaders' => $securityHeaders,
            'headerValues' => $headerValues,
            'missingHeaders' => $this->identifyMissingHeaders(array_keys($securityHeaders)),
            'recommendedHeaders' => $this->getRecommendedHeaders()
        ];
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
            $header = $row['header'] ?? '';
            $status = $this->determineHeaderStatus($row);
            
            $recommendations = [];
            if (isset($row['recommendation']) && !empty($row['recommendation'])) {
                if (is_array($row['recommendation'])) {
                    $recommendations = $row['recommendation'];
                } elseif (is_string($row['recommendation'])) {
                    $recommendations = [$row['recommendation']];
                }
            }
            
            $transformed[$header] = [
                'status' => $status,
                'counts' => [
                    'ok' => (int)($row['ok'] ?? 0),
                    'notice' => (int)($row['notice'] ?? 0),
                    'warning' => (int)($row['warning'] ?? 0),
                    'critical' => (int)($row['critical'] ?? 0)
                ],
                'recommendations' => $recommendations,
                'description' => $this->headerDescriptions[$header] ?? null
            ];
        }
        
        return $transformed;
    }
    
    /**
     * Determine the overall status of a security header
     * 
     * @param array $row The security header row
     * @return string The header status ('ok', 'notice', 'warning', or 'critical')
     */
    private function determineHeaderStatus(array $row): string
    {
        if (($row['critical'] ?? 0) > 0) {
            return 'critical';
        } elseif (($row['warning'] ?? 0) > 0) {
            return 'warning';
        } elseif (($row['notice'] ?? 0) > 0) {
            return 'notice';
        }
        
        return 'ok';
    }
    
    /**
     * Extract header data from the headers table
     * 
     * @param array $rows The headers table rows
     * @return array The extracted header data
     */
    private function extractHeadersData(array $rows): array
    {
        $headersData = [];
        
        foreach ($rows as $row) {
            $header = $row['header'] ?? '';
            
            $headersData[$header] = [
                'occurrences' => (int)($row['occurrences'] ?? 0),
                'uniqueValues' => $row['uniqueValues'] ?? [],
                'valuesPreview' => $row['valuesPreview'] ?? ''
            ];
        }
        
        return $headersData;
    }
    
    /**
     * Extract header values for specific security headers
     * 
     * @param array $rows The header values table rows
     * @param array $securityHeaders Security headers to extract values for
     * @return array The extracted header values
     */
    private function extractHeaderValues(array $rows, array $securityHeaders): array
    {
        $headerValues = [];
        
        foreach ($rows as $row) {
            $header = $row['header'] ?? '';
            
            // Only include security-related headers
            if (!in_array($header, $securityHeaders)) {
                continue;
            }
            
            if (!isset($headerValues[$header])) {
                $headerValues[$header] = [];
            }
            
            $headerValues[$header][] = [
                'value' => $row['value'] ?? '',
                'occurrences' => (int)($row['occurrences'] ?? 0)
            ];
        }
        
        return $headerValues;
    }
    
    /**
     * Extract unique domains from crawler results
     * 
     * @param array $results The crawler results
     * @return array The unique domains
     */
    private function extractDomains(array $results): array
    {
        $domains = [];
        
        foreach ($results as $result) {
            $url = $result['url'] ?? '';
            $domain = parse_url($url, PHP_URL_HOST);
            
            if ($domain && !in_array($domain, $domains)) {
                $domains[] = $domain;
            }
        }
        
        return $domains;
    }
    
    /**
     * Calculate a simple security score based on header status
     * 
     * @param array $securityHeaders The security headers data
     * @return float A score from 0 to 100
     */
    private function calculateSecurityScore(array $securityHeaders): float
    {
        if (empty($securityHeaders)) {
            return 0;
        }
        
        $totalHeaders = count($this->headerDescriptions);
        $presentHeaders = count($securityHeaders);
        $criticalCount = 0;
        $warningCount = 0;
        $noticeCount = 0;
        $okCount = 0;
        
        foreach ($securityHeaders as $header => $data) {
            switch ($data['status']) {
                case 'critical':
                    $criticalCount++;
                    break;
                case 'warning':
                    $warningCount++;
                    break;
                case 'notice':
                    $noticeCount++;
                    break;
                case 'ok':
                    $okCount++;
                    break;
            }
        }
        
        // Calculate the score:
        // - Start with the percentage of headers present
        // - Give full points for 'ok' headers
        // - Subtract for each issue level (notice, warning, critical)
        $baseScore = ($presentHeaders / $totalHeaders) * 100;
        $implementationScore = 0;
        
        if ($presentHeaders > 0) {
            $implementationScore = ($okCount * 100 - $noticeCount * 25 - $warningCount * 50 - $criticalCount * 75) / $presentHeaders;
        }
        
        // Weighted average: 40% for presence, 60% for correct implementation
        $finalScore = ($baseScore * 0.4) + (max(0, $implementationScore) * 0.6);
        
        return round(max(0, min(100, $finalScore)), 1);
    }
    
    /**
     * Identify which recommended security headers are missing
     * 
     * @param array $presentHeaders Headers that are present
     * @return array Missing headers with descriptions
     */
    private function identifyMissingHeaders(array $presentHeaders): array
    {
        $missing = [];
        
        foreach ($this->headerDescriptions as $header => $description) {
            if (!in_array($header, $presentHeaders)) {
                $missing[$header] = [
                    'description' => $description,
                    'impact' => $this->getHeaderImpact($header)
                ];
            }
        }
        
        return $missing;
    }
    
    /**
     * Get the impact level of a missing security header
     * 
     * @param string $header The header name
     * @return string The impact level ('low', 'medium', or 'high')
     */
    private function getHeaderImpact(string $header): string
    {
        $highImpactHeaders = [
            'Content-Security-Policy',
            'Strict-Transport-Security',
            'X-Content-Type-Options'
        ];
        
        $mediumImpactHeaders = [
            'X-Frame-Options',
            'Referrer-Policy',
            'Permissions-Policy'
        ];
        
        if (in_array($header, $highImpactHeaders)) {
            return 'high';
        } elseif (in_array($header, $mediumImpactHeaders)) {
            return 'medium';
        }
        
        return 'low';
    }
    
    /**
     * Get recommended header values for missing or problematic headers
     * 
     * @return array Recommended header values
     */
    private function getRecommendedHeaders(): array
    {
        return [
            'Strict-Transport-Security' => [
                'value' => 'max-age=31536000; includeSubDomains; preload',
                'explanation' => 'Enforces HTTPS for 1 year, includes subdomains, and allows preloading in browsers'
            ],
            'Content-Security-Policy' => [
                'value' => "default-src 'self'; script-src 'self'; object-src 'none'; upgrade-insecure-requests;",
                'explanation' => 'Basic policy that allows resources only from the same origin and upgrades HTTP to HTTPS'
            ],
            'X-Frame-Options' => [
                'value' => 'SAMEORIGIN',
                'explanation' => 'Allows framing only by pages from the same origin'
            ],
            'X-Content-Type-Options' => [
                'value' => 'nosniff',
                'explanation' => 'Prevents MIME type sniffing'
            ],
            'Referrer-Policy' => [
                'value' => 'strict-origin-when-cross-origin',
                'explanation' => 'Sends full URL for same-origin requests, only origin for cross-origin requests'
            ],
            'Permissions-Policy' => [
                'value' => 'camera=(), microphone=(), geolocation=(self), payment=()',
                'explanation' => 'Restricts access to sensitive features like camera and microphone'
            ]
        ];
    }
} 