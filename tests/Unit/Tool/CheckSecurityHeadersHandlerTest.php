<?php
/**
 * CheckSecurityHeadersHandler Test
 * 
 * Unit test for the CheckSecurityHeadersHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\CheckSecurityHeadersHandler;

class CheckSecurityHeadersHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var CheckSecurityHeadersHandler
     */
    private $handler;
    
    /**
     * Set up the test case
     */
    protected function setUp(): void
    {
        // Create a mock for the CrawlerExecutor
        $this->crawlerExecutor = $this->createMock(CrawlerExecutor::class);
        
        // Create the handler with the mock executor
        $this->handler = new CheckSecurityHeadersHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/checkSecurityHeaders', $this->handler->getName());
    }
    
    /**
     * Test that getDescription returns a non-empty string
     */
    public function testGetDescription(): void
    {
        $description = $this->handler->getDescription();
        $this->assertIsString($description);
        $this->assertNotEmpty($description);
    }
    
    /**
     * Test that getParameterSchema returns the expected schema
     */
    public function testGetParameterSchema(): void
    {
        $schema = $this->handler->getParameterSchema();
        
        // Check the schema structure
        $this->assertIsArray($schema);
        $this->assertEquals('object', $schema['type']);
        $this->assertArrayHasKey('properties', $schema);
        $this->assertArrayHasKey('required', $schema);
        
        // Check required properties
        $this->assertContains('url', $schema['required']);
        
        // Check property definitions
        $this->assertArrayHasKey('url', $schema['properties']);
        $this->assertArrayHasKey('crawl', $schema['properties']);
    }
    
    /**
     * Test that execute throws an exception when the url parameter is missing
     */
    public function testExecuteWithMissingUrl(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('Missing required parameter: url');
        
        $this->handler->execute([]);
    }
    
    /**
     * Test that execute calls the crawler executor with the correct parameters when crawl is false
     */
    public function testExecuteWithCrawlFalse(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'crawl' => false
        ];
        
        $expectedCrawlerParams = [
            'url' => 'https://example.com',
            'max-depth' => 0,
            'analyze' => true,
            'analyze-headers' => true
        ];
        
        $mockResult = $this->createMockCrawlerResult();
        
        // Set up the mock to expect the execute method to be called with the expected parameters
        $this->crawlerExecutor
            ->expects($this->once())
            ->method('execute')
            ->with($this->equalTo($expectedCrawlerParams))
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert - Check the result structure
        $this->assertIsArray($result);
        $this->assertArrayHasKey('summary', $result);
        $this->assertArrayHasKey('securityScore', $result);
        $this->assertArrayHasKey('headers', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('securityHeadersFound', $result['summary']);
        $this->assertArrayHasKey('securityHeadersMissing', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
        
        // Verify headers were analyzed correctly
        $this->assertCount(2, $result['headers']);
        
        // Check security score calculation
        $this->assertGreaterThanOrEqual(0, $result['securityScore']);
        $this->assertLessThanOrEqual(100, $result['securityScore']);
    }
    
    /**
     * Test that execute calls the crawler executor with the correct parameters when crawl is true
     */
    public function testExecuteWithCrawlTrue(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'crawl' => true
        ];
        
        $expectedCrawlerParams = [
            'url' => 'https://example.com',
            'max-depth' => 1,
            'analyze' => true,
            'analyze-headers' => true
        ];
        
        $mockResult = $this->createMockCrawlerResult();
        
        // Set up the mock to expect the execute method to be called with the expected parameters
        $this->crawlerExecutor
            ->expects($this->once())
            ->method('execute')
            ->with($this->equalTo($expectedCrawlerParams))
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertIsArray($result);
        $this->assertArrayHasKey('headers', $result);
    }
    
    /**
     * Test handling of missing headers
     */
    public function testExecuteWithMissingHeaders(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                [
                    'url' => 'https://example.com',
                    'type' => 1,
                    'status' => '200',
                    'headers' => []
                ]
            ],
            'tables' => []
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertEquals(0, $result['summary']['securityHeadersFound']);
        $this->assertGreaterThan(0, $result['summary']['securityHeadersMissing']);
        $this->assertEquals(0, $result['securityScore']);
    }
    
    /**
     * Test the security score calculation
     */
    public function testSecurityScoreCalculation(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                [
                    'url' => 'https://example.com',
                    'type' => 1,
                    'status' => '200',
                    'headers' => [
                        // All important security headers present with good values
                        'Strict-Transport-Security' => 'max-age=31536000; includeSubDomains; preload',
                        'Content-Security-Policy' => "default-src 'self'",
                        'X-Frame-Options' => 'DENY',
                        'X-Content-Type-Options' => 'nosniff',
                        'X-XSS-Protection' => '1; mode=block',
                        'Referrer-Policy' => 'no-referrer',
                        'Permissions-Policy' => 'geolocation=(), microphone=()'
                    ]
                ]
            ],
            'tables' => []
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        // With all important headers present, score should be high
        $this->assertGreaterThan(70, $result['securityScore']);
    }
    
    /**
     * Create a mock crawler result with headers
     */
    private function createMockCrawlerResult(): array
    {
        return [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                [
                    'url' => 'https://example.com',
                    'type' => 1,
                    'status' => '200',
                    'headers' => [
                        'Strict-Transport-Security' => 'max-age=31536000',
                        'X-Frame-Options' => 'SAMEORIGIN'
                    ]
                ]
            ],
            'tables' => [
                'headers' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com',
                            'header' => 'Strict-Transport-Security',
                            'value' => 'max-age=31536000'
                        ],
                        [
                            'url' => 'https://example.com',
                            'header' => 'X-Frame-Options',
                            'value' => 'SAMEORIGIN'
                        ]
                    ]
                ]
            ]
        ];
    }
} 