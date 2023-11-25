<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result\Storage;

interface Storage
{
    public function save(string $uqId, string $content): void;

    public function load(string $uqId): string;

    public function delete(string $uqId): void;

    public function deleteAll(): void;
}