<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export\Utils;

class HtmlToMarkdownConverter
{
    private readonly \DOMDocument $dom;
    private string $strongDelimiter = '**';
    private string $emDelimiter = '*';
    private string $bulletListMarker = '-';
    private string $codeBlockFence = '```';
    private string $horizontalRule = '* * *';
    private string $headingStyle = 'setext'; // 'atx' or 'setext'
    private bool $escapeMode = true;
    private bool $includeImages = true;
    private bool $convertTables = true;
    private bool $convertStrikethrough = true;
    private string $strikethroughDelimiter = '~~';

    public function __construct(
        private readonly string $html,
        private readonly array  $excludedSelectors = []
    )
    {
        $this->dom = new \DOMDocument();
        $this->dom->preserveWhiteSpace = true;
        $this->dom->formatOutput = false;

        // Convert HTML to UTF-8 entities and load HTML
        $html = mb_convert_encoding($this->html, 'HTML-ENTITIES', 'UTF-8');
        if (!$this->dom->loadHTML($html, LIBXML_NOERROR | LIBXML_HTML_NOIMPLIED | LIBXML_HTML_NODEFDTD)) {
            error_log('HTML load error in HtmlToMarkdownConverter');
        }
        $this->removeUnwantedNodes();
        $this->removeExcludedNodes();
    }

    /**
     * Remove unwanted nodes such as <script>, <style>, <noscript>, <head>, <meta>, <link>, and <iframe>
     * as well as comments.
     */
    private function removeUnwantedNodes(): void
    {
        $xpath = new \DOMXPath($this->dom);
        // Remove unwanted tags
        foreach ($xpath->query('//script | //style | //noscript | //head | //meta | //link | //iframe | //frame') as $node) {
            $node->parentNode?->removeChild($node);
        }
        // Remove comments
        foreach ($xpath->query('//comment()') as $comment) {
            $comment->parentNode?->removeChild($comment);
        }
    }

    /**
     * Remove nodes that match the excluded selectors.
     */
    private function removeExcludedNodes(): void
    {
        $xpath = new \DOMXPath($this->dom);
        foreach ($this->excludedSelectors as $selector) {
            if (strpos($selector, '#') === 0) {
                $id = substr($selector, 1);
                $selectorForXPath = "//*[@id='" . $id . "']";
            } elseif (strpos($selector, '.') === 0) {
                $class = substr($selector, 1);
                $selectorForXPath = "//*[contains(concat(' ', normalize-space(@class), ' '), ' " . $class . " ')]";
            } else {
                $selectorForXPath = '//' . $selector;
            }
            foreach ($xpath->query($selectorForXPath) as $node) {
                $node->parentNode?->removeChild($node);
            }
        }
    }

    public function getMarkdown(): string
    {
        $node = $this->dom->documentElement ?? $this->dom->getElementsByTagName('body')->item(0);
        if (!$node) {
            return '';
        }
        $markdown = $this->convertNode($node);
        return trim($this->normalizeWhitespace($markdown));
    }

