<?php

namespace Crawler\Result\Summary;

enum ItemStatus
{
    case OK;
    case NOTICE;
    case WARNING;
    case CRITICAL;
    case INFO;

    public static function fromRangeId(int $rangeId): ItemStatus
    {
        return match ($rangeId) {
            0 => ItemStatus::OK,
            1 => ItemStatus::NOTICE,
            2 => ItemStatus::WARNING,
            3 => ItemStatus::CRITICAL,
            4 => ItemStatus::INFO,
        };
    }

    public static function fromText(string $text): ItemStatus
    {
        return match (strtoupper($text)) {
            'OK' => ItemStatus::OK,
            'NOTICE' => ItemStatus::NOTICE,
            'WARNING' => ItemStatus::WARNING,
            'CRITICAL' => ItemStatus::CRITICAL,
            'INFO' => ItemStatus::INFO,
        };
    }

    public static function getSortOrder(ItemStatus $status): int
    {
        return match ($status) {
            self::CRITICAL => 1,
            self::WARNING => 2,
            self::NOTICE => 3,
            self::OK => 4,
            self::INFO => 5,
        };
    }
}
