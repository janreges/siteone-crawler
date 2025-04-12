<?php
/**
 * MCP Server Entry Point Script
 * 
 * This is the main entry point for the MCP Server implementation of SiteOne Crawler.
 * It initializes the appropriate transport mechanism based on the provided arguments
 * and starts the MCP server.
 */
declare(strict_types=1);

require_once 'mcp-bootstrap.php';

if (!defined('BASE_DIR')) {
    // If bootstrap wasn't loaded, define BASE_DIR directly
    if ($baseDir) {
        define('BASE_DIR', $baseDir);
        define('SRC_DIR', BASE_DIR . '/src');
        
        // Set up a basic autoloader
        spl_autoload_register(function ($class) {
            $classFile = SRC_DIR . '/' . str_replace('\\', '/', $class) . '.php';
            if (file_exists($classFile)) {
                require_once $classFile;
                return true;
            }
            return false;
        });
    } else {
        die("Cannot determine the project root directory. Bootstrap file not found.");
    }
}

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
use SiteOne\Mcp\Logger;
use SiteOne\Mcp\ErrorHandler;

// Parse command-line arguments
$options = getopt('', ['transport::', 'host::', 'port::', 'log-level::', 'log-dir::', 'debug::']);
$transport = $options['transport'] ?? 'stdio';
$host = $options['host'] ?? '127.0.0.1';
$port = (int)($options['port'] ?? 7777);
$logLevel = $options['log-level'] ?? Logger::INFO;
$logDir = $options['log-dir'] ?? BASE_DIR . '/log';
$debug = isset($options['debug']);

// If debug is enabled, set it in $_SERVER for the autoloader
if ($debug) {
    $_SERVER['MCP_DEBUG'] = true;
}

// Set up the logger with appropriate log levels
$consoleLogLevel = $debug ? Logger::DEBUG : Logger::INFO;
$fileLogLevel = $logLevel;

// Initialize logger
$logger = new Logger(
    $logDir,
    'mcp-server',
    $fileLogLevel,
    $consoleLogLevel,
    true // Console output enabled
);

// Log startup information
$logger->info('MCP Server starting up', [
    'transport' => $transport,
    'host' => $host,
    'port' => $port,
    'logLevel' => $logLevel,
    'debug' => $debug,
    'baseDir' => BASE_DIR
]);

// Initialize error handler
$errorHandler = new ErrorHandler($logger);
$errorHandler->register();

try {
    // Initialize the crawler executor with the logger
    $executor = new CrawlerExecutor(null, null, $logger);
    
    // Initialize the appropriate transport
    if ($transport === 'http') {
        $logger->info("Starting MCP HTTP/SSE server on {$host}:{$port}");
        $transportHandler = new HttpSseTransport($host, $port);
    } else {
        // Default to stdio transport
        $logger->info("Starting MCP stdio transport");
        $transportHandler = new StdioTransport();
    }
    
    // Create and configure the MCP server
    $server = new McpServer($transportHandler, $logger);
    
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
        $logger->debug("Registering tool: " . $tool->getName());
        $server->registerTool($tool);
    }
    
    // Start the server
    $logger->info("MCP Server initialization complete, starting server...");
    
    if ($transport === 'http') {
        // For HTTP transport, we need to call the transport's start method
        $transportHandler->start();
    } else {
        // For stdio transport, we use the server's start method
        $server->start();
    }
} catch (\Throwable $e) {
    // Log the exception
    $logger->critical("Fatal error during MCP Server startup", ['exception' => $e]);
    
    // Display error message
    echo "Fatal error: " . $e->getMessage() . PHP_EOL;
    
    // Exit with error code
    exit(1);
} finally {
    // Unregister error handler on shutdown
    $errorHandler->unregister();
    
    // Log server shutdown
    $logger->info("MCP Server shutting down");
} 