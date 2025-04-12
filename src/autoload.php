<?php
/**
 * Simple PSR-4 autoloader for SiteOne MCP implementation
 */
declare(strict_types=1);

spl_autoload_register(function (string $class): void {
    // Define the namespace prefix for our project
    $prefix = 'SiteOne\\';
    
    // Base directory for the namespace prefix
    $baseDir = __DIR__ . '/';
    
    // Check if the class uses our namespace prefix
    $len = strlen($prefix);
    if (strncmp($prefix, $class, $len) !== 0) {
        // Class doesn't use our namespace prefix, let other autoloaders handle it
        return;
    }
    
    // Get the relative class name
    $relativeClass = substr($class, $len);
    
    // Convert namespace separator to directory separator
    // and append .php extension
    $file = $baseDir . str_replace('\\', '/', $relativeClass) . '.php';
    
    // If the file exists, require it
    if (file_exists($file)) {
        require $file;
    }
}); 