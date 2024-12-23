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
use Exception;

class AstroProcessor extends BaseProcessor implements ContentProcessor
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
        // component-url="/_astro/TestimoniesSlider.fb32dc5a.js" component-export="default" renderer-url="/_astro/client.c4e17359.js"
        if (str_contains($content, 'astro')) {
            $foundUrls = new FoundUrls();
            preg_match_all('/(component-url|renderer-url)=["\']([^"\']+)["\']/i', $content, $matches);

            foreach ($matches[2] ?? [] as $match) {
                $url = ParsedUrl::parse($match, $sourceUrl);
                $foundUrls->addUrl(new FoundUrl(
                    $url->getFullUrl(true, false),
                    $sourceUrl->getFullUrl(true, false),
                    FoundUrl::SOURCE_JS_URL
                ));
            }

            return $foundUrls;
        }
        return null;
    }

    /**
     * Astro will need to replace all modules with inline content due to CORS is blocking modules with file:// protocol
     *
     * @inheritDoc
     * @throws Exception
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        if (!str_contains($content, 'astro')) {
            return;
        }

        // stack of already included modules to prevent duplicates
        $alreadyIncludedModules = [];

        $replaceCallback = function ($match) use ($url, $alreadyIncludedModules) {
            $src = $match[1];
            $srcParsedUrl = ParsedUrl::parse($src, $url);
            $srcContent = $this->crawler->getStatus()->getStorage()->load(
                $this->crawler->getUrlUqId($srcParsedUrl)
            );

            $inlineModules = [];

            $srcContent = $this->detectAndIncludeOtherModules($srcContent, $srcParsedUrl, $inlineModules);
            $result = '';
            foreach ($inlineModules as $inlineModule) {
                $moduleMd5 = md5($inlineModule);
                if (isset($alreadyIncludedModules[$moduleMd5])) {
                    continue;
                }

                $result .= '<script type="module">' . $inlineModule . '</script>' . "\n";
                $alreadyIncludedModules[$moduleMd5] = true;
            }

            $result .= '<script type="module">' . $srcContent . '</script>';
            return $result;
        };

        $content = preg_replace_callback('/<script[^>]+type="module"[^>]+src="([^"]+)"[^>]*>\s*<\/script>/im', $replaceCallback, $content);
        $content = preg_replace_callback('/<script[^>]+src="([^"]+)"[^>]+type="module"[^>]*>\s*<\/script>/im', $replaceCallback, $content);
    }

    /**
     * @param string $moduleContent
     * @param ParsedUrl $moduleUrl
     * @param array $inlineModules
     * @param int $depth
     * @return string
     * @throws Exception
     */
    private function detectAndIncludeOtherModules(string $moduleContent, ParsedUrl $moduleUrl, array &$inlineModules, int $depth = 0): string
    {
        if ($depth > 10) {
            throw new Exception(__METHOD__ . ": Too many nested modules. Last module URL: {$moduleUrl->getFullUrl()}");
        }

        return preg_replace_callback('/import\s*["\']([^"\']+)["\']\s*;?/i', function ($match) use ($moduleUrl, $depth, &$inlineModules) {
            $src = trim($match[1]);
            $srcParsedUrl = ParsedUrl::parse($src, $moduleUrl);
            $srcContent = $this->crawler->getStatus()->getStorage()->load(
                $this->crawler->getUrlUqId($srcParsedUrl)
            );
            if (str_contains($srcContent, 'import')) {
                $srcContent = $this->detectAndIncludeOtherModules($srcContent, $srcParsedUrl, $inlineModules, $depth + 1);
            }
            $inlineModules[] = $srcContent;
            return $depth === 0 ? '/* SiteOne Crawler: imported as inline modules recursively */' : $match[1];
        }, $moduleContent);
    }

}