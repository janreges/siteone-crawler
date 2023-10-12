<?php

namespace Crawler\Result\Summary;

enum ItemStatus
{
    case OK;
    case WARNING;
    case ERROR;
    case INFO;

    public static function fromRangeId(int $rangeId): ItemStatus
    {
        return match ($rangeId) {
            0 => ItemStatus::OK,
            1 => ItemStatus::WARNING,
            2 => ItemStatus::ERROR,
            3 => ItemStatus::INFO,
        };
    }

    public static function fromText(string $text): ItemStatus
    {
        return match (strtoupper($text)) {
            'OK' => ItemStatus::OK,
            'WARN' => ItemStatus::WARNING,
            'ERROR' => ItemStatus::ERROR,
            'INFO' => ItemStatus::INFO,
        };
    }

    public static function getSortOrder(ItemStatus $status): int
    {
        return match ($status) {
            self::OK => 3,
            self::WARNING => 2,
            self::ERROR => 1,
            self::INFO => 4
        };
    }
}
