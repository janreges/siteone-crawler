<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

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
    const ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS = 'Non-clickable phone numbers';
    const ANALYSIS_DOM_DEPTH = 'DOM depth';
    const ANALYSIS_TITLE_UNIQUENESS = 'Title uniqueness';
    const ANALYSIS_DESCRIPTION_UNIQUENESS = 'Description uniqueness';
    const ANALYSIS_BROTLI_SUPPORT = 'Brotli support';
    const ANALYSIS_WEBP_SUPPORT = 'WebP support';
    const ANALYSIS_AVIF_SUPPORT = 'AVIF support';

    const SUPER_TABLE_BEST_PRACTICES = 'best-practices';
    const SUPER_TABLE_NON_UNIQUE_TITLES = 'non-unique-titles';
    const SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS = 'non-unique-descriptions';

    private readonly AnalyzerStats $stats;

    private array $topTitlesToCount = [];
    private array $topDescriptionsToCount = [];

    // options
    private int $maxInlineSvgSize = 5 * 1024;
    private int $maxInlineSvgDuplicateSize = 1024;
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
    private int $pagesWithNonClickablePhoneNumbers = 0;

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
            return $visitedUrl->isAllowedForCrawling && $visitedUrl->statusCode === 200 && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML;
        });

        $imagesUrls = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->statusCode === 200 && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_IMAGE;
        });

        $superTable = new SuperTable(
            self::SUPER_TABLE_BEST_PRACTICES,
            "Best practices",
            "Nothing to report.",
            [
                new SuperTableColumn('analysisName', 'Analysis name', SuperTableColumn::AUTO_WIDTH, function ($value) {
                    if ($value === self::ANALYSIS_LARGE_SVGS) {
                        $value .= " (> {$this->maxInlineSvgSize} B)";
                    } elseif ($value === self::ANALYSIS_DUPLICATED_SVGS) {
                        $value .= " (> {$this->maxInlineSvgDuplicates} and > {$this->maxInlineSvgDuplicateSize} B)";
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
                    return $value > 0 ? Utils::getColorText(strval($value), 'green') : '0';
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

        $superTable->setData($this->analyzeUrls($htmlUrls, $imagesUrls));
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        $this->setFindingsToSummary();
    }

    /**
     * @param VisitedUrl[] $htmlUrls
     * @param VisitedUrl[] $imagesUrls
     * @return array
     */
    private function analyzeUrls(array $htmlUrls, array $imagesUrls): array
    {
        $data = $this->stats->toTableArray();

        // check for title uniqueness
        $s = microtime(true);
        $data[] = $this->checkTitleUniqueness(array_map(function (VisitedUrl $url) {
            return $url->extras['Title'] ?? null;
        }, $htmlUrls));
        $this->measureExecTime(__CLASS__, 'checkTitleUniqueness', $s);

        // check for meta description uniqueness
        $s = microtime(true);
        $data[] = $this->checkMetaDescriptionUniqueness(array_map(function (VisitedUrl $url) {
            return $url->extras['Description'] ?? null;
        }, $htmlUrls));
        $this->measureExecTime(__CLASS__, 'checkMetaDescriptionUniqueness', $s);

        // check for brotli support on HTML pages (just for non-external URLs)
        $brotliSupportedInRequests = str_contains($this->crawler->getCoreOptions()->acceptEncoding, 'br');
        if ($brotliSupportedInRequests) {
            $s = microtime(true);
            $data[] = $this->checkBrotliSupport(array_filter($htmlUrls, function (VisitedUrl $url) {
                return !$url->isExternal && $url->contentType === Crawler::CONTENT_TYPE_ID_HTML;
            }));
            $this->measureExecTime(__CLASS__, 'checkBrotliSupport', $s);
        }

        // check for webp support on images
        $s = microtime(true);
        $data[] = $this->checkWebpSupport($imagesUrls);
        $this->measureExecTime(__CLASS__, 'checkWebpSupport', $s);

        // check for avif support on images
        $s = microtime(true);
        $data[] = $this->checkAvifSupport($imagesUrls);
        $this->measureExecTime(__CLASS__, 'checkAvifSupport', $s);

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
     * @param DOMDocument|null $dom
     * @param array|null $headers
     * @return UrlAnalysisResult|null
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?DOMDocument $dom, ?array $headers): ?UrlAnalysisResult
    {
        $result = null;
        $isHtml = $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $body;

        if ($isHtml && $dom) {
            $result = new UrlAnalysisResult();

            $s = microtime(true);
            $this->checkInlineSvg($body, $result);
            $this->measureExecTime(__CLASS__, 'checkInlineSvg', $s);

            $s = microtime(true);
            $this->checkMissingQuotesOnAttributes($body, $result);
            $this->measureExecTime(__CLASS__, 'checkMissingQuotesOnAttributes', $s);

            $s = microtime(true);
            $this->checkMaxDOMDepth($dom, $body, $result);
            $this->measureExecTime(__CLASS__, 'checkMaxDOMDepth', $s);

            $s = microtime(true);
            $this->checkHeadingStructure($dom, $body, $result);
            $this->measureExecTime(__CLASS__, 'checkHeadingStructure', $s);

            $s = microtime(true);
            $this->checkNonClickablePhoneNumbers($body, $result);
            $this->measureExecTime(__CLASS__, 'checkNonClickablePhoneNumbers', $s);
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
     * @return void
     */
    private function checkInlineSvg(string $html, UrlAnalysisResult $result): void
    {
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

            // skip SVGs which are escaped because they are not inline SVGs but e.g. code example
            $isEscapedSvg = str_contains($svg, '&#x22;') || str_contains($svg, '&#x27;');
            if ($isEscapedSvg) {
                continue;
            }

            $svg = trim($svg);
            $okSvg[$svg] = true;
            // get hash of svg because svg can be larger than PHP array key limit
            $svgHash = md5($svg);
            $size = strlen($svg);

            // check inline SVG size
            if ($size > $this->maxInlineSvgSize) {
                unset($okSvg[$svg]);
                $largeSvgs[$svgHash] = Utils::sanitizeSvg($svg);
                $maxFoundSvgSize = max($maxFoundSvgSize, $size);
                $this->stats->addWarning(self::ANALYSIS_LARGE_SVGS, $svg);
            } else {
                $this->stats->addOk(self::ANALYSIS_LARGE_SVGS, $svg);
            }

            // check duplicates
            if (!isset($duplicates[$svgHash])) {
                $duplicates[$svgHash] = ['count' => 0, 'svg' => Utils::sanitizeSvg($svg), 'size' => strlen($svg)];
            }
            $duplicates[$svgHash]['count']++;

            // check validity
            $errors = $this->validateSvg($svg);
            if ($errors) {
                unset($okSvg[$svg]);
                $invalidSvgs[$svgHash] = ['svg' => Utils::sanitizeSvg($svg), 'errors' => $errors];
                $this->stats->addWarning(self::ANALYSIS_INVALID_SVGS, $svg);
            } else {
                $this->stats->addOk(self::ANALYSIS_INVALID_SVGS, $svg);
            }
        }

        // evaluate inline SVG duplicates
        foreach ($duplicates as $hash => $info) {
            if ($info['count'] > $this->maxInlineSvgDuplicates && $info['size'] > $this->maxInlineSvgDuplicateSize) {
                unset($okSvg[$info['svg']]);
                $duplicatedSvgs[$hash] = "{$info['count']}x SVG (" . Utils::getFormattedSize(strlen($info['svg'])) . "): {$info['svg']}";
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
                $invalidSvgsDetail[] = $info['svg'] . '<br />Found ' . count($info['errors']) . " error(s) in SVG. Errors:<br /> &nbsp; &gt; " . implode("<br /> &nbsp; &gt; ", $info['errors']);
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

        $attributesPattern = '\s(' . implode('|', $attributesToCheck) . ')\s*=\s*([^"\'][^\s>]*)';
        $tagPattern = '/<[^>]*' . $attributesPattern . '[^>]*>/i';

        if (preg_match_all($tagPattern, $html, $matches, PREG_SET_ORDER)) {
            foreach ($matches as $match) {
                // skip attributes without value or with very long value
                if (!isset($match[2]) || trim($match[2]) === '' || strlen($match[0]) > 1000) {
                    continue;
                }

                // skip attributes with quotes inside real HTML tag attribute .. eg. <code data-value="some <a href={foo}>">
                $regex = '/=["\'][^"\']*' . preg_quote($match[0], '/') . '/';
                $isMatchInsideTagAttribute = preg_match($regex, $html) === 1;
                if ($isMatchInsideTagAttribute) {
                    continue;
                }

                // skip cases like this, see "href={to}":
                // <button title="Copy to clipboard" data-copied="Copied!" data-code="---const { to } = Astro.props---<a href={to}>
                $regex2 = '/=["\'][^"\']*' . preg_quote($match[1], '/') . '\s*=\s*' . preg_quote($match[2], '/') . '/';
                $isMatchInsideTagAttribute2 = preg_match($regex2, $match[0]) === 1;
                if ($isMatchInsideTagAttribute2) {
                    continue;
                }

                // skip cases where attribute is in <svg> and its content
                $regex3 = '/<svg[^>]*>.{1,500}' . preg_quote($match[1], '/') . '\s*=\s*' . preg_quote($match[2], '/') . '.{1,500}<\/svg>/is';
                $isInSvg = preg_match($regex3, $html) === 1;
                if ($isInSvg) {
                    continue;
                }


                // skip <astro-* tags from Astro framework (it uses custom syntax for attributes) and backslash-escaped quotes (typically in JS code)
                $containsEscapedQuotes = str_contains($match[0], '\\"') || str_contains($match[0], "\\'");
                $containsEscapes = str_contains($match[0], '&#');
                $containsSpecialFrameworkCases = str_starts_with($match[0], '<astro');
                if ($containsEscapedQuotes || $containsEscapes || $containsSpecialFrameworkCases) {
                    continue;
                }

                $attribute = $match[1];
                $value = $match[2];
                if (trim($value) != '' && !is_numeric($value)) {
                    $issues[] = "The attribute '{$attribute}' has a value '{$value}' not enclosed in quotes in tag {$match[0]}";
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
        $dom->preserveWhiteSpace = false;

        if (!$dom->loadXML($svg)) {
            $errors = libxml_get_errors();
            $errorMessages = [];

            foreach ($errors as $error) {
                if ($error->level === LIBXML_ERR_FATAL) {
                    $errorMessages[] = trim($error->message);
                }
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
            $result->addCritical("The DOM depth exceeds the critical limit: {$this->maxDomDepthCritical}. Found depth: {$maxDepth}.", self::ANALYSIS_DOM_DEPTH, ["The DOM depth exceeds the critical limit: {$this->maxDomDepthCritical}. Found depth: {$maxDepth}."]);
            $this->stats->addCritical(self::ANALYSIS_DOM_DEPTH, $html);
            $this->pagesWithDeepDom++;
        } elseif ($maxDepth >= $this->maxDomDepthWarning) {
            $result->addWarning("The DOM depth exceeds the warning limit: {$this->maxDomDepthWarning}. Found depth: {$maxDepth}.", self::ANALYSIS_DOM_DEPTH, ["The DOM depth exceeds the warning limit: {$this->maxDomDepthWarning}. Found depth: {$maxDepth}."]);
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
        if ($node->childNodes->count() > 0) {
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
    public function checkHeadingStructure(DOMDocument $dom, string $html, UrlAnalysisResult $result): void
    {
        $warningIssues = [];
        $criticalIssues = [];

        libxml_clear_errors();
        libxml_use_internal_errors(false);

        $xpath = new DOMXPath($dom);
        $headings = @$xpath->query('//h1 | //h2 | //h3 | //h4 | //h5 | //h6');

        if ($headings && $headings->length === 0) {
            $result->addNotice('No headings found in the HTML content.', self::ANALYSIS_HEADING_STRUCTURE, ['No headings found in the HTML content.']);
            $this->stats->addNotice(self::ANALYSIS_HEADING_STRUCTURE, $html);
            return;
        }

        $foundH1 = false;
        $previousHeadingLevel = 0;

        foreach ($headings as $heading) {
            $currentHeadingLevel = (int)substr($heading->tagName, 1);

            if ($currentHeadingLevel === 1) {
                if ($foundH1) {
                    $criticalIssues[] = 'Multiple <h1> headings found.';
                    $this->stats->addCritical(self::ANALYSIS_HEADING_STRUCTURE, $html . ' - multiple h1 tags found');
                } else {
                    $foundH1 = true;
                }
            }

            if ($currentHeadingLevel > $previousHeadingLevel + 1) {
                $warningIssues[] = "Heading structure is skipping levels: found an <{$heading->tagName}> " . ($previousHeadingLevel > 0 ? "after an <h{$previousHeadingLevel}>." : 'without a previous higher heading.');
                $this->stats->addWarning(self::ANALYSIS_HEADING_STRUCTURE, $html . " - found <{$heading->tagName}> " . ($previousHeadingLevel > 0 ? "after an <h{$previousHeadingLevel}>." : 'without a previous higher heading.'));
            }

            $previousHeadingLevel = $currentHeadingLevel;
        }

        // check if at least one h1 tag was found
        if (!$foundH1) {
            $criticalIssues[] = 'No <h1> tag found in the HTML content.';
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

    /**
     * Check HTML for all phone numbers and check if they are clickable (by tel: protocol)
     *
     * @param string $html
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkNonClickablePhoneNumbers(string $html, UrlAnalysisResult $result): void
    {
        $allPhoneNumbers = Utils::parsePhoneNumbersFromHtml($html);
        $nonClickablePhoneNumbers = Utils::parsePhoneNumbersFromHtml($html, true);
        if ($nonClickablePhoneNumbers) {
            $result->addWarning(count($nonClickablePhoneNumbers) . ' non-clickable phone number(s) found.', self::ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS, $nonClickablePhoneNumbers);

            // add non-clickable phone numbers to stats
            foreach ($nonClickablePhoneNumbers as $nonClickablePhoneNumber) {
                $this->stats->addWarning(self::ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS, $nonClickablePhoneNumber);
            }

            $this->pagesWithNonClickablePhoneNumbers++;
        } else {
            $result->addOk('No non-clickable phone numbers found.', self::ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS);
            foreach ($allPhoneNumbers as $phoneNumber) {
                if (!in_array($phoneNumber, $nonClickablePhoneNumbers)) {
                    // add clickable phone numbers to stats
                    $this->stats->addOk(self::ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS, $phoneNumber);
                }
            }
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

        // sort $counts by value descending
        arsort($counts);
        foreach ($counts as $title => $count) {
            if ($count > 1 && count($this->topTitlesToCount) < 10) {
                $this->topTitlesToCount[$title] = ['title' => $title, 'count' => $count];
            }
        }

        // add supertable for $this->topTitlesToCount
        $consoleWidth = Utils::getConsoleWidth();
        $superTable = new SuperTable(
            self::SUPER_TABLE_NON_UNIQUE_TITLES,
            "TOP non-unique titles",
            "Nothing to report.",
            [
                new SuperTableColumn('count', 'Count', 5, null, null, false, false),
                new SuperTableColumn('title', 'Title', max(20, min(200, $consoleWidth - 10)), null, null, true, false),
            ], true, 'count', 'DESC'
        );
        $superTable->setData($this->topTitlesToCount);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

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

        // sort $counts by value descending
        arsort($counts);
        foreach ($counts as $desc => $count) {
            if ($count > 1 && count($this->topDescriptionsToCount) < 10) {
                $this->topDescriptionsToCount[$desc] = ['description' => $desc, 'count' => $count];
            }
        }

        // add supertable for $this->topDescriptionsToCount
        $consoleWidth = Utils::getConsoleWidth();
        $superTable = new SuperTable(
            self::SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS,
            "TOP non-unique descriptions",
            "Nothing to report.",
            [
                new SuperTableColumn('count', 'Count', 5, null, null, false, false),
                new SuperTableColumn('description', 'Description', max(20, min(200, $consoleWidth - 10)), null, null, true, false),
            ], true, 'count', 'DESC'
        );
        $superTable->setData($this->topDescriptionsToCount);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        // If no non-unique descriptions were found, report a success message
        if (!$nonUniqueFound) {
            $this->status->addOkToSummary($summaryAplCode, "All {$ok} description(s) are within the allowed {$this->metaDescriptionUniquenessPercentage}% duplicity. Highest duplicity description has {$highestDuplicityPercentage}%.");
        }

        return $this->getAnalysisResult(self::ANALYSIS_DESCRIPTION_UNIQUENESS, $ok, 0, $warnings, 0);
    }

    /**
     * @param VisitedUrl[] $urls
     * @return array
     */
    private function checkBrotliSupport(array $urls): array
    {
        $summaryAplCode = 'brotli-support';
        $urlsWithoutBrotli = array_filter($urls, function (VisitedUrl $url) {
            return $url->contentEncoding !== 'br';
        });
        $urlsWithBrotliSupport = count($urls) - count($urlsWithoutBrotli);

        if ($urlsWithoutBrotli) {
            $this->status->addWarningToSummary($summaryAplCode, count($urlsWithoutBrotli) . ' page(s) do not support Brotli compression.');
        } else {
            $this->status->addOkToSummary($summaryAplCode, 'All pages support Brotli compression.');
        }

        return $this->getAnalysisResult(self::ANALYSIS_BROTLI_SUPPORT, $urlsWithBrotliSupport, 0, count($urlsWithoutBrotli), 0);
    }

    /**
     * @param VisitedUrl[] $urls
     * @return array
     */
    private function checkWebpSupport(array $urls): array
    {
        $summaryAplCode = 'webp-support';
        $webpImages = array_filter($urls, function (VisitedUrl $url) {
            return $url->contentTypeHeader === 'image/webp';
        });

        // Check if AVIF is already supported (AVIF is more modern than WebP)
        $avifImages = array_filter($urls, function (VisitedUrl $url) {
            return $url->contentTypeHeader === 'image/avif';
        });

        if ($webpImages) {
            $this->status->addOkToSummary($summaryAplCode, count($webpImages) . ' WebP image(s) found on the website.');
        } elseif ($avifImages) {
            // If AVIF is supported, don't warn about missing WebP
            $this->status->addOkToSummary($summaryAplCode, 'No WebP images found, but AVIF (more modern format) is supported with ' . count($avifImages) . ' image(s).');
            return $this->getAnalysisResult(self::ANALYSIS_WEBP_SUPPORT, 1, 0, 0, 0);
        } else {
            $this->status->addWarningToSummary($summaryAplCode, 'No WebP image found on the website.');
        }

        return $this->getAnalysisResult(self::ANALYSIS_WEBP_SUPPORT, count($webpImages), 0, $webpImages ? 0 : 1, 0);
    }

    /**
     * @param VisitedUrl[] $urls
     * @return array
     */
    private function checkAvifSupport(array $urls): array
    {
        $summaryAplCode = 'avif-support';
        $avifImages = array_filter($urls, function (VisitedUrl $url) {
            return $url->contentTypeHeader === 'image/avif';
        });

        if ($avifImages) {
            $this->status->addOkToSummary($summaryAplCode, count($avifImages) . ' AVIF image(s) found on the website.');
        } else {
            $this->status->addWarningToSummary($summaryAplCode, 'No AVIF image found on the website.');
        }

        return $this->getAnalysisResult(self::ANALYSIS_AVIF_SUPPORT, count($avifImages), 0, $avifImages ? 0 : 1, 0);
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
            $this->status->addOkToSummary('pages-with-missing-quotes', "All pages have quoted attributes");
        }

        // inline SVGs
        if ($this->pagesWithLargeSvgs > 0) {
            $this->status->addWarningToSummary('pages-with-large-svgs', "{$this->pagesWithLargeSvgs} page(s) with large inline SVGs (> {$this->maxInlineSvgSize} bytes)");
        } else {
            $this->status->addOkToSummary('pages-with-large-svgs', "All pages have inline SVGs smaller than {$this->maxInlineSvgSize} bytes");
        }

        if ($this->pagesWithDuplicatedSvgs > 0) {
            $this->status->addWarningToSummary('pages-with-duplicated-svgs', "{$this->pagesWithDuplicatedSvgs} page(s) with duplicated inline SVGs (> {$this->maxInlineSvgDuplicates} duplicates)");
        } else {
            $this->status->addOkToSummary('pages-with-duplicated-svgs', "All pages have inline SVGs with less than {$this->maxInlineSvgDuplicates} duplicates");
        }

        if ($this->pagesWithInvalidSvgs > 0) {
            $this->status->addWarningToSummary('pages-with-invalid-svgs', "{$this->pagesWithInvalidSvgs} page(s) with invalid inline SVGs");
        } else {
            $this->status->addOkToSummary('pages-with-invalid-svgs', "All pages have valid or none inline SVGs");
        }

        // heading structure
        if ($this->pagesWithMultipleH1 > 0) {
            $this->status->addCriticalToSummary('pages-with-multiple-h1', "{$this->pagesWithMultipleH1} page(s) with multiple <h1> headings");
        } else {
            $this->status->addOkToSummary('pages-with-multiple-h1', "All pages without multiple <h1> headings");
        }

        if ($this->pagesWithoutH1 > 0) {
            $this->status->addCriticalToSummary('pages-without-h1', "{$this->pagesWithoutH1} page(s) without <h1> heading");
        } else {
            $this->status->addOkToSummary('pages-without-h1', "All pages have <h1> heading");
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

        // non-clickable phone numbers
        if ($this->pagesWithNonClickablePhoneNumbers > 0) {
            $this->status->addWarningToSummary('pages-with-non-clickable-phone-numbers', "{$this->pagesWithNonClickablePhoneNumbers} page(s) with non-clickable (non-interactive) phone numbers");
        } else {
            $this->status->addOkToSummary('pages-with-non-clickable-phone-numbers', "All pages have clickable (interactive) phone numbers");
        }
    }

    public static function getAnalysisNames(): array
    {
        return [
            self::ANALYSIS_LARGE_SVGS,
            self::ANALYSIS_DUPLICATED_SVGS,
            self::ANALYSIS_INVALID_SVGS,
            self::ANALYSIS_MISSING_QUOTES,
            self::ANALYSIS_DOM_DEPTH,
            self::ANALYSIS_HEADING_STRUCTURE,
            self::ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS,
            self::ANALYSIS_TITLE_UNIQUENESS,
            self::ANALYSIS_DESCRIPTION_UNIQUENESS,
        ];
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