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

class JavaScriptProcessor extends BaseProcessor implements ContentProcessor
{
    protected array $relevantContentTypes = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_SCRIPT,
    ];

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        return $this->findUrlsImportFrom($content, $sourceUrl);
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        $content = str_ireplace('crossorigin', '_SiteOne_CO_', $content);

        // $webpackPathPrefix is JS-code which replace first "/" in URL with our prefix (calculated in each HTML file and set to special _SiteOneUrlDepth variable)
        $webpackPathPrefix = '(' . HtmlProcessor::JS_VARIABLE_NAME_URL_DEPTH . ' > 0 ? "../".repeat(' . HtmlProcessor::JS_VARIABLE_NAME_URL_DEPTH . ') : "./")';

        // webpack case - replace a.p="/" with a.p="WITH_OUR_PREFIX/"
        if (stripos($content, 'a.p=') !== false) {
            $content = preg_replace('/a\.p="\/"/', 'a.p=' . $webpackPathPrefix, $content);
        }

        // webpack cases - replace href/path/Path:".." with href/path/Path:"WITH_OUR_PREFIX/.."
        if (stripos($content, 'href:"/') !== false) {
            $content = preg_replace('/href:"\//', 'href:' . $webpackPathPrefix . '+"', $content);
        }
        if (stripos($content, 'path:"/') !== false) {
            $content = preg_replace('/path:"\//', 'href:' . $webpackPathPrefix . '+"', $content);
        }
        if (stripos($content, 'Path:"/') !== false) {
            $content = preg_replace('/Path:"\//', 'href:' . $webpackPathPrefix . '+"', $content);
        }
    }

    /**
     * Find URLs in JavaScript import from statements
     * Example JS content ...import{R as W}from"./Repl.209fef3e.js";...
     *
     * @param string $content
     * @param ParsedUrl $sourceUrl
     * @return FoundUrls|null
     */
    private function findUrlsImportFrom(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        $isHtmlFile = stripos($content, '<html') !== false;
        if ($isHtmlFile || !str_contains($content, 'from')) {
            return null;
        }

        preg_match_all('/from\s*["\']([^"\']+\.js[^"\']*)["\']/i', $content, $matches);
        $foundUrlsTxt = [];
        foreach ($matches[1] ?? [] as $match) {
            $foundUrlsTxt[] = trim($match);
        }

        // example JS from docs.netlify.com - "/assets/js/12.c6446aa6.js","/assets/js/120.03870a87.js"
        // we are strict here - the path must be in quotes, start with a slash and end with *.js
        preg_match_all('/["\'](\/[^"\']+\.js)["\']/i', $content, $matches);
        foreach ($matches[1] ?? [] as $match) {
            $foundUrlsTxt[] = trim($match);
        }

        // example JS from docs.netlify.com - "/assets/js/12.c6446aa6.js","/assets/js/120.03870a87.js"
        // we are strict here - the path must be in quotes, start with a slash and end with *.js
        preg_match_all('/["\'](https:\/\/[^"\']+\.js)["\']/i', $content, $matches);
        foreach ($matches[1] ?? [] as $match) {
            $foundUrlsTxt[] = trim($match);
        }

        // special webpack case (we need to build all chunks urls), JS example: function(e){return a.p+"assets/js/"+({5:"vendors~docsearch"}[e]||e)+"."+{1:"5152a0bf",2:"f24bc225",3:"be674a14",5:"168e5268",6:"503c0dbb",7:"9db8eec7",8:"636a1276",9:"01fad13a",10:"330d609b",11:"470e17cb",12:"c6446aa6",13:"90299b76",14:"b2dcb0d8",15:"6d589b72",16:"dc8f34ea",17:"4f2d0100",18:"99c55d9f",19:"303e86ef"
        $pattern = '/"assets\/js\/".*\+.*\(\{([^}]*)\}.*\[e\].*\|\|.*e\)\s*\+\s*"\.".*\+\s*\{([^}]+)\}/i';
        preg_match($pattern, $content, $matches);
        // matches[1] example from docs.netlify.com: {5:"vendors~docsearch"}
        $tmpWebpack = [];
        if (isset($matches[1]) && $matches[1]) {
            $items = explode(',', $matches[1]);
            foreach ($items as $item) {
                if (preg_match('/([0-9]+):\s*"([^"\']+)"/', $item, $itemMatches)) {
                    $tmpWebpack[$itemMatches[1]] = $itemMatches[2];
                }
            }
        }
        if (isset($matches[2]) && $matches[2]) {
            $items = explode(',', $matches[2]);
            foreach ($items as $item) {
                if (preg_match('/([0-9]+):\s*"([a-f0-9]+)"/', $item, $itemMatches)) {
                    $foundUrlsTxt[] = "/assets/js/{$itemMatches[1]}.{$itemMatches[2]}.js";
                }

                // special case: {5:"vendors~docsearch"} -> "/assets/js/vendors~docsearch.168e5268.js"
                if (isset($tmpWebpack[$itemMatches[1]])) {
                    $foundUrlsTxt[] = "/assets/js/{$tmpWebpack[$itemMatches[1]]}.{$itemMatches[2]}.js";
                }
            }
        }

        if (!$foundUrlsTxt) {
            return null;
        }

        $foundUrls = new FoundUrls();
        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $sourceUrl->path, FoundUrl::SOURCE_JS_URL);
        return $foundUrls;
    }

}