<?php

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Options;

class Page404Analyzer extends BaseAnalyzer implements Analyzer
{
    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $urls404 = array_filter($this->status->getVisitedUrls(), function ($visitedUrl) {
            return $visitedUrl->statusCode === 404;
        });

        $status = $this->status;
        $superTable = new SuperTable(
            '404',
            '404 URLs',
            'No 404 URLs found.',
            [
                new SuperTableColumn('statusCode', 'Status', 6, null),
                new SuperTableColumn('url', 'URL 404', 100, null),
                new SuperTableColumn('sourceUqId', 'Found at URL', 100, function ($value) use ($status) {
                    return $value ? $status->getUrlByUqId($value) : '';
                }),
            ], true, 'url', 'ASC');

        $superTable->setData($urls404);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);
    }

    public function getOrder(): int
    {
        return 100;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}