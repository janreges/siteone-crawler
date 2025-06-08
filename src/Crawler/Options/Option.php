<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Options;

use Crawler\Utils;
use Exception;

class Option
{

    /**
     * Option name with '--' prefix, for example "--user-agent"
     * @var string
     */
    public readonly string $name;

    /**
     * Optional alternative (short) name with '-', for example "-ua" for "--user-agent"
     * @var string|null
     */
    public readonly ?string $altName;

    /**
     * Property name to fill in Options or Exporter/Analyzer class
     * @var string
     */
    public readonly string $propertyToFill;

    /**
     * Option value type
     * @var Type
     */
    public readonly Type $type;

    /**
     * Is array of comma delimited values
     * @var bool
     */
    public readonly bool $isArray;

    /**
     * Description for help
     * @var string
     */
    public readonly string $description;

    /**
     * Default value
     * @var mixed
     */
    public readonly mixed $defaultValue;

    /**
     * @var bool
     */
    public readonly bool $isNullable;

    /**
     * @var bool
     */
    public readonly bool $callableMultipleTimes;

    /**
     * Optional extras
     * @var array
     */
    public readonly ?array $extras;

    /**
     * Value parsed and validate from argv
     * @var mixed
     */
    private mixed $value;

    /**
     * Value is already set from argv
     * @var bool
     */
    private bool $isValueSet = false;

    /**
     * Domain for %domain% replacement in file path
     * @var string|null
     */
    private static ?string $extrasDomain;

    /**
     * @param string $name
     * @param string|null $altName
     * @param string $propertyToFill
     * @param Type $type
     * @param bool $isArray
     * @param string $description
     * @param mixed|null $defaultValue
     * @param bool $isNullable
     * @param bool $callableMultipleTimes
     * @param ?array $extras
     */
    public function __construct(string $name, ?string $altName, string $propertyToFill, Type $type, bool $isArray, string $description, mixed $defaultValue = null, bool $isNullable = true, bool $callableMultipleTimes = false, ?array $extras = null)
    {
        $this->name = $name;
        $this->altName = $altName;
        $this->propertyToFill = $propertyToFill;
        $this->type = $type;
        $this->isArray = $isArray;
        $this->description = $description;
        $this->defaultValue = $defaultValue;
        $this->isNullable = $isNullable;
        $this->callableMultipleTimes = $callableMultipleTimes;
        $this->extras = $extras;
    }

    /**
     * @param array $argv
     * @return void
     * @throws Exception
     */
    public function setValueFromArgv(array $argv): void
    {
        if ($this->isValueSet) {
            throw new Exception("Value for option {$this->name} is already set. Did you call setValueFromArgv() twice?");
        }

        $value = $this->defaultValue;
        $definedByAltName = false;

        // find value in arguments
        foreach ($argv as $arg) {
            $argValue = null;
            if ($arg === $this->name || $arg === $this->altName) {
                $argValue = true;
            } else if (str_starts_with($arg, $this->name . '=')) {
                $argValue = substr($arg, strlen($this->name) + 1);
            } else if ($this->altName && str_starts_with($arg, $this->altName . '=')) {
                $argValue = substr($arg, strlen($this->altName) + 1);
                $definedByAltName = true;
            }
            if ($argValue !== null) {
                if ($this->isArray) {
                    if ($value === null) {
                        $value = [];
                    }
                    if (is_string($argValue)) {
                        $this->unquoteValue($argValue);
                    }
                    if (str_contains($argValue, ',')) {
                        $value = preg_split('/\s*,\s*/', $argValue);
                        $value = array_filter($value, fn($item) => trim($item) !== '');
                        foreach ($value ?: [] as $key => $item) {
                            if (is_string($item)) {
                                $this->unquoteValue($value[$key]);
                            }
                        }
                    } else {
                        $value[] = $argValue;
                    }
                } else {
                    $value = $argValue;
                    if (is_string($value)) {
                        $this->unquoteValue($value);
                    }
                }
            }

            // Set timezone if specified
            if ($this->name === '--timezone' && $argValue !== null) {
                define('CRAWLER_TIMEZONE', $argValue);
            }
        }

        // convert to array if needed
        if ($this->isArray && is_string($value)) {
            $this->unquoteValue($value);
            $value = preg_split('/\s*,\s*/', $value);
            foreach ($value ?: [] as $key => $item) {
                if (is_string($item)) {
                    $this->unquoteValue($value[$key]);
                }
            }
        } elseif ($this->isArray && !is_array($value)) {
            $value = [];
        }

        // validate value(s)
        if ($this->isArray) {
            if (!is_array($value)) {
                throw new Exception("Option " . ($definedByAltName ? $this->altName : $this->name) . " must be array");
            }
            foreach ($value as $item) {
                $this->validateValue($item);
            }
        } else {
            $this->validateValue($value);
        }

        // correct type
        $this->value = $this->isArray ? $this->correctArrayValueType($value) : $this->correctValueType($value);

        // set flag
        $this->isValueSet = true;
    }

