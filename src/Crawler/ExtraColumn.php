<?php

namespace Crawler;

class ExtraColumn
{

    public readonly string $name;
    public readonly ?int $length;
    public readonly bool $truncate;

    public function __construct(string $name, ?int $length, bool $truncate)
    {
        $this->name = $name;
        $this->length = $length;
        $this->truncate = $truncate;
    }

    public function getLength(): int
    {
        return $this->length !== null ? $this->length : strlen($this->name);
    }

    public function getTruncatedValue(?string $value): ?string
    {
        if ($value === null) {
            return null;
        }

        $length = $this->getLength();
        if ($this->truncate && mb_strlen($value) > $length) {
            return trim(mb_substr($value, 0, $this->length - 2)) . '..';
        }

        return str_pad($value, $length);
    }

    /**
     * Get ExtraColumn instance from text like "Column", "Column(20)" or "Column(20!)"
     * @param string $text
     * @return ExtraColumn
     */
    public static function fromText(string $text): ExtraColumn
    {
        $length = null;
        $truncate = false;

        if (preg_match('/^([^(]+)(\((\d+)(!?)\))?$/', $text, $matches)) {
            $name = trim($matches[1]);
            if (isset($matches[3])) {
                $length = (int)$matches[3];
                $truncate = isset($matches[4]) && $matches[4] === '!';
            }
        } else {
            $name = trim($text);
        }

        return new ExtraColumn($name, $length, $truncate);
    }

}