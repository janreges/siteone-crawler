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
    // Deduplication properties removed
    public function __construct(
        private readonly string $html,
        private readonly array $excludedSelectors = [],
        private readonly array $implicitExcludedSelectors = ['.hidden', '.hide', '.invisible', '.lg:sl-hidden', '.md:sl-hidden', '.lg:hidden', '.md:hidden']
    ) {
        $this->dom = new \DOMDocument();
        $this->dom->preserveWhiteSpace = true;
        $this->dom->formatOutput = false;

        // Convert HTML to UTF-8 entities and load HTML
        // Use error suppression for loadHTML as it generates warnings for invalid HTML
        $html = mb_convert_encoding($this->html, 'HTML-ENTITIES', 'UTF-8');
        if (@!$this->dom->loadHTML($html, LIBXML_NOERROR | LIBXML_HTML_NOIMPLIED | LIBXML_HTML_NODEFDTD)) {
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
        $allExcludedSelectors = array_merge($this->excludedSelectors, $this->implicitExcludedSelectors);
        foreach ($allExcludedSelectors as $selector) {
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
        // Try to get the body element first, fallback to documentElement
        $body = $this->dom->getElementsByTagName('body')->item(0);
        $node = $body ?? $this->dom->documentElement;
        if (!$node) {
            return '';
        }
        $rawMarkdown = $this->convertNode($node);
        $normalizedMarkdown = $this->normalizeWhitespace($rawMarkdown);

        // --- Start Deduplication Logic ---
        // Split into blocks based on two or more newlines
        $blocks = preg_split('/\n{2,}/', $normalizedMarkdown);
        // Deduplication logic disabled for now as it's causing type errors
        // $blocks = false; // temporary disabled duplicates removing
        if ($blocks === false || count($blocks) <= 1) {
            // No blocks or only one block, nothing to deduplicate
            return trim($normalizedMarkdown);
        }

        $fingerprints = []; // Map: [fingerprint => ['block' => original_block, 'index' => original_index]]
        $uniqueBlocksInOrder = []; // Array to store final blocks preserving relative order

        foreach ($blocks as $index => $originalBlock) {
            $trimmedBlock = trim($originalBlock);

            // Skip completely empty blocks after trimming, but keep track of original index if needed for spacing later
            // For now, we filter them out before fingerprinting. If spacing needs explicit preservation, logic might adjust.
            if (empty($trimmedBlock)) {
                // Option 1: Keep empty blocks to preserve spacing structure
                 $uniqueBlocksInOrder[$index] = $originalBlock; // Store original block (might be just whitespace/newlines)
                // Option 2: Discard empty blocks (simpler, might alter vertical spacing)
                // continue; // Uncomment this line to discard empty blocks entirely

                // Let's go with Option 1 for now to be less destructive of original spacing
                continue;
            }

            // Create fingerprint: lowercase alphanumeric only
            // Using mb_strtolower for multi-byte safety, although preg_replace might handle it
            $fingerprint = mb_strtolower(preg_replace('/[^a-z0-9]+/i', '', $trimmedBlock));

            // If fingerprint is empty (block contained only non-alphanumeric chars), treat as unique
            if (empty($fingerprint)) {
                 $uniqueBlocksInOrder[$index] = $originalBlock;
                 continue;
            }

            if (!isset($fingerprints[$fingerprint])) {
                // First time seeing this fingerprint
                $fingerprints[$fingerprint] = [
                    'block' => $originalBlock,
                    'index' => $index // Store original index
                ];
                 $uniqueBlocksInOrder[$index] = $originalBlock; // Add to ordered list
            } else {
                // Duplicate fingerprint found
                $existingBlockData = $fingerprints[$fingerprint];
                $existingBlock = $existingBlockData['block'];
                $existingIndex = $existingBlockData['index'];

                // Compare original lengths (using mb_strlen for multi-byte safety)
                if (mb_strlen(trim($originalBlock)) > mb_strlen(trim($existingBlock))) {
                    // Current block is longer, replace the previous one in the ordered list
                    // Check if the shorter block actually exists in the ordered list before unsetting
                    if (isset($uniqueBlocksInOrder[$existingIndex])) {
                        unset($uniqueBlocksInOrder[$existingIndex]); // Remove the shorter one
                    }
                    $uniqueBlocksInOrder[$index] = $originalBlock; // Add the longer one at its original position
                    // Update the fingerprint map to point to the new longer block and its index
                    $fingerprints[$fingerprint] = [
                        'block' => $originalBlock,
                        'index' => $index
                    ];
                } else {
                    // Existing block is longer or equal, do nothing.
                    // The current (shorter or equal) block is implicitly discarded
                    // as it's not added to $uniqueBlocksInOrder at this index.
                }
            }
        }

        // Filter out potential null/unset values and re-index if necessary before imploding
        // Using array_filter might re-index numerically, potentially losing original spacing context if empty blocks were kept via index.
        // Let's stick with the indexed array and implode directly. Empty blocks kept by index will become empty strings between \n\n.
        // If empty blocks were fully discarded, this is simpler.
        // ksort($uniqueBlocksInOrder); // Ensure original order if indices matter and manipulation occurred

        // Reconstruct the markdown preserving original relative order and spacing blocks
        // Implode will place "\n\n" between elements.
        $finalMarkdown = implode("\n\n", $uniqueBlocksInOrder);
        // --- End Deduplication Logic ---

        // Replace backslashes with the actual characters they represent
        $originalMarkdown = $finalMarkdown;
        $finalMarkdown = @preg_replace('/\\\\([.-])/', '$1', $finalMarkdown);
        if ($finalMarkdown === null) {
            $finalMarkdown = $originalMarkdown;
        }

        // Final trim to remove leading/trailing whitespace from the whole result
        return trim($finalMarkdown);
    }

    private function convertNode(\DOMNode $node): string
    {
        if ($node instanceof \DOMText) {
            // Handle potential parent context (e.g., don't escape inside <code>)
            $parentTagName = $node->parentNode instanceof \DOMElement ? strtolower($node->parentNode->tagName) : null;
            if ($parentTagName === 'code' || $parentTagName === 'pre') {
                return $node->nodeValue; // No escaping for code blocks/inline code text nodes
            }
            return $this->escapeMarkdownChars($node->nodeValue);
        }
        if (!($node instanceof \DOMElement)) {
            return ''; // Ignore comments, processing instructions, etc.
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
                // Add block context only if the paragraph is not empty after conversion
                $innerMd = trim($this->getInnerMarkdown($node));
                return !empty($innerMd) ? "\n\n" . $innerMd . "\n\n" : '';
            case 'br':
                // Check parent context, maybe ignore inside <pre>?
                // Standard Markdown: two spaces for hard break
                return "  \n";
            case 'hr':
                return "\n\n" . $this->horizontalRule . "\n\n";
            case 'a':
                // Links are handled by getInnerMarkdown if consecutive, otherwise convertLink is called
                // This case should ideally not be hit directly if getInnerMarkdown works correctly,
                // but keep convertLink call as a fallback for single links.
                return $this->convertLink($node);
            case 'img':
                return $this->convertImage($node);
            case 'code':
                // Inline code - content is handled by convertNode(DOMText) above
                return $this->convertInlineCode($node);
            case 'pre':
                // Block code - content is handled by convertNode(DOMText) above
                return $this->convertCodeBlock($node);
            case 'ul':
            case 'ol':
                return $this->convertListToMarkdown($node);
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
            // dt and dd are handled within convertDefinitionList or getInnerMarkdown
            case 'dt':
            case 'dd':
                // Should typically be handled by parent (dl), but provide fallback
                return $this->getInnerMarkdown($node);
            case 'sup':
                // Basic superscript handling
                return '^' . $this->collapseInlineWhitespace($this->getInnerMarkdown($node)) . '^';
            case 'sub':
                // Basic subscript handling
                return '~' . $this->collapseInlineWhitespace($this->getInnerMarkdown($node)) . '~';

            // ignore form elements and other non-content tags
            case 'form':
            case 'fieldset':
            case 'legend':
            case 'label':
            case 'dialog':
            case 'button':
            case 'input':
            case 'select':
            case 'textarea':
            case 'script': // Already removed by removeUnwantedNodes, but belt-and-suspenders
            case 'style':
            case 'noscript':
            case 'head':
            case 'meta':
            case 'link':
            case 'iframe':
            case 'frame':
                return '';

            case 'nav': // Often contains lists/links handled inside, but nav itself adds no markdown
            case 'header':
            case 'footer':
            case 'aside':
            case 'article': // Treat like div by default
            case 'section': // Treat like div by default
            case 'main':    // Treat like div by default
            case 'figure':  // Content (img, figcaption) handled inside
            case 'figcaption': // Treat like p? Or just inner markdown?
                return $this->getInnerMarkdown($node); // Process children by default for containers
            case 'div':
            case 'span':
                // Treat like containers, just process children
                return $this->getInnerMarkdown($node);
            default:
                // For unknown tags, just process their children
                // error_log("HtmlToMarkdownConverter: Encountered unknown tag '{$node->nodeName}', processing children.");
                return $this->getInnerMarkdown($node);
        }
    }

    // *** CORRECTED getInnerMarkdown ***
    private function getInnerMarkdown(\DOMNode $node): string
    {
        $markdown = '';
        $consecutiveLinks = []; // Stores only *valid* consecutive links

        foreach ($node->childNodes as $child) {
            $isPotentialLink = ($child instanceof \DOMElement && strtolower($child->nodeName) === 'a');
            $isValidLink = false;

            // Check if it's a valid link for consecutive formatting purposes
            if ($isPotentialLink && $child instanceof \DOMElement) {
                $href = $child->getAttribute('href');
                // Must have href AND (non-empty text content OR an image child)
                $textContent = trim($child->textContent);
                $hasImageChild = $child->getElementsByTagName('img')->length > 0;

                if (!empty($href) && (!empty($textContent) || $hasImageChild)) {
                    $isValidLink = true;
                }
            }

            if ($isValidLink) {
                // It's a valid link, add it to the list
                $consecutiveLinks[] = $child;
            } elseif ($child instanceof \DOMText && trim($child->nodeValue) === '' && !empty($consecutiveLinks)) {
                // Ignore whitespace text nodes *between* potential valid links
                continue;
            } else {
                // Not a valid link, or not a link at all. Process collected valid links first.
                // Use original threshold (>= 2) for table formatting of VALID links.
                if (count($consecutiveLinks) >= 2) {
                    $markdown .= $this->convertConsecutiveLinksToTable($consecutiveLinks);
                } elseif (count($consecutiveLinks) === 1) {
                    $markdown .= $this->convertLink($consecutiveLinks[0]);
                }
                $consecutiveLinks = []; // Reset

                // Now process the current node that broke the sequence
                // (This will correctly handle anchors or other non-valid links via convertNode)
                $markdown .= $this->convertNode($child);
            }
        }

        // Process any remaining collected valid links at the end
        if (count($consecutiveLinks) >= 2) { // Use original threshold >= 2
            $markdown .= $this->convertConsecutiveLinksToTable($consecutiveLinks);
        } elseif (count($consecutiveLinks) === 1) {
            $markdown .= $this->convertLink($consecutiveLinks[0]);
        }

        return $markdown;
    }


    /**
     * Collapse multiple whitespace characters into a single space.
     */
    private function collapseInlineWhitespace(string $text): string
    {
        // Also replaces non-breaking spaces with regular spaces
        $text = str_replace(['&nbsp;', "\xc2\xa0"], ' ', $text); // Handle UTF-8 non-breaking space too
        return trim(preg_replace('/\s+/u', ' ', $text));
    }

    private function convertHeading(\DOMElement $node): string
    {
        $level = (int) substr($node->nodeName, 1);
        // Use collapseInlineWhitespace for content extraction
        $content = $this->collapseInlineWhitespace($this->getInnerMarkdown($node));
        // Remove markdown characters that might interfere if used inside headings
        $content = trim(str_replace(['#', '*', '_', '`', '[', ']'], '', $content));

        if (empty($content)) {
            return '';
        }

        if ($this->headingStyle === 'setext' && $level <= 2) {
            $underline = str_repeat($level === 1 ? '=' : '-', mb_strlen($content));
            return "\n\n" . $content . "\n" . $underline . "\n\n";
        }
        // ATX style
        $prefix = str_repeat('#', $level);
        return "\n\n" . $prefix . ' ' . $content . "\n\n";
    }

    // *** CORRECTED convertLink ***
    private function convertLink(\DOMNode $node): string
    {
        // This method is now only called for single links (not consecutive ones)
        $href = $node->getAttribute('href');
        if (empty($href)) {
            // If href is missing, return the inner content as plain text
            return $this->getInnerMarkdown($node);
        }

        // Extract text content using getInnerMarkdown for consistency, then collapse whitespace
        $text = $this->collapseInlineWhitespace($this->getInnerMarkdown($node));

        // If link text is empty after collapsing, use the href as text
        if (empty($text)) {
            $text = $href;
        }

        $title = $node->getAttribute('title');

        // Build the markdown link
        $markdown = '[' . $text . '](' . $href; // Don't escape text, it might contain valid markdown (e.g., images)
        if (!empty($title)) {
            $markdown .= ' "' . $this->escapeMarkdownChars($title) . '"'; // Escape title
        }
        $markdown .= ')';

        // No special handling for block elements inside needed anymore

        return $markdown;
    }


    private function convertImage(\DOMElement $node): string
    {
        if (!$this->includeImages) {
            $alt = $node->getAttribute('alt');
            return $alt ? $this->escapeMarkdownChars($alt) : '';
        }

        $alt = $this->collapseInlineWhitespace($node->getAttribute('alt'));
        $src = $node->getAttribute('src');
        $title = $node->getAttribute('title');

        if (empty($src)) {
            return ''; // Don't output anything if src is missing
        }

        // Escape title only
        // $alt = $this->escapeMarkdownChars($alt); // Don't escape alt text, it might contain valid markdown
        $title = $this->escapeMarkdownChars($title);

        $markdown = "![$alt]($src";
        if (!empty($title)) {
            $markdown .= " \"$title\"";
        }
        $markdown .= ")";

        // Add block context (newlines) around images
        return "\n\n" . $markdown . "\n\n";
    }

    private function convertInlineCode(\DOMElement $node): string
    {
        $code = $node->textContent; // Get raw text content
        // Determine required backticks based on content
        preg_match_all('/`+/', $code, $matches);
        $maxBackticks = 0;
        if (!empty($matches[0])) {
            foreach ($matches[0] as $seq) {
                $maxBackticks = max($maxBackticks, mb_strlen($seq));
            }
        }
        $fence = str_repeat('`', $maxBackticks + 1);

        // Add spaces if code starts or ends with a backtick
        $prefixSpace = str_starts_with($code, '`') ? ' ' : '';
        $suffixSpace = str_ends_with($code, '`') ? ' ' : '';

        // Trim leading/trailing whitespace *within* the code before adding fence/spaces
        $trimmedCode = trim($code);

        return $fence . $prefixSpace . $trimmedCode . $suffixSpace . $fence;
    }

    private function convertCodeBlock(\DOMElement $node): string
    {
        // Find inner <code> element if present, otherwise use <pre> content
        $codeElement = $node->getElementsByTagName('code')->item(0);
        $code = $codeElement ? $codeElement->textContent : $node->textContent;

        // Trim leading/trailing newlines often added by browsers/editors
        $code = trim($code, "\n\r");

        // Replace '\' followed by multiple spaces with '\' + newline for multi-line commands
        // Replace '\' followed by multiple spaces with '\' + newline + original spaces for multi-line commands
        $code = preg_replace('/(\\\\)(\s{2,})/', "$1\n$2", $code);

        $language = '';
        // Check class on <pre> or inner <code>
        $classAttr = $node->getAttribute('class');
        if ($codeElement && !$classAttr) {
            $classAttr = $codeElement->getAttribute('class');
        }

        if ($classAttr) {
            $classes = explode(' ', $classAttr);
            foreach ($classes as $class) {
                if (str_starts_with($class, 'language-')) {
                    $language = substr($class, 9);
                    break;
                } elseif (str_starts_with($class, 'lang-')) {
                    $language = substr($class, 5);
                    break;
                }
            }
        }

        $fence = $this->codeBlockFence;
        // Ensure language identifier doesn't contain spaces or backticks
        $language = preg_replace('/[\s`]/', '', $language);

        return "\n\n" . $fence . $language . "\n" . $code . "\n" . $fence . "\n\n";
    }

    private function convertBlockquote(\DOMElement $node): string
    {
        $content = trim($this->getInnerMarkdown($node));
        if (empty($content)) {
            return '';
        }
        $lines = explode("\n", $content);
        $markdown = '';
        foreach ($lines as $line) {
            $markdown .= "> " . $line . "\n"; // Apply '>' to each line
        }
        // Remove trailing newline added by loop, add block context
        return "\n\n" . rtrim($markdown) . "\n\n";
    }

    // *** CORRECTED convertTable ***
    private function convertTable(\DOMElement $node): string
    {
        if (!$this->convertTables) {
            return $this->getCleanHtmlTable($node);
        }

        $rows = [];
        $headerCells = [];
        $maxColLengths = [];
        $hasHeader = false;

        // Process thead first if it exists
        $thead = $node->getElementsByTagName('thead')->item(0);
        if ($thead) {
            $hasHeader = true;
            $tr = $thead->getElementsByTagName('tr')->item(0); // Assuming single header row
            if (!$tr) {
                // Handle case where <tr> is not present but <th> is a direct child of <thead>
                $thElements = $thead->getElementsByTagName('th');
                if ($thElements->length > 0) {
                    $colIndex = 0;
                    foreach ($thElements as $cell) {
                        $content = $this->extractHeaderContent($cell);
                        $headerCells[$colIndex] = $content;
                        $maxColLengths[$colIndex] = max($maxColLengths[$colIndex] ?? 0, mb_strlen($content));
                        $colIndex++;
                    }
                }
            } else {
                $colIndex = 0;
                foreach ($tr->childNodes as $cell) {
                    if ($cell instanceof \DOMElement && ($cell->nodeName === 'th' || $cell->nodeName === 'td')) {
                        $content = $this->extractHeaderContent($cell);
                        $headerCells[$colIndex] = $content;
                        $maxColLengths[$colIndex] = max($maxColLengths[$colIndex] ?? 0, mb_strlen($content));
                        $colIndex++;
                    }
                }
            }
        }

        // Process tbody and direct tr children
        $bodies = $node->getElementsByTagName('tbody');
        $directTrs = [];
        if ($bodies->length > 0) {
            foreach ($bodies as $tbody) {
                foreach ($tbody->getElementsByTagName('tr') as $tr) {
                    $directTrs[] = $tr;
                }
            }
        } else {
            // Look for direct TR children if no thead/tbody
            foreach ($node->childNodes as $child) {
                if ($child instanceof \DOMElement && $child->nodeName === 'tr') {
                    $directTrs[] = $child;
                }
            }
        }


        foreach ($directTrs as $tr) {
            // If no header found yet, treat the first row as header if it contains <th>
            if (!$hasHeader && empty($rows)) {
                $potentialHeaderCells = [];
                $isPotentialHeader = false;
                $colIndex = 0;
                foreach ($tr->childNodes as $cell) {
                    if ($cell instanceof \DOMElement && ($cell->nodeName === 'th' || $cell->nodeName === 'td')) {
                        if ($cell->nodeName === 'th')
                            $isPotentialHeader = true;
                        $content = $this->extractHeaderContent($cell);
                        $potentialHeaderCells[$colIndex] = $content;
                        $maxColLengths[$colIndex] = max($maxColLengths[$colIndex] ?? 0, mb_strlen($content));
                        $colIndex++;
                    }
                }
                if ($isPotentialHeader) {
                    $headerCells = $potentialHeaderCells;
                    $hasHeader = true;
                    continue; // Skip adding this row to data rows
                }
            }

            // Process as data row
            $rowCells = [];
            $colIndex = 0;
            foreach ($tr->childNodes as $cell) {
                if ($cell instanceof \DOMElement && ($cell->nodeName === 'th' || $cell->nodeName === 'td')) {
                    $content = $this->collapseInlineWhitespace($this->getInnerMarkdown($cell));
                    $rowCells[$colIndex] = $content;
                    $maxColLengths[$colIndex] = max($maxColLengths[$colIndex] ?? 0, mb_strlen($content));
                    $colIndex++;
                }
            }
            // Pad row if it has fewer cells than max columns determined so far
            $numCols = count($maxColLengths);
            while (count($rowCells) < $numCols) {
                $rowCells[] = '';
            }
            // Update maxColLengths if this row is longer
            foreach ($rowCells as $idx => $content) {
                $maxColLengths[$idx] = max($maxColLengths[$idx] ?? 0, mb_strlen($content));
            }
            $rows[] = $rowCells;
        }


        if (empty($headerCells) && empty($rows)) {
            return ''; // Empty table
        }

        // Ensure maxColLengths covers all columns found in header or rows
        $numCols = 0;
        if (!empty($headerCells))
            $numCols = count($headerCells);
        if (!empty($rows))
            $numCols = max($numCols, ...array_map('count', $rows)); // Get max row length

        for ($i = 0; $i < $numCols; $i++) {
            $maxColLengths[$i] = max(3, $maxColLengths[$i] ?? 0); // Ensure min length 3 for separator
        }


        $markdown = "\n\n";
        if (!empty($headerCells)) {
            // Pad header if needed
            while (count($headerCells) < $numCols)
                $headerCells[] = '';
            $markdown .= $this->formatTableRow($headerCells, $maxColLengths);
            $markdown .= $this->formatTableSeparator($maxColLengths);
        } else {
            // Add separator even without header if there are rows (GFM requires it)
            $markdown .= $this->formatTableSeparator($maxColLengths);
        }

        foreach ($rows as $row) {
            // Pad row if needed
            while (count($row) < $numCols)
                $row[] = '';
            $markdown .= $this->formatTableRow($row, $maxColLengths);
        }

        return rtrim($markdown) . "\n\n"; // Ensure trailing newline
    }

    /**
     * Helper method to extract header content from TH elements
     * This special method handles nested structure commonly found in table headers
     *
     * @param \DOMElement $cell The header cell element (usually TH)
     * @return string The extracted content with formatting preserved
     */
    private function extractHeaderContent(\DOMElement $cell): string
    {
        // First try normal inner markdown conversion
        $content = $this->collapseInlineWhitespace($this->getInnerMarkdown($cell));
        
        // If content is empty after processing, try more direct extraction
        if (empty(trim($content))) {
            // Look for specific nested structure: div > p > strong
            $divs = $cell->getElementsByTagName('div');
            if ($divs->length > 0) {
                foreach ($divs as $div) {
                    $paragraphs = $div->getElementsByTagName('p');
                    if ($paragraphs->length > 0) {
                        foreach ($paragraphs as $p) {
                            $strongs = $p->getElementsByTagName('strong');
                            if ($strongs->length > 0) {
                                return $this->strongDelimiter . trim($strongs->item(0)->textContent) . $this->strongDelimiter;
                            } else {
                                return trim($p->textContent);
                            }
                        }
                    } else {
                        // Just div with no paragraphs
                        $strongs = $div->getElementsByTagName('strong');
                        if ($strongs->length > 0) {
                            return $this->strongDelimiter . trim($strongs->item(0)->textContent) . $this->strongDelimiter;
                        } else {
                            return trim($div->textContent);
                        }
                    }
                }
            }
            
            // If we still don't have content, try other specific elements
            $strongs = $cell->getElementsByTagName('strong');
            if ($strongs->length > 0) {
                return $this->strongDelimiter . trim($strongs->item(0)->textContent) . $this->strongDelimiter;
            }
            
            // Last resort - get direct text content of the cell
            return trim($cell->textContent);
        }
        
        return $content;
    }

    // *** NEW METHOD ***
    /**
     * Converts an array of consecutive <a> link nodes into a Markdown table.
     *
     * @param array<int, \DOMNode> $linkNodes Array of <a> DOMNode nodes.
     * @return string Markdown table representation.
     */
    private function convertConsecutiveLinksToTable(array $linkNodes): string
    {
        if (empty($linkNodes)) {
            return '';
        }

        $cells = [];
        $maxColLengths = [];
        $validLinkIndex = 0; // Index for valid links only
        foreach ($linkNodes as $linkNode) {
            // Use convertLink to get the basic [text](url "title") format
            $cellContent = $this->convertLink($linkNode);

            // If convertLink returned empty (e.g., no href), skip this link
            if (empty($cellContent))
                continue;

            $cells[] = $cellContent;
            // Calculate length based on the final Markdown string
            $maxColLengths[$validLinkIndex] = max($maxColLengths[$validLinkIndex] ?? 0, mb_strlen($cellContent));
            $validLinkIndex++;
        }

        // If all links were invalid, return empty
        if (empty($cells)) {
            return '';
        }

        // Ensure all columns have a min length for the separator
        foreach ($maxColLengths as $i => $length) {
            $maxColLengths[$i] = max(3, $length);
        }

        // Create a single-row table
        $markdown = "\n\n";
        // No header row, no separator row, just the data row
        // $markdown .= $this->formatTableSeparator($maxColLengths); // Removed separator line
        $markdown .= $this->formatTableRow($cells, $maxColLengths);
        // formatTableRow adds a newline

        return $markdown . "\n"; // Add extra newline for block context after table
    }


    // *** ADJUSTED formatTableRow/Separator ***
    private function formatTableRow(array $cells, array $maxLengths): string
    {
        $row = '|';
        foreach ($cells as $i => $cell) {
            // Ensure index exists in maxLengths, default to cell length if not
            $maxLength = $maxLengths[$i] ?? mb_strlen($cell);
            // Calculate padding based on multibyte length
            $padding = str_repeat(' ', $maxLength - mb_strlen($cell));
            // Escape pipes within the cell content *before* adding padding
            $row .= ' ' . $this->escapeMarkdownTableCellContent($cell) . $padding . ' |';
        }
        return $row . "\n";
    }

    private function formatTableSeparator(array $maxLengths): string
    {
        $separator = '|';
        foreach ($maxLengths as $length) {
            // Ensure minimum length of 3 for separator dashes (GFM spec)
            $separator .= ' ' . str_repeat('-', max(3, $length)) . ' |';
        }
        return $separator . "\n";
    }

    // *** ORIGINAL getCleanHtmlTable ***
    private function getCleanHtmlTable(\DOMElement $node): string
    {
        $cleanTable = $this->dom->createElement('table');
        // Copy attributes from original table? Maybe border="1"? No, keep it clean.

        // Handle caption
        $caption = $node->getElementsByTagName('caption')->item(0);
        if ($caption) {
            $cleanTable->appendChild($caption->cloneNode(true));
        }

        // Handle thead, tbody, tfoot
        foreach (['thead', 'tbody', 'tfoot'] as $sectionName) {
            $sections = $node->getElementsByTagName($sectionName);
            foreach ($sections as $section) {
                // Ensure the section is a direct child of the table? No, getElementsByTagName is fine.
                $cleanSection = $this->dom->createElement($sectionName);
                foreach ($section->childNodes as $tr) {
                    if ($tr instanceof \DOMElement && $tr->nodeName === 'tr') {
                        $cleanSection->appendChild($this->createCleanTableRow($tr));
                    }
                }
                if ($cleanSection->hasChildNodes()) {
                    $cleanTable->appendChild($cleanSection);
                }
            }
        }

        // Handle TRs that are direct children of TABLE (if no thead/tbody/tfoot)
        if (!$cleanTable->getElementsByTagName('tbody')->length && !$cleanTable->getElementsByTagName('thead')->length && !$cleanTable->getElementsByTagName('tfoot')->length) {
            foreach ($node->childNodes as $tr) {
                if ($tr instanceof \DOMElement && $tr->nodeName === 'tr') {
                    $cleanTable->appendChild($this->createCleanTableRow($tr));
                }
            }
        }


        // Use saveHTML with the specific node to avoid XML declaration etc.
        $html = $this->dom->saveHTML($cleanTable);
        // Ensure proper spacing around the table
        return "\n\n" . trim($html) . "\n\n";
    }

    // *** ORIGINAL createCleanTableRow ***
    private function createCleanTableRow(\DOMElement $tr): \DOMElement
    {
        $newTr = $this->dom->createElement('tr');
        foreach ($tr->childNodes as $cell) {
            if ($cell instanceof \DOMElement && in_array($cell->nodeName, ['td', 'th'])) {
                $newCell = $this->dom->createElement($cell->nodeName);
                // Copy common attributes like colspan and rowspan
                if ($cell->hasAttribute('colspan')) {
                    $newCell->setAttribute('colspan', $cell->getAttribute('colspan'));
                }
                if ($cell->hasAttribute('rowspan')) {
                    $newCell->setAttribute('rowspan', $cell->getAttribute('rowspan'));
                }
                // Copy alignment attributes? Maybe just style? No, keep it simple.

                // Copy content recursively
                foreach ($cell->childNodes as $content) {
                    // Clone node deeply to get all descendants
                    $newCell->appendChild($content->cloneNode(true));
                }
                $newTr->appendChild($newCell);
            }
        }
        return $newTr;
    }

    // *** ORIGINAL wrapWithDelimiter ***
    private function wrapWithDelimiter(string $text, string $delimiter): string
    {
        // Avoid adding delimiters if text is only whitespace
        if (trim($text) === '') {
            return $text;
        }
        return $delimiter . trim($text) . $delimiter;
    }

    // *** ORIGINAL escapeMarkdownChars ***
    private function escapeMarkdownChars(string $text): string
    {
        if (!$this->escapeMode) {
            return $text;
        }
        // Escape backslashes first, then other chars
        $text = str_replace('\\', '\\\\', $text);
        $chars = ['`', '*', '_', '{', '}', '[', ']', '(', ')', '#', '+', '-', '.', '!', '|'];
        foreach ($chars as $char) {
            $text = str_replace($char, '\\' . $char, $text);
        }
        return $text;
    }

    // *** ORIGINAL escapeMarkdownTableCellContent ***
    private function escapeMarkdownTableCellContent(string $text): string
    {
        // Escape pipe character for table cells
        // Also escape backslash before pipe if using regex replace later? No, simple replace is fine.
        return str_replace('|', '\\|', $text);
    }

    // *** ORIGINAL convertDefinitionList ***
    private function convertDefinitionList(\DOMElement $node): string
    {
        $markdown = '';
        $dtContent = null; // Store dt content temporarily

        foreach ($node->childNodes as $item) {
            if (!($item instanceof \DOMElement))
                continue; // Skip non-element nodes

            if ($item->nodeName === 'dt') {
                // If there was a previous dt without a dd, output it as a simple line
                if ($dtContent !== null) {
                    $markdown .= $dtContent . "\n";
                }
                $dtContent = $this->getInnerMarkdown($item); // Get content for the current dt
            } elseif ($item->nodeName === 'dd') {
                $ddContent = $this->getInnerMarkdown($item);
                if ($dtContent !== null) {
                    // Output stored dt and current dd in definition list format
                    $markdown .= "\n" . $dtContent . "\n:   " . $ddContent . "\n";
                    $dtContent = null; // Reset dt content
                } else {
                    // Handle dd without preceding dt (output as indented block)
                    $markdown .= "\n:   " . $ddContent . "\n";
                }
            }
        }
        // Output any remaining dt content at the end
        if ($dtContent !== null) {
            $markdown .= "\n" . $dtContent . "\n";
        }

        // Add block context if markdown was generated
        return !empty($markdown) ? "\n" . trim($markdown) . "\n\n" : '';
    }


    // *** ORIGINAL normalizeWhitespace ***
    private function normalizeWhitespace(string $text): string
    {
        // 1. Replace CRLF with LF
        $text = str_replace("\r\n", "\n", $text);
        // 2. Replace multiple consecutive newlines with a maximum of two
        $text = preg_replace('/\n{3,}/', "\n\n", $text);
        // 3. Trim trailing spaces/tabs from each line
        $text = preg_replace('/[ \t]+$/m', '', $text);
        // 4. Trim leading/trailing whitespace from the whole string
        $text = trim($text);
        // 5. Ensure a single newline at the very end (optional, common practice)
        // if (!empty($text)) {
        //     $text .= "\n";
        // }
        return $text;
    }


    // *** SETTER METHODS (Original) ***
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
        // Basic validation for common markers
        if (!in_array($marker, ['-', '*', '+'])) {
            error_log("HtmlToMarkdownConverter: Invalid bullet list marker '{$marker}'. Using '-'.");
            $marker = '-';
        }
        $this->bulletListMarker = $marker;
        return $this;
    }

    public function setCodeBlockFence(string $fence): self
    {
        if (strlen($fence) < 3 || strpos($fence, '`') !== 0) {
            error_log("HtmlToMarkdownConverter: Invalid code block fence '{$fence}'. Using '```'.");
            $fence = '```';
        }
        $this->codeBlockFence = $fence;
        return $this;
    }

    public function setHorizontalRule(string $rule): self
    {
        // Basic validation
        if (!preg_match('/^(\* *){3,}$|^(- *){3,}$|^(_ *){3,}$/', $rule)) {
            error_log("HtmlToMarkdownConverter: Invalid horizontal rule '{$rule}'. Using '* * *'.");
            $rule = '* * *';
        }
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

    // *** LIST CONVERSION METHODS (Original/Restored) ***

    /**
     * Main function to convert a DOMElement (UL or OL) to Markdown.
     * Adds block context (newlines).
     *
     * @param \DOMElement $element The input list element (ul or ol).
     * @return string Markdown representation of the list.
     * @throws \InvalidArgumentException If the input element is not ul or ol.
     */
    private function convertListToMarkdown(\DOMElement $element): string
    {
        $tagName = strtolower($element->tagName);
        if ($tagName !== 'ul' && $tagName !== 'ol') {
            throw new \InvalidArgumentException('Input element must be <ul> or <ol>. Found: <' . $element->tagName . '>');
        }
        // Process list and add surrounding newlines for block context
        $listMarkdown = trim($this->processList($element, 0));
        return !empty($listMarkdown) ? "\n\n" . $listMarkdown . "\n\n" : '';
    }

    /**
     * Recursively processes a list element (ul or ol) and its items (li).
     *
     * @param \DOMElement $listElement The list element (ul or ol).
     * @param int $level The current nesting level (for indentation).
     * @return string Markdown for this list and its sub-items.
     */
    private function processList(\DOMElement $listElement, int $level): string
    {
        $markdown = '';
        $itemCounter = 1; // Counter for ordered lists (ol)
        $isOrdered = strtolower($listElement->tagName) === 'ol';
        $startAttribute = $listElement->getAttribute('start');
        if ($isOrdered && ctype_digit($startAttribute) && $startAttribute > 1) {
            $itemCounter = (int) $startAttribute;
        }

        // Indentation string for items in this list (4 spaces per level typical)
        $indent = str_repeat('    ', $level);
        $indentNextLevel = str_repeat('    ', $level + 1);

        // Iterates through all direct child nodes of the list element
        foreach ($listElement->childNodes as $childNode) {
            // We are only interested in <li> elements directly under this list
            if ($childNode instanceof \DOMElement && strtolower($childNode->tagName) === 'li') {
                // Determine the marker for the list item
                $marker = $isOrdered ? ($itemCounter++) . '.' : $this->bulletListMarker;

                // Extracts the content markdown and any nested list markdown from the <li>
                list($itemContentMarkdown, $nestedListMarkdown) = $this->extractLiData($childNode, $level);

                // Format the current list item line
                $trimmedItemContent = trim($itemContentMarkdown);
                // Handle potential multiple lines within the item content
                $lines = preg_split('/\n/', $trimmedItemContent, -1, PREG_SPLIT_NO_EMPTY);
                $firstLine = $lines[0] ?? ''; // Handle empty items

                // Add the first line of the item
                $markdown .= $indent . $marker . ' ' . $firstLine . "\n";

                // Add subsequent lines of the same item, indented correctly
                $subsequentIndent = $indent . str_repeat(' ', strlen($marker) + 1); // Align with text after marker
                for ($i = 1; $i < count($lines); $i++) {
                    $markdown .= $subsequentIndent . $lines[$i] . "\n";
                }

                // Append the Markdown for any nested list found within this <li>
                if (!empty($nestedListMarkdown)) {
                    $nestedListMarkdown = preg_replace("/^\n/", "\n{$indentNextLevel}", $nestedListMarkdown);
                    // processList returns trimmed markdown, add necessary indentation/spacing
                    $markdown .= $nestedListMarkdown . "\n"; // Nested list already includes indentation
                }
            }
            // Ignore other direct children like text nodes containing only whitespace
        }
        // Return the markdown for this level, let the caller handle trimming/spacing
        return $markdown;
    }


    /**
     * Extracts the markdown content and any nested list markdown from an <li> element.
     *
     * @param \DOMElement $liElement The <li> element.
     * @param int $level The current nesting level of the parent list.
     * @return array An array containing [itemContentMarkdown, nestedListMarkdown].
     */
    private function extractLiData(\DOMElement $liElement, int $level): array
    {
        $itemContentMarkdown = ''; // Store markdown of non-list content within <li>
        $nestedListMarkdown = '';  // Store markdown of direct ul/ol children

        // Iterate through direct children of <li>
        foreach ($liElement->childNodes as $node) {
            if ($node instanceof \DOMElement && (strtolower($node->tagName) === 'ul' || strtolower($node->tagName) === 'ol')) {
                // Process nested lists separately, adding necessary spacing
                $nestedListMarkdown .= "\n" . $this->processList($node, $level + 1); // Add newline before nested list
            } else {
                // Convert other child nodes (text, p, span, a, etc.) to markdown
                // Handle paragraphs inside list items: don't add extra \n\n
                if ($node instanceof \DOMElement && strtolower($node->tagName) === 'p') {
                    // Convert paragraph content, trim potential surrounding newlines from its conversion
                    $itemContentMarkdown .= trim($this->getInnerMarkdown($node));
                    // Add a single newline to simulate paragraph break within list item if needed
                    $itemContentMarkdown .= "\n";
                } else {
                    $itemContentMarkdown .= $this->convertNode($node);
                }
            }
        }

        // Clean up the item content:
        // - Trim leading/trailing whitespace and newlines.
        // - Collapse multiple newlines/spaces within the item content? No, preserve intended breaks.
        $cleanedItemText = trim($itemContentMarkdown);


        // Clean up nested list markdown
        $cleanedNestedListMarkdown = trim($nestedListMarkdown);
        // Ensure nested list starts on a new line relative to item text if item text exists
        if (!empty($cleanedNestedListMarkdown) && !empty($cleanedItemText)) {
            $cleanedNestedListMarkdown = "\n" . $cleanedNestedListMarkdown;
        }


        return [$cleanedItemText, $cleanedNestedListMarkdown];
    }


    /**
     * Processes a <details> element. Currently converts to bold summary + content.
     * This might need refinement based on desired Markdown output for <details>.
     * NOTE: This method is likely NOT called by the current list logic.
     *
     * @param \DOMElement $detailsElement The <details> element.
     * @param int $level The current nesting level.
     * @return array An array containing [summaryText, nestedListMarkdown].
     */
    /** @phpstan-ignore-next-line */
    private function processDetailsElement(\DOMElement $detailsElement, int $level): array
    {
        // This method seems unused by the refactored list logic.
        // Keeping the structure but it might need removal or integration elsewhere if <details> support is needed.
        $summaryText = '';
        $nestedListMarkdown = ''; // Only lists directly under details

        foreach ($detailsElement->childNodes as $node) {
            if ($node instanceof \DOMElement) {
                $tagName = strtolower($node->tagName);
                if ($tagName === 'summary') {
                    $summaryText = $this->getInnerMarkdown($node); // Just get inner markdown of summary
                } elseif ($tagName === 'ul' || $tagName === 'ol') {
                    $nestedListMarkdown .= $this->processList($node, $level + 1); // Process lists directly under details
                }
                // Ignore other elements within details for this specific extraction logic
            }
        }
        $cleanedSummaryText = trim(preg_replace('/\s+/u', ' ', $summaryText));
        // Return format expected by original caller (if any)
        return [$cleanedSummaryText, $nestedListMarkdown];
    }


    /**
     * Recursively extracts visible text content from a node and its children.
     * Primarily used by formatLink. Collapses whitespace.
     *
     * @param \DOMNode $node The starting node.
     * @return string Extracted and cleaned text content.
     */
    private function extractVisibleTextRecursive(\DOMNode $node): string
    {
        // Base case: Text node
        if ($node instanceof \DOMText) {
            return $node->nodeValue; // Return raw value, collapse later
        }

        // Base case: Not an element or is an ignored element type
        if (!($node instanceof \DOMElement)) {
            return ''; // Ignore comments, etc.
        }
        $tagName = strtolower($node->tagName);
        // Ignore elements that don't contribute to visible text or are handled specially
        if (in_array($tagName, ['ul', 'ol', 'svg', 'script', 'style', 'head', 'meta', 'link', 'iframe', 'frame', 'br', 'hr', 'pre', 'code'])) {
            return '';
        }

        // Recursive step: Process children
        $text = '';
        foreach ($node->childNodes as $child) {
            $text .= $this->extractVisibleTextRecursive($child);
        }

        // Collapse whitespace only at the end of recursion for a node?
        // No, let the caller (formatLink) handle final collapse. Return combined text.
        return $text;
    }

    /**
     * Formats a DOMElement representing an <a> tag into a Markdown link.
     * Used by list processing and potentially single link conversion.
     *
     * @param \DOMElement $aElement The <a> element.
     * @return string Markdown link string "[text](url)". Returns inner text if href is missing.
     * @phpstan-ignore-next-line
     */
    private function formatLink(\DOMElement $aElement): string
    {
        $href = $aElement->getAttribute('href');

        // Extract raw text content recursively first
        $linkText = $this->extractVisibleTextRecursive($aElement);
        // Collapse whitespace in the extracted text
        $cleanedLinkText = $this->collapseInlineWhitespace($linkText);

        if (empty($href)) {
            // If href is missing, return the cleaned text content, escaped
            return $this->escapeMarkdownChars($cleanedLinkText);
        }

        // If link text is empty after collapsing, use the href as text
        if (empty($cleanedLinkText)) {
            $cleanedLinkText = $href;
        }

        $title = $aElement->getAttribute('title');

        // Build the markdown link - escape text and title
        $markdown = '[' . $this->escapeMarkdownChars($cleanedLinkText) . '](' . $href;
        if (!empty($title)) {
            $markdown .= ' "' . $this->escapeMarkdownChars($title) . '"'; // Add escaped title if present
        }
        $markdown .= ')';
        return $markdown;
    }

    // Deduplication helper methods removed
}
