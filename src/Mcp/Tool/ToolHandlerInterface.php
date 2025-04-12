<?php
/**
 * Tool Handler Interface for MCP
 * 
 * This interface defines the contract for MCP tool handlers.
 */
declare(strict_types=1);

namespace SiteOne\Mcp\Tool;

interface ToolHandlerInterface
{
    /**
     * Get the name of the tool.
     * 
     * The name should be in the format "namespace/toolName".
     * 
     * @return string The tool name
     */
    public function getName(): string;
    
    /**
     * Get the description of the tool.
     * 
     * @return string The tool description
     */
    public function getDescription(): string;
    
    /**
     * Get the parameter schema for the tool.
     * 
     * The schema should be a JSON Schema object describing the expected parameters.
     * 
     * @return array The parameter schema
     */
    public function getParameterSchema(): array;
    
    /**
     * Execute the tool with the given parameters.
     * 
     * @param array $parameters The parameters to pass to the tool
     * @return array The result of executing the tool
     * @throws \RuntimeException If the tool execution fails
     */
    public function execute(array $parameters): array;
} 