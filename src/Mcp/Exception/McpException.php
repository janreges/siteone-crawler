<?php
/**
 * MCP Exception Class
 * 
 * Base exception class for MCP-related errors.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Exception;

/**
 * Base exception class for MCP-related errors
 */
class McpException extends \RuntimeException
{
    /**
     * JSON-RPC error codes
     */
    public const PARSE_ERROR = -32700;
    public const INVALID_REQUEST = -32600;
    public const METHOD_NOT_FOUND = -32601;
    public const INVALID_PARAMS = -32602;
    public const INTERNAL_ERROR = -32603;
    
    /**
     * Custom MCP error codes
     */
    public const SERVER_NOT_INITIALIZED = -32002;
    public const TOOL_NOT_FOUND = -32001;
    public const TOOL_EXECUTION_ERROR = -32000;
    public const VALIDATION_ERROR = -32010;
    public const RESOURCE_NOT_FOUND = -32020;
    public const TRANSPORT_ERROR = -32030;
    
    /**
     * Error details
     */
    private array $details;
    
    /**
     * Constructor
     *
     * @param string $message Error message
     * @param int $code Error code (use class constants)
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     */
    public function __construct(
        string $message,
        int $code = self::INTERNAL_ERROR,
        array $details = [],
        ?\Throwable $previous = null
    ) {
        parent::__construct($message, $code, $previous);
        $this->details = $details;
    }
    
    /**
     * Get error details
     *
     * @return array Error details
     */
    public function getDetails(): array
    {
        return $this->details;
    }
    
    /**
     * Create a parse error exception
     * 
     * @param string $message Error message
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Parse error exception
     */
    public static function parseError(
        string $message = 'Parse error',
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self($message, self::PARSE_ERROR, $details, $previous);
    }
    
    /**
     * Create an invalid request exception
     * 
     * @param string $message Error message
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Invalid request exception
     */
    public static function invalidRequest(
        string $message = 'Invalid request',
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self($message, self::INVALID_REQUEST, $details, $previous);
    }
    
    /**
     * Create a method not found exception
     * 
     * @param string $method Method name
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Method not found exception
     */
    public static function methodNotFound(
        string $method,
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self(
            "Method '{$method}' not found",
            self::METHOD_NOT_FOUND,
            $details,
            $previous
        );
    }
    
    /**
     * Create an invalid params exception
     * 
     * @param string $message Error message
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Invalid params exception
     */
    public static function invalidParams(
        string $message = 'Invalid params',
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self($message, self::INVALID_PARAMS, $details, $previous);
    }
    
    /**
     * Create a server not initialized exception
     * 
     * @param string $message Error message
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Server not initialized exception
     */
    public static function serverNotInitialized(
        string $message = 'Server not initialized',
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self($message, self::SERVER_NOT_INITIALIZED, $details, $previous);
    }
    
    /**
     * Create a tool not found exception
     * 
     * @param string $toolName Tool name
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Tool not found exception
     */
    public static function toolNotFound(
        string $toolName,
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self(
            "Tool '{$toolName}' not found",
            self::TOOL_NOT_FOUND,
            $details,
            $previous
        );
    }
    
    /**
     * Create a tool execution error exception
     * 
     * @param string $toolName Tool name
     * @param string $message Error message
     * @param array $details Additional error details
     * @param \Throwable|null $previous Previous exception
     * @return self Tool execution error exception
     */
    public static function toolExecutionError(
        string $toolName,
        string $message,
        array $details = [],
        ?\Throwable $previous = null
    ): self {
        return new self(
            "Error executing tool '{$toolName}': {$message}",
            self::TOOL_EXECUTION_ERROR,
            $details,
            $previous
        );
    }
    
    /**
     * Create a validation error exception
     * 
     * @param array $validationErrors Validation errors
     * @param string $message Error message
     * @param \Throwable|null $previous Previous exception
     * @return self Validation error exception
     */
    public static function validationError(
        array $validationErrors,
        string $message = 'Validation failed',
        ?\Throwable $previous = null
    ): self {
        return new self(
            $message,
            self::VALIDATION_ERROR,
            ['validationErrors' => $validationErrors],
            $previous
        );
    }
} 