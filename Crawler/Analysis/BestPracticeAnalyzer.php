<?php

namespace Crawler\Analysis;

use Crawler\Analysis\Result\AnalyzerStats;
use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\ExtraColumn;
use Crawler\Options\Options;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;
use DOMNode;
use DOMXPath;

class BestPracticeAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_BEST_PRACTICE_ANALYZER = 'best-practice-analyzer';

    const ANALYSIS_VALID_HTML = 'Valid HTML';
    const ANALYSIS_LARGE_SVGS = 'Large inline SVGs';
    const ANALYSIS_DUPLICATED_SVGS = 'Duplicate inline SVGs';
    const ANALYSIS_INVALID_SVGS = 'Invalid inline SVGs';
    const ANALYSIS_MISSING_QUOTES = 'Missing quotes on attributes';
    const ANALYSIS_HEADING_STRUCTURE = 'Heading structure';
    const ANALYSIS_DOM_DEPTH = 'DOM depth';
    const ANALYSIS_TITLE_UNIQUENESS = 'Title uniqueness';
    const ANALYSIS_DESCRIPTION_UNIQUENESS = 'Description uniqueness';

    private readonly AnalyzerStats $stats;

    // options
    private int $maxInlineSvgSize = 5 * 1024;
    private int $maxInlineSvgDuplicates = 5;
    private int $titleUniquenessPercentage = 10;
    private int $metaDescriptionUniquenessPercentage = 10;
    private int $maxDomDepthWarning = 30;
    private int $maxDomDepthCritical = 50;

    // stats
    private int $pagesWithLargeSvgs = 0;
    private int $pagesWithDuplicatedSvgs = 0;
    private int $pagesWithInvalidSvgs = 0;
    private int $pagesWithMissingQuotes = 0;
    private int $pagesWithMultipleH1 = 0;
    private int $pagesWithoutH1 = 0;
    private int $pagesWithSkippedHeadingLevels = 0;
    private int $pagesWithDeepDom = 0;

    /**
     * Continuous result of all analyzed URLs in analyzeVisitedUrl()
     * Key is URL and value is UrlAnalysisResult if URL was analyzed
     *
     * @var UrlAnalysisResult[]
     */
    private array $continuousResults = [];

    public function __construct()
    {
        $this->stats = new AnalyzerStats();
    }


    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $htmlUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->statusCode === 200 && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML;
        });

        $superTable = new SuperTable(
            'best-practices',
            "Best practices summary",
            "Nothing to report.",
            [
                new SuperTableColumn('analysisName', 'Analysis name', SuperTableColumn::AUTO_WIDTH, function ($value) {
                    if ($value === self::ANALYSIS_LARGE_SVGS) {
                        $value .= " (> {$this->maxInlineSvgSize} B)";
                    } elseif ($value === self::ANALYSIS_DUPLICATED_SVGS) {
                        $value .= " (> {$this->maxInlineSvgDuplicates})";
                    } elseif ($value === self::ANALYSIS_DOM_DEPTH) {
                        $value .= " (> {$this->maxDomDepthWarning})";
                    } elseif ($value === self::ANALYSIS_TITLE_UNIQUENESS) {
                        $value .= " (> {$this->titleUniquenessPercentage}%)";
                    } elseif ($value === self::ANALYSIS_DESCRIPTION_UNIQUENESS) {
                        $value .= " (> {$this->metaDescriptionUniquenessPercentage}%)";
                    }
                    return $value;
                }),
                new SuperTableColumn('ok', 'OK', 5, function ($value) {
                    return $value > 0 ? Utils::getColorText($value, 'green') : '0';
                }, null, false, false),
                new SuperTableColumn('notice', 'Notice', 6, function ($value) {
                    return Utils::getColoredNotices($value, 6);
                }, null, false, true),
                new SuperTableColumn('warning', 'Warning', 7, function ($value) {
                    return Utils::getColoredWarnings($value, 7);
                }, null, false, true),
                new SuperTableColumn('critical', 'Critical', 8, function ($value) {
                    return Utils::getColoredCriticals($value, 8);
                }, null, false, true),
            ], true, null
        );

        $superTable->setData($this->analyzeUrls($htmlUrls));
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        $this->setFindingsToSummary();
    }

    /**
     * @param VisitedUrl[] $urls
     * @return array
     */
    private function analyzeUrls(array $urls): array
    {
        $data = $this->stats->toTableArray();

        // check for title uniqueness
        $data[] = $this->checkTitleUniqueness(array_map(function (VisitedUrl $url) {
            return $url->extras['Title'] ?? null;
        }, $urls));

        // check for meta description uniqueness
        $data[] = $this->checkMetaDescriptionUniqueness(array_map(function (VisitedUrl $url) {
            return $url->extras['Description'] ?? null;
        }, $urls));

        // check for brotli support on HTML pages (just for non-external URLs)
        $brotliSupportedInRequests = str_contains($this->crawler->getCoreOptions()->acceptEncoding, 'br');
        if ($brotliSupportedInRequests) {
            $this->checkBrotliSupport(array_filter($urls, function (VisitedUrl $url) {
                return !$url->isExternal && $url->contentType === Crawler::CONTENT_TYPE_ID_HTML;
            }));
        }

        return $data;
    }

    private function getAnalysisResult(string $analysisName, int $ok, int $notice, int $warning, int $critical): array
    {
        return [
            'analysisName' => $analysisName,
            'ok' => $ok,
            'notice' => $notice,
            'warning' => $warning,
            'critical' => $critical,
        ];
    }

    /**
     * Analyze HTML URLs for best practices - return URL analysis result with all findings
     *
     * @param VisitedUrl $visitedUrl
     * @param string|null $body
     * @param array|null $headers
     * @return UrlAnalysisResult|null
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?array $headers): ?UrlAnalysisResult
    {
        $result = null;
        $isHtml = $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $body;

        if ($isHtml) {
            $result = new UrlAnalysisResult();

            $dom = new DOMDocument();
            libxml_use_internal_errors(true);
            if (!@$dom->loadHTML($body)) {
                $result->addCritical('Failed to parse HTML - it may be badly malformed.', self::ANALYSIS_VALID_HTML);
                return $result;
            }

            $this->checkInlineSvg($body, $result);
            $this->checkMissingQuotesOnAttributes($body, $result);
            $this->checkMaxDOMDepth($dom, $body, $result);
            $this->checkHeadingStructure($dom, $body, $result);
        }

        if ($result) {
            $this->continuousResults[$visitedUrl->url] = $result;
        }

        return $result;
    }

    /**
     * @return ExtraColumn|null
     */
    public function showAnalyzedVisitedUrlResultAsColumn(): ?ExtraColumn
    {
        return new ExtraColumn('Best pr.', 8, false);
    }

    /**
     * Check common issues in inline SVGs which can cause unexpected behavior or performance issues (size, duplicates, validity)
     *
     * @param string $html
     * @param UrlAnalysisResult $result
     * @param int $maxSvgSize
     * @param int $maxDuplicates
     * @return void
     */
    private function checkInlineSvg(string $html, UrlAnalysisResult $result): void
    {
        $svgCount = 0;
        $largeSvgs = [];
        $maxFoundSvgSize = 0;
        $duplicatedSvgs = [];
        $invalidSvgs = [];
        $duplicates = [];

        if (!preg_match_all('/<svg[^>]*>(.*?)<\/svg>/is', $html, $matches)) {
            return;
        }

        $svgCount = count($matches[0]);
        $okSvg = [];
        foreach ($matches[0] as $svg) {
            $svg = trim($svg);
            $okSvg[$svg] = true;
            // get hash of svg because svg can be larger than PHP array key limit
            $svgHash = md5($svg);
            $size = strlen($svg);

            // check inline SVG size
            if ($size > $this->maxInlineSvgSize) {
                unset($okSvg[$svg]);
                $largeSvgs[$svgHash] = $svg;
                $maxFoundSvgSize = max($maxFoundSvgSize, $size);
                $this->stats->addWarning(self::ANALYSIS_LARGE_SVGS, $svg);
            } else {
                $this->stats->addOk(self::ANALYSIS_LARGE_SVGS, $svg);
            }

            // check duplicates
            if (!isset($duplicates[$svgHash])) {
                $duplicates[$svgHash] = ['count' => 0, 'svg' => $svg];
            }
            $duplicates[$svgHash]['count']++;

            // check validity
            $errors = $this->validateSvg($svg);
            if ($errors) {
                unset($okSvg[$svg]);
                $invalidSvgs[$svgHash] = ['svg' => $svg, 'errors' => $errors];
                $this->stats->addWarning(self::ANALYSIS_INVALID_SVGS, $svg);
            } else {
                $this->stats->addOk(self::ANALYSIS_INVALID_SVGS, $svg);
            }
        }

        // evaluate inline SVG duplicates
        foreach ($duplicates as $hash => $info) {
            if ($info['count'] > $this->maxInlineSvgDuplicates) {
                unset($okSvg[$info['svg']]);
                $duplicatedSvgs[$hash] = "{$info['count']}x SVG: {$info['svg']}";
                $this->stats->addWarning(self::ANALYSIS_DUPLICATED_SVGS, $info['svg']);
            } else {
                $this->stats->addOk(self::ANALYSIS_DUPLICATED_SVGS, $info['svg']);
            }
        }

        // evaluate large SVGs
        if ($largeSvgs) {
            $result->addWarning(count($largeSvgs) . " inline svg(s) larger than " . ($this->maxInlineSvgSize) . " bytes. Largest SVG is {$maxFoundSvgSize} bytes. Consider loading from an external file to minimize HTML size", self::ANALYSIS_LARGE_SVGS, $largeSvgs);
            $this->pagesWithLargeSvgs++;
        }

        $smallSvgs = $svgCount - count($largeSvgs);
        if ($smallSvgs > 0) {
            $result->addOk("{$smallSvgs} inline svg(s) have a size less than {$this->maxInlineSvgSize} bytes", self::ANALYSIS_LARGE_SVGS);
        }

        // evaluate duplicated SVGs
        if ($duplicatedSvgs) {
            $result->addWarning(count($duplicatedSvgs) . ' inline svg(s) are duplicated. Consider loading from an external file to minimize HTML size', self::ANALYSIS_DUPLICATED_SVGS, $duplicatedSvgs);
            $this->pagesWithDuplicatedSvgs++;
        }

        $uniqSvgs = $svgCount - count($duplicatedSvgs);
        if ($uniqSvgs > 0) {
            $result->addOk("{$uniqSvgs} inline svg(s) are unique (less than {$this->maxInlineSvgDuplicates} duplicates)", self::ANALYSIS_DUPLICATED_SVGS);
        }

        // evaluate invalid SVGs
        if ($invalidSvgs) {
            $invalidSvgsDetail = [];
            foreach ($invalidSvgs as $hash => $info) {
                $invalidSvgsDetail[] = "Found " . count($info['errors']) . " error(s) in SVG " . htmlspecialchars($info['svg']) . ".\nErrors:\n" . implode("\n", $info['errors']);
            }
            $result->addCritical(count($invalidSvgs) . ' invalid inline svg(s). Check the content of the SVG as it may contain invalid XML and cause unexpected display problems', self::ANALYSIS_INVALID_SVGS, $invalidSvgsDetail);
            $this->pagesWithInvalidSvgs++;
        }

        $validSvgs = $svgCount - count($invalidSvgs);
        if ($validSvgs > 0) {
            $result->addOk("{$validSvgs} inline svg(s) are valid", self::ANALYSIS_INVALID_SVGS);
        }
    }

    /**
     * Check for missing quotes on attributes. Missing quotes can cause unexpected behavior when the attribute
     * value is not properly escaped.
     *
     * @param string $html
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMissingQuotesOnAttributes(string $html, UrlAnalysisResult $result): void
    {
        $issues = [];
        $attributesToCheck = ['href', 'src', 'content', 'alt', 'title'];

        $attributesPattern = '\b\s(' . implode('|', $attributesToCheck) . ')\s*=\s*([^"\'][^\s>]*)';
        $tagPattern = '/<[^>]*' . $attributesPattern . '[^>]*>/i';

        if (preg_match_all($tagPattern, $html, $matches, PREG_SET_ORDER)) {
            foreach ($matches as $match) {
                // skip attributes without value
                if (!isset($match[3])) {
                    continue;
                }
                // skip <astro-* tags from Astro framework (it uses custom syntax for attributes)
                if (str_starts_with($match[0], '<astro')) {
                    continue;
                }

                $attribute = $match[2];
                $value = $match[3];
                if (trim($value) != '' && !is_numeric($value)) {
                    $issues[] = "The attribute '{$attribute}' has a value not enclosed in quotes in tag '{$match[0]}'";
                    $this->stats->addWarning(self::ANALYSIS_MISSING_QUOTES, $match[0]);
                }
            }
        }

        if ($issues) {
            $result->addWarning(count($issues) . ' attribute(s) with missing quotes', self::ANALYSIS_MISSING_QUOTES, $issues);
            $this->pagesWithMissingQuotes++;
        }
    }

    /**
     * Validate SVG and return NULL for OK or array of errors
     *
     * @param string $svg
     * @return string[]|null
     */
    private function validateSvg(string $svg): ?array
    {
        libxml_use_internal_errors(true);
        $dom = new DOMDocument();

        if (!$dom->loadXML($svg)) {
            $errors = libxml_get_errors();
            $errorMessages = [];

            foreach ($errors as $error) {
                $errorMessages[] = trim($error->message);
            }

            libxml_clear_errors();
            return $errorMessages;
        }

        libxml_use_internal_errors(false);
        return null;
    }

    /**
     * Check too deep DOM tree. Too deep DOM trees can cause performance issues.
     *
     * @param DOMDocument $dom
     * @param string $html
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMaxDOMDepth(DOMDocument $dom, string $html, UrlAnalysisResult $result): void
    {
        // Reset libxml error buffering
        libxml_clear_errors();
        libxml_use_internal_errors(false);

        // Start depth search from the body element
        $body = $dom->getElementsByTagName('body')->item(0);

        // Calculate the maximum depth
        $maxDepth = $body ? $this->findMaxDepth($body) : 0;

        // Report based on the depth
        if ($maxDepth >= $this->maxDomDepthCritical) {
            $result->addCritical("The DOM depth exceeds the critical limit: {$this->maxDomDepthCritical}. Found depth: {$maxDepth}", self::ANALYSIS_DOM_DEPTH);
            $this->stats->addCritical(self::ANALYSIS_DOM_DEPTH, $html);
            $this->pagesWithDeepDom++;
        } elseif ($maxDepth >= $this->maxDomDepthWarning) {
            $result->addWarning("The DOM depth exceeds the warning limit: {$this->maxDomDepthWarning}. Found depth: {$maxDepth}", self::ANALYSIS_DOM_DEPTH);
            $this->stats->addWarning(self::ANALYSIS_DOM_DEPTH, $html);
            $this->pagesWithDeepDom++;
        } else {
            $result->addOk("The DOM depth is within acceptable limits. Found depth: {$maxDepth}", self::ANALYSIS_DOM_DEPTH);
            $this->stats->addOk(self::ANALYSIS_DOM_DEPTH, $html);
        }
    }

    /**
     * Find the maximum depth of the DOM tree.
     *
     * @param DOMNode $node
     * @param int $depth
     * @return int
     */
    private function findMaxDepth(DOMNode $node, int $depth = 0): int
    {
        if ($node->childNodes) {
            $childDepth = 0;
            foreach ($node->childNodes as $child) {
                $childDepth = max($childDepth, $this->findMaxDepth($child, $depth + 1));
            }
            $depth = max($depth, $childDepth);
        }

        return $depth;
    }

    /**
     * Check heading structure - exactly one <h1> and no missing levels (e.g. <h1> followed by <h3>)
     *
     * @param DOMDocument $dom
     * @param string $html
     * @param UrlAnalysisResult $result
     * @return void
     */
    public function checkHeadingStructure(DOMDocument $dom, string $html, UrlAnalysisResult $result)
    {
        $warningIssues = [];
        $criticalIssues = [];

        libxml_clear_errors();
        libxml_use_internal_errors(false);

        $xpath = new DOMXPath($dom);
        $headings = $xpath->query('//h1 | //h2 | //h3 | //h4 | //h5 | //h6');

        if ($headings->length === 0) {
            $result->addNotice('No headings found in the HTML content.', self::ANALYSIS_HEADING_STRUCTURE);
            $this->stats->addNotice(self::ANALYSIS_HEADING_STRUCTURE, $html);
            return;
        }

        $foundH1 = false;
        $previousHeadingLevel = 0;

        foreach ($headings as $heading) {
            $currentHeadingLevel = (int)substr($heading->tagName, 1);

            if ($currentHeadingLevel === 1) {
                if ($foundH1) {
                    $criticalIssues[] = 'Multiple <h1> tags found.';
                    $this->stats->addCritical(self::ANALYSIS_HEADING_STRUCTURE, $html . ' - multiple h1 tags found');
                } else {
                    $foundH1 = true;
                }
            }

            if ($currentHeadingLevel > $previousHeadingLevel + 1) {
                $warningIssues[] = "Heading structure is skipping levels: found an <{$heading->tagName}> after an <h{$previousHeadingLevel}>.";
                $this->stats->addWarning(self::ANALYSIS_HEADING_STRUCTURE, $html . " - skipped levels {$heading->tagName} after <h{$previousHeadingLevel}>");
            }

            $previousHeadingLevel = $currentHeadingLevel;
        }

        // check if at least one h1 tag was found
        if (!$foundH1) {
            $criticalIssues[] = 'No <h1> tag found in the HTML content . ';
            $this->pagesWithoutH1++;
        } else {
            $result->addOk('At least one h1 tag was found . ', self::ANALYSIS_HEADING_STRUCTURE);
            $this->stats->addOk(self::ANALYSIS_HEADING_STRUCTURE, $html . ' - at least one h1 tag found');

            // critical issues at this point mean that multiple h1 tags were found
            if ($criticalIssues) {
                $this->pagesWithMultipleH1++;
            }
        }

        // evaluate issues
        if ($criticalIssues) {
            if (!$foundH1) {
                $result->addCritical('No <h1> found.', self::ANALYSIS_HEADING_STRUCTURE, $criticalIssues);
            } else {
                $result->addCritical('Up to ' . (count($criticalIssues) + 1) . ' headings <h1> found.', self::ANALYSIS_HEADING_STRUCTURE, $criticalIssues);
            }
        }
        if ($warningIssues) {
            $result->addWarning(count($warningIssues) . ' heading structure issue(s) found.', self::ANALYSIS_HEADING_STRUCTURE, $warningIssues);
            $this->pagesWithSkippedHeadingLevels++;
        }
        if (!$criticalIssues && !$warningIssues) {
            $result->addOk('Heading structure is valid . ', self::ANALYSIS_HEADING_STRUCTURE);
            $this->stats->addOk(self::ANALYSIS_HEADING_STRUCTURE, $html . ' - heading structure is valid');
        }
    }

    private function checkTitleUniqueness(array $titles): array
    {
        $summaryAplCode = 'title-uniqueness';
        if (!$titles) {
            $this->status->addWarningToSummary($summaryAplCode, 'No titles provided for uniqueness check.');
            return $this->getAnalysisResult(self::ANALYSIS_TITLE_UNIQUENESS, 0, 0, 1, 0);
        }

        $titles = array_filter($titles, function ($title) {
            return $title !== null;
        });

        if (count($titles) <= 1) {
            $this->status->addOkToSummary($summaryAplCode, 'Only one title provided for uniqueness check.');
            return $this->getAnalysisResult(self::ANALYSIS_TITLE_UNIQUENESS, 1, 0, 0, 0);
        }

        // Count the occurrences of each title
        $counts = array_count_values($titles);

        // Calculate the total number of titles
        $totalTitles = count($titles);

        // Track whether we've found any non - unique titles
        $nonUniqueFound = false;

        // Check each title's count against the allowed duplicity percentage
        $ok = 0;
        $warnings = 0;
        $highestDuplicityPercentage = 0;
        foreach ($counts as $title => $count) {
            $duplicityPercentage = intval(($count / $totalTitles) * 100);
            $highestDuplicityPercentage = max($highestDuplicityPercentage, $duplicityPercentage);

            if ($count > 1 && $duplicityPercentage > $this->titleUniquenessPercentage) {
                $this->status->addWarningToSummary($summaryAplCode, "The title '{$title}' exceeds the allowed {$this->titleUniquenessPercentage}% duplicity. {$duplicityPercentage}% of pages have this same title.");
                $nonUniqueFound = true;
                $warnings++;
            } else {
                $ok++;
            }
        }

        // If no non-unique titles were found, report a success message
        if (!$nonUniqueFound) {
            $this->status->addOkToSummary($summaryAplCode, "All {$ok} unique title(s) are within the allowed {$this->titleUniquenessPercentage}% duplicity. Highest duplicity title has {$highestDuplicityPercentage}%.");
        }

        return $this->getAnalysisResult(self::ANALYSIS_TITLE_UNIQUENESS, $ok, 0, $warnings, 0);
    }

    private function checkMetaDescriptionUniqueness(array $descriptions): array
    {
        $summaryAplCode = 'meta-description-uniqueness';
        if (!$descriptions) {
            $this->status->addWarningToSummary($summaryAplCode, 'No meta descriptions provided for uniqueness check.');
            return $this->getAnalysisResult(self::ANALYSIS_DESCRIPTION_UNIQUENESS, 0, 0, 1, 0);
        }

        $descriptions = array_filter($descriptions, function ($description) {
            return $description !== null;
        });

        if (count($descriptions) <= 1) {
            $this->status->addOkToSummary($summaryAplCode, 'Only one meta description provided for uniqueness check.');
            return $this->getAnalysisResult(self::ANALYSIS_DESCRIPTION_UNIQUENESS, 1, 0, 0, 0);
        }

        // Count the occurrences of each description
        $counts = array_count_values($descriptions);

        // Calculate the total number of descriptions
        $totalDescriptions = count($descriptions);

        // Track whether we've found any non - unique descriptions
        $nonUniqueFound = false;

        // Check each description's count against the allowed duplicity percentage
        $ok = 0;
        $warnings = 0;
        $highestDuplicityPercentage = 0;
        foreach ($counts as $description => $count) {
            $duplicityPercentage = intval(($count / $totalDescriptions) * 100);
            $highestDuplicityPercentage = max($highestDuplicityPercentage, $duplicityPercentage);

            if ($count > 1 && $duplicityPercentage > $this->metaDescriptionUniquenessPercentage) {
                $this->status->addWarningToSummary($summaryAplCode, "The description '{$description}' exceeds the allowed {$this->metaDescriptionUniquenessPercentage}% duplicity. {$duplicityPercentage}% of pages have this same description.");
                $nonUniqueFound = true;
                $warnings++;
            } else {
                $ok++;
            }
        }

        // If no non-unique descriptions were found, report a success message
        if (!$nonUniqueFound) {
            $this->status->addOkToSummary($summaryAplCode, "All {$ok} description(s) are within the allowed {$this->metaDescriptionUniquenessPercentage}% duplicity. Highest duplicity description has {$highestDuplicityPercentage}%.");
        }

        return $this->getAnalysisResult(self::ANALYSIS_DESCRIPTION_UNIQUENESS, $ok, 0, $warnings, 0);
    }

    /**
     * @param VisitedUrl[] $urls
     * @return void
     */
    private function checkBrotliSupport(array $urls): void
    {
        $summaryAplCode = 'brotli-support';
        $urlsWithoutBrotli = array_filter($urls, function (VisitedUrl $url) {
            return $url->contentEncoding !== 'br';
        });

        if ($urlsWithoutBrotli) {
            $this->status->addWarningToSummary($summaryAplCode, count($urlsWithoutBrotli) . ' page(s) do not support Brotli compression.');
        } else {
            $this->status->addOkToSummary($summaryAplCode, 'All pages support Brotli compression.');
        }
    }

    /**
     * Set findings to summary
     *
     * @return void
     */
    private function setFindingsToSummary(): void
    {
        // quotes on attributes
        if ($this->pagesWithMissingQuotes > 0) {
            $this->status->addWarningToSummary('pages-with-missing-quotes', "{$this->pagesWithMissingQuotes} page(s) with missing quotes on attributes");
        } else {
            $this->status->addOkToSummary('pages-with-missing-quotes', "All pages have quotes on attributes");
        }

        // inline SVGs
        if ($this->pagesWithLargeSvgs > 0) {
            $this->status->addWarningToSummary('pages-with-large-svgs', "{$this->pagesWithLargeSvgs} page(s) with large inline SVGs (> {$this->maxInlineSvgSize} bytes))");
        } else {
            $this->status->addOkToSummary('pages-with-large-svgs', "All pages have inline SVGs smaller than {$this->maxInlineSvgSize} bytes");
        }

        if ($this->pagesWithDuplicatedSvgs > 0) {
            $this->status->addWarningToSummary('pages-with-duplicated-svgs', "{$this->pagesWithDuplicatedSvgs} page(s) with duplicated inline SVGs (> {$this->maxInlineSvgDuplicates} duplicates))");
        } else {
            $this->status->addOkToSummary('pages-with-duplicated-svgs', "All pages have inline SVGs with less than {$this->maxInlineSvgDuplicates} duplicates");
        }

        if ($this->pagesWithInvalidSvgs > 0) {
            $this->status->addCriticalToSummary('pages-with-invalid-svgs', "{$this->pagesWithInvalidSvgs} page(s) with invalid inline SVGs");
        } else {
            $this->status->addOkToSummary('pages-with-invalid-svgs', "All pages have valid or none inline SVGs");
        }

        // heading structure
        if ($this->pagesWithMultipleH1 > 0) {
            $this->status->addCriticalToSummary('pages-with-multiple-h1', "{$this->pagesWithMultipleH1} page(s) with multiple <h1> tags");
        } else {
            $this->status->addOkToSummary('pages-with-multiple-h1', "All pages without multiple <h1> tags");
        }

        if ($this->pagesWithoutH1 > 0) {
            $this->status->addCriticalToSummary('pages-without-h1', "{$this->pagesWithoutH1} page(s) without <h1> tag");
        } else {
            $this->status->addOkToSummary('pages-without-h1', "All pages have at least one <h1> tag");
        }

        if ($this->pagesWithSkippedHeadingLevels > 0) {
            $this->status->addWarningToSummary('pages-with-skipped-heading-levels', "{$this->pagesWithSkippedHeadingLevels} page(s) with skipped heading levels");
        } else {
            $this->status->addOkToSummary('pages-with-skipped-heading-levels', "All pages have heading structure without skipped levels");
        }

        // DOM depth
        if ($this->pagesWithDeepDom > 0) {
            $this->status->addWarningToSummary('pages-with-deep-dom', "{$this->pagesWithDeepDom} page(s) with deep DOM (> {$this->maxDomDepthWarning} levels)");
        } else {
            $this->status->addOkToSummary('pages-with-deep-dom', "All pages have DOM depth less than {$this->maxDomDepthWarning}");
        }
    }

    public function getOrder(): int
    {
        return 170;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }


}