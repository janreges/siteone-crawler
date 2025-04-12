<?php
/**
 * GenerateSitemapHandler Test
 * 
 * Unit test for the GenerateSitemapHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit\Tool;

use PHPUnit\Framework\TestCase;
use PHPUnit\Framework\MockObject\MockObject;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\GenerateSitemapHandler;

class GenerateSitemapHandlerTest extends TestCase
{
    /**
     * @var CrawlerExecutor|MockObject
     */
    private $crawlerExecutor;
    
    /**
     * @var GenerateSitemapHandler
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
        $this->handler = new GenerateSitemapHandler($this->crawlerExecutor);
    }
    
    /**
     * Test that getName returns the correct value
     */
    public function testGetName(): void
    {
        $this->assertEquals('siteone/generateSitemap', $this->handler->getName());
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
        $this->assertContains('outputFile', $schema['required']);
        
        // Check property definitions
        $this->assertArrayHasKey('url', $schema['properties']);
        $this->assertArrayHasKey('outputFile', $schema['properties']);
        $this->assertArrayHasKey('depth', $schema['properties']);
        $this->assertArrayHasKey('includeImages', $schema['properties']);
        $this->assertArrayHasKey('priority', $schema['properties']);
        $this->assertArrayHasKey('changefreq', $schema['properties']);
    }
    
    /**
     * Test that execute throws an exception when the url parameter is missing
     */
    public function testExecuteWithMissingUrl(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('Missing required parameter: url');
        
        $this->handler->execute(['outputFile' => 'sitemap.xml']);
    }
    
    /**
     * Test that execute throws an exception when the outputFile parameter is missing
     */
    public function testExecuteWithMissingOutputFile(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('Missing required parameter: outputFile');
        
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
            'outputFile' => 'sitemap.xml',
            'depth' => 2,
            'includeImages' => true,
            'priority' => 0.8,
            'changefreq' => 'daily'
        ];
        
        $expectedCrawlerParams = [
            'url' => 'https://example.com',
            'max-depth' => 2,
            'export-sitemap' => true,
            'sitemap-file' => 'sitemap.xml',
            'sitemap-include-images' => true,
            'sitemap-priority' => 0.8,
            'sitemap-changefreq' => 'daily'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/image.jpg', 'type' => 2, 'status' => '200']
            ]
        ];
        
        // Set up the mock to expect the execute method to be called with the expected parameters
        $this->crawlerExecutor
            ->expects($this->once())
            ->method('execute')
            ->with($this->equalTo($expectedCrawlerParams))
            ->willReturn($mockResult);
        
        // Act - Simulate successful sitemap creation by creating a temporary file
        $tempFile = sys_get_temp_dir() . '/sitemap.xml';
        file_put_contents($tempFile, '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>');
        
        // Execute the handler
        $result = $this->handler->execute($parameters);
        
        // Clean up
        if (file_exists($tempFile)) {
            unlink($tempFile);
        }
        
        // Assert - Check the result structure
        $this->assertIsArray($result);
        $this->assertArrayHasKey('success', $result);
        $this->assertArrayHasKey('summary', $result);
        $this->assertArrayHasKey('domains', $result);
        
        // Check summary fields
        $this->assertArrayHasKey('crawledUrls', $result['summary']);
        $this->assertArrayHasKey('htmlUrls', $result['summary']);
        $this->assertArrayHasKey('sitemapFile', $result['summary']);
        $this->assertArrayHasKey('crawlDate', $result['summary']);
    }
    
    /**
     * Test the extractDomains method through execute
     */
    public function testExtractDomains(): void
    {
        // Arrange
        $parameters = [
            'url' => 'https://example.com',
            'outputFile' => 'sitemap.xml'
        ];
        
        $mockResult = [
            'crawler' => [
                'executedAt' => '2023-01-01T12:00:00Z'
            ],
            'results' => [
                ['url' => 'https://example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.com/page1', 'type' => 1, 'status' => '200'],
                ['url' => 'https://subdomain.example.com', 'type' => 1, 'status' => '200'],
                ['url' => 'https://example.org', 'type' => 1, 'status' => '200']
            ]
        ];
        
        // Set up the mock
        $this->crawlerExecutor
            ->method('execute')
            ->willReturn($mockResult);
        
        // Act
        $result = $this->handler->execute($parameters);
        
        // Assert
        $domains = $result['domains'];
        $this->assertCount(3, $domains);
        
        // Check domain extraction
        $exampleCom = null;
        foreach ($domains as $domain) {
            if ($domain['name'] === 'example.com') {
                $exampleCom = $domain;
                break;
            }
        }
        
        $this->assertNotNull($exampleCom);
        $this->assertEquals(2, $exampleCom['totalUrls']);
        $this->assertEquals(2, $exampleCom['htmlUrls']);
    }
} 