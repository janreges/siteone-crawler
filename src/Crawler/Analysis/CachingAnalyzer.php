<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;

class CachingAnalyzer extends BaseAnalyzer implements Analyzer
{
    const GROUP_CONTENT_TYPE_ANALYZER = 'content-type-analyzer';
    const SUPER_TABLE_CACHING_PER_CONTENT_TYPE = 'caching-per-content-type';
    const SUPER_TABLE_CACHING_PER_DOMAIN = 'caching-per-domain';
    const SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE = 'caching-per-domain-and-content-type';

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $this->addContentTypeSuperTable();
    }

    private function addContentTypeSuperTable(): void
    {
        $statsPerContentType = [];
        $statsPerDomain = [];
        $statsPerDomainAndContentType = [];

        foreach ($this->status->getVisitedUrls() as $visitedUrl) {
            $contentTypeName = Utils::getContentTypeNameById($visitedUrl->contentType);
            $cacheTypeLabel = $visitedUrl->getCacheTypeLabel();
            $contentTypeKey = "$contentTypeName.$cacheTypeLabel";
            $domainName = $visitedUrl->getHost();

            $this->updateStatsPerDomain($visitedUrl, $domainName, $cacheTypeLabel, $statsPerDomain);
            $this->updateStatsPerDomainAndContentType($visitedUrl, $domainName, $contentTypeName, $cacheTypeLabel, $statsPerDomainAndContentType);

            if ($visitedUrl->isAllowedForCrawling) {
                $this->updateStatsPerContentType($contentTypeKey, $visitedUrl, $contentTypeName, $cacheTypeLabel, $statsPerContentType);
            }
        }

        $lifetimeFormatter = function ($value) {
            if (is_numeric($value)) {
                return Utils::getColoredCacheLifetime(intval($value), 6);
            } else {
                return '-';
            }
        };

        if ($statsPerContentType) {
            $this->addSuperTablePerContentType($statsPerContentType, $lifetimeFormatter);
        }

        $this->addSuperTablePerDomain($statsPerDomain, $lifetimeFormatter);
        $this->addSuperTablePerDomainAndContentType($statsPerDomainAndContentType, $lifetimeFormatter);
    }

    private function addSuperTablePerContentType(array $data, callable $lifetimeFormatter): void
    {
        $superTable = new SuperTable(
            self::SUPER_TABLE_CACHING_PER_CONTENT_TYPE,
            "HTTP Caching by content type (only from crawlable domains)",
            "No URLs found.",
            [
                new SuperTableColumn('contentType', 'Content type', 12),
                new SuperTableColumn('cacheType', 'Cache type', 12),
                new SuperTableColumn('count', 'URLs', 5),
                new SuperTableColumn('avgLifetime', 'AVG lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('minLifetime', 'MIN lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('maxLifetime', 'MAX lifetime', 10, $lifetimeFormatter)
            ], true, 'count', 'DESC', null, null, 'HTTP cache'
        );

        $superTable->setData($data);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    private function addSuperTablePerDomain(array $data, callable $lifetimeFormatter): void
    {
        $superTable = new SuperTable(
            self::SUPER_TABLE_CACHING_PER_DOMAIN,
            "HTTP Caching by domain",
            "No URLs found.",
            [
                new SuperTableColumn('domain', 'Domain', 20),
                new SuperTableColumn('cacheType', 'Cache type', 12),
                new SuperTableColumn('count', 'URLs', 5),
                new SuperTableColumn('avgLifetime', 'AVG lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('minLifetime', 'MIN lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('maxLifetime', 'MAX lifetime', 10, $lifetimeFormatter)
            ], true, 'count', 'DESC'
        );

        $superTable->setData($data);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    private function addSuperTablePerDomainAndContentType(array $data, callable $lifetimeFormatter): void
    {
        $superTable = new SuperTable(
            self::SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE,
            "HTTP Caching by domain and content type",
            "No URLs found.",
            [
                new SuperTableColumn('domain', 'Domain', 20),
                new SuperTableColumn('contentType', 'Content type', 12),
                new SuperTableColumn('cacheType', 'Cache type', 12),
                new SuperTableColumn('count', 'URLs', 5),
                new SuperTableColumn('avgLifetime', 'AVG lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('minLifetime', 'MIN lifetime', 10, $lifetimeFormatter),
                new SuperTableColumn('maxLifetime', 'MAX lifetime', 10, $lifetimeFormatter)
            ], true, 'count', 'DESC'
        );

        $superTable->setData($data);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    /**
     * @param VisitedUrl $visitedUrl
     * @param string $domain
     * @param string $cacheTypeLabel
     * @param array $stats
     * @return void
     */
    public function updateStatsPerDomain(VisitedUrl $visitedUrl, string $domain, string $cacheTypeLabel, array &$stats): void
    {
        $key = "$domain.$cacheTypeLabel";

        // stats per domain
        if (!isset($stats[$key])) {
            $stats[$key] = [
                'domain' => $domain,
                'cacheType' => $cacheTypeLabel,
                'count' => 0,
                'countWithLifetime' => 0,
                'totalLifetime' => null,
                'avgLifetime' => null,
                'minLifetime' => null,
                'maxLifetime' => null,
            ];
        }

        $stats[$key]['count']++;
        if (is_int($visitedUrl->cacheLifetime)) {
            $stats[$key]['countWithLifetime']++;
            $stats[$key]['totalLifetime'] += ($visitedUrl->cacheLifetime ?: 0);
            $stats[$key]['avgLifetime'] = $stats[$key]['totalLifetime'] / $stats[$key]['countWithLifetime'];
            $stats[$key]['minLifetime'] = $stats[$key]['minLifetime'] === null ? $visitedUrl->cacheLifetime : intval(min($stats[$key]['minLifetime'], $visitedUrl->cacheLifetime));
            $stats[$key]['maxLifetime'] = $stats[$key]['maxLifetime'] === null ? $visitedUrl->cacheLifetime : intval(max($stats[$key]['maxLifetime'], $visitedUrl->cacheLifetime));
        }
    }

    /**
     * @param VisitedUrl $visitedUrl
     * @param string $domainName
     * @param string $contentTypeName
     * @param string $cacheTypeLabel
     * @param array $stats
     * @return void
     */
    public function updateStatsPerDomainAndContentType(VisitedUrl $visitedUrl, string $domainName, string $contentTypeName, string $cacheTypeLabel, array &$stats): void
    {
        $key = "$domainName.$contentTypeName.$cacheTypeLabel";

        if (!isset($stats[$key])) {
            $stats[$key] = [
                'domain' => $domainName,
                'contentType' => $contentTypeName,
                'cacheType' => $cacheTypeLabel,
                'count' => 0,
                'countWithLifetime' => 0,
                'totalLifetime' => null,
                'avgLifetime' => null,
                'minLifetime' => null,
                'maxLifetime' => null,
            ];
        }

        $stats[$key]['count']++;
        if (is_int($visitedUrl->cacheLifetime)) {
            $stats[$key]['countWithLifetime']++;
            $stats[$key]['totalLifetime'] += ($visitedUrl->cacheLifetime ?: 0);
            $stats[$key]['avgLifetime'] = $stats[$key]['totalLifetime'] / $stats[$key]['countWithLifetime'];
            $stats[$key]['minLifetime'] = $stats[$key]['minLifetime'] === null ? $visitedUrl->cacheLifetime : intval(min($stats[$key]['minLifetime'], $visitedUrl->cacheLifetime));
            $stats[$key]['maxLifetime'] = $stats[$key]['maxLifetime'] === null ? $visitedUrl->cacheLifetime : intval(max($stats[$key]['maxLifetime'], $visitedUrl->cacheLifetime));
        }
    }

    /**
     * @param string $contentTypeKey
     * @param VisitedUrl $visitedUrl
     * @param string $contentTypeName
     * @param string $cacheTypeLabel
     * @param array $stats
     * @return void
     */
    public function updateStatsPerContentType(string $contentTypeKey, VisitedUrl $visitedUrl, string $contentTypeName, string $cacheTypeLabel, array &$stats): void
    {
        if (!isset($stats[$contentTypeKey])) {
            $stats[$contentTypeKey] = [
                'contentTypeId' => $visitedUrl->contentType,
                'contentType' => $contentTypeName,
                'cacheType' => $cacheTypeLabel,
                'count' => 0,
                'countWithLifetime' => 0,
                'totalLifetime' => null,
                'avgLifetime' => null,
                'minLifetime' => null,
                'maxLifetime' => null,
            ];
        }

        $stats[$contentTypeKey]['count']++;
        if (is_int($visitedUrl->cacheLifetime)) {
            $stats[$contentTypeKey]['countWithLifetime']++;
            $stats[$contentTypeKey]['totalLifetime'] += ($visitedUrl->cacheLifetime ?: 0);
            $stats[$contentTypeKey]['avgLifetime'] = $stats[$contentTypeKey]['totalLifetime'] / $stats[$contentTypeKey]['countWithLifetime'];
            $stats[$contentTypeKey]['minLifetime'] = $stats[$contentTypeKey]['minLifetime'] === null ? $visitedUrl->cacheLifetime : intval(min($stats[$contentTypeKey]['minLifetime'], $visitedUrl->cacheLifetime));
            $stats[$contentTypeKey]['maxLifetime'] = $stats[$contentTypeKey]['maxLifetime'] === null ? $visitedUrl->cacheLifetime : intval(max($stats[$contentTypeKey]['maxLifetime'], $visitedUrl->cacheLifetime));
        }
    }

    public function getOrder(): int
    {
        return 116;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}