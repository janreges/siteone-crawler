<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Export\Utils\HtmlToMarkdownConverter;
use Crawler\Export\Utils\OfflineUrlConverter;
use Crawler\Export\Utils\TargetDomainRelation;
use Crawler\FoundUrl;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\ParsedUrl;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use Exception;

class MarkdownExporter extends BaseExporter implements Exporter
{

    const GROUP_MARKDOWN_EXPORTER = 'markdown-exporter';

    private static $contentTypesThatRequireChanges = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_REDIRECT
    ];

    /**
     * Directory where markdown files will be stored. If not set, markdown export is disabled.
     * @var string|null
     */
    protected ?string $markdownExportDirectory = null;

    /**
     * Path where combined markdown file will be stored, if set, all exported markdown files will be combined into a single file.
     * @var string|null
     */
    protected ?string $markdownExportSingleFile = null;

    /**
     * Do not export and show images in markdown files. Images are enabled by default.
     * @var bool
     */
    protected bool $markdownDisableImages = false;

    /**
     * Do not export and link files other than HTML/CSS/JS/fonts/images - eg. PDF, ZIP, etc.
     * @var bool
     */
    protected bool $markdownDisableFiles = false;

    /**
     * Remove links and images from the combined single markdown file. 
     * Useful for AI tools that don't need these elements.
     * @var bool
     */
    protected bool $markdownRemoveLinksAndImagesFromSingleFile = false;

    /**
     * Exclude some page content (DOM elements) from markdown export defined by CSS selectors like 'header', 'footer', '.header', '#footer', etc.
     * @var string[]
     */
    protected array $markdownExcludeSelector = [];

    /**
     * For debug - when filled it will activate debug mode and store only URLs which match one of these regexes
     * @var string[]
     */
    protected array $markdownExportStoreOnlyUrlRegex = [];

    /**
     * Ignore issues with storing files and continue saving other files. Useful in case of too long file names (depends on OS, FS, base directory, etc.)
     * @var bool
     */
    protected bool $markdownIgnoreStoreFileError = false;

    /**
     * Replace HTML/JS/CSS content with `xxx -> bbb` or regexp in PREG format: `/card[0-9]/ -> card`
     *
     * @var string[]
     */
    protected array $markdownReplaceContent = [];

    /**
     * Instead of using a short hash instead of a query string in the filename, just replace some characters.
     * You can use a regular expression. E.g. '/([^&]+)=([^&]*)(&|$)/' -> '$1-$2_'
     *
     * @var string[]
     */
    protected array $markdownReplaceQueryString = [];

    /**
     * Move all content before the main H1 heading (typically the header with the menu) to the end of the markdown
     *
     * @var bool
     */
    protected bool $markdownMoveContentBeforeH1ToEnd = false;

    /**
     * Exporter is activated when either --markdown-export-dir or --markdown-export-single-file is set
     * @return bool
     */
    public function shouldBeActivated(): bool
    {
        ini_set('pcre.backtrack_limit', '100000000');
        ini_set('pcre.recursion_limit', '100000000');
        $this->markdownExportDirectory = $this->markdownExportDirectory ? rtrim($this->markdownExportDirectory, '/') : null;
        return $this->markdownExportDirectory !== null || $this->markdownExportSingleFile !== null;
    }

    /**
     * Export all visited URLs to directory with markdown version of the website
     * @return void
     * @throws Exception
     */
    public function export(): void
    {
        $startTime = microtime(true);
        $visitedUrls = $this->status->getVisitedUrls();

        // user-defined markdownReplaceQueryString will deactivate replacing query string with hash and use custom replacement
        OfflineUrlConverter::setReplaceQueryString($this->markdownReplaceQueryString);

        // store only URLs with relevant content types
        $validContentTypes = [Crawler::CONTENT_TYPE_ID_HTML, Crawler::CONTENT_TYPE_ID_REDIRECT];
        if (!$this->markdownDisableImages) {
            $validContentTypes[] = Crawler::CONTENT_TYPE_ID_IMAGE;
        }
        if (!$this->markdownDisableFiles) {
            $validContentTypes[] = Crawler::CONTENT_TYPE_ID_DOCUMENT;
        }

        // filter only relevant URLs with OK status codes
        $exportedUrls = array_filter($visitedUrls, function (VisitedUrl $visitedUrl) use ($validContentTypes) {
            // do not store images if they are not from <img src="..."> (e.g. background-image in CSS or alternative image sources in <picture>)
            if ($visitedUrl->isImage() && !in_array($visitedUrl->sourceAttr, [FoundUrl::SOURCE_IMG_SRC, FoundUrl::SOURCE_A_HREF])) {
                return false;
            }

            return $visitedUrl->statusCode === 200 && in_array($visitedUrl->contentType, $validContentTypes);
        });
        /** @var VisitedUrl[] $exportedUrls */

        // store all allowed URLs
        try {
            foreach ($exportedUrls as $exportedUrl) {
                if ($this->isValidUrl($exportedUrl->url) && $this->shouldBeUrlStored($exportedUrl)) {
                    $this->storeFile($exportedUrl);
                }
            }
        } catch (Exception $e) {
            throw new Exception(__METHOD__ . ': ' . $e->getMessage());
        }

        // add info to summary
        $this->status->addInfoToSummary(
            'markdown-generated',
            sprintf(
                "Markdown content generated to '%s' and took %s",
                Utils::getOutputFormattedPath($this->markdownExportDirectory),
                Utils::getFormattedDuration(microtime(true) - $startTime)
            )
        );
        
        // combine markdown files to a single file if requested
        if ($this->markdownExportSingleFile !== null && $this->markdownExportDirectory !== null) {
            try {
                $combineStartTime = microtime(true);
                $combiner = new \Crawler\Export\Utils\MarkdownSiteAggregator($this->crawler->getCoreOptions()->url);
                $combinedMarkdown = $combiner->combineDirectory(
                    $this->markdownExportDirectory,
                    $this->markdownRemoveLinksAndImagesFromSingleFile
                );
                
                // ensure directory exists
                $singleFileDir = dirname($this->markdownExportSingleFile);
                if (!is_dir($singleFileDir)) {
                    if (!mkdir($singleFileDir, 0777, true)) {
                        throw new Exception("Cannot create directory for single markdown file: '$singleFileDir'");
                    }
                }
                
                // write the combined file
                if (file_put_contents($this->markdownExportSingleFile, $combinedMarkdown) === false) {
                    throw new Exception("Cannot write single markdown file: '{$this->markdownExportSingleFile}'");
                }
                
                $this->status->addInfoToSummary(
                    'markdown-combined',
                    sprintf(
                        "Markdown files combined into single file '%s' and took %s",
                        Utils::getOutputFormattedPath($this->markdownExportSingleFile),
                        Utils::getFormattedDuration(microtime(true) - $combineStartTime)
                    )
                );
            } catch (Exception $e) {
                $this->status->addCriticalToSummary('markdown-combine-error', "Error combining markdown files: " . $e->getMessage());
            }
        }
    }

    /**
     * Store file of visited URL to offline export directory and apply all required changes
     *
     * @param VisitedUrl $visitedUrl
     * @return void
     * @throws Exception
     */
    private function storeFile(VisitedUrl $visitedUrl): void
    {
        $content = $this->status->getUrlBody($visitedUrl->uqId);

        // apply required changes through all content processors
        if (in_array($visitedUrl->contentType, self::$contentTypesThatRequireChanges)) {
            $this->crawler->getContentProcessorManager()->applyContentChangesForOfflineVersion(
                $content,
                $visitedUrl->contentType,
                ParsedUrl::parse($visitedUrl->url),
                true
            );

            // apply custom content replacements
            if ($content && $this->markdownReplaceContent) {
                foreach ($this->markdownReplaceContent as $replace) {
                    $parts = explode('->', $replace);
                    $replaceFrom = trim($parts[0]);
                    $replaceTo = trim($parts[1] ?? '');
                    $isRegex = preg_match('/^([\/#~%]).*\1[a-z]*$/i', $replaceFrom);
                    if ($isRegex) {
                        $content = preg_replace($replaceFrom, $replaceTo, $content);
                    } else {
                        $content = str_replace($replaceFrom, $replaceTo, $content);
                    }
                }
            }
        }

        // sanitize and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
        // same logic is in method convertUrlToRelative()
        $storeFilePath = sprintf('%s/%s',
            $this->markdownExportDirectory,
            OfflineUrlConverter::sanitizeFilePath($this->getRelativeFilePathForFileByUrl($visitedUrl), false)
        );

        $directoryPath = dirname($storeFilePath);
        if (!is_dir($directoryPath)) {
            if (!mkdir($directoryPath, 0777, true)) {
                throw new Exception("Cannot create directory '$directoryPath'");
            }
        }

        $saveFile = true;
        clearstatcache(true);

        // do not overwrite existing file if initial request was HTTPS and this request is HTTP, otherwise referenced
        // http://your.domain.tld/ will override wanted HTTPS page with small HTML file with meta redirect
        if (is_file($storeFilePath)) {
            if (!$visitedUrl->isHttps() && $this->crawler->getInitialParsedUrl()->isHttps()) {
                $saveFile = false;
                $message = "File '$storeFilePath' already exists and will not be overwritten because initial request was HTTPS and this request is HTTP: " . $visitedUrl->url;
                $this->output->addNotice($message);
                $this->status->addNoticeToSummary('markdown-exporter-store-file-ignored', $message);
                return;
            }
        }

        if (@file_put_contents($storeFilePath, $content) === false) {
            // throw exception if file has extension (handle edge-cases as <img src="/icon/hash/"> and response is SVG)
            $exceptionOnError = preg_match('/\.[a-z0-9\-]{1,15}$/i', $storeFilePath) === 1;
            // AND if the exception should NOT be ignored
            if ($exceptionOnError && !$this->markdownIgnoreStoreFileError) {
                throw new Exception("Cannot store file '$storeFilePath'.");
            } else {
                $message = "Cannot store file '$storeFilePath' (undefined extension). Original URL: {$visitedUrl->url}";
                $this->output->addNotice($message);
                $this->status->addNoticeToSummary('markdown-exporter-store-file-error', $message);
                return;
            }
        }

        // convert *.html to *.md and remove *.html file
        if (str_ends_with($storeFilePath, '.html')) {
            $storeMdFilePath = substr($storeFilePath, 0, -5) . '.md';

            $converter = new HtmlToMarkdownConverter(file_get_contents($storeFilePath), $this->markdownExcludeSelector);
            $markdown = $converter->getMarkdown();
            @file_put_contents($storeMdFilePath, $markdown);
            @unlink($storeFilePath);

            if (!is_file($storeMdFilePath)) {
                $message = "Cannot convert HTML file to Markdown file '$storeMdFilePath'. Original URL: {$visitedUrl->url}";
                $this->output->addNotice($message);
                $this->status->addNoticeToSummary('markdown-exporter-store-file-error', $message);
                return;
            }

            $this->normalizeMarkdownFile($storeMdFilePath);
        }
    }

    /**
     * Normalize markdown file after conversion from HTML:
     *  - replace *.html links to *.md in saved *.md file
     *  - remove images if disabled
     *  - remove files if disabled
     *  - lot of other small fixes
     *
     * @param string $mdFilePath
     * @return void
     */
    private function normalizeMarkdownFile(string $mdFilePath): void
    {
        $ignoreRegexes = $this->crawler->getCoreOptions()->ignoreRegex;
        $mdContent = file_get_contents($mdFilePath);

        // replace .html with .md in links, but respect ignore patterns
        $mdContent = preg_replace_callback(
            '/\[([^\]]*)\]\(([^)]+)\)/',
            function ($matches) use ($ignoreRegexes) {
                $linkText = $matches[1];
                $url = $matches[2];

                // check if URL matches any ignore pattern
                if ($ignoreRegexes) {
                    foreach ($ignoreRegexes as $ignoreRegex) {
                        if (preg_match($ignoreRegex, $url)) {
                            return $matches[0]; // Return link unchanged
                        }
                    }
                }

                // no ignore pattern matched - replace .html with .md
                $url = preg_replace(['/\.html/', '/\.html#/'], ['.md', '.md#'], $url);
                return "[$linkText]($url)";
            },
            $mdContent
        );

        if ($this->markdownDisableImages) {
            // replace image in anchor text, like in [![logo by @foobar](data:image/gif;base64,fooo= "logo by @foobar")](index.html)
            $mdContent = preg_replace('/\[!\[[^\]]*\]\([^\)]*\)\]\([^\)]*\)/', '', $mdContent);

            // replace standard image
            $mdContent = preg_replace('/!\[.*\]\(.*\)/', '', $mdContent);
        }

        if ($this->markdownDisableFiles) {
            // replace links to files except allowed extensions and those matching ignore patterns
            $mdContent = preg_replace_callback(
                '/\[([^\]]+)\]\((?!https?:\/\/)([^)]+)\.([a-z0-9]{1,5})\)/i',
                function ($matches) use ($ignoreRegexes) {
                    $linkText = $matches[1];
                    $fullUrl = $matches[2] . '.' . $matches[3];

                    // keep if it matches ignore patterns
                    if ($ignoreRegexes) {
                        foreach ($ignoreRegexes as $ignoreRegex) {
                            if (preg_match($ignoreRegex, $fullUrl)) {
                                return $matches[0];
                            }
                        }
                    }

                    // keep if it's an allowed extension
                    if (in_array(strtolower($matches[3]), ['md', 'jpg', 'png', 'gif', 'webp', 'avif'])) {
                        return $matches[0];
                    }

                    return ''; // remove link
                },
                $mdContent
            );

            $mdContent = str_replace('  ', ' ', $mdContent);
        }

        // remove empty links
        $mdContent = preg_replace('/\[[^\]]*\]\(\)/', '', $mdContent);

        // remove empty lines in code blocks (multi-line commands)
        $mdContent = str_replace(
            ["\\\n\n  -"],
            ["\\\n  -"],
            $mdContent
        );

        // remove empty lines in the beginning of code blocks
        $mdContent = preg_replace('/```\n{2,}/', "```\n", $mdContent);

        // apply additional fixes
        $mdContent = $this->removeEmptyLinesInLists($mdContent);
        $mdContent = $this->moveContentBeforeMainHeadingToTheEnd($mdContent);
        $mdContent = $this->fixMultilineImages($mdContent);
        $mdContent = $this->detectAndSetCodeLanguage($mdContent);

        // add "`" around "--param" inside tables
        $mdContent = preg_replace('/\| -{1,2}([a-z0-9][a-z0-9-]*) \|/i', '| `--$1` |', $mdContent);

        // remove 3+ empty lines to 2 empty lines
        $mdContent = preg_replace('/\n{3,}/', "\n\n", $mdContent);

        // trim -#*
        $mdContent = trim($mdContent, "\n\t -#*");
        
        // fix excessive whitespace issues
        $mdContent = $this->removeExcessiveWhitespace($mdContent);

        file_put_contents($mdFilePath, $mdContent);
    }

    /**
     * Removes excessive whitespace from markdown content while preserving
     * necessary indentation for nested structures like lists and code blocks
     *
     * @param string $md
     * @return string
     */
    private function removeExcessiveWhitespace(string $md): string
    {
        $lines = explode("\n", $md);
        $result = [];
        $inCodeBlock = false;
        $lastLineWasEmpty = false;

        foreach ($lines as $line) {
            // Check if we're entering or leaving a code block
            if (preg_match('/^```/', $line)) {
                $inCodeBlock = !$inCodeBlock;
                $result[] = $line;
                $lastLineWasEmpty = false;
                continue;
            }

            // Don't modify lines inside code blocks
            if ($inCodeBlock) {
                $result[] = $line;
                $lastLineWasEmpty = false;
                continue;
            }

            // Check if this is a list item line (preserve indentation)
            $isListItem = preg_match('/^(\s*)([-*+]|\d+\.)\s/', $line);
            
            // Check if line is part of a table
            $isTableRow = preg_match('/^\s*\|.*\|\s*$/', $line);
            
            // Check if line is a heading
            $isHeading = preg_match('/^#+\s+/', $line);
            
            // Line is completely empty
            if (trim($line) === '') {
                // Avoid multiple consecutive empty lines
                if (!$lastLineWasEmpty) {
                    $result[] = '';
                    $lastLineWasEmpty = true;
                }
                continue;
            }
            
            if ($isListItem || $isTableRow || $isHeading) {
                // For list items, tables and headings - preserve their structure
                $result[] = $line;
            } else {
                // For regular text - trim excess spaces, but maintain paragraph structure
                $trimmedLine = preg_replace('/\s+/', ' ', trim($line));
                if (!empty($trimmedLine)) {
                    $result[] = $trimmedLine;
                }
            }
            $lastLineWasEmpty = false;
        }
        
        // Join lines back together
        $content = implode("\n", $result);
        
        // Fix multiple spaces in text (but not in code blocks or tables)
        $content = preg_replace_callback(
            '/```.*?```|`.*?`|\|.*?\||([^`\|]+)/s', 
            function($matches) {
                if (isset($matches[1])) {
                    // Only replace multiple spaces in regular text
                    return preg_replace('/[ ]{2,}/', ' ', $matches[1]);
                }
                // Return code blocks, inline code and table cells unchanged
                return $matches[0];
            },
            $content
        );
        
        // Remove spaces at the end of lines
        $content = preg_replace('/[ \t]+$/m', '', $content);
        
        return $content;
    }

    /**
     * Removes empty lines between list items in markdown content while preserving list structure
     * Works with both ordered and unordered lists of any nesting level
     *
     * @param string $md
     * @return string
     */
    private function removeEmptyLinesInLists(string $md): string
    {
        $lines = explode("\n", $md);
        $result = [];
        $inList = false;
        $lastLineEmpty = false;
        $lastIndentLevel = 0;
        $lastNonEmptyLine = '';

        foreach ($lines as $line) {
            $trimmedLine = trim($line);
            $isEmptyLine = $trimmedLine === '';

            if (preg_match('/^[ ]{0,3}[-*+][ ]|^[ ]{0,3}\d+\.[ ]|^[ ]{2,}[-*+][ ]/', $line)) {
                $inList = true;

                if ($lastLineEmpty) {
                    // Only add an empty line between list items of different nesting levels
                    preg_match('/^[ ]*/', $line, $matches);
                    $currentIndent = strlen($matches[0]);
                    
                    if (abs($currentIndent - $lastIndentLevel) > 2) {
                        // Different nesting level, keep the empty line
                    } else {
                        // Same nesting level, remove the empty line
                        array_pop($result);
                    }
                }

                $result[] = $line;
                $lastLineEmpty = false;
                $lastNonEmptyLine = $line;

                preg_match('/^[ ]*/', $line, $matches);
                $lastIndentLevel = strlen($matches[0]);
            } elseif ($isEmptyLine) {
                if ($inList) {
                    $lastLineEmpty = true;
                    $result[] = $line;
                } else {
                    $result[] = $line;
                    $lastLineEmpty = true;
                }
            } else {
                preg_match('/^[ ]*/', $line, $matches);
                $currentIndent = strlen($matches[0]);

                if ($inList && $currentIndent < $lastIndentLevel) {
                    $inList = false;
                }

                $result[] = $line;
                $lastLineEmpty = false;
                $lastNonEmptyLine = $line;
                $lastIndentLevel = $currentIndent;
            }
        }

        return implode("\n", $result);
    }

    /**
     * Find the first occurrence of the main heading (<h1>, if it does not exist <h2> or <h3>) and
     * all content before it, move to the end in the section below the "---"
     *
     * @param string $md
     * @return string
     */
    private function moveContentBeforeMainHeadingToTheEnd(string $md): string
    {
        if (!$this->markdownMoveContentBeforeH1ToEnd) {
            return $md;
        }

        $headings = [];

        // ATX headings (e.g. "# Title")
        if (preg_match_all('/^(#{1,6})\s.*$/m', $md, $atxMatches, PREG_OFFSET_CAPTURE)) {
            foreach ($atxMatches[1] as $key => $match) {
                $level = strlen($match[0]);
                $offset = $atxMatches[0][$key][1];
                $headings[] = ['offset' => $offset, 'level' => $level];
            }
        }

        // Setext headings, e.g.:
        // Title
        // =====
        // or
        // Subtitle
        // ------
        if (preg_match_all('/^(?!\s*$)(.+?)\n(=+|-+)\s*$/m', $md, $setextMatches, PREG_OFFSET_CAPTURE)) {
            foreach ($setextMatches[2] as $key => $match) {
                $underline = $match[0];
                $offset = $setextMatches[1][$key][1]; // offset of the text line
                $level = ($underline[0] === '=') ? 1 : 2;
                $headings[] = ['offset' => $offset, 'level' => $level];
            }
        }

        if (empty($headings)) {
            return $md; // No headings found
        }

        // Find the highest level (lowest number) among headings
        $minLevel = min(array_column($headings, 'level'));
        $candidates = array_filter($headings, function ($h) use ($minLevel) {
            return $h['level'] === $minLevel;
        });

        // Choose the candidate with the smallest offset (first occurrence)
        $mainHeading = array_reduce($candidates, function ($carry, $item) {
            return ($carry === null || $item['offset'] < $carry['offset']) ? $item : $carry;
        });

        if (!$mainHeading) {
            return $md;
        }

        $headingPosition = $mainHeading['offset'];
        $contentBefore = substr($md, 0, $headingPosition);
        $contentAfter = substr($md, $headingPosition);

        if (trim($contentBefore) === '') {
            return $md;
        }

        return trim($contentAfter) . "\n\n---\n\n" . trim($contentBefore);
    }

    /**
     * Fixes multi-line image and link definitions in markdown to be on single line
     * Specifically handles cases where markdown image/link syntax is split across multiple lines
     *
     * @param string $md
     * @return string
     */
    private function fixMultilineImages(string $md): string
    {
        $md = str_replace(
            ["[\n![", ")\n]("],
            ["[![", ")]("],
            $md
        );

        return $md;
    }

    /**
     * Detects and sets code language for markdown code blocks that don't have language specified
     * Uses simple but reliable patterns to identify common programming languages
     *
     * @param string $md
     * @return string
     */
    private function detectAndSetCodeLanguage(string $md): string
    {
        // pattern to find code blocks without specified language
        $codeBlockPattern = '/```\s*\n((?:[^`]|`[^`]|``[^`])*?)\n```/s';

        $result = preg_replace_callback($codeBlockPattern, function ($matches) {
            $code = $matches[1];
            $detectedLang = $this->detectLanguage($code);
            return "```{$detectedLang}\n{$code}\n```";
        }, $md);

        return $result ?: $md;
    }

    /**
     * Detects programming language based on code content using characteristic patterns
     *
     * @param string $code
     * @return string
     */
    private function detectLanguage(string $code): string
    {
        $patterns = [
            'php' => [
                '/^<\?php/',              // PHP opening tag
                '/\$[a-zA-Z_]/',          // PHP variables
                '/\b(?:public|private|protected)\s+function\b/', // PHP methods
                '/\bnamespace\s+[a-zA-Z\\\\]+;/', // PHP namespace
            ],
            'javascript' => [
                '/\bconst\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=/',  // const declarations
                '/\bfunction\s*\([^)]*\)\s*{/',            // function declarations
                '/\blet\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=/',    // let declarations
                '/\bconsole\.log\(/',                      // console.log
                '/=>\s*{/',                                // arrow functions
            ],
            'jsx' => [
                '/return\s+\(/',                         // JSX return statements
                '/import\s+[a-zA-Z0-9_,\{\} ]+\s+from/',     // imports
                '/export\s+(default|const)/',                  // exports
            ],
            'typescript' => [
                '/:\s*(?:string|number|boolean|any)\b/',   // type annotations
                '/interface\s+[A-Z][a-zA-Z0-9_]*\s*{/',    // interfaces
                '/type\s+[A-Z][a-zA-Z0-9_]*\s*=/',        // type aliases
            ],
            'python' => [
                '/def\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\([^)]*\):\s*$/', // function definitions
                '/^from\s+[a-zA-Z_.]+\s+import\b/',       // imports
                '/^if\s+__name__\s*==\s*[\'"]__main__[\'"]:\s*$/', // main guard
            ],
            'java' => [
                '/public\s+class\s+[A-Z][a-zA-Z0-9_]*/',  // class definitions
                '/System\.out\.println\(/',                // println
                '/private\s+final\s+/',                    // private final fields
            ],
            'rust' => [
                '/fn\s+[a-z_][a-z0-9_]*\s*\([^)]*\)\s*(?:->\s*[a-zA-Z<>]+\s*)?\{/', // functions
                '/let\s+mut\s+/',                         // mutable variables
                '/impl\s+[A-Z][a-zA-Z0-9_]*/',           // implementations
            ],
            'ruby' => [
                '/^require\s+[\'"][a-zA-Z0-9_\/]+[\'"]/',  // requires
                '/def\s+[a-z_][a-z0-9_]*\b/',             // method definitions
                '/\battr_accessor\b/',                     // attr_accessor
            ],
            'css' => [
                '/^[.#][a-zA-Z-_][^{]*\{/',              // selectors
                '/\b(?:margin|padding|border|color|background):\s*[^;]+;/', // common properties
                '/@media\s+/',                            // media queries
            ],
            'bash' => [
                '/^#!\/bin\/(?:bash|sh)/',               // shebang
                '/\$\([^)]+\)/',                         // command substitution
                '/(?:^|\s)(?:-{1,2}[a-zA-Z0-9]+)/',     // command options
                '/\becho\s+/',                           // echo command
                '/\|\s*grep\b/',                         // pipes and common commands
            ],
            'go' => [
                '/\bfunc\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\([^)]*\)/',  // function declarations
                '/\btype\s+[A-Z][a-zA-Z0-9_]*\s+struct\b/',       // struct definitions
                '/\bpackage\s+[a-z][a-z0-9_]*\b/',                // package declarations
                '/\bif\s+err\s*!=\s*nil\b/',                      // error handling
            ],
            'csharp' => [
                '/\bnamespace\s+[A-Za-z.]+\b/',                   // namespace declarations
                '/\bpublic\s+(?:class|interface|enum)\b/',        // public types
                '/\busing\s+[A-Za-z.]+;/',                        // using statements
                '/\basync\s+Task</',                              // async methods
            ],
            'kotlin' => [
                '/\bfun\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\(/',         // function declarations
                '/\bval\s+[a-zA-Z_][a-zA-Z0-9_]*:/',             // immutable variables
                '/\bvar\s+[a-zA-Z_][a-zA-Z0-9_]*:/',             // mutable variables
                '/\bdata\s+class\b/',                             // data classes
            ],
            'swift' => [
                '/\bfunc\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\(/',        // function declarations
                '/\bvar\s+[a-zA-Z_][a-zA-Z0-9_]*:\s*[A-Z]/',     // typed variables
                '/\blet\s+[a-zA-Z_][a-zA-Z0-9_]*:/',             // constants
                '/\bclass\s+[A-Z][A-Za-z0-9_]*:/',               // class inheritance
            ],
            'cpp' => [
                '/\b(?:class|struct)\s+[A-Z][a-zA-Z0-9_]*\b/',   // class/struct declarations
                '/\bstd::[a-z0-9_]+/',                           // std namespace usage
                '/\b#include\s+[<"][a-z0-9_.]+[>"]/',            // includes
                '/\btemplate\s*<[^>]+>/',                        // templates
            ],
            'scala' => [
                '/\bdef\s+[a-z][a-zA-Z0-9_]*\s*\(/',            // method declarations
                '/\bcase\s+class\b/',                            // case classes
                '/\bobject\s+[A-Z][a-zA-Z0-9_]*\b/',            // objects
                '/\bval\s+[a-z][a-zA-Z0-9_]*\s*=/',             // value declarations
            ],
            'perl' => [
                '/\buse\s+[A-Z][A-Za-z:]+;/',                   // module imports
                '/\bsub\s+[a-z_][a-z0-9_]*\s*\{/',             // subroutine definitions
                '/\@[a-zA-Z_][a-zA-Z0-9_]*/',                   // array variables
            ],
            'lua' => [
                '/\bfunction\s+[a-z_][a-z0-9_]*\s*\(/',         // function definitions
                '/\blocal\s+[a-z_][a-z0-9_]*\s*=/',             // local variables
                '/\brequire\s*\(?[\'"][^\'"]+[\'"]\)?/',        // require statements
            ],
            'vb' => [
                '/\bPublic\s+(?:Class|Interface|Module)\b/',        // type declarations
                '/\bPrivate\s+Sub\s+[A-Za-z_][A-Za-z0-9_]*\(/',    // private methods
                '/\bDim\s+[A-Za-z_][A-Za-z0-9_]*\s+As\b/',         // variable declarations
                '/\bEnd\s+(?:Sub|Function|Class|If|While)\b/',      // end blocks
            ],
            'fsharp' => [
                '/\blet\s+[a-z_][a-zA-Z0-9_]*\s*=/',              // value bindings
                '/\bmodule\s+[A-Z][A-Za-z0-9_]*\s*=/',            // module definitions
                '/\btype\s+[A-Z][A-Za-z0-9_]*\s*=/',              // type definitions
                '/\bmatch\s+.*\bwith\b/',                         // pattern matching
            ],
            'powershell' => [
                '/\$[A-Za-z_][A-Za-z0-9_]*/',                     // variables
                '/\[Parameter\(.*?\)\]/',                          // parameter attributes
                '/\bfunction\s+[A-Z][A-Za-z0-9-]*/',              // function declarations
                '/\b(?:Get|Set|New|Remove)-[A-Z][A-Za-z]*/',      // common cmdlets
            ],
            'xaml' => [
                '/<Window\s+[^>]*>/',                             // WPF windows
                '/<UserControl\s+[^>]*>/',                        // user controls
                '/xmlns:(?:x|d)="[^"]+"/',                       // common namespaces
                '/<(?:Grid|StackPanel|DockPanel)[^>]*>/',        // common layout controls
            ],
            'razor' => [
                '/@(?:model|using|inject)/',                      // Razor directives
                '/@Html\.[A-Za-z]+\(/',                          // Html helpers
                '/@\{.*?\}/',                                    // code blocks
                '/<partial\s+name="[^"]+"\s*\/>/',              // partial views
            ],
            'html' => [
                '/<(html|head|body|h1|a|img|table|tr|td|ul|ol|li|script|style)[^>]*>/', // HTML tags
            ]
        ];

        $scores = [];
        foreach ($patterns as $lang => $langPatterns) {
            $scores[$lang] = 0;
            foreach ($langPatterns as $pattern) {
                $matches = preg_match_all($pattern, $code);
                if ($matches) {
                    $scores[$lang] += $matches;
                }
            }
        }

        // find language with highest score
        $maxScore = 0;
        $detectedLang = '';

        foreach ($scores as $lang => $score) {
            if ($score > $maxScore) {
                $maxScore = $score;
                $detectedLang = $lang;
            }
        }

        // return detected language or empty string if nothing was detected
        return $maxScore > 0 ? $detectedLang : '';
    }

    private function getRelativeFilePathForFileByUrl(VisitedUrl $visitedUrl): string
    {
        $urlConverter = new OfflineUrlConverter(
            $this->crawler->getInitialParsedUrl(),
            ParsedUrl::parse($visitedUrl->sourceUqId ? $this->status->getUrlByUqId($visitedUrl->sourceUqId) : $this->crawler->getCoreOptions()->url),
            ParsedUrl::parse($visitedUrl->url),
            [$this->crawler, 'isDomainAllowedForStaticFiles'],
            [$this->crawler, 'isExternalDomainAllowedForCrawling'],
            // give hint about image (simulating 'src' attribute) to have same logic about dynamic images URL without extension
            $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_IMAGE ? 'src' : 'href'
        );

        $relativeUrl = $urlConverter->convertUrlToRelative(false);
        $relativeTargetUrl = $urlConverter->getRelativeTargetUrl();
        $relativePath = '';

        switch ($urlConverter->getTargetDomainRelation()) {
            case TargetDomainRelation::INITIAL_DIFFERENT__BASE_SAME:
            case TargetDomainRelation::INITIAL_DIFFERENT__BASE_DIFFERENT:
                $relativePath = ltrim(str_replace('../', '', $relativeUrl), '/ ');
                if (!str_starts_with($relativePath, '_' . $relativeTargetUrl->host)) {
                    $relativePath = '_' . $relativeTargetUrl->host . '/' . $relativePath;
                }
                break;
            case TargetDomainRelation::INITIAL_SAME__BASE_SAME:
            case TargetDomainRelation::INITIAL_SAME__BASE_DIFFERENT:
                $relativePath = ltrim(str_replace('../', '', $relativeUrl), '/ ');
                break;
        }

        return $relativePath;
    }

    private function isValidUrl(string $url): bool
    {
        return filter_var($url, FILTER_VALIDATE_URL) !== false;
    }

    /**
     * Check if URL can be stored with respect to --markdown-export-store-only-url-regex option and --allow-domain-*
     *
     * @param VisitedUrl $visitedUrl
     * @return bool
     */
    private function shouldBeUrlStored(VisitedUrl $visitedUrl): bool
    {
        $result = false;

        // by --markdown-export-store-only-url-regex
        if ($this->markdownExportStoreOnlyUrlRegex) {
            foreach ($this->markdownExportStoreOnlyUrlRegex as $storeOnlyUrlRegex) {
                if (preg_match($storeOnlyUrlRegex, $visitedUrl->url) === 1) {
                    $result = true;
                    break;
                }
            }
        } else {
            $result = true;
        }

        // by --allow-domain-* for external domains
        if ($result && $visitedUrl->isExternal) {
            $parsedUrl = ParsedUrl::parse($visitedUrl->url);
            if ($this->crawler->isExternalDomainAllowedForCrawling($parsedUrl->host)) {
                $result = true;
            } else if (($visitedUrl->isStaticFile() || $parsedUrl->isStaticFile()) && $this->crawler->isDomainAllowedForStaticFiles($parsedUrl->host)) {
                $result = true;
            } else {
                $result = false;
            }
        }

        // do not store robots.txt
        if (basename($visitedUrl->url) === 'robots.txt') {
            $result = false;
        }

        return $result;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_MARKDOWN_EXPORTER,
            'Markdown exporter options', [
            new Option('--markdown-export-dir', '-med', 'markdownExportDirectory', Type::DIR, false, 'Path to directory where to save the markdown version of the website.', null, true),
            new Option('--markdown-export-single-file', null, 'markdownExportSingleFile', Type::FILE, false, 'Path to a file where to save the combined markdown files into one document. Requires --markdown-export-dir to be set.', null, true),
            new Option('--markdown-move-content-before-h1-to-end', null, 'markdownMoveContentBeforeH1ToEnd', Type::BOOL, false, 'Move all content before the main H1 heading (typically the header with the menu) to the end of the markdown.', false, true, false),
            new Option('--markdown-disable-images', '-mdi', 'markdownDisableImages', Type::BOOL, false, 'Do not export and show images in markdown files. Images are enabled by default.', false, true),
            new Option('--markdown-disable-files', '-mdf', 'markdownDisableFiles', Type::BOOL, false, 'Do not export and link files other than HTML/CSS/JS/fonts/images - eg. PDF, ZIP, etc. These files are enabled by default.', false, true),
            new Option('--markdown-remove-links-and-images-from-single-file', null, 'markdownRemoveLinksAndImagesFromSingleFile', Type::BOOL, false, 'Remove links and images from the combined single markdown file. Useful for AI tools that don\'t need these elements.', false, false),
            new Option('--markdown-exclude-selector', '-mes', 'markdownExcludeSelector', Type::STRING, true, "Exclude some page content (DOM elements) from markdown export defined by CSS selectors like 'header', '.header', '#header', etc.", null, false, true),
            new Option('--markdown-replace-content', null, 'markdownReplaceContent', Type::REPLACE_CONTENT, true, "Replace text content with `foo -> bar` or regexp in PREG format: `/card[0-9]/i -> card`", null, true, true),
            new Option('--markdown-replace-query-string', null, 'markdownReplaceQueryString', Type::REPLACE_CONTENT, true, "Instead of using a short hash instead of a query string in the filename, just replace some characters. You can use simple format 'foo -> bar' or regexp in PREG format, e.g. '/([a-z]+)=([^&]*)(&|$)/i -> $1__$2'", null, true, true),
            new Option('--markdown-export-store-only-url-regex', null, 'markdownExportStoreOnlyUrlRegex', Type::REGEX, true, 'For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.', null, true),
            new Option('--markdown-ignore-store-file-error', null, 'markdownIgnoreStoreFileError', Type::BOOL, false, 'Ignores any file storing errors. The export process will continue.', false, false),
        ]));
        return $options;
    }
}
