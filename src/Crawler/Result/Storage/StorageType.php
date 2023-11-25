<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Storage;

use Exception;
use JsonSerializable;

enum StorageType implements JsonSerializable
{
    case MEMORY;
    case FILE;

    /**
     * @param string $text
     * @return self
     * @throws Exception
     */
    public static function fromText(string $text): self
    {
        $text = trim(strtolower($text));
        if ($text === 'memory') {
            return self::MEMORY;
        } elseif ($text === 'file') {
            return self::FILE;
        } else {
            throw new Exception("Unknown storage type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['memory', 'file'];
    }

    /**
     * @return string
     * @throws Exception
     */
    public function jsonSerialize(): string
    {
        if ($this === self::MEMORY) {
            return 'memory';
        } elseif ($this === self::FILE) {
            return 'file';
        } else {
            throw new Exception("Unknown storage type");
        }
    }
}