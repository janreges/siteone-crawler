<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Analysis\AccessibilityAnalyzer;
use Crawler\Analysis\CachingAnalyzer;
use Crawler\Analysis\Manager;
use Crawler\Analysis\BestPracticeAnalyzer;
use Crawler\Analysis\ContentTypeAnalyzer;
use Crawler\Analysis\DnsAnalyzer;
use Crawler\Analysis\FastestAnalyzer;
use Crawler\Analysis\HeadersAnalyzer;
use Crawler\Analysis\Page404Analyzer;
use Crawler\Analysis\RedirectsAnalyzer;
use Crawler\Analysis\Result\SeoAndOpenGraphResult;
use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Analysis\SecurityAnalyzer;
use Crawler\Analysis\SeoAndOpenGraphAnalyzer;
use Crawler\Analysis\SkippedUrlsAnalyzer;
use Crawler\Analysis\SlowestAnalyzer;
use Crawler\Analysis\SourceDomainsAnalyzer;
use Crawler\Analysis\SslTlsAnalyzer;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\ContentProcessor\Manager as ContentProcessorManager;
use Crawler\Export\HtmlReport\Badge;
use Crawler\Export\HtmlReport\Tab;
use Crawler\FoundUrl;
use Crawler\Result\Status;
use Crawler\Result\Summary\ItemStatus;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use Crawler\Version;

class HtmlReport
{

    const SUPER_TABLE_VISITED_URLS = 'visited-urls';

    public readonly Status $status;
    public readonly int $maxExampleUrls;
    private readonly array $skippedSuperTables;
    private readonly ?array $allowedSections;

    /**
     * @var int[]
     */
    private static $severityOrder = [
        'critical' => 1,
        'warning' => 2,
        'notice' => 3,
        'ok' => 4,
        'info' => 5
    ];

    public function __construct(Status $status, int $maxExampleUrls = 5, ?string $htmlReportOptions = null)
    {
        $this->status = $status;
        $this->maxExampleUrls = $maxExampleUrls;
        $this->skippedSuperTables = [
            Manager::SUPER_TABLE_ANALYSIS_STATS, // will be in tab Crawler info
            HeadersAnalyzer::SUPER_TABLE_HEADERS_VALUES, // will be in tab Headers
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO, // will be in tab SEO and OpenGraph
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_OPEN_GRAPH, // will be in tab SEO and OpenGraph
            DnsAnalyzer::SUPER_TABLE_DNS, // will be in tab DNS and SSL/TLS
            SslTlsAnalyzer::SUPER_TABLE_CERTIFICATE_INFO, // will be in tab DNS and SSL/TLS
            BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_TITLES, // will be in tab SEO and OpenGraph
            BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS, // will be in tab SEO and OpenGraph
            ContentTypeAnalyzer::SUPER_TABLE_CONTENT_MIME_TYPES, // will be in tab Content Types
            SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED, // will be in tab Skipped URLs
            CachingAnalyzer::SUPER_TABLE_CACHING_PER_DOMAIN, // will be in tab Caching
            CachingAnalyzer::SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE, // will be in tab Caching
            ContentProcessorManager::SUPER_TABLE_CONTENT_PROCESSORS_STATS, // will be in tab Crawler stats
        ];
        
        // Parse allowed sections from options
        if ($htmlReportOptions !== null && $htmlReportOptions !== '') {
            $sections = array_map('trim', explode(',', $htmlReportOptions));
            $this->allowedSections = array_filter($sections);
        } else {
            $this->allowedSections = null; // null means all sections are allowed
        }
    }

    public function getHtml(): string
    {
        $html = $this->getTemplate();
        $templateVariables = $this->getTemplateVariables();

        foreach ($templateVariables as $variableName => $variableValue) {
            $html = str_replace('{$' . $variableName . '}', $variableValue, $html);
        }

        return $this->finalizeHtml($html);
    }

    private function getTemplate(): string
    {
        return file_get_contents(__DIR__ . '/HtmlReport/template.html');
    }

    private function finalizeHtml(string $html): string
    {
        // add badges to colored spans
        $html = preg_replace('/(<span)\s+(style="background-color:[^"]+">)/i', '$1 class="badge" $2', $html);
        $html = preg_replace('/(<span)\s+(style="color:[^"]+">)/i', '$1 class="badge in-table" $2', $html);
        $html = str_replace('style="background-color: #ffff00"', 'style="background-color: #ffff00; color: #1F2937"', $html);
        $html = preg_replace("/(<td data-value='[0-9]+'[^>]*>)([0-9\-]+)(<\/td>)/", '$1<span class="badge">$2</span>$3', $html);

        // other changes
        $html = str_replace('color: #ff00ff', 'color: #ff9234', $html); // change magenta to orange
        $html = preg_replace_callback('/(<td[^>]*>)(\s*[a-z0-9. \/]+\/[a-z0-9. \/]+\s*)(<\/td>)/i', function ($matches) {
            $result = $matches[1] . preg_replace('/\s*\/\s*/', ' / ', $matches[2]) . $matches[3];
            return $result;
        }, $html);

        // badges
        $html = str_replace(
            [
                '<span class="badge in-table" style="color: #00ff00">',
                '<span class="badge in-table" style="color: #ff9234">',
                '<span class="badge in-table" style="color: #ff0000">',
                '<span class="badge in-table" style="background-color: #ffff00; color: #1F2937">',
                '<span class="badge" style="background-color: #ffff00; color: #1F2937">',
                '<span class="badge in-table" style="color: #ffff00">',
                '<span class="badge in-table" style="color: #0000ff">',
            ],
            [
                '<span class="badge green">',
                '<span class="badge orange">',
                '<span class="badge red">',
                '<span class="badge yellow">',
                '<span class="badge yellow">',
                '<span class="badge yellow">',
                '<span class="badge blue">',
            ],
            $html
        );

        $html = Utils::removeWhitespacesFromHtml($html);

        return $html;
    }

    /**
     * @return array<string, string>
     */
    private function getTemplateVariables(): array
    {
        $host = parse_url($this->status->getOptions()->url, PHP_URL_HOST);
        $info = $this->status->getCrawlerInfo();
        $tabs = $this->getTabs();

        return [
            'initialHost' => $host,
            'initialUrl' => $this->status->getOptions()->url,
            'version' => Version::CODE,
            'executedAt' => $info->executedAt,
            'command' => preg_replace('/^\S+\.php\s+/i', '', Utils::getSafeCommand($info->command)),
            'hostname' => $info->hostname,
            'userAgent' => $info->finalUserAgent,
            'tabs' => $this->getTabsHtml($tabs),
            'tabsRadios' => $this->getTabsRadios($tabs),
            'tabsContent' => $this->getTabsContentHtml($tabs),
            'tabsCss' => $this->getTabsCss($tabs),
        ];
    }

