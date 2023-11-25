<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Summary;

use Crawler\Utils;

class Summary
{

    /**
     * @var Item[]
     */
    private array $items = [];

    /**
     * @param Item $item
     * @return void
     */
    public function addItem(Item $item): void
    {
        $this->items[] = $item;
    }

    /**
     * @return Item[]
     */
    public function getItems(): array
    {
        return $this->items;
    }

    private function sortItems(): void
    {
        usort($this->items, function (Item $a, Item $b) {
            return ItemStatus::getSortOrder($a->status) <=> ItemStatus::getSortOrder($b->status);
        });
    }

    public function getAsHtml(): string
    {
        $result = "<ul>\n";
        $this->sortItems();
        foreach ($this->items as $item) {
            $result .= '    <li>' . $item->getAsHtml() . "</li>\n";
        }
        $result .= '</ul>';
        return $result;

    }

    public function getAsConsoleText(): string
    {
        $title = "Summary";
        $titleOutput = $title . PHP_EOL . str_repeat('-', mb_strlen($title)) . PHP_EOL . PHP_EOL;;
        $result = Utils::getColorText($titleOutput, 'blue');

        $this->sortItems();
        foreach ($this->items as $item) {
            $result .= $item->getAsConsoleText() . "\n";
        }
        return $result;
    }

    public function getCountByItemStatus(ItemStatus $status): int
    {
        $count = 0;
        foreach ($this->items as $item) {
            if ($item->status === $status) {
                $count++;
            }
        }
        return $count;
    }

}