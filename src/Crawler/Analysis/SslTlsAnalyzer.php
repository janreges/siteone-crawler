<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Components\SuperTable;
use Crawler\Components\SuperTableColumn;
use Crawler\Options\Options;
use Crawler\Result\Summary\Item;
use Crawler\Result\Summary\ItemStatus;
use Crawler\Utils;

class SslTlsAnalyzer extends BaseAnalyzer implements Analyzer
{
    const SUPER_TABLE_CERTIFICATE_INFO = 'certificate-info';

    public function shouldBeActivated(): bool
    {
        return true;
    }

    public function analyze(): void
    {
        if (!str_starts_with($this->status->getOptions()->url, 'https://')) {
            $this->status->getSummary()->addItem(new Item('ssl-tls-analyzer', "SSL/TLS not supported, analyzer skipped.", ItemStatus::NOTICE));
            return;
        }

        $host = $this->status->getOptions()->getInitialHost(false);

        $s = microtime(true);
        $certificateInfo = $this->getTLSandSSLCertificateInfo($host);
        $this->measureExecTime(__CLASS__, 'getTLSandSSLCertificateInfo', $s);

        $consoleWidth = Utils::getConsoleWidth();

        $tableData = [];
        foreach ($certificateInfo as $key => $value) {
            if ($value) {
                $tableData[] = ['info' => $key, 'value' => $value];
            }
        }

        $superTable = new SuperTable(
            self::SUPER_TABLE_CERTIFICATE_INFO,
            'SSL/TLS info',
            'No SSL/TLS info.',
            [
                new SuperTableColumn('info', 'Info', SuperTableColumn::AUTO_WIDTH),
                new SuperTableColumn('value', 'Text', $consoleWidth - 30, function ($value, $renderInto) {
                    if (is_array($value)) {
                        if ($value && (str_contains($value[0], 'TLS') || str_contains($value[0], 'SSL'))) {
                            // ssl/tls protocols
                            $value = implode(', ', $value);
                            $result = str_replace('SSLv2', Utils::getColorText('SSLv2', 'red', true), $value);
                            $result = str_replace('SSLv3', Utils::getColorText('SSLv3', 'red', true), $result);
                            $result = str_replace('TLSv1.0', Utils::getColorText('TLSv1.0', 'red', true), $result);
                            $result = str_replace('TLSv1.1', Utils::getColorText('TLSv1.1', 'red', true), $result);
                            $result = str_replace('TLSv1.2', Utils::getColorText('TLSv1.2', 'green', true), $result);
                            $result = str_replace('TLSv1.3', Utils::getColorText('TLSv1.3', 'green', true), $result);
                            return $result;
                        } else {
                            // errors
                            $result = count($value) . " error(s): ";
                            $result .= implode($renderInto === SuperTable::RENDER_INTO_HTML ? '<br>' : ", ", $value);

                            $result = trim($result, ' :,');
                            return count($value) > 0 ? Utils::getColorText($result, 'red', true) : Utils::getColorText($result, 'green');
                        }
                    }

                    if ($renderInto === SuperTable::RENDER_INTO_HTML) {
                        return nl2br(str_replace(' ', '&nbsp;', $value));
                    } else {
                        return $value;
                    }
                }, null, true, true, false, false),
            ],
            true
        );

        $superTable->setData($tableData);
        $this->status->addSuperTableAtBeginning($superTable);
        $this->output->addSuperTable($superTable);

        if ($certificateInfo && isset($certificateInfo['Issuer'])) {
            $this->status->getSummary()->addItem(new Item('certificate-info', "SSL/TLS certificate issued by '{$certificateInfo['Issuer']}'.", ItemStatus::OK));
        } else {
            $this->status->getSummary()->addItem(new Item('certificate-info', 'SSL/TLS: unable to load certificate info', ItemStatus::CRITICAL));
        }
    }

    private function getTLSandSSLCertificateInfo(string $hostname, int $port = 443): array
    {
        $isCygwin = stripos(PHP_OS, 'CYGWIN') !== false;
        if ($isCygwin) {
            // fallback to native PHP functions without shell_exec
            return $this->getCertificateInfoNative($hostname, $port);
        }

        // get certificate
        $certificateCommand = "timeout 3s echo | openssl s_client -connect {$hostname}:{$port} -servername {$hostname} 2>/dev/null | openssl x509 -text -noout";
        $certificateOutput = shell_exec($certificateCommand);

        // get supported protocols
        $protocols = [
            'ssl2' => 'SSLv2',
            'ssl3' => 'SSLv3',
            'tls1' => 'TLSv1.0',
            'tls1_1' => 'TLSv1.1',
            'tls1_2' => 'TLSv1.2',
            'tls1_3' => 'TLSv1.3',
        ];
        $supportedProtocols = [];
        $unsafeProtocols = ['ssl2', 'ssl3', 'tls1', 'tls1_1'];
        $protocolsOutput = "";

        foreach ($protocols as $protocolCode => $protocolName) {
            $protocolsCommand = "timeout 3s echo 'Q' | openssl s_client -connect {$hostname}:{$port} -servername {$hostname} -{$protocolCode} 2>&1";
            $protocolsOutputEach = shell_exec($protocolsCommand);
            $protocolsOutput .= "\n=== {$protocolCode} ===\n" . $protocolsOutputEach;

            if ($protocolsOutputEach && str_contains($protocolsOutputEach, 'Certificate chain')) {
                $supportedProtocols[] = $protocolName;
                // report unsafe protocols
                if (in_array($protocolCode, $unsafeProtocols)) {
                    $this->status->getSummary()->addItem(new Item('ssl-protocol-unsafe', "SSL/TLS protocol {$protocolName} is unsafe.", ItemStatus::CRITICAL));
                }
            }
        }

        if (!in_array('TLSv1.3', $supportedProtocols)) {
            $this->status->getSummary()->addItem(new Item('ssl-protocol-hint', "Latest SSL/TLS protocol TLSv1.3 is not supported. Ask your admin/provider to add TLSv1.3 support.", ItemStatus::WARNING));
        } else if (!in_array('TLSv1.2', $supportedProtocols)) {
            $this->status->getSummary()->addItem(new Item('ssl-protocol-hint', "SSL/TLS protocol TLSv1.2 is not supported. Ask your admin/provider to add TLSv1.2 support.", ItemStatus::CRITICAL));
        }

        // parse info
        $errors = [];
        $issuer = $subject = $validFrom = $validTo = "";
        if ($certificateOutput && preg_match("/Issuer:\s*(.+?)\n/", $certificateOutput, $matches)) {
            $issuer = $matches[1];
        }
        if ($certificateOutput && preg_match("/Subject:\s*(.+?)\n/", $certificateOutput, $matches)) {
            $subject = $matches[1];
        }
        if ($certificateOutput && preg_match("/Not Before:\s*(.+?)\n/", $certificateOutput, $matches)) {
            $validFrom = $matches[1];

            // check if the certificate is not yet valid
            if (time() < strtotime($validFrom)) {
                $error = "SSL/TLS certificate is not yet valid, it will be in " . Utils::getFormattedAge(abs(strtotime($validFrom) - time())) . ".";
                $this->status->getSummary()->addItem(new Item('ssl-certificate-valid-from', $error, ItemStatus::CRITICAL));
                $validFrom .= " (" . Utils::getColorText('NOT YET VALID', 'red', true) . ")";
                $errors[] = $error;
            } else {
                $validFrom .= " (" . Utils::getColorText('VALID already ' . Utils::getFormattedAge(abs(strtotime($validFrom) - time())), 'green') . ")";
            }
        }

        $validToOrig = "";
        if ($certificateOutput && preg_match("/Not After\s*:\s*(.+?)\n/", $certificateOutput, $matches)) {
            $validTo = $matches[1];
            $validToOrig = $validTo;

            // check if the certificate is expired
            if (time() > strtotime($validTo)) {
                $expiredAgo = Utils::getFormattedAge(abs(strtotime($validTo) - time())) . " ago";
                $error = "SSL/TLS certificate expired {$expiredAgo}.";
                $this->status->getSummary()->addItem(new Item('ssl-certificate-valid-to', $error, ItemStatus::CRITICAL));
                $validTo .= " (" . Utils::getColorText('EXPIRED ' . $expiredAgo, 'red', true) . ")";
                $errors[] = $error;
            } else {
                $validTo .= " (" . Utils::getColorText('VALID still for ' . Utils::getFormattedAge(abs(strtotime($validTo) - time())), 'green') . ")";
            }
        }

        if (!$errors && $issuer && $validToOrig) {
            $this->status->getSummary()->addItem(new Item('ssl-certificate-valid', "SSL/TLS certificate is valid until {$validToOrig}. Issued by {$issuer}. Subject is {$subject}", ItemStatus::OK));
        }

        return [
            'Issuer' => $issuer,
            'Subject' => $subject,
            'Valid from' => $validFrom,
            'Valid to' => $validTo,
            'Supported protocols' => $supportedProtocols,
            'Errors' => $errors,
            'RAW certificate output' => $certificateOutput,
            'RAW protocols output' => $protocolsOutput,
        ];
    }

    /**
     * Fallback to native PHP functions without shell_exec
     *
     * @param string $hostname
     * @param int $port
     * @return array
     */
    private function getCertificateInfoNative(string $hostname, int $port = 443): array
    {
        $errors = [];
        $issuer = $subject = $validFrom = $validTo = "";
        $supportedProtocols = [];

        $context = stream_context_create(["ssl" => [
            "capture_peer_cert" => true,
            "verify_peer" => false,
            "verify_peer_name" => false,
            "allow_self_signed" => true,
        ]]);

        $fp = @stream_socket_client("ssl://{$hostname}:{$port}", $errno, $errstr, 3, STREAM_CLIENT_CONNECT, $context);
        if (!$fp) {
            $error = "Unable to connect via SSL to {$hostname}:{$port}. Error: {$errstr}";
            $this->status->getSummary()->addItem(new Item('ssl-certificate-connect', $error, ItemStatus::CRITICAL));
            $errors[] = $error;
            return [
                'Issuer' => '',
                'Subject' => '',
                'Valid from' => '',
                'Valid to' => '',
                'Supported protocols' => $supportedProtocols,
                'Errors' => $errors,
                'RAW certificate output' => '',
                'RAW protocols output' => '',
            ];
        }

        $cert = stream_context_get_params($fp);
        if (!isset($cert['options']['ssl']['peer_certificate'])) {
            $error = "No certificate found.";
            $this->status->getSummary()->addItem(new Item('ssl-certificate-missing', $error, ItemStatus::CRITICAL));
            $errors[] = $error;
            return [
                'Issuer' => '',
                'Subject' => '',
                'Valid from' => '',
                'Valid to' => '',
                'Supported protocols' => $supportedProtocols,
                'Errors' => $errors,
                'RAW certificate output' => '',
                'RAW protocols output' => '',
            ];
        }

        $certinfo = openssl_x509_parse($cert['options']['ssl']['peer_certificate']);
        if (!$certinfo) {
            $error = "Unable to parse certificate.";
            $this->status->getSummary()->addItem(new Item('ssl-certificate-parse', $error, ItemStatus::CRITICAL));
            $errors[] = $error;
            return [
                'Issuer' => '',
                'Subject' => '',
                'Valid from' => '',
                'Valid to' => '',
                'Supported protocols' => $supportedProtocols,
                'Errors' => $errors,
                'RAW certificate output' => '',
                'RAW protocols output' => '',
            ];
        }

        if (isset($certinfo['issuer'])) {
            $issuerParts = [];
            foreach ($certinfo['issuer'] as $k => $v) {
                $issuerParts[] = "{$k}={$v}";
            }
            $issuer = implode(', ', $issuerParts);
        }

        if (isset($certinfo['subject'])) {
            $subjectParts = [];
            foreach ($certinfo['subject'] as $k => $v) {
                $subjectParts[] = "{$k}={$v}";
            }
            $subject = implode(', ', $subjectParts);
        }

        if (isset($certinfo['validFrom_time_t'])) {
            $validFromTime = $certinfo['validFrom_time_t'];
            $validFrom = gmdate("Y-m-d H:i:s", $validFromTime) . " UTC";
            if (time() < $validFromTime) {
                $error = "SSL/TLS certificate is not yet valid, it will be in " . Utils::getFormattedAge(abs($validFromTime - time())) . ".";
                $this->status->getSummary()->addItem(new Item('ssl-certificate-valid-from', $error, ItemStatus::CRITICAL));

                $validFrom .= " (" . Utils::getColorText('NOT YET VALID', 'red', true) . ")";
                $errors[] = $error;
            } else {
                $validFrom .= " (" . Utils::getColorText('VALID already ' . Utils::getFormattedAge(abs($validFromTime - time())), 'green') . ")";
            }
        }

        $validToOrig = "";
        if (isset($certinfo['validTo_time_t'])) {
            $validToTime = $certinfo['validTo_time_t'];
            $validTo = gmdate("Y-m-d H:i:s", $validToTime) . " UTC";
            $validToOrig = $validTo;

            if (time() > $validToTime) {
                $expiredAgo = Utils::getFormattedAge(abs($validToTime - time())) . " ago";
                $error = "SSL/TLS certificate expired {$expiredAgo}.";
                $this->status->getSummary()->addItem(new Item('ssl-certificate-valid-to', $error, ItemStatus::CRITICAL));

                $validTo .= " (" . Utils::getColorText('EXPIRED ' . $expiredAgo, 'red', true) . ")";
                $errors[] = $error;
            } else {
                $validTo .= " (" . Utils::getColorText('VALID still for ' . Utils::getFormattedAge(abs($validToTime - time())), 'green') . ")";
            }
        }

        if (!$errors && $issuer && $validToOrig) {
            $this->status->getSummary()->addItem(new Item('ssl-certificate-valid', "SSL/TLS certificate is valid until {$validToOrig}. Issued by {$issuer}. Subject is {$subject}", ItemStatus::OK));
        }

        return [
            'Issuer' => $issuer,
            'Subject' => $subject,
            'Valid from' => $validFrom,
            'Valid to' => $validTo,
            'Supported protocols' => $supportedProtocols,
            'Errors' => $errors,
            'RAW certificate output' => '', // N/A
            'RAW protocols output' => '', // N/A
        ];
    }

    public function getOrder(): int
    {
        return 20;
    }

    public static function getOptions(): Options
    {
        return new Options();
    }
}