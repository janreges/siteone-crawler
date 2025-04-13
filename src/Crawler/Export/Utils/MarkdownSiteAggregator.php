<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

 declare(strict_types=1);

 namespace Crawler\Export\Utils;

class MarkdownSiteAggregator
{

    private const SIMILARITY_THRESHOLD = 80.0;

    private string $baseUrl;

    public function __construct(string $baseUrl = '')
    {
        // Base URL (e.g. "https://example.com/") for building complete addresses
        $this->baseUrl = rtrim($baseUrl, '/');
    }

    public function combineDirectory(string $directoryPath): string
    {
        $files = $this->getMarkdownFiles($directoryPath);
        sort($files);  // alphabetical sorting of paths (for final sorting by URL)
        
        // Load the content of all files into an array [url => content]
        $pages = [];
        foreach ($files as $filePath) {
            $url = $this->makeUrlFromPath($filePath, $directoryPath);
            $content = file_get_contents($filePath);
            $pages[$url] = explode("\n", rtrim($content));  // store content as an array of lines (without trailing empty line)
        }
        
        // Detect common header and footer (as array of lines)
        $headerLines = $this->detectCommonHeader($pages);
        $footerLines = $this->detectCommonFooter($pages);
        
        // Remove header and footer from the content of individual pages
        foreach ($pages as $url => &$lines) {
            if (!empty($headerLines)) {
                $lines = $this->removePrefix($lines, $headerLines);
            }
            if (!empty($footerLines)) {
                $lines = $this->removeSuffix($lines, $footerLines);
            }
        }
        unset($lines); // release the reference variable
        
        // Build the resulting Markdown string
        $resultLines = [];
        if (!empty($headerLines)) {
            // Add header to the beginning
            $resultLines = array_merge($resultLines, $headerLines);
            $resultLines[] = "";  // empty line after header
        }
        // Add content of all pages with their URLs
        foreach ($pages as $url => $lines) {
            $resultLines[] = "--- URL: {$url} ---";
            foreach ($lines as $line) {
                $resultLines[] = $line;
            }
            $resultLines[] = "";  // separator empty line between pages
        }
        if (!empty($footerLines)) {
            // Remove the last empty line before footer, if present
            if (end($resultLines) === "") {
                array_pop($resultLines);
            }
            // Add footer to the end
            $resultLines[] = "";
            $resultLines = array_merge($resultLines, $footerLines);
        }
        
        // Merge the array of lines into a single text separated by newlines
        $finalMarkdown = implode("\n", $resultLines);
        return $finalMarkdown;
    }

    private function getMarkdownFiles(string $dir): array
    {
        $paths = [];
        $iterator = new \RecursiveIteratorIterator(new \RecursiveDirectoryIterator($dir, \FilesystemIterator::SKIP_DOTS));
        foreach ($iterator as $fileInfo) {
            if ($fileInfo->isFile() && strtolower($fileInfo->getExtension()) === 'md') {
                $paths[] = $fileInfo->getPathname();
            }
        }
        return $paths;
    }

    private function makeUrlFromPath(string $filePath, string $rootDir): string
    {
        // Remove root path and extension, replace directory separators with "/"
        $relPath = ltrim(str_replace('\\', '/', substr($filePath, strlen(rtrim($rootDir, '/')))), '/');
        // Remove .md from the end
        if (str_ends_with($relPath, '.md')) {
            $relPath = substr($relPath, 0, -3);
        }
        // If the file is named index (e.g. "about/index"), the URL can end with a slash ... (optional modification)
        $relPath = preg_replace('#/index$#', '/', $relPath);
        return $this->baseUrl !== '' ? $this->baseUrl . '/' . ltrim($relPath, '/') : $relPath;
    }