    private function convertNode(\DOMNode $node): string
    {
        if ($node instanceof \DOMText) {
            return $this->escapeMarkdownChars($node->nodeValue);
        }
        if (!($node instanceof \DOMElement)) {
            return '';
        }
        switch (strtolower($node->nodeName)) {
            case 'strong':
            case 'b':
                return $this->wrapWithDelimiter(
                    $this->collapseInlineWhitespace($this->getInnerMarkdown($node)),
                    $this->strongDelimiter
                );
            case 'em':
            case 'i':
                return $this->wrapWithDelimiter(
                    $this->collapseInlineWhitespace($this->getInnerMarkdown($node)),
                    $this->emDelimiter
                );
            case 'h1':
            case 'h2':
            case 'h3':
            case 'h4':
            case 'h5':
            case 'h6':
                return $this->convertHeading($node);
            case 'p':
                return "\n\n" . $this->getInnerMarkdown($node) . "\n\n";
            case 'br':
                return "  \n";
            case 'hr':
                return "\n\n" . $this->horizontalRule . "\n\n";
            case 'a':
                return $this->convertLink($node);
            case 'img':
                return $this->convertImage($node);
            case 'code':
                return $this->convertInlineCode($node);
            case 'pre':
                return $this->convertCodeBlock($node);
            case 'ul':
            case 'ol':
                return $this->convertList($node);
            case 'blockquote':
                return $this->convertBlockquote($node);
            case 'table':
                return $this->convertTable($node);
            case 's':
            case 'del':
            case 'strike':
                if (!$this->convertStrikethrough) {
                    return $this->getInnerMarkdown($node);
                }
                return $this->wrapWithDelimiter(
                    $this->collapseInlineWhitespace($this->getInnerMarkdown($node)),
                    $this->strikethroughDelimiter
                );
            case 'dl':
                return $this->convertDefinitionList($node);
            case 'dt':
            case 'dd':
                return $this->getInnerMarkdown($node);
            case 'sup':
                return '^' . $this->collapseInlineWhitespace($this->getInnerMarkdown($node)) . '^';
            case 'sub':
                return '~' . $this->collapseInlineWhitespace($this->getInnerMarkdown($node)) . '~';
            default:
                return $this->getInnerMarkdown($node);
        }
    }

    private function getInnerMarkdown(\DOMNode $node): string
    {
        $markdown = '';
        foreach ($node->childNodes as $child) {
            $markdown .= $this->convertNode($child);
        }
        return $markdown;
    }

    /**
     * Collapse multiple whitespace characters into a single space.
     */
    private function collapseInlineWhitespace(string $text): string
    {
        return trim(preg_replace('/\s+/', ' ', $text));
    }

    private function convertHeading(\DOMElement $node): string
    {
        $level = (int)substr($node->nodeName, 1);
        $content = $this->getInnerMarkdown($node);
        $content = trim(str_replace(['#', '*', '_'], '', $content)); // Remove unwanted heading characters

        if (trim($content) === '') {
            return '';
        }

        if ($this->headingStyle === 'setext' && $level <= 2) {
            $underline = str_repeat($level === 1 ? '=' : '-', mb_strlen($content));
            return "\n\n$content\n$underline\n\n";
        }
        $prefix = str_repeat('#', $level);
        return "\n\n$prefix $content\n\n";
    }

    private function convertLink(\DOMElement $node): string
    {
        $text = $this->collapseInlineWhitespace($this->getInnerMarkdown($node));
        $href = $node->getAttribute('href');
        $title = $node->getAttribute('title');

        if (empty($href)) {
            return $text;
        }

        // Check if link contains block elements or spans
        $hasBlockOrSpanElements = false;
        foreach ($node->childNodes as $child) {
            if ($child instanceof \DOMElement && in_array(strtolower($child->nodeName), ['div', 'span', 'p'])) {
                $hasBlockOrSpanElements = true;
                break;
            }
        }

        // If link text is empty, use the URL as text
        if (trim($text) === '') {
            $text = $href;
        }

        $markdown = "[$text]($href";
        if (!empty($title)) {
            $markdown .= " \"$title\"";
        }
        $markdown .= ")";

        // Add newlines around the link if it contains block elements
        if ($hasBlockOrSpanElements) {
            $markdown = "\n{$this->bulletListMarker} " . $markdown . "\n";
        }

        return $markdown;
    }

    private function convertImage(\DOMElement $node): string
    {
        if (!$this->includeImages) {
            $alt = $node->getAttribute('alt');
            return $alt ? $alt : '';
        }

        $alt = $this->collapseInlineWhitespace($node->getAttribute('alt'));
        $src = $node->getAttribute('src');
        $title = $node->getAttribute('title');

        if (empty($src)) {
            return '';
        }

        $markdown = "![$alt]($src";
        if (!empty($title)) {
            $markdown .= " \"$title\"";
        }
        $markdown .= ")";

        return $markdown;
    }

