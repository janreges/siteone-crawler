<?php
/**
 * JSON-RPC Handler for MCP
 * 
 * This class handles JSON-RPC 2.0 protocol messages for the MCP server.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

class JsonRpcHandler
{
    /**
     * JSON-RPC version constant
     */
    private const JSON_RPC_VERSION = '2.0';
    
    /**
     * Parse a JSON-RPC request from a JSON string
     * 
     * @param string $json The JSON string to parse
     * @return array The parsed request object
     * @throws \RuntimeException If the request is invalid
     */
    public function parseRequest(string $json): array
    {
        $request = json_decode($json, true);
        
        if (json_last_error() !== JSON_ERROR_NONE) {
            throw $this->createParseError();
        }
        
        $this->validateRequest($request);
        
        return $request;
    }
    
    /**
     * Validate a JSON-RPC request object
     * 
     * @param mixed $request The request object to validate
     * @throws \RuntimeException If the request is invalid
     */
    private function validateRequest($request): void
    {
        if (!is_array($request)) {
            throw $this->createInvalidRequestError("Request must be an object");
        }
        
        if (!isset($request['jsonrpc']) || $request['jsonrpc'] !== self::JSON_RPC_VERSION) {
            throw $this->createInvalidRequestError("Invalid or missing 'jsonrpc' property");
        }
        
        if (!isset($request['method']) || !is_string($request['method']) || empty($request['method'])) {
            throw $this->createInvalidRequestError("Invalid or missing 'method' property");
        }
        
        if (isset($request['params']) && !is_array($request['params'])) {
            throw $this->createInvalidRequestError("'params' must be an object or array");
        }
        
        if (!isset($request['id'])) {
            throw $this->createInvalidRequestError("Missing 'id' property");
        }
        
        if (!is_string($request['id']) && !is_int($request['id']) && !is_null($request['id'])) {
            throw $this->createInvalidRequestError("'id' must be a string, number, or null");
        }
    }
    
    /**
     * Create a successful JSON-RPC response
     * 
     * @param mixed $id The request ID
     * @param mixed $result The result data
     * @return string The JSON-RPC response as a JSON string
     */
    public function createResponse($id, $result): string
    {
        $response = [
            'jsonrpc' => self::JSON_RPC_VERSION,
            'result' => $result,
            'id' => $id
        ];
        
        return json_encode($response);
    }
    
    /**
     * Create a JSON-RPC error response
     * 
     * @param mixed $id The request ID
     * @param int $code The error code
     * @param string $message The error message
     * @param mixed $data Additional error data (optional)
     * @return string The JSON-RPC error response as a JSON string
     */
    public function createErrorResponse($id, int $code, string $message, $data = null): string
    {
        $error = [
            'code' => $code,
            'message' => $message
        ];
        
        if ($data !== null) {
            $error['data'] = $data;
        }
        
        $response = [
            'jsonrpc' => self::JSON_RPC_VERSION,
            'error' => $error,
            'id' => $id
        ];
        
        return json_encode($response);
    }
    
    /**
     * Create a parse error exception
     * 
     * @return \RuntimeException The parse error exception
     */
    public function createParseError(): \RuntimeException
    {
        return new \RuntimeException('Parse error: ' . json_last_error_msg(), -32700);
    }
    
    /**
     * Create an invalid request error exception
     * 
     * @param string $message The error message
     * @return \RuntimeException The invalid request error exception
     */
    public function createInvalidRequestError(string $message): \RuntimeException
    {
        return new \RuntimeException('Invalid Request: ' . $message, -32600);
    }
    
    /**
     * Create a method not found error exception
     * 
     * @param string $method The method name
     * @return \RuntimeException The method not found error exception
     */
    public function createMethodNotFoundError(string $method): \RuntimeException
    {
        return new \RuntimeException("Method not found: {$method}", -32601);
    }
    
    /**
     * Create an invalid params error exception
     * 
     * @param string $message The error message
     * @return \RuntimeException The invalid params error exception
     */
    public function createInvalidParamsError(string $message): \RuntimeException
    {
        return new \RuntimeException("Invalid params: {$message}", -32602);
    }
    
    /**
     * Create an internal error exception
     * 
     * @param string $message The error message
     * @return \RuntimeException The internal error exception
     */
    public function createInternalError(string $message): \RuntimeException
    {
        return new \RuntimeException("Internal error: {$message}", -32603);
    }
} 