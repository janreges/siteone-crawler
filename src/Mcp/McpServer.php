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
     * Constructor
     * 
     * @param TransportInterface $transport The transport to use
     */
    public function __construct(TransportInterface $transport)
    {
        $this->transport = $transport;
        $this->jsonRpcHandler = new JsonRpcHandler();
        $this->toolRegistry = new ToolRegistry();
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
        try {
            // Main server loop
            while (true) {
                // Read a message from the transport
                $message = $this->transport->readMessage();
                
                // If message is null, the transport has been closed
                if ($message === null) {
                    break;
                }
                
                // Process the message and send the response
                $response = $this->handleMessage($message);
                
                if ($response !== null) {
                    $this->transport->writeMessage($response);
                }
            }
        } catch (\Exception $e) {
            // Handle unexpected exceptions
            $errorResponse = $this->jsonRpcHandler->createErrorResponse(
                null,
                -32603,
                "Internal error: " . $e->getMessage()
            );
            $this->transport->writeMessage($errorResponse);
        } finally {
            // Ensure the transport is closed
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
        $this->toolRegistry->registerTool($toolHandler);
    }
    
    /**
     * Handle an incoming JSON-RPC message
     * 
     * @param string $message The JSON-RPC message
     * @return string|null The JSON-RPC response, or null if no response should be sent
     */
    private function handleMessage(string $message): ?string
    {
        try {
            // Parse the message as a JSON-RPC request
            $request = $this->jsonRpcHandler->parseRequest($message);
            
            // Get the method and params
            $method = $request['method'];
            $params = $request['params'] ?? [];
            $id = $request['id'];
            
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
                    throw $this->jsonRpcHandler->createMethodNotFoundError($method);
            }
        } catch (\RuntimeException $e) {
            // Handle expected exceptions
            return $this->jsonRpcHandler->createErrorResponse(
                $request['id'] ?? null,
                $e->getCode(),
                $e->getMessage()
            );
        } catch (\Exception $e) {
            // Handle unexpected exceptions
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
            throw new \RuntimeException('Server not initialized', -32002);
        }
        
        // Check for required parameters
        if (!isset($params['name']) || !is_string($params['name'])) {
            throw $this->jsonRpcHandler->createInvalidParamsError("Missing or invalid 'name' parameter");
        }
        
        $toolName = $params['name'];
        $toolParams = $params['params'] ?? [];
        
        // Check if the tool exists
        if (!$this->toolRegistry->hasTool($toolName)) {
            throw new \RuntimeException("Tool '{$toolName}' not found", -32601);
        }
        
        // Get the tool and execute it
        $tool = $this->toolRegistry->getTool($toolName);
        $result = $tool->execute($toolParams);
        
        // Return the result
        return $this->jsonRpcHandler->createResponse($id, [
            'name' => $toolName,
            'result' => $result
        ]);
    }
} 