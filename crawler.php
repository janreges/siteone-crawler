<?php

use Swoole\Coroutine\Http\Client;
use Swoole\Coroutine;

const VERSION = '2023.10.1';
$startTime = microtime(true);
$options = CrawlerOptions::parse($argv);

echo "========================\n";
echo "= Fast website crawler =\n";
echo "= Version: " . VERSION . "   =\n";
echo "= jan.reges@siteone.cz =\n";
echo "========================\n\n";
echo "Used options: " . getColorText(json_encode($options, JSON_PRETTY_PRINT), 'gray') . "\n";

$parsedUrl = parse_url($options->url);
if (!isset($parsedUrl['scheme']) || !isset($parsedUrl['host'])) {
    echo "Invalid URL provided ({$options->url})\n";
    exit(1);
}

$userAgents = [
    'desktop' => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/' . date('y') . '0.0.0.0 Safari/537.36',
    'mobile' => 'Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15A5370a Safari/604.1',
    'tablet' => 'Mozilla/5.0 (Linux; Android 11; SAMSUNG SM-T875) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/14.0 Chrome/87.0.4280.141 Safari/537.36',
];

$userAgent = $options->userAgent ?: ($userAgents[$options->device] ?? $userAgents['desktop']);
echo "Used User-Agent: " . getColorText($userAgent, 'gray') . "\n\n";

$domain = $parsedUrl['host'];
$scheme = $parsedUrl['scheme'];

$workers = new \Swoole\Table(1);
$workers->column('workers', \Swoole\Table::TYPE_INT, 2);
$workers->create();
$workers->set('1', ['workers' => 0]);

$queue = new \Swoole\Table(5000);
$queue->column('url', \Swoole\Table::TYPE_STRING, 1000);
$queue->create();
$queue->set(md5($options->url), ['url' => $options->url]);

$visited = new \Swoole\Table(10000);
$visited->column('url', \Swoole\Table::TYPE_STRING, 1000);
$visited->column('time', \Swoole\Table::TYPE_FLOAT, 8);
$visited->column('status', \Swoole\Table::TYPE_INT, 8);
$visited->column('size', \Swoole\Table::TYPE_INT, 8);
$visited->create();

$maxWorkers = $options->maxWorkers;

$header = str_pad("URL", $options->tableUrlColumnSize) . "|" . " Status " . "|" . " Time  " . "|" . " Size     ";
foreach ($options->headersToTable as $headerName) {
    $header .= " | {$headerName}";
}
$header .= "\n";
echo $header . str_repeat("-", strlen($header)) . "\n";

Coroutine\run(function () use ($queue, $visited, $workers) {
    global $maxWorkers;
    while ($workers->get('1', 'workers') < $maxWorkers && $queue->count() > 0) {
        Coroutine::create('processNextUrl', $queue, $visited, $workers);
    }
});

