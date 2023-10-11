<?php

namespace Crawler\Result\Storage;

use Crawler\Result\Storage\Storage;

class MemoryStorage implements Storage
{
    private array $storage = [];
    private bool $compress;

    /**
     * @param bool $compress
     */
    public function __construct(bool $compress = false)
    {
        $this->compress = $compress;
    }


    public function save(string $uqId, string $content): void
    {
        if ($this->compress) {
            $content = gzencode($content);
        }
        $this->storage[$uqId] = $content;
    }

    public function load(string $uqId): string
    {
        $content = $this->storage[$uqId] ?? '';
        if ($content && $this->compress) {
            $content = gzdecode($content);
        }
        return $content;
    }

    public function delete(string $uqId): void
    {
        unset($this->storage[$uqId]);
    }

    public function deleteAll(): void
    {
        $this->storage = [];
    }
}