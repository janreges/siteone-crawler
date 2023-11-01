<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Parser;

use Crawler\FoundUrl;
use Crawler\FoundUrls;

class HtmlUrlParser
{

    public static array $htmlPagesExtensions = ['htm', 'html', 'shtml', 'php', 'phtml', 'ashx', 'xhtml', 'asp', 'aspx', 'jsp', 'jspx', 'do', 'cfm', 'cgi', 'pl'];

    private readonly string $html;
    private readonly string $sourceUrl;
    private readonly bool $files;
    private readonly bool $images;
    private readonly bool $scripts;
    private readonly bool $styles;
    private readonly bool $fonts;

    /**
     * @param string $html
     * @param string $sourceUrl
     * @param bool $files
     * @param bool $images
     * @param bool $scripts
     * @param bool $styles
     * @param bool $fonts
     */
    public function __construct(string $html, string $sourceUrl, bool $files, bool $images, bool $scripts, bool $styles, bool $fonts)
    {
        $this->html = self::treatFrameworkSpecificCode($html);
        $this->sourceUrl = $sourceUrl;
        $this->files = $files;
        $this->images = $images;
        $this->scripts = $scripts;
        $this->styles = $styles;
        $this->fonts = $fonts;
    }


    /**
     * @return FoundUrls
     */
    public function getUrlsFromHtml(): FoundUrls
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', self::$htmlPagesExtensions) . ')/i';
        }

        $foundUrls = new FoundUrls();

        $this->findHrefUrls($foundUrls, $regexForHtmlExtensions);

        if ($this->fonts) {
            $this->findFonts($foundUrls);
        }

        if ($this->images) {
            $this->findImages($foundUrls);
        }

        if ($this->scripts) {
            $this->findScripts($foundUrls);
        }

        if ($this->styles) {
            $this->findStylesheets($foundUrls);
        }

        return $foundUrls;
    }

    /**
     * @param FoundUrls $foundUrls
     * @param string $regexForHtmlExtensions
     * @return void
     */
    private function findHrefUrls(FoundUrls $foundUrls, string $regexForHtmlExtensions): void
    {
        preg_match_all('/<a[^>]*\shref=["\']?([^#][^"\'\s]+)["\'\s]?[^>]*>/im', $this->html, $matches);
        $foundUrlsTxt = $matches[1];

        if (!$this->files) {
            $foundUrlsTxt = array_filter($foundUrlsTxt, function ($url) use ($regexForHtmlExtensions) {
                return preg_match('/\.[a-z0-9]{1,10}(|\?.*)$/i', $url) === 0 || preg_match($regexForHtmlExtensions, $url) === 1;
            });
        }

        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $this->sourceUrl, FoundUrl::SOURCE_A_HREF);
    }

    /**
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findFonts(FoundUrls $foundUrls): void
    {
        // CSS @font-face
        preg_match_all("/url\s*\(\s*['\"]?([^'\"\s>]+\.(eot|ttf|woff2|woff|otf))/im", $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_CSS_URL);

        // <link href="...(eot|ttf|woff2|woff|otf)
        preg_match_all('/<link\s+[^>]*href=["\']([^"\']+\.(eot|ttf|woff2|woff|otf)[^"\']*)["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_LINK_HREF);
    }

    /**
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findImages(FoundUrls $foundUrls): void
    {
        // <img src="..."
        preg_match_all('/<img\s+[^>]*?src=["\']([^"\'>]+)["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_IMG_SRC);

        // <input src="..."
        preg_match_all('/<input\s+[^>]*?src=["\']([^"\'>]+\.[a-z0-9]{1,10})["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_IMG_SRC);

        // <link href="...(png|gif|jpg|jpeg|webp|avif|tif|bmp|svg)"
        preg_match_all('/<link\s+[^>]*?href=["\']([^"\'>]+\.(png|gif|jpg|jpeg|webp|avif|tif|bmp|svg|ico)(|\?[^"\']))["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_LINK_HREF);

        // <source src="..."
        preg_match_all('/<source\s+[^>]*?src=["\']([^"\'>]+)["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_IMG_SRC);

        // find
        preg_match_all("/url\s*\(\s*['\"]?([^'\")]+\.(jpg|jpeg|png|gif|bmp|tif|webp|avif))/im", $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_CSS_URL);

        // <picture><source srcset="..."><img src="..."></picture>
        // <img srcset="..."
        // <* imageSrcSet="..."

        $urls = [];
        preg_match_all('/<source\s+[^>]*?srcset=["\']([^"\'>]+)["\'][^>]*>/im', $this->html, $matches);
        $tmpMatches = $matches[1] ?? [];
        preg_match_all('/<img[^>]+srcset=["\']([^"\']+)["\']/im', $this->html, $matches);
        $tmpMatches = array_merge($tmpMatches, $matches[1] ?? []);
        preg_match_all('/<[a-z]+[^>]+imagesrcset=["\']([^"\']+)["\']/im', $this->html, $matches);
        $tmpMatches = array_merge($tmpMatches, $matches[1] ?? []);

        if ($tmpMatches) {
            foreach ($tmpMatches as $srcset) {
                $sources = preg_split('/\s*,\s*/', $srcset);
                foreach ($sources as $source) {
                    list($url,) = preg_split('/\s+/', trim($source), 2);
                    if (!in_array($url, $urls)) {
                        $urls[] = trim($url);
                    }
                }
            }
        }
        $foundUrls->addUrlsFromTextArray(array_unique($urls), $this->sourceUrl, FoundUrl::SOURCE_IMG_SRC);
    }

    /**
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findScripts(FoundUrls $foundUrls): void
    {
        preg_match_all('/<script\s+[^>]*?src=["\']([^"\']+)["\'][^>]*>/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_SCRIPT_SRC);

        // often used for lazy loading in JS code
        preg_match_all('/\.src\s*=\s*["\']([^"\']+)["\']/im', $this->html, $matches);
        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_INLINE_SCRIPT_SRC);
    }

    /**
     * @param FoundUrls $foundUrls
     * @return void
     */
    private function findStylesheets(FoundUrls $foundUrls): void
    {
        preg_match_all('/<link\s+[^>]*?href=["\']([^"\']+)["\'][^>]*>/im', $this->html, $matches);
        foreach ($matches[0] as $key => $match) {
            if (stripos($match, 'rel=') !== false && stripos($match, 'stylesheet') === false) {
                unset($matches[0][$key]);
                unset($matches[1][$key]);
            }
        }

        $foundUrls->addUrlsFromTextArray($matches[1], $this->sourceUrl, FoundUrl::SOURCE_LINK_HREF);
    }

    /**
     * Treat some framework-specific code which could cause problems during parsing
     *
     * @param string $html
     * @return string
     */
    public static function treatFrameworkSpecificCode(string $html): string
    {
        if (str_contains($html, '<svelte:')) {
            $html = preg_replace('/<svelte:[^>]+>\s*/i', '', $html);
        }
        return $html;
    }

}