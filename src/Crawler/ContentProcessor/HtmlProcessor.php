<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\Crawler;
use Crawler\FoundUrl;
use Crawler\FoundUrls;
use Crawler\ParsedUrl;
use Crawler\Utils;

class HtmlProcessor extends BaseProcessor implements ContentProcessor
{
    protected array $relevantContentTypes = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_REDIRECT,
    ];

    public const JS_VARIABLE_NAME_URL_DEPTH = '_SiteOneUrlDepth';
    public static array $htmlPagesExtensions = ['htm', 'html', 'shtml', 'php', 'phtml', 'ashx', 'xhtml', 'asp', 'aspx', 'jsp', 'jspx', 'do', 'cfm', 'cgi', 'pl'];

    private readonly bool $singlePageOnly;
    private readonly bool $singleForeignPageOnly;
    private readonly int $maxDepth;
    private readonly bool $filesEnabled;
    private readonly bool $imagesEnabled;
    private readonly bool $scriptsEnabled;
    private readonly bool $stylesEnabled;
    private readonly bool $fontsEnabled;

    /**
     * @param Crawler $crawler
     */
    public function __construct(Crawler $crawler)
    {
        parent::__construct($crawler);

        $this->singlePageOnly = $this->options->singlePage;
        $this->singleForeignPageOnly = $this->options->singleForeignPage;
        $this->maxDepth = $this->options->maxDepth;
        $this->filesEnabled = !$this->options->disableFiles;
        $this->imagesEnabled = !$this->options->disableImages;
        $this->scriptsEnabled = !$this->options->disableJavascript;
        $this->stylesEnabled = !$this->options->disableStyles;
        $this->fontsEnabled = !$this->options->disableFonts;
    }

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', self::$htmlPagesExtensions) . ')/i';
        }

        $foundUrls = new FoundUrls();

        if (!$this->singlePageOnly) {
            $this->findHrefUrls($content, $sourceUrl, $foundUrls, $regexForHtmlExtensions);
        }

        if ($this->fontsEnabled) {
            $this->findFonts($content, $sourceUrl, $foundUrls);
        }

        if ($this->imagesEnabled) {
            $this->findImages($content, $sourceUrl, $foundUrls);
        }

        if ($this->filesEnabled) {
            $this->findAudio($content, $sourceUrl, $foundUrls);
            $this->findVideo($content, $sourceUrl, $foundUrls);
        }

        if ($this->scriptsEnabled) {
            $this->findScripts($content, $sourceUrl, $foundUrls);
        }

        if ($this->stylesEnabled) {
            $this->findStylesheets($content, $sourceUrl, $foundUrls);
        }

        return $foundUrls->getCount() > 0 ? $foundUrls : null;
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        $baseUrl = $url->getFullUrl();

        // remove unwanted full urls (with origin domain) from HTML - it simplifies relative paths conversion
        $content = $this->removeSchemaAndHostFromFullOriginUrls($url, $content);

        // remove unwanted code from HTML with respect to --disable-* options
        $content = $this->removeUnwantedCodeFromHtml($content);

        // update all paths to relative (for href, src, srcset and also for url() in CSS or some special cases in JS)
        $content = $this->updateHtmlPathsToRelative($content, $url);

        // meta redirects e.g. in Astro projects - <meta http-equiv="refresh" content="0;url=/en/getting-started">
        if (preg_match('/(<meta[^>]*url=)([^"\']+)(["\'][^>]*>)/i', $content, $matches) === 1) {
            $content = str_replace($matches[0], $matches[1] . $this->convertUrlToRelative($url, $matches[2]) . $matches[3], $content);
        }

        // specific HTML changes
        $this->applySpecificHtmlChanges(
            $content,
            $url,
            $this->options->disableJavascript,
            $removeUnwantedCode, // removeCrossOrigins
            $removeUnwantedCode, // removeAnalytics
            $removeUnwantedCode, // removeSocnets
            $removeUnwantedCode  // removeCookiesRelated
        );

        // set JS variable with number of levels before close </head> tag and remove all anchor listeners when needed
        if ($this->scriptsEnabled) {
            $content = $this->setJsVariableWithUrlDepth($content, $baseUrl);
            if ($this->options->removeAllAnchorListeners || $this->isForcedToRemoveAnchorListeners($content)) {
                $content = $this->setJsFunctionToRemoveAllAnchorListeners($content);
            }
        }
    }

    /**
     * Update all paths to relative (for href, src, srcset and also for url() in CSS or some special cases in JS)
     *
     * @param string $html
     * @param ParsedUrl $parsedBaseUrl
     * @return string
     */
    private function updateHtmlPathsToRelative(string $html, ParsedUrl $parsedBaseUrl): string
    {
        $patternHrefSrc = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(href|src|component-url)\s*(=)\s*([\'"]?)([^\'">]+)\4([^>]*)/is';
        $patternSrcset = '/(\.|<[a-z0-9]{1,10}[^>]*\s+)(imagesrcset|srcset|renderer-url)\s*(=)\s*([\'"]?)([^\'">]+)\4([^>]*)/is';
        $patternMetaUrl = '/(<meta[^>]*)(url)\s*(=)\s*([\'"]?)([^\'">]+)\4(")/im';
        $escapedHref = '/(.)(href\\\\["\']|src\\\\["\'])([:=])(\\\\["\'])([^"\'\\\\]+)\\\\["\'](.)/is';

        $replaceCallback = function ($matches) use ($parsedBaseUrl) {
            $start = $matches[1];
            $attribute = trim($matches[2], ' \\"\'');
            $attributeRaw = $matches[2];
            $assignmentChar = $matches[3];
            $quote = $matches[4];
            $value = $matches[5];
            $end = $matches[6];

            // when modifying x.src (JS) and there is no quote, we do not convert, because it is not a valid URL but JS code
            if ($start === '.' && $quote === '') {
                return $matches[0];
            }

            // ignore data URI, dotted relative path or #anchor
            if (str_starts_with($value, '#') || preg_match('/^[a-z]+:[a-z0-9+]/i', $value) === 1) {
                return $matches[0];
            }

            // ignore and don't rewrite URLs that match the ignoreRegex
            foreach ($this->options->ignoreRegex as $ignoreRegex) {
                if (preg_match($ignoreRegex, $value) === 1) {
                    return $matches[0];
                }
            }

            if (in_array(strtolower($attribute), ['srcset', 'imagesrcset'])) {
                $sources = preg_split('/,\s/', $value);
                foreach ($sources as &$source) {
                    if (!str_contains($source, ' ')) {
                        // URL in srcset without a defined size by "url 2x", "url 100w", etc.
                        $relativeUrl = $this->convertUrlToRelative($parsedBaseUrl, trim($source), $attribute);
                        $source = $relativeUrl;
                    } else {
                        // URL in srcset with format "url 2x", "url 100w", etc.
                        @list($url, $size) = preg_split('/\s+/', trim($source), 2);
                        $relativeUrl = $this->convertUrlToRelative($parsedBaseUrl, $url, $attribute);
                        $source = $relativeUrl . ' ' . $size;
                    }
                }
                $newValue = implode(', ', $sources);
            } else {
                $newValue = $this->convertUrlToRelative($parsedBaseUrl, $value, $attribute);
            }

            // this solves issue Uncaught (in promise) TypeError: Failed to resolve module specifier '_astro/NewsletterForm.c015f42c.js'
            // See https://github.com/vitejs/vite/discussions/13536
            if (in_array($attribute, ['component-url', 'renderer-url'])) {
                $newValue = "./" . $newValue;
            }

            return $start . $attributeRaw . $assignmentChar . $quote . $newValue . $quote . $end;
        };

        $html = preg_replace_callback($patternHrefSrc, $replaceCallback, $html);
        $html = preg_replace_callback($patternSrcset, $replaceCallback, $html);
        $html = preg_replace_callback($patternMetaUrl, $replaceCallback, $html);
        $html = preg_replace_callback($escapedHref, $replaceCallback, $html);

        return $html;
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @param string $regexForHtmlExtensions
     * @return void
     * @throws \Exception
     */
    private function findHrefUrls(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls, string $regexForHtmlExtensions): void
    {
        preg_match_all('/<a[^>]*\shref=["\']?([^#][^"\'\s>]+)["\'\s]?[^>]*>/is', $html, $matches);
        $foundUrlsTxt = $matches[1];

        preg_match_all('/href\\\\["\'][:=]\\\\["\'](https?:\/\/[^"\'\\\\]+)\\\\["\']/i', $html, $matches);
        $foundUrlsTxt = array_merge($foundUrlsTxt, $matches[1] ?? []);

        // if $this->singleForeignPageOnly is set to true and if we crawl a $sourceUrl that is
        // on a different second-level domain than the initial URL, we won't look for links to other pages
        $initialUrl = $this->crawler->getInitialParsedUrl();
        if ($this->singleForeignPageOnly && $sourceUrl->domain2ndLevel !== $initialUrl->domain2ndLevel) {
            return;
        }

        if ($this->maxDepth > 0) {
            $crawler = $this->crawler;
            $foundUrlsTxt = array_filter($foundUrlsTxt, function ($url) use ($sourceUrl, $crawler) {
                $parsedUrl = ParsedUrl::parse($url, $sourceUrl);
                $result = $parsedUrl->getDepth() <= $this->maxDepth;
                if (!$result) {
                    $crawler->addUrlToSkipped($parsedUrl, Crawler::SKIPPED_REASON_EXCEEDS_MAX_DEPTH, $crawler->getUrlUqId($sourceUrl), FoundUrl::SOURCE_A_HREF);
                }
                return $result;
            });
        }

        if (!$this->filesEnabled) {
            $foundUrlsTxt = array_filter($foundUrlsTxt, function ($url) use ($regexForHtmlExtensions) {
                return preg_match('/\.[a-z0-9]{1,10}(|\?.*)$/i', $url) === 0 || preg_match($regexForHtmlExtensions, $url) === 1;
            });
        }

        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $sourceUrl->getFullUrl(true, false), FoundUrl::SOURCE_A_HREF);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findFonts(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        $sourceUrlWithoutFragment = $sourceUrl->getFullUrl(true, false);

        // CSS @font-face
        preg_match_all("/url\s*\(\s*['\"]?([^'\"\s>]+\.(eot|ttf|woff2|woff|otf)[^'\")]*)['\"]?\s*\)/is", $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_CSS_URL);

        // <link href="...(eot|ttf|woff2|woff|otf)
        preg_match_all('/<link\s+[^>]*href=["\']?([^"\' ]+\.(eot|ttf|woff2|woff|otf)[^"\' ]*)["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_LINK_HREF);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findImages(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        $sourceUrlWithoutFragment = $sourceUrl->getFullUrl(true, false);

        // <img src="..."
        preg_match_all('/<img\s+[^>]*?src=["\']?([^"\'> ]+)["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_IMG_SRC);

        // <input src="..."
        preg_match_all('/<input\s+[^>]*?src=["\']?([^"\'> ]+\.[a-z0-9]{1,10})["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_INPUT_SRC);

        // <link href="...(png|gif|jpg|jpeg|webp|avif|tif|bmp|svg)"
        preg_match_all('/<link\s+[^>]*?href=["\']?([^"\'> ]+\.(png|gif|jpg|jpeg|webp|avif|tif|bmp|svg|ico)(|\?[^"\' ]))["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_LINK_HREF);

        // <source src="..."
        preg_match_all('/<source\s+[^>]*?src=["\']([^"\'>]+)["\'][^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_SOURCE_SRC);

        // CSS url()
        preg_match_all("/url\s*\(\s*['\"]?([^'\")]+\.(jpg|jpeg|png|gif|bmp|tif|webp|avif)[^'\")]*)['\"]?\s*\)/is", $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_CSS_URL);

        // <picture><source srcset="..."><img src="..."></picture>
        // <img srcset="..."
        // <* imageSrcSet="..."

        $urls = [];
        preg_match_all('/<source\s+[^>]*?srcset=["\']([^"\'>]+)["\'][^>]*>/is', $html, $matches);
        $tmpMatches = $matches[1] ?? [];
        preg_match_all('/<img[^>]+srcset=["\']([^"\']+)["\']/is', $html, $matches);
        $tmpMatches = array_merge($tmpMatches, $matches[1] ?? []);
        preg_match_all('/<[a-z]+[^>]+imagesrcset=["\']([^"\']+)["\']/is', $html, $matches);
        $tmpMatches = array_merge($tmpMatches, $matches[1] ?? []);

        if ($tmpMatches) {
            foreach ($tmpMatches as $srcset) {
                // srcset can contain multiple sources separated by comma and whitespaces, not only comma because comma can be a valid part of the URL
                $sources = preg_split('/,\s/', $srcset);
                foreach ($sources as $source) {
                    list($url,) = preg_split('/\s+/', trim($source), 2);
                    if (!in_array($url, $urls)) {
                        $urls[] = trim($url);
                    }
                }
            }
        }
        $foundUrls->addUrlsFromTextArray(array_unique($urls), $sourceUrlWithoutFragment, FoundUrl::SOURCE_IMG_SRCSET);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findAudio(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        // <audio src="..."
        preg_match_all('/<audio\s+[^>]*?src=["\']?([^"\'> ]+)["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrl->getFullUrl(true, false), FoundUrl::SOURCE_AUDIO_SRC);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findVideo(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        // <video src="..."
        preg_match_all('/<video\s+[^>]*?src=["\']?([^"\'> ]+)["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrl->getFullUrl(true, false), FoundUrl::SOURCE_VIDEO_SRC);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findScripts(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        $sourceUrlWithoutFragment = $sourceUrl->getFullUrl(true, false);

        preg_match_all('/<script\s+[^>]*?src=["\']?([^"\' ]+)["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_SCRIPT_SRC);

        // <link href="...(js)"
        preg_match_all('/<link\s+[^>]*href=["\']?([^"\'> ]+\.(json|js)(|\?[^"\']))["\']?[^>]*>/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_LINK_HREF);

        // often used for lazy loading in JS code
        preg_match_all('/\.src\s*=\s*["\']([^"\']+)["\']/is', $html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrlWithoutFragment, FoundUrl::SOURCE_INLINE_SCRIPT_SRC);

        // NextJS chunks
        preg_match_all('/:([a-z0-9\/._\-\[\]]+chunks[a-z0-9\/._\-\[\]]+.js)/is', $html, $matches);
        $nextJsChunks = [];
        foreach ($matches[1] ?? [] as $match) {
            if (str_starts_with($match, '//')) {
                $chunkUrl = ($sourceUrl->scheme ?: 'https') . ':' . $match;
            } elseif (str_starts_with($match, 'http://') || str_starts_with($match, 'https://')) {
                $chunkUrl = $match;
            } elseif (str_contains($match, '/_next/')) {
                $chunkUrl = $match;
                if ($sourceUrl->host && $sourceUrl->host !== $this->crawler->getInitialParsedUrl()->host) {
                    $chunkUrl = $sourceUrl->getFullHomepageUrl() . $chunkUrl;
                }
            } else {
                $chunkUrl = $sourceUrl->getFullHomepageUrl() . '/_next/' . $match;
            }

            $nextJsChunks[] = $chunkUrl;
        }
        $foundUrls->addUrlsFromTextArray($nextJsChunks, $sourceUrlWithoutFragment, FoundUrl::SOURCE_INLINE_SCRIPT_SRC);
    }

    /**
     * @param string $html
     * @param ParsedUrl $sourceUrl
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findStylesheets(string $html, ParsedUrl $sourceUrl, FoundUrls $foundUrls): void
    {
        preg_match_all('/<link\s+[^>]*?href=["\']([^"\']+)["\'][^>]*>/is', $html, $matches);
        foreach ($matches[0] as $key => $match) {
            if (stripos($match, 'rel=') !== false && stripos($match, 'stylesheet') === false) {
                unset($matches[0][$key]);
                unset($matches[1][$key]);
            }
        }

        $foundUrls->addUrlsFromTextArray($matches[1], $sourceUrl->getFullUrl(true, false), FoundUrl::SOURCE_LINK_HREF);
    }

    /**
     * Remove all unwanted code from HTML with respect to --disable-* options
     *
     * @param string $html
     * @return string
     */
    private function removeUnwantedCodeFromHtml(string $html): string
    {
        if (!$this->scriptsEnabled) {
            $html = Utils::stripJavaScript($html);
        }
        if (!$this->stylesEnabled) {
            $html = Utils::stripStyles($html);
        }
        if (!$this->fontsEnabled) {
            $html = Utils::stripFonts($html);
        }
        if (!$this->imagesEnabled && stripos($html, '<img') !== false) {
            $html = Utils::stripImages($html);
            $html = $this->setCustomCssForTileImages($html);
            $html = Utils::addClassToHtmlImages($html, 'siteone-crawler-bg');
        }

        return $html;
    }

    private function setCustomCssForTileImages(string $html): string
    {
        // background is 64x36px with diagonal lines with transparent spaces between them and Crawler logo as a watermark
        $backgroundBase64 = 'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAkCAMAAAAO0sygAAAAAXNSR0IB2cksfwAAAAlwSFlzAAALEwAACxMBAJqcGAAAAMlQTFRFFxcXwMDA////1NTU5+fnpaWl0tLSIyMj5ubmlJSUxcXF29vbz8/P9PT01tbWxMTE8fHxaWlp39/f9fX1yMjI3NzciYmJeXl5Gxsb2dnZNTU18/PzXFxc5eXlJycnysrKZGRk6enp3d3dW1tbsrKyWFhYIiIi19fXvLy8w8PDuLi47e3tzMzM0dHRx8fH09PTHR0dzs7OLy8vwcHB0NDQSEhIqamp4uLiHh4eOzs74ODg3t7ewsLCISEhJCQkaGhoy8vLzc3N2NjYEPdgjAAAAaRJREFUeJzdlWlTgzAQhiGlWlsCCHgg9vAWpalavK3X//9RJptACoEwTp1x9MPOht19k+VJCIZhIvNXzVh1DmPVHowfe5cOsrpr5dh6D1kb/VJs0LWQ3cAAO7gyp0tjXinGavBmPQOf5gaVvgIaC5eet5jeceoZbNPcjhjvcu9GGHl7sjYGfbXP3HyaG/Dx/pD7UYDwOFT0ThuDSYwOahgcCn0rgyNWZykMQNtpYXBM9+wkhtreKZ8zZ8D52RoGEeT6E9EntTPmzxP5/mEIXscAX8SF/hL6TSEHc6VTRGbNDMyrYaElRPRxncr1bVpzEzczyHugNjeIGHO9ydbNMjomGgbUUq5PlDMzp3p5FpoYmLei77sERUJ//yBy0wy8lkFf8s/XV/rVMXjk9VahnYF/KvWrY/AMuZdFrvdQCHtPSnVt52BOfa/YP6LcBzoGfj73K1s/q70PtAzYt/DGx8P3Dx6r3Ad6Br6ce2GLmLwPknGAgigAPfNBFDffB6WzKRiMvGJfK/urMlgyycBV9FjDoLAlBl5V/9nwPXzb/tO/8e8y+AJh0S3ETlwQiAAAAABJRU5ErkJggg==';
        return preg_replace(
            '/<\s*\/\s*head\s*>/i',
            '<style>
                .siteone-crawler-bg {
                    background-image: url("data:image/png;base64,' . $backgroundBase64 . '");
                    background-repeat: repeat;
                    opacity: 0.15;
                }
            </style></head>',
            $html
        );
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

        $depth = substr_count(ltrim($basePath, '/'), '/');
        $baseUrlNeedsIndexHtml = $basePath !== '/' && str_ends_with($basePath, '/');
        if ($baseUrlNeedsIndexHtml) {
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
                setTimeout(_SiteOneRemoveAllAnchorListeners, 5000);
             </script></body>",
            $html
        );
    }

    /**
     * @param ParsedUrl $url
     * @param string $content
     * @return string
     */
    private function removeSchemaAndHostFromFullOriginUrls(ParsedUrl $url, string $content): string
    {
        $baseUrlRoot = preg_replace('/((https?:)?\/\/[^\/]+\/?).*/i', '$1', $url->getFullUrl());

        // normalize any port numbers in URLs
        $content = preg_replace('/((https?:)?\/\/[a-z0-9._-]+):[0-9]+/i', '$1', $content);

        // get all URLs from attributes and process them individually
        $patterns = [
            '/(href=(["\']))' . preg_quote($baseUrlRoot, '/') . '([^"\']*)(["\'])/i',
            '/(src=(["\']))' . preg_quote($baseUrlRoot, '/') . '([^"\']*)(["\'])/i',
            '/(url=(["\']))' . preg_quote($baseUrlRoot, '/') . '([^"\']*)(["\'])/i',
            '/(url\((["\']?))' . preg_quote($baseUrlRoot, '/') . '([^"\')]*)([\'"]\)|\))/i'
        ];

        foreach ($patterns as $pattern) {
            $content = preg_replace_callback($pattern, function ($matches) {
                $attrStart = $matches[1]; // href=", src=", url=" or url(
                $quote = $matches[2];     // quote character or empty for url()
                $path = $matches[3];      // URL path after baseUrlRoot
                $attrEnd = $matches[4];   // closing quote/bracket

                // full URL to check against ignore patterns
                $fullUrl = $matches[0];

                // check if URL matches any ignore pattern and if so, do not remove schema/host
                if ($this->options->ignoreRegex) {
                    foreach ($this->options->ignoreRegex as $ignorePattern) {
                        if (preg_match($ignorePattern, $fullUrl)) {
                            return $matches[0];
                        }
                    }
                }

                // no ignore pattern matched - remove schema/host
                return $attrStart . '/' . $path . $attrEnd;

            }, $content);
        }

        return $content;
    }

    /**
     * Apply specific changes to HTML related to the crawler options
     *
     * @param string $html
     * @param ParsedUrl $parsedBaseUrl
     * @param bool $removeExternalJs
     * @param bool $removeCrossOrigins
     * @param bool $removeAnalytics
     * @param bool $removeSocnets
     * @param bool $removeCookiesRelated
     * @return void
     */
    private function applySpecificHtmlChanges(string &$html, ParsedUrl $parsedBaseUrl, bool $removeExternalJs, bool $removeCrossOrigins, bool $removeAnalytics, bool $removeSocnets, bool $removeCookiesRelated): void
    {
        if (trim($html) === '') {
            return;
        }

        $baseHost = $parsedBaseUrl->host;

        if ($removeExternalJs) {
            $html = preg_replace_callback('/<script[^>]*src=["\']?(.*?)["\']?[^>]*>.*?<\/script>/is', function ($matches) use ($baseHost) {
                if (preg_match("/^(https?:)?\/\//i", $matches[1]) === 1 && parse_url($matches[1], PHP_URL_HOST) !== $baseHost) {
                    return '';
                }
                return $matches[0];
            }, $html);
        }

        if ($removeCrossOrigins) {
            $html = preg_replace('/(<link[^>]+)\s*crossorigin(\s*=\s*["\']?.*?["\']?)?(\s*[^>]*>)/i', '$1$3', $html);
            $html = preg_replace('/(<script[^>]+)\s*crossorigin(\s*=\s*["\']?.*?["\']?)?(\s*[^>]*>)/i', '$1$3', $html);
        }

        if ($removeAnalytics || $removeSocnets) {
            $patterns = [];

            if ($removeAnalytics) {
                $patternsAnalytics = [
                    'googletagmanager.com',
                    'google-analytics.com',
                    'ga.js',
                    'gtag.js',
                    'gtag(',
                    'analytics.',
                    'connect.facebook.net',
                    'fbq(', // Facebook Pixel
                ];
                $patterns = array_merge($patterns, $patternsAnalytics);
            }

            if ($removeSocnets) {
                $patternsSocnets = [
                    'connect.facebook.net',
                    'connect.facebook.com',
                    'twitter.com',
                    '.x.com',
                    'linkedin.com',
                    'instagram.com',
                    'pinterest.com',
                    'tumblr.com',
                    'plus.google.com',
                    'curator.io',
                ];
                $patterns = array_merge($patterns, $patternsSocnets);
            }

            if ($removeCookiesRelated) {
                $patternsCookies = [
                    'cookies',
                    'cookiebot',
                ];
                $patterns = array_merge($patterns, $patternsCookies);
            }

            $patterns = array_unique($patterns);

            $html = preg_replace_callback('/<script[^>]*>(.*?)<\/script>/is', function ($matches) use ($patterns) {
                if ($matches[0]) {
                    foreach ($patterns as $keyword) {
                        if (stripos($matches[0], $keyword) !== false) {
                            return '';
                        }
                    }
                }
                return $matches[0];
            }, $html);

            if ($removeSocnets && $html) {
                $html = preg_replace('/<iframe[^>]*(facebook\.com|twitter\.com|linkedin\.com)[^>]*>.*?<\/iframe>/is', '', $html);
            }
        }
    }

    /**
     * Is it necessary to remove all anchor listeners due to modern JS framework which will add click handler and prevent default behavior?
     * It is very hard to patch some framework's code (e.g. NextJS) to work properly with local file:// protocol
     *
     * @param string $html
     * @return bool
     */
    private function isForcedToRemoveAnchorListeners(string $html): bool
    {
        return str_contains($html, '_next/');
    }

}