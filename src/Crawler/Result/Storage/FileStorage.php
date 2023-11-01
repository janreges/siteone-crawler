<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Storage;

class FileStorage implements Storage
{

    private string $tmpDir;
    private string $instanceTmpDir;
    private bool $compress = false;

    /**
     * @param string $tmpDir
     * @param bool $compress
     */
    public function __construct(string $tmpDir, bool $compress = false)
    {
        $this->tmpDir = $tmpDir;
        $this->instanceTmpDir = $this->tmpDir . '/' . uniqid();
        if (!mkdir($this->instanceTmpDir, 0777) && !is_dir($this->instanceTmpDir)) {
            throw new \RuntimeException(sprintf('Directory "%s" was not created', $this->instanceTmpDir));
        }
        $this->compress = $compress;
    }

    public function save(string $uqId, string $content): void
    {
        if ($this->compress) {
            $content = gzencode($content);
        }
        file_put_contents($this->instanceTmpDir . '/' . $uqId . $this->getFileExtension(), $content);
    }

    public function load(string $uqId): string
    {
        $content = file_get_contents($this->instanceTmpDir . '/' . $uqId . $this->getFileExtension());
        if ($this->compress) {
            $content = gzdecode($content);
        }
        return $content;
    }

    public function delete(string $uqId): void
    {
        unlink($this->instanceTmpDir . '/' . $uqId);
    }

    public function deleteAll(): void
    {
        foreach (glob($this->instanceTmpDir . '/*.*') as $file) {
            unlink($file);
        }
    }

    private function getFileExtension(): string
    {
        return $this->compress ? '.gz' : '.txt';
    }
}