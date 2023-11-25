<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Options;

use InvalidArgumentException;

class Group
{

    /**
     * Unique application code for the group
     * @var string
     */
    public readonly string $aplCode;

    /**
     * Readable name for the group
     * @var string
     */
    public readonly string $name;

    /**
     * Array of options - key is property name to fill
     * @var Option[]
     */
    public readonly array $options;

    /**
     * @param string $aplCode
     * @param string $name
     * @param Option[] $options
     * @throws InvalidArgumentException
     */
    public function __construct(string $aplCode, string $name, array $options)
    {
        foreach ($options as $option) {
            if (!($option instanceof Option)) {
                throw new InvalidArgumentException('Options must be instance of Option class');
            }
        }
        $this->aplCode = $aplCode;
        $this->name = $name;

        $optionsWithKeys = [];
        foreach ($options as $option) {
            $optionsWithKeys[$option->propertyToFill] = $option;
        }
        $this->options = $optionsWithKeys;
    }

}