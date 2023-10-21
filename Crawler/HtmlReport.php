<?php

namespace Crawler;

use Crawler\Components\SuperTable;
use Crawler\Result\Status;
use Crawler\Result\Summary\Summary;

class HtmlReport
{

    public static function generate(Status $status): string
    {
        $crawlerInfo = $status->getCrawlerInfo();

        $html = '<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>SiteOne Website Crawler Report - ' . htmlspecialchars($status->getOption('url')) . '</title>
                <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.0.2/dist/css/bootstrap.min.css" rel="stylesheet">
                <style>
                        table { border-collapse: collapse;  }
                        table.table-compact { font-size: 0.9em; }
                        table, table th, table td {
                            border: 1px #dee2e6 solid;
                            padding: 2px 4px !important;
                            vertical-align: top;
                            text-align: left;
                        } 
                        table.table-two-col th {
                            background-color: #f3f3f3;
                            width: 20%;
                        }
                    </style>
            </head>
            <body>
                <div class="container mt-4" style="max-width: 1880px;">
                    <h1 class="mb-4">
                        <a href="https://www.siteone.io/?utm_source=siteone_crawler&utm_medium=logo&utm_campaign=crawler_report&utm_content=v' . htmlspecialchars($crawlerInfo->version) . '" target="_blank" style="color: #ffffff; text-decoration: none;">  
                            <svg viewBox="0 0 119 70" width="61px" height="34px" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <path d="M92.0551 14.9476V48.07H75.2954V58.0351H118.638V48.07H102.303V0H92.9895L66.8594 26.13L73.8804 33.1223C73.9083 33.1223 92.0551 14.9476 92.0551 14.9476Z" fill="#999999"></path>
                                <path fill-rule="evenodd" clip-rule="evenodd" d="M0 0.0527344H57.9785V58.0312H0V0.0527344ZM10.25 48.0639H47.7323V10.0156H10.25V48.0639Z" fill="#333333"></path>
                            </svg>
                        </a>
                        Website crawler report
                    </h1>
                    
                    <section class="mb-5">
                        <h2>Basic info</h2>
                        <table class="table table-bordered table-hover table-two-col" style="border-collapse: collapse;">
                            <tr>
                                <th>Version</th>
                                <td>' . htmlspecialchars($crawlerInfo->version) . '</td>
                            </tr>
                            <tr>
                                <th>Executed At</th>
                                <td>' . htmlspecialchars($crawlerInfo->executedAt) . '</td>
                            </tr>
                            <tr>
                                <th>Command</th>
                                <td>' . htmlspecialchars($crawlerInfo->command) . '</td>
                            </tr>
                            <tr>
                                <th>Hostname</th>
                                <td>' . htmlspecialchars($crawlerInfo->hostname) . '</td>
                            </tr>
                            <tr>
                                <th>User-Agent</th>
                                <td>' . htmlspecialchars($crawlerInfo->finalUserAgent) . '</td>
                            </tr>
                        </table>
                    </section>
            
                    ';

        $html .= self::getHtmlForSummary($status->getSummary());
        $html .= '
                 
                    <section class="mb-5">
                        <h2>Stats</h2>
                        <table class="table table-bordered table-hover table-two-col" style="border-collapse: collapse;">';
        foreach ((array)$status->getBasicStats() as $key => $value) {
            $html .= '<tr>
                                    <th>' . htmlspecialchars(ucfirst(str_replace('_', ' ', $key))) . '</th>
                                    <td>';
            if (is_array($value)) {
                foreach ($value as $key2 => $value2) {
                    $html .= '<strong>' . htmlspecialchars($key2) . '</strong>: ' . htmlspecialchars($value2) . '<br>';
                }
            } else {
                $html .= htmlspecialchars($value);
            }
            $html .= '</td>
                                </tr>';
        }
        $html .= '</table>
                    </section>';

        $html .= self::getHtmlForSupertables($status->getSuperTablesAtBeginning());

        $html .= '<section class="mb-5">
                        <h2>Results</h2>
                        <table class="table table-bordered table-hover table-compact" style="border-collapse: collapse;">
                            <thead>
                                <tr>
                                    <th>URL</th>
                                    <th>Status</th>
                                    <th>Type</th>
                                    <th style="width: 80px">Time (s)</th>
                                    <th style="width: 80px">Size</th>';

        $extraHeaders = $status->getOptions()->extraColumnsNamesOnly;
        foreach ($extraHeaders as $headerName) {
            $html .= '<th>' . htmlspecialchars($headerName) . '</th>';
        }
        $html .= ' </tr>
                            </thead>
                            <tbody>';
        foreach ($status->getVisitedUrls() as $visitedUrl) {
            $html .= '<tr>
                                    <td><a href="' . htmlspecialchars($visitedUrl->url, ENT_QUOTES, 'UTF-8') . '" target="_blank">' . htmlspecialchars($visitedUrl->url) . '</a></td>
                                    <td>' . htmlspecialchars($visitedUrl->statusCode) . '</td>
                                    <td>' . htmlspecialchars(Utils::getContentTypeNameById($visitedUrl->contentType)) . '</td>
                                    <td>' . htmlspecialchars($visitedUrl->requestTimeFormatted) . ' sec</td>
                                    <td>' . htmlspecialchars($visitedUrl->sizeFormatted) . '</td>';
            foreach ($extraHeaders as $headerName) {
                $value = $visitedUrl->extras[$headerName] ?? '';
                $html .= '<td>' . htmlspecialchars($value) . '</td>';
            }
            $html .= '</tr>';
        }
        $html .= '</tbody>
                        </table>
                    </section>';

        $html .= self::getHtmlForSupertables($status->getSuperTablesAtEnd());

        $html .= '<section>
                        <br />
                        <hr />
                        The report was generated <strong>' . date('Y-m-d - H:i:s') . '</strong> using the ♥ <a href="https://github.com/janreges/siteone-website-crawler"><strong>SiteOne Website Crawler</strong></a> by Ján Regeš from <a href="https://www.siteone.io/?utm_source=siteone_crawler&utm_medium=email&utm_campaign=crawler_report&utm_content=v' . VERSION . '"><strong>SiteOne</strong></a> (Czech Republic).<br />
                        <br />
                    </section>
                </div>
            </body>
            </html>';

        return $html;
    }

    /**
     * @param SuperTable[] $supertables
     * @return string
     */
    private static function getHtmlForSupertables(array $supertables): string
    {
        $result = '';
        foreach ($supertables as $supertable) {
            $result .= $supertable->getHtmlOutput() . "\n";
        }
        return $result;
    }

    /**
     * @param Summary $summary
     * @return string
     */
    private static function getHtmlForSummary(Summary $summary): string
    {
        $result = '<section class="mb-5"><h2>Summary</h2>' . "\n";
        $result .= $summary->getAsHtml();
        $result .= '</section>';
        return $result;
    }
}