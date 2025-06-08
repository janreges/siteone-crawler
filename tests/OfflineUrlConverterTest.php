<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Export\Utils\OfflineUrlConverter;
use Crawler\ParsedUrl;
use PHPUnit\Framework\TestCase;

class OfflineUrlConverterTest extends TestCase
{
    /**
     * @dataProvider getOfflineBaseUrlDepthProvider
     */
    public function testGetOfflineBaseUrlDepthProvider(string $fullUrl, int $expected)
    {
        $result = OfflineUrlConverter::getOfflineBaseUrlDepth(ParsedUrl::parse($fullUrl));
        $this->assertEquals($expected, $result);
    }

    /**
     * @return array[]
     */
    public static function getOfflineBaseUrlDepthProvider(): array
    {
        return [
            // absolute path
            ['/', 0], # because /index.html
            ['/foo', 0], # because /foo.html
            ['/foo/', 1], # because /foo/index.html
            ['/foo/bar', 1], # because /foo/bar.html
            ['/foo/bar/', 2], # because /foo/bar/index.html
            ['/?param=1', 0], # because /index.queryMd5Hash.html
            ['/foo?param=1', 0], # because /foo.queryMd5Hash.html
            ['/foo/?param=1', 1], # because /foo/index.queryMd5Hash.html
            ['/foo/bar?param=1', 1], # because /foo/bar.queryMd5Hash.html
            ['/foo/bar/?param=1', 2], # because /foo/bar/index.queryMd5Hash.html
        ];
    }

    /**
     * @dataProvider urlConversionProvider
     */
    public function testUrlConversion(string $baseUrl, string $targetUrl, string $expected)
    {
        $initialUrl = ParsedUrl::parse('https://example.com/');
        $baseUrlParsed = ParsedUrl::parse($baseUrl);
        $targetUrlParsed = ParsedUrl::parse($targetUrl, $baseUrlParsed);
        
        $converter = new OfflineUrlConverter(
            $initialUrl,
            $baseUrlParsed,
            $targetUrlParsed,
            [__CLASS__, 'mockCallbackReturnFalse'],
            [__CLASS__, 'mockCallbackReturnFalse'],
            null
        );
        
        $result = $converter->convertUrlToRelative(false);
        
        // Debug output
        if ($result !== $expected) {
            echo "\nDebug for test case: baseUrl=$baseUrl, targetUrl=$targetUrl\n";
            echo "Target URL path after parsing: " . $targetUrlParsed->path . "\n";
            echo "Relative target URL path: " . $converter->getRelativeTargetUrl()->path . "\n";
            echo "Expected: $expected\n";
            echo "Actual: $result\n";
        }
        
        $this->assertEquals($expected, $result, "Failed converting $targetUrl from base $baseUrl");
    }
    
    public static function mockCallbackReturnFalse(): bool
    {
        return false;
    }

    /**
     * @return array[]
     */
    public static function urlConversionProvider(): array
    {
        return [
            // Test cases for relative URL conversion from subdirectories
            ['https://example.com/page/', '/style.css', '../style.css'],
            ['https://example.com/page/', '/images/logo.png', '../images/logo.png'],
            ['https://example.com/dir/page/', '/style.css', '../../style.css'],
            ['https://example.com/dir/page/', '/assets/style.css', '../../assets/style.css'],
            
            // Test cases for same directory
            ['https://example.com/', '/style.css', 'style.css'],
            ['https://example.com/', '/images/logo.png', 'images/logo.png'],
            
            // Test cases for relative paths - these now follow the existing behavior
            ['https://example.com/page/', 'style.css', '../page/style.css'],
            ['https://example.com/page/', './style.css', '../page/style.css'],
            ['https://example.com/page/', '../style.css', '../style.css'],
        ];
    }

    /**
     * Test sanitizeFilePath with UTF-8 characters
     * @dataProvider utf8FilePathProvider
     */
    public function testSanitizeFilePathWithUtf8($input, $expected)
    {
        $initialUrl = ParsedUrl::parse('https://example.com/');
        $baseUrl = ParsedUrl::parse('https://example.com/');
        $targetUrl = ParsedUrl::parse('https://example.com/test');
        
        $converter = new OfflineUrlConverter(
            $initialUrl,
            $baseUrl,
            $targetUrl,
            [__CLASS__, 'mockCallbackReturnFalse'],
            [__CLASS__, 'mockCallbackReturnFalse'],
            null
        );
        
        $result = OfflineUrlConverter::sanitizeFilePath($input, false);
        $this->assertEquals($expected, $result, "Failed for input: $input");
    }