    /**
     * @return mixed
     * @throws Exception
     */
    public function getValue(): mixed
    {
        if (!$this->isValueSet) {
            throw new Exception("Value for option {$this->name} is not set. Did you call setValueFromArgv()?");
        }

        return $this->value;
    }

    /**
     * @param mixed $value
     * @return void
     * @throws Exception
     */
    private function validateValue(mixed $value): void
    {
        if ($this->isNullable && ($value === null || $value === '')) {
            return;
        }

        if ($this->type === Type::INT && (!is_numeric($value) || $value < 0)) {
            throw new Exception("Option {$this->name} ({$value}) must be positive integer");
        } else if ($this->type === Type::FLOAT && !is_numeric($value)) {
            throw new Exception("Option {$this->name} ({$value}) must be float");
        } else if ($this->type === Type::BOOL && !in_array($value, ['1', '0', 'yes', 'no', 'true', 'false'])) {
            throw new Exception("Option {$this->name} ({$value}) must be boolean (1/0, yes/no, true/false)");
        } else if ($this->type === Type::STRING && !is_string($value)) {
            throw new Exception("Option {$this->name} ({$value}) must be string");
        } else if ($this->type === Type::SIZE_M_G && (!is_string($value) || !preg_match('/^\d+(\.\d+)?[MG]$/', $value))) {
            throw new Exception("Option {$this->name} ({$value}) must be string with M/G suffix (for example 512M or 1.5G)");
        } else if ($this->type === Type::REGEX && @preg_match($value, '') === false) {
            throw new Exception("Option {$this->name} ({$value}) must be valid PCRE regular expression");
        } else if ($this->type === Type::URL) {
            $value = is_string($value) ? $this->correctUrl($value) : null;
            // Check if value is null or empty
            if ($value === null || $value === '') {
                throw new Exception("Option {$this->name} must be valid URL (starting with http:// or https://)");
            }
            // First try standard validation
            if (!filter_var($value, FILTER_VALIDATE_URL)) {
                // If that fails, try with URL-encoded version for URLs with international characters
                $encodedUrl = preg_replace_callback(
                    '/[^\x20-\x7E]/',
                    function ($matches) {
                        return rawurlencode($matches[0]);
                    },
                    $value
                );
                if (!filter_var($encodedUrl, FILTER_VALIDATE_URL)) {
                    throw new Exception("Option {$this->name} ({$value}) must be valid URL");
                }
            }
        } else if ($this->type === Type::EMAIL && !filter_var($value, FILTER_VALIDATE_EMAIL)) {
            throw new Exception("Option {$this->name} ({$value}) must be valid email '{$value}'");
        } else if ($this->type === Type::FILE) {
            $this->replacePlaceholders($value);
            $value = Utils::getAbsolutePath($value);
            if (!is_writable(dirname($value)) && !is_writable($value)) {
                throw new Exception("Option {$this->name} ({$value}) must be valid writable file. Check permissions.");
            }
        } else if ($this->type === Type::DIR && $value !== 'off') {
            $this->replacePlaceholders($value);
            $value = Utils::getAbsolutePath($value);
            if (!is_string($value) || trim($value) === '') {
                throw new Exception("Option {$this->name} ({$value}) must be string");
            }
            if ((!is_dir($value) || !is_writable(dirname($value))) && mkdir($value, 0777, true) === false) {
                throw new Exception("Option {$this->name} ({$value}) must be valid and writable directory. Check permissions.");
            }
        } else if ($this->type === Type::HOST_AND_PORT && (!is_string($value) || !preg_match('/^[a-z0-9\-.:]{1,100}:[0-9]{1,5}$/i', $value))) {
            throw new Exception("Option {$this->name} ({$value}) must be in format host:port");
        } else if ($this->type === Type::REPLACE_CONTENT) {
            if (!is_string($value) || !preg_match('/^.+->/i', $value)) {
                throw new Exception("Option {$this->name} ({$value}) must be in format `foo -> bar` or `/preg-regexp/ -> bar`)");
            }

            $parts = explode('->', $value);
            $replaceFrom = trim($parts[0]);
            $isRegex = preg_match('/^([\/#~%]).*\1[a-z]*$/i', $replaceFrom);

            if ($isRegex && @preg_match($replaceFrom, '') === false) {
                throw new Exception("Option {$this->name} and its first part ({$replaceFrom}) must be valid PCRE regular expression");
            }
        } else if ($this->type === Type::RESOLVE) {
            // --resolve is in the same format as curl --resolve (ipv4 and ipv6 supported)
            if (!is_string($value) || !preg_match('/^[a-z0-9\-.]{1,200}:[0-9]{1,5}:[a-f0-9\-.:]{1,100}$/i', $value)) {
                throw new Exception("Option {$this->name} ({$value}) must be in format `domain:port:ip`");
            }
        }

        // extra validations
        $isNumber = $this->type === Type::INT || $this->type === Type::FLOAT;
        if ($isNumber && $this->extras && count($this->extras) === 2) {
            if ($value < $this->extras[0] || $value > $this->extras[1]) {
                throw new Exception("Option {$this->name} ({$value}) must be in range {$this->extras[0]}-{$this->extras[1]}");
            }
        }
    }

