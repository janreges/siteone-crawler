<?php
/**
 * AnalyzeWebsiteHandler Test
 * 
 * Unit test for the AnalyzeWebsiteHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\AnalyzeWebsiteHandler;

class AnalyzeWebsiteHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var AnalyzeWebsiteHandler
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
        $this->handler = new AnalyzeWebsiteHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/analyzeWebsite', $this->handler->getName());
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
            'analyze-broken-links' => true,
            'analyze-headers' => true,
            'analyze-seo' => true,
            'analyze-performance' => true,
            'record-response-time' => true
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
        $this->assertArrayHasKey('contentTypes', $result);
        $this->assertArrayHasKey('statusCodes', $result);
        $this->assertArrayHasKey('topPerformingPages', $result);
        $this->assertArrayHasKey('slowestPages', $result);
        $this->assertArrayHasKey('brokenLinks', $result);
        $this->assertArrayHasKey('domains', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('htmlCount', $result['summary']);
        $this->assertArrayHasKey('imageCount', $result['summary']);
        $this->assertArrayHasKey('otherCount', $result['summary']);
        $this->assertArrayHasKey('totalErrors', $result['summary']);
        $this->assertArrayHasKey('totalRedirects', $result['summary']);
        $this->assertArrayHasKey('averageResponseTime', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
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
                '404' => ['rows' => []],
                'redirects' => ['rows' => []],
                'performance' => ['rows' => []]
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
        $this->assertEquals(0, $result['summary']['htmlCount']);
        $this->assertEquals(0, $result['summary']['totalErrors']);
        $this->assertEmpty($result['contentTypes']);
        $this->assertEmpty($result['statusCodes']);
        $this->assertEmpty($result['topPerformingPages']);
        $this->assertEmpty($result['slowestPages']);
        $this->assertEmpty($result['brokenLinks']);
    }
    
    /**
     * Test the content type detection
     */
    public function testContentTypeDetection(): void
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
                // Type 1 = HTML
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '200'],
                // Type 2 = Image
                ['url' => 'https://example.com/image1.jpg', 'type' => 2, 'status' => '200'],
                ['url' => 'https://example.com/image2.png', 'type' => 2, 'status' => '200'],
                // Type 3 = CSS
                ['url' => 'https://example.com/style.css', 'type' => 3, 'status' => '200'],
                // Type 4 = JavaScript
                ['url' => 'https://example.com/script.js', 'type' => 4, 'status' => '200']
            ],
            'tables' => [
                '404' => ['rows' => []],
                'redirects' => ['rows' => []],
                'performance' => ['rows' => []]
            ]
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertEquals(6, $result['summary']['crawledUrls']);
        $this->assertEquals(2, $result['summary']['htmlCount']);
        $this->assertEquals(2, $result['summary']['imageCount']);
        $this->assertEquals(2, $result['summary']['otherCount']);
        
        // Check content types
        $this->assertArrayHasKey('html', $result['contentTypes']);
        $this->assertArrayHasKey('image', $result['contentTypes']);
        $this->assertArrayHasKey('css', $result['contentTypes']);
        $this->assertArrayHasKey('javascript', $result['contentTypes']);
        $this->assertEquals(2, $result['contentTypes']['html']);
        $this->assertEquals(2, $result['contentTypes']['image']);
        $this->assertEquals(1, $result['contentTypes']['css']);
        $this->assertEquals(1, $result['contentTypes']['javascript']);
    }
    
    /**
     * Test the status code detection
     */
    public function testStatusCodeDetection(): void
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
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '404'],
                ['url' => 'https://example.com/page2', 'type' => 1, 'status' => '301'],
                ['url' => 'https://example.com/page3', 'type' => 1, 'status' => '500']
            ],
            'tables' => [
                '404' => ['rows' => [
                    ['url' => 'https://example.com/page1', 'statusCode' => 404]
                ]],
                'redirects' => ['rows' => [
                    ['url' => 'https://example.com/page2', 'statusCode' => 301]
                ]],
                'performance' => ['rows' => []]
            ]
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertEquals(4, $result['summary']['crawledUrls']);
        $this->assertEquals(2, $result['summary']['totalErrors']); // 404 + 500
        $this->assertEquals(1, $result['summary']['totalRedirects']);
        
        // Check status codes
        $this->assertArrayHasKey('200', $result['statusCodes']);
        $this->assertArrayHasKey('301', $result['statusCodes']);
        $this->assertArrayHasKey('404', $result['statusCodes']);
        $this->assertArrayHasKey('500', $result['statusCodes']);
        $this->assertEquals(1, $result['statusCodes']['200']);
        $this->assertEquals(1, $result['statusCodes']['301']);
        $this->assertEquals(1, $result['statusCodes']['404']);
        $this->assertEquals(1, $result['statusCodes']['500']);
    }
    
    /**
     * Create a mock crawler result
     */
    private function createMockCrawlerResult(): array
    {
        return [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                // HTML pages
                [
                    'url' => 'https://example.com',
                    'type' => 1,
                    'status' => '200',
                    'time' => 100,
                    'contentLength' => 10240
                ],
                [
                    'url' => 'https://example.com/page1',
                    'type' => 1,
                    'status' => '200',
                    'time' => 300,
                    'contentLength' => 20480
                ],
                // Images
                [
                    'url' => 'https://example.com/image.jpg',
                    'type' => 2,
                    'status' => '200',
                    'time' => 50,
                    'contentLength' => 51200
                ],
                // Error page
                [
                    'url' => 'https://example.com/notfound',
                    'type' => 1,
                    'status' => '404',
                    'time' => 20,
                    'contentLength' => 5120
                ]
            ],
            'tables' => [
                '404' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com/notfound',
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
                'performance' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com',
                            'responseTime' => 100,
                            'contentLength' => 10240,
                            'contentType' => 'text/html'
                        ],
                        [
                            'url' => 'https://example.com/page1',
                            'responseTime' => 300,
                            'contentLength' => 20480,
                            'contentType' => 'text/html'
                        ],
                        [
                            'url' => 'https://example.com/image.jpg',
                            'responseTime' => 50,
                            'contentLength' => 51200,
                            'contentType' => 'image/jpeg'
                        ]
                    ]
                ]
            ]
        ];
    }
} 