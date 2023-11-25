<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Storage;

class MemoryStorage implements Storage
{
    private array $storage = [];
    private bool $compress;

    /**
     * @param bool $compression
     */
    public function __construct(bool $compression = false)
    {
        $this->compress = $compression;
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