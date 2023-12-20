<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use Exception;
use SimpleXMLElement;

class SitemapExporter extends BaseExporter implements Exporter
{
    const GROUP_SITEMAP = 'sitemap';

    protected ?string $outputSitemapXml = null;
    protected ?string $outputSitemapTxt = null;
    protected ?float $basePriority = null;
    protected ?float $priorityIncrease = null;

    public function shouldBeActivated(): bool
    {
        return $this->outputSitemapXml || $this->outputSitemapTxt;
    }

    public function export(): void
    {
        $urls = [];
        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            if (!$visitedUrl->isExternal && $visitedUrl->contentType === Crawler::CONTENT_TYPE_ID_HTML && $visitedUrl->statusCode === 200) {
                $urls[] = $visitedUrl->url;
            }
        }

        // sort $urls primary by number of dashes ASC, secondary alphabetically ASC
        usort($urls, function ($a, $b) {
            $aDashes = substr_count(rtrim($a, '/'), '/');
            $bDashes = substr_count(rtrim($b, '/'), '/');
            if ($aDashes === $bDashes) {
                return strcmp($a, $b);
            }
            return $aDashes - $bDashes;
        });

        if ($this->outputSitemapXml) {
            try {
                $sitemapFile = $this->generateXmlSitemap($this->outputSitemapXml, $urls);
                $outputSitemapFile = Utils::getOutputFormattedPath($sitemapFile);
                $this->status->addInfoToSummary('sitemap-xml', "XML sitemap generated to '{$outputSitemapFile}'");
            } catch (Exception $e) {
                $this->status->addCriticalToSummary('sitemap-xml', "Sitemap XML ERROR: {$e->getMessage()}");
            }
        }

        if ($this->outputSitemapTxt) {
            try {
                $sitemapFile = $this->generateTxtSitemap($this->outputSitemapTxt, $urls);
                $outputSitemapFile = Utils::getOutputFormattedPath($sitemapFile);
                $this->status->addInfoToSummary('sitemap-txt', "TXT sitemap generated to '{$outputSitemapFile}'");
            } catch (Exception $e) {
                $this->status->addCriticalToSummary('sitemap-txt', "Sitemap TXT ERROR: {$e->getMessage()}");
            }
        }
    }

    /**
     * @param string $outputFile
     * @param string[] $urls
     * @return string
     * @throws Exception
     */
    private function generateXmlSitemap(string $outputFile, array $urls): string
    {
        $outputFile = preg_replace('/\.xml$/i', '', $outputFile) . '.xml';
        if (!is_writable(dirname($outputFile)) && !is_writable($outputFile)) {
            throw new Exception("Output file {$outputFile} is not writable. Check permissions.");
        }

        $xml = new SimpleXMLElement('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="https://www.sitemaps.org/schemas/sitemap/0.9"><!-- Sitemap generated using SiteOne Crawler - https://crawler.siteone.io/features/sitemap-generator/ --></urlset>');
        foreach ($urls as $url) {
            $urlElement = $xml->addChild('url');
            $urlElement->addChild('loc', htmlspecialchars($url));

            $slashesCount = substr_count(parse_url($url, PHP_URL_PATH), '/');
            $priority = max(0.1, min($this->basePriority + ($this->priorityIncrease * (1 - $slashesCount)), 1.0));

            $urlElement->addChild('priority', number_format($priority, 1, '.', ''));
        }

        $xml->asXML($outputFile);
        return $outputFile;
    }

    /**
     * @param string $outputFile
     * @param string[] $urls
     * @return string
     * @throws Exception
     */
    private function generateTxtSitemap(string $outputFile, array $urls): string
    {
        $outputFile = preg_replace('/\.txt$/i', '', $outputFile) . '.txt';
        if (!is_writable(dirname($outputFile)) && !is_writable($outputFile)) {
            throw new Exception("Output file {$outputFile} is not writable. Check permissions.");
        }

        $sitemapContent = implode("\n", $urls);
        file_put_contents($outputFile, $sitemapContent);

        return $outputFile;
    }

    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_SITEMAP,
            'Sitemap options', [
            new Option('--sitemap-xml-file', null, 'outputSitemapXml', Type::FILE, false, 'Save sitemap to XML. `.xml` added if missing.', null, true),
            new Option('--sitemap-txt-file', null, 'outputSitemapTxt', Type::FILE, false, 'Save sitemap to TXT. `.txt` added if missing.', null, true),
            new Option('--sitemap-base-priority', null, 'basePriority', Type::FLOAT, false, 'Base priority for XML sitemap.', 0.5, false),
            new Option('--sitemap-priority-increase', null, 'priorityIncrease', Type::FLOAT, false, 'Priority increase value based on slashes count in the URL', 0.1, false),
        ]));
        return $options;
    }


}