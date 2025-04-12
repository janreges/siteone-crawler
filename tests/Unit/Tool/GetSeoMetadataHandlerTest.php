<?php
/**
 * GetSeoMetadataHandler Test
 * 
 * Unit test for the GetSeoMetadataHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\GetSeoMetadataHandler;

class GetSeoMetadataHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var GetSeoMetadataHandler
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
        $this->handler = new GetSeoMetadataHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/getSeoMetadata', $this->handler->getName());
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
            'analyze-seo' => true
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
        $this->assertArrayHasKey('pages', $result);
        $this->assertArrayHasKey('issues', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('pagesWithTitle', $result['summary']);
        $this->assertArrayHasKey('pagesWithDescription', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
        
        // Verify pages were processed correctly
        $this->assertCount(2, $result['pages']);
        $this->assertEquals('https://example.com', $result['pages'][0]['url']);
        $this->assertEquals('Example Domain', $result['pages'][0]['title']);
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
            'analyze-seo' => true
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
        $this->assertArrayHasKey('pages', $result);
    }
    
    /**
     * Test handling of SEO issues detection
     */
    public function testSeoIssuesDetection(): void
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
                    'title' => 'Example Domain',
                    'metaDescription' => 'This is a sample website',
                    'h1Count' => 0,
                    'metaTags' => [
                        'og:title' => 'Different Title'
                    ]
                ],
                [
                    'url' => 'https://example.com/page1',
                    'type' => 1,
                    'status' => '200',
                    'title' => '', // Missing title
                    'metaDescription' => '',  // Missing description
                    'h1Count' => 2  // Multiple H1 tags
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
        $this->assertArrayHasKey('issues', $result);
        $this->assertGreaterThan(0, count($result['issues']));
        
        // Find issues for page without title and description
        $issuesForPage1 = [];
        foreach ($result['issues'] as $issue) {
            if ($issue['url'] === 'https://example.com/page1') {
                $issuesForPage1[] = $issue;
            }
        }
        
        $this->assertGreaterThan(0, count($issuesForPage1));
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
            'tables' => []
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $this->assertEquals(0, $result['summary']['crawledUrls']);
        $this->assertEquals(0, $result['summary']['pagesWithTitle']);
        $this->assertEquals(0, $result['summary']['pagesWithDescription']);
        $this->assertEmpty($result['pages']);
        $this->assertEmpty($result['issues']);
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
                [
                    'url' => 'https://example.com',
                    'type' => 1,
                    'status' => '200',
                    'title' => 'Example Domain',
                    'metaDescription' => 'This is a sample website',
                    'h1Count' => 1,
                    'h1Text' => 'Example Domain',
                    'metaTags' => [
                        'og:title' => 'Example Domain',
                        'og:description' => 'This is a sample website',
                        'og:image' => 'https://example.com/image.jpg'
                    ]
                ],
                [
                    'url' => 'https://example.com/page1',
                    'type' => 1,
                    'status' => '200',
                    'title' => 'Page 1',
                    'metaDescription' => 'This is page 1',
                    'h1Count' => 1,
                    'h1Text' => 'Page 1',
                    'metaTags' => []
                ]
            ],
            'tables' => []
        ];
    }
} 