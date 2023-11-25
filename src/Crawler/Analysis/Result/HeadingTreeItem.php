<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

use Crawler\Utils;
use DOMDocument;
use DOMNode;
use DOMXPath;

class HeadingTreeItem
{

    /**
     * Heading level (1-6)
     * @var int
     */
    public readonly int $level;

    /**
     * Real heading level by heading structure in HTML
     * @var int|null
     */
    public ?int $realLevel = null;

    /**
     * Heading text
     * @var string
     */
    public readonly string $text;

    /**
     * Heading ID attribute
     * @var string|null
     */
    public readonly ?string $id;

    /**
     * Parent level just for tree building (it is unset after tree is built)
     * @var HeadingTreeItem|null
     */
    private ?HeadingTreeItem $parent = null;

    /**
     * @var HeadingTreeItem[]
     */
    public array $children = [];

    /**
     * Error text in case of error (typically multiple H1s or wrong heading level)
     * @var string|null
     */
    public ?string $errorText = null;

    /**
     * @param int $level
     * @param string $text
     * @param string|null $id
     */
    public function __construct(int $level, string $text, ?string $id)
    {
        $this->level = $level;
        $this->text = $text;
        $this->id = $id;
    }

    public function addChild(HeadingTreeItem $child): void
    {
        $this->children[] = $child;
    }

    public function hasError(): bool
    {
        return $this->errorText !== null;
    }

    /**
     * @param DOMDocument $dom
     * @param int $maxLevels
     * @return HeadingTreeItem[]
     */
    public static function getHeadingTreeFromHtml(DOMDocument $dom, int $maxLevels = 3): array
    {
        $xpath = new DOMXPath($dom);
        $xPathAppend = '';
        for ($i = 2; $i <= $maxLevels; $i++) {
            $xPathAppend .= " or self::h{$i}";
        }
        $nodes = @$xpath->query('//*[self::h1' . $xPathAppend . ']');

        $root = new HeadingTreeItem(0, '', null);
        $currentNode = $root;

        $h1References = [];

        foreach ($nodes ?: [] as $node) {
            /* @var $node DOMNode */
            $level = (int)substr($node->nodeName, 1);

            // WARNING: $node->textContent is not working properly in cases where the website uses other HTML elements
            // inside <h*>, including <script>, so JS code (without <script> tag) is included in the textContent
            //  $text = trim(preg_replace('/\s+/', ' ', $node->textContent));
            $nodeContent = strip_tags(Utils::stripJavaScript($node->ownerDocument->saveHTML($node)));
            $text = trim(preg_replace('/\s+/', ' ', $nodeContent));

            $id = $node->getAttribute('id') ?: null;

            $item = new HeadingTreeItem($level, $text, $id);
            if ($level === 1) {
                $h1References[] = $item;
            }

            while ($currentNode->level >= $level) {
                $currentNode = $currentNode->parent;
            }

            $item->parent = $currentNode;
            $currentNode->children[] = $item;

            $currentNode = $item;
        }

        // set error to multiple h1s if exists
        $h1Count = $dom->getElementsByTagName('h1')->length;
        if ($h1Count > 1) {
            foreach ($h1References as $h1Reference) {
                $h1Reference->errorText = "Multiple H1s ({$h1Count}) found.";
            }
        }

        $finalTreeItemFixes = function (HeadingTreeItem $item, int $realLevel = 1) use (&$finalTreeItemFixes) {
            unset($item->parent);
            $item->realLevel = $realLevel;
            foreach ($item->children as $child) {
                unset($child->parent);
                $finalTreeItemFixes($child, $realLevel + 1);
            }

            if ($item->level !== $item->realLevel) {
                $item->errorText = "Heading level {$item->level} is not correct. Should be {$item->realLevel}.";
            }
        };

        $result = array_filter($root->children, fn($child) => $child instanceof HeadingTreeItem);
        foreach ($result as $item) {
            $finalTreeItemFixes($item);
        }
        return $result;
    }

    /**
     * @param HeadingTreeItem[] $items
     * @return string
     */
    public static function getHeadingTreeUlLiList(array $items): string
    {
        $result = '<ul>';
        foreach ($items as $item) {
            $result .= '<li>' . self::getHeadingTreeUlLi($item, true) . '</li>';
        }
        $result .= '</ul>';
        return $result;
    }

    public static function getHeadingTreeUlLi(HeadingTreeItem $item, bool $addItem = true): string
    {
        $result = '';
        if ($addItem) {
            $txtRow = "<h{$item->level}> {$item->text}" . ($item->id ? ' [#' . $item->id . ']' : '');
            $result = $item->hasError()
                ? (
                    '<span class="help" title="' . htmlspecialchars($item->errorText) . '">'
                    . Utils::getColorText(htmlspecialchars($txtRow), 'magenta')
                    . '</span>'
                )
                : htmlspecialchars($txtRow);
        }

        if ($item->children) {
            $result .= '<ul>';
            foreach ($item->children as $child) {
                $result .= '<li>';
                $txtRow = "<h{$child->level}> " . $child->text . ($child->id ? ' [#' . $child->id . ']' : '');
                $result .= $child->hasError()
                    ? (
                        '<span class="help" title="' . htmlspecialchars($child->errorText) . '">'
                        . Utils::getColorText(htmlspecialchars($txtRow), 'magenta')
                        . '</span>'
                    )
                    : htmlspecialchars($txtRow);
                $result .= self::getHeadingTreeUlLi($child, false);
                $result .= '</li>';
            }
            $result .= '</ul>';
        }
        return $result;
    }

    /**
     * @param HeadingTreeItem[] $items
     * @return string
     */
    public static function getHeadingTreeTxtList(array $items): string
    {
        $result = '';
        foreach ($items as $item) {
            $result .= self::getHeadingTreeTxt($item, true);
        }
        return preg_replace('/\s+/', ' ', $result);
    }

    public static function getHeadingTreeTxt(HeadingTreeItem $item, bool $addItem = true): string
    {
        $result = '';
        if ($addItem) {
            $result .= "<h{$item->level}> {$item->text}" . ($item->id ? ' [#' . $item->id . ']' : '') . "\n";
        }
        foreach ($item->children as $child) {
            $result .= str_repeat('  ', $child->level - 1);
            $txtRow = "<h{$child->level}> {$child->text}" . ($child->id ? ' [#' . $child->id . ']' : '');
            // $result .= ($child->level !== $child->realLevel ? Utils::getColorText($txtRow, 'magenta') : $txtRow);
            $result .= $txtRow;
            $result .= "\n";
            $result .= self::getHeadingTreeTxt($child, false);
        }
        return $result;
    }

}