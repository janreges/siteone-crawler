<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class ExtraColumn
{

    public readonly string $name;
    public readonly ?int $length;
    public readonly bool $truncate;

    /**
     * Custom extraction method and pattern (xpath or regexp)
     * @var string|null
     */
    public ?string $customMethod = null;

    /**
     * Custom extraction pattern
     * Xpath example: '//div[@class="title"]' or '/<title>(.*?)<\/title>/i' or '//a/@href'
     * Regexp example: '<title>(.*?)<\/title>' or '<h1[^>]*>(.*?)<\/h1>'
     * @var string|null
     */
    public ?string $customPattern = null;

    /**
     * Custom extraction group (optional) defined by number #x in regex pattern
     * @var int|null
     */
    public ?int $customGroup = null;

    private static array $defaultColumnSizes = [
        'Title' => 20,
        'Description' => 20,
        'Keywords' => 20,
    ];

    public const CUSTOM_METHOD_XPATH = 'xpath';
    public const CUSTOM_METHOD_REGEXP = 'regexp';

    /**
     * Constructor
     * @param string $name
     * @param int|null $length
     * @param bool $truncate
     * @param string|null $customMethod
     * @param string|null $customPattern
     * @param int|null $customGroup
     */
    public function __construct(string $name, ?int $length, bool $truncate, ?string $customMethod = null, ?string $customPattern = null, ?int $customGroup = null)
    {
        $this->name = $name;
        $this->length = $length;
        $this->truncate = $truncate;

        // Validate customMethod if provided
        if ($customMethod !== null) {
            $customMethodLower = strtolower($customMethod);
            if (!in_array($customMethodLower, [self::CUSTOM_METHOD_XPATH, self::CUSTOM_METHOD_REGEXP], true)) {
                throw new \InvalidArgumentException("Invalid custom extraction method: $customMethod. Expected '" . self::CUSTOM_METHOD_XPATH . "' or '" . self::CUSTOM_METHOD_REGEXP . "'.");
            }
            $this->customMethod = $customMethodLower;
        }

        // Validate and assign customPattern if provided
        if ($customPattern !== null) {
            $this->customPattern = $customPattern;
            if ($this->customMethod === self::CUSTOM_METHOD_REGEXP) {
                // Validate the complete regex provided by user (should include delimiters and modifiers)
                if (@preg_match($customPattern, "") === false) {
                    throw new \InvalidArgumentException("Invalid regexp pattern provided: $customPattern");
                }
            }
        }

        $this->customGroup = $customGroup;
    }

    public function getLength(): int
    {
        return $this->length !== null ? $this->length : strlen($this->name);
    }

    public function getTruncatedValue(?string $value): ?string
    {
        if ($value === null) {
            return null;
        }

        $length = $this->getLength();
        if ($this->truncate && mb_strlen($value) > $length) {
            return trim(mb_substr($value, 0, $length)) . '…';
        }

        return str_pad($value, $length);
    }

    /**
     * Get ExtraColumn instance from text like "Column", "Column(20)" or "Column(20>)"
     * @param string $text
     * @return ExtraColumn
     */
    public static function fromText(string $text): ExtraColumn
    {
        // If the string contains '=', then it is a custom extraction.
        if (strpos($text, '=') !== false) {
            // New regex: optional column length specifier is allowed.
            if (preg_match('/^([^=]+)=(xpath|regexp):(.+?)(?:#(\d+))?(?:\((\d+)(>?)\))?$/i', $text, $matches)) {
                $name = trim($matches[1]);
                $customMethod = strtolower($matches[2]);
                $customPattern = trim($matches[3]);
                $customGroup = (isset($matches[4]) && $matches[4] !== '') ? (int)$matches[4] : 0;
                if (isset($matches[5]) && $matches[5] !== '') {
                    $length = (int)$matches[5];
                    $truncate = !($matches[6] === '>');
                } else {
                    $length = null;  // No length provided: use default (getLength returns strlen($name) if null)
                    $truncate = true;
                }
                return new ExtraColumn($name, $length, $truncate, $customMethod, $customPattern, $customGroup);
            }
            // If parsing of the custom syntax fails, return a standard column.
            return new ExtraColumn(trim($text), null, true);
        } else {
            $length = null;
            $truncate = true;
            if (preg_match('/^([^(]+)(\((\d+)(>?)\))?$/', $text, $matches)) {
                $name = trim($matches[1]);
                if (isset($matches[3])) {
                    $length = (int)$matches[3];
                    $truncate = !($matches[4] === '>');
                } elseif (isset(self::$defaultColumnSizes[$name])) {
                    $length = self::$defaultColumnSizes[$name];
                }
            } else {
                $name = trim($text);
            }
            return new ExtraColumn($name, $length, $truncate);
        }
    }

    /**
     * Extract value from text using custom method and pattern
     * @param string $text
     * @return string|null
     */
    public function extractValue(string $text): ?string
    {
        if ($this->customMethod === null || $this->customPattern === null) {
            return null;
        }
        if ($this->customMethod === self::CUSTOM_METHOD_REGEXP) {
            // Use the full regex provided by the user (with delimiters and modifiers)
            if (preg_match($this->customPattern, $text, $matches)) {
                $group = $this->customGroup ?? 0;
                return $matches[$group] ?? null;
            }
            return null;
        } elseif ($this->customMethod === self::CUSTOM_METHOD_XPATH) {
            libxml_use_internal_errors(true);
            $doc = new \DOMDocument();
            if (!$doc->loadHTML($text)) {
                return null;
            }
            $xpath = new \DOMXPath($doc);
            $nodes = $xpath->query($this->customPattern);
            if ($nodes && $nodes->length > 0) {
                $index = $this->customGroup ?? 0;
                if ($index < $nodes->length) {
                    return trim($nodes->item($index)->textContent);
                }
            }
            return null;
        }
        return null;
    }

}