# Model Context Protocol (MCP) Implementation for SiteOne Crawler

This is a custom implementation of the Model Context Protocol (MCP) for the SiteOne Crawler tool, allowing AI applications and agents to leverage the crawler's capabilities through a standardized interface.

## Features

- Implements the Model Context Protocol (JSON-RPC 2.0) for AI agent compatibility
- Supports both stdio and HTTP/SSE transports for flexible integration
- Exposes SiteOne Crawler's powerful features as MCP tools
- Compatible with PHP 8.1 and uses strict typing throughout
- No third-party MCP SDKs used (custom implementation)

## Getting Started

### Prerequisites

- PHP 8.1 or higher
- SiteOne Crawler installed (the `crawler` or `crawler.bat` executable must be available)
- For HTTP/SSE transport: Swoole PHP extension version 6.0 or higher

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/siteone-crawler.git
   cd siteone-crawler
   ```

2. Ensure the crawler executable is available in your path or in the project directory.

3. Make the MCP script executable (on Linux/macOS):
   ```
   chmod +x mcp
   ```

### Usage

#### Using the stdio transport (for local process communication)

Unix-like systems (Linux, macOS):
```bash
./mcp
```

Windows:
```cmd
mcp.bat
```

This will start the MCP server using the stdio transport, allowing an AI agent to communicate with it through standard input/output.

#### Using the HTTP/SSE transport (for network-based communication)

Unix-like systems (Linux, macOS):
```bash
./mcp --transport=http --host=127.0.0.1 --port=7777
```

Windows:
```cmd
mcp.bat --transport=http --host=127.0.0.1 --port=7777
```

This will start the MCP server using the HTTP/SSE transport, allowing an AI agent to connect to it over the network.

## Available MCP Tools

The following MCP tools are implemented:

1. **siteone/analyzeWebsite** - Performs a general crawl and analysis of a website starting from a given URL, returning summary statistics and key issues.
   - Parameters: 
     - `url` (string, required): The URL to analyze
     - `depth` (integer, optional, default 1): Crawl depth

2. **siteone/getSeoMetadata** - Analyzes a specific URL or crawls a site to gather SEO-related metadata (titles, descriptions, OpenGraph tags, headings) and identifies potential issues.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `crawl` (boolean, optional, default false): Whether to crawl the entire site from the URL

3. **siteone/findBrokenLinks** - Specifically crawls a website starting from a URL to identify and report broken internal and external links.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `depth` (integer, optional, default 1): Crawl depth

4. **siteone/getWebsitePerformance** - Analyzes website performance by crawling and identifying the slowest and fastest loading pages.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `depth` (integer, optional, default 1): Crawl depth

5. **siteone/checkSecurityHeaders** - Checks for the presence and configuration of important security-related HTTP headers for a given URL or site.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `crawl` (boolean, optional, default false): Whether to crawl the entire site from the URL

6. **siteone/generateMarkdown** - Uses the crawler's website-to-markdown conversion feature to export a website or specific page(s) into Markdown format.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `outputDir` (string, required): The directory where the Markdown files will be saved

7. **siteone/generateSitemap** - Leverages the crawler's sitemap generation feature to create an XML sitemap for the website based on the crawled pages.
   - Parameters:
     - `url` (string, required): The URL to analyze
     - `outputFile` (string, required): The path where the sitemap file will be saved

## MCP Protocol Implementation

This implementation follows the [Model Context Protocol](https://modelcontextprotocol.io/) specification and uses JSON-RPC 2.0 for communication. It supports the following MCP features:

- `initialize` - Initializes the MCP server and returns its capabilities
- `shutdown` - Shuts down the MCP server
- `tools/execute` - Executes an MCP tool with the specified parameters

## Development

### Project Structure

```
/
├── mcp                    # Executable script for Linux/macOS
├── mcp.bat                # Executable script for Windows
├── src/
│   ├── mcp-server.php     # Main entry point
│   ├── autoload.php       # Simple autoloader
│   ├── Mcp/
│   │   ├── McpServer.php  # Core MCP server class
│   │   ├── JsonRpcHandler.php  # JSON-RPC 2.0 handler
│   │   ├── ToolRegistry.php  # Tool registry
│   │   ├── CrawlerExecutor.php  # Crawler CLI executor
│   │   ├── Transport/
│   │   │   ├── TransportInterface.php  # Transport interface
│   │   │   ├── StdioTransport.php  # stdio transport implementation
│   │   │   └── HttpSseTransport.php  # HTTP/SSE transport implementation
│   │   └── Tool/
│   │       ├── ToolHandlerInterface.php  # Tool handler interface
│   │       ├── AnalyzeWebsiteHandler.php  # Analyze website tool
│   │       └── ... (other tool handlers)
└── tests/
    ├── Unit/  # Unit tests
    └── Integration/  # Integration tests
```

### Running Tests

To run the tests, you need PHPUnit installed. Then, execute:

```bash
phpunit tests
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgements

- This implementation follows the [Model Context Protocol](https://modelcontextprotocol.io/) specification.
- It is designed to work with the [SiteOne Crawler](https://crawler.siteone.io/) tool.
- The HTTP/SSE transport is implemented using the [Swoole](https://www.swoole.com/) PHP extension. 