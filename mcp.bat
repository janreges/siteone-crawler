@echo off
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

:: Get the directory of the script
set DIR=%~dp0

:: Run the PHP MCP server with the provided arguments
php -f "%DIR%src\mcp-server.php" -- --transport=%TRANSPORT% --host=%HOST% --port=%PORT% 