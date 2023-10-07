<?php

namespace Crawler\Export;

class SitemapExporter
{

    /**
     * @var string[]
     */
    private array $urls;

    /**
     * @var float[]
     * Options for generating the sitemap. Possible keys are:
     *  - 'basePriority': (float) Base priority for the URLs. Default is 0.5.
     *  - 'priorityIncrease': (float) The increase in priority for each level up in URL hierarchy. Default is 0.1.
     */
    private array $options = [];

    /**
     * @param string[] $urls
     * @param array $options
     */
    public function __construct(array $urls, array $options)
    {
        $this->urls = $urls;
        $this->options = $options;

        sort($this->urls);
    }

    public function generateXmlSitemap(string $outputFile): void
    {
        if (!is_writable(dirname($outputFile)) && !is_writable($outputFile)) {
            throw new \Exception("Output file {$outputFile} is not writable. Check permissions.");
        }

        $basePriority = $this->options['basePriority'] ?? 0.7;
        $priorityIncrease = $this->options['priorityIncrease'] ?? 0.1;

        $xml = new \SimpleXMLElement('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>');
        foreach ($this->urls as $url) {
            $urlElement = $xml->addChild('url');
            $urlElement->addChild('loc', htmlspecialchars($url));

            $slashesCount = substr_count(parse_url($url, PHP_URL_PATH), '/');
            $priority = max(0.1, min($basePriority + ($priorityIncrease * (1 - $slashesCount)), 1.0));

            $urlElement->addChild('priority', number_format($priority, 1, '.', ''));
        }

        $xml->asXML($outputFile);
    }

    public function generateTxtSitemap(string $outputFile): void
    {
        if (!is_writable(dirname($outputFile)) && !is_writable($outputFile)) {
            throw new \Exception("Output file {$outputFile} is not writable. Check permissions.");
        }
        $sitemapContent = implode("\n", $this->urls);
        file_put_contents($outputFile, $sitemapContent);
    }

}