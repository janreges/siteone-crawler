<?php

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\OutputType;
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
        $urls = $this->getJsonOutput()->getUrlsForSitemap(Crawler::URL_TYPE_HTML);
        if ($this->outputSitemapXml) {
            try {
                $sitemapFile = $this->generateXmlSitemap($this->outputSitemapXml, $urls);
                $this->output->addNotice("XML sitemap generated to '{$sitemapFile}'.");
            } catch (Exception $e) {
                $this->output->addError("Sitemap XML ERROR: {$e->getMessage()}");
            }
        }

        if ($this->outputSitemapTxt) {
            try {
                $sitemapFile = $this->generateTxtSitemap($this->outputSitemapTxt, $urls);
                $this->output->addNotice("TXT sitemap generated to '{$sitemapFile}'.");
            } catch (Exception $e) {
                $this->output->addError("Sitemap TXT ERROR: {$e->getMessage()}");
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

        $xml = new SimpleXMLElement('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="https://www.sitemaps.org/schemas/sitemap/0.9"></urlset>');
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

    private function getJsonOutput(): JsonOutput
    {
        $multiOutput = $this->output;
        /* @var $multiOutput MultiOutput */
        $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
        /* @var $jsonOutput JsonOutput */
        return $jsonOutput;
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