<?php
/**
 * MCP Server Entry Point Script
 * 
 * This is the main entry point for the MCP Server implementation of SiteOne Crawler.
 * It initializes the appropriate transport mechanism based on the provided arguments
 * and starts the MCP server.
 */
declare(strict_types=1);

// Include the autoloader
require_once __DIR__ . '/autoload.php';

use SiteOne\Mcp\McpServer;
use SiteOne\Mcp\Transport\StdioTransport;
use SiteOne\Mcp\Transport\HttpSseTransport;
use SiteOne\Mcp\CrawlerExecutor;
use SiteOne\Mcp\Tool\AnalyzeWebsiteHandler;
use SiteOne\Mcp\Tool\GetSeoMetadataHandler;
use SiteOne\Mcp\Tool\FindBrokenLinksHandler;
use SiteOne\Mcp\Tool\GetWebsitePerformanceHandler;
use SiteOne\Mcp\Tool\CheckSecurityHeadersHandler;
use SiteOne\Mcp\Tool\GenerateMarkdownHandler;
use SiteOne\Mcp\Tool\GenerateSitemapHandler;

// Parse command-line arguments
$options = getopt('', ['transport::', 'host::', 'port::']);
$transport = $options['transport'] ?? 'stdio';
$host = $options['host'] ?? '127.0.0.1';
$port = (int)($options['port'] ?? 7777);

// Initialize the crawler executor
$executor = new CrawlerExecutor();

// Initialize the appropriate transport
if ($transport === 'http') {
    echo "Starting MCP HTTP/SSE server on {$host}:{$port}\n";
    $transportHandler = new HttpSseTransport($host, $port);
} else {
    // Default to stdio transport
    $transportHandler = new StdioTransport();
}

// Create and configure the MCP server
$server = new McpServer($transportHandler);

// Register all MCP tools
$tools = [
    new AnalyzeWebsiteHandler($executor),
    new GetSeoMetadataHandler($executor),
    new FindBrokenLinksHandler($executor),
    new GetWebsitePerformanceHandler($executor),
    new CheckSecurityHeadersHandler($executor),
    new GenerateMarkdownHandler($executor),
    new GenerateSitemapHandler($executor)
];

foreach ($tools as $tool) {
    $server->registerTool($tool);
}

// Start the server
if ($transport === 'http') {
    // For HTTP transport, we need to call the transport's start method
    $transportHandler->start();
} else {
    // For stdio transport, we use the server's start method
    $server->start();
} 