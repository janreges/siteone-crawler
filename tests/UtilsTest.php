<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Utils;
use PHPUnit\Framework\TestCase;

class UtilsTest extends TestCase
{

    public function testParsePhoneNumbersFromHtml()
    {
        $html = '<html>
            <body>
                <b>number 1234567890</b>
                <b>+420 123 456 789</b>
                <b>test +420123456789 test</b>
                <b>00 420 123 456 789</b>
                <b>00420123456789</b>
                <b>(123)123-456-789</b>
                <b>(123) 123-456-789</b>
                <a href="tel:+420987654321">+420987654321</a>
                <a href="tel:+420 987 654 321">+420 987 654 321</a>
                <a href="tel:00420987654321">00420987654321</a>
                <a href="tel:whatever">00 420 987 654 321</a>
                <a href="tel:(987) 987-654-321">foo</a>
                02-11-2023
                +1 123-456-7890
                234567
                +61 2 1234 5678
                +55 11 12345-6789
                +86 123 4567 8901
                +91 12345-67890
                +52 1 234 567 8900
                +34 912 34 56 78
                +46 123-456 78
                +47 123 45 678
                +358 123 456789
                (123) 456-7890
                123-456-7890
            </body>
        </html>';

        $expected = [
            '+420 123 456 789',
            '+420123456789',
            '+1 123-456-7890',
            '+61 2 1234 5678',
            '+55 11 12345-6789',
            '+86 123 4567 8901',
            '+91 12345-67890',
            '+52 1 234 567 8900',
            '+34 912 34 56 78',
            '+46 123-456 78',
            '+47 123 45 678',
            '+358 123 456789',
            '(123) 456-7890',
            '123-456-7890',
        ];

        $result = Utils::parsePhoneNumbersFromHtml($html, true);

        $this->assertEquals($expected, $result);
    }

}