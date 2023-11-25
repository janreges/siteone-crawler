<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Summary;

use Crawler\Utils;

class Item
{
    public readonly string $aplCode;
    public readonly string $text;
    public readonly ItemStatus $status;

    /**
     * @param string $aplCode
     * @param string $text
     * @param ItemStatus $status
     */
    public function __construct(string $aplCode, string $text, ItemStatus $status)
    {
        $this->aplCode = $aplCode;
        $this->text = $text;
        $this->status = $status;
    }

    // public function getAsHtml(string $okIcon = '✅', string $noticeIcon = '📌', string $warningIcon = '⚠️', string $errorIcon = '⛔', string $infoIcon = '⏩'): string
    public function getAsHtml(string $okIcon = '✅', string $noticeIcon = '⏩', string $warningIcon = '⚠️', string $errorIcon = '⛔', string $infoIcon = '📌'): string
    {
        $icon = match ($this->status) {
            ItemStatus::OK => $okIcon,
            ItemStatus::NOTICE => $noticeIcon,
            ItemStatus::WARNING => $warningIcon,
            ItemStatus::CRITICAL => $errorIcon,
            ItemStatus::INFO => $infoIcon,
        };
        return $icon . ' ' . rtrim(htmlspecialchars(Utils::removeAnsiColors($this->text)), '. ') . '.';
    }

    public function getAsConsoleText(string $okIcon = '✅', string $noticeIcon = '⏩', string $warningIcon = '⚠️', string $errorIcon = '⛔', string $infoIcon = '📌'): string
    {
        $icon = match ($this->status) {
            ItemStatus::OK => $okIcon,
            ItemStatus::NOTICE => $noticeIcon,
            ItemStatus::WARNING => $warningIcon,
            ItemStatus::CRITICAL => $errorIcon,
            ItemStatus::INFO => $infoIcon,
        };
        return $icon . ' ' . rtrim($this->text, '. ') . '.';
    }

}