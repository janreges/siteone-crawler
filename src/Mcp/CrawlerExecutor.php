<?php
/**
 * Crawler Executor for MCP
 * 
 * This class is responsible for executing the SiteOne Crawler CLI tool
 * and processing its output.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

class CrawlerExecutor
{
    /**
     * Path to the crawler executable
     */
    private string $crawlerPath;
    
    /**
     * Directory for temporary files
     */
    private string $tempDir;
    
    /**
     * Logger instance
     */
    private Logger $logger;
    
    /**
     * Constructor
     * 
     * @param string|null $crawlerPath Path to the crawler executable
     * @param string|null $tempDir Directory for temporary files
     * @param Logger|null $logger Logger instance
     */
    public function __construct(
        ?string $crawlerPath = null, 
        ?string $tempDir = null,
        ?Logger $logger = null
    ) {
        // Detect the appropriate crawler executable based on the platform
        if ($crawlerPath === null) {
            $this->crawlerPath = PHP_OS_FAMILY === 'Windows' ? 'crawler.bat' : './crawler';
        } else {
            $this->crawlerPath = $crawlerPath;
        }
        
        // Set temporary directory for output files
        if ($tempDir === null) {
            $this->tempDir = sys_get_temp_dir() . DIRECTORY_SEPARATOR . 'siteone-mcp';
        } else {
            $this->tempDir = rtrim($tempDir, '\\/');
        }
        
        // Set logger
        $this->logger = $logger ?? new Logger();
        
        // Ensure the temporary directory exists
        $this->ensureTempDirExists();
    }
    
    /**
     * Ensure that the temporary directory exists
     * 
     * @return void
     * @throws \RuntimeException If the directory cannot be created
     */
    private function ensureTempDirExists(): void
    {
        if (!is_dir($this->tempDir)) {
            $this->logger->info("Creating temporary directory: {$this->tempDir}");
            
            if (!mkdir($this->tempDir, 0755, true) && !is_dir($this->tempDir)) {
                $error = "Failed to create temporary directory: {$this->tempDir}";
                $this->logger->error($error);
                throw new \RuntimeException($error);
            }
        }
    }
    
    /**
     * Execute the crawler with the given parameters
     * 
     * @param array $parameters Parameters to pass to the crawler
     * @return array The parsed JSON output from the crawler
     * @throws \RuntimeException If the crawler execution fails
     */
    public function execute(array $parameters): array
    {
        $this->logger->info("Executing crawler", ['parameters' => $this->sanitizeParameters($parameters)]);
        
        // Generate a unique output file name
        $jsonOutputFile = $this->tempDir . DIRECTORY_SEPARATOR . 'mcp_output_' . uniqid() . '.json';
        $this->logger->debug("Using JSON output file: {$jsonOutputFile}");
        
        // Build the command with parameters
        $command = $this->buildCommand($parameters, $jsonOutputFile);
        $this->logger->debug("Generated command: " . $this->sanitizeCommand($command));
        
        try {
            // Measure execution time
            $startTime = microtime(true);
            
            // Execute the command
            $output = $this->executeCommand($command);
            
            $executionTime = microtime(true) - $startTime;
            $this->logger->info("Crawler execution completed", [
                'time' => $executionTime,
                'exitCode' => 0
            ]);
            
            // Check if the JSON output file was created
            if (!file_exists($jsonOutputFile)) {
                $error = "JSON output file was not created.";
                $this->logger->error($error, [
                    'stdout' => $output['stdout'],
                    'stderr' => $output['stderr']
                ]);
                throw new \RuntimeException($error . " Command output: " . implode("\n", $output));
            }
            
            // Get file size for logging
            $fileSize = filesize($jsonOutputFile);
            $this->logger->debug("JSON output file size: {$fileSize} bytes");
            
            // Parse the JSON output
            $jsonContent = file_get_contents($jsonOutputFile);
            $data = json_decode($jsonContent, true);
            
            if (json_last_error() !== JSON_ERROR_NONE) {
                $error = "Failed to parse JSON output: " . json_last_error_msg();
                $this->logger->error($error);
                throw new \RuntimeException($error);
            }
            
            // Log some basic stats about the data
            $resultsCount = count($data['results'] ?? []);
            $this->logger->info("Parsed crawler output", [
                'resultsCount' => $resultsCount
            ]);
            
            // Clean up the temporary file
            if (file_exists($jsonOutputFile)) {
                $this->logger->debug("Cleaning up temporary file: {$jsonOutputFile}");
                unlink($jsonOutputFile);
            }
            
            return $data;
        } catch (\Throwable $e) {
            // Ensure temp file is cleaned up even on error
            if (file_exists($jsonOutputFile)) {
                $this->logger->debug("Cleaning up temporary file after error: {$jsonOutputFile}");
                unlink($jsonOutputFile);
            }
            
            // Log and re-throw the exception
            $this->logger->error("Error during crawler execution", [
                'exception' => $e,
                'parameters' => $this->sanitizeParameters($parameters)
            ]);
            throw $e;
        }
    }
    
    /**
     * Build the command string with parameters
     * 
     * @param array $parameters Parameters to pass to the crawler
     * @param string $jsonOutputFile Path to the JSON output file
     * @return string The command string
     */
    private function buildCommand(array $parameters, string $jsonOutputFile): string
    {
        // Start with the crawler path
        $command = escapeshellcmd($this->crawlerPath);
        
        // Always add JSON output parameters
        $command .= ' --output=json --output-json-file=' . escapeshellarg($jsonOutputFile);
        
        // Add other parameters
        foreach ($parameters as $key => $value) {
            if (is_bool($value)) {
                // Boolean parameters
                if ($value) {
                    $command .= ' --' . escapeshellarg($key);
                }
            } elseif ($value !== null) {
                // Value parameters
                $command .= ' --' . escapeshellarg($key) . '=' . escapeshellarg((string)$value);
            }
        }
        
        return $command;
    }
    
    /**
     * Execute a command and return its output
     * 
     * @param string $command The command to execute
     * @return array An array containing stdout and stderr output
     * @throws \RuntimeException If the command execution fails
     */
    private function executeCommand(string $command): array
    {
        // Set up process
        $descriptorSpec = [
            0 => ['pipe', 'r'],  // stdin
            1 => ['pipe', 'w'],  // stdout
            2 => ['pipe', 'w']   // stderr
        ];
        
        $this->logger->debug("Starting process execution");
        
        // Start the process
        $process = proc_open($command, $descriptorSpec, $pipes);
        
        if (!is_resource($process)) {
            $error = "Failed to execute crawler process";
            $this->logger->error($error, ['command' => $this->sanitizeCommand($command)]);
            throw new \RuntimeException("{$error}: {$command}");
        }
        
        // Close stdin as we don't need it
        fclose($pipes[0]);
        
        // Read stdout and stderr
        $stdout = stream_get_contents($pipes[1]);
        $stderr = stream_get_contents($pipes[2]);
        
        // Close remaining pipes
        fclose($pipes[1]);
        fclose($pipes[2]);
        
        // Get process exit code
        $exitCode = proc_close($process);
        
        if ($exitCode !== 0) {
            $error = "Crawler process failed with exit code {$exitCode}";
            $this->logger->error($error, [
                'stderr' => $stderr,
                'stdout' => $stdout,
                'exitCode' => $exitCode
            ]);
            throw new \RuntimeException("{$error}: {$stderr}");
        }
        
        $this->logger->debug("Process completed successfully", [
            'exitCode' => $exitCode,
            'stderr_size' => strlen($stderr),
            'stdout_size' => strlen($stdout)
        ]);
        
        return [
            'stdout' => $stdout,
            'stderr' => $stderr
        ];
    }
    
    /**
     * Sanitize parameters for logging
     * 
     * @param array $parameters The parameters to sanitize
     * @return array The sanitized parameters
     */
    private function sanitizeParameters(array $parameters): array
    {
        // Create a copy of the parameters
        $sanitized = $parameters;
        
        // Truncate potentially large values
        foreach ($sanitized as $key => $value) {
            if (is_string($value) && strlen($value) > 100) {
                $sanitized[$key] = substr($value, 0, 50) . '...' . substr($value, -50);
            }
        }
        
        return $sanitized;
    }
    
    /**
     * Sanitize command for logging (remove sensitive information)
     * 
     * @param string $command The command to sanitize
     * @return string The sanitized command
     */
    private function sanitizeCommand(string $command): string
    {
        // For now, just return the command as is
        // In the future, could mask tokens, API keys, etc.
        return $command;
    }
} 