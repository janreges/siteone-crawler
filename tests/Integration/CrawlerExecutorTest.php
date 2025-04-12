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
        $this->executor = new CrawlerExecutor();
        
        // Skip if the crawler executable doesn't exist
        if (!file_exists('./crawler') && !file_exists('crawler.bat')) {
            $this->markTestSkipped('Crawler executable not found');
        }
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
        // This test might be skipped if the crawler handles non-existent URLs gracefully
        // without returning a non-zero exit code
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