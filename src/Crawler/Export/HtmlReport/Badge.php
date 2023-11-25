<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export\HtmlReport;

class Badge
{

    public const COLOR_RED = 'red';
    public const COLOR_ORANGE = 'orange';
    public const COLOR_GREEN = 'green';
    public const COLOR_BLUE = 'blue';
    public const COLOR_NEUTRAL = 'neutral';

    public readonly string $value;
    public readonly string $color;
    public readonly ?string $title;

    /**
     * @param string $value
     * @param string $color See Badge::COLOR_*
     * @param string|null $title
     */
    public function __construct(string $value, string $color, ?string $title = null)
    {
        $this->value = $value;
        $this->color = $color;
        $this->title = $title;
    }
}