    private function convertInlineCode(\DOMElement $node): string
    {
        $code = $node->textContent;
        preg_match_all('/(`+)/', $code, $matches);
        $maxBackticks = 1;
        if (!empty($matches[1])) {
            foreach ($matches[1] as $seq) {
                $len = mb_strlen($seq);
                if ($len >= $maxBackticks) {
                    $maxBackticks = $len;
                }
            }
        }
        $fence = str_repeat('`', $maxBackticks + 1);
        return $fence . $code . $fence;
    }

    private function convertCodeBlock(\DOMElement $node): string
    {
        $code = $node->textContent;
        $language = '';

        if ($node->hasAttribute('class')) {
            $classes = explode(' ', $node->getAttribute('class'));
            foreach ($classes as $class) {
                if (str_starts_with($class, 'language-')) {
                    $language = substr($class, 9);
                    break;
                }
            }
        }

        $fence = $this->codeBlockFence;
        return "\n\n$fence$language\n$code\n$fence\n\n";
    }

    /**
     * Convert lists with proper handling of nested ul/li/ul blocks,
     * including nested elements containing lists.
     */
    private function convertList(\DOMElement $node, int $indentation = 0): string
    {
        $result = "";
        $isOrdered = $node->nodeName === 'ol';
        $counter = (int)($node->getAttribute('start') ?: 1);

        foreach ($node->childNodes as $child) {
            if ($child instanceof \DOMElement && $child->nodeName === 'li') {
                $liContent = "";
                $nestedLists = "";
                foreach ($child->childNodes as $liChild) {
                    if ($liChild instanceof \DOMElement && (
                            in_array($liChild->nodeName, ['ul', 'ol']) ||
                            ($liChild->getElementsByTagName('ul')->length > 0 || $liChild->getElementsByTagName('ol')->length > 0)
                        )) {
                        if (in_array($liChild->nodeName, ['ul', 'ol'])) {
                            $nestedLists .= "\n" . $this->convertList($liChild, $indentation + 1);
                        } else {
                            // Clone the element to remove nested lists from its text content
                            $clone = $liChild->cloneNode(true);
                            $this->removeNestedLists($clone);
                            $liContent .= $this->convertNode($clone);
                            $nestedLists .= "\n" . $this->extractListsFromElement($liChild, $indentation + 1);
                        }
                    } else {
                        $liContent .= $this->convertNode($liChild);
                    }
                }

                $indentSpaces = str_repeat("    ", $indentation);
                $prefix = $isOrdered ? "$counter. " : $this->bulletListMarker . " ";
                $result .= $indentSpaces . $prefix . $liContent;
                if (trim($nestedLists) !== "") {
                    $result .= "\n" . $nestedLists;
                }
                $result .= "\n";
                if ($isOrdered) {
                    $counter++;
                }
            }
        }
        if ($indentation === 0) {
            $result = "\n" . $result;
        }
        return $result;
    }

    /**
     * Remove any nested ul/ol elements from the given node.
     */
    private function removeNestedLists(\DOMNode $node): void
    {
        if (!($node instanceof \DOMElement)) {
            return;
        }
        $xpath = new \DOMXPath($this->dom);
        foreach ($xpath->query('.//ul | .//ol', $node) as $listNode) {
            $listNode->parentNode?->removeChild($listNode);
        }
    }

    /**
     * Extract and convert all nested ul/ol elements from the given node.
     */
    private function extractListsFromElement(\DOMNode $node, int $indentation): string
    {
        $markdown = "";
        if ($node instanceof \DOMElement) {
            $xpath = new \DOMXPath($this->dom);
            foreach ($xpath->query('.//ul | .//ol', $node) as $listNode) {
                $markdown .= "\n" . $this->convertList($listNode, $indentation);
            }
        }
        return $markdown;
    }


    private function convertBlockquote(\DOMElement $node): string
    {
        $content = $this->getInnerMarkdown($node);
        $lines = explode("\n", $content);
        $markdown = "\n\n";
        foreach ($lines as $line) {
            $markdown .= "> " . trim($line) . "\n";
        }
        return $markdown . "\n";
    }