function processNextUrl(\Swoole\Table $queue, \Swoole\Table $visited, \Swoole\Table $workers): void
{
    global $domain, $scheme, $userAgent, $options;

    if ($queue->count() === 0) {
        echo "Queue empty\n";
        return;
    }

    $nextUrl = null;
    foreach ($queue as $key => $url) {
        $nextUrl = $url['url'];
        $queue->del($key);
        $visited->set($key, ['url' => $nextUrl]);
        break;
    }

    if (!$nextUrl) {
        echo "Queue empty - no url\n";
        return;
    }

    $url = $nextUrl;
    $workers->incr('1', 'workers');

    $parsedUrl = parse_url($url);
    $client = new Client($parsedUrl['host'], $scheme === 'https' ? 443 : 80, $scheme === 'https');
    $start = microtime(true);

    $urlToGet = $options->addRandomQueryParams ? addRandomQueryParams($parsedUrl['path'] ?? '/') : $parsedUrl['path'] ?? '/';

    $client->setHeaders(['User-Agent' => $userAgent]);
    $client->setHeaders(['Accept-Encoding' => $options->acceptEncoding]);
    $client->set(['timeout' => $options->timeout]);
    $client->get($urlToGet);
    $body = $client->body;
    $status = $client->statusCode;
    $extraHeadersContent = '';
    foreach ($options->headersToTable as $headerName) {
        if (isset($client->headers[strtolower($headerName)])) {
            $extraHeadersContent .= (' | ' . ($client->headers[strtolower($headerName)] ?? 'N/A'));
        }
    }
    $workers->decr('1', 'workers');

    preg_match_all('/<a\s+.*?href="([^"]+)"[^>]*>/i', $body, $matches);
    foreach ($matches[1] as $match) {
        $match = trim($match);
        $parsedMatch = parse_url($match);

        $validForParsing = !isset($parsedMatch['host']) || $parsedMatch['host'] === $domain;
        if (preg_match('/(mailto|phone|tel|javascript):/', $match) === 1) {
            $validForParsing = false;
        }

        if ($validForParsing) {
            $nextUrl = $match;

            if (isset($parsedMatch['host']) && !isset($parsedMatch['scheme'])) {
                $nextUrl = "$scheme://$nextUrl";
            } elseif (!isset($parsedMatch['host'])) {
                $nextUrl = "$scheme://$domain$nextUrl";
            }

            $nextUrl = preg_replace('/#.*$/', '', $nextUrl);
            if ($options->removeQueryParams) {
                $nextUrl = preg_replace('/\?.*$/', '', $nextUrl);
            }

            if (!$visited->exist(md5($nextUrl)) && !$queue->exist(md5($nextUrl)) && @parse_url($nextUrl) !== false && (preg_match('/\.[a-z0-9]{2,4}$/i', $nextUrl) === 0 || stripos($nextUrl, '.html') !== false)) {
                $queue->set(md5($nextUrl), ['url' => $nextUrl]);
            }
        }
    }

    $elapsedTime = microtime(true) - $start;

    // update stats for visited row
    $visitedRow = $visited->get(md5($url));
    $visitedRow['time'] = $elapsedTime;
    $visitedRow['status'] = $status;
    $visitedRow['size'] = $body ? strlen($body) : 0;
    $visited->set(md5($url), $visitedRow);

    $coloredStatus = $status;
    if ($status == 200) {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'green');
    } else if ($status > 300 && $status < 400) {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'yellow', true);
    } elseif ($status == 404) {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'magenta', true);
    } elseif ($status == 429) {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'red', true);
    } elseif ($status > 400 && $status < 500) {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'cyan', true);
    } else {
        $coloredStatus = getColorText(str_pad($status, 6, ' '), 'red', true);
    }

    $coloredElapsedTime = sprintf(" %.3f ", $elapsedTime);
    if ($coloredElapsedTime >= 2) {
        $coloredElapsedTime = getColorText($coloredElapsedTime, 'red', true);
    } else if ($coloredElapsedTime >= 1) {
        $coloredElapsedTime = getColorText($coloredElapsedTime, 'magenta', true);
    }

    $bodySize = $body ? strlen($body) : 0;
    $coloredSize = sprintf(
        " %s ", $bodySize > 1024 * 1024
        ? getColorText(str_pad(getFormattedSize($bodySize), 8), 'red')
        : str_pad(getFormattedSize($bodySize), 8)
    );

    $urlForTable = $options->showUrlsWithoutDomain ? (preg_replace('/^https?:\/\/[^\/]+\//i', '/', $url)) : $url;
    echo str_pad($urlForTable . ($options->addRandomQueryParams ? getColorText('+%random-query%', 'gray') : ''), $options->tableUrlColumnSize), "|", sprintf(" %s ", $coloredStatus), "|", $coloredElapsedTime, "|", $coloredSize, $extraHeadersContent, "\n";

    if ($queue->count() === 0 && $workers->get('1', 'workers') === 0) {
        displayTotalStats($visited);
        Coroutine::cancel(Coroutine::getCid());
    } else {
        while ($workers->get('1', 'workers') < $options->maxWorkers && $queue->count() > 0) {
            Coroutine::create('processNextUrl', $queue, $visited, $workers);
        }
    }
}

function displayTotalStats(\Swoole\Table $visited): void
{
    global $startTime;

    $info = [
        'totalUrls' => $visited->count(),
        'totalSize' => 0,
        'countByStatus' => [],
        'totalTime' => 0,
        'minTime' => null,
        'maxTime' => null,
    ];

    foreach ($visited as $row) {
        $info['totalTime'] += $row['time'];
        $info['totalSize'] += $row['size'];
        $info['countByStatus'][$row['status']] = ($info['countByStatus'][$row['status']] ?? 0) + 1;
        $info['minTime'] = $info['minTime'] === null ? $row['time'] : min($row['time'], $info['minTime']);
        $info['maxTime'] = $info['maxTime'] === null ? $row['time'] : max($row['time'], $info['maxTime']);
    }

    echo "\n";
    $resultHeader = "Total execution time: " . getColorText(number_format(microtime(true) - $startTime, 2, '.', ' ') . " seconds", 'cyan');
    echo str_repeat('=', 80) . "\n";
    echo "{$resultHeader}\n";
    echo "Total processed URLs: " . getColorText($info['totalUrls'], 'cyan') . " with total size " . getColorText(getFormattedSize($info['totalSize']), 'cyan') . "\n";
    echo "Response times: "
        . " AVG " . getColorText(number_format($info['totalTime'] / $info['totalUrls'], 3, '.', ' ') . ' sec', 'magenta', true)
        . " MIN " . getColorText(number_format($info['minTime'], 3, '.', ' ') . ' sec', 'green', true)
        . " MAX " . getColorText(number_format($info['maxTime'], 3, '.', ' ') . ' sec', 'red', true)
        . " TOTAL " . getColorText(number_format($info['totalTime'], 3, '.', ' ') . ' sec', 'cyan', true) . "\n";
    echo "URLs by status:\n";
    ksort($info['countByStatus']);
    $statuses = '';
    foreach ($info['countByStatus'] as $status => $count) {
        $statuses .= " {$status}: $count\n";
    }
    echo getColorText(rtrim($statuses), 'yellow') . "\n";
    echo str_repeat('=', 80) . "\n";
}

