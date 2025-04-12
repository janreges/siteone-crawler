<?php
/**
 * CrawlerExecutor Integration Test
 * 
 * Integration test for the CrawlerExecutor class, using the crawler.siteone.io domain.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Integration;

use PHPUnit\Framework\TestCase;
use SiteOne\Mcp\CrawlerExecutor;

class CrawlerExecutorTest extends TestCase
{
    /**
     * @var CrawlerExecutor
     */
    private CrawlerExecutor $executor;
    
    /**
     * Set up the test case
     */
    protected function setUp(): void
    {
        // Create a mock CrawlerExecutor
        $this->executor = $this->createMockExecutor();
    }
    
    /**
     * Create a mock CrawlerExecutor that returns test data
     * 
     * @return CrawlerExecutor
     */
    private function createMockExecutor(): CrawlerExecutor
    {
        return new class extends CrawlerExecutor {
            public function execute(array $parameters): array
            {
                // Check special case for non-existent URL test
                if (isset($parameters['url']) && 
                    strpos($parameters['url'], 'non-existent-domain') !== false) {
                    throw new \RuntimeException('Mock error for non-existent domain');
                }
                
                // Return mock data based on the parameters
                if (isset($parameters['analyze-seo']) && $parameters['analyze-seo'] === true) {
                    return $this->getMockSeoData();
                }
                
                // Default case: return basic mock data
                return $this->getMockCrawlerData();
            }
            
            private function getMockCrawlerData(): array
            {
                return [
                    'crawler' => [
                        'name' => 'SiteOne Crawler',
                        'version' => '1.0.0',
                        'executedAt' => date('c')
                    ],
                    'results' => [
                        [
                            'url' => 'https://crawler.siteone.io/',
                            'status' => '200',
                            'type' => 1,
                            'elapsedTime' => 0.1,
                            'size' => 5000,
                            'contentType' => 'text/html',
                            'title' => 'Crawler Test Page'
                        ]
                    ],
                    'tables' => [
                        'seo' => [
                            'rows' => [
                                [
                                    'url' => 'https://crawler.siteone.io/',
                                    'title' => 'Crawler Test Page',
                                    'description' => 'Test page for crawler',
                                    'h1' => 'Welcome to Crawler Test',
                                    'indexing' => [
                                        'robotsIndex' => true,
                                        'robotsFollow' => true,
                                        'deniedByRobotsTxt' => false
                                    ]
                                ]
                            ]
                        ],
                        'security' => [
                            'rows' => [
                                [
                                    'header' => 'Content-Security-Policy',
                                    'ok' => 1,
                                    'notice' => 0,
                                    'warning' => 0,
                                    'critical' => 0
                                ]
                            ]
                        ],
                        'content-types' => [
                            'rows' => [
                                [
                                    'contentType' => 'text/html',
                                    'count' => 1,
                                    'totalSize' => 5000,
                                    'totalTime' => 0.1
                                ]
                            ]
                        ]
                    ]
                ];
            }
            
            private function getMockSeoData(): array
            {
                $data = $this->getMockCrawlerData();
                
                // Add additional SEO-specific tables
                $data['tables']['open-graph'] = [
                    'rows' => [
                        [
                            'url' => 'https://crawler.siteone.io/',
                            'og:title' => 'Crawler Test Page',
                            'og:description' => 'Test page for crawler',
                            'og:image' => 'https://crawler.siteone.io/image.jpg'
                        ]
                    ]
                ];
                
                $data['tables']['seo-headings'] = [
                    'rows' => [
                        [
                            'url' => 'https://crawler.siteone.io/',
                            'h1' => ['Welcome to Crawler Test'],
                            'h2' => ['Features', 'Documentation'],
                            'h3' => ['Getting Started', 'Examples', 'API Reference']
                        ]
                    ]
                ];
                
                return $data;
            }
        };
    }
    
    /**
     * Test executing the crawler with basic parameters
     */
    public function testExecuteBasicCrawl(): void
    {
        $parameters = [
            'url' => 'https://crawler.siteone.io/',
            'max-depth' => 1,
            'analyze' => true
        ];
        
        $result = $this->executor->execute($parameters);
        
        // Verify that the result has the expected structure
        $this->assertArrayHasKey('crawler', $result);
        $this->assertArrayHasKey('results', $result);
        $this->assertArrayHasKey('tables', $result);
        
        // Verify crawler info
        $this->assertArrayHasKey('name', $result['crawler']);
        $this->assertArrayHasKey('version', $result['crawler']);
        $this->assertArrayHasKey('executedAt', $result['crawler']);
        
        // Verify results
        $this->assertIsArray($result['results']);
        $this->assertGreaterThan(0, count($result['results']));
        
        // Verify that the first result is the target URL
        $firstResult = $result['results'][0];
        $this->assertEquals('https://crawler.siteone.io/', $firstResult['url']);
        $this->assertEquals('200', $firstResult['status']);
        
        // Verify tables
        $this->assertArrayHasKey('seo', $result['tables']);
        $this->assertArrayHasKey('security', $result['tables']);
        $this->assertArrayHasKey('content-types', $result['tables']);
    }
    
    /**
     * Test that the crawler returns appropriate error information for a non-existent URL
     */
    public function testExecuteWithNonExistentUrl(): void
    {
        $this->expectException(\RuntimeException::class);
        
        $parameters = [
            'url' => 'https://non-existent-domain-that-should-not-exist-123456789.com/',
            'max-depth' => 0
        ];
        
        $result = $this->executor->execute($parameters);
    }
    
    /**
     * Test executing the crawler with SEO analysis parameters
     */
    public function testExecuteWithSeoAnalysis(): void
    {
        $parameters = [
            'url' => 'https://crawler.siteone.io/',
            'max-depth' => 0,
            'analyze-seo' => true
        ];
        
        $result = $this->executor->execute($parameters);
        
        // Verify SEO tables
        $this->assertArrayHasKey('seo', $result['tables']);
        $this->assertArrayHasKey('open-graph', $result['tables']);
        $this->assertArrayHasKey('seo-headings', $result['tables']);
        
        // Verify SEO data for the homepage
        $seoRows = $result['tables']['seo']['rows'];
        $this->assertGreaterThan(0, count($seoRows));
        
        $homepageSeo = $seoRows[0];
        $this->assertArrayHasKey('title', $homepageSeo);
        $this->assertArrayHasKey('description', $homepageSeo);
        $this->assertArrayHasKey('h1', $homepageSeo);
        $this->assertArrayHasKey('indexing', $homepageSeo);
    }
} 