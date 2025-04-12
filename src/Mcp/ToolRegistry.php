<?php
/**
 * Tool Registry for MCP
 * 
 * This class manages the registration and retrieval of MCP tools.
 */
declare(strict_types=1);

namespace SiteOne\Mcp;

use SiteOne\Mcp\Tool\ToolHandlerInterface;

class ToolRegistry
{
    /**
     * Registered tools
     * 
     * @var array<string, ToolHandlerInterface>
     */
    private array $tools = [];
    
    /**
     * Register a tool handler
     * 
     * @param ToolHandlerInterface $toolHandler The tool handler to register
     * @return void
     * @throws \RuntimeException If a tool with the same name is already registered
     */
    public function registerTool(ToolHandlerInterface $toolHandler): void
    {
        $name = $toolHandler->getName();
        
        if (isset($this->tools[$name])) {
            throw new \RuntimeException("Tool '{$name}' is already registered");
        }
        
        $this->tools[$name] = $toolHandler;
    }
    
    /**
     * Check if a tool is registered
     * 
     * @param string $name The name of the tool
     * @return bool True if the tool is registered, false otherwise
     */
    public function hasTool(string $name): bool
    {
        return isset($this->tools[$name]);
    }
    
    /**
     * Get a registered tool handler
     * 
     * @param string $name The name of the tool
     * @return ToolHandlerInterface The tool handler
     * @throws \RuntimeException If the tool is not registered
     */
    public function getTool(string $name): ToolHandlerInterface
    {
        if (!$this->hasTool($name)) {
            throw new \RuntimeException("Tool '{$name}' is not registered");
        }
        
        return $this->tools[$name];
    }
    
    /**
     * Get all registered tools
     * 
     * @return array<string, ToolHandlerInterface> The registered tools
     */
    public function getAllTools(): array
    {
        return $this->tools;
    }
    
    /**
     * Get tool specifications for all registered tools
     * 
     * @return array The tool specifications
     */
    public function getToolSpecifications(): array
    {
        $specifications = [];
        
        foreach ($this->tools as $name => $tool) {
            $specifications[] = [
                'name' => $name,
                'description' => $tool->getDescription(),
                'schema' => $tool->getParameterSchema()
            ];
        }
        
        return $specifications;
    }
} 