<?php

namespace Crawler;

class Mailer
{

    private Options $options;

    /**
     * @param Options $options
     */
    public function __construct(Options $options)
    {
        $this->options = $options;
    }

    /**
     * @throws \Exception
     */
    public function sendEmail(string $htmlBody): void
    {
        $htmlBodyForEmail = $this->styleHtmlBodyForEmail($htmlBody);
        $parsedUrl = ParsedUrl::parse($this->options->url);

        $this->sendEmailBySMTP(
            $this->options->mailTo,
            $this->options->mailFrom,
            "SiteOne Crawler report for {$parsedUrl->host} (" . date('Y-m-d H:i:s') . ')',
            $htmlBodyForEmail,
            $this->options->mailSmtpHost,
            $this->options->mailSmtpPort,
            $this->options->mailSmtpUser,
            $this->options->mailSmtpPass,
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
                    
                        table, th, td {
                            border: 1px solid #666666;
                            padding: 4px;
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
     * @param string $subject
     * @param string $htmlBody
     * @param string $smtpHost
     * @param int $smtpPort
     * @param string|null $smtpUser
     * @param string|null $smtpPass
     * @return void
     * @throws \Exception
     */
    private function sendEmailBySMTP(array $recipients, string $sender, string $subject, string $htmlBody, string $smtpHost, int $smtpPort, ?string $smtpUser = null, ?string $smtpPass = null): void
    {
        $senderName = 'SiteOne Crawler';

        // Connect to SMTP server
        $socket = fsockopen($smtpHost, $smtpPort, $errno, $errstr, 10);
        if (!$socket) {
            throw new \Exception("Failed to connect to SMTP server '{$smtpHost}:{$smtpPort}': $errstr ($errno)");
        }

        // Read server greeting
        $response = fgets($socket, 515);
        if (substr($response, 0, 3) != '220') {
            fclose($socket);
            throw new \Exception("Invalid server response: $response");
        }

        // Send HELO command
        fwrite($socket, "HELO {$smtpHost}\r\n");
        $response = fgets($socket, 515);
        if (substr($response, 0, 3) != '250') {
            fclose($socket);
            throw new \Exception("Invalid response to HELO command: $response");
        }

        // Authenticate
        if ($smtpUser && $smtpPass) {
            fwrite($socket, "AUTH LOGIN\r\n");
            $response = fgets($socket, 515);
            if (substr($response, 0, 3) != '334') {
                fclose($socket);
                throw new \Exception("Invalid response to AUTH command: $response");
            }

            fwrite($socket, base64_encode($smtpUser) . "\r\n");
            $response = fgets($socket, 515);
            if (substr($response, 0, 3) != '334') {
                fclose($socket);
                throw new \Exception("Invalid response to username: $response");
            }

            fwrite($socket, base64_encode($smtpPass) . "\r\n");
            $response = fgets($socket, 515);
            if (substr($response, 0, 3) != '235') {
                fclose($socket);
                throw new \Exception("Invalid response to password: $response");
            }
        }

        // Send MAIL FROM command
        fwrite($socket, "MAIL FROM: {$senderName} <{$sender}>\r\n");
        $response = fgets($socket, 515);
        if (substr($response, 0, 3) != '250') {
            fclose($socket);
            throw new \Exception("Invalid response to MAIL FROM command: $response");
        }

        // Send RCPT TO command for each recipient
        foreach ($recipients as $recipient) {
            fwrite($socket, "RCPT TO: <$recipient>\r\n");
            $response = fgets($socket, 515);
            if (substr($response, 0, 3) != '250') {
                fclose($socket);
                throw new \Exception("Invalid response to RCPT TO command: $response");
            }
        }

        // Send DATA command
        fwrite($socket, "DATA\r\n");
        $response = fgets($socket, 515);
        if (substr($response, 0, 3) != '354') {
            fclose($socket);
            throw new \Exception("Invalid response to DATA command: $response");
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
        if (substr($response, 0, 3) != '250') {
            fclose($socket);
            throw new \Exception("Invalid response to end of DATA command: $response");
        }

        // Quit and close
        fwrite($socket, "QUIT\r\n");
        fclose($socket);
    }

}