    private function convertTable(\DOMElement $node): string
    {
        if (!$this->convertTables) {
            return $this->getCleanHtmlTable($node);
        }

        $rows = [];
        $headerCells = [];
        $maxColLengths = [];
        $trs = $node->getElementsByTagName('tr');
        foreach ($trs as $tr) {
            $isHeader = false;
            $parent = $tr->parentNode;
            if ($parent instanceof \DOMElement && $parent->nodeName === 'thead') {
                $isHeader = true;
            }
            $rowCells = [];
            foreach ($tr->childNodes as $cell) {
                if (!($cell instanceof \DOMElement)) {
                    continue;
                }
                if ($cell->nodeName === 'th') {
                    $isHeader = true;
                }
                if (in_array($cell->nodeName, ['th', 'td'])) {
                    $content = trim($this->getInnerMarkdown($cell));
                    $rowCells[] = $content;
                    $colIndex = count($rowCells) - 1;
                    $maxColLengths[$colIndex] = max(
                        $maxColLengths[$colIndex] ?? 0,
                        mb_strlen($content)
                    );
                }
            }
            if ($isHeader) {
                $headerCells = $rowCells;
            } else {
                $rows[] = $rowCells;
            }
        }

        if (empty($headerCells) && empty($rows)) {
            return '';
        }

        $markdown = "\n\n";
        if (!empty($headerCells)) {
            $markdown .= $this->formatTableRow($headerCells, $maxColLengths);
            $markdown .= $this->formatTableSeparator($maxColLengths);
        }
        foreach ($rows as $row) {
            $markdown .= $this->formatTableRow($row, $maxColLengths);
        }

        $markdown = rtrim($markdown, "\n") . "\n\n";
        return $markdown;
    }


    private function formatTableRow(array $cells, array $maxLengths): string
    {
        $row = '|';
        foreach ($cells as $i => $cell) {
            $padding = str_repeat(' ', $maxLengths[$i] - mb_strlen($cell));
            $row .= ' ' . $this->escapeMarkdownTableCellContent($cell) . $padding . ' |';
        }
        return $row . "\n";
    }

    private function formatTableSeparator(array $maxLengths): string
    {
        $separator = '|';
        foreach ($maxLengths as $length) {
            $separator .= ' ' . str_repeat('-', $length) . ' |';
        }
        return $separator . "\n";
    }

    private function getCleanHtmlTable(\DOMElement $node): string
    {
        $cleanTable = $this->dom->createElement('table');
        foreach ($node->childNodes as $child) {
            if ($child instanceof \DOMElement) {
                if (in_array($child->nodeName, ['thead', 'tbody'])) {
                    $section = $this->dom->createElement($child->nodeName);
                    foreach ($child->childNodes as $tr) {
                        if ($tr instanceof \DOMElement) {
                            $newTr = $this->createCleanTableRow($tr);
                            $section->appendChild($newTr);
                        }
                    }
                    $cleanTable->appendChild($section);
                    continue;
                }
                if ($child->nodeName === 'tr') {
                    $cleanTable->appendChild($this->createCleanTableRow($child));
                    continue;
                }
            }
        }
        $html = $this->dom->saveHTML($cleanTable);
        return "\n\n" . $html . "\n\n";
    }

    private function createCleanTableRow(\DOMElement $tr): \DOMElement
    {
        $newTr = $this->dom->createElement('tr');
        foreach ($tr->childNodes as $cell) {
            if ($cell instanceof \DOMElement && in_array($cell->nodeName, ['td', 'th'])) {
                $newCell = $this->dom->createElement($cell->nodeName);
                if ($cell->hasAttribute('colspan')) {
                    $newCell->setAttribute('colspan', $cell->getAttribute('colspan'));
                }
                if ($cell->hasAttribute('rowspan')) {
                    $newCell->setAttribute('rowspan', $cell->getAttribute('rowspan'));
                }
                foreach ($cell->childNodes as $content) {
                    $newCell->appendChild($content->cloneNode(true));
                }
                $newTr->appendChild($newCell);
            }
        }
        return $newTr;
    }

