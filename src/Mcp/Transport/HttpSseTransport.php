<?php
/**
 * HTTP/SSE Transport implementation for MCP
 * 
 * This class implements the TransportInterface using Swoole HTTP server
 * with Server-Sent Events (SSE) for communication between the MCP server and client.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Transport;

use Swoole\Http\Server;
use Swoole\Http\Request;
use Swoole\Http\Response;

class HttpSseTransport implements TransportInterface
{
    /**
     * Swoole HTTP server instance
     */
    private Server $server;
    
    /**
     * The host to bind to
     */
    private string $host;
    
    /**
     * The port to listen on
     */
    private int $port;
    
    /**
     * SSE response object for the current connection
     */
    private ?Response $sseResponse = null;
    
    /**
     * Queue of incoming messages from clients
     */
    private array $messageQueue = [];
    
    /**
     * Flag indicating if the transport has been closed
     */
    private bool $closed = false;
    
    /**
     * Constructor
     * 
     * @param string $host The host to bind to
     * @param int $port The port to listen on
     * @throws \RuntimeException If Swoole extension is not installed
     */
    public function __construct(string $host = '127.0.0.1', int $port = 7777)
    {
        if (!extension_loaded('swoole')) {
            throw new \RuntimeException('Swoole extension is required for HTTP/SSE transport');
        }
        
        $this->host = $host;
        $this->port = $port;
        
        // Create Swoole HTTP server
        $this->server = new Server($host, $port);
        
        // Configure server settings
        $this->server->set([
            'worker_num' => 1,
            'log_level' => SWOOLE_LOG_INFO,
            'daemonize' => false
        ]);
        
        // Set up request handler
        $this->server->on('request', [$this, 'handleRequest']);
    }
    
    /**
     * Handle incoming HTTP requests
     * 
     * @param Request $request The HTTP request
     * @param Response $response The HTTP response
     */
    public function handleRequest(Request $request, Response $response): void
    {
        $path = $request->server['request_uri'] ?? '/';
        
        if ($path === '/sse') {
            // SSE connection request
            $this->handleSseConnection($request, $response);
        } elseif ($path === '/message' && $request->getMethod() === 'POST') {
            // Message from client
            $this->handleMessagePost($request, $response);
        } else {
            // Invalid request
            $response->status(404);
            $response->end('Not Found');
        }
    }
    
    /**
     * Handle SSE connection request
     * 
     * @param Request $request The HTTP request
     * @param Response $response The HTTP response
     */
    private function handleSseConnection(Request $request, Response $response): void
    {
        // Set required SSE headers
        $response->header('Content-Type', 'text/event-stream');
        $response->header('Cache-Control', 'no-cache');
        $response->header('Connection', 'keep-alive');
        $response->header('X-Accel-Buffering', 'no'); // Disable Nginx buffering
        
        // Store the response for sending SSE events
        $this->sseResponse = $response;
        
        // Send a comment to keep the connection alive
        $response->write(":" . str_repeat(' ', 2048) . "\n\n");
        
        // Send a ready event
        $response->write("event: ready\ndata: {\"status\":\"ready\"}\n\n");
    }
    
    /**
     * Handle incoming message POSTs from clients
     * 
     * @param Request $request The HTTP request
     * @param Response $response The HTTP response
     */
    private function handleMessagePost(Request $request, Response $response): void
    {
        // Get message from request body
        $message = $request->rawContent();
        
        if (!empty($message)) {
            // Add message to queue for processing
            $this->messageQueue[] = $message;
            
            // Respond with success
            $response->header('Content-Type', 'application/json');
            $response->end('{"status":"ok"}');
        } else {
            // Empty message
            $response->status(400);
            $response->header('Content-Type', 'application/json');
            $response->end('{"status":"error","message":"Empty message"}');
        }
    }
    
    /**
     * {@inheritdoc}
     */
    public function readMessage(): ?string
    {
        if ($this->closed) {
            return null;
        }
        
        // Check if we have any messages in the queue
        if (!empty($this->messageQueue)) {
            return array_shift($this->messageQueue);
        }
        
        // No messages yet - sleep a bit to avoid busy waiting
        usleep(100000); // 100ms
        
        return null;
    }
    
    /**
     * {@inheritdoc}
     */
    public function writeMessage(string $message): void
    {
        if ($this->closed || $this->sseResponse === null) {
            return;
        }
        
        // Send message as SSE data event
        $this->sseResponse->write("event: message\ndata: {$message}\n\n");
    }
    
    /**
     * {@inheritdoc}
     */
    public function close(): void
    {
        if (!$this->closed) {
            $this->server->shutdown();
            $this->closed = true;
            $this->sseResponse = null;
        }
    }
    
    /**
     * Start the HTTP server to handle SSE connections
     */
    public function start(): void
    {
        $this->server->start();
    }
} 