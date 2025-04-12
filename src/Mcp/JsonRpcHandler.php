<?php
/**
 * JSON-RPC Handler for MCP
 * 
 * This class handles JSON-RPC 2.0 protocol messages for the MCP server.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

use SiteOne\Mcp\Exception\McpException;

class JsonRpcHandler
{
    /**
     * JSON-RPC version constant
     */
    private const JSON_RPC_VERSION = '2.0';
    
    /**
     * Logger instance
     */
    private ?Logger $logger;
    
    /**
     * Constructor
     * 
     * @param Logger|null $logger The logger instance
     */
    public function __construct(?Logger $logger = null)
    {
        $this->logger = $logger;
    }
    
    /**
     * Parse a JSON-RPC request from a JSON string
     * 
     * @param string $json The JSON string to parse
     * @return array The parsed request object
     * @throws McpException If the request is invalid
     */
    public function parseRequest(string $json): array
    {
        try {
            $request = json_decode($json, true);
            
            if (json_last_error() !== JSON_ERROR_NONE) {
                $error = 'Parse error: ' . json_last_error_msg();
                if ($this->logger) {
                    $this->logger->error($error, ['json' => $this->sanitizeJson($json)]);
                }
                throw McpException::parseError($error);
            }
            
            $this->validateRequest($request);
            
            return $request;
        } catch (McpException $e) {
            // Re-throw McpException instances as they are already properly formatted
            throw $e;
        } catch (\Throwable $e) {
            // Convert other exceptions to McpException
            if ($this->logger) {
                $this->logger->error('Error parsing JSON-RPC request', ['exception' => $e]);
            }
            throw McpException::parseError('Internal error during request parsing', [], $e);
        }
    }
    
    /**
     * Validate a JSON-RPC request object
     * 
     * @param mixed $request The request object to validate
     * @throws McpException If the request is invalid
     */
    private function validateRequest($request): void
    {
        $errors = [];
        
        if (!is_array($request)) {
            throw McpException::invalidRequest("Request must be an object");
        }
        
        if (!isset($request['jsonrpc']) || $request['jsonrpc'] !== self::JSON_RPC_VERSION) {
            $errors['jsonrpc'] = "Invalid or missing 'jsonrpc' property";
        }
        
        if (!isset($request['method']) || !is_string($request['method']) || empty($request['method'])) {
            $errors['method'] = "Invalid or missing 'method' property";
        }
        
        if (isset($request['params']) && !is_array($request['params'])) {
            $errors['params'] = "'params' must be an object or array";
        }
        
        if (!isset($request['id'])) {
            $errors['id'] = "Missing 'id' property";
        } elseif (!is_string($request['id']) && !is_int($request['id']) && !is_null($request['id'])) {
            $errors['id'] = "'id' must be a string, number, or null";
        }
        
        if (!empty($errors)) {
            // Throw with detailed validation errors
            throw McpException::invalidRequest(
                'Invalid JSON-RPC request',
                ['validationErrors' => $errors]
            );
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
        
        if ($this->logger) {
            $this->logger->debug('Creating JSON-RPC response', [
                'id' => $id,
                'hasResult' => $result !== null
            ]);
        }
        
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
        
        if ($this->logger) {
            $this->logger->warning('Creating JSON-RPC error response', [
                'id' => $id,
                'code' => $code,
                'message' => $message
            ]);
        }
        
        return json_encode($response);
    }
    
    /**
     * Create a JSON-RPC error response from an McpException
     * 
     * @param mixed $id The request ID
     * @param McpException $exception The exception
     * @return string The JSON-RPC error response as a JSON string
     */
    public function createErrorResponseFromException($id, McpException $exception): string
    {
        return $this->createErrorResponse(
            $id,
            $exception->getCode(),
            $exception->getMessage(),
            $exception->getDetails()
        );
    }
    
    /**
     * Create a parse error exception
     * 
     * @param string|null $message Custom error message (optional)
     * @return McpException The parse error exception
     */
    public function createParseError(?string $message = null): McpException
    {
        return McpException::parseError($message ?? 'Parse error: ' . json_last_error_msg());
    }
    
    /**
     * Create an invalid request error exception
     * 
     * @param string $message The error message
     * @return McpException The invalid request error exception
     */
    public function createInvalidRequestError(string $message): McpException
    {
        return McpException::invalidRequest('Invalid Request: ' . $message);
    }
    
    /**
     * Create a method not found error exception
     * 
     * @param string $method The method name
     * @return McpException The method not found error exception
     */
    public function createMethodNotFoundError(string $method): McpException
    {
        return McpException::methodNotFound($method);
    }
    
    /**
     * Create an invalid params error exception
     * 
     * @param string $message The error message
     * @param array $details Additional error details
     * @return McpException The invalid params error exception
     */
    public function createInvalidParamsError(string $message, array $details = []): McpException
    {
        return McpException::invalidParams('Invalid params: ' . $message, $details);
    }
    
    /**
     * Create an internal error exception
     * 
     * @param string $message The error message
     * @param \Throwable|null $previous The previous exception
     * @return McpException The internal error exception
     */
    public function createInternalError(string $message, ?\Throwable $previous = null): McpException
    {
        return new McpException('Internal error: ' . $message, McpException::INTERNAL_ERROR, [], $previous);
    }
    
    /**
     * Sanitize JSON for logging
     * 
     * @param string $json The JSON to sanitize
     * @return string The sanitized JSON
     */
    private function sanitizeJson(string $json): string
    {
        // Truncate large JSON strings for logging
        if (strlen($json) > 1000) {
            return substr($json, 0, 500) . '...[truncated]...' . substr($json, -500);
        }
        
        return $json;
    }
} 