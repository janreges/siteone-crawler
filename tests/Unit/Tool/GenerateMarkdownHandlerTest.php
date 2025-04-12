<?php
/**
 * GenerateMarkdownHandler Test
 * 
 * Unit test for the GenerateMarkdownHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\GenerateMarkdownHandler;

class GenerateMarkdownHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var GenerateMarkdownHandler
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
        $this->handler = new GenerateMarkdownHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/generateMarkdown', $this->handler->getName());
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
        $this->assertContains('outputDir', $schema['required']);
        
        // Check property definitions
        $this->assertArrayHasKey('url', $schema['properties']);
        $this->assertArrayHasKey('outputDir', $schema['properties']);
        $this->assertArrayHasKey('depth', $schema['properties']);
        $this->assertArrayHasKey('includeImages', $schema['properties']);
        $this->assertArrayHasKey('includeLinks', $schema['properties']);
    }
    
    /**
     * Test that execute throws an exception when the url parameter is missing
     */
    public function testExecuteWithMissingUrl(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('Missing required parameter: url');
        
        $this->handler->execute(['outputDir' => 'output']);
    }
    
    /**
     * Test that execute throws an exception when the outputDir parameter is missing
     */
    public function testExecuteWithMissingOutputDir(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('Missing required parameter: outputDir');
        
        $this->handler->execute(['url' => 'https://example.com']);
    }
    
    /**
     * Test that execute calls the crawler executor with the correct parameters
     */
    public function testExecuteWithValidParameters(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'outputDir' => 'output',
            'depth' => 2,
            'includeImages' => true,
            'includeLinks' => true,
            'frontMatter' => true,
            'githubFlavor' => true
        ];
        
        $expectedCrawlerParams = [
            'url' => 'https://example.com',
            'max-depth' => 2,
            'export-md' => true,
            'md-dir' => 'output',
            'md-include-images' => true,
            'md-include-links' => true,
            'md-front-matter' => true,
            'md-github-flavor' => true
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/image.jpg', 'type' => 2, 'status' => '200']
            ],
            'tables' => [
                'markdown' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com',
                            'outputFile' => 'output/index.md',
                            'title' => 'Example Domain',
                            'contentLength' => 1024
                        ],
                        [
                            'url' => 'https://example.com/page1',
                            'outputFile' => 'output/page1.md',
                            'title' => 'Page 1',
                            'contentLength' => 2048
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
        
        // Act - Simulate markdown files being created
        $tempDir = sys_get_temp_dir() . '/output';
        if (!is_dir($tempDir)) {
            mkdir($tempDir, 0777, true);
        }
        file_put_contents($tempDir . '/index.md', '# Example Domain');
        file_put_contents($tempDir . '/page1.md', '# Page 1');
        
        // Execute the handler
        $result = $this->handler->execute($parameters);
        
        // Clean up
        if (file_exists($tempDir . '/index.md')) {
            unlink($tempDir . '/index.md');
        }
        if (file_exists($tempDir . '/page1.md')) {
            unlink($tempDir . '/page1.md');
        }
        if (is_dir($tempDir)) {
            rmdir($tempDir);
        }
        
        // Assert - Check the result structure
        $this->assertIsArray($result);
        $this->assertArrayHasKey('success', $result);
        $this->assertArrayHasKey('summary', $result);
        $this->assertArrayHasKey('pages', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('convertedPages', $result['summary']);
        $this->assertArrayHasKey('totalContentSize', $result['summary']);
        $this->assertArrayHasKey('outputDirectory', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
        
        // Check pages array
        $this->assertCount(2, $result['pages']);
        $this->assertEquals('https://example.com', $result['pages'][0]['url']);
        $this->assertEquals('output/index.md', $result['pages'][0]['file']);
    }
    
    /**
     * Test handling of empty result tables
     */
    public function testExecuteWithEmptyResults(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'outputDir' => 'output'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [],
            'tables' => [
                'markdown' => [
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
        $this->assertEquals(0, $result['summary']['convertedPages']);
        $this->assertEmpty($result['pages']);
    }
    
    /**
     * Test the calculation of total content size
     */
    public function testContentSizeCalculation(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'outputDir' => 'output'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [],
            'tables' => [
                'markdown' => [
                    'rows' => [
                        [
                            'url' => 'https://example.com',
                            'outputFile' => 'output/index.md',
                            'title' => 'Example Domain',
                            'contentLength' => 1000
                        ],
                        [
                            'url' => 'https://example.com/page1',
                            'outputFile' => 'output/page1.md',
                            'title' => 'Page 1',
                            'contentLength' => 2000
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
        $this->assertEquals(2, $result['summary']['convertedPages']);
        $this->assertEquals(3000, $result['summary']['totalContentSize']); // 1000 + 2000
    }
} 