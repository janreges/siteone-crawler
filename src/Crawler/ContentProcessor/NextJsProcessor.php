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

class NextJsProcessor extends BaseProcessor implements ContentProcessor
{
    protected array $relevantContentTypes = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_SCRIPT,
        Crawler::CONTENT_TYPE_ID_STYLESHEET,
    ];

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        $isNextJsManifest = str_contains($sourceUrl->path, '_next/') && stripos($sourceUrl->path, 'manifest') !== false;
        if (!$isNextJsManifest) {
            return null;
        }

        $nextJsBaseDir = preg_replace('/(\/_next\/).*$/', '$1', $sourceUrl->path);

        preg_match_all('/["\']([a-z0-9\/._\-\[\]]\.js)["\']/is', $content, $matches);
        $foundUrlsTxt = [];
        foreach ($matches[1] ?? [] as $match) {
            $foundUrlsTxt[] = $nextJsBaseDir . $match;
        }

        $foundUrls = new FoundUrls();
        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $sourceUrl->path, FoundUrl::SOURCE_JS_URL);
        return $foundUrls;
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        // do nothing if html/js does not contain _next (each html/js of NextJS contains _next)
        if (stripos($content, '_next') === false) {
            return;
        }

        // disable prefetching in NextJS
        $content = preg_replace('/(prefetch:\([a-z]+,[a-z]+\)=>\{)if/i', '$1 return; if', $content);

        // add relative prefix to all _next/
        $basePath = $url->path;
        $depth = $basePath ? substr_count(ltrim($basePath, '/'), '/') : 0;
        $baseUrlNeedsIndexHtml = $basePath !== '/' && $basePath && str_ends_with($basePath, '/');
        if ($baseUrlNeedsIndexHtml) {
            $depth++;
        }

        $nextJsPrefix1 = $depth > 0 ? str_repeat('../', $depth) : './';
        $content = preg_replace('/\\\\(["\'])\/_next\//i', '\\\\$1' . $nextJsPrefix1 . '_next/', $content);

        $nextJsPrefix2 = '(' . HtmlProcessor::JS_VARIABLE_NAME_URL_DEPTH . ' > 0 ? "../".repeat(' . HtmlProcessor::JS_VARIABLE_NAME_URL_DEPTH . ') : "./")';
        $content = preg_replace('/([a-z0-9]+\.[a-z0-9]+=|:)(["\'])\/_next\//i', '$1' . $nextJsPrefix2 . ' + $2_next/', $content);

        // concat(e,"/_next/" -> concat(e,(PREFIX)"/next/")
        $content = preg_replace('/(concat\([a-z]+,)(["\']\/_next\/)(["\'])/i', '$1' . $nextJsPrefix2 . '+$2$3', $content);

        // remove <script id="__NEXT_DATA__" type="application/json">...</script>
        $emptyNextJsData = '<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{}}}</script>';
        $content = preg_replace('/<script[^>]+__NEXT_DATA__[^>]*>.*?<\/script>/is', $emptyNextJsData, $content);

        // add prefix to prefetch(t) { let ...}
        $content = preg_replace('/(prefetch\()([a-z]+)(\)\s*\{)\s*let/i', '$1$2$3 $2=' . $nextJsPrefix2 . '+$2; let', $content);

        // {href:"/".concat
        $content = preg_replace('/(\{href:)(["\'])(\/)(["\']\.)/i', '$1' . $nextJsPrefix2 . '+$2$3$4', $content);

        // push(["/[slug]"
        $content = preg_replace('/(push\(\[)(["\']\/)/i', '$1' . $nextJsPrefix2 . '+$2', $content);

        // return"?dpl=dpl_Es8ZzBRosxdiiRhkKSKrp9h56u6K"
        $content = preg_replace('/(return\s*["\'])\s*\?[^"\']+=[^"\']*(["\'])/i', '$1$2', $content);

        // ./_next/static/css/832b02c26afacbf3.css?dpl=dpl_Es8ZzBRosxdiiRhkKSKrp9h56u6K
        // static/chunks/css/832b02c26afacbf3.css?dpl=dpl_Es8ZzBRosxdiiRhkKSKrp9h56u6K
        $content = preg_replace('/((_next|chunks)\/[a-z0-9\/()\[\]._@%^{}-]+\.[a-z0-9]{1,5})\?[a-z0-9_&=.-]+/i', '$1', $content);
        $content = preg_replace('/\?dpl=[^"\' ]+/i', '', $content);
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesBeforeUrlParsing(string &$content, int $contentType, ParsedUrl $url): void
    {
        // do nothing if html/js does not contain _next (each html/js of NextJS contains _next)
        if (stripos($content, '_next') === false) {
            return;
        }

        // remove query params to static assets in NextJS
        $content = preg_replace('/((_next|chunks)\/[a-z0-9\/()\[\]._@%^{}-]+\.[a-z0-9]{1,5})\?[a-z0-9_&=.-]+/i', '$1', $content);
        $content = preg_replace('/\?dpl=[^"\' ]+/i', '', $content);
    }

    public function setDebugMode(bool $debugMode): void
    {
        // debug mode not implemented
    }

}