<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis\Result;

class SecurityResult
{

    /**
     * @var SecurityCheckedHeader[]
     */
    public array $checkedHeaders = [];

    public function getCheckedHeader(string $header): SecurityCheckedHeader
    {
        if (!isset($this->checkedHeaders[$header])) {
            $this->checkedHeaders[$header] = new SecurityCheckedHeader($header);
        }
        return $this->checkedHeaders[$header];
    }

    public function getHighestSeverity(): int
    {
        $highestSeverity = SecurityCheckedHeader::OK;
        foreach ($this->checkedHeaders as $item) {
            if ($item->highestSeverity > $highestSeverity) {
                $highestSeverity = $item->highestSeverity;
            }
        }
        return $highestSeverity;
    }

}