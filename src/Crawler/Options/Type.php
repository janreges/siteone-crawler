<?php

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
}
