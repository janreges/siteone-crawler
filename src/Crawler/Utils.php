<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class Utils
{
    private static bool $colorsEnabled = true;

    public static function disableColors(): void
    {
        self::$colorsEnabled = false;
    }

    public static function getFormattedSize(int $bytes, int $precision = 0): string
    {
        $units = array('B', 'kB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB');

        $bytes = max($bytes, 0);
        $pow = floor(($bytes ? log($bytes) : 0) / log(1024));
        $pow = min($pow, count($units) - 1);

        $bytes /= pow(1024, $pow);

        return round($bytes, $precision) . ' ' . $units[$pow];
    }

    public static function getFormattedDuration(float $duration): string
    {
        if ($duration < 1) {
            return number_format($duration * 1000, 0, '.', ' ') . ' ms';
        } elseif ($duration < 10) {
            return str_replace('.0', '', number_format($duration, 1, '.', ' ')) . ' s';
        } else {
            return number_format($duration, 0, '.', ' ') . ' s';
        }
    }

    public static function getFormattedAge(int $age): string
    {
        if ($age < 60) {
            return $age . ' sec(s)';
        } elseif ($age < 3600) {
            return round($age / 60, 1) . ' min(s)';
        } elseif ($age < 86400) {
            return round($age / 3600, 1) . ' hour(s)';
        } else {
            return round($age / 86400, 1) . ' day(s)';
        }
    }

    public static function getColorText(string $text, string $color, ?bool $setBackground = false): string
    {
        if (!self::$colorsEnabled) {
            return $text;
        }

        // if output is not visible (non-interactive mode), do not colorize text
        $isOutputVisible = posix_isatty(STDOUT);
        if (!$isOutputVisible) {
            return $text;
        }

        $colors = [
            'black' => '0;30',
            'red' => '0;31',
            'green' => '0;32',
            'yellow' => '0;33',
            'blue' => '0;34',
            'magenta' => '0;35',
            'cyan' => '0;36',
            'white' => '0;37',
            'gray' => '38;5;244',
            'dark-gray' => '38;5;240',
        ];

        $bgColors = [
            'black' => '1;40',
            'red' => '1;41',
            'green' => '1;42',
            'yellow' => '1;43',
            'blue' => '1;44',
            'magenta' => '1;45',
            'cyan' => '1;46',
            'white' => '1;47',
        ];

        if ($setBackground) {
            $coloredString = "\033[" . $bgColors[$color] . "m";
        } else {
            $coloredString = "\033[" . $colors[$color] . "m";
        }

        $coloredString .= $text . "\033[0m";

        return $coloredString;
    }

    public static function convertBashColorsInTextToHtml(string $text): string
    {
        $text = preg_replace_callback('/\033\[(.*?)m(.*?)\033\[0m/', function ($matches) {
            $styles = explode(';', $matches[1]);
            $fontColor = null;
            $backgroundColor = null;
            foreach ($styles as $style) {
                if (in_array($style, ['30', '31', '32', '33', '34', '35', '36', '37'])) {
                    $fontColor = $style;
                } else if (in_array($style, ['40', '41', '42', '43', '44', '45', '46', '47'])) {
                    $backgroundColor = $style;
                }
            }

            $style = '';
            if ($fontColor) {
                $style .= 'color: ' . self::getHtmlColorByBashColor($fontColor) . ';';
            }
            if ($backgroundColor) {
                $style .= 'background-color: ' . self::getHtmlColorByBashColor($backgroundColor) . ';';
            }

            $style = trim($style, ';');
            if ($style) {
                return '<span style="' . $style . '">' . $matches[2] . '</span>';
            } else {
                return $matches[2];
            }
        }, $text);

        return $text;

    }

    private static function getHtmlColorByBashColor(string $color): string
    {
        static $colors = [
            '30' => '#000000',
            '31' => '#ff0000',
            '32' => '#00ff00',
            '33' => '#ffff00',
            '34' => '#0000ff',
            '35' => '#ff00ff',
            '36' => '#00ffff',
            '37' => '#ffffff',
            '40' => '#000000',
            '41' => '#ff0000',
            '42' => '#00ff00',
            '43' => '#ffff00',
            '44' => '#0000ff',
            '45' => '#ff00ff',
            '46' => '#00ffff',
            '47' => '#ffffff',
        ];

        return $colors[$color] ?? '#000000';
    }

    public static function addRandomQueryParams(string $url): string
    {
        $generateRandomString = function (int $length): string {
            $characters = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
            $charactersLength = strlen($characters);
            $randomString = '';

            for ($i = 0; $i < $length; $i++) {
                $randomString .= $characters[rand(0, $charactersLength - 1)];
            }

            return $randomString;
        };

        // parse original URL
        $parsedUrl = parse_url($url);
        $queryParams = [];
        if (isset($parsedUrl['query'])) {
            parse_str($parsedUrl['query'], $queryParams);
        }

        // add random params
        $randomParamCount = rand(1, 3);
        for ($i = 0; $i < $randomParamCount; $i++) {
            $key = $generateRandomString(rand(1, 4));
            $value = $generateRandomString(rand(1, 4));
            $queryParams[$key] = $value;
        }

        // get final url
        $newQuery = http_build_query($queryParams);
        if (isset($parsedUrl['scheme']) && isset($parsedUrl['host'])) {
            $baseUrl = $parsedUrl['scheme'] . '://' . $parsedUrl['host'] . ($parsedUrl['path'] ?? '');
        } else {
            $baseUrl = $parsedUrl['path'] ?? '/';
        }
        return $baseUrl . '?' . $newQuery;
    }

    public static function truncateInTwoThirds(string $text, int $maxLength, string $placeholder = '...'): string
    {
        if (mb_strlen($text) <= $maxLength) {
            return $text;
        }

        $firstPartLength = ceil($maxLength * (2 / 3));
        $secondPartLength = $maxLength - $firstPartLength - mb_strlen($placeholder);

        $firstPart = mb_substr($text, 0, intval($firstPartLength));
        $secondPart = mb_substr($text, -1 * intval($secondPartLength));

        return $firstPart . $placeholder . $secondPart;
    }

    public static function truncateUrl(string $url, int $maxLength, string $placeholder = '...', ?string $stripHostname = null): string
    {
        if ($stripHostname) {
            $url = str_ireplace(['http://' . $stripHostname, 'https://' . $stripHostname], ['', ''], $url);
        }

        if (mb_strlen($url) > $maxLength) {
            $url = self::truncateInTwoThirds($url, $maxLength, $placeholder);
        }

        return $url;
    }

    public static function getProgressBar(int $done, int $total, int $segments = 20): string
    {
        $percentage = ($done / $total) * 100;
        $filledSegments = round(($done / $total) * $segments);
        $progressBar = str_repeat('>', intval($filledSegments)) . str_repeat(' ', intval($segments - $filledSegments));
        return sprintf("%s|%s|", str_pad(intval($percentage) . '%', 5), $progressBar);
    }

    public static function removeAnsiColors(string $text): string
    {
        return preg_replace('/\033\[\d+(;\d+)*m|\e\[\d+(;\d+)*m/', '', $text);
    }

    public static function getHttpClientCodeWithErrorDescription(int $httpCode, bool $shortVersion = false): string
    {
        static $errors = [
            -1 => ['short' => '-1:CON', 'long' => '-1:CONN-FAIL'],
            -2 => ['short' => '-2:TIM', 'long' => '-2:TIMEOUT'],
            -3 => ['short' => '-3:RST', 'long' => '-3:SRV-RESET'],
            -4 => ['short' => '-4:SND', 'long' => '-4:SEND-ERROR']
        ];

        if (isset($errors[$httpCode])) {
            return $shortVersion ? $errors[$httpCode]['short'] : $errors[$httpCode]['long'];
        }

        return strval($httpCode);
    }

    public static function getConsoleWidth(): int
    {
        static $width = null;
        if ($width) {
            return $width;
        }
        if (stripos(PHP_OS, 'WIN') === 0) {
            $output = [];
            exec('mode con', $output);
            foreach ($output as $line) {
                if (stripos($line, 'CON') !== false) {
                    $parts = preg_split('/\s+/', $line);
                    $width = intval($parts[1]);
                    break;
                }
            }
        } else {
            $width = intval(shell_exec('tput cols'));
        }

        $width = max($width, 100);
        return $width;
    }

    /**
     * Get URL without a scheme and host. If $onlyWhenHost is defined and URL does not contain this host, return original URL.
     *
     * @param string $url
     * @param string|null $onlyWhenHost
     * @return string
     */
    public static function getUrlWithoutSchemeAndHost(string $url, ?string $onlyWhenHost = null): string
    {
        if ($onlyWhenHost && stripos($url, $onlyWhenHost) === false) {
            return $url;
        }

        $parsedUrl = parse_url($url);
        return $parsedUrl['path'] ?? '/';
    }

    public static function getSafeCommand(string $command): string
    {
        return preg_replace(
            ['/(pass[a-z]{0,5})=\S+/i', '/(keys?)=\S+/i', '/(secrets?)=\S+/i'],
            ['$1=***', '$1=***', '$1=***'],
            $command
        );
    }

    public static function getColoredRequestTime(float $requestTime, int $strPadTo = 6): string
    {
        $result = str_pad(self::getFormattedDuration($requestTime), $strPadTo);
        if ($requestTime >= 2) {
            $result = Utils::getColorText($result, 'red', true);
        } else if ($requestTime >= 1) {
            $result = Utils::getColorText($result, 'magenta', true);
        } else if ($requestTime >= 0.5) {
            $result = Utils::getColorText($result, 'yellow');
        } else {
            $result = Utils::getColorText($result, 'green');
        }
        return $result;
    }

    public static function getColoredStatusCode(int $statusCode, int $strPadTo = 6): string
    {
        if ($statusCode >= 200 && $statusCode < 300) {
            return Utils::getColorText(str_pad(strval($statusCode), $strPadTo), 'green');
        } else if ($statusCode >= 300 && $statusCode < 400) {
            return Utils::getColorText(str_pad(strval($statusCode), $strPadTo), 'yellow', true);
        } else if ($statusCode >= 400 && $statusCode < 500) {
            return Utils::getColorText(str_pad(strval($statusCode), $strPadTo), 'magenta', true);
        } else if ($statusCode >= 500 && $statusCode < 600) {
            return Utils::getColorText(str_pad(strval($statusCode), $strPadTo), 'red', true);
        } else {
            return Utils::getColorText(str_pad(Utils::getHttpClientCodeWithErrorDescription($statusCode, true), $strPadTo), 'red', true);
        }
    }

    public static function getColoredSeverity(string $severity): string
    {
        if ($severity === 'critical') {
            return Utils::getColorText($severity, 'red', true);
        } else if ($severity === 'warning') {
            return Utils::getColorText($severity, 'magenta', true);
        } else if ($severity === 'notice') {
            return Utils::getColorText($severity, 'blue');
        } else {
            return Utils::getColorText($severity, 'green');
        }
    }

    public static function getColoredCriticals(int $criticals, int $strPadTo = 6): string
    {
        if ($criticals === 0) {
            return strval($criticals);
        }

        return Utils::getColorText(str_pad(strval($criticals), $strPadTo), 'red', true);
    }

    public static function getColoredWarnings(int $warnings, int $strPadTo = 6): string
    {
        if ($warnings === 0) {
            return strval($warnings);
        }

        return Utils::getColorText(str_pad(strval($warnings), $strPadTo), 'magenta');
    }

    public static function getColoredNotices(int $notices, int $strPadTo = 6): string
    {
        if ($notices === 0) {
            return strval($notices);
        }

        return Utils::getColorText(str_pad(strval($notices), $strPadTo), 'blue');
    }

    public static function getContentTypeNameById(int $contentTypeId): string
    {
        static $typeToName = [
            Crawler::CONTENT_TYPE_ID_HTML => 'HTML',
            Crawler::CONTENT_TYPE_ID_SCRIPT => 'JS',
            Crawler::CONTENT_TYPE_ID_STYLESHEET => 'CSS',
            Crawler::CONTENT_TYPE_ID_IMAGE => 'Image',
            Crawler::CONTENT_TYPE_ID_AUDIO => 'Audio',
            Crawler::CONTENT_TYPE_ID_VIDEO => 'Video',
            Crawler::CONTENT_TYPE_ID_FONT => 'Font',
            Crawler::CONTENT_TYPE_ID_DOCUMENT => 'Document',
            Crawler::CONTENT_TYPE_ID_JSON => 'JSON',
            Crawler::CONTENT_TYPE_ID_XML => 'XML',
            Crawler::CONTENT_TYPE_ID_REDIRECT => 'Redirect',
            Crawler::CONTENT_TYPE_ID_OTHER => 'Other',
        ];

        return $typeToName[$contentTypeId] ?? 'Unknown';
    }

    /**
     * Apply specific changes to HTML related to the crawler options
     *
     * @param string $html
     * @param string $baseUrl
     * @param bool $removeExternalJs
     * @param bool $removeCrossOrigins
     * @param bool $removeAnalytics
     * @param bool $removeSocnets
     * @param bool $removeCookiesRelated
     * @return string
     */
    public static function applySpecificHtmlChanges(string $html, string $baseUrl, bool $removeExternalJs, bool $removeCrossOrigins, bool $removeAnalytics, bool $removeSocnets, bool $removeCookiesRelated): string
    {
        $orig = $html;
        if (trim($html) === '') {
            return $html;
        }

        $baseHost = parse_url($baseUrl, PHP_URL_HOST);

        if ($removeExternalJs) {
            $html = preg_replace_callback('/<script[^>]*src=["\']?(.*?)["\']?[^>]*>.*?<\/script>/is', function ($matches) use ($baseHost) {
                if (preg_match("/^(https?:)?\/\//i", $matches[1]) === 1 && parse_url($matches[1], PHP_URL_HOST) !== $baseHost) {
                    return '';
                }
                return $matches[0];
            }, $html);
        }

        if ($removeCrossOrigins) {
            $html = preg_replace('/(<link[^>]+)\s*crossorigin(\s*=\s*["\']?.*?["\']?)?(\s*[^>]*>)/i', '$1$3', $html);
            $html = preg_replace('/(<script[^>]+)\s*crossorigin(\s*=\s*["\']?.*?["\']?)?(\s*[^>]*>)/i', '$1$3', $html);
        }

        if ($removeAnalytics || $removeSocnets) {
            $patterns = [];

            if ($removeAnalytics) {
                $patternsAnalytics = [
                    'googletagmanager.com',
                    'google-analytics.com',
                    'ga.js',
                    'gtag.js',
                    'gtag(',
                    'analytics.',
                    'connect.facebook.net',
                    'fbq(', // Facebook Pixel
                ];
                $patterns = array_merge($patterns, $patternsAnalytics);
            }

            if ($removeSocnets) {
                $patternsSocnets = [
                    'connect.facebook.net',
                    'connect.facebook.com',
                    'twitter.com',
                    '.x.com',
                    'linkedin.com',
                    'instagram.com',
                    'pinterest.com',
                    'tumblr.com',
                    'plus.google.com',
                    'curator.io',
                ];
                $patterns = array_merge($patterns, $patternsSocnets);
            }

            if ($removeCookiesRelated) {
                $patternsCookies = [
                    'cookies',
                    'cookiebot',
                ];
                $patterns = array_merge($patterns, $patternsCookies);
            }

            $patterns = array_unique($patterns);

            $html = preg_replace_callback('/<script[^>]*>(.*?)<\/script>/is', function ($matches) use ($patterns) {
                if ($matches[0]) {
                    foreach ($patterns as $keyword) {
                        if (stripos($matches[0], $keyword) !== false) {
                            return '';
                        }
                    }
                }
                return $matches[0];
            }, $html);

            if ($removeSocnets && $html) {
                $html = preg_replace('/<iframe[^>]*(facebook\.com|twitter\.com|linkedin\.com)[^>]*>.*?<\/iframe>/is', '', $html);
            }
        }

        return $html ?: '';
    }

    /**
     * Check HTML and get all found errors
     * @param string $html
     * @return string[]
     */
    public static function getHtmlErrors(string $html): array
    {
        libxml_use_internal_errors(true);
        $document = new \DOMDocument();
        @$document->loadHTML(mb_convert_encoding($html, 'HTML-ENTITIES', 'UTF-8'));
        $errors = libxml_get_errors();
        $errorMessages = [];
        foreach ($errors as $error) {
            $errorMessages[] = $error->message;
        }
        libxml_clear_errors();
        libxml_use_internal_errors(false);
        return $errorMessages;
    }

    /**
     * Is this href a valid URL for "requestable" resource through HTTP(S) request?
     * Non-requestable resources starts with "xyz:" (e.g. data:, javascript:, mailto:, tel:, ftp:, file:) but is not http:// or https://
     *
     * @param string $href
     * @return bool
     */
    public static function isHrefForRequestableResource(string $href): bool
    {
        if (str_starts_with($href, '#')) {
            // ignore anchors
            return false;
        } else if (str_contains($href, '{')) {
            // "{" is used by some frameworks (e.g. Angular) for dynamic URLs
            return false;
        } else if (str_contains($href, '<')) {
            // "<" is quite often visible in HTML code, but it is not valid URL
            return false;
        } else if (str_contains($href, '&#')) {
            // "&#" is quite often visible in HTML code, but it is not valid URL
            return false;
        } else if (preg_match('/^[a-z0-9]+:/i', $href) === 1 && preg_match('/^https?:\//i', $href) === 0) {
            return false;
        }

        return true;
    }

    /**
     * Takes a base URL, and a target URL from href, and resolves them as a browser would for an anchor tag.
     * Examples of handled targetUrl are:
     *  - /about
     *  - /about?foo=bar
     *  - /about#contact
     *  - //example.com/about
     *  - ../about
     *  - ./about
     *  - https://example.com/about
     *
     * @author https://github.com/dldnh/rel2abs
     * @param string $baseUrl
     * @param string $targetUrl
     * @return string
     */
    public static function getAbsoluteUrlByBaseUrl(string $baseUrl, string $targetUrl): string
    {
        // init
        $base = parse_url($baseUrl);
        $rel = parse_url($targetUrl);

        if (!$rel) {
            return $targetUrl;
        }

        // init paths so we can blank the base path if we have a rel host
        if (array_key_exists("path", $rel)) {
            $relPath = $rel["path"];
        } else {
            $relPath = "";
        }
        if (array_key_exists("path", $base)) {
            $basePath = $base["path"];
        } else {
            $basePath = "";
        }

        // if rel has scheme, it has everything
        if (array_key_exists("scheme", $rel)) {
            return $targetUrl;
        }

        // else use base scheme
        if (array_key_exists("scheme", $base)) {
            $abs = $base["scheme"];
        } else {
            $abs = "";
        }

        if (strlen($abs) > 0) {
            $abs .= "://";
        }

        // if rel has host, it has everything, so blank the base path
        // else use base host and carry on
        if (array_key_exists("host", $rel)) {
            $abs .= $rel["host"];
            if (array_key_exists("port", $rel)) {
                $abs .= ":";
                $abs .= $rel["port"];
            }
            $basePath = "";
        } else if (array_key_exists("host", $base)) {
            $abs .= $base["host"];
            if (array_key_exists("port", $base)) {
                $abs .= ":";
                $abs .= $base["port"];
            }
        }

        // if rel starts with slash, that's it
        if (strlen($relPath) > 0 && $relPath[0] == "/") {
            return $abs . $relPath . (array_key_exists("query", $rel) ? "?" . $rel["query"] : "") . (array_key_exists("fragment", $rel) ? "#" . $rel["fragment"] : "");
        }

        // split the base path parts
        $parts = array();
        $absParts = explode("/", $basePath);
        foreach ($absParts as $part) {
            array_push($parts, $part);
        }

        // remove the first empty part
        while (count($parts) >= 1 && strlen($parts[0]) == 0) {
            array_shift($parts);
        }

        // split the rel base parts
        $relParts = explode("/", $relPath);

        // @phpstan-ignore-next-line
        if (count($relParts) > 0 && strlen($relParts[0]) > 0) {
            array_pop($parts);
        }

        // iterate over rel parts and do the math
        $addSlash = false;
        foreach ($relParts as $part) {
            if ($part == "") {
            } else if ($part == ".") {
                $addSlash = true;
            } else if ($part == "..") {
                array_pop($parts);
                $addSlash = true;
            } else {
                array_push($parts, $part);
                $addSlash = false;
            }
        }

        // combine the result
        foreach ($parts as $part) {
            $abs .= "/";
            $abs .= $part;
        }

        if ($addSlash) {
            $abs .= "/";
        }

        if (array_key_exists("query", $rel)) {
            $abs .= "?";
            $abs .= $rel["query"];
        }

        if (array_key_exists("fragment", $rel)) {
            $abs .= "#";
            $abs .= $rel["fragment"];
        }

        return $abs;
    }

    /**
     * Strip all JavaScript and related code from HTML
     *
     * @param string $html
     * @return string
     */
    public static function stripJavaScript(string $html): string
    {
        $orig = $html;
        // script tags
        $scriptPattern = '/<script[^>]*>(.*)<\/script>/is';
        $html = preg_replace($scriptPattern, '', $html);

        // link tags by "href"
        $linkPatternHref = '/<link[^>]*href=["\'][^"\']+\.js[^"\']*["\'][^>]*>/is';
        $html = preg_replace($linkPatternHref, '', $html);

        // link tags by "as"
        $linkPatternAs = '/<link[^>]*as=["\']script["\'][^>]*>/is';
        $html = preg_replace($linkPatternAs, '', $html);

        // on* attributes
        $onEventPattern = '/\s+on[a-z]+=("[^"]*"|\'[^\']*\'|[^\s>]*)/is';
        $html = preg_replace($onEventPattern, '', $html);

        return $html;
    }

    /**
     * Strip all styles and related code from HTML
     *
     * @param string $html
     * @return string
     */
    public static function stripStyles(string $html): string
    {
        $styleTagPattern = '/<style\b[^>]*>(.*?)<\/style>/is';
        $html = preg_replace($styleTagPattern, '', $html);

        $linkTagPattern = '/<link\b[^>]*rel=["\']stylesheet["\'][^>]*>/is';
        $html = preg_replace($linkTagPattern, '', $html);

        $styleAttrPattern = '/\s+style=("[^"]*"|\'[^\']*\'|[^\s>]*)/is';
        $html = preg_replace($styleAttrPattern, ' ', $html);

        return $html;
    }

    /**
     * Strip all fonts and related code from HTML
     *
     * @param string $htmlOrCss
     * @return string
     */
    public static function stripFonts(string $htmlOrCss): string
    {
        $fontLinkPattern = '/<link\b[^>]*href=["\'][^"\']+\.(eot|ttf|woff2|woff|otf)[^"\']*["\'][^>]*>/is';
        $htmlOrCss = preg_replace($fontLinkPattern, '', $htmlOrCss);

        $fontFacePattern = '/@font-face\s*{[^}]*}\s*/is';
        $htmlOrCss = preg_replace($fontFacePattern, '', $htmlOrCss);

        $fontStylePattern = '/\b(font|font-family)\s*:[^;]+;/i';
        $htmlOrCss = preg_replace($fontStylePattern, '', $htmlOrCss);

        $emptyStyleAttrPattern = '/\s*style=["\']\s*["\']/i';
        $htmlOrCss = preg_replace($emptyStyleAttrPattern, '', $htmlOrCss);

        return $htmlOrCss;
    }

    /**
     * @param string $htmlOrCss
     * @return string
     */
    public static function stripImages(string $htmlOrCss): string
    {
        $placeholderImage = '/placeholder.png';
        $patterns = [
            '/(<img[^>]+)src=[\'"][^\'"]*[\'"]([^>]*>)/im',
            '/(<img[^>]+)srcset=[\'"][^\'"]*[\'"]([^>]*>)/im',
            '/(<source[^>]+)srcset=[\'"][^\'"]*[\'"]([^>]*>)/im',
            '/(<source[^>]+)src=[\'"][^\'"]*[\'"]([^>]*>)/im',
            '/url\(\s*[\'"]?(?![data:])([^\'")]*\.(?:png|jpe?g|gif|webp|svg|bmp))[\'"]?\s*\)/i',
        ];
        $replacements = [
            '$1src="' . $placeholderImage . '"$2',
            '$1srcset="' . $placeholderImage . '"$2',
            '$1srcset="' . $placeholderImage . '"$2',
            '$1src="' . $placeholderImage . '"$2',
            'url("' . $placeholderImage . '")'
        ];

        foreach ($patterns as $index => $pattern) {
            $htmlOrCss = preg_replace($pattern, $replacements[$index], $htmlOrCss);
        }

        $htmlOrCss = preg_replace_callback('/<picture[^>]*>.*?<\/picture>/is', function ($matches) use ($patterns, $replacements) {
            $pictureContent = preg_replace($patterns, $replacements, $matches[0]);
            return $pictureContent;
        }, $htmlOrCss);

        return $htmlOrCss;
    }

    /**
     * Recursively add redirect folderName.html files for all subfolders which contains index.html
     *
     * @param string $dir
     * @param array $changes
     * @return void
     */
    public static function addRedirectHtmlToSubfolders(string $dir, array &$changes): void
    {
        if (!is_dir($dir)) {
            echo "Directory '$dir' does not exist\n";
            return;
        }

        if ($handle = opendir($dir)) {
            while (false !== ($entry = readdir($handle))) {
                if ($entry == "." || $entry == "..") {
                    continue;
                }

                $path = "{$dir}/{$entry}";
                if (is_dir($path) && !is_file($path . ".html") && is_file("{$path}/index.html")) {
                    $htmlContent = '<!DOCTYPE html><meta http-equiv="refresh" content="0;url=' . $entry . '/index.html">';
                    $filePath = "{$dir}/{$entry}.html";
                    file_put_contents($filePath, $htmlContent);
                    $changes[] = "Added redirect file '$filePath'";
                }

                if (is_dir($path)) {
                    Utils::addRedirectHtmlToSubfolders($path, $changes);
                }
            }
            closedir($handle);
        }
    }

    /**
     * Parse and get all phone numbers from HTML
     *
     * If $onlyNonClickable = TRUE, filter only phone numbers that are not wrapped within <a href="tel: ...">
     *
     * @param string $html
     * @param bool $onlyNonClickable
     * @return string[]
     */
    public static function parsePhoneNumbersFromHtml(string $html, bool $onlyNonClickable = false): array
    {
        $phoneNumbers = [];

        // strip all JavaScript and styles - typically phone numbers are not visible in these parts of HTML
        $html = Utils::stripJavaScript($html);
        $html = Utils::stripStyles($html);

        // replace &nbsp; with space - phone numbers are typically separated by non-breaking spaces
        $html = str_replace('&nbsp;', ' ', $html);

        // formats with country codes and spaces, e.g.: +420 123 456 789 or +1234 1234567890
        $formatWithSpaces = '/\+\d{1,4}(\s?[0-9\- ]{1,5}){1,5}/s';
        preg_match_all($formatWithSpaces, $html, $matchesWithSpaces);
        $phoneNumbers = array_merge($phoneNumbers, $matchesWithSpaces[0]);

        // formats with country codes without spaces, e.g.: +420123456789
        $formatWithoutSpaces = '/\+[0-9\- ]{7,20}/s';
        preg_match_all($formatWithoutSpaces, $html, $matchesWithoutSpaces);
        $phoneNumbers = array_merge($phoneNumbers, $matchesWithoutSpaces[0]);

        // US format with parentheses, e.g.: (123) 456-7890
        $formatWithBrackets = '/\(\d{1,5}\)\s?\d{3,4}-\d{4}/';
        preg_match_all($formatWithBrackets, $html, $matchesWithBrackets);
        $phoneNumbers = array_merge($phoneNumbers, $matchesWithBrackets[0]);

        // regular format with dashes, e.g.: 123-456-7890
        $formatWithDashes = '/\d{1,5}-\d{3,4}-\d{4}/';
        preg_match_all($formatWithDashes, $html, $matchesWithDashes);
        $phoneNumbers = array_merge($phoneNumbers, $matchesWithDashes[0]);

        // trim spaces from all found numbers
        $phoneNumbers = array_map(function ($number) {
            return trim($number);
        }, $phoneNumbers);

        // filters out matches that are wrapped within <a href="tel: ...">
        if ($onlyNonClickable) {
            $phoneNumbers = array_filter($phoneNumbers, function ($number) use ($html) {
                $telPattern1 = '/<a[^>]*href=["\']tel:' . preg_quote($number, '/') . '["\'][^>]*>.*?<\/a>/';
                $telPattern2 = '/<a[^>]*href=["\']tel:[^"\'>]+["\'][^>]*>.*' . preg_quote($number, '/') . '.*<\/a>/s';
                return !preg_match($telPattern1, $html) && !preg_match($telPattern2, $html);
            });
        }

        // phone number must contain at least 8 digits
        $phoneNumbers = array_filter($phoneNumbers, function ($number) {
            return strlen($number) >= 8;
        });

        return array_values(array_unique($phoneNumbers));
    }

    /**
     * Parse HTML and remove all unwanted attributes from all HTML tags (except for $allowedAttrs)
     *
     * @param string $html
     * @param string[] $allowedAttrs
     * @param string $replaceTo
     * @return string
     */
    public static function removeUnwantedHtmlAttributes(string $html, array $allowedAttrs, string $replaceTo = ' *** '): string
    {
        if (!str_contains($html, '<')) {
            return $html;
        }

        $regex = '/<([a-z][a-z0-9]*)\s+([^>]*)>/i';
        $callback = function ($matches) use ($allowedAttrs, $replaceTo) {
            $tagName = $matches[1];
            $attrsString = $matches[2];

            preg_match_all('/([a-z][-a-z0-9_]*)\s*=\s*("|\')(.*?)\2/si', $attrsString, $attrMatches, PREG_SET_ORDER);
            $allowedAttributes = '';
            $attributesRemoved = false;

            foreach ($attrMatches as $attr) {
                if (in_array($attr[1], $allowedAttrs)) {
                    $allowedAttributes .= $attr[0] . ' ';
                } else {
                    $attributesRemoved = true;
                }
            }

            $result = "<{$tagName} " . rtrim($allowedAttributes) . ($attributesRemoved ? $replaceTo : '') . '>';
            return str_replace(' >', '>', $result);
        };

        return preg_replace_callback($regex, $callback, $html);
    }

    public static function removeWhitespacesFromHtml(string $html): string
    {
        $regexSkip = '/<(script|style)\b[^>]*>.*?<\/\1>/isx';

        $callback = function ($matches) {
            return preg_replace('/>\s+</', '> <', $matches[0]);
        };

        $html = preg_replace_callback($regexSkip, $callback, $html);

        $html = preg_replace('/\s+/', ' ', $html);
        $html = preg_replace('/>\s+</', '> <', $html);

        return $html;
    }

    /**
     * Remove all invalid/unsafe tags from SVC
     * @param string $svg
     * @return string
     */
    public static function sanitizeSvg(string $svg): string
    {
        return strip_tags($svg, [
            'animate',
            'animateColor',
            'animateMotion',
            'animateTransform',
            'circle',
            'clipPath',
            'defs',
            'desc',
            'filter',
            'foreignObject',
            'g',
            'image',
            'line',
            'linearGradient',
            'marker',
            'mask',
            'metadata',
            'mpath',
            'path',
            'pattern',
            'polygon',
            'polyline',
            'radialGradient',
            'rect',
            'set',
            'stop',
            'style',
            'svg',
            'switch',
            'symbol',
            'text',
            'title',
            'tspan',
            'use',
            'view',
        ]);
    }

}