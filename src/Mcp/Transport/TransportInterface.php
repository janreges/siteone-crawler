<?php
/**
 * Transport Interface for MCP
 * 
 * This interface defines the contract for transport mechanisms
 * used by the MCP server to communicate with MCP clients.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Transport;

interface TransportInterface
{
    /**
     * Read a message from the transport.
     * 
     * This method should block until a message is available, or return null
     * if the transport has been closed.
     * 
     * @return string|null The message, or null if the transport is closed
     */
    public function readMessage(): ?string;
    
    /**
     * Write a message to the transport.
     * 
     * @param string $message The message to write
     * @return void
     */
    public function writeMessage(string $message): void;
    
    /**
     * Close the transport.
     * 
     * @return void
     */
    public function close(): void;
} 