    private function detectCommonHeader(array $pages): array
    {
        if (empty($pages)) return [];
        // Take an array of pages (url=>lines). For comparison, use the first few pages (e.g., 5 or all if fewer).
        $urls = array_keys($pages);
        $sampleUrls = array_slice($urls, 0, min(5, count($urls)));
        
        $commonHeader = $pages[$sampleUrls[0]];  // start with the complete content of the first page as a candidate
        // Gradually narrow down commonHeader by comparing with others from the sample
        for ($i = 1; $i < count($sampleUrls); $i++) {
            $otherLines = $pages[$sampleUrls[$i]];
            $commonHeader = $this->alignCommonPrefix($commonHeader, $otherLines);
            if (empty($commonHeader)) break;  // no common header
        }
        return $commonHeader;
    }

    private function detectCommonFooter(array $pages): array
    {
        if (empty($pages)) return [];
        $urls = array_keys($pages);
        $sampleUrls = array_slice($urls, 0, min(5, count($urls)));
        $commonFooter = $pages[$sampleUrls[0]];
        // Reverse the first page (to compare suffix as prefix)
        $commonFooter = array_reverse($commonFooter);
        for ($i = 1; $i < count($sampleUrls); $i++) {
            $otherRev = array_reverse($pages[$sampleUrls[$i]]);
            $commonFooter = $this->alignCommonPrefix($commonFooter, $otherRev);
            if (empty($commonFooter)) break;
        }
        // After obtaining the common prefix in the reversed commonFooter array, we turn it back to the correct order
        $commonFooter = array_reverse($commonFooter);
        return $commonFooter;
    }

    // Helper function: aligns two lists of lines and finds their common prefix (with fuzzy tolerance)
    private function alignCommonPrefix(array $linesA, array $linesB): array
    {
        $result = [];
        $i = 0;
        $j = 0;
        while ($i < count($linesA) && $j < count($linesB)) {
            if ($this->linesSimilar($linesA[$i], $linesB[$j])) {
                // Lines are (fuzzy) identical
                $result[] = $linesA[$i];
                $i++;
                $j++;
            } else {
                // Try skipping a line in A or in B
                $skipA = false;
                $skipB = false;
                if ($i+1 < count($linesA) && $this->linesSimilar($linesA[$i+1], $linesB[$j])) {
                    // Extra line in A (skip linesA[$i])
                    $i++;
                    $skipA = true;
                }
                if (!$skipA && $j+1 < count($linesB) && $this->linesSimilar($linesA[$i], $linesB[$j+1])) {
                    // Extra line in B (skip linesB[$j])
                    $j++;
                    $skipB = true;
                }
                if (!($skipA || $skipB)) {
                    // If we didn't skip anything (nor could skip) -> end of common prefix
                    break;
                }
                // if we skipped, continue in the while loop without adding a common line (thus aligning the offsets)
            }
        }
        return $result;
    }

    // Helper function: evaluates the similarity of two lines (ignores formatting)
    private function linesSimilar(string $a, string $b): bool
    {
        // For simplicity, remove markdown emphasis (**, *, __, _)
        $normalize = function(string $s): string {
            $s = preg_replace('/[*_]+/', '', $s);
            return trim($s ?? '');
        };
        $na = $normalize($a);
        $nb = $normalize($b);
        if ($na === $nb) {
            return true;
        }
        // If not exactly the same, calculate similarity percentage (similar_text)
        $percent = 0.0;
        similar_text($na, $nb, $percent);
        return $percent >= self::SIMILARITY_THRESHOLD;
    }

    // Removes common prefix (header) from the page's line array
    private function removePrefix(array $lines, array $prefixLines): array
    {
        if (empty($prefixLines)) return $lines;
        $len = count($prefixLines);
        // Find the position where prefixLines occur in lines (expected at the beginning, possibly with minor deviations through skip)
        // Simply remove the first $len lines from the page, as they should correspond to the header.
        // (For higher reliability, the index could be compared and fine-tuned, but simplified:)
        return array_slice($lines, $len);
    }

    // Removes common suffix (footer) from the page's line array
    private function removeSuffix(array $lines, array $suffixLines): array
    {
        if (empty($suffixLines)) return $lines;
        $len = count($suffixLines);
        // Remove the last $len lines
        return array_slice($lines, 0, count($lines) - $len);
    }
}
