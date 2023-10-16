<?php

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Debugger;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\ParsedUrl;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;

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
     * @var string[]|null
     */
    protected ?array $debugMessages = null;

    public function shouldBeActivated(): bool
    {
        $this->offlineExportDirectory = $this->offlineExportDirectory ? rtrim($this->offlineExportDirectory, '/') : null;
        return $this->offlineExportDirectory !== null;
    }

    /**
     * Export all visited URLs to directory with offline browsable version of the website
     * @return void
     */
    public function export(): void
    {
        $visitedUrls = $this->status->getVisitedUrls();
        $this->initialUrlHost = parse_url($this->status->getOptions()->url, PHP_URL_HOST);

        $exportedUrls = array_filter($visitedUrls, function (VisitedUrl $visitedUrl) {
            return $visitedUrl->statusCode === 200;
        });
        /** @var VisitedUrl[] $exportedUrls */

        // activate debug mode and start storing debug messages
        if ($this->crawler->getCoreOptions()->debugUrlRegex) {
            $this->debugMessages = [];
        }

        // store all allowed URLs
        foreach ($exportedUrls as $exportedUrl) {
            if ($this->isValidUrl($exportedUrl->url) && $this->shouldBeUrlStored($exportedUrl)) {
                $this->storeFile($exportedUrl);
            }
        }

        // print debug messages
        if ($this->debugMessages) {
            foreach ($this->debugMessages as $debugMessage) {
                Debugger::consoleArrayDebug($debugMessage, [24, 60, 80, 80]);
            }
        }

    }

    private function storeFile(VisitedUrl $visitedUrl): void
    {
        $content = $this->status->getUrlBody($visitedUrl->uqId);
        $relativeFilePath = $this->getRelativeFilePathForFileByUrl($visitedUrl);

        // update all paths to relative (for href, src, srcset and also for url() in CSS or some special cases in JS)
        if ($visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML ||
            $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_STYLESHEET ||
            $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_SCRIPT
        ) {
            $content = $this->updatePathsToRelative($content, $visitedUrl->contentType, $visitedUrl->url, $this->debugMessages);
        }

        // apply HTML changes optimal for offline use (e.g. remove Google Analytics, Facebook Pixel, etc.)
        if ($visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML) {
            $content = Utils::applySpecificHtmlChanges($content, $visitedUrl->url, true, true, true, true, true);
        }

        $storeFilePath = $this->offlineExportDirectory . '/' . $relativeFilePath;

        // sanitize and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
        // same logic is in method convertUrlToRelative()
        $storeFilePath = $this->sanitizeFilePath($storeFilePath);
        $directoryPath = dirname($storeFilePath);
        if (!is_dir($directoryPath)) {
            mkdir($directoryPath, 0777, true);
        }

        file_put_contents($storeFilePath, $content);
    }

    private function shouldBeUrlStored(VisitedUrl $visitedUrl): bool
    {
        if ($this->offlineExportStoreOnlyUrlRegex) {
            foreach ($this->offlineExportStoreOnlyUrlRegex as $storeOnlyUrlRegex) {
                if (preg_match($storeOnlyUrlRegex, $visitedUrl->url) === 1) {
                    return true;
                }
            }
            return false;
        }

        return true;
    }

    private function getRelativeFilePathForFileByUrl(VisitedUrl $visitedUrl): string
    {
        $url = $this->stripQueryAndFragment($visitedUrl->url);
        $path = parse_url($url, PHP_URL_PATH);

        if ($this->endsWithSlashOrNoExtension($path)) {
            $path = rtrim($path, '/') . '/index.html';
        }
        if (str_starts_with($path, '/')) {
            $path = substr($path, 1);
        }

        if ($visitedUrl->isExternal) {
            $path = '_' . ParsedUrl::parse($visitedUrl->url)->host . '/' . $path;
        }

        return $path;
    }

    private function stripQueryAndFragment(string $url): string
    {
        $urlComponents = parse_url($url);

        $scheme = isset($urlComponents['scheme']) ? $urlComponents['scheme'] . '://' : '';
        $host = $urlComponents['host'] ?? '';
        $path = $urlComponents['path'] ?? '/';

        return $scheme . $host . $path;
    }

    private function endsWithSlashOrNoExtension(string $url): bool
    {
        $path = parse_url($url, PHP_URL_PATH);
        if (str_ends_with($path, '/')) {
            return true;
        } elseif (preg_match('/\.[a-z0-9]{1,10}(|\?.*)$/i', $url) === 0) {
            return true;
        }
        return false;
    }

    private function updatePathsToRelative(string $content, int $contentType, string $baseUrl, ?array &$debug = null): string
    {
        if ($contentType === Crawler::CONTENT_TYPE_ID_STYLESHEET) {
            $content = $this->updateCssPathsToRelative($content, $baseUrl, $debug);
        } elseif ($contentType === Crawler::CONTENT_TYPE_ID_SCRIPT) {
            $content = $this->updateJsPathsToRelative($content, $baseUrl, $debug);
        } else if ($contentType === Crawler::CONTENT_TYPE_ID_HTML) {
            // remove unwanted full urls (width initial domain) from HTML - it simplifies relative paths conversion
            $baseUrlRoot = preg_replace('/((https?:)?\/\/[^\/]+\/?).*/i', '$1', $baseUrl);
            $content = preg_replace('/((https?:)?\/\/[^\/]+):[0-9]+/i', '$1', $content);
            $content = str_replace(
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
                $content
            );

            $content = $this->updateHtmlPathsToRelative($content, $baseUrl, $debug);
            $content = $this->updateCssPathsToRelative($content, $baseUrl, $debug);
            $content = $this->setJsVariableWithUrlDepth($content, $baseUrl);
            $content = $this->setJsFunctionToRemoveAllAnchorListeners($content);
        }

        return $content;
    }

    private function updateHtmlPathsToRelative(string $html, string $baseUrl, ?array &$debug = null): string
    {
        $patternHrefSrc = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(href|src)\s*=\s*([\'"]?)([^\'">\s]+)\3([^>]*)/im';
        $patternSrcset = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(srcset)\s*=\s*([\'"]?)([^\'">]+)\3([^>]*)/im';
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

            if (strtolower($attribute) === 'srcset' && str_contains($value, ',')) {
                $sources = preg_split('/\s*,\s*/', $value);
                foreach ($sources as &$source) {
                    if (!str_contains($source, ' ')) {
                        continue;
                    } else {
                        @list($url, $size) = preg_split('/\s+/', trim($source), 2);
                        $relativeUrl = $this->convertUrlToRelative($baseUrl, $url);
                        $source = $relativeUrl . ' ' . $size;
                    }
                }
                $newValue = implode(', ', $sources);
            } else {
                $newValue = $this->convertUrlToRelative($baseUrl, $value);
            }

            if ($debug !== null && $value !== $newValue && $isUrlForDebug) {
                $debug[] = ['updateHtmlPathsToRelative', $baseUrl, $value, '> ' . $newValue];
            }

            return $start . $attribute . '=' . $quote . $newValue . $quote . $end;
        };

        // visible in Astro projects - <meta http-equiv="refresh" content="0;url=/en/getting-started">
        if (preg_match('/(<meta[^>]*url=)([^"\']+)(["\'][^>]*>)/i', $html, $matches) === 1) {
            $html = str_replace($matches[0], $matches[1] . $this->convertUrlToRelative($baseUrl, $matches[2]) . $matches[3], $html);
        }

        $html = preg_replace_callback($patternHrefSrc, $replaceCallback, $html);
        return preg_replace_callback($patternSrcset, $replaceCallback, $html);
    }

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
        $js = str_ireplace('crossorigin', '_SiteOne_CO_', $js);

        return $js;
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
            sprintf("<script>function _SiteOneRemoveAllAnchorListeners(){ var anchors=document.getElementsByTagName('a');for(var i=0;i<anchors.length;i++){var anchor=anchors[i];var newAnchor=anchor.cloneNode(true);anchor.parentNode.replaceChild(newAnchor,anchor);}} setTimeout(_SiteOneRemoveAllAnchorListeners, 200); setTimeout(_SiteOneRemoveAllAnchorListeners, 1000);</script></body>"),
            $html
        );
    }

    public function convertUrlToRelative(string $baseUrl, string $targetUrl): string
    {
        $targetUrl = trim($targetUrl);
        if (!Utils::isHrefForRequestableResource($targetUrl)) {
            return $targetUrl;
        }

        // $baseUrlOriginal = $baseUrl;
        // $targetUrlOriginal = $targetUrl;
        $baseHost = parse_url($baseUrl, PHP_URL_HOST);
        $targetHost = parse_url($targetUrl, PHP_URL_HOST);
        $initialHost = $this->initialUrlHost;

        // when target host is not defined, we use base host (because this relative URL is on the same host)
        if (!$targetHost) {
            $targetHost = $baseHost;
        }

        $isExternalHost = $targetHost && $baseHost && $baseHost !== $targetHost;

        // do not convert external URLs which are not allowed for crawling and saving to static files (for offline website)
        if ($isExternalHost && !$this->crawler->isDomainAllowedForStaticFiles($targetHost) && !$this->crawler->isExternalDomainAllowedForCrawling($targetHost)) {
            return $targetUrl;
        } elseif ($targetHost === $baseHost) {
            // remove unwanted https://MYDOMAIN.COM/ or //MYDOMAIN.COM/ from URL
            $targetUrl = preg_replace('/^(https?:)?\/\/[^\/]+\//i', '/', $targetUrl);
        }

        if ($targetUrl === '/' || $targetUrl === '') {
            $targetUrl = '/index.html';
        } else {
            $targetUrl = $this->getUrlWithIndexHtmlIfNeeded($targetUrl);
        }

        // remove double slashes from beginning of URL
        $targetUrl = preg_replace('/\/{2,}/', '/', $targetUrl);

        // replace % because it is not allowed in file names or there will be issues on some platforms (e.g. Windows)
        $targetUrl = str_replace('%', '_', $targetUrl);

        $basePath = parse_url($baseUrl, PHP_URL_PATH);
        if ($basePath === null) {
            $basePath = '/'; // example case is https://www.siteone.io?callback
        }

        $targetPath = parse_url($targetUrl, PHP_URL_PATH);
        if ($targetPath === null) {
            $targetPath = '/'; // example case is https://www.siteone.io?callback
        }

        $targetQuery = parse_url($targetUrl, PHP_URL_QUERY);
        $targetFragment = parse_url($targetUrl, PHP_URL_FRAGMENT);

        // append query and fragment to non-static files
        $isStaticFile = preg_match('/\.(js|css|json|txt|eot|ttf|woff2|woff|otf|png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg|pdf|doc|docx|xls|xlsx|ppt|pptx)/i', $targetUrl) === 1;
        if ($targetQuery && !$isStaticFile) {
            $targetPath .= '?' . $targetQuery;
        }
        if ($targetFragment && !$isStaticFile) {
            $targetPath .= '#' . $targetFragment;
        }

        // SPECIAL CASE: when base & target hosts are defined and baseHost is different from initial host
        // Example: CSS file on https://_global-uploads.webflow.com/ with font-url from https://uploads-ssl.webflow.com/,
        // but initial host is wwww.czechitas.cz
        if ($baseHost && $targetHost && $baseHost !== $this->initialUrlHost && $baseHost !== $targetHost) {
            $depth = substr_count($basePath, '/');
        } elseif ($basePath) {
            $depth = substr_count($basePath, '/') - 1;
        }

        // when base url needs index.html file in subfolder and basePath do not end with /, we have to increase depth
        $baseUrlNeedsIndexHtml = preg_match('/\.[a-z0-9]{1,10}$/i', $basePath) === 0 && trim($basePath, '/') !== '';
        if ($baseUrlNeedsIndexHtml && !str_ends_with($basePath, '/')) {
            $depth++;
        }

        // decreased depth by already dotted levels in current target path
        $depth -= substr_count($targetPath, '../');

        $relativePrefix = str_repeat('../', max($depth, 0));

        // when target is on the same level and defined like a href="local.html", no prefix needed
        if (!$isExternalHost && preg_match('/^[a-z0-9]/i', $targetPath) === 1) {
            $relativePrefix = '';
        }

        // for external URLs add prefix "_", e.g. "_cdn.siteone.io"
        if ($isExternalHost) {
            $relativePrefix = $relativePrefix . '_';
        }

        // sanitize and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
        // same logic is in method storeFile()
        $targetPath = $this->sanitizeFilePath($targetPath);

        return trim($relativePrefix ? ($relativePrefix . ltrim($targetPath, '/ ')) : ltrim($targetPath, '/ '));
    }

    /**
     * Add /index.html to URL when needed with respect to optional query and fragment
     * @param string $url
     * @return string
     */
    private function getUrlWithIndexHtmlIfNeeded(string $url): string
    {
        $urlComponents = parse_url($url);
        $urlPath = $urlComponents['path'] ?? '';
        if (preg_match('/\.[a-z0-9]{1,10}$/i', $urlPath) === 0) {
            $urlPath = rtrim($urlPath) . '/index.html';
        }

        return (isset($urlComponents['scheme']) ? "{$urlComponents['scheme']}://" : '') .
            ($urlComponents['host'] ?? '') .
            $urlPath .
            (isset($urlComponents['query']) ? '?' . $urlComponents['query'] : '') .
            (isset($urlComponents['fragment']) ? '#' . $urlComponents['fragment'] : '');
    }

    private function isValidUrl(string $url): bool
    {
        return filter_var($url, FILTER_VALIDATE_URL) !== false;
    }

    private function sanitizeFileName(string $filename): string
    {
        $dangerousCharacters = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        $filename = str_replace($dangerousCharacters, '', $filename);
        return mb_substr($filename, 0, 250);
    }

    /**
     * Sanitize file path and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
     * When long filename and potential of OS filepath limit (~256 on Windows), we replace filename with shorter md5 and the same extension
     *
     * @param string $filePath
     * @return string
     */
    private function sanitizeFilePath(string $filePath): string
    {
        // remove query and fragment from static-file URL
        if (preg_match('/\.(js|css|json|txt|eot|ttf|woff2|woff|otf|png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg|pdf|doc|docx|xls|xlsx|ppt|pptx)/i', $filePath) === 1) {
            $filePath = preg_replace('/[?#].*$/', '', $filePath);
        }

        $dangerousCharacters = ['\\', ':', '%', '*', '?', '"', "'", '<', '>', '|'];
        $filePath = str_replace($dangerousCharacters, '_', $filePath);

        // when filepath is too long and there is long filename, we replace filename with shorter md5 and the same extension
        // filepath length is calculated from root of offline website directory for better results
        // 200 is just a safe limit, because there is also directory path
        $filePathLength = strlen(str_replace($this->offlineExportDirectory, '/', $filePath));
        if ($filePathLength > 200 && strlen(basename($filePath)) > 40) {
            $basename = basename($filePath);
            $extension = pathinfo($basename, PATHINFO_EXTENSION);
            $filePath = str_replace($basename, md5($basename) . '.' . $extension, $filePath);
        }
        return $filePath;
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
