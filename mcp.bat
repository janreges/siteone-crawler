@echo off
setlocal enabledelayedexpansion
:: MCP Server for SiteOne Crawler
:: This script serves as the entry point for the MCP server (Windows version)

:: Default parameters
set TRANSPORT=stdio
set HOST=127.0.0.1
set PORT=7777

:: Parse command-line arguments
for %%i in (%*) do (
    set ARG=%%i
    if "!ARG:~0,12!"=="--transport=" set TRANSPORT=!ARG:~12!
    if "!ARG:~0,7!"=="--host=" set HOST=!ARG:~7!
    if "!ARG:~0,7!"=="--port=" set PORT=!ARG:~7!
)

:: Get the directory of the script (absolute path)
set DIR=%~dp0
set DIR=%DIR:~0,-1%
:: Replace backslashes with forward slashes for PHP
set DIR_SLASH=%DIR:\=/%

echo Starting MCP server with transport=%TRANSPORT%, host=%HOST%, port=%PORT%
echo Project directory: %DIR%

:: Check if bootstrap file exists
if not exist "%DIR%\src\mcp-bootstrap.php" (
    echo ERROR: Bootstrap file does not exist: %DIR%\src\mcp-bootstrap.php
    exit /b 1
)

:: Set environment variables that will help PHP script identify correct paths
set SCRIPT_DIR=%DIR%
set SCRIPT_FILENAME=%DIR%\src\mcp-server.php
set MCP_DEBUG=1

echo Running swoole-cli from %DIR%\bin\swoole-cli
echo Trying to run MCP server...

:: Try to run the server directly with swoole-cli
cd %DIR%
"%DIR%\bin\swoole-cli" -d display_errors=1 "%DIR%\src\mcp-server.php" -- --transport=%TRANSPORT% --host=%HOST% --port=%PORT% --debug 