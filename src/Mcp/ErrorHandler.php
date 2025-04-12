<?php
/**
 * Error Handler for MCP
 * 
 * This class provides error handling functionality for the MCP server and tools.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

/**
 * Error handler implementation for MCP
 */
class ErrorHandler
{
    /**
     * Logger instance
     */
    private Logger $logger;
    
    /**
     * Whether the error handler is registered
     */
    private bool $isRegistered = false;
    
    /**
     * Error types map for debugging
     */
    private array $errorTypeMap = [
        E_ERROR => 'E_ERROR',
        E_WARNING => 'E_WARNING',
        E_PARSE => 'E_PARSE',
        E_NOTICE => 'E_NOTICE',
        E_CORE_ERROR => 'E_CORE_ERROR',
        E_CORE_WARNING => 'E_CORE_WARNING',
        E_COMPILE_ERROR => 'E_COMPILE_ERROR',
        E_COMPILE_WARNING => 'E_COMPILE_WARNING',
        E_USER_ERROR => 'E_USER_ERROR',
        E_USER_WARNING => 'E_USER_WARNING',
        E_USER_NOTICE => 'E_USER_NOTICE',
        E_STRICT => 'E_STRICT',
        E_RECOVERABLE_ERROR => 'E_RECOVERABLE_ERROR',
        E_DEPRECATED => 'E_DEPRECATED',
        E_USER_DEPRECATED => 'E_USER_DEPRECATED',
    ];
    
    /**
     * Constructor
     * 
     * @param Logger $logger The logger instance
     */
    public function __construct(Logger $logger)
    {
        $this->logger = $logger;
    }
    
    /**
     * Register error handlers
     * 
     * @return void
     */
    public function register(): void
    {
        if ($this->isRegistered) {
            return;
        }
        
        // Set error handler
        set_error_handler([$this, 'handleError']);
        
        // Set exception handler
        set_exception_handler([$this, 'handleException']);
        
        // Register shutdown function
        register_shutdown_function([$this, 'handleShutdown']);
        
        $this->isRegistered = true;
        
        $this->logger->info('Error handler registered');
    }
    
    /**
     * Unregister error handlers
     * 
     * @return void
     */
    public function unregister(): void
    {
        if (!$this->isRegistered) {
            return;
        }
        
        // Restore previous error handler
        restore_error_handler();
        
        // Restore previous exception handler
        restore_exception_handler();
        
        // We can't unregister the shutdown function, but we can set a flag
        $this->isRegistered = false;
        
        $this->logger->info('Error handler unregistered');
    }
    
    /**
     * Handle PHP errors
     * 
     * @param int $errno The error number
     * @param string $errstr The error message
     * @param string $errfile The file where the error occurred
     * @param int $errline The line where the error occurred
     * @param array $errcontext The error context (variables in scope)
     * @return bool True if the error was handled, false otherwise
     * @throws \ErrorException If the error is fatal or user error
     */
    public function handleError(
        int $errno,
        string $errstr,
        string $errfile,
        int $errline,
        array $errcontext = []
    ): bool {
        // Don't handle errors if error reporting is disabled
        if (!(error_reporting() & $errno)) {
            return false;
        }
        
        $errorType = $this->errorTypeMap[$errno] ?? 'Unknown';
        
        // Format error message
        $message = sprintf(
            '%s: %s in %s on line %d',
            $errorType,
            $errstr,
            $errfile,
            $errline
        );
        
        // Log the error with appropriate level
        switch ($errno) {
            case E_ERROR:
            case E_CORE_ERROR:
            case E_COMPILE_ERROR:
            case E_USER_ERROR:
                $this->logger->error($message);
                // Convert fatal errors to exceptions
                throw new \ErrorException($errstr, 0, $errno, $errfile, $errline);
                
            case E_WARNING:
            case E_CORE_WARNING:
            case E_COMPILE_WARNING:
            case E_USER_WARNING:
                $this->logger->warning($message);
                break;
                
            case E_NOTICE:
            case E_USER_NOTICE:
                $this->logger->notice($message);
                break;
                
            case E_STRICT:
            case E_RECOVERABLE_ERROR:
            case E_DEPRECATED:
            case E_USER_DEPRECATED:
                $this->logger->info($message);
                break;
                
            default:
                $this->logger->warning($message);
                break;
        }
        
        // Return true to indicate that we've handled the error
        return true;
    }
    
    /**
     * Handle uncaught exceptions
     * 
     * @param \Throwable $exception The uncaught exception
     * @return void
     */
    public function handleException(\Throwable $exception): void
    {
        $message = sprintf(
            'Uncaught exception: %s: %s in %s on line %d',
            get_class($exception),
            $exception->getMessage(),
            $exception->getFile(),
            $exception->getLine()
        );
        
        $this->logger->error($message, ['exception' => $exception]);
    }
    
    /**
     * Handle script shutdown, capturing fatal errors
     * 
     * @return void
     */
    public function handleShutdown(): void
    {
        if (!$this->isRegistered) {
            return;
        }
        
        // Get last error
        $error = error_get_last();
        
        // Check if the error is fatal
        if ($error !== null && in_array($error['type'], [
            E_ERROR,
            E_PARSE,
            E_CORE_ERROR,
            E_CORE_WARNING,
            E_COMPILE_ERROR,
            E_COMPILE_WARNING
        ])) {
            $errorType = $this->errorTypeMap[$error['type']] ?? 'Unknown';
            
            $message = sprintf(
                'Fatal %s: %s in %s on line %d',
                $errorType,
                $error['message'],
                $error['file'],
                $error['line']
            );
            
            $this->logger->critical($message);
        }
    }
    
    /**
     * Create a standardized error response for tool handlers
     * 
     * @param string $message The error message
     * @param \Throwable|null $exception The exception, if any
     * @param array $details Additional error details
     * @return array The error response
     */
    public function createErrorResponse(
        string $message,
        ?\Throwable $exception = null,
        array $details = []
    ): array {
        $errorData = [
            'success' => false,
            'error' => [
                'message' => $message,
                'details' => $details
            ]
        ];
        
        if ($exception !== null) {
            // Log the exception
            $this->logger->error($message, ['exception' => $exception]);
            
            // Add exception details for internal use (will be sanitized in production)
            $errorData['error']['exception'] = [
                'type' => get_class($exception),
                'message' => $exception->getMessage(),
                'file' => $exception->getFile(),
                'line' => $exception->getLine()
            ];
        } else {
            // Log the error without an exception
            $this->logger->error($message);
        }
        
        return $errorData;
    }
    
    /**
     * Create a standardized validation error response for tool handlers
     * 
     * @param array $validationErrors The validation errors
     * @return array The validation error response
     */
    public function createValidationErrorResponse(array $validationErrors): array
    {
        $message = 'Validation failed: ' . implode(', ', array_keys($validationErrors));
        
        $this->logger->warning($message, ['validationErrors' => $validationErrors]);
        
        return [
            'success' => false,
            'error' => [
                'message' => 'Validation failed',
                'details' => [
                    'validationErrors' => $validationErrors
                ]
            ]
        ];
    }
} 