    /**
     * Map SuperTable aplCode to section name for filtering
     */
    private function getSectionNameBySuperTableAplCode(string $aplCode): ?string
    {
        $mapping = [
            // Accessibility
            'accessibility' => 'accessibility',
            
            // 404 Pages
            '404' => '404-pages',
            
            // Source Domains
            'source-domains' => 'source-domains',
            
            // Caching
            'caching-per-content-type' => 'caching',
            'caching-per-domain' => 'caching',
            'caching-per-domain-and-content-type' => 'caching',
            
            // Headers
            'headers' => 'headers',
            'headers-values' => 'headers',
            
            // Slowest/Fastest URLs
            'slowest-urls' => 'slowest-urls',
            'fastest-urls' => 'fastest-urls',
            
            // Best Practices
            'best-practices' => 'best-practices',
            
            // Skipped URLs
            'skipped-summary' => 'skipped-urls',
            'skipped' => 'skipped-urls',
            
            // Redirects
            'redirects' => 'redirects',
            
            // Security
            'security' => 'security',
            
            // Content Types
            'content-types' => 'content-types',
            'content-types-raw' => 'content-types',
            
            // These are already included in other tabs
            'dns' => 'dns-ssl',
            'certificate-info' => 'dns-ssl',
            'seo' => 'seo-opengraph',
            'open-graph' => 'seo-opengraph',
            'seo-headings' => 'seo-opengraph',
            'non-unique-titles' => 'seo-opengraph',
            'non-unique-descriptions' => 'seo-opengraph',
        ];
        
        return $mapping[$aplCode] ?? null;
    }

    /**
     * Check if a section is allowed based on htmlReportOptions
     */
    private function isSectionAllowed(string $sectionName): bool
    {
        if ($this->allowedSections === null) {
            return true; // All sections allowed if no filter specified
        }
        return in_array($sectionName, $this->allowedSections, true);
    }

