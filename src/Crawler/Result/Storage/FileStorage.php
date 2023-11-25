<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Storage;

use Exception;

class FileStorage implements Storage
{
    private string $cacheDir;
    private bool $compress = false;

    /**
     * @param string $tmpDir
     * @param bool $compress
     * @param string $originUrlDomain
     */
    public function __construct(string $tmpDir, bool $compress, string $originUrlDomain)
    {
        $this->cacheDir = $tmpDir . '/' . preg_replace('/[^a-z0-9.-_]/i', '-', strtolower($originUrlDomain));
        if (!is_dir($this->cacheDir) && !@mkdir($this->cacheDir, 0777, true)) {
            clearstatcache(true);
            if (!is_dir($this->cacheDir) || !is_writable($this->cacheDir)) {
                throw new Exception(sprintf('Directory "%s" was not created', $this->cacheDir));
            }
        }
        $this->compress = $compress;
    }

    /**
     * @param string $uqId
     * @param string $content
     * @return void
     * @throws Exception
     */
    public function save(string $uqId, string $content): void
    {
        if ($this->compress) {
            $content = gzencode($content);
        }

        $filePath = $this->getFilePath($uqId);
        $this->createDirectoryIfNeeded(dirname($filePath));

        file_put_contents($filePath, $content);
    }

    public function load(string $uqId): string
    {
        $content = file_get_contents($this->getFilePath($uqId));
        if ($this->compress) {
            $content = gzdecode($content);
        }
        return $content;
    }

    public function delete(string $uqId): void
    {
        unlink($this->cacheDir . '/' . $uqId);
    }

    public function deleteAll(): void
    {
        foreach (glob($this->cacheDir . '/*.*') as $file) {
            unlink($file);
        }
    }

    private function getFileExtension(): string
    {
        return $this->compress ? 'cache.gz' : 'cache';
    }

    /**
     * @param string $uqId
     * @return string
     */
    private function getFilePath(string $uqId): string
    {
        return $this->cacheDir . '/' . substr($uqId, 0, 2) . '/' . $uqId . '.' . $this->getFileExtension();
    }

    /**
     * @param string $path
     * @return void
     * @throws Exception
     */
    private function createDirectoryIfNeeded(string $path): void
    {
        if (!is_dir($path) || !is_writable($path)) {
            if (!@mkdir($path, 0777, true)) {
                clearstatcache(true);
                if (!is_dir($path) || !is_writable($path)) {
                    throw new Exception("Directory '{$path}' was not created. Please check permissions.");
                }
            }
        }
    }
}