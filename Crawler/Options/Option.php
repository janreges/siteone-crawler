<?php

namespace Crawler\Options;

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
     * @param string $name
     * @param string|null $altName
     * @param string $propertyToFill
     * @param Type $type
     * @param bool $isArray
     * @param string $description
     * @param mixed|null $defaultValue
     * @param bool $isNullable
     * @param bool $callableMultipleTimes
     */
    public function __construct(string $name, ?string $altName, string $propertyToFill, Type $type, bool $isArray, string $description, mixed $defaultValue = null, bool $isNullable = true, bool $callableMultipleTimes = false)
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
                    if (str_contains($argValue, ',')) {
                        $value = preg_split('/\s*,\s*/', $argValue);
                    } else {
                        $value[] = $argValue;
                    }
                } else {
                    $value = $argValue;
                }
            }
        }

        // remove quotes from value
        if (is_string($value) && str_starts_with($value, '"') && str_ends_with($value, '"')) {
            $value = substr($value, 1, -1);
        } else if (is_string($value) && str_starts_with($value, "'") && str_ends_with($value, "'")) {
            $value = substr($value, 1, -1);
        }

        // convert to array if needed
        if ($this->isArray && is_string($value)) {
            $value = preg_split('/\s*,\s*/', $value);
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
        if ($value === null && $this->isNullable) {
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
        } else if ($this->type === Type::REGEX && @preg_match($value, null) === false) {
            throw new Exception("Option {$this->name} ({$value}) must be valid PCRE regular expression");
        } else if ($this->type === Type::URL && !filter_var($value, FILTER_VALIDATE_URL)) {
            throw new Exception("Option {$this->name} ({$value}) must be valid URL");
        } else if ($this->type === Type::EMAIL && !filter_var($value, FILTER_VALIDATE_EMAIL)) {
            throw new Exception("Option {$this->name} ({$value}) must be valid email '{$value}'");
        } else if ($this->type === Type::FILE && !is_writable(dirname($value)) && !is_writable($value)) {
            throw new Exception("Option {$this->name} ({$value}) must be valid writable file. Check permissions.");
        } else if ($this->type === Type::DIR && !is_dir($value)) {
            throw new Exception("Option {$this->name} ({$value}) must be valid and existing directory");
        }
    }

    /**
     * @param mixed $value
     * @return string|int|bool|float
     * @throws Exception
     */
    private function correctValueType(mixed $value): string|int|bool|float
    {
        if ($this->type === Type::INT) {
            return (int)$value;
        } else if ($this->type === Type::FLOAT) {
            return (float)$value;
        } else if ($this->type === Type::BOOL) {
            return in_array($value, ['1', 'yes', 'true']);
        } else if ($this->type === Type::STRING || $this->type === Type::SIZE_M_G) {
            return (string)$value;
        } else if ($this->type === Type::REGEX) {
            return (string)$value;
        } else if ($this->type === Type::URL) {
            return (string)$value;
        } else if ($this->type === Type::EMAIL) {
            return (string)$value;
        } else if ($this->type === Type::FILE) {
            return (string)$value;
        } else if ($this->type === Type::DIR) {
            return (string)$value;
        } else {
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
            $result[$key] = $this->correctValueType($value2);
        }
        return $result;
    }

}