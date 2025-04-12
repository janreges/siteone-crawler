<?php
/**
 * PSR-3 Logger Implementation for MCP
 * 
 * This class provides a logging system for the MCP server and tools.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

/**
 * PSR-3 compatible logger implementation for MCP
 */
class Logger
{
    /**
     * Log levels defined by PSR-3
     */
    public const EMERGENCY = 'emergency';
    public const ALERT     = 'alert';
    public const CRITICAL  = 'critical';
    public const ERROR     = 'error';
    public const WARNING   = 'warning';
    public const NOTICE    = 'notice';
    public const INFO      = 'info';
    public const DEBUG     = 'debug';
    
    /**
     * Log directory
     */
    private string $logDir;
    
    /**
     * Log file name
     */
    private string $logFileName;
    
    /**
     * Log file path (computed from $logDir and $logFileName)
     */
    private string $logFilePath;
    
    /**
     * Minimum log level to write to file
     */
    private string $minFileLevel;
    
    /**
     * Minimum log level to write to console
     */
    private string $minConsoleLevel;
    
    /**
     * Whether to write logs to console
     */
    private bool $consoleOutput;
    
    /**
     * Mapping of log levels to numeric values for comparison
     */
    private array $levelMap = [
        self::EMERGENCY => 0,
        self::ALERT     => 1,
        self::CRITICAL  => 2,
        self::ERROR     => 3,
        self::WARNING   => 4,
        self::NOTICE    => 5,
        self::INFO      => 6,
        self::DEBUG     => 7
    ];
    
    /**
     * Color mapping for console output
     */
    private array $colorMap = [
        self::EMERGENCY => "\033[41;37m", // White on red background
        self::ALERT     => "\033[41;37m", // White on red background
        self::CRITICAL  => "\033[41;37m", // White on red background
        self::ERROR     => "\033[31m",    // Red
        self::WARNING   => "\033[33m",    // Yellow
        self::NOTICE    => "\033[36m",    // Cyan
        self::INFO      => "\033[32m",    // Green
        self::DEBUG     => "\033[90m"     // Gray
    ];
    
    /**
     * Reset ANSI color code
     */
    private const COLOR_RESET = "\033[0m";
    
    /**
     * Constructor
     * 
     * @param string $logDir Directory where log files will be stored
     * @param string $logFileName Base name for the log file (without extension)
     * @param string $minFileLevel Minimum log level to write to file
     * @param string $minConsoleLevel Minimum log level to write to console
     * @param bool $consoleOutput Whether to write logs to console
     */
    public function __construct(
        string $logDir = 'log',
        string $logFileName = 'mcp',
        string $minFileLevel = self::INFO,
        string $minConsoleLevel = self::INFO,
        bool $consoleOutput = true
    ) {
        $this->logDir = rtrim($logDir, '/\\');
        $this->logFileName = $logFileName;
        $this->minFileLevel = $minFileLevel;
        $this->minConsoleLevel = $minConsoleLevel;
        $this->consoleOutput = $consoleOutput;
        
        // Create log directory if it doesn't exist
        if (!file_exists($this->logDir)) {
            mkdir($this->logDir, 0777, true);
        }
        
        // Set log file path with date prefix
        $this->logFilePath = sprintf(
            '%s/%s-%s.log',
            $this->logDir,
            $this->logFileName,
            date('Y-m-d')
        );
    }
    
