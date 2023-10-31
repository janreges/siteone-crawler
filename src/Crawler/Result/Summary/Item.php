<?php

namespace Crawler\Result\Summary;

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

    public function getAsHtml(string $okIcon = '✅', string $noticeIcon = '🌟', string $warningIcon = '⚠️', string $errorIcon = '⛔', string $infoIcon = 'ℹ️'): string
    {
        $icon = match ($this->status) {
            ItemStatus::OK => $okIcon,
            ItemStatus::NOTICE => $noticeIcon,
            ItemStatus::WARNING => $warningIcon,
            ItemStatus::CRITICAL => $errorIcon,
            ItemStatus::INFO => $infoIcon,
        };
        return $icon . ' ' . rtrim(htmlspecialchars($this->text), '. ') . '.';
    }

    public function getAsConsoleText(string $okIcon = '✅', string $noticeIcon = '🌟', string $warningIcon = '⚠️', string $errorIcon = '⛔', string $infoIcon = 'ℹ️'): string
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