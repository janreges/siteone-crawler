<?php
/**
 * GetWebsitePerformanceHandler Test
 * 
 * Unit test for the GetWebsitePerformanceHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\GetWebsitePerformanceHandler;

class GetWebsitePerformanceHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var GetWebsitePerformanceHandler
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
        $this->handler = new GetWebsitePerformanceHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/getWebsitePerformance', $this->handler->getName());
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
            'analyze-performance' => true,
            'record-response-time' => true
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
                    'time' => 150, // 150ms
                    'contentLength' => 10240 // 10KB
                ],
                [
                    'url' => 'https://example.com/fast',
                    'type' => 1,
                    'status' => '200',
                    'time' => 50, // 50ms
                    'contentLength' => 5120 // 5KB
                ],
                [
                    'url' => 'https://example.com/slow',
                    'type' => 1,
                    'status' => '200',
                    'time' => 500, // 500ms
                    'contentLength' => 102400 // 100KB
                ]
            ],
            'tables' => [
                'performance' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com',
                            'responseTime' => 150,
                            'contentLength' => 10240,
                            'contentType' => 'text/html'
                        ],
                        [
                            'url' => 'https://example.com/fast',
                            'responseTime' => 50,
                            'contentLength' => 5120,
                            'contentType' => 'text/html'
                        ],
                        [
                            'url' => 'https://example.com/slow',
                            'responseTime' => 500,
                            'contentLength' => 102400,
                            'contentType' => 'text/html'
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
        $this->assertArrayHasKey('slowestPages', $result);
        $this->assertArrayHasKey('fastestPages', $result);
        $this->assertArrayHasKey('largestPages', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('averageResponseTime', $result['summary']);
        $this->assertArrayHasKey('totalContentSize', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
        
        // Verify slowest pages were calculated correctly
        $this->assertCount(3, $result['slowestPages']);
        $this->assertEquals('https://example.com/slow', $result['slowestPages'][0]['url']);
        $this->assertEquals(500, $result['slowestPages'][0]['responseTime']);
        
        // Verify fastest pages were calculated correctly
        $this->assertCount(3, $result['fastestPages']);
        $this->assertEquals('https://example.com/fast', $result['fastestPages'][0]['url']);
        $this->assertEquals(50, $result['fastestPages'][0]['responseTime']);
        
        // Verify largest pages were calculated correctly
        $this->assertCount(3, $result['largestPages']);
        $this->assertEquals('https://example.com/slow', $result['largestPages'][0]['url']);
        $this->assertEquals(102400, $result['largestPages'][0]['contentLength']);
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
            'results' => [],
            'tables' => [
                'performance' => [
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
        $this->assertEquals(0, $result['summary']['crawledUrls']);
        $this->assertEquals(0, $result['summary']['averageResponseTime']);
        $this->assertEquals(0, $result['summary']['totalContentSize']);
        $this->assertEmpty($result['slowestPages']);
        $this->assertEmpty($result['fastestPages']);
        $this->assertEmpty($result['largestPages']);
    }
    
    /**
     * Test the calculation of average response time and total content size
     */
    public function testPerformanceCalculations(): void
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
                    'url' => 'https://example.com/page1',
                    'type' => 1,
                    'status' => '200',
                    'time' => 100,
                    'contentLength' => 10000
                ],
                [
                    'url' => 'https://example.com/page2',
                    'type' => 1,
                    'status' => '200',
                    'time' => 300,
                    'contentLength' => 20000
                ]
            ],
            'tables' => [
                'performance' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com/page1',
                            'responseTime' => 100,
                            'contentLength' => 10000,
                            'contentType' => 'text/html'
                        ],
                        [
                            'url' => 'https://example.com/page2',
                            'responseTime' => 300,
                            'contentLength' => 20000,
                            'contentType' => 'text/html'
                        ]
                    ]
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
        $this->assertEquals(2, $result['summary']['crawledUrls']);
        $this->assertEquals(200, $result['summary']['averageResponseTime']); // (100 + 300) / 2
        $this->assertEquals(30000, $result['summary']['totalContentSize']); // 10000 + 20000
    }
} 