<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\ParsedUrl;
use Crawler\Utils;
use Crawler\Version;
use Exception;
use Swoole\Coroutine\Http\Client;

class UploadExporter extends BaseExporter implements Exporter
{
    const GROUP_UPLOAD = 'upload';

    protected bool $uploadEnabled = false;
    protected ?string $endpoint;
    protected ?string $retention;
    protected ?string $password;
    protected ?int $uploadTimeout;

    public function shouldBeActivated(): bool
    {
        return $this->uploadEnabled;
    }

    public function export(): void
    {
        $htmlReport = new HtmlReport($this->status);
        $htmlReportHtml = $htmlReport->getHtml();

        $start = microtime(true);
        try {
            $onlineHtmlUrl = $this->upload($htmlReportHtml);
            $this->status->addInfoToSummary('upload-done', "HTML report uploaded to '{$onlineHtmlUrl}' and took " . Utils::getFormattedDuration(microtime(true) - $start));
        } catch (Exception $e) {
            $this->status->addCriticalToSummary('upload-failed', "HTML report upload failed: {$e->getMessage()} ({$e->getCode()}) and took " . Utils::getFormattedDuration(microtime(true) - $start));
        }
    }

    /**
     * @param string $html
     * @return string
     * @throws Exception
     */
    private function upload(string $html): string
    {
        $compressedHtml = gzencode($html);
        $parsedUrl = ParsedUrl::parse($this->endpoint);

        // prepare post data
        $postData = [
            'htmlBody' => $compressedHtml,
            'version' => Version::CODE,
            'platform' => PHP_OS,
            'arch' => $this->getArch(),
        ];

        if ($this->retention) {
            $postData['retention'] = $this->retention;
        }
        if ($this->password !== null && trim($this->password) !== '') {
            $postData['password'] = $this->password;
        }

        // send request
        $client = new Client($parsedUrl->host, $parsedUrl->port, $parsedUrl->isHttps());
        $client->set(['timeout' => $this->uploadTimeout]);
        $client->setHeaders([
            'Content-Type' => 'application/x-www-form-urlencoded',
        ]);
        $client->post($parsedUrl->path, $postData);

        // handle response
        $resultUrl = null;
        $resultError = null;
        $responseJson = null;
        if (str_contains($client->headers['content-type'] ?? '', 'application/json')) {
            $responseJson = @json_decode($client->body, true);
            $resultError = $responseJson['error'] ?? null;
        }
        if (is_array($responseJson) && isset($responseJson['url'])) {
            $resultUrl = $responseJson['url'];
        }

        $client->close();

        if ($resultUrl) {
            return $resultUrl;
        } else {
            throw new Exception("Upload failed: " . ($resultError ?? 'unknown error'), (int)$client->statusCode);
        }
    }

    /**
     * @return string
     */
    private function getArch()
    {
        $is64bit = PHP_INT_SIZE === 8;

        if ($is64bit) {
            $systemInfo = php_uname();
            if (str_contains($systemInfo, 'x86_64')) {
                return 'x64';
            } elseif (str_contains($systemInfo, 'aarch64') || str_contains($systemInfo, 'arm64')) {
                return 'arm64';
            }
        }

        return 'unknown';
    }

    /**
     * @inheritDoc
     */
    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_UPLOAD,
            'Upload options', [
            new Option('--upload', '-up', 'uploadEnabled', Type::BOOL, false, 'Enable HTML report upload to `--upload-to`.', false, false),
            new Option('--upload-to', '-upt', 'endpoint', Type::URL, false, 'URL of the endpoint where to send the HTML report.', 'https://crawler.siteone.io/up', false),
            new Option('--upload-retention', '-upr', 'retention', Type::STRING, false, 'How long should the HTML report be kept in the online version? Values: 1h / 4h / 12h / 24h / 3d / 7d / 30d / 365d / forever', '30d', false),
            new Option('--upload-password', '-uppass', 'password', Type::STRING, false, "Optional password, which must be entered (the user will be 'crawler') to display the online HTML report.", null, true),
            new Option('--upload-timeout', '-upti', 'uploadTimeout', Type::INT, false, "Upload timeout in seconds.", 3600, false),
        ]));
        return $options;
    }
}