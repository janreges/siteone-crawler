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
     * Constructor
     * 
     * @param string $crawlerPath Path to the crawler executable
     * @param string $tempDir Directory for temporary files
     */
    public function __construct(string $crawlerPath = null, string $tempDir = null)
    {
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
        
        // Ensure the temporary directory exists
        if (!is_dir($this->tempDir)) {
            if (!mkdir($this->tempDir, 0755, true) && !is_dir($this->tempDir)) {
                throw new \RuntimeException("Failed to create temporary directory: {$this->tempDir}");
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
        // Generate a unique output file name
        $jsonOutputFile = $this->tempDir . DIRECTORY_SEPARATOR . 'mcp_output_' . uniqid() . '.json';
        
        // Build the command with parameters
        $command = $this->buildCommand($parameters, $jsonOutputFile);
        
        // Execute the command
        $output = $this->executeCommand($command);
        
        // Check if the JSON output file was created
        if (!file_exists($jsonOutputFile)) {
            throw new \RuntimeException("JSON output file was not created. Command output: " . implode("\n", $output));
        }
        
        // Parse the JSON output
        $jsonContent = file_get_contents($jsonOutputFile);
        $data = json_decode($jsonContent, true);
        
        if (json_last_error() !== JSON_ERROR_NONE) {
            throw new \RuntimeException("Failed to parse JSON output: " . json_last_error_msg());
        }
        
        // Clean up the temporary file
        unlink($jsonOutputFile);
        
        return $data;
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
        
        // Start the process
        $process = proc_open($command, $descriptorSpec, $pipes);
        
        if (!is_resource($process)) {
            throw new \RuntimeException("Failed to execute crawler process: {$command}");
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
            throw new \RuntimeException("Crawler process failed with exit code {$exitCode}: {$stderr}");
        }
        
        return [
            'stdout' => $stdout,
            'stderr' => $stderr
        ];
    }
} 