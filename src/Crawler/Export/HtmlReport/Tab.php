<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export\HtmlReport;

class Tab
{

    public readonly string $name;
    public readonly ?string $description;
    public readonly string $radioHtmlId;
    public readonly string $contentHtmlId;
    public readonly string $tabContent;
    public readonly bool $addHeading;
    public readonly ?int $fixedOrder;
    public ?int $order = null;

    /**
     * @var Badge[]
     */
    public array $badges;

    /**
     * @param string $name
     * @param string|null $description
     * @param string $tabContent
     * @param bool $addHeading
     * @param Badge[] $badges
     * @param int|null $fixedOrder
     */
    public function __construct(string $name, ?string $description, string $tabContent, bool $addHeading = false, array $badges = [], ?int $fixedOrder = null)
    {
        $this->name = $name;
        $this->description = $description;
        $this->tabContent = $tabContent;
        $this->addHeading = $addHeading;

        $this->radioHtmlId = 'radio_' . strtolower(preg_replace('/[^a-z0-9\-]+/i', '_', $name));
        $this->contentHtmlId = 'content_' . strtolower(preg_replace('/[^a-z0-9\-]+/i', '_', $name));

        $this->badges = [];
        foreach ($badges as $badge) {
            $this->addBadge($badge);
        }

        $this->fixedOrder = $fixedOrder;
    }

    public function addBadge(Badge $badge): void
    {
        $this->badges[] = $badge;
    }

    public function setOrder(?int $order): void
    {
        $this->order = $order;
    }

    public function getFinalSortOrder(): int
    {
        return $this->order !== null ? $this->order : ($this->fixedOrder !== null ? $this->fixedOrder : 1000);
    }

}