<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Output;

use Exception;
use JsonSerializable;

enum OutputType implements JsonSerializable
{
    case TEXT;
    case JSON;
    case MULTI;

    /**
     * @param string $text
     * @return self
     * @throws Exception
     */
    public static function fromText(string $text): self
    {
        $text = trim(strtolower($text));
        if ($text === 'text') {
            return self::TEXT;
        } elseif ($text === 'json') {
            return self::JSON;
        } else {
            throw new Exception("Unknown output type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['text', 'json'];
    }

    /**
     * @return string
     * @throws Exception
     */
    public function jsonSerialize(): string
    {
        if ($this === self::TEXT) {
            return 'text';
        } elseif ($this === self::JSON) {
            return 'json';
        } elseif ($this === self::MULTI) {
            return 'multi';
        } else {
            throw new Exception("Unknown output type");
        }
    }
}