    /**
     * @param mixed $value
     * @return string|int|bool|float|null
     *
     * @throws Exception
     */
    private function correctValueType(mixed $value): string|int|bool|float|null
    {
        if ($this->isNullable && ($value === null || $value === '')) {
            return null;
        }

        if ($this->type === Type::INT) {
            return (int)$value;
        } else if ($this->type === Type::FLOAT) {
            return (float)$value;
        } else if ($this->type === Type::BOOL) {
            return in_array($value, ['1', 'yes', 'true', true], true);
        } else if ($this->type === Type::STRING || $this->type === Type::SIZE_M_G) {
            return (string)$value;
        } else if ($this->type === Type::REGEX) {
            return (string)$value;
        } else if ($this->type === Type::URL) {
            return is_string($value) ? $this->correctUrl($value) : null;
        } else if ($this->type === Type::EMAIL) {
            return (string)$value;
        } else if ($this->type === Type::FILE) {
            $value = (string)$value;
            $this->replacePlaceholders($value);
            return Utils::getAbsolutePath($value);
        } else if ($this->type === Type::DIR) {
            $value = (string)$value;
            if ($value === 'off') {
                return $value;
            }
            $this->replacePlaceholders($value);
            return Utils::getAbsolutePath($value);
        } else if ($this->type === Type::HOST_AND_PORT) {
            return (string)$value;
        } else if ($this->type === Type::REPLACE_CONTENT) {
            return (string)$value;
        } else if ($this->type === Type::RESOLVE) {
            return (string)$value;
        } /* @phpstan-ignore-line */ else {
            throw new Exception("Unknown type {$this->type}");
        }
    }

    /**
     * @param array $value
     * @return array
     * @throws Exception
     */
    private function correctArrayValueType(array $value): array
    {
        $result = $value;
        foreach ($result as $key => $value2) {
            // ignore empty values
            if (trim($value2) === '') {
                unset($result[$key]);
                continue;
            }
            $result[$key] = $this->correctValueType($value2);
        }
        return $result;
    }

    private function replacePlaceholders(string &$value): void
    {
        static $date = null;
        static $datetime = null;

        // Get the timezone setting if available
        $timezone = null;
        if (defined('CRAWLER_TIMEZONE') && CRAWLER_TIMEZONE) {
            $timezone = CRAWLER_TIMEZONE;
            date_default_timezone_set($timezone);
        }

        if (!$date) {
            $date = date('Y-m-d');
        }
        if (!$datetime) {
            $datetime = date('Ymd-His');
        }

        $value = str_replace(
            ['%domain%', '%date%', '%datetime%'],
            [self::$extrasDomain, $date, $datetime],
            $value
        );
    }

    /**
     * Correct URL to valid URL, e.g. crawler.siteone.io => https://crawler.siteone.io, or localhost to http://localhost)
     * @param string $url
     * @return string
     */
    private function correctUrl(string $url): string
    {
        if (!str_starts_with($url, 'http') && preg_match('/^[a-z0-9\-.:]{1,100}$/i', $url) === 1) {
            // if contains dot, use https, otherwise http (e.g. localhost)
            $defaultProtocol = str_contains($url, '.') ? 'https' : 'http';
            $url = $defaultProtocol . '://' . ltrim($url, '/');
        }

        return $url;
    }

    /**
     * Remove quotes from given string - as an quote we consider chars " ' `
     *
     * @param string $value
     * @return void
     */
    private function unquoteValue(string &$value): void
    {
        if (str_starts_with($value, '"') && str_ends_with($value, '"')) {
            $value = substr($value, 1, -1);
        } else if (str_starts_with($value, "'") && str_ends_with($value, "'")) {
            $value = substr($value, 1, -1);
        } else if (str_starts_with($value, "`") && str_ends_with($value, "`")) {
            $value = substr($value, 1, -1);
        }
    }

    public static function setExtrasDomain(?string $extrasDomain): void
    {
        self::$extrasDomain = $extrasDomain;
    }

}