    /**
     * Log an emergency message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function emergency(string $message, array $context = []): void
    {
        $this->log(self::EMERGENCY, $message, $context);
    }
    
    /**
     * Log an alert message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function alert(string $message, array $context = []): void
    {
        $this->log(self::ALERT, $message, $context);
    }
    
    /**
     * Log a critical message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function critical(string $message, array $context = []): void
    {
        $this->log(self::CRITICAL, $message, $context);
    }
    
    /**
     * Log an error message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function error(string $message, array $context = []): void
    {
        $this->log(self::ERROR, $message, $context);
    }
    
    /**
     * Log a warning message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function warning(string $message, array $context = []): void
    {
        $this->log(self::WARNING, $message, $context);
    }
    
    /**
     * Log a notice message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function notice(string $message, array $context = []): void
    {
        $this->log(self::NOTICE, $message, $context);
    }
    
    /**
     * Log an info message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function info(string $message, array $context = []): void
    {
        $this->log(self::INFO, $message, $context);
    }
    
    /**
     * Log a debug message
     * 
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function debug(string $message, array $context = []): void
    {
        $this->log(self::DEBUG, $message, $context);
    }
    
    /**
     * Log a message with any level
     * 
     * @param string $level The log level
     * @param string $message The log message
     * @param array $context Additional context data
     * @return void
     */
    public function log(string $level, string $message, array $context = []): void
    {
        // Validate log level
        if (!isset($this->levelMap[$level])) {
            throw new \InvalidArgumentException(sprintf(
                'Invalid log level "%s". Valid levels are: %s',
                $level,
                implode(', ', array_keys($this->levelMap))
            ));
        }
        
        // Interpolate message with context variables if any
        $interpolatedMessage = $this->interpolate($message, $context);
        
        // Get formatted log line
        $logLine = $this->formatLogLine($level, $interpolatedMessage, $context);
        
        // Write to file if level is sufficient
        if ($this->shouldLogToFile($level)) {
            $this->writeToFile($logLine);
        }
        
        // Write to console if enabled and level is sufficient
        if ($this->consoleOutput && $this->shouldLogToConsole($level)) {
            $this->writeToConsole($level, $logLine);
        }
    }
    
    /**
     * Check if a given level should be logged to file
     * 
     * @param string $level The log level to check
     * @return bool True if the level should be logged
     */
    private function shouldLogToFile(string $level): bool
    {
        return $this->levelMap[$level] <= $this->levelMap[$this->minFileLevel];
    }
    
    /**
     * Check if a given level should be logged to console
     * 
     * @param string $level The log level to check
     * @return bool True if the level should be logged
     */
    private function shouldLogToConsole(string $level): bool
    {
        return $this->levelMap[$level] <= $this->levelMap[$this->minConsoleLevel];
    }
    
    /**
     * Format a log line
     * 
     * @param string $level The log level
     * @param string $message The log message
     * @param array $context Additional context data
     * @return string The formatted log line
     */
    private function formatLogLine(string $level, string $message, array $context): string
    {
        $dateTime = date('Y-m-d H:i:s');
        $logLine = sprintf('[%s] [%s] %s', $dateTime, strtoupper($level), $message);
        
        // Include exception traces
        if (isset($context['exception']) && $context['exception'] instanceof \Throwable) {
            $exception = $context['exception'];
            $logLine .= sprintf(
                ' Exception: %s: %s in %s on line %d%sStack trace:%s%s',
                get_class($exception),
                $exception->getMessage(),
                $exception->getFile(),
                $exception->getLine(),
                PHP_EOL,
                PHP_EOL,
                $exception->getTraceAsString()
            );
        }
        
        return $logLine;
    }
    
    /**
     * Interpolate context values into the message placeholders
     * 
     * @param string $message Message with context placeholders
     * @param array $context Context data to replace placeholders
     * @return string The interpolated message
     */
    private function interpolate(string $message, array $context): string
    {
        // Build a replacement array with braces around the context keys
        $replace = [];
        foreach ($context as $key => $val) {
            // Skip the 'exception' key as it's used for special handling
            if ($key === 'exception' && $val instanceof \Throwable) {
                continue;
            }
            
            // Convert objects to string, if possible
            if (is_object($val) && method_exists($val, '__toString')) {
                $val = (string)$val;
            } else if (is_object($val) || is_array($val)) {
                $val = json_encode($val, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) ?: '[unserializable]';
            }
            
            $replace['{' . $key . '}'] = $val;
        }
        
        // Replace placeholders in the message
        return strtr($message, $replace);
    }
    
    /**
     * Write a log line to the file
     * 
     * @param string $logLine The formatted log line
     * @return void
     */
    private function writeToFile(string $logLine): void
    {
        file_put_contents(
            $this->logFilePath,
            $logLine . PHP_EOL,
            FILE_APPEND
        );
    }
    
    /**
     * Write a log line to the console
     * 
     * @param string $level The log level
     * @param string $logLine The formatted log line
     * @return void
     */
    private function writeToConsole(string $level, string $logLine): void
    {
        $color = $this->colorMap[$level] ?? '';
        echo $color . $logLine . self::COLOR_RESET . PHP_EOL;
    }
} 