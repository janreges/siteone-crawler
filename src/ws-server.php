<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * Script ws-server.php is used for re-sending messages from crawler.php to connected websocket clients (e.g. Electron app).
 * It is hard to start websocket server directly from crawler.php because our Swoole's eventLoop is used primary for crawler coroutines
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Swoole\WebSocket\Server as WebSocketServer;
use Swoole\Server as TcpServer;

// get options
$options = getopt("", ["tcp-host:", "tcp-port:", "ws-host:", "ws-port:"]);

$wsHost = $options['ws-host'] ?? '0.0.0.0';
$wsPort = intval($options['ws-port'] ?? 8000);
$tcpHost = $options['tcp-host'] ?? '0.0.0.0';
$tcpPort = intval($options['tcp-port'] ?? 8001);

// websocket server for sending messages to clients
$websocketServer = new WebSocketServer($wsHost, $wsPort, SWOOLE_PROCESS);

$websocketServer->on('open', function (WebSocketServer $server, $request) {
    logMessage("WebSocket client {$request->fd} connected.");
});

$websocketServer->on('message', function (WebSocketServer $server, $frame) {
    logMessage("Retrieved message from client {$frame->fd}: {$frame->data}");
});

$websocketServer->on('close', function (WebSocketServer $server, $fd) {
    logMessage("WebSocket client {$fd} disconnected.");
});

// tcp server for receiving messages from crawler.php
$tcpServer = $websocketServer->addListener($tcpHost, $tcpPort, SWOOLE_SOCK_TCP);
$tcpServer->set([
    'open_http_protocol' => false,
    'open_websocket_protocol' => false,
    'open_length_check' => false,
]);
logMessage("TCP server started on {$tcpHost}:{$tcpPort}");

$tcpServer->on('connect', function (TcpServer $server, $fd) {
    logMessage("TCP client {$fd} connected.");
});

$tcpServer->on('receive', function (TcpServer $server, $fd, $reactor_id, $data) use ($websocketServer) {
    logMessage("Received TCP: " . $data);
    // resending message to all connected websocket clients
    foreach ($websocketServer->connections as $clientFd) {
        if ($websocketServer->isEstablished($clientFd)) {
            $websocketServer->push($clientFd, $data);
        }
    }
});

logMessage("Starting websocket server on {$wsHost}:{$wsPort} and TCP server on {$tcpHost}:{$tcpPort}");
$websocketServer->start();

function logMessage(string $message)
{
    // $fullMsg = date('Y-m-d_H:i:s') . ": {$message}\n";
    // echo $fullMsg;
    // file_put_contents(dirname(__FILE__) . '/ws-server.log', $fullMsg, FILE_APPEND);
}