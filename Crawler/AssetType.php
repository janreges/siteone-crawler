<?php

namespace Crawler;

enum AssetType implements \JsonSerializable
{
    case FONTS;
    case IMAGES;
    case STYLES;
    case SCRIPTS;
    case FILES;

    public static function fromText(string $text): self
    {
        $text = trim(strtolower($text));
        if ($text === 'fonts') {
            return self::FONTS;
        } elseif ($text === 'images') {
            return self::IMAGES;
        } elseif ($text === 'styles') {
            return self::STYLES;
        } elseif ($text === 'scripts') {
            return self::SCRIPTS;
        } elseif ($text === 'files') {
            return self::FILES;
        } else {
            throw new \Exception("Unknown asset type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['fonts', 'images', 'styles', 'scripts', 'files'];
    }

    public function jsonSerialize(): mixed
    {
        if ($this === self::FONTS) {
            return 'fonts';
        } elseif ($this === self::IMAGES) {
            return 'images';
        } elseif ($this === self::STYLES) {
            return 'styles';
        } elseif ($this === self::SCRIPTS) {
            return 'scripts';
        } elseif ($this === self::FILES) {
            return 'files';
        } else {
            throw new \Exception("Unknown asset type '{$this}'");
        }
    }
}