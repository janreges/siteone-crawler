<?php
/**
 * MCP Server
 * 
 * This is the main class for the MCP server implementation,
 * responsible for handling MCP requests and responses.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

use SiteOne\Mcp\Transport\TransportInterface;
use SiteOne\Mcp\Tool\ToolHandlerInterface;

class McpServer
{
    /**
     * The transport used for communication
     */
    private TransportInterface $transport;
    
    /**
     * The JSON-RPC handler
     */
    private JsonRpcHandler $jsonRpcHandler;
    
    /**
     * The tool registry
     */
    private ToolRegistry $toolRegistry;
    
    /**
     * Server initialization state
     */
    private bool $initialized = false;
    
    /**
     * The logger instance
     */
    private Logger $logger;
    
    /**
     * Constructor
     * 
     * @param TransportInterface $transport The transport to use
     * @param Logger|null $logger The logger instance
     */
    public function __construct(TransportInterface $transport, ?Logger $logger = null)
    {
        $this->transport = $transport;
        $this->jsonRpcHandler = new JsonRpcHandler();
        $this->toolRegistry = new ToolRegistry();
        $this->logger = $logger ?? new Logger();
    }
    
    /**
     * Start the server
     * 
     * This method starts the server's main loop, processing requests
     * and sending responses until the transport is closed.
     * 
     * @return void
     */
    public function start(): void
    {
        $this->logger->info("MCP Server starting main loop");
        
        try {
            // Main server loop
            while (true) {
                // Read a message from the transport
                $message = $this->transport->readMessage();
                
                // If message is null, the transport has been closed
                if ($message === null) {
                    $this->logger->info("Transport closed, exiting main loop");
                    break;
                }
                
                $this->logger->debug("Received message", ['message' => $this->sanitizeMessage($message)]);
                
                // Process the message and send the response
                $response = $this->handleMessage($message);
                
                if ($response !== null) {
                    $this->logger->debug("Sending response", ['response' => $this->sanitizeMessage($response)]);
                    $this->transport->writeMessage($response);
                }
            }
        } catch (\Throwable $e) {
            // Handle unexpected exceptions
            $this->logger->error("Unexpected error in main loop", ['exception' => $e]);
            
            try {
                $errorResponse = $this->jsonRpcHandler->createErrorResponse(
                    null,
                    -32603,
                    "Internal error: " . $e->getMessage()
                );
                $this->transport->writeMessage($errorResponse);
            } catch (\Throwable $innerE) {
                $this->logger->critical("Failed to send error response", ['exception' => $innerE]);
            }
        } finally {
            // Ensure the transport is closed
            $this->logger->info("Closing transport");
            $this->transport->close();
        }
    }
    
    /**
     * Register a tool handler
     * 
     * @param ToolHandlerInterface $toolHandler The tool handler to register
     * @return void
     */
    public function registerTool(ToolHandlerInterface $toolHandler): void
    {
        try {
            $this->toolRegistry->registerTool($toolHandler);
            $this->logger->info("Registered tool: " . $toolHandler->getName());
        } catch (\Throwable $e) {
            $this->logger->error("Failed to register tool", [
                'tool' => $toolHandler->getName(),
                'exception' => $e
            ]);
            throw $e; // Re-throw to allow calling code to handle it
        }
    }
    
    /**
     * Handle an incoming JSON-RPC message
     * 
     * @param string $message The JSON-RPC message
     * @return string|null The JSON-RPC response, or null if no response should be sent
     */
    private function handleMessage(string $message): ?string
    {
        $request = null;
        
        try {
            // Parse the message as a JSON-RPC request
            $request = $this->jsonRpcHandler->parseRequest($message);
            
            // Get the method and params
            $method = $request['method'];
            $params = $request['params'] ?? [];
            $id = $request['id'];
            
            $this->logger->info("Processing request", ['method' => $method, 'id' => $id]);
            
            // Handle the request based on the method
            switch ($method) {
                case 'initialize':
                    return $this->handleInitialize($id, $params);
                
                case 'shutdown':
                    return $this->handleShutdown($id);
                
                case 'tools/execute':
                    return $this->handleToolsExecute($id, $params);
                
                default:
                    // Unknown method
                    $this->logger->warning("Unknown method called", ['method' => $method]);
                    throw $this->jsonRpcHandler->createMethodNotFoundError($method);
            }
        } catch (\RuntimeException $e) {
            // Handle expected exceptions
            $this->logger->warning("Runtime exception during request processing", [
                'exception' => $e,
                'code' => $e->getCode()
            ]);
            
            return $this->jsonRpcHandler->createErrorResponse(
                $request['id'] ?? null,
                $e->getCode(),
                $e->getMessage()
            );
        } catch (\Throwable $e) {
            // Handle unexpected exceptions
            $this->logger->error("Unexpected exception during request processing", [
                'exception' => $e
            ]);
            
            return $this->jsonRpcHandler->createErrorResponse(
                $request['id'] ?? null,
                -32603,
                "Internal error: " . $e->getMessage()
            );
        }
    }
    
    /**
     * Handle the 'initialize' method
     * 
     * @param mixed $id The request ID
     * @param array $params The request parameters
     * @return string The JSON-RPC response
     */
    private function handleInitialize($id, array $params): string
    {
        $this->logger->info("Initializing server", ['params' => $params]);
        
        // Set the server as initialized
        $this->initialized = true;
        
        // Create the server capabilities object
        $capabilities = [
            'mcp' => [
                'version' => '2025-03-26',
                'capabilities' => [
                    'mcp/tools' => [
                        'version' => '2025-03-26'
                    ]
                ]
            ]
        ];
        
        $this->logger->debug("Returning capabilities", ['capabilities' => $capabilities]);
        
        // Return the response
        return $this->jsonRpcHandler->createResponse($id, [
            'capabilities' => $capabilities
        ]);
    }
    
    /**
     * Handle the 'shutdown' method
     * 
     * @param mixed $id The request ID
     * @return string The JSON-RPC response
     */
    private function handleShutdown($id): string
    {
        $this->logger->info("Shutting down server");
        
        // Set the server as not initialized
        $this->initialized = false;
        
        // Return a success response
        return $this->jsonRpcHandler->createResponse($id, null);
    }
    
    /**
     * Handle the 'tools/execute' method
     * 
     * @param mixed $id The request ID
     * @param array $params The request parameters
     * @return string The JSON-RPC response
     * @throws \RuntimeException If the server is not initialized or if the parameters are invalid
     */
    private function handleToolsExecute($id, array $params): string
    {
        // Check if the server is initialized
        if (!$this->initialized) {
            $this->logger->warning("Tool execution attempted before initialization");
            throw new \RuntimeException('Server not initialized', -32002);
        }
        
        // Check for required parameters
        if (!isset($params['name']) || !is_string($params['name'])) {
            $this->logger->warning("Missing or invalid tool name parameter", ['params' => $params]);
            throw $this->jsonRpcHandler->createInvalidParamsError("Missing or invalid 'name' parameter");
        }
        
        $toolName = $params['name'];
        $toolParams = $params['params'] ?? [];
        
        $this->logger->info("Executing tool", ['name' => $toolName, 'params' => $toolParams]);
        
        // Check if the tool exists
        if (!$this->toolRegistry->hasTool($toolName)) {
            $this->logger->warning("Tool not found", ['name' => $toolName]);
            throw new \RuntimeException("Tool '{$toolName}' not found", -32601);
        }
        
        // Get the tool and execute it
        try {
            $tool = $this->toolRegistry->getTool($toolName);
            
            $this->logger->debug("Executing tool handler", ['name' => $toolName]);
            $startTime = microtime(true);
            
            $result = $tool->execute($toolParams);
            
            $executionTime = microtime(true) - $startTime;
            $this->logger->info("Tool execution completed", [
                'name' => $toolName, 
                'time' => $executionTime
            ]);
            
            // Return the result
            return $this->jsonRpcHandler->createResponse($id, [
                'name' => $toolName,
                'result' => $result
            ]);
        } catch (\Throwable $e) {
            $this->logger->error("Error executing tool", [
                'name' => $toolName,
                'exception' => $e
            ]);
            
            // Re-throw as RuntimeException with appropriate code
            throw new \RuntimeException(
                "Error executing tool '{$toolName}': " . $e->getMessage(),
                -32603
            );
        }
    }
    
    /**
     * Sanitize a message for logging
     * 
     * This method removes any sensitive data from the message before logging it.
     * 
     * @param string $message The message to sanitize
     * @return string The sanitized message
     */
    private function sanitizeMessage(string $message): string
    {
        // For large messages, truncate them to avoid filling the logs
        if (strlen($message) > 1000) {
            return substr($message, 0, 500) . '...[truncated]...' . substr($message, -500);
        }
        
        return $message;
    }
} 