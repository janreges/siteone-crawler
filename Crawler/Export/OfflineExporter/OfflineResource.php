<?php

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