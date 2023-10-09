<?php

namespace Crawler\Options;

enum Type
{
    case INT;
    case FLOAT;
    case BOOL;
    case STRING;
    case EMAIL;
    case URL;
    case REGEX;
    case FILE;
    case DIR;
}
