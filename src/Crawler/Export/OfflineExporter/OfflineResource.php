<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export\OfflineExporter;

use Crawler\ParsedUrl;
use Crawler\Result\VisitedUrl;

class OfflineResource
{
    public readonly ParsedUrl $targetUrl;
    public readonly ?string $relativeFilePath;

    public function __construct(ParsedUrl $targetUrl, ?string $relativeFilePath)
    {
        $this->targetUrl = $targetUrl;
        $this->relativeFilePath = $relativeFilePath;
    }
}