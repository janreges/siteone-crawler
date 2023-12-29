<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\DnsAnalysisResult;
use Crawler\Analysis\Result\SecurityCheckedHeader;
use Crawler\Analysis\Result\SecurityResult;
use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Result\VisitedUrl;
use Crawler\Utils;
use DOMDocument;
use Exception;

class SecurityAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_SECURITY = 'security';
    const ANALYSIS_HEADERS = 'Security headers';

    const HEADER_ACCESS_CONTROL_ALLOW_ORIGIN = 'access-control-allow-origin';
    const HEADER_STRICT_TRANSPORT_SECURITY = 'strict-transport-security';
    const HEADER_X_FRAME_OPTIONS = 'x-frame-options';
    const HEADER_X_XSS_PROTECTION = 'x-xss-protection';
    const HEADER_X_CONTENT_TYPE_OPTIONS = 'x-content-type-options';
    const HEADER_REFERRER_POLICY = 'referrer-policy';
    const HEADER_CONTENT_SECURITY_POLICY = 'content-security-policy';
    const HEADER_FEATURE_POLICY = 'feature-policy';
    const HEADER_PERMISSIONS_POLICY = 'permissions-policy';
    const HEADER_SERVER = 'server';
    const HEADER_X_POWERED_BY = 'x-powered-by';
    const HEADER_SET_COOKIE = 'set-cookie';

    private SecurityResult $result;

    private int $pagesWithCritical = 0;
    private int $pagesWithWarning = 0;
    private int $pagesWithNotice = 0;

    public function __construct()
    {
        $this->result = new SecurityResult();
    }

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        $consoleWidth = Utils::getConsoleWidth();
        $recommendationColWidth = $consoleWidth - 70; // 70 = width of other columns (with spaces and pipes)

        $superTable = new SuperTable(
            self::SUPER_TABLE_SECURITY,
            "Security",
            "Nothing to report.",
            [
                new SuperTableColumn('header', 'Header', 26, null, null, true),
                new SuperTableColumn('ok', 'OK', 5, function ($value) {
                    return $value > 0 ? Utils::getColorText(strval($value), 'green') : '0';
                }, null, false, false),
                new SuperTableColumn('notice', 'Notice', 6, function ($value) {
                    return $value > 0 ? Utils::getColorText(strval($value), 'blue') : '0';
                }, null, false, false),
                new SuperTableColumn('warning', 'Warning', 7, function ($value) {
                    return $value > 0 ? Utils::getColorText(strval($value), 'magenta', true) : '0';
                }, null, false, false),
                new SuperTableColumn('critical', 'Critical', 8, function ($value) {
                    return $value > 0 ? Utils::getColorText(strval($value), 'red', true) : '0';
                }, null, false, false),
                new SuperTableColumn('recommendation', 'Recommendation', $recommendationColWidth, function ($value, $renderInto) {
                    if ($value) {
                        return implode($renderInto === SuperTable::RENDER_INTO_HTML ? '<br>' : '.', $value);
                    } else {
                        return '';
                    }
                }, null, true, true, false, false),
            ], true, 'highestSeverity', 'DESC'
        );

        $data = [];
        foreach ($this->result->checkedHeaders as $header) {
            $data[] = [
                'header' => $header->getFormattedHeader(),
                'highestSeverity' => $header->highestSeverity,
                'ok' => $header->countPerSeverity[SecurityCheckedHeader::OK] ?? 0,
                'notice' => $header->countPerSeverity[SecurityCheckedHeader::NOTICE] ?? 0,
                'warning' => $header->countPerSeverity[SecurityCheckedHeader::WARNING] ?? 0,
                'critical' => $header->countPerSeverity[SecurityCheckedHeader::CRITICAL] ?? 0,
                'recommendation' => $header->recommendations,
            ];
        }

        $superTable->setData($data);
        $this->status->addSuperTableAtEnd($superTable);
        $this->output->addSuperTable($superTable);

        $this->setFindingsToSummary();
    }

    /**
     * Analyze HTML URLs for security
     *
     * @param VisitedUrl $visitedUrl
     * @param string|null $body
     * @param DOMDocument|null $dom
     * @param array|null $headers
     * @return UrlAnalysisResult|null
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?DOMDocument $dom, ?array $headers): ?UrlAnalysisResult
    {
        if (!$visitedUrl->isAllowedForCrawling || $visitedUrl->contentType !== Crawler::CONTENT_TYPE_ID_HTML || $visitedUrl->looksLikeStaticFileByUrl()) {
            return null;
        }

        $result = new UrlAnalysisResult();

        $this->checkHeaders($headers, $visitedUrl->isHttps(), $result);

        if ($body !== null && trim($body) !== '') {
            $this->checkHtmlSecurity($body, $visitedUrl->isHttps(), $result);
        }

        return $result;
    }

    private function setFindingsToSummary(): void
    {
        $this->pagesWithCritical = 0;
        $this->pagesWithWarning = 0;
        $this->pagesWithNotice = 0;

        foreach ($this->result->checkedHeaders as $header) {
            $this->pagesWithCritical += $header->countPerSeverity[SecurityCheckedHeader::CRITICAL] ?? 0;
            $this->pagesWithWarning += $header->countPerSeverity[SecurityCheckedHeader::WARNING] ?? 0;
            $this->pagesWithNotice += $header->countPerSeverity[SecurityCheckedHeader::NOTICE] ?? 0;
        }

        if ($this->pagesWithCritical) {
            $this->status->addCriticalToSummary('security', "Security - {$this->pagesWithCritical} pages(s) with critical finding(s).");
        } else if ($this->pagesWithWarning) {
            $this->status->addWarningToSummary('security', "Security - {$this->pagesWithWarning} pages(s) with warning(s).");
        } else if ($this->pagesWithNotice) {
            $this->status->addNoticeToSummary('security', "Security - {$this->pagesWithNotice} pages(s) with notice(s).");
        } else {
            $this->status->addOkToSummary('security', "Security - no findings.");
        }
    }

    private function checkHeaders(array $headers, bool $isHttps, UrlAnalysisResult $urlAnalysisResult): void
    {
        foreach (self::getCheckedHeaders() as $header) {
            switch ($header) {
                case self::HEADER_ACCESS_CONTROL_ALLOW_ORIGIN:
                    $this->checkAccessControlAllowOrigin($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_STRICT_TRANSPORT_SECURITY:
                    if ($isHttps) {
                        $this->checkStrictTransportSecurity($headers, $urlAnalysisResult);
                    }
                    break;
                case self::HEADER_X_FRAME_OPTIONS:
                    $this->checkXFrameOptions($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_X_XSS_PROTECTION:
                    $this->checkXXssProtection($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_X_CONTENT_TYPE_OPTIONS:
                    $this->checkXContentTypeOptions($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_REFERRER_POLICY:
                    $this->checkReferrerPolicy($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_CONTENT_SECURITY_POLICY:
                    $this->checkContentSecurityPolicy($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_FEATURE_POLICY:
                    $this->checkFeaturePolicy($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_PERMISSIONS_POLICY:
                    $this->checkPermissionsPolicy($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_SERVER:
                    $this->checkServer($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_X_POWERED_BY:
                    $this->checkXPoweredBy($headers, $urlAnalysisResult);
                    break;
                case self::HEADER_SET_COOKIE:
                    $this->checkSetCookie($headers, $isHttps, $urlAnalysisResult);
                    break;
            }
        }
    }

    private function checkHtmlSecurity(?string $html, bool $isHttps, UrlAnalysisResult $urlAnalysisResult): void
    {
        if ($html === null) {
            return;
        }

        if ($isHttps) {
            preg_match_all('/<form[^>]*action=["\']http:\/\/[^"\']+["\'][^>]*>/i', $html, $matches);
            foreach ($matches[0] ?? [] as $match) {
                $finding = 'Form actions that send data over non-secure HTTP detected in ' . $match;
                $urlAnalysisResult->addCritical($finding, self::ANALYSIS_HEADERS, [$finding]);
            }

            preg_match_all('/<iframe[^>]*src=["\']http:\/\/[^"\']+["\'][^>]*>/i', $html, $matches);
            foreach ($matches[0] ?? [] as $match) {
                $finding = 'Iframe with non-secure HTTP detected in ' . $match;
                $urlAnalysisResult->addCritical($finding, self::ANALYSIS_HEADERS, [$finding]);
            }
        }
    }

    private function checkAccessControlAllowOrigin(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_ACCESS_CONTROL_ALLOW_ORIGIN;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            return;
        } elseif ($value === '*') {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Access-Control-Allow-Origin is set to '*' which allows any origin to access the resource. This can be a security risk.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value !== 'same-origin' && $value !== 'none') {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "Access-Control-Allow-Origin is set to '{$value}' which allows this origin to access the resource.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::OK;
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkStrictTransportSecurity(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_STRICT_TRANSPORT_SECURITY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "Strict-Transport-Security header is not set. It enforces secure connections and protects against MITM attacks.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif (str_contains($value, 'max-age=0')) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "Strict-Transport-Security header is set to max-age=0 which disables HSTS. This can be a security risk.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif (preg_match('/max-age=([0-9]+)/i', $value, $matches) === 1) {
            if ($matches[1] < 31 * 24 * 60 * 60) {
                $severity = SecurityCheckedHeader::WARNING;
                $recommendation = "Strict-Transport-Security header is set to max-age={$matches[1]} which is less than 31 days. This can be a security risk.";
                $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
            } else {
                $severity = SecurityCheckedHeader::OK;
            }
        } else {
            $severity = SecurityCheckedHeader::OK;
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkXFrameOptions(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_X_FRAME_OPTIONS;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "X-Frame-Options header is not set. It prevents clickjacking attacks when set to 'deny' or 'sameorigin.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === 'DENY') {
            $severity = SecurityCheckedHeader::OK;
        } elseif ($value === 'SAMEORIGIN') {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "X-Frame-Options header is set to SAMEORIGIN which allows this origin to embed the resource in a frame.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === 'ALLOW-FROM') {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "X-Frame-Options header is set to ALLOW-FROM which allows this origin to embed the resource in a frame.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "X-Frame-Options header is set to '{$value}' which allows this origin to embed the resource in a frame. This can be a security risk.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkXXssProtection(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_X_XSS_PROTECTION;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "X-XSS-Protection header is not set. It enables browser's built-in defenses against XSS attacks.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === '0') {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "X-XSS-Protection header is set to 0 which disables XSS protection. This can be a security risk.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === '1') {
            $severity = SecurityCheckedHeader::OK;
            $urlAnalysisResult->addOk("X-XSS-Protection header is set to 1. It is secure.", self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === '1; mode=block' || $value === '1;mode=block') {
            $severity = SecurityCheckedHeader::OK;
        } else {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "X-XSS-Protection header is set to '{$value}'. This can be a security risk.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkXContentTypeOptions(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_X_CONTENT_TYPE_OPTIONS;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "X-Content-Type-Options header is not set. It stops MIME type sniffing and mitigates content type attacks.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === 'nosniff') {
            $severity = SecurityCheckedHeader::OK;
        } else {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "X-Content-Type-Options header is set to '{$value}'. This can be a security risk.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkReferrerPolicy(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_REFERRER_POLICY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        $okValues = [
            'no-referrer',
            'no-referrer-when-downgrade',
            'origin',
            'origin-when-cross-origin',
            'same-origin',
            'strict-origin',
            'strict-origin-when-cross-origin',
            'unsafe-url',
        ];

        if ($value === null) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Referrer-Policy header is not set. It controls referrer header sharing and enhances privacy and security.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif (in_array($value, $okValues)) {
            $severity = SecurityCheckedHeader::OK;
        } else {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "Referrer-Policy header is set to '{$value}'. This can be a security risk.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkContentSecurityPolicy(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_CONTENT_SECURITY_POLICY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "Content-Security-Policy header is not set. It restricts resources the page can load and prevents XSS attacks.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::OK;
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkFeaturePolicy(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_FEATURE_POLICY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null && isset($headers[self::HEADER_PERMISSIONS_POLICY]) && $headers[self::HEADER_PERMISSIONS_POLICY]) {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "Feature-Policy header is not set but Permissions-Policy is set. That's enough.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else if ($value === null) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Feature-Policy header is not set. It allows enabling/disabling browser APIs and features for security. Not important if Permissions-Policy is set.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::OK;
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkPermissionsPolicy(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_PERMISSIONS_POLICY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null && isset($headers[self::HEADER_FEATURE_POLICY]) && $headers[self::HEADER_FEATURE_POLICY]) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Permissions-Policy header is not set but Feature-Policy is. We recommend transforming it to this newer header.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($value === null) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Permissions-Policy header is not set. It allows enabling/disabling browser APIs and features for security.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::OK;
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkServer(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_SERVER;
        $value = $headers[$header] ?? null;

        $knownValues = [
            'Apache',
            'nginx',
            'Microsoft-IIS',
        ];

        $checkForKnownValues = function ($value) use ($knownValues) {
            foreach ($knownValues as $knownValue) {
                if (stripos($value, $knownValue) !== false) {
                    return true;
                }
            }
            return false;
        };

        if ($value === null || trim(strval($value), ' /-.~:') === '') {
            $severity = SecurityCheckedHeader::OK;
            $recommendation = "Server header is not set or empty. This is recommended.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif (preg_match('/[0-9]/', $value) === 1) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "Server header is set to '{$value}'. It is better not to reveal the technologies used and especially their versions.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } elseif ($checkForKnownValues($value)) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Server header is set to known '{$value}'. It is better not to reveal used technologies.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "Server header is set to '{$value}'. It is better not to reveal used technologies.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkXPoweredBy(array $headers, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_X_POWERED_BY;
        $recommendation = null;
        $value = $headers[$header] ?? null;

        if ($value === null) {
            return;
        } elseif (preg_match('/[0-9]/', $value) === 1) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "X-Powered-By header is set to '{$value}'. It is better not to reveal the technologies used and especially their versions.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        } else {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "X-Powered-By header is set to '{$value}'. It is better not to reveal used technologies.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader($header)->setFinding($value, $severity, $recommendation);
    }

    private function checkSetCookie(array $headers, bool $isHttps, UrlAnalysisResult $urlAnalysisResult): void
    {
        $header = self::HEADER_SET_COOKIE;
        $value = $headers[$header] ?? null;
        if (!is_array($value)) {
            return;
        }

        foreach ($value as $cookie) {
            $this->checkSetCookieValue($cookie, $isHttps, $urlAnalysisResult);
        }
    }

    private function checkSetCookieValue(string $setCookie, bool $isHttps, UrlAnalysisResult $urlAnalysisResult): void
    {
        $recommendation = null;
        $severity = SecurityCheckedHeader::OK;
        list($cookieName) = explode('=', $setCookie);

        if (stripos($setCookie, 'SameSite') === false) {
            $severity = SecurityCheckedHeader::NOTICE;
            $recommendation = "Set-Cookie header for '{$cookieName}' does not have 'SameSite' flag. Consider using 'SameSite=Strict' or 'SameSite=Lax'.";
            $urlAnalysisResult->addNotice($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }
        if (stripos($setCookie, 'HttpOnly') === false) {
            $severity = SecurityCheckedHeader::WARNING;
            $recommendation = "Set-Cookie header for '{$cookieName}' does not have 'HttpOnly' flag. Attacker can steal the cookie using XSS. Consider using 'HttpOnly' when cookie is not used by JavaScript.";
            $urlAnalysisResult->addWarning($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }
        if ($isHttps && stripos($setCookie, 'Secure') === false) {
            $severity = SecurityCheckedHeader::CRITICAL;
            $recommendation = "Set-Cookie header for '{$cookieName}' does not have 'Secure' flag. Attacker can steal the cookie over HTTP.";
            $urlAnalysisResult->addCritical($recommendation, self::ANALYSIS_HEADERS, [$recommendation]);
        }

        $this->result->getCheckedHeader(self::HEADER_SET_COOKIE)->setFinding($cookieName, $severity, $recommendation);
    }

    public function getOrder(): int
    {
        return 215;
    }

    public static function getAnalysisNames(): array
    {
        return [
            self::ANALYSIS_HEADERS,
        ];
    }

    public static function getOptions(): Options
    {
        return new Options();
    }

    /**
     * @return string[]
     */
    private static function getCheckedHeaders(): array
    {
        $reflection = new \ReflectionClass(self::class);
        return array_filter($reflection->getConstants(), function ($key) {
            return str_starts_with($key, 'HEADER_');
        }, ARRAY_FILTER_USE_KEY);
    }
}