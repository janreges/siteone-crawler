<?php

namespace Crawler;

enum DeviceType implements \JsonSerializable
{
    case DESKTOP;
    case MOBILE;
    case TABLET;

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
            throw new \Exception("Unknown device type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['desktop', 'mobile', 'tablet'];
    }

    public function jsonSerialize(): mixed
    {
        if ($this === self::DESKTOP) {
            return 'desktop';
        } elseif ($this === self::MOBILE) {
            return 'mobile';
        } elseif ($this === self::TABLET) {
            return 'tablet';
        } else {
            throw new \Exception("Unknown device type '{$this}'");
        }
    }
}