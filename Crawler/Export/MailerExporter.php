<?php

namespace Crawler\Export;

use Crawler\HtmlReport;
use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Output\JsonOutput;
use Crawler\Output\MultiOutput;
use Crawler\Output\OutputType;
use Crawler\ParsedUrl;
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

    public function shouldBeActivated(): bool
    {
        return count($this->mailTo) > 0;
    }

    public function export(): void
    {
        $multiOutput = $this->output;
        /* @var $multiOutput MultiOutput */
        $jsonOutput = $multiOutput->getOutputByType(OutputType::JSON);
        /* @var $jsonOutput JsonOutput */
        $htmlReport = HtmlReport::generate($jsonOutput->getJson());
        $this->sendEmail($htmlReport);
        $this->output->addNotice("HTML report sent to " . implode(', ', $this->mailTo) . ".");
    }

    /**
     * @throws Exception
     */
    private function sendEmail(string $htmlBody): void
    {
        $htmlBodyForEmail = $this->styleHtmlBodyForEmail($htmlBody);
        $parsedUrl = ParsedUrl::parse($this->crawler->getCoreOptions()->url);

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
            $this->mailSmtpHost,
            $this->mailSmtpPort,
            $this->mailSmtpUser,
            $this->mailSmtpPass,
        );
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
     * @param string $smtpHost
     * @param int $smtpPort
     * @param string|null $smtpUser
     * @param string|null $smtpPass
     * @return void
     * @throws Exception
     */
    private function sendEmailBySmtp(array $recipients, string $sender, string $senderName, string $subject, string $htmlBody, string $smtpHost, int $smtpPort, ?string $smtpUser = null, ?string $smtpPass = null): void
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
        $headers = "From: {$senderName}<{$sender}>\r\n";
        $headers .= "MIME-Version: 1.0\r\n";
        $headers .= "Content-type: text/html; charset=utf-8\r\n";
        $headers .= "To: " . implode(", ", $recipients) . "\r\n";
        $headers .= "Subject: $subject\r\n";
        $headers .= "\r\n";
        $headers .= $htmlBody;
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
            new Option('--mail-from', null, 'mailFrom', Type::EMAIL, false, 'E-mail sender address.', 'siteone-website-crawler@your-hostname.com', false),
            new Option('--mail-from-name', null, 'mailFromName', Type::STRING, false, 'E-mail sender name', 'SiteOne Crawler', false),
            new Option('--mail-subject-template', null, 'mailSubjectTemplate', Type::STRING, false, 'E-mail subject template. You can use dynamic variables %domain% and %datetime%', 'Crawler report for %domain% (%datetime%)', true),
            new Option('--mail-smtp-host', null, 'mailSmtpHost', Type::STRING, false, 'SMTP host.', 'localhost', true),
            new Option('--mail-smtp-port', null, 'mailSmtpPort', Type::INT, false, 'SMTP port.', 25, true),
            new Option('--mail-smtp-user', null, 'mailSmtpUser', Type::STRING, false, 'SMTP user for authentication.', null, true),
            new Option('--mail-smtp-pass', null, 'mailSmtpPass', Type::STRING, false, 'SMTP password for authentication.', null, true),
        ]));
        return $options;
    }
}