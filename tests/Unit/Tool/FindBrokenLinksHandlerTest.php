<?php
/**
 * FindBrokenLinksHandler Test
 * 
 * Unit test for the FindBrokenLinksHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\FindBrokenLinksHandler;

class FindBrokenLinksHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var FindBrokenLinksHandler
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
        $this->handler = new FindBrokenLinksHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/findBrokenLinks', $this->handler->getName());
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
        $this->assertArrayHasKey('depth', $schema['properties']);
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
     * Test that execute calls the crawler executor with the correct parameters
     */
    public function testExecuteWithValidParameters(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'depth' => 2
        ];
        
        $expectedCrawlerParams = [
            'url' => 'https://example.com',
            'max-depth' => 2,
            'analyze' => true,
            'analyze-broken-links' => true
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/broken', 'type' => 1, 'status' => '404']
            ],
            'tables' => [
                '404' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com/missing',
                            'statusCode' => 404,
                            'foundOnUrl' => 'https://example.com',
                            'foundOnTitle' => 'Example Domain'
                        ]
                    ]
                ],
                'redirects' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com/redirect',
                            'statusCode' => 301,
                            'location' => 'https://example.com/new-page',
                            'foundOnUrl' => 'https://example.com',
                            'foundOnTitle' => 'Example Domain'
                        ]
                    ]
                ],
                'skipped' => [
                    'rows' => [
                        [
                            'url' => 'https://external.com',
                            'reason' => 'external domain'
                        ]
                    ]
                ]
            ]
        ];
        
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
        $this->assertArrayHasKey('brokenLinks', $result);
        $this->assertArrayHasKey('redirects', $result);
        $this->assertArrayHasKey('skippedUrls', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('totalBrokenLinks', $result['summary']);
        $this->assertArrayHasKey('totalRedirects', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
        
        // Verify broken links were transformed correctly
        $this->assertCount(1, $result['brokenLinks']);
        $this->assertEquals('https://example.com/missing', $result['brokenLinks'][0]['url']);
        $this->assertEquals(404, $result['brokenLinks'][0]['statusCode']);
        
        // Verify redirects were transformed correctly
        $this->assertCount(1, $result['redirects']);
        $this->assertEquals('https://example.com/redirect', $result['redirects'][0]['url']);
        $this->assertEquals(301, $result['redirects'][0]['statusCode']);
        
        // Verify skipped URLs were transformed correctly
        $this->assertCount(1, $result['skippedUrls']);
        $this->assertEquals('https://external.com', $result['skippedUrls'][0]['url']);
    }
    
    /**
     * Test handling of empty result tables
     */
    public function testExecuteWithEmptyResults(): void
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
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200']
            ],
            'tables' => [
                '404' => [
                    'rows' => []
                ],
                'redirects' => [
                    'rows' => []
                ],
                'skipped' => [
                    'rows' => []
                ]
            ]
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertEquals(0, $result['summary']['totalBrokenLinks']);
        $this->assertEquals(0, $result['summary']['totalRedirects']);
        $this->assertEmpty($result['brokenLinks']);
        $this->assertEmpty($result['redirects']);
        $this->assertEmpty($result['skippedUrls']);
    }
} 