    private function wrapWithDelimiter(string $text, string $delimiter): string
    {
        return $delimiter . trim($text) . $delimiter;
    }

    private function escapeMarkdownChars(string $text): string
    {
        if (!$this->escapeMode) {
            return $text;
        }
        $chars = ['\\', '`', '*', '_', '{', '}', '[', ']', '(', ')', '#', '+', '-', '.', '!', '|'];
        $pattern = '/([' . preg_quote(implode('', $chars), '/') . '])/';
        return preg_replace($pattern, '\\\\$1', $text);
    }

    private function escapeMarkdownTableCellContent(string $text): string
    {
        $chars = ['|'];
        $pattern = '/([' . preg_quote(implode('', $chars), '/') . '])/';
        return preg_replace($pattern, '\\\\$1', $text);
    }

    private function convertDefinitionList(\DOMElement $node): string
    {
        $markdown = "\n\n";
        foreach ($node->childNodes as $item) {
            if ($item->nodeName === 'dt') {
                $markdown .= $this->getInnerMarkdown($item) . "\n";
            } elseif ($item->nodeName === 'dd') {
                $markdown .= ": " . $this->getInnerMarkdown($item) . "\n";
            }
        }
        return $markdown . "\n";
    }

    /**
     * Normalize whitespace by removing duplicate blank lines,
     * trimming trailing spaces, and removing unwanted leading spaces before links.
     */
    private function normalizeWhitespace(string $text): string
    {
        // Remove duplicate empty lines
        $text = preg_replace('/[\n\r]{3,}/', "\n\n", $text);
        // Remove trailing spaces at the end of lines
        $text = preg_replace('/[ \t]+\n/', "\n", $text);
        // Remove unwanted leading spaces before links on new lines
        $text = preg_replace('/^\s+\[/m', '[', $text);
        // Ensure a space between consecutive markdown links on the same line
        $text = preg_replace('/\)\[/', ') [', $text);
        // Remove tabs at the beginning of lines
        $text = preg_replace('/\n[\t]+/', "\n", $text);
        // Remove duplicate empty lines
        $text = preg_replace('/[\n\r]{3,}/', "\n\n", $text);
        // Ensure a space after bullet list markers
        $text = preg_replace('/\n([\#\*\-]+)([ \t]+)/', "\n$1 ", $text);
        // Remove just one "-" on the line
        $text = preg_replace('/[\n\r]-[\n\r$]/', "\n", $text);
        return trim($text, ' -#*');
    }


    public function setStrongDelimiter(string $delimiter): self
    {
        $this->strongDelimiter = $delimiter;
        return $this;
    }

    public function setEmDelimiter(string $delimiter): self
    {
        $this->emDelimiter = $delimiter;
        return $this;
    }

    public function setBulletListMarker(string $marker): self
    {
        $this->bulletListMarker = $marker;
        return $this;
    }

    public function setCodeBlockFence(string $fence): self
    {
        $this->codeBlockFence = $fence;
        return $this;
    }

    public function setHorizontalRule(string $rule): self
    {
        $this->horizontalRule = $rule;
        return $this;
    }

    public function setHeadingStyle(string $style): self
    {
        if (!in_array($style, ['atx', 'setext'])) {
            throw new \InvalidArgumentException('Heading style must be either "atx" or "setext"');
        }
        $this->headingStyle = $style;
        return $this;
    }

    public function setEscapeMode(bool $enable): self
    {
        $this->escapeMode = $enable;
        return $this;
    }

    public function setIncludeImages(bool $include): self
    {
        $this->includeImages = $include;
        return $this;
    }

    public function setConvertTables(bool $convert): self
    {
        $this->convertTables = $convert;
        return $this;
    }

    public function setConvertStrikethrough(bool $convert): self
    {
        $this->convertStrikethrough = $convert;
        return $this;
    }

    public function setStrikethroughDelimiter(string $delimiter): self
    {
        $this->strikethroughDelimiter = $delimiter;
        return $this;
    }
}
