<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class Info
{

    public readonly string $name;
    public readonly string $version;
    public readonly string $executedAt;
    public readonly string $command;
    public readonly string $hostname;
    public string $finalUserAgent;

    /**
     * @param string $name
     * @param string $version
     * @param string $executedAt
     * @param string $command
     * @param string $hostname
     * @param string $finalUserAgent
     */
    public function __construct(string $name, string $version, string $executedAt, string $command, string $hostname, string $finalUserAgent)
    {
        $this->name = $name;
        $this->version = $version;
        $this->executedAt = $executedAt;
        $this->command = $command;
        $this->hostname = $hostname;
        $this->finalUserAgent = $finalUserAgent;
    }

    public function setFinalUserAgent(string $finalUserAgent): void
    {
        $this->finalUserAgent = $finalUserAgent;
    }

}