function addRandomQueryParams(string $url): string
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

function getColorText(string $text, string $color, ?bool $setBackground = false): string
{
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

function getFormattedSize(int $bytes, int $precision = 1): string
{
    $units = array('B', 'kB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB');

    $bytes = max($bytes, 0);
    $pow = floor(($bytes ? log($bytes) : 0) / log(1024));
    $pow = min($pow, count($units) - 1);

    $bytes /= pow(1024, $pow);

    return round($bytes, $precision) . ' ' . $units[$pow];
}

class CrawlerOptions
{
    public string $url;
    public string $device = 'desktop';
    public int $maxWorkers = 3;
    public int $timeout = 10;
    public int $tableUrlColumnSize = 120;
    public string $acceptEncoding = 'gzip, deflate, br';
    public ?string $userAgent = null;
    public ?array $headersToTable = [];
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;
    public bool $showUrlsWithoutDomain = false;

    private static array $required = ["url"];

    public static function parse(array $argv): self
    {
        $result = new self();

        // Parsing input parameters
        foreach ($argv as $arg) {
            if (strpos($arg, '--url=') === 0) {
                $result->url = trim(substr($arg, 6), ' "\'');
            }
            if (strpos($arg, '--device=') === 0) {
                $result->device = trim(substr($arg, 9), ' "\'');
            }
            if (strpos($arg, '--max-workers=') === 0) {
                $result->maxWorkers = (int)substr($arg, 14);
            }
            if (strpos($arg, '--timeout=') === 0) {
                $result->timeout = (int)substr($arg, 10);
            }
            if (strpos($arg, '--table-url-column-size=') === 0) {
                $result->tableUrlColumnSize = (int)substr($arg, 24);
            }
            if (strpos($arg, '--accept-encoding=') === 0) {
                $result->acceptEncoding = trim(substr($arg, 18), ' "\'');
            }
            if (strpos($arg, '--user-agent=') === 0) {
                $result->userAgent = trim(substr($arg, 13), ' "\'');
            }
            if (strpos($arg, '--headers-to-table=') === 0) {
                $result->headersToTable = explode(',', str_replace(' ', '', trim(substr($arg, 19), ' "\'')));
            }
            if (strpos($arg, '--add-random-query-params') === 0) {
                $result->addRandomQueryParams = true;
            }
            if (strpos($arg, '--remove-query-params') === 0) {
                $result->removeQueryParams = true;
            }
            if (strpos($arg, '--show-urls-without-domain') === 0) {
                $result->showUrlsWithoutDomain = true;
            }
        }

        // Checking required parameters
        foreach (self::$required as $param) {
            if (!isset($result->$param)) {
                echo "Missing required parameter --$param\n";
                self::displayHelp();
                exit(1);
            }
        }

        return $result;
    }

    private static function displayHelp(): void
    {
        echo "Usage: ./swoole-cli crawler.php --url=https://mydomain.tld [optional parameters]\n";
        echo "--url=<url>                     Required. The URL address. Use quotation marks if the URL contains query parameters.\n";
        echo "--device=<device>               Optional. Device for choosing a predefined user-agent. Ignored when --user-agent is defined. Supported values: 'desktop', 'tablet', 'mobile'. Default is 'desktop'\n";
        echo "--max-workers=<n>               Optional. Maximum number of workers (threads). Use carefully. A high number of threads will cause a DoS attack. Default is 3.\n";
        echo "--timeout=<n>                   Optional. Timeout in seconds. Default is 10.\n";
        echo "--table-url-column-size=<value> Optional. Basic URL column width. Default is 120.\n";
        echo "--accept-encoding=<value>       Optional. Custom Accept-Encoding. Default is 'gzip, deflate, br'.\n";
        echo "--user-agent=<value>            Optional. Custom user agent. Use quotation marks. If specified, it takes precedence over the device parameter.\n";
        echo "--headers-to-table=<value>      Optional. Comma delimited list of HTTP response headers added to output table.\n";
        echo "--remove-query-params           Optional. Remove query parameters from found URLs.\n";
        echo "--add-random-query-params       Optional. Adds several random query parameters to each URL.\n";
        echo "--show-urls-without-domain      Optional. On output, show only the URL without protocol and domain..\n";
        echo "\n";
        echo "Version: " . VERSION . "\n";
        echo "Created with ♥ by Ján Regeš (jan.reges@siteone.cz) [10/2023]\n";
    }
}
