<?php

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Debugger;
use Crawler\Export\Utils\OfflineUrlConverter;
use Crawler\Export\Utils\TargetDomainRelation;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\ParsedUrl;
use Crawler\Parser\HtmlUrlParser;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use Exception;

class OfflineWebsiteExporter extends BaseExporter implements Exporter
{

    const GROUP_OFFLINE_WEBSITE_EXPORTER = 'offline-website-exporter';
    const JS_VARIABLE_NAME_URL_DEPTH = '_SiteOneUrlDepth';

    protected ?string $offlineExportDirectory = null;

    public ?string $initialUrlHost;

    /**
     * For debug - when filled it will activate debug mode and store only URLs which match one of these regexes
     * @var string[]
     */
    protected array $offlineExportStoreOnlyUrlRegex = [];

    /**
     * For debug only - storage of debug messages if debug mode is activated (storeOnlyUrls)
     * @var array|null
     */
    protected ?array $debugMessages = null;

    /**
     * Exporter is activated when --offline-export-dir is set
     * @return bool
     */
    public function shouldBeActivated(): bool
    {
        $this->offlineExportDirectory = $this->offlineExportDirectory ? rtrim($this->offlineExportDirectory, '/') : null;
        return $this->offlineExportDirectory !== null;
    }

    /**
     * Export all visited URLs to directory with offline browsable version of the website
     * @return void
     * @throws Exception
     */
    public function export(): void
    {
        $startTime = microtime(true);
        $visitedUrls = $this->status->getVisitedUrls();
        $this->initialUrlHost = parse_url($this->status->getOptions()->url, PHP_URL_HOST);

        // filter only relevant URLs with OK status codes
        $exportedUrls = array_filter($visitedUrls, function (VisitedUrl $visitedUrl) {
            return in_array($visitedUrl->statusCode, [200, 201, 301, 302, 303, 308]);
        });
        /** @var VisitedUrl[] $exportedUrls */

        // activate debug mode and start storing debug messages
        if ($this->crawler->getCoreOptions()->debugUrlRegex) {
            $this->debugMessages = [];
        }

        // store all allowed URLs
        try {
            foreach ($exportedUrls as $exportedUrl) {
                if ($this->isValidUrl($exportedUrl->url) && $this->shouldBeUrlStored($exportedUrl)) {
                    $this->storeFile($exportedUrl);
                }
            }
        } catch (Exception $e) {
            var_dump(__METHOD__ . ": ERROR {$e->getMessage()}");
            throw new Exception(__METHOD__ . ': ' . $e->getMessage());
        }

        // add redirect HTML files for each subfolder (if contains index.html) recursively
        $changes = [];
        Utils::addRedirectHtmlToSubfolders($this->offlineExportDirectory, $changes);

        // print debug messages
        if ($this->debugMessages) {
            foreach ($this->debugMessages as $debugMessage) {
                Debugger::consoleArrayDebug($debugMessage, [24, 60, 80, 80]);
            }
        }

        // add info to summary
        $this->status->addInfoToSummary(
            'offline-website-generated',
            sprintf(
                "Offline website generated to '%s'. It takes %s",
                $this->offlineExportDirectory,
                Utils::getFormattedDuration(microtime(true) - $startTime)
            )
        );
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

        // apply all required changes to HTML/JS/CSS and REDIRECT content (is HTML with redirect by META tag)
        $contentTypesThatRequireChanges = [
            Crawler::CONTENT_TYPE_ID_HTML,
            Crawler::CONTENT_TYPE_ID_SCRIPT,
            Crawler::CONTENT_TYPE_ID_STYLESHEET,
            Crawler::CONTENT_TYPE_ID_REDIRECT
        ];

        if (in_array($visitedUrl->contentType, $contentTypesThatRequireChanges)) {
            $content = $this->applyRequiredContentChanges($content, $visitedUrl->contentType, $visitedUrl->url, $this->debugMessages);
        }

        // apply HTML changes optimal for offline use (e.g. remove Google Analytics, Facebook Pixel, etc.)
        if ($visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML) {
            $content = Utils::applySpecificHtmlChanges(
                $content,
                $visitedUrl->url,
                $this->crawler->getCoreOptions()->disableJavascript,
                true,
                true,
                true,
                true
            );
        }

        // sanitize and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
        // same logic is in method convertUrlToRelative()
        $storeFilePath = $this->offlineExportDirectory . '/' . $this->getRelativeFilePathForFileByUrl($visitedUrl);
        $storeFilePath = OfflineUrlConverter::sanitizeFilePath($storeFilePath, false);

        $directoryPath = dirname($storeFilePath);
        if (!is_dir($directoryPath)) {
            if (!mkdir($directoryPath, 0777, true)) {
                throw new Exception("Cannot create directory '$directoryPath'");
            }
        }

        if (file_put_contents($storeFilePath, $content) === false) {
            throw new Exception("Cannot store file '$storeFilePath'");
        }
    }

