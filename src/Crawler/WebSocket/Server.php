<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\WebSocket;

use Crawler\Output\Output;
use Swoole\WebSocket\Server as SwooleWebSocketServer;
use Swoole\Http\Request;
use Swoole\WebSocket\Frame;

class Server
{
    private SwooleWebSocketServer $server;
    private Output $output;
    private array $retrievedMessageCallback;
    private array $workerStartCallback;

    /**
     * Messages that are queued for sending to clients
     *
     * @var object[]
     */
    private array $queuedMessagesForSending = [];

    /**
     * @param string $listenHost
     * @param int $listenPort
     * @param callable $retrievedMessageCallback
     * @param callable $workerStartCallback
     * @param Output $output
     */
    public function __construct(string $listenHost, int $listenPort, callable $retrievedMessageCallback, callable $workerStartCallback, Output $output)
    {
        $this->server = new SwooleWebSocketServer($listenHost, $listenPort);
        $this->output = $output;
        $this->retrievedMessageCallback = $retrievedMessageCallback;
        $this->workerStartCallback = $workerStartCallback;

        $this->server->on('WorkerStart', function (Server $server, int $workerId) {
            // run only by first worker
            if ($workerId === 0) {
                call_user_func($this->workerStartCallback);
            }
        });
        $this->server->on('open', [$this, 'onOpen']);
        $this->server->on('message', [$this, 'onMessage']);
        $this->server->on('close', [$this, 'onClose']);
    }

    public function start()
    {
        $this->server->start();
        $this->output->addNotice('WebSocket server started on ' . $this->server->host . ':' . $this->server->port);
        $error = $this->server->getLastError();
        if ($error) {
            $this->output->addError('WebSocket server error: ' . $error);
        }
    }

    public function end()
    {
        $this->server->shutdown();
        $this->output->addNotice('WebSocket server stopped');
    }

    public function sendStartMessage(): void
    {
        $this->sendMessage((object)[
            'type' => 'start',
        ]);
    }

    public function sendStopMessage(): void
    {
        $this->sendMessage((object)[
            'type' => 'stop',
        ]);
    }

    public function sendUrlResultMessage(string $url, int $statusCode, int $size, float $execTime): void
    {
        $this->sendMessage((object)[
            'type' => 'urlResult',
            'url' => $url,
            'statusCode' => $statusCode,
            'size' => $size,
            'execTime' => $execTime,
        ]);
    }

    private function sendMessage(object $message)
    {
        $sent = false;
        foreach ($this->server->connections as $fd) {
            if ($this->server->isEstablished($fd)) {
                $this->server->push($fd, json_encode($message, JSON_UNESCAPED_UNICODE));
                $sent = true;
            }
        }

        if (!$sent) {
            $this->queuedMessagesForSending[] = $message;
        }
    }

    public function onOpen(SwooleWebSocketServer $server, Request $request)
    {
        if ($this->queuedMessagesForSending) {
            foreach ($this->queuedMessagesForSending as $message) {
                $this->server->push($request->fd, json_encode($message, JSON_UNESCAPED_UNICODE));
            }
            $this->queuedMessagesForSending = [];
        }
    }

    public function onMessage(SwooleWebSocketServer $server, Frame $frame)
    {
        call_user_func($this->retrievedMessageCallback, $frame->data);
    }

    public function onClose(SwooleWebSocketServer $server, int $fd)
    {
        // nothing to do
    }

}