    /**
     * @return Tab[]
     */
    private function getTabs(): array
    {
        $tabs = [];
        
        if ($this->isSectionAllowed('summary')) {
            $tabs[] = $this->getSummaryTab();
        }
        if ($this->isSectionAllowed('seo-opengraph')) {
            $tabs[] = $this->getSeoAndOpenGraphTab();
        }
        if ($this->isSectionAllowed('image-gallery')) {
            $tabs[] = $this->getImageGalleryTab();
        }
        if ($this->isSectionAllowed('video-gallery')) {
            $tabs[] = $this->getVideoGalleryTab();
        }
        if ($this->isSectionAllowed('visited-urls')) {
            $tabs[] = $this->getVisitedUrlsTab();
        }
        if ($this->isSectionAllowed('dns-ssl')) {
            $tabs[] = $this->getDnsAndSslTlsTab();
        }
        if ($this->isSectionAllowed('crawler-stats')) {
            $tabs[] = $this->getCrawlerStatsTab();
        }
        if ($this->isSectionAllowed('crawler-info')) {
            $tabs[] = $this->getCrawlerInfo();
        }

        $hostToStripFromUrls = $this->getInitialHost();
        $schemeOfHostToStripFromUrls = $this->status->getOptions()->getInitialScheme();
        $initialUrl = $this->status->getOptions()->url;

        $superTables = array_merge($this->status->getSuperTablesAtBeginning(), $this->status->getSuperTablesAtEnd());
        foreach ($superTables as $superTable) {
            if (in_array($superTable->aplCode, $this->skippedSuperTables, true)) {
                continue;
            }

            // Check if this SuperTable's section is allowed
            $sectionName = $this->getSectionNameBySuperTableAplCode($superTable->aplCode);
            if ($sectionName !== null && !$this->isSectionAllowed($sectionName)) {
                continue; // Skip this SuperTable if its section is not allowed
            }

            // set props used for clickable URLs building
            $superTable->setHostToStripFromUrls($hostToStripFromUrls, $schemeOfHostToStripFromUrls);
            $superTable->setInitialUrl($initialUrl);

            $badges = $this->getSuperTableBadgesByAplCode($superTable);
            if (!$badges) {
                $badges = $this->getSuperTableGenericBadges($superTable);
            }
            $tabs[] = new Tab($superTable->forcedTabLabel ?: $superTable->title, null, $this->getTabContentBySuperTable($superTable), false, $badges, $this->getSuperTableOrder($superTable));
        }

        // unset empty tabs
        $tabs = array_filter($tabs, function (?Tab $tab) {
            return $tab && $tab->tabContent !== '';
        });

        // sort tabs by order
        usort($tabs, function (Tab $a, Tab $b) {
            return $a->getFinalSortOrder() <=> $b->getFinalSortOrder();
        });

        return $tabs;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsRadios(array $tabs): string
    {
        $html = '';

        // hidden radio buttons for tabs - to be able to use :checked selector (HTML+CSS only solution for tabs
        // which works in all browsers and e-
        $isFirst = true;
        foreach ($tabs as $tab) {
            $html .= '<input type="radio" id="' . htmlspecialchars($tab->radioHtmlId) . '" name="tabs" arial-label="Show tab ' . htmlspecialchars($tab->name) . '" class="tabs__radio"' . ($isFirst ? ' checked' : '') . '>' . "\n";
            if ($isFirst) {
                $isFirst = false;
            }
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsHtml(array $tabs): string
    {
        $html = '';

        foreach ($tabs as $tab) {
            $badges = '';
            foreach ($tab->badges as $badge) {
                $badges .= '<span class="badge ' . $badge->color . '"' . ($badge->title ? ' style="cursor: help" title="' . htmlspecialchars($badge->title) . '"' : '') . '>' . htmlspecialchars($badge->value) . '</span> ';
            }

            $html .= '<label for="' . htmlspecialchars($tab->radioHtmlId) . '" class="tabs__title ' . htmlspecialchars($tab->radioHtmlId) . '">' . htmlspecialchars($tab->name) . ($badges ? " {$badges}" : '') . "</label>\n";
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsContentHtml(array $tabs): string
    {
        $html = '';
        $linePrefix = '                ';

        foreach ($tabs as $tab) {
            $html .= $linePrefix . '<div class="tabs__tab ' . htmlspecialchars($tab->contentHtmlId) . '">' . "\n";
            if ($tab->addHeading) {
                $html .= $linePrefix . '    <h2>' . htmlspecialchars($tab->name) . '</h2>' . "\n";
            }
            $html .= $linePrefix . '    ' . str_replace("\n", "\n" . $linePrefix . '    ', $tab->tabContent) . "\n";
            $html .= $linePrefix . '</div>' . "\n";
        }

        return $html;
    }

    /**
     * @param Tab[] $tabs
     * @return string
     */
    private function getTabsCss(array $tabs): string
    {
        $linePrefix = '        ';

        $selectors = [];
        foreach ($tabs as $tab) {
            $selectors[] = '#' . $tab->radioHtmlId . ':checked ~ .tabs__content .' . $tab->contentHtmlId;
        }

        $css = implode(', ', $selectors) . " {\n";
        $css .= $linePrefix . "    display: block;\n";
        $css .= $linePrefix . "}\n";

        // active tab title
        $selectors = [];
        foreach ($tabs as $tab) {
            $selectors[] = '#' . $tab->radioHtmlId . ':checked ~ .tabs__navigation .' . $tab->radioHtmlId;
        }

        $css .= implode(', ', $selectors) . " {\n";
        $css .= $linePrefix . "    background-color: var(--color-blue-600);\n";
        $css .= $linePrefix . "    color: var(--color-white);\n";
        $css .= $linePrefix . "}\n";

        return $css;
    }

    private function getSummaryTab(): ?Tab
    {
        $summary = $this->status->getSummary();
        if (!$summary->getItems()) {
            return null;
        }
        $colorToCount = [
            Badge::COLOR_RED => $summary->getCountByItemStatus(ItemStatus::CRITICAL),
            Badge::COLOR_ORANGE => $summary->getCountByItemStatus(ItemStatus::WARNING),
            Badge::COLOR_BLUE => $summary->getCountByItemStatus(ItemStatus::NOTICE),
            Badge::COLOR_GREEN => $summary->getCountByItemStatus(ItemStatus::OK),
            Badge::COLOR_NEUTRAL => $summary->getCountByItemStatus(ItemStatus::INFO),
        ];

        $badges = [];
        foreach ($colorToCount as $color => $count) {
            if ($count > 0) {
                $badges[] = new Badge((string)$count, $color);
            }
        }

        return new Tab('Summary', null, $summary->getAsHtml(), true, $badges, -100);
    }

    private function getSeoAndOpenGraphTab(): ?Tab
    {
        $html = '';
        $superTables = [
            BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_TITLES,
            BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS,
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO,
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_OPEN_GRAPH,
        ];

        $badgeCount = 0;

        $order = null;
        foreach ($superTables as $superTable) {
            $superTable = $this->status->getSuperTableByAplCode($superTable);
            if ($superTable) {
                $html .= $superTable->getHtmlOutput() . '<br/>';
                if (!$badgeCount) {
                    $badgeCount = $superTable->getTotalRows();
                }
                if ($superTable->aplCode === SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO) {
                    $order = $this->getSuperTableOrder($superTable);
                }
            }
        }

        if (!$html) {
            return null;
        }

        $badges = [];

        $nonUniqueTitles = $this->status->getSuperTableByAplCode(BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_TITLES);
        if ($nonUniqueTitles && $nonUniqueTitles->getTotalRows() > 0) {
            $badges[] = new Badge(strval($nonUniqueTitles->getTotalRows()), Badge::COLOR_ORANGE, 'Non-unique titles');
        }

        $nonUniqueDescriptions = $this->status->getSuperTableByAplCode(BestPracticeAnalyzer::SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS);
        if ($nonUniqueDescriptions && $nonUniqueDescriptions->getTotalRows() > 0) {
            $badges[] = new Badge(strval($nonUniqueDescriptions->getTotalRows()), Badge::COLOR_ORANGE, 'Non-unique descriptions');
        }

        $badges[] = new Badge(strval($badgeCount), Badge::COLOR_NEUTRAL, 'Total URL with SEO info');

        return new Tab('SEO and OpenGraph', null, $html, false, $badges, $order);
    }

    private function getImageGalleryTab(): ?Tab
    {
        $summary = $this->status->getSummary();
        if (!$summary->getItems()) {
            return null;
        }

        $images = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            // only images from img:src (not from srcset)
            if ($visitedUrl->isImage() && $visitedUrl->statusCode === 200 && (in_array($visitedUrl->sourceAttr, [FoundUrl::SOURCE_IMG_SRC, FoundUrl::SOURCE_INPUT_SRC, FoundUrl::SOURCE_CSS_URL]))) {
                $images[] = $visitedUrl;
            }
        }

        if (!$images) {
            return null;
        }

        // igc & igcf containers are used for variable styling (controlled by radio buttons)
        $html = $this->getImageGalleryFormHtml();
        $html .= '<div id="igc" class="small"><div id="igcf" class="scaleDown"><div id="image-gallery" class="image-gallery">';
        foreach ($images as $image) {
            $imageDescription = Utils::getFormattedSize($image->size) . ' (' . $image->contentTypeHeader . ')';
            $imageDescription .= ', found as ' . $image->getSourceDescription($this->status->getUrlByUqId($image->sourceUqId));

            $html .= sprintf(
                '<a href="%s" target="_blank" data-size="%s" data-source="%s" data-type="%s" data-sizematch="1" data-typematch="1" data-sourcematch="1">',
                htmlspecialchars($image->url),
                $image->size,
                $image->getSourceShortName(),
                htmlspecialchars(str_replace('image/', '', $image->contentTypeHeader)
                ));
            $html .= '<img loading="lazy" width="140" height="140" src="' . htmlspecialchars($image->url) . '" alt="' . htmlspecialchars($imageDescription) . '" title="' . htmlspecialchars($imageDescription) . '">';
            $html .= '</a>' . "\n";
        }
        $html .= '</div></div></div>';

        $badges = [new Badge(strval(count($images)), Badge::COLOR_NEUTRAL, 'Found images')];
        return new Tab('Image Gallery', null, $html, true, $badges, 6);
    }

    private function getVideoGalleryTab(): ?Tab
    {
        $summary = $this->status->getSummary();
        if (!$summary->getItems()) {
            return null;
        }

        $videos = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if ($visitedUrl->isVideo() && $visitedUrl->statusCode === 200) {
                $videos[] = $visitedUrl;
            }
        }

        if (!$videos) {
            return null;
        }

        $html = '<button onclick="playVideos()" class="btn">▶ Play the first 2 seconds of each video</button>';
        $html .= '<div id="vgc" class="small"><div id="vgcf" class="scaleDown"><div id="video-gallery" class="video-container">';
        foreach ($videos as $video) {
            $videoDescription = Utils::getFormattedSize($video->size) . ' (' . $video->contentTypeHeader . ')';
            $videoDescription .= sprintf(
                ', <a href="%s" target="_blank">video</a> found on <a href="%s" target="_blank">this page</a>',
                $video->url,
                $this->status->getUrlByUqId($video->sourceUqId)
            );

            $html .= sprintf(
                    '<div class="video-card">
                        <video data-src="%s" preload="metadata" controls></video>
                        <div class="video-caption">%s</div>
                    </div>',
                    htmlspecialchars($video->url),
                    $videoDescription
                ) . "\n";
        }
        $html .= '</div></div></div>';

        $html .= '<script> function playVideos() {
            const videos = document.querySelectorAll("video");
    
            function playVideoSequentially(index) {
                if (index >= videos.length) return;
    
                const video = videos[index];
                video.load();
                video.currentTime = 0;
    
                video.addEventListener("loadeddata", function() {
                    video.play();
    
                    setTimeout(() => {
                        video.pause();
                        setTimeout(() => playVideoSequentially(index + 1), 10);
                    }, 2000);
                }, { once: true });
            }
    
            playVideoSequentially(0);
        }
        
        /* init lazy loading */
        document.addEventListener("DOMContentLoaded", function() {
            const videos = document.querySelectorAll("video");
    
            const observer = new IntersectionObserver(entries => {
                entries.forEach(entry => {
                    if (entry.isIntersecting) {
                        const video = entry.target;
                        if (!video.src) {
                            video.src = video.dataset.src;
                            video.load();
                        }
                        observer.unobserve(video);
                    }
                });
            });
    
            videos.forEach(video => {
                observer.observe(video);
            });
        });
        
        </script>';

        $badges = [new Badge(strval(count($videos)), Badge::COLOR_NEUTRAL, 'Found videos')];
        return new Tab('Video Gallery', null, $html, true, $badges, 6);
    }

    private function getImageGalleryFormHtml(): string
    {
        $html = '
            <style>
            #imageDisplayForm {
                display: flex;
                gap: 12px;
                flex-wrap: wrap;
                margin-bottom: 20px;
            }
            </style>';

        $html .= '<script>
                function updateClassName(elementId, className) {
                    document.getElementById(elementId).className = className;
                    if (elementId === "igc") {
                        var images = document.getElementById(elementId).getElementsByTagName("img");
                        for (var i = 0; i < images.length; i++) {
                            var image = images[i];
                            image.width = className === "small" ? 140 : (className === "medium" ? 200 : 360);
                            image.height = className === "small" ? 140 : (className === "medium" ? 200 : 360);
                        }
                    }
                }
            </script>';

        $html .= "<script> function initializeFilters() {
                const links = document.querySelectorAll('#image-gallery a');
                const types = new Set();
                const sources = new Set();
                const sizeCategories = [
                    { label: 'any', filter: () => true },
                    { label: '> 5 MB', filter: size => size > 5 * 1024 * 1024 },
                    { label: '> 1MB', filter: size => size > 1 * 1024 * 1024 },
                    { label: '> 500kB', filter: size => size > 500 * 1024 },
                    { label: '> 100kB', filter: size => size > 100 * 1024 },
                    { label: '> 10kB', filter: size => size > 10 * 1024 },
                    { label: '< 10kB', filter: size => size < 10 * 1024 }
                ];
            
                links.forEach(link => {
                    types.add(link.dataset.type);
                    sources.add(link.dataset.source);
                });
            
                addSizeFilters('sizeFilters', sizeCategories, links, filterImagesBySize);
                addToggleButtonsToFilter('typeFilters', ['any'].concat(Array.from(types).sort((a, b) => countLinksOfType(b, links) - countLinksOfType(a, links))), filterImagesByType, links);
                addToggleButtonsToFilter('sourceFilters', ['any'].concat(Array.from(sources).sort((a, b) => countLinksOfSource(b, links) - countLinksOfSource(a, links))), filterImagesBySource, links);
            }
            
            function addToggleButtonsToFilter(filterId, categories, filterFunction, links) {
                const filterDiv = document.getElementById(filterId);
                categories.forEach((category, index) => {
                    const radioId = filterId + category;
                    const radioInput = document.createElement('input');
                    radioInput.setAttribute('type', 'radio');
                    radioInput.setAttribute('id', radioId);
                    radioInput.setAttribute('name', filterId);
                    radioInput.setAttribute('value', category);
                    if (category === 'any') {
                        radioInput.setAttribute('checked', 'checked');
                    }
                    radioInput.onchange = () => filterFunction(category);
            
                    const label = document.createElement('label');
                    label.setAttribute('for', radioId);
                    
                    let labelCountText = category;
                    if (category !== 'any') {
                        const count = filterId === 'typeFilters' ? countLinksOfType(category, links) : countLinksOfSource(category, links);
                        labelCountText += ` (\${count})`;
                    } else {
                        labelCountText += ' (' + links.length + ')';
                    }
                    label.textContent = labelCountText;
            
                    filterDiv.appendChild(radioInput);
                    filterDiv.appendChild(label);
                });
            }
            
            function addToggleButton(filterDiv, filterId, value, labelText, filterFunction) {
                const radioId = filterId + '-' + value.replace(/\s/g, '-');
            
                const radioInput = document.createElement('input');
                radioInput.setAttribute('type', 'radio');
                radioInput.setAttribute('id', radioId);
                radioInput.setAttribute('name', filterId);
                radioInput.setAttribute('value', value);
                radioInput.addEventListener('change', () => filterFunction(value));
                
                if (labelText === 'any') {
                    radioInput.setAttribute('checked', 'checked');
                }
            
                const label = document.createElement('label');
                label.setAttribute('for', radioId);
                label.textContent = labelText;
            
                filterDiv.appendChild(radioInput);
                filterDiv.appendChild(label);
            }

            function countLinksOfType(type, links) {
                return Array.from(links).filter(link => link.dataset.type === type).length;
            }
            
            function countLinksOfSource(source, links) {
                return Array.from(links).filter(link => link.dataset.source === source).length;
            }
            
            function doesSizeMatchCategory(size, category) {
                const sizeInKB = size / 1024;
            
                switch (category) {
                    case 'any':
                        return true;
                    case '> 5 MB':
                        return sizeInKB > 5120;
                    case '> 1MB':
                        return sizeInKB > 1024;
                    case '> 500kB':
                        return sizeInKB > 500;
                    case '> 100kB':
                        return sizeInKB > 100;
                    case '> 10kB':
                        return sizeInKB > 10;
                    case '< 10kB':
                        return sizeInKB < 10;
                    default:
                        return false;
                }
            }
            
            function filterImagesByType(selectedType) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (selectedType === 'any' || link.dataset.type === selectedType) {
                        link.dataset.typematch = '1';
                    } else {
                        link.dataset.typematch = '0';
                    }
                });
                filterByMatched();
            }
            
            function filterImagesBySource(selectedSource) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (selectedSource === 'any' || link.dataset.source === selectedSource) {
                        link.dataset.sourcematch = '1';
                    } else {
                        link.dataset.sourcematch = '0';
                    }
                });
                filterByMatched();
            }
            
            function filterImagesBySize(selectedSizeCategory) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    const imageSize = parseInt(link.dataset.size, 10);
            
                    if (doesSizeMatchCategory(imageSize, selectedSizeCategory)) {
                        link.dataset.sizematch = '1';
                    } else {
                        link.dataset.sizematch = '0';
                    }
                });
                filterByMatched();
            }
            
            function addSizeFilters(filterId, categories, links, filterFunction) {
                const filterDiv = document.getElementById(filterId);
                categories.forEach(category => {
                    const count = Array.from(links).filter(link => category.filter(parseInt(link.dataset.size, 10))).length;
                    const labelWithCount = `\${category.label} (\${count})`;
                    if (count > 0) {
                        addToggleButton(filterDiv, filterId, category.label, labelWithCount, filterFunction);
                    }
                });
            }
            
            function filterByMatched() {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (link.dataset.sizematch === '1' && link.dataset.typematch === '1' && link.dataset.sourcematch === '1') {
                        link.style.display = 'inline-block'
                    } else {
                        link.style.display = 'none';
                    }
                });
            }
            
            document.addEventListener('DOMContentLoaded', function() {
                initializeFilters();
            });
            
            </script>";

        $html .= '<form id="imageDisplayForm">
                <div class="form-group">
                    <div class="btn-group">
                        <input class="idf" type="radio" id="sizeSmall" name="thumbnailSize" value="small" data-key="igc" checked>
                        <label for="sizeSmall">small</label>
        
                        <input class="idf" type="radio" id="sizeMedium" name="thumbnailSize" value="medium" data-key="igc">
                        <label for="sizeMedium">medium</label>
        
                        <input class="idf" type="radio" id="sizeLarge" name="thumbnailSize" value="large" data-key="igc">
                        <label for="sizeLarge">large</label>
                    </div>
                </div>
        
                <div class="form-group">
                    <div class="btn-group">
                        <input class="idf" type="radio" id="modeScaleDown" name="thumbnailMode" value="scaleDown" data-key="igcf" checked>
                        <label for="modeScaleDown">scale-down</label>
                        
                        <input class="idf" type="radio" id="modeContain" name="thumbnailMode" value="contain" data-key="igcf">
                        <label for="modeContain">contain</label>
                        
                        <input class="idf" type="radio" id="modeCover" name="thumbnailMode" value="cover" data-key="igcf">
                        <label for="modeCover">cover</label>
                    </div>
                </div>

                <div class="form-group">
                    <div class="btn-group" id="typeFilters">
                        <!-- will be inserted by initializeFilters() -->
                    </div>
                </div>

                <div class="form-group">
                    <div class="btn-group" id="sourceFilters">
                        <!-- will be inserted by initializeFilters() -->
                    </div>
                </div>
                
                <div class="form-group">
                    <div class="btn-group" id="sizeFilters">
                        <!-- will be inserted by initializeFilters() -->
                    </div>
                </div>
                
            </form>';

        return $html;
    }

    private function getDnsAndSslTlsTab(): ?Tab
    {
        $html = '';
        $superTables = [
            DnsAnalyzer::SUPER_TABLE_DNS,
            SslTlsAnalyzer::SUPER_TABLE_CERTIFICATE_INFO
        ];

        $order = null;
        $badges = [];
        foreach ($superTables as $superTable) {
            $superTable = $this->status->getSuperTableByAplCode($superTable);
            if ($superTable) {
                $html .= $superTable->getHtmlOutput() . '<br/>';
                if ($superTable->aplCode === DnsAnalyzer::SUPER_TABLE_DNS) {
                    $order = $this->getSuperTableOrder($superTable);
                    $ipv4 = 0;
                    $ipv6 = 0;
                    foreach ($superTable->getData() as $row) {
                        if (stripos($row['info'], 'IPv4') !== false) {
                            $ipv4++;
                        } elseif (stripos($row['info'], 'IPv6') !== false) {
                            $ipv6++;
                        }
                    }

                    if ($ipv4) {
                        $badges[] = new Badge("{$ipv4}x IPv4", $ipv4 > 1 ? Badge::COLOR_GREEN : Badge::COLOR_NEUTRAL);
                    }
                    if ($ipv6) {
                        $badges[] = new Badge("{$ipv6}x IPv6", $ipv6 > 1 ? Badge::COLOR_GREEN : Badge::COLOR_NEUTRAL);
                    }
                } elseif ($superTable->aplCode === SslTlsAnalyzer::SUPER_TABLE_CERTIFICATE_INFO) {
                    $errors = 0;
                    foreach ($superTable->getData() as $row) {
                        if ($row['info'] === 'Errors' && is_array($row['value'])) {
                            $errors += count($row['value']);
                        }
                    }
                    $badges[] = new Badge("TLS", $errors ? Badge::COLOR_RED : Badge::COLOR_GREEN, $errors ? "SSL/TLS certificate: {$errors} error(s)" : 'SSL/TLS certificate OK');
                }
            }
        }

        if (!$html) {
            return null;
        }

        return new Tab('DNS and SSL', null, $html, false, $badges, $order);
    }

    private function getCrawlerStatsTab(): Tab
    {
        $html = '';

        $stats = $this->status->getBasicStats();
        $badges = [
            new Badge(strval($stats->totalUrls), Badge::COLOR_NEUTRAL, 'Total visited URLs'),
            new Badge($stats->totalSizeFormatted, Badge::COLOR_NEUTRAL, 'Total size of all visited URLs'),
            new Badge(Utils::getFormattedDuration($stats->totalExecutionTime), Badge::COLOR_NEUTRAL, 'Total execution time'),
        ];

        $html .= $stats->getAsHtml();

        $analysisStats = $this->status->getSuperTableByAplCode(Manager::SUPER_TABLE_ANALYSIS_STATS);
        if ($analysisStats) {
            $html .= '<br/>' . $analysisStats->getHtmlOutput();
        }

        $contentProcessorsStats = $this->status->getSuperTableByAplCode(ContentProcessorManager::SUPER_TABLE_CONTENT_PROCESSORS_STATS);
        if ($contentProcessorsStats) {
            $html .= '<br/>' . $contentProcessorsStats->getHtmlOutput();
        }

        return new Tab('Crawler stats', null, $html, true, $badges, 900);
    }

    private function getCrawlerInfo(): Tab
    {
        $html = '
            <h2>Crawler info</h2>
            <div class="info__wrapper">
                <table style="border-collapse: collapse;">
                    <tr>
                        <th>Version</th>
                        <td>{$version}</td>
                    </tr>
                    <tr>
                        <th>Executed At</th>
                        <td>{$executedAt}</td>
                    </tr>
                    <tr>
                        <th>Command</th>
                        <td>{$command}</td>
                    </tr>
                    <tr>
                        <th>Hostname</th>
                        <td>{$hostname}</td>
                    </tr>
                    <tr>
                        <th>User-Agent</th>
                        <td>{$userAgent}</td>
                    </tr>
                </table>
            </div>';

        $html = str_replace(
            ['{$version}', '{$executedAt}', '{$command}', '{$hostname}', '{$userAgent}'],
            [
                $this->status->getCrawlerInfo()->version,
                $this->status->getCrawlerInfo()->executedAt,
                Utils::getSafeCommand($this->status->getCrawlerInfo()->command),
                $this->status->getCrawlerInfo()->hostname,
                $this->status->getCrawlerInfo()->finalUserAgent
            ], $html
        );

        $badges = [new Badge('v' . Version::CODE, Badge::COLOR_NEUTRAL, 'Crawler version')];
        return new Tab('Crawler info', null, $html, false, $badges, 5000);
    }

    private function getVisitedUrlsTab(): Tab
    {
        $visitedUrlsTable = $this->getVisitedUrlsTable();
        $visitedUrlsTable->setHostToStripFromUrls($this->getInitialHost(), $this->status->getOptions()->getInitialScheme());
        $badges = $this->getSuperTableBadgesByAplCode($visitedUrlsTable);
        return new Tab($visitedUrlsTable->title, $visitedUrlsTable->description, $visitedUrlsTable->getHtmlOutput(), false, $badges, $this->getSuperTableOrder($visitedUrlsTable));
    }

    private function getVisitedUrlsTable(): SuperTable
    {
        $initialHost = $this->getInitialHost();
        $schemeOfInitialHost = $this->status->getOptions()->getInitialScheme();

        $cacheLifetimeDataValueCallback = function ($row) {
            $cacheLifetime = $row['cacheLifetime'];
            $cacheTypeFlags = $row['cacheTypeFlags'];
            if ($cacheLifetime === null) {
                if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_NO_STORE) {
                    return -2;
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_NO_CACHE) {
                    return -1;
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_ETAG) {
                    return 0.1;
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_LAST_MODIFIED) {
                    return 0.2;
                } else {
                    return 0.01; // 0.01 because exact 0 is used by explicit max-age=0 and s-maxage=0 ... and 'None' for this case have to be next to '0s'
                }
            } else {
                return intval($cacheLifetime);
            }
        };

        // setup columns
        $columns = [
            new SuperTableColumn('url', 'URL', SuperTableColumn::AUTO_WIDTH, null, function ($row) use ($initialHost, $schemeOfInitialHost) {
                return '<a href="' . htmlspecialchars($row['url']) . '" target="_blank">' . Utils::truncateUrl($row['url'], 80, '…', $initialHost, $schemeOfInitialHost) . '</a>';
            }),
            new SuperTableColumn('status', 'Status', 6, function ($value) {
                return Utils::getColoredStatusCode($value);
            }),
            new SuperTableColumn('type', 'Type', 8),
            new SuperTableColumn('time', 'Time (s)', 8, null, function ($row) {
                return Utils::getColoredRequestTime($row['time'], 6);
            }),
            new SuperTableColumn('size', 'Size', 8, null, function ($row) {
                if ($row['size'] > 1024 * 1024) {
                    return Utils::getColorText($row['sizeFormatted'], 'red', true);
                } else {
                    return $row['sizeFormatted'];
                }
            }),
            new SuperTableColumn('cacheLifetime', 'Cache', 8, null, function ($row) {
                $strPadTo = 6;
                $cacheLifetime = $row['cacheLifetime'];
                $cacheTypeFlags = $row['cacheTypeFlags'];
                if ($cacheLifetime !== null) {
                    return Utils::getColoredCacheLifetime($cacheLifetime, $strPadTo);
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_NO_STORE) {
                    return Utils::getColorText(str_pad('0s (no-store)', $strPadTo), 'red', true);
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_NO_CACHE) {
                    return Utils::getColorText(str_pad('0s (no-cache)', $strPadTo), 'red');
                } else if (($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_ETAG)) {
                    return Utils::getColorText(str_pad('ETag-only', $strPadTo), 'magenta');
                } else if ($cacheTypeFlags & VisitedUrl::CACHE_TYPE_HAS_LAST_MODIFIED) {
                    return Utils::getColorText(str_pad('Last-Mod-only', $strPadTo), 'magenta');
                } else {
                    return Utils::getColorText(str_pad('None', $strPadTo), 'red');
                }
            }, false, true, false, true, $cacheLifetimeDataValueCallback),
        ];


        foreach ($this->status->getOptions()->extraColumns as $extraColumn) {
            $columns[] = new SuperTableColumn($extraColumn->name, $extraColumn->name, $extraColumn->length ?: SuperTableColumn::AUTO_WIDTH);
        }

        // setup supertable
        $superTable = new SuperTable(self::SUPER_TABLE_VISITED_URLS, 'Visited URLs', 'No visited URLs.', $columns, false);
        $superTable->setIgnoreHardRowsLimit(true);
        $superTable->getColumns()['cacheLifetime']->forcedDataType = 'number';

        // set data
        $data = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if ($visitedUrl->statusCode === VisitedUrl::ERROR_SKIPPED) {
                continue;
            }
            $row = [
                'url' => $visitedUrl->url,
                'status' => $visitedUrl->statusCode,
                'type' => Utils::getContentTypeNameById($visitedUrl->contentType),
                'time' => $visitedUrl->requestTime,
                'size' => $visitedUrl->size,
                'sizeFormatted' => $visitedUrl->sizeFormatted,
                'cacheTypeFlags' => $visitedUrl->cacheTypeFlags,
                'cacheLifetime' => $visitedUrl->cacheLifetime,
            ];

            if ($visitedUrl->extras) {
                $row = array_merge($row, $visitedUrl->extras);
            }

            $data[] = $row;
        }
        $superTable->setData($data);

        return $superTable;
    }


    /**
     * @param SuperTable $superTable
     * @return Badge[]
     */
    private function getSuperTableBadgesByAplCode(SuperTable $superTable): array
    {
        $badges = [];

        switch ($superTable->aplCode) {
            case RedirectsAnalyzer::SUPER_TABLE_REDIRECTS:
                $redirects = $superTable->getTotalRows();
                $color = $redirects > 100 ? Badge::COLOR_RED : ($redirects > 0 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                $badges[] = new Badge((string)$redirects, $color);
                break;
            case Page404Analyzer::SUPER_TABLE_404:
                $notFound = $superTable->getTotalRows();
                $color = $notFound > 10 ? Badge::COLOR_RED : ($notFound > 0 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                $badges[] = new Badge((string)$notFound, $color);
                break;
            case SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED_SUMMARY:
                $skipped = $superTable->getTotalRows();
                $color = $skipped > 100 ? Badge::COLOR_ORANGE : ($skipped > 10 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                $badges[] = new Badge((string)$skipped, $color, 'Skipped URL domains');
                $superTableSkippedUrls = $this->status->getSuperTableByAplCode(SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED);
                if ($superTableSkippedUrls) {
                    $badges[] = new Badge((string)$superTableSkippedUrls->getTotalRows(), Badge::COLOR_NEUTRAL, 'Total skipped URLs');
                }
                break;
            case SourceDomainsAnalyzer::SUPER_TABLE_SOURCE_DOMAINS:
                $domains = $superTable->getTotalRows();
                $color = $domains > 10 ? Badge::COLOR_ORANGE : Badge::COLOR_NEUTRAL;
                $badges[] = new Badge((string)$domains, $color);
                break;
            case ContentTypeAnalyzer::SUPER_TABLE_CONTENT_TYPES:
                $contentTypes = $superTable->getTotalRows();
                $badges[] = new Badge((string)$contentTypes, Badge::COLOR_NEUTRAL, 'Total content types');
                $superTableMimeTypes = $this->status->getSuperTableByAplCode(ContentTypeAnalyzer::SUPER_TABLE_CONTENT_MIME_TYPES);
                if ($superTableMimeTypes) {
                    $badges[] = new Badge((string)$superTableMimeTypes->getTotalRows(), Badge::COLOR_NEUTRAL, 'Total MIME types');
                }
                break;
            case FastestAnalyzer::SUPER_TABLE_FASTEST_URLS:
                $fastestTime = null;
                foreach ($superTable->getData() as $row) {
                    if ($fastestTime === null) {
                        $fastestTime = $row->requestTime;
                    } else {
                        $fastestTime = min($fastestTime, $row->requestTime);
                    }
                }
                $color = $fastestTime < 0.5 ? Badge::COLOR_GREEN : ($fastestTime < 2 ? Badge::COLOR_ORANGE : Badge::COLOR_RED);
                $badges[] = new Badge(Utils::getFormattedDuration($fastestTime ?: 0), $color);
                break;
            case SlowestAnalyzer::SUPER_TABLE_SLOWEST_URLS:
                $slowestTime = null;
                foreach ($superTable->getData() as $row) {
                    if ($slowestTime === null) {
                        $slowestTime = $row->requestTime;
                    } else {
                        $slowestTime = max($slowestTime, $row->requestTime);
                    }
                }
                $color = $slowestTime < 0.5 ? Badge::COLOR_GREEN : ($slowestTime < 2 ? Badge::COLOR_ORANGE : Badge::COLOR_RED);
                $badges[] = new Badge(Utils::getFormattedDuration($slowestTime ?: 0), $color);
                break;
            case SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO_HEADINGS:
                $ok = 0;
                $errors = 0;
                foreach ($superTable->getData() as $row) {
                    /* @var SeoAndOpenGraphResult $row */
                    if ($row->headingsErrorsCount > 0) {
                        $errors++;
                    } else {
                        $ok++;
                    }
                }
                $badges[] = new Badge((string)$ok, Badge::COLOR_GREEN, "Pages with proper heading structure");
                if ($errors) {
                    $badges[] = new Badge((string)$errors, Badge::COLOR_RED, "Pages with errors in heading structure");
                }
                break;
            case HeadersAnalyzer::SUPER_TABLE_HEADERS:
                $headers = $superTable->getTotalRows();
                $color = $headers > 50 ? Badge::COLOR_RED : Badge::COLOR_NEUTRAL;
                $badges[] = new Badge((string)$headers, $color);
                break;
            case CachingAnalyzer::SUPER_TABLE_CACHING_PER_CONTENT_TYPE:
                $minCacheLifetime = null;
                $maxCacheLifetime = null;
                foreach ($superTable->getData() as $row) {
                    if (!in_array($row['contentType'], ['Image', 'CSS', 'JS', 'Font'])) {
                        continue;
                    }

                    if ($minCacheLifetime === null && $row['minLifetime'] !== null) {
                        $minCacheLifetime = $row['minLifetime'];
                    } elseif ($row['minLifetime'] !== null) {
                        $minCacheLifetime = min($minCacheLifetime, $row['minLifetime']);
                    }

                    if ($maxCacheLifetime === null && $row['maxLifetime'] !== null) {
                        $maxCacheLifetime = $row['maxLifetime'];
                    } elseif ($row['maxLifetime'] !== null) {
                        $maxCacheLifetime = max($maxCacheLifetime, $row['maxLifetime']);
                    }
                }

                if ($minCacheLifetime !== null) {
                    $color = $minCacheLifetime < 60 ? Badge::COLOR_RED : ($minCacheLifetime < 3600 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                    $badges[] = new Badge(Utils::getFormattedCacheLifetime($minCacheLifetime), $color, 'Minimal cache lifetime for images/css/js/fonts');
                }
                if ($maxCacheLifetime !== null) {
                    $color = $maxCacheLifetime < 60 ? Badge::COLOR_RED : ($maxCacheLifetime < 3600 ? Badge::COLOR_ORANGE : Badge::COLOR_GREEN);
                    $badges[] = new Badge(Utils::getFormattedCacheLifetime($maxCacheLifetime), $color, 'Maximal cache lifetime for images/css/js/fonts');
                }
                break;
            case self::SUPER_TABLE_VISITED_URLS:
                $red = 0;
                $orange = 0;
                $green = 0;
                foreach ($superTable->getData() as $row) {
                    $statusCode = $row['status'] ?? 0;
                    if ($statusCode <= 0 || $statusCode >= 400) {
                        $red++;
                    } elseif ($statusCode >= 300) {
                        $orange++;
                    } else {
                        $green++;
                    }
                }

                if ($red) {
                    $badges[] = new Badge((string)$red, Badge::COLOR_RED, 'Errors (40x, 50x, timeout, etc.)');
                }
                if ($orange) {
                    $badges[] = new Badge((string)$orange, Badge::COLOR_ORANGE, 'Redirects (30x)');
                }
                if ($green) {
                    $badges[] = new Badge((string)$green, Badge::COLOR_GREEN, 'OK (20x)');
                }
                break;
        }

        return $badges;
    }

    /**
     * @param SuperTable $superTable
     * @return Badge[]
     */
    private function getSuperTableGenericBadges(SuperTable $superTable): array
    {
        $badges = [];

        $red = 0;
        $orange = 0;
        $blue = 0;
        $green = 0;
        $neutral = 0;

        foreach ($superTable->getData() as $row) {
            if (is_object($row)) {
                $row = (array)$row;
            }
            if (isset($row['ok']) && is_numeric($row['ok'])) {
                $green += $row['ok'];
            }
            if (isset($row['notice']) && is_numeric($row['notice'])) {
                $blue += $row['notice'];
            }
            if (isset($row['warning']) && is_numeric($row['warning'])) {
                $orange += $row['warning'];
            }
            if (isset($row['critical']) && is_numeric($row['critical'])) {
                $red += $row['critical'];
            }
            if (isset($row['error']) && is_numeric($row['error'])) {
                $red += $row['error'];
            }
            if (isset($row['info']) && is_numeric($row['info'])) {
                $neutral += $row['info'];
            }
        }

        if ($red > 0) {
            $badges[] = new Badge((string)$red, Badge::COLOR_RED, 'Critical');
        }
        if ($orange > 0) {
            $badges[] = new Badge((string)$orange, Badge::COLOR_ORANGE, 'Warning');
        }
        if ($blue > 0) {
            $badges[] = new Badge((string)$blue, Badge::COLOR_BLUE, 'Notice');
        }
        if ($green > 0) {
            $badges[] = new Badge((string)$green, Badge::COLOR_GREEN, 'OK');
        }
        if ($neutral > 0) {
            $badges[] = new Badge((string)$neutral, Badge::COLOR_NEUTRAL, 'Info');
        }

        return $badges;
    }

    private function getTabContentBySuperTable(SuperTable $superTable): string
    {
        $html = $superTable->getHtmlOutput();
        $superTables = [];

        switch ($superTable->aplCode) {
            case SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED_SUMMARY:
                $superTables[] = $this->status->getSuperTableByAplCode(SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED);
                break;
            case BestPracticeAnalyzer::SUPER_TABLE_BEST_PRACTICES:
                foreach (BestPracticeAnalyzer::getAnalysisNames() as $analysisName) {
                    $superTable = $this->getSuperTableForUrlAnalysis($analysisName);
                    if ($superTable) {
                        $superTables[] = $superTable;
                    }
                }
                break;
            case AccessibilityAnalyzer::SUPER_TABLE_ACCESSIBILITY:
                foreach (AccessibilityAnalyzer::getAnalysisNames() as $analysisName) {
                    $superTable = $this->getSuperTableForUrlAnalysis($analysisName);
                    if ($superTable) {
                        $superTables[] = $superTable;
                    }
                }
                break;
            case SecurityAnalyzer::SUPER_TABLE_SECURITY:
                foreach (SecurityAnalyzer::getAnalysisNames() as $analysisName) {
                    $superTable = $this->getSuperTableForUrlAnalysis($analysisName);
                    if ($superTable) {
                        $superTables[] = $superTable;
                    }
                }
                break;
            case HeadersAnalyzer::SUPER_TABLE_HEADERS:
                $superTables[] = $this->status->getSuperTableByAplCode(HeadersAnalyzer::SUPER_TABLE_HEADERS_VALUES);
                break;
            case ContentTypeAnalyzer::SUPER_TABLE_CONTENT_TYPES:
                $superTables[] = $this->status->getSuperTableByAplCode(ContentTypeAnalyzer::SUPER_TABLE_CONTENT_MIME_TYPES);
                break;
            case CachingAnalyzer::SUPER_TABLE_CACHING_PER_CONTENT_TYPE:
                $superTables[] = $this->status->getSuperTableByAplCode(CachingAnalyzer::SUPER_TABLE_CACHING_PER_DOMAIN);
                $superTables[] = $this->status->getSuperTableByAplCode(CachingAnalyzer::SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE);
                break;
        }

        foreach ($superTables as $superTable) {
            $html .= '<br/>' . $superTable->getHtmlOutput();
        }

        return $html;
    }

    public function getSuperTableOrder(SuperTable $superTable): int
    {
        static $orders = [
            self::SUPER_TABLE_VISITED_URLS,
            BestPracticeAnalyzer::SUPER_TABLE_BEST_PRACTICES,
            AccessibilityAnalyzer::SUPER_TABLE_ACCESSIBILITY,
            SecurityAnalyzer::SUPER_TABLE_SECURITY,
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO,
            SeoAndOpenGraphAnalyzer::SUPER_TABLE_SEO_HEADINGS,
            Page404Analyzer::SUPER_TABLE_404,
            RedirectsAnalyzer::SUPER_TABLE_REDIRECTS,
            SkippedUrlsAnalyzer::SUPER_TABLE_SKIPPED_SUMMARY,
            FastestAnalyzer::SUPER_TABLE_FASTEST_URLS,
            SlowestAnalyzer::SUPER_TABLE_SLOWEST_URLS,
            ContentTypeAnalyzer::SUPER_TABLE_CONTENT_TYPES,
            SourceDomainsAnalyzer::SUPER_TABLE_SOURCE_DOMAINS,
            HeadersAnalyzer::SUPER_TABLE_HEADERS,
            CachingAnalyzer::SUPER_TABLE_CACHING_PER_CONTENT_TYPE,
            DnsAnalyzer::SUPER_TABLE_DNS,
        ];

        $index = array_search($superTable->aplCode, $orders, true);
        return is_int($index) ? $index : 1000;
    }

    public function getSuperTableForUrlAnalysis(string $analysisName): ?SuperTable
    {
        static $details = null;
        if ($details === null) {
            $details = $this->getDataForSuperTablesWithDetails();
        }

        $initialHost = $this->getInitialHost();
        $schemeOfInitialHost = $this->status->getOptions()->getInitialScheme();
        $data = $details[$analysisName] ?? [];

        $analysisAplCode = strtolower(str_replace(' ', '-', $analysisName));
        $superTable = new SuperTable($analysisAplCode, $analysisName, 'No problems found.', [
            new SuperTableColumn('severity', 'Severity', 10, null, function ($row) {
                return Utils::getColoredSeverity($row['severityFormatted']);
            }),
            new SuperTableColumn('count', 'Occurs', 8, function ($value) {
                return $value;
            }),
            new SuperTableColumn('detail', 'Detail', 200, function ($detail) {
                if (is_string($detail) || is_numeric($detail)) {
                    // check if string contains only non-HTML content or HTML tags <svg>
                    $isSvg = preg_match('/<svg/i', $detail) === 1;
                    if ($isSvg || preg_match('/^[\s\w\d.,:;!?()\/\-]*$/i', $detail)) {
                        if ($isSvg) {
                            $detail = str_replace(' display="block', '', $detail);
                            // add SVG size to the detail if detail contains only SVG
                            if (str_starts_with($detail, '<')) {
                                $isSvgIconSet = str_contains($detail, '<symbol') || str_contains($detail, '<g');
                                $finalDetailHtml = $isSvgIconSet
                                    ? (Utils::svgSetFillCurrentColor($detail) . ' ' . Utils::svgSetToPreview($detail))
                                    : Utils::svgSetFillCurrentColor($detail);
                                return Utils::getFormattedSize(strlen($detail)) . ' ' . $finalDetailHtml;
                            } else {
                                return $detail;
                            }
                        } else {
                            return htmlspecialchars($detail);
                        }
                    } else {
                        return nl2br(htmlspecialchars($detail));
                    }
                } elseif (is_array($detail) || is_object($detail)) {
                    return nl2br(htmlspecialchars(json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES)));
                } else {
                    return '';
                }
            }, null, false, true, false, false),
            new SuperTableColumn('exampleUrls', 'Affected URLs (max ' . $this->maxExampleUrls . ')', 60, null, function ($row) use ($initialHost, $schemeOfInitialHost) {
                $result = '';
                if (isset($row['exampleUrls']) && $row['exampleUrls'] && count($row['exampleUrls']) === 1) {
                    foreach ($row['exampleUrls'] as $exampleUrl) {
                        $result .= '<a href="' . htmlspecialchars($exampleUrl) . '" target="_blank">' . htmlspecialchars(Utils::truncateUrl($exampleUrl, 60, '…', $initialHost, $schemeOfInitialHost)) . '</a><br />';
                    }
                } elseif (isset($row['exampleUrls']) && $row['exampleUrls']) {
                    $counter = 1;
                    foreach ($row['exampleUrls'] as $exampleUrl) {
                        $result .= '<a href="' . htmlspecialchars($exampleUrl) . '" target="_blank">' . "URL {$counter}</a>, ";
                        $counter++;
                    }
                }
                return rtrim($result, ', ');
            }, false, true, false, false),
        ], false, null, 'ASC', null, 100);// sort primary by severity and secondary by count
        ;
        usort($data, function ($a, $b) {
            if ($a['severity'] === $b['severity']) {
                return $b['count'] <=> $a['count'];
            } else {
                return $a['severity'] <=> $b['severity'];
            }
        });

        $superTable->setData($data);
        return $superTable;
    }

    /**
     * @return array<string, array>
     */
    private function getDataForSuperTablesWithDetails(): array
    {
        $data = [];
        foreach ($this->status->getVisitedUrlToAnalysisResult() as $visitedUrlUqId => $analysisResults) {
            $url = $this->status->getUrlByUqId(strval($visitedUrlUqId));
            foreach ($analysisResults as $analysisResult) {
                /* @var UrlAnalysisResult $analysisResult */
                foreach ($analysisResult->getCriticalDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'critical',
                            'severity' => self::$severityOrder['critical'],
                            'detail' => $detail,
                        ];
                    }
                }

                foreach ($analysisResult->getWarningDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'warning',
                            'severity' => self::$severityOrder['warning'],
                            'detail' => $detail,
                        ];
                    }
                }

                foreach ($analysisResult->getNoticeDetails() as $analysisName => $details) {
                    if (!isset($data[$analysisName])) {
                        $data[$analysisName] = [];
                    }
                    foreach ($details as $detail) {
                        $data[$analysisName][] = [
                            'url' => $url,
                            'severityFormatted' => 'notice',
                            'severity' => self::$severityOrder['notice'],
                            'detail' => $detail,
                        ];
                    }
                }
            }
        }

        $data = $this->getSuperTableDataAggregated($data);

        return $data;
    }

    /**
     * This method solves data aggregation so that the table with findings does not contain tens of thousands
     * of identical findings, on different URLs, but they are meaningfully grouped and "masked" when needed
     *
     * @param array $data
     * @return array
     */
    private function getSuperTableDataAggregated(array $data): array
    {
        $result = [];

        foreach ($data as $analysisName => $rows) {
            if (!isset($result[$analysisName])) {
                $result[$analysisName] = [];
            }
            foreach ($rows as $row) {
                $detail = $this->getAggregatedDetail($row['detail']);
                $key = $this->getAggregatedDetailKey($row['severityFormatted'], $detail);
                if (!isset($result[$analysisName][$key])) {
                    $result[$analysisName][$key] = [
                        'severityFormatted' => $row['severityFormatted'],
                        'severity' => $row['severity'],
                        'detail' => $detail,
                        'count' => 1,
                        'exampleUrls' => [$row['url'] => $row['url']],
                    ];
                } else {
                    $result[$analysisName][$key]['count']++;
                    if (count($result[$analysisName][$key]['exampleUrls']) < $this->maxExampleUrls) {
                        $result[$analysisName][$key]['exampleUrls'][$row['url']] = $row['url'];
                    }
                }
            }
        }

        return $result;
    }

    private function getAggregatedDetail(mixed $detail): string
    {
        $result = null;
        if (is_string($detail)) {
            if (str_starts_with($detail, '<svg') || str_contains($detail, 'x SVG ')) {
                return $detail;
            }
            $result = Utils::removeUnwantedHtmlAttributes($detail, ['id', 'class', 'name']);

            // when detail is HTML tag, get only the first HTML tag
            if (str_starts_with(trim($result, '"\' '), '<')) {
                if (preg_match('/^[\s"\']*(<[^>]+>)/s', $result, $matches) === 1) {
                    $result = $matches[1];
                }
            }

            // replace trailing numbers in attributes to ***, e.g. in <img class="image-215">
            $result = preg_replace('/([0-9]+)(["\'])/s', '***$2', $result);

        } elseif (is_array($detail)) {
            foreach ($detail as $key => $value) {
                $detail[$key] = $this->getAggregatedDetail($value);
            }
            $result = json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } elseif (is_object($detail)) {
            foreach ($detail as $key => $value) {
                $detail->$key = $this->getAggregatedDetail($value);
            }
            $result = json_encode($detail, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        }
        return $result ?? '';
    }

    private function getAggregatedDetailKey(string $severity, string $detail): string
    {
        // remove clip-path attribute from SVGs (often is used in dynamic SVGs, but it is not important for comparison)
        if (str_contains($detail, '<svg')) {
            $detail = preg_replace('/<clipPath[^>]+>/i', '', $detail);
            $detail = preg_replace('/clip-path="[^"]+"/i', '', $detail);
        }
        return $severity . ' | ' . md5($detail);
    }

    private function getInitialHost(): string
    {
        return $this->status->getOptions()->getInitialHost();
    }

}