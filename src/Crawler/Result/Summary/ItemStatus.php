<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Summary;

use Exception;

enum ItemStatus
{
    case OK;
    case NOTICE;
    case WARNING;
    case CRITICAL;
    case INFO;

    /**
     * @param int $rangeId
     * @return ItemStatus
     * @throws Exception
     */
    public static function fromRangeId(int $rangeId): ItemStatus
    {
        return match ($rangeId) {
            0 => ItemStatus::OK,
            1 => ItemStatus::NOTICE,
            2 => ItemStatus::WARNING,
            3 => ItemStatus::CRITICAL,
            4 => ItemStatus::INFO,
            default => throw new Exception(__METHOD__ . ": Unknown range ID '{$rangeId}'"),
        };
    }

    /**
     * @param string $text
     * @return ItemStatus
     * @throws Exception
     */
    public static function fromText(string $text): ItemStatus
    {
        return match (strtoupper($text)) {
            'OK' => ItemStatus::OK,
            'NOTICE' => ItemStatus::NOTICE,
            'WARNING' => ItemStatus::WARNING,
            'CRITICAL' => ItemStatus::CRITICAL,
            'INFO' => ItemStatus::INFO,
            default => throw new Exception(__METHOD__ . ": Unknown status '{$text}'"),
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
