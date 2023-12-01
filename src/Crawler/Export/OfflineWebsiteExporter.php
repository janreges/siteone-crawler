<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

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
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use Exception;

class OfflineWebsiteExporter extends BaseExporter implements Exporter
{

    const GROUP_OFFLINE_WEBSITE_EXPORTER = 'offline-website-exporter';

    private static $contentTypesThatRequireChanges = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_SCRIPT,
        Crawler::CONTENT_TYPE_ID_STYLESHEET,
        Crawler::CONTENT_TYPE_ID_REDIRECT
    ];

    protected ?string $offlineExportDirectory = null;

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
                "Offline website generated to '%s' and took %s",
                Utils::getOutputFormattedPath($this->offlineExportDirectory),
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

        // apply required changes through all content processors
        if (in_array($visitedUrl->contentType, self::$contentTypesThatRequireChanges)) {
            $this->crawler->getContentProcessorManager()->applyContentChangesForOfflineVersion(
                $content,
                $visitedUrl->contentType,
                ParsedUrl::parse($visitedUrl->url)
            );
        }

        // sanitize and replace special chars because they are not allowed in file/dir names on some platforms (e.g. Windows)
        // same logic is in method convertUrlToRelative()
        $storeFilePath = sprintf('%s/%s',
            $this->offlineExportDirectory,
            OfflineUrlConverter::sanitizeFilePath($this->getRelativeFilePathForFileByUrl($visitedUrl), false)
        );

        $directoryPath = dirname($storeFilePath);
        if (!is_dir($directoryPath)) {
            if (!mkdir($directoryPath, 0777, true)) {
                throw new Exception("Cannot create directory '$directoryPath'");
            }
        }

        $saveFile = true;
        clearstatcache(true);

        // do not overwrite existing file if initial request was HTTPS and this request is HTTP, otherwise referenced
        // http://your.domain.tld/ will override wanted HTTPS page with small HTML file with meta redirect
        if (is_file($storeFilePath)) {
            if (!$visitedUrl->isHttps() && $this->crawler->getInitialParsedUrl()->isHttps()) {
                $saveFile = false;
                $message = "File '$storeFilePath' already exists and will not be overwritten because initial request was HTTPS and this request is HTTP: " . $visitedUrl->url;
                $this->output->addNotice($message);
                $this->status->addNoticeToSummary('offline-exporter-store-file-ignored', $message);
            }
        }

        if ($saveFile && file_put_contents($storeFilePath, $content) === false) {
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
            $this->crawler->getInitialParsedUrl(),
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
            new Option('--offline-export-dir', '-oed', 'offlineExportDirectory', Type::DIR, false, 'Path to directory where to save the offline version of the website.', null, true),
            new Option('--offline-export-store-only-url-regex', null, 'offlineExportStoreOnlyUrlRegex', Type::REGEX, true, 'For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.', null, true),
        ]));
        return $options;
    }
}
