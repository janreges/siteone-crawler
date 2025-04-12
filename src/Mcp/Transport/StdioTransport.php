<?php
/**
 * Stdio Transport implementation for MCP
 * 
 * This class implements the TransportInterface using standard input/output
 * for communication between the MCP server and client.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Transport;

class StdioTransport implements TransportInterface
{
    /**
     * Standard input stream resource
     */
    private $stdin;
    
    /**
     * Standard output stream resource
     */
    private $stdout;
    
    /**
     * Flag indicating if the transport has been closed
     */
    private bool $closed = false;
    
    /**
     * Constructor
     */
    public function __construct()
    {
        // Open stdin and stdout streams in binary mode to ensure proper handling of JSON messages
        $this->stdin = fopen('php://stdin', 'rb');
        $this->stdout = fopen('php://stdout', 'wb');
        
        if ($this->stdin === false || $this->stdout === false) {
            throw new \RuntimeException('Failed to open stdin or stdout');
        }
        
        // Set stream to non-blocking mode if needed
        stream_set_blocking($this->stdin, true);
    }
    
    /**
     * Destructor
     */
    public function __destruct()
    {
        $this->close();
    }
    
    /**
     * {@inheritdoc}
     */
    public function readMessage(): ?string
    {
        if ($this->closed || feof($this->stdin)) {
            return null;
        }
        
        // Read the Content-Length header
        $headers = [];
        $currentHeader = '';
        
        while (true) {
            $line = fgets($this->stdin);
            
            if ($line === false) {
                // End of stream or error
                $this->closed = true;
                return null;
            }
            
            $line = rtrim($line, "\r\n");
            
            if ($line === '') {
                // Empty line indicates end of headers
                break;
            }
            
            $currentHeader .= $line;
            
            if (strpos($line, ':') !== false) {
                list($name, $value) = explode(':', $currentHeader, 2);
                $headers[trim($name)] = trim($value);
                $currentHeader = '';
            }
        }
        
        // Check for Content-Length header
        if (!isset($headers['Content-Length'])) {
            return null;
        }
        
        $contentLength = (int)$headers['Content-Length'];
        
        // Read the message body
        $message = '';
        $bytesRemaining = $contentLength;
        
        while ($bytesRemaining > 0) {
            $chunk = fread($this->stdin, $bytesRemaining);
            
            if ($chunk === false) {
                // End of stream or error
                $this->closed = true;
                return null;
            }
            
            $message .= $chunk;
            $bytesRemaining -= strlen($chunk);
        }
        
        return $message;
    }
    
    /**
     * {@inheritdoc}
     */
    public function writeMessage(string $message): void
    {
        if ($this->closed) {
            return;
        }
        
        // Write the Content-Length header and the message
        $contentLength = strlen($message);
        $data = "Content-Length: {$contentLength}\r\n\r\n{$message}";
        
        fwrite($this->stdout, $data);
        fflush($this->stdout);
    }
    
    /**
     * {@inheritdoc}
     */
    public function close(): void
    {
        if (!$this->closed) {
            if (is_resource($this->stdin)) {
                fclose($this->stdin);
            }
            
            if (is_resource($this->stdout)) {
                fclose($this->stdout);
            }
            
            $this->closed = true;
        }
    }
} 