    /**
     * @return array[]
     */
    public static function utf8FilePathProvider(): array
    {
        return [
            // Czech characters
            ['české-výrobky', 'české-výrobky'],
            ['příliš-žluťoučký', 'příliš-žluťoučký'],
            ['žlutý-kůň', 'žlutý-kůň'],
            ['o-nás', 'o-nás'],
            ['úžasné-věci', 'úžasné-věci'],
            
            // German characters
            ['über-uns', 'über-uns'],
            ['bücher', 'bücher'],
            ['größe-ändern', 'größe-ändern'],
            ['süße-träume', 'süße-träume'],
            
            // Chinese characters
            ['电子产品', '电子产品'],
            ['联系我们', '联系我们'],
            ['新闻中心', '新闻中心'],
            ['技术支持', '技术支持'],
            ['产品列表', '产品列表'],
            ['中文图片', '中文图片'],
            
            // Japanese characters
            ['日本語', '日本語'],
            ['ページ', 'ページ'],
            ['こんにちは', 'こんにちは'],
            
            // Korean characters
            ['한국어', '한국어'],
            ['페이지', '페이지'],
            
            // Cyrillic characters
            ['категория', 'категория'],
            ['товары', 'товары'],
            ['список', 'список'],
            
            // Greek characters
            ['ελληνικά', 'ελληνικά'],
            ['σελίδα', 'σελίδα'],
            
            // Arabic characters
            ['العربية', 'العربية'],
            ['صفحة', 'صفحة'],
            
            // Mixed UTF-8 and ASCII
            ['page-české', 'page-české'],
            ['test_电子_page', 'test_电子_page'],
            ['über-page-123', 'über-page-123'],
            
            // URL encoded input (should be decoded)
            ['%C4%8Desk%C3%A9-v%C3%BDrobky', 'české-výrobky'],
            ['%C3%BCber-uns', 'über-uns'],
            ['%E7%94%B5%E5%AD%90%E4%BA%A7%E5%93%81', '电子产品'],
            ['%E8%81%94%E7%B3%BB%E6%88%91%E4%BB%AC', '联系我们'],
            
            // Characters that should be replaced
            ['file:with:colons', 'file_with_colons'],
            ['file*with*asterisks', 'file_with_asterisks'],
            ['file?with?questions', 'file_with_questions'],
            ['file"with"quotes', 'file_with_quotes'],
            ['file<with>brackets', 'file_with_brackets'],
            ['file|with|pipes', 'file_with_pipes'],
            ['file\\with\\backslashes', 'file_with_backslashes'],
            
            // Mixed problematic and UTF-8 characters
            ['české:výrobky', 'české_výrobky'],
            ['über*uns', 'über_uns'],
            ['电子?产品', '电子_产品'],
            ['файл:с:двоеточиями', 'файл_с_двоеточиями'],
            
            // Special cases
            ['', ''],
            ['.', '.'],
            ['..', '..'],
            ['...', '...'],
            
            // Path traversal attempts (should be sanitized)
            ['../../../etc/passwd', '../../../etc/passwd'],
            ['..\\..\\windows\\system32', '.._.._windows_system32'],
            
            // Long UTF-8 filenames (should be preserved)
            ['velmi-dlouhý-název-souboru-s-českými-znaky-žluťoučký-kůň-úpěl-ďábelské-ódy', 'velmi-dlouhý-název-souboru-s-českými-znaky-žluťoučký-kůň-úpěl-ďábelské-ódy'],
            ['非常长的中文文件名包含很多汉字和其他字符', '非常长的中文文件名包含很多汉字和其他字符'],
        ];
    }

    /**
     * Test URL conversion with UTF-8 URLs
     * @dataProvider utf8UrlConversionProvider
     */
    public function testUtf8UrlConversion(string $baseUrl, string $targetUrl, string $expected)
    {
        $initialUrl = ParsedUrl::parse('https://example.com/');
        $baseUrlParsed = ParsedUrl::parse($baseUrl);
        $targetUrlParsed = ParsedUrl::parse($targetUrl, $baseUrlParsed);
        
        $converter = new OfflineUrlConverter(
            $initialUrl,
            $baseUrlParsed,
            $targetUrlParsed,
            [__CLASS__, 'mockCallbackReturnFalse'],
            [__CLASS__, 'mockCallbackReturnFalse'],
            null
        );
        
        $result = $converter->convertUrlToRelative(false);
        $this->assertEquals($expected, $result, "Failed converting UTF-8 URL: $targetUrl from base $baseUrl");
    }

    /**
     * @return array[]
     */
    public static function utf8UrlConversionProvider(): array
    {
        return [
            // Czech URLs
            ['https://example.com/', 'https://example.com/české-výrobky', 'české-výrobky.html'],
            ['https://example.com/', 'https://example.com/products/české-výrobky', 'products/české-výrobky.html'],
            ['https://example.com/page/', 'https://example.com/české-výrobky', '../české-výrobky.html'],
            
            // German URLs
            ['https://example.com/', 'https://example.com/über-uns', 'über-uns.html'],
            ['https://example.com/', 'https://example.com/products/bücher', 'products/bücher.html'],
            
            // Chinese URLs
            ['https://example.com/', 'https://example.com/电子产品', '电子产品.html'],
            ['https://example.com/', 'https://example.com/联系我们', '联系我们.html'],
            ['https://example.com/dir/', 'https://example.com/电子产品', '../电子产品.html'],
            
            // Mixed paths
            ['https://example.com/', 'https://example.com/products/电子产品', 'products/电子产品.html'],
            ['https://example.com/české/', 'https://example.com/české/výrobky', '../české/výrobky.html'],
            
            // With index.html
            ['https://example.com/', 'https://example.com/české-výrobky/', 'české-výrobky/index.html'],
            ['https://example.com/', 'https://example.com/电子产品/', '电子产品/index.html'],
            
            // Relative paths with UTF-8
            ['https://example.com/dir/', '/české-výrobky', '../české-výrobky.html'],
            ['https://example.com/dir/', '/products/电子产品', '../products/电子产品.html'],
        ];
    }
}