    /**
     * Check if URL can be stored with respect to --offline-export-store-only-url-regex option and --allow-domain-*
     *
     * @param VisitedUrl $visitedUrl
     * @return bool
     */
    private function shouldBeUrlStored(VisitedUrl $visitedUrl): bool
    {
        $result = false;

        // by --offline-export-store-only-url-regex
        if ($this->offlineExportStoreOnlyUrlRegex) {
            foreach ($this->offlineExportStoreOnlyUrlRegex as $storeOnlyUrlRegex) {
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

        return $result;
    }

    private function getRelativeFilePathForFileByUrl(VisitedUrl $visitedUrl): string
    {
        $urlConverter = new OfflineUrlConverter(
            ParsedUrl::parse($this->crawler->getCoreOptions()->url),
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

    /**
     * Apply all required content changes (URL to relative, remove unwanted code, etc.)
     *
     * @param string $content
     * @param int $contentType
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function applyRequiredContentChanges(string $content, int $contentType, string $baseUrl, ?array &$debug = null): string
    {
        if ($contentType === Crawler::CONTENT_TYPE_ID_HTML || $contentType === Crawler::CONTENT_TYPE_ID_REDIRECT) {
            $content = $this->applyHtmlContentChanges($content, $baseUrl, $debug);
        } else if ($contentType === Crawler::CONTENT_TYPE_ID_STYLESHEET) {
            $content = $this->applyCssContentChanges($content, $baseUrl, $debug);
        } elseif ($contentType === Crawler::CONTENT_TYPE_ID_SCRIPT) {
            $content = $this->applyJsContentChanges($content, $baseUrl, $debug);
        }

        return $content;
    }

    /**
     * Apply all required changes to HTML content
     *
     * @param string $html
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function applyHtmlContentChanges(string $html, string $baseUrl, ?array &$debug): string
    {
        // treat some framework-specific code which could cause problems during parsing
        $html = HtmlUrlParser::treatFrameworkSpecificCode($html);

        // remove unwanted full urls (width initial domain) from HTML - it simplifies relative paths conversion
        $baseUrlRoot = preg_replace('/((https?:)?\/\/[^\/]+\/?).*/i', '$1', $baseUrl);
        $html = preg_replace('/((https?:)?\/\/[^\/]+):[0-9]+/i', '$1', $html);
        $html = str_replace(
            [
                'href="' . $baseUrlRoot,
                "href='" . $baseUrlRoot,
                'src="' . $baseUrlRoot,
                "src='" . $baseUrlRoot,
                "url(" . $baseUrlRoot,
                'url("' . $baseUrlRoot,
                "url('" . $baseUrlRoot,
            ],
            [
                'href="/',
                "href='/",
                'src="/',
                "src='/",
                "url(/",
                'url("/',
                "url('/",
            ],
            $html
        );

        // remove unwanted code from HTML with respect to --disable-* options
        $html = $this->removeUnwantedCodeFromHtml($html);

        // update all paths to relative (for href, src, srcset and also for url() in CSS or some special cases in JS)
        $html = $this->updateHtmlPathsToRelative($html, $baseUrl, $debug);

        // apply all paths to relative in <style> tags
        if (!$this->crawler->getCoreOptions()->disableStyles) {
            $html = $this->updateCssPathsToRelative($html, $baseUrl, $debug);
        }

        // set JS variable with number of levels before close </head> tag and remove all anchor listeners when needed
        if (!$this->crawler->getCoreOptions()->disableJavascript) {
            $html = $this->setJsVariableWithUrlDepth($html, $baseUrl);
            if ($this->crawler->getCoreOptions()->removeAllAnchorListeners) {
                $html = $this->setJsFunctionToRemoveAllAnchorListeners($html);
            }
        }

        return $html;
    }

    /**
     * Apply all required changes to CSS content
     *
     * @param string $css
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function applyCssContentChanges(string $css, string $baseUrl, ?array &$debug): string
    {
        $css = $this->removeUnwantedCodeFromCss($css);
        return $this->updateCssPathsToRelative($css, $baseUrl, $debug);
    }

    /**
     * Apply all required changes to JS content
     *
     * @param string $js
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function applyJsContentChanges(string $js, string $baseUrl, ?array &$debug): string
    {
        return $this->updateJsPathsToRelative($js, $baseUrl, $debug);
    }

    /**
     * Update all paths to relative (for href, src, srcset and also for url() in CSS or some special cases in JS)
     *
     * @param string $html
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function updateHtmlPathsToRelative(string $html, string $baseUrl, ?array &$debug = null): string
    {
        $patternHrefSrc = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(href|src)\s*=\s*([\'"]?)([^\'">]+)\3([^>]*)/im';
        $patternSrcset = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(imagesrcset|srcset)\s*=\s*([\'"]?)([^\'">]+)\3([^>]*)/im';
        $patternMetaUrl = '/(<meta[^>]*)(url)\s*=\s*([\'"]?)([^\'">]+)\3(")/im';
        $isUrlForDebug = $this->crawler->getCoreOptions()->isUrlSelectedForDebug($baseUrl);

        $replaceCallback = function ($matches) use ($baseUrl, &$debug, $isUrlForDebug) {
            $start = $matches[1];
            $attribute = $matches[2];
            $quote = $matches[3];
            $value = $matches[4];
            $end = $matches[5];

            // when modifying x.src (JS) and there is no quote, we do not convert, because it is not a valid URL but JS code
            if ($start === '.' && $quote === '') {
                return $matches[0];
            }

            // ignore data URI, dotted relative path or #anchor
            if (str_starts_with($value, '#') || preg_match('/^[a-z]+:[a-z0=9]]/i', $value) === 1) {
                return $matches[0];
            }

            if (in_array(strtolower($attribute), ['srcset', 'imagesrcset'])) {
                $sources = preg_split('/\s*,\s*/', $value);
                foreach ($sources as &$source) {
                    if (!str_contains($source, ' ')) {
                        continue;
                    } else {
                        @list($url, $size) = preg_split('/\s+/', trim($source), 2);
                        $relativeUrl = $this->convertUrlToRelative($baseUrl, $url, $attribute);
                        $source = $relativeUrl . ' ' . $size;
                    }
                }
                $newValue = implode(', ', $sources);
            } else {
                $newValue = $this->convertUrlToRelative($baseUrl, $value, $attribute);
            }

            if ($debug !== null && $value !== $newValue && $isUrlForDebug) {
                $debug[] = ['updateHtmlPathsToRelative', $baseUrl, $value, '> ' . $newValue];
            }

            return $start . $attribute . '=' . $quote . $newValue . $quote . $end;
        };

        // meta redirects e.g. in Astro projects - <meta http-equiv="refresh" content="0;url=/en/getting-started">
        if (preg_match('/(<meta[^>]*url=)([^"\']+)(["\'][^>]*>)/i', $html, $matches) === 1) {
            $html = str_replace($matches[0], $matches[1] . $this->convertUrlToRelative($baseUrl, $matches[2]) . $matches[3], $html);
        }

        $html = preg_replace_callback($patternHrefSrc, $replaceCallback, $html);
        $html = preg_replace_callback($patternSrcset, $replaceCallback, $html);
        $html = preg_replace_callback($patternMetaUrl, $replaceCallback, $html);

        return $html;
    }

    /**
     * Update all paths to relative in CSS (for url() in CSS or some special cases in JS)
     *
     * @param string $css
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function updateCssPathsToRelative(string $css, string $baseUrl, ?array &$debug = null): string
    {
        $pattern = '/url\((["\']?)([^)]+\.[a-z0-9]{1,10}[^)]*)\1\)/i';
        $isUrlForDebug = $this->crawler->getCoreOptions()->isUrlSelectedForDebug($baseUrl);

        return preg_replace_callback($pattern, function ($matches) use ($baseUrl, &$debug, $isUrlForDebug) {
            // if is data URI, dotted relative path or #anchor, do not convert
            $url = $matches[2];
            if (!Utils::isHrefForRequestableResource($url) || str_starts_with($url, '.') || str_starts_with($matches[2], '#')) {
                return $matches[0];
            }
            $relativeUrl = $this->convertUrlToRelative($baseUrl, $url);

            $newValue = 'url(' . $matches[1] . $relativeUrl . $matches[1] . ')';
            if ($debug !== null && $matches[0] !== $newValue && $isUrlForDebug) {
                $debug[] = ['updateCssPathsToRelative', $baseUrl, $matches[0], '> ' . $newValue];
            }

            return $newValue;
        }, $css);
    }

    /**
     * Update specific JS code to use relative paths ()
     *
     * @param string $js
     * @param string $baseUrl
     * @param array|null $debug
     * @return string
     */
    private function updateJsPathsToRelative(string $js, string $baseUrl, ?array &$debug = null): string
    {
        $js = $this->updateNextJsCode($js);

        // rename "crossorigin" in JS to brake it functionality as fix to CORS issues in offline website
        return str_ireplace('crossorigin', '_SiteOne_CO_', $js);
    }

    private function updateNextJsCode(string $js): string
    {
        $js = preg_replace(
            '/\(t\.src\s*=\s*s\.src\)/i',
            '(t.src = "../".repeat(' . self::JS_VARIABLE_NAME_URL_DEPTH . ') + s.src)',
            $js
        );

        $js = preg_replace(
            '/r\.href\s*=\s*t\s*,/i',
            'r.href = "../".repeat(' . self::JS_VARIABLE_NAME_URL_DEPTH . ') + t,',
            $js
        );

        $js = preg_replace(
            '/t\.src\s*=\s*e,/i',
            't.src = "../".repeat(' . self::JS_VARIABLE_NAME_URL_DEPTH . ') + e,',
            $js
        );

        $js = preg_replace(
            "/var\s*s\s*=\s*'link\[rel=\"preload\"\]/i",
            "n = '../'.repeat(" . self::JS_VARIABLE_NAME_URL_DEPTH . "); var s = 'link[rel=\"preload\"]",
            $js,
        );

        return $js;
    }

    /**
     * Remove all unwanted code from HTML with respect to --disable-* options
     *
     * @param string $html
     * @return string
     */
    private function removeUnwantedCodeFromHtml(string $html): string
    {
        if ($this->crawler->getCoreOptions()->disableJavascript) {
            $html = Utils::stripJavaScript($html);
        }
        if ($this->crawler->getCoreOptions()->disableStyles) {
            $html = Utils::stripStyles($html);
        }
        if ($this->crawler->getCoreOptions()->disableFonts) {
            $html = Utils::stripFonts($html);
        }
        if ($this->crawler->getCoreOptions()->disableImages) {
            $html = Utils::stripImages($html);
        }

        return $html;
    }

    /**
     * Remove all unwanted code from CSS with respect to --disable-* options
     *
     * @param string $css
     * @return string
     */
    private function removeUnwantedCodeFromCss(string $css): string
    {
        if ($this->crawler->getCoreOptions()->disableFonts) {
            $css = Utils::stripFonts($css);
        }
        if ($this->crawler->getCoreOptions()->disableImages) {
            $css = Utils::stripImages($css);
        }

        return $css;
    }

    /**
     * Add JS variable _SiteOneUrlDepth with number of levels before close </head> tag
     * This variable is used in replaced JS code for relative paths (for example in NextJS framework*.js files)
     *
     * @param string $html
     * @param string $baseUrl
     * @return string
     */
    private function setJsVariableWithUrlDepth(string $html, string $baseUrl): string
    {
        $basePath = parse_url($baseUrl, PHP_URL_PATH);
        if (!$basePath) {
            $basePath = '/';
        }
        $depth = substr_count($basePath, '/') - 1;
        $baseUrlNeedsIndexHtml = preg_match('/\.[a-z0-9]{1,10}$/i', $basePath) === 0 && trim($basePath, '/') !== '';
        if ($baseUrlNeedsIndexHtml && !str_ends_with($basePath, '/')) {
            $depth++;
        }

        return preg_replace(
            '/<\s*\/\s*head\s*>/i',
            sprintf("<script>var %s = %d;</script></head>", self::JS_VARIABLE_NAME_URL_DEPTH, $depth),
            $html
        );
    }

    private function setJsFunctionToRemoveAllAnchorListeners(string $html): string
    {
        return preg_replace(
            '/<\s*\/\s*body\s*>/i',
            "<script>
                function _SiteOneRemoveAllAnchorListeners(){
                    var anchors=document.getElementsByTagName('a');
                    for(var i=0;i<anchors.length;i++){
                        var anchor=anchors[i];
                        var newAnchor=anchor.cloneNode(true);
                        anchor.parentNode.replaceChild(newAnchor,anchor);
                    }
                }
                setTimeout(_SiteOneRemoveAllAnchorListeners, 200);
                setTimeout(_SiteOneRemoveAllAnchorListeners, 1000);
             </script></body>",
            $html
        );
    }

    /**
     * @param string $baseUrl
     * @param string $targetUrl
     * @param string|null $attribute
     * @return string
     */
    public function convertUrlToRelative(string $baseUrl, string $targetUrl, ?string $attribute = null): string
    {
        $urlConverter = new OfflineUrlConverter(
            ParsedUrl::parse($this->crawler->getCoreOptions()->url),
            ParsedUrl::parse($baseUrl),
            ParsedUrl::parse($targetUrl),
            [$this->crawler, 'isDomainAllowedForStaticFiles'],
            [$this->crawler, 'isExternalDomainAllowedForCrawling'],
            $attribute
        );

        return $urlConverter->convertUrlToRelative(true);
    }

    private function isValidUrl(string $url): bool
    {
        return filter_var($url, FILTER_VALIDATE_URL) !== false;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_OFFLINE_WEBSITE_EXPORTER,
            'Offline exporter options', [
            new Option('--offline-export-directory', null, 'offlineExportDirectory', Type::DIR, false, 'Path to directory where to save the offline version of the website.', null, true),
            new Option('--offline-export-store-only-url-regex', null, 'offlineExportStoreOnlyUrlRegex', Type::REGEX, true, 'For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.', null, true),
        ]));
        return $options;
    }
}
