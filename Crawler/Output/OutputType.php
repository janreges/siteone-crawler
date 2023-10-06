<?php

namespace Crawler\Output;

enum OutputType implements \JsonSerializable
{
    case FORMATTED_TEXT;
    case JSON;
    case MULTI;

    public static function fromText(string $text): self
    {
        $text = trim(strtolower($text));
        if ($text === 'text') {
            return self::FORMATTED_TEXT;
        } elseif ($text === 'json') {
            return self::JSON;
        } else {
            throw new \Exception("Unknown output type '{$text}'. Supported values are: " . implode(', ', self::getAvailableTextTypes()));
        }
    }

    /**
     * @return string[]
     */
    public static function getAvailableTextTypes(): array
    {
        return ['text', 'json'];
    }

    public function jsonSerialize(): mixed
    {
        if ($this === self::FORMATTED_TEXT) {
            return 'text';
        } elseif ($this === self::JSON) {
            return 'json';
        } elseif ($this === self::MULTI) {
            return 'multi';
        } else {
            throw new \Exception("Unknown output type '{$this}'");
        }
    }
}