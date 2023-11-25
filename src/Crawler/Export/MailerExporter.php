<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\ParsedUrl;
use Crawler\Version;
use Exception;

class MailerExporter extends BaseExporter implements Exporter
{
    const GROUP_MAILER = 'mailer';

    protected array $mailTo = [];
    protected ?string $mailFrom = null;
    protected ?string $mailFromName = null;
    protected ?string $mailSmtpHost = null;
    protected ?int $mailSmtpPort = null;
    protected ?string $mailSmtpUser = null;
    protected ?string $mailSmtpPass = null;
    protected string $mailSubjectTemplate = '';

    /**
     * If true, crawler will not send any e-mails
     * Easy way how to disable e-mail sending in case of CTRL+C (handled in Crawler.php->run())
     *
     * @var bool
     */
    public static bool $crawlerInterrupted = false;

    public function shouldBeActivated(): bool
    {
        return count($this->mailTo) > 0;
    }

    public function export(): void
    {
        if (self::$crawlerInterrupted) {
            return;
        }

        $host = parse_url($this->status->getOptions()->url, PHP_URL_HOST);
        $datetime = date('YmdHis', strtotime($this->status->getCrawlerInfo()->executedAt));
        $htmlReport = new HtmlReport($this->status);
        $emailBody = $this->getEmailBody($host);

        $this->sendEmail($emailBody, "report-{$host}-{$datetime}.html", $htmlReport->getHtml());
        $this->status->addInfoToSummary('mail-report-sent', "HTML report sent to " . implode(', ', $this->mailTo) . ' using ' . $this->mailSmtpHost . ':' . $this->mailSmtpPort);
    }

    /**
     * @param string $htmlBody
     * @param string|null $attachedFileName
     * @param string|null $attachedFileContent
     * @return void
     * @throws Exception
     */
    private function sendEmail(string $htmlBody, ?string $attachedFileName = null, ?string $attachedFileContent = null): void
    {
        $htmlBodyForEmail = $this->styleHtmlBodyForEmail($htmlBody);
        $parsedUrl = $this->crawler->getInitialParsedUrl();

        $this->mailFrom = str_replace('@your-hostname.com', '@' . gethostname(), $this->mailFrom);

        $subject = str_replace(
            ['%domain%', '%date%', '%datetime%'],
            [$parsedUrl->host, date('Y-m-d'), date('Y-m-d H:i')],
            $this->mailSubjectTemplate
        );

        $this->sendEmailBySmtp(
            $this->mailTo,
            $this->mailFrom,
            $this->mailFromName,
            $subject,
            $htmlBodyForEmail,
            $attachedFileName,
            $attachedFileContent,
            $this->mailSmtpHost,
            $this->mailSmtpPort,
            $this->mailSmtpUser,
            $this->mailSmtpPass,
        );
    }

    private function getEmailBody(string $host): string
    {
        $body = 'Hello,<br>
<br>
We are pleased to deliver the attached report detailing a thorough crawling and analysis of your website, <b>{$host}</b>. Our advanced website crawler has identified key areas that require your attention, including found redirects, 404 error pages, and potential issues in accessibility, best practices, performance, and security.<br>
<br>
The report is in HTML format and for full functionality, it should be opened in a JavaScript-enabled browser. This will allow you to access advanced features such as searching and sorting data within tables. Some mobile email clients may not support all interactive elements.<br>
<br>
In case you have any suggestions for improvements and other useful features, feel free to send them as Feature requests to <a href="https://github.com/janreges/siteone-crawler/issues/">our project\'s GitHub</a>.<br>
<br>
Best regards,<br>
<br>
<a href="https://crawler.siteone.io/?utm_source=siteone_crawler&utm_medium=email-report&utm_campaign=crawler_report&utm_content=v{$version}">SiteOne Crawler</a> Team';

        $body = str_replace(
            [
                '{$host}',
                '{$version}'
            ],
            [
                $host,
                Version::CODE
            ],
            $body);

        return $body;

    }

    private function styleHtmlBodyForEmail(string $html): string
    {
        return str_replace(
            '<body>',
            '<body style="font-family: Arial, Helvetica, sans-serif;">
                    <style>
                        table {
                            border-collapse: collapse;
                        }
                    
                        body table, body table th, body table td {
                            border: 1px solid #555555;
                            padding: 3px !important;
                            vertical-align: top;
                            text-align: left;
                        }
                    </style>
                ',
            $html
        );
    }

    /**
     * @param string[] $recipients
     * @param string $sender
     * @param string $senderName
     * @param string $subject
     * @param string $htmlBody
     * @param string|null $attachedFileName
     * @param string|null $attachedFileContent
     * @param string $smtpHost
     * @param int $smtpPort
     * @param string|null $smtpUser
     * @param string|null $smtpPass
     * @return void
     * @throws Exception
     */
    private function sendEmailBySmtp(array $recipients, string $sender, string $senderName, string $subject, string $htmlBody, ?string $attachedFileName, ?string $attachedFileContent, string $smtpHost, int $smtpPort, ?string $smtpUser = null, ?string $smtpPass = null): void
    {
        // Connect to SMTP server
        $socket = @fsockopen($smtpHost, $smtpPort, $errno, $errstr, 5);
        if (!$socket) {
            throw new Exception("Failed to connect to SMTP server '{$smtpHost}:{$smtpPort}': $errstr ($errno)");
        }

        // Read server greeting
        $response = fgets($socket, 515);
        if (!str_starts_with($response, '220')) {
            fclose($socket);
            throw new Exception("Invalid server response: $response");
        }

        // Send HELO command
        fwrite($socket, "HELO {$smtpHost}\r\n");
        $response = fgets($socket, 515);
        if (!str_starts_with($response, '250')) {
            fclose($socket);
            throw new Exception("Invalid response to HELO command: $response");
        }

        // Authenticate
        if ($smtpUser && $smtpPass) {
            fwrite($socket, "AUTH LOGIN\r\n");
            $response = fgets($socket, 515);
            if (!str_starts_with($response, '334')) {
                fclose($socket);
                throw new Exception("Invalid response to AUTH command: $response");
            }

            fwrite($socket, base64_encode($smtpUser) . "\r\n");
            $response = fgets($socket, 515);
            if (!str_starts_with($response, '334')) {
                fclose($socket);
                throw new Exception("Invalid response to username: $response");
            }

            fwrite($socket, base64_encode($smtpPass) . "\r\n");
            $response = fgets($socket, 515);
            if (!str_starts_with($response, '235')) {
                fclose($socket);
                throw new Exception("Invalid response to password: $response");
            }
        }

        // Send MAIL FROM command
        fwrite($socket, "MAIL FROM: {$senderName} <{$sender}>\r\n");
        $response = fgets($socket, 515);
        if (!str_starts_with($response, '250')) {
            fclose($socket);
            throw new Exception("Invalid response to MAIL FROM command: $response");
        }

        // Send RCPT TO command for each recipient
        foreach ($recipients as $recipient) {
            fwrite($socket, "RCPT TO: <$recipient>\r\n");
            $response = fgets($socket, 515);
            if (!str_starts_with($response, '250')) {
                fclose($socket);
                throw new Exception("Invalid response to RCPT TO command: $response");
            }
        }

        // Send DATA command
        fwrite($socket, "DATA\r\n");
        $response = fgets($socket, 515);
        if (!str_starts_with($response, '354')) {
            fclose($socket);
            throw new Exception("Invalid response to DATA command: $response");
        }

        // Send headers and body
        if ($attachedFileName && $attachedFileContent) {
            $boundary = md5(uniqid(strval(time())));
            $headers = "From: {$senderName}<{$sender}>\r\n";
            $headers .= "MIME-Version: 1.0\r\n";
            $headers .= "Content-Type: multipart/mixed; boundary=\"{$boundary}\"\r\n";
            $headers .= "To: " . implode(", ", $recipients) . "\r\n";
            $headers .= "Subject: $subject\r\n";
            $headers .= "\r\n";

            // Message part
            $headers .= "--{$boundary}\r\n";
            $headers .= "Content-Type: text/html; charset=utf-8\r\n";
            $headers .= "\r\n";
            $headers .= $htmlBody . "\r\n";

            // Attachment part
            $filename = basename($attachedFileName);
            $attachmentData = $attachedFileContent;
            $base64Attachment = chunk_split(base64_encode($attachmentData));
            $headers .= "--{$boundary}\r\n";
            $headers .= "Content-Type: application/octet-stream; name=\"{$filename}\"\r\n";
            $headers .= "Content-Transfer-Encoding: base64\r\n";
            $headers .= "Content-Disposition: attachment; filename=\"{$filename}\"\r\n";
            $headers .= "\r\n";
            $headers .= $base64Attachment . "\r\n";

            // Boundary end
            $headers .= "--{$boundary}--\r\n";
        } else {
            $headers = "From: {$senderName}<{$sender}>\r\n";
            $headers .= "MIME-Version: 1.0\r\n";
            $headers .= "Content-type: text/html; charset=utf-8\r\n";
            $headers .= "To: " . implode(", ", $recipients) . "\r\n";
            $headers .= "Subject: $subject\r\n";
            $headers .= "\r\n";
            $headers .= $htmlBody;
        }
        fwrite($socket, $headers . "\r\n.\r\n");

        $response = fgets($socket, 515);
        if (!str_starts_with($response, '250')) {
            fclose($socket);
            throw new Exception("Invalid response to end of DATA command: $response");
        }

        // Quit and close
        fwrite($socket, "QUIT\r\n");
        fclose($socket);
    }

    /**
     * @inheritDoc
     */
    public static function getOptions(): Options
    {
        $options = new Options();
        $options->addGroup(new Group(
            self::GROUP_MAILER,
            'Mailer options', [
            new Option('--mail-to', null, 'mailTo', Type::EMAIL, true, 'E-mail report recipient address(es). Can be specified multiple times.', [], true, true),
            new Option('--mail-from', null, 'mailFrom', Type::EMAIL, false, 'E-mail sender address.', 'siteone-crawler@your-hostname.com', false),
            new Option('--mail-from-name', null, 'mailFromName', Type::STRING, false, 'E-mail sender name', 'SiteOne Crawler', false),
            new Option('--mail-subject-template', null, 'mailSubjectTemplate', Type::STRING, false, 'E-mail subject template. You can use dynamic variables %domain% and %datetime%', 'Crawler Report for %domain% (%date%)', true),
            new Option('--mail-smtp-host', null, 'mailSmtpHost', Type::STRING, false, 'SMTP host.', 'localhost', true),
            new Option('--mail-smtp-port', null, 'mailSmtpPort', Type::INT, false, 'SMTP port.', 25, true),
            new Option('--mail-smtp-user', null, 'mailSmtpUser', Type::STRING, false, 'SMTP user for authentication.', null, true),
            new Option('--mail-smtp-pass', null, 'mailSmtpPass', Type::STRING, false, 'SMTP password for authentication.', null, true),
        ]));
        return $options;
    }
}