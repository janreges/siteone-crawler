<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Options;

enum Type
{
    case INT;
    case FLOAT;
    case BOOL;
    case STRING;
    case SIZE_M_G;
    case EMAIL;
    case URL;
    case REGEX;
    case FILE;
    case DIR;
    case PROXY;
}
