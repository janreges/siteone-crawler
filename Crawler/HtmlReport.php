<?php

namespace Crawler;

class HtmlReport
{

    public static function generate(string $jsonReport): string
    {
        $data = json_decode($jsonReport, true);

        $html = '<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Website Crawler Report</title>
                <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.0.2/dist/css/bootstrap.min.css" rel="stylesheet">
            </head>
            <body>
                <div class="container mt-4" style="max-width: 1880px;">
                    <h1 class="mb-4">SiteOne Website Crawler Report</h1>
                    
                    <section class="mb-5">
                        <h2>Crawler Info</h2>
                        <table class="table table-bordered">
                            <tr>
                                <th>Version</th>
                                <td>' . htmlspecialchars($data['crawler']['version']) . '</td>
                            </tr>
                            <tr>
                                <th>Executed At</th>
                                <td>' . htmlspecialchars($data['crawler']['executedAt']) . '</td>
                            </tr>
                            <tr>
                                <th>Command</th>
                                <td>' . htmlspecialchars($data['crawler']['command']) . '</td>
                            </tr>
                            <tr>
                                <th>Hostname</th>
                                <td>' . htmlspecialchars($data['crawler']['hostname']) . '</td>
                            </tr>
                            <tr>
                                <th>Final User-Agent</th>
                                <td>' . htmlspecialchars($data['crawler']['finalUserAgent']) . '</td>
                            </tr>
                        </table>
                    </section>
            
                    <section class="mb-5">
                        <h2>Options</h2>
                        <table class="table table-bordered">';
                    foreach ($data['options'] as $key => $value) {
                        $html .= '<tr>
                                    <th>' . htmlspecialchars(ucfirst(str_replace('_', ' ', $key))) . '</th>
                                    <td>' . htmlspecialchars(is_array($value) ? implode(', ', $value) : (is_bool($value) ? ($value ? 'YES' : 'NO') : ($value ?: ''))) . '</td>
                                </tr>';
                    }
                    $html .= '</table>
                    </section>
            
                    <section class="mb-5">
                        <h2>Results</h2>
                        <table class="table table-bordered table-hover">
                            <thead>
                                <tr>
                                    <th>URL</th>
                                    <th>Status</th>
                                    <th>Elapsed Time</th>
                                    <th>Size</th>';
                    foreach ($data['results'][0]['extras'] as $key => $value) {
                        $html .= '<th>' . htmlspecialchars($key) . '</th>';
                    }
                    $html .= ' </tr>
                            </thead>
                            <tbody>';
                    foreach ($data['results'] as $result) {
                        $html .= '<tr>
                                    <td><a href="' . htmlspecialchars($result['url'], ENT_QUOTES, 'UTF-8') . '" target="_blank">' . htmlspecialchars($result['url']) . '</a></td>
                                    <td>' . htmlspecialchars($result['status']) . '</td>
                                    <td>' . htmlspecialchars($result['elapsedTime']) . ' sec</td>
                                    <td>' . htmlspecialchars(Utils::getFormattedSize($result['size'])). '</td>';
                        foreach ($result['extras'] as $extra) {
                            $html .= '<td>' . htmlspecialchars($extra) . '</td>';
                        }
                        $html .= '</tr>';
                    }
                    $html .= '</tbody>
                        </table>
                    </section>
            
                    <section class="mb-5">
                        <h2>Stats</h2>
                        <table class="table table-bordered">';
                    foreach ($data['stats'] as $key => $value) {
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
                    </section>
                    <section>
                        <br />
                        <hr />
                        The report was created <strong>' . date('Y-m-d - H:i:s') . '</strong> using the <a href="https://github.com/janreges/siteone-website-crawler"><strong>SiteOne Website Crawler</strong></a> with ♥ by Ján Regeš from <a href="https://www.siteone.io/?utm_source=siteone_crawler&utm_medium=email&utm_campaign=crawler_report&utm_content=v' . VERSION . '"><strong>SiteOne</strong></a> (Czech Republic).
                    </section>
                </div>
            </body>
            </html>';

        return $html;
    }


}