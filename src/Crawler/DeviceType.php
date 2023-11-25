<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

use Exception;
use JsonSerializable;

enum DeviceType implements JsonSerializable
{
    case DESKTOP;
    case MOBILE;
    case TABLET;

    /**
     * @param string $text
     * @return self
     * @throws Exception
     */
    public static function fromText(string $text): self
    {
        $text = trim(strtolower($text));
        if ($text === 'desktop') {
            return self::DESKTOP;
        } elseif ($text === 'mobile') {
            return self::MOBILE;
        } elseif ($text === 'tablet') {
            return self::TABLET;
        } else {
            throw new Exception("Unknown device type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['desktop', 'mobile', 'tablet'];
    }

    /**
     * @return string
     * @throws Exception
     */
    public function jsonSerialize(): string
    {
        if ($this === self::DESKTOP) {
            return 'desktop';
        } elseif ($this === self::MOBILE) {
            return 'mobile';
        } elseif ($this === self::TABLET) {
            return 'tablet';
        } else {
            throw new Exception("Unknown device type");
        }
    }
}