<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Options;

class Options
{

    /**
     * @var Group[]
     */
    private array $groups = [];

    public function addGroup(Group $group): void
    {
        $this->groups[$group->aplCode] = $group;
    }

    /**
     * @return Group[]
     */
    public function getGroups(): array
    {
        return $this->groups;
    }

    public function getGroup(string $aplCode): ?Group
    {
        return $this->groups[$aplCode] ?? null;
    }

}