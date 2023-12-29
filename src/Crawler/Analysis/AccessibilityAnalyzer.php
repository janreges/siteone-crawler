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
use DOMElement;
use DOMNode;
use DOMXPath;

class AccessibilityAnalyzer extends BaseAnalyzer implements Analyzer
{
    const ANALYSIS_VALID_HTML = 'Valid HTML';
    const ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES = 'Missing image alt attributes';
    const ANALYSIS_MISSING_FORM_LABELS = 'Missing form labels';
    const ANALYSIS_MISSING_ARIA_LABELS = 'Missing aria labels';
    const ANALYSIS_MISSING_ROLES = 'Missing roles';
    const ANALYSIS_MISSING_LANG_ATTRIBUTE = 'Missing html lang attribute';

    const SUPER_TABLE_ACCESSIBILITY = 'accessibility';

    // stats
    private readonly AnalyzerStats $stats;

    private int $pagesWithInvalidHtml = 0;
    private int $pagesWithoutImageAltAttributes = 0;
    private int $pagesWithoutFormLabels = 0;
    private int $pagesWithoutAriaLabels = 0;
    private int $pagesWithoutRoles = 0;
    private int $pagesWithoutLang = 0;

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
        $superTable = new SuperTable(
            self::SUPER_TABLE_ACCESSIBILITY,
            "Accessibility",
            "Nothing to report.",
            [
                new SuperTableColumn('analysisName', 'Analysis name', SuperTableColumn::AUTO_WIDTH),
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

        $superTable->setData($this->stats->toTableArray());
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        $this->setFindingsToSummary();
    }

    /**
     * Analyze HTML URLs for accessibility - return URL analysis result with all findings
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
        $isHtml = $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $visitedUrl->statusCode === 200;

        if ($isHtml && $body && $dom && $visitedUrl->isAllowedForCrawling) {
            $result = new UrlAnalysisResult();

            $s = microtime(true);
            $this->checkImageAltAttributes($body, $result);
            $this->measureExecTime(__CLASS__, 'checkImageAltAttributes', $s);

            $s = microtime(true);
            $this->checkMissingLabels($dom, $result);
            $this->measureExecTime(__CLASS__, 'checkMissingLabels', $s);

            $s = microtime(true);
            $this->checkMissingAriaLabels($dom, $result);
            $this->measureExecTime(__CLASS__, 'checkMissingAriaLabels', $s);

            $s = microtime(true);
            $this->checkMissingRoles($dom, $result);
            $this->measureExecTime(__CLASS__, 'checkMissingRoles', $s);

            $s = microtime(true);
            $this->checkMissingLang($dom, $result);
            $this->measureExecTime(__CLASS__, 'checkMissingLang', $s);
        }

        return $result;
    }

    /**
     * Check if images have an 'alt' attribute. Missing 'alt' attributes can cause accessibility issues.
     * @param string $html
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkImageAltAttributes(string $html, UrlAnalysisResult $result): void
    {
        $badImages = [];
        $foundImagesCount = 0;
        if (preg_match_all('/<img[^>]+>/i', $html, $matches)) {
            $foundImagesCount = count($matches[0]);
            foreach ($matches[0] as $img) {
                if (stripos($img, ' alt=') === false || stripos($img, ' alt=""') !== false || stripos($img, " alt=''") !== false) {
                    $badImages[] = $img;
                    $this->stats->addWarning(self::ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, $img);
                } else {
                    $this->stats->addOk(self::ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, $img);
                }
            }
        }

        if ($badImages) {
            $result->addWarning(count($badImages) . " image(s) without 'alt' attribute", self::ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, $badImages);
            $this->pagesWithoutImageAltAttributes++;
        } else {
            $result->addOk("All {$foundImagesCount} image(s) have an 'alt' attribute", self::ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES);
        }
    }

    /**
     * @return ExtraColumn|null
     */
    public function showAnalyzedVisitedUrlResultAsColumn(): ?ExtraColumn
    {
        return new ExtraColumn('Access.', 8, false);
    }

    /**
     * Check if form inputs have associated 'label' elements.
     * Missing 'label' elements can cause accessibility issues.
     *
     * @param DOMDocument $dom
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMissingLabels(DOMDocument $dom, UrlAnalysisResult $result): void
    {
        $xpath = new DOMXPath($dom);

        $inputsWithoutLabels = [];

        // Find all input elements that are not of type 'hidden'
        $inputs = @$xpath->query("//input[not(@type='hidden')]");

        foreach ($inputs ?: [] as $input) {
            /** @var DOMNode $input */
            $id = $input->attributes->getNamedItem('id');
            $inputHtml = $dom->saveHTML($input);

            // remove all content after the first opening tag (it is not needed for the analysis)
            $inputHtml = preg_replace('/^(<[^>]+>).+$/s', '$1', $inputHtml);

            // If the input has an id, check for a label with a 'for' attribute that matches the id
            if ($id) {
                $label = @$xpath->query("//label[@for='{$id->nodeValue}']");

                if (!$label || $label->length == 0) {
                    $inputsWithoutLabels[] = $inputHtml;
                    $this->stats->addWarning(self::ANALYSIS_MISSING_FORM_LABELS, $inputHtml);
                }
            } else {
                $inputsWithoutLabels[] = $inputHtml;
                $this->stats->addWarning(self::ANALYSIS_MISSING_FORM_LABELS, $inputHtml);
            }
        }

        if ($inputsWithoutLabels) {
            $result->addWarning(count($inputsWithoutLabels) . " input(s) without associated <label>", self::ANALYSIS_MISSING_FORM_LABELS, $inputsWithoutLabels);
            $this->pagesWithoutFormLabels++;
        } elseif (count($inputs) > 0) {
            $result->addOk("All " . count($inputs) . " input(s) have associated 'label'", self::ANALYSIS_MISSING_FORM_LABELS);
        }
    }

    /**
     * Check if certain key interactive elements have 'aria-label' or 'aria-labelledby' attributes defined for accessibility.
     *
     * @param DOMDocument $dom
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMissingAriaLabels(DOMDocument $dom, UrlAnalysisResult $result): void
    {
        $xpath = new DOMXPath($dom);

        $criticalElementsWithoutAriaLabels = [];
        $criticalElements = ['input', 'select', 'textarea'];
        foreach ($criticalElements as $elementName) {
            $elements = @$xpath->query("//{$elementName}");

            foreach ($elements ?: [] as $element) {
                /* @var $element DOMElement */
                $elementHtml = $dom->saveHTML($element);

                // remove all content after the first opening tag (it is not needed for the analysis)
                $elementHtml = preg_replace('/^(<[^>]+>).+$/s', '$1', $elementHtml);
                if (!$element->getAttribute('aria-label') && !$element->getAttribute('aria-labelledby')) {
                    $criticalElementsWithoutAriaLabels[] = $elementHtml;
                    $this->stats->addCritical(self::ANALYSIS_MISSING_ARIA_LABELS, $elementHtml);
                } else {
                    $this->stats->addOk(self::ANALYSIS_MISSING_ARIA_LABELS, $elementHtml);
                }
            }
        }

        $warningElementsWithoutAriaLabels = [];
        $warningElements = ['a', 'button'];
        foreach ($warningElements as $elementName) {
            $elements = @$xpath->query("//{$elementName}");

            foreach ($elements ?: [] as $element) {
                /* @var $element DOMElement */
                $elementHtml = $dom->saveHTML($element);

                // remove all content after the first opening tag (it is not needed for the analysis)
                $elementHtml = preg_replace('/^(<[^>]+>).+$/s', '$1', $elementHtml);
                if (!$element->getAttribute('aria-label') && !$element->getAttribute('aria-labelledby')) {
                    $warningElementsWithoutAriaLabels[] = $elementHtml;
                    $this->stats->addWarning(self::ANALYSIS_MISSING_ARIA_LABELS, $elementHtml);
                } else {
                    $this->stats->addOk(self::ANALYSIS_MISSING_ARIA_LABELS, $elementHtml);
                }
            }
        }

        // set info to result
        if ($criticalElementsWithoutAriaLabels) {
            $result->addCritical(count($criticalElementsWithoutAriaLabels) . " form element(s) without defined 'aria-label' or 'aria-labelledby'", self::ANALYSIS_MISSING_ARIA_LABELS, $criticalElementsWithoutAriaLabels);
        }
        if ($warningElementsWithoutAriaLabels) {
            $result->addWarning(count($warningElementsWithoutAriaLabels) . " element(s) without defined 'aria-label' or 'aria-labelledby'", self::ANALYSIS_MISSING_ARIA_LABELS, $warningElementsWithoutAriaLabels);
        }

        if ($criticalElementsWithoutAriaLabels || $warningElementsWithoutAriaLabels) {
            $this->pagesWithoutAriaLabels++;
        } else {
            $result->addOk("All key interactive element(s) have defined 'aria-label' or 'aria-labelledby'", self::ANALYSIS_MISSING_ARIA_LABELS);
        }
    }

    /**
     * Check if certain key elements have the 'role' attribute defined for accessibility.
     *
     * @param DOMDocument $dom
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMissingRoles(DOMDocument $dom, UrlAnalysisResult $result): void
    {
        $xpath = new DOMXPath($dom);

        $elementsWithoutRoles = [];

        // List of elements to check for roles. You can expand this list based on your requirements.
        $elementsToCheck = ['nav', 'main', 'aside', 'header', 'footer'];

        foreach ($elementsToCheck as $elementName) {
            $elements = @$xpath->query("//{$elementName}[not(@role)]");

            foreach ($elements ?: [] as $element) {
                $elementHtml = $dom->saveHTML($element);

                // remove all content after the first opening tag (it is not needed for the analysis)
                $elementHtml = preg_replace('/^(<[^>]+>).+$/s', '$1', $elementHtml);

                $elementsWithoutRoles[] = $elementHtml;
                $this->stats->addWarning(self::ANALYSIS_MISSING_ROLES, $elementHtml);
            }
        }

        if ($elementsWithoutRoles) {
            $result->addWarning(count($elementsWithoutRoles) . " element(s) without defined 'role'", self::ANALYSIS_MISSING_ROLES, $elementsWithoutRoles);
            $this->pagesWithoutRoles++;
        } else {
            $result->addOk("All key element(s) have defined 'role'", self::ANALYSIS_MISSING_ROLES);
        }
    }

    /**
     * Check if the document has the 'lang' attribute defined.
     *
     * @param DOMDocument $dom
     * @param UrlAnalysisResult $result
     * @return void
     */
    private function checkMissingLang(DOMDocument $dom, UrlAnalysisResult $result): void
    {
        $htmlElement = $dom->getElementsByTagName("html")->item(0);

        if ($htmlElement && $htmlElement->hasAttribute('lang')) {
            $langValue = $htmlElement->getAttribute('lang');
            $elementHtml = '<html lang="' . $langValue . '">';
            if (empty($langValue)) {
                $result->addCritical("The 'lang' attribute is present in <html> but empty.", self::ANALYSIS_MISSING_LANG_ATTRIBUTE, ["HTML lang attribute value is empty ''."]);
                $this->stats->addCritical(self::ANALYSIS_MISSING_LANG_ATTRIBUTE, $elementHtml);
                $this->pagesWithoutLang++;
            } else {
                $result->addOk("Document has defined 'lang' attribute as '{$langValue}'.", self::ANALYSIS_MISSING_LANG_ATTRIBUTE);
                $this->stats->addOk(self::ANALYSIS_MISSING_LANG_ATTRIBUTE, $elementHtml);
            }
        } else {
            $result->addCritical("Document does not have a defined 'lang' attribute in <html>.", self::ANALYSIS_MISSING_LANG_ATTRIBUTE, ["HTML lang attribute is not present."]);
            $this->stats->addCritical(self::ANALYSIS_MISSING_LANG_ATTRIBUTE, '<html>');
            $this->pagesWithoutLang++;
        }
    }

    /**
     * Set findings to summary
     *
     * @return void
     */
    private function setFindingsToSummary(): void
    {
        // pages with invalid HTML
        if ($this->pagesWithInvalidHtml > 0) {
            $this->status->addCriticalToSummary('pages-with-invalid-html', "{$this->pagesWithInvalidHtml} page(s) with invalid HTML");
        } else {
            $this->status->addOkToSummary('pages-with-invalid-html', "All pages have valid HTML");
        }

        // image alt attributes
        if ($this->pagesWithoutImageAltAttributes > 0) {
            $this->status->addWarningToSummary('pages-without-image-alt-attributes', "{$this->pagesWithoutImageAltAttributes} page(s) without image alt attributes");
        } else {
            $this->status->addOkToSummary('pages-without-image-alt-attributes', "All pages have image alt attributes");
        }

        // pages without form labels
        if ($this->pagesWithoutFormLabels > 0) {
            $this->status->addWarningToSummary('pages-without-form-labels', "{$this->pagesWithoutFormLabels} page(s) without form labels");
        } else {
            $this->status->addOkToSummary('pages-without-form-labels', "All pages have form labels");
        }

        // pages without aria labels
        if ($this->pagesWithoutAriaLabels > 0) {
            $this->status->addWarningToSummary('pages-without-aria-labels', "{$this->pagesWithoutAriaLabels} page(s) without aria labels");
        } else {
            $this->status->addOkToSummary('pages-without-aria-labels', "All pages have aria labels");
        }

        // pages without roles
        if ($this->pagesWithoutRoles > 0) {
            $this->status->addWarningToSummary('pages-without-roles', "{$this->pagesWithoutRoles} page(s) without role attributes");
        } else {
            $this->status->addOkToSummary('pages-without-roles', "All pages have role attributes");
        }

        // pages without lang
        if ($this->pagesWithoutLang > 0) {
            $this->status->addCriticalToSummary('pages-without-lang', "{$this->pagesWithoutLang} page(s) without lang attribute");
        } else {
            $this->status->addOkToSummary('pages-without-lang', "All pages have lang attribute");
        }
    }

    public function getOrder(): int
    {
        return 175;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }

    public static function getAnalysisNames(): array
    {
        return [
            self::ANALYSIS_VALID_HTML,
            self::ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES,
            self::ANALYSIS_MISSING_FORM_LABELS,
            self::ANALYSIS_MISSING_ARIA_LABELS,
            self::ANALYSIS_MISSING_ROLES,
            self::ANALYSIS_MISSING_LANG_ATTRIBUTE,
        ];
    }


}