<?php

namespace Crawler;

class Utils
{

    public static function relativeToAbsoluteUrl(string $relativeUrl, string $baseUrl): ?string
    {
        if (substr($relativeUrl, 0, 1) === '/' || preg_match('/^https?:\/\//', $relativeUrl) === 1) {
            return $relativeUrl;
        }

        // handle href="./xyz" - it is equivalent to href="xyz"
        if (str_starts_with($relativeUrl, './')) {
            $relativeUrl = substr($relativeUrl, 2);
        }

        // remove query params and hash from base URL
        $baseUrl = preg_replace(['/\?.*$/', '/#.*$/'], ['', ''], $baseUrl);

        // remove file name from base URL and trim trailing slash
        $baseUrl = preg_match('/\.[a-z0-9]{2,10}$/i', $baseUrl) === 1 ? rtrim(preg_replace('/\/[^\/]+$/i', '', $baseUrl), ' /') : rtrim($baseUrl, ' /');

        // explode base URL and relative URL to segments
        $baseSegments = explode('/', trim($baseUrl, '/'));
        $relativeSegments = explode('/', $relativeUrl);

        foreach ($relativeSegments as $segment) {
            if ($segment === '..') {
                // Odebrání posledního segmentu z base URL, pokud se jedná o proteckování na úroveň výše
                array_pop($baseSegments);
            } else {
                $baseSegments[] = $segment;
            }
        }

        // build and validate final URL
        $finalUrl = implode('/', $baseSegments);
        if (!filter_var($finalUrl, FILTER_VALIDATE_URL)) {
            $finalUrl = null;
        }
        return $finalUrl;
    }

    public static function getFormattedSize(int $bytes, int $precision = 1): string
    {
        $units = array('B', 'kB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB');

        $bytes = max($bytes, 0);
        $pow = floor(($bytes ? log($bytes) : 0) / log(1024));
        $pow = min($pow, count($units) - 1);

        $bytes /= pow(1024, $pow);

        return round($bytes, $precision) . ' ' . $units[$pow];
    }

    public static function getColorText(string $text, string $color, ?bool $setBackground = false): string
    {
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
            'gray' => '1;30',
        ];

        $bgColors = [
            'black' => '40',
            'red' => '41',
            'green' => '42',
            'yellow' => '43',
            'blue' => '44',
            'magenta' => '45',
            'cyan' => '46',
            'white' => '47',
        ];

        if ($setBackground) {
            $coloredString = "\033[" . $bgColors[$color] . "m";
        } else {
            $coloredString = "\033[" . $colors[$color] . "m";
        }

        $coloredString .= $text . "\033[0m";

        return $coloredString;
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

        $firstPart = mb_substr($text, 0, $firstPartLength);
        $secondPart = mb_substr($text, -1 * $secondPartLength);

        return $firstPart . $placeholder . $secondPart;
    }
}