<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

use Crawler\Analysis\Manager as AnalysisManager;
use Crawler\Analysis\Analyzer;
use Crawler\Export\Exporter;
use Crawler\Options\Option;
use Crawler\Options\Options;
use Crawler\Options\Type;
use Exception;
use InvalidArgumentException;

class Initiator
{

    /**
     * Command-line arguments for options parsing
     *
     * @var array
     */
    private readonly array $argv;

    /**
     * Absolute path to Crawler class directory where are subfolders Export and Analysis
     * @var string
     */
    private readonly string $crawlerClassDir;

    /**
     * @var Options
     */
    private Options $options;

    /**
     * @var CoreOptions
     */
    private CoreOptions $coreOptions;

    /**
     * @var AnalysisManager
     */
    private AnalysisManager $analysisManager;

    /**
     * @var Exporter[]
     */
    private array $exporters;

    /**
     * Array of all known options for unknown option detection
     * @var string[]
     */
    private array $knownOptions = [];

    /**
     * @throws Exception
     */
    public function __construct(array $argv, string $crawlerClassDir)
    {
        if (!is_dir($crawlerClassDir) || !is_dir($crawlerClassDir . '/Export') || !is_dir($crawlerClassDir . '/Analysis')) {
            throw new InvalidArgumentException("Crawler class directory {$crawlerClassDir} does not exist or does not contain folders Export and Analysis.");
        }

        $this->argv = $argv;
        $this->crawlerClassDir = $crawlerClassDir;

        // init analysis manager
        $this->analysisManager = new AnalysisManager($crawlerClassDir);

        // import options config from crawler and all exporters/analyzers
        $this->setupOptions();

        // handle special options like --help, --version
        $this->handleSpecialOptions();
    }

    /**
     * @return void
     * @throws Exception
     */
    public function validateAndInit(): void
    {
        // set values to all configured options from CLI parameters
        $this->fillAllOptionsValues();

        // check for given unknown options
        $this->checkUnknownOptions();

        // import core crawler options and fill with given parameters
        $this->importCoreOptions();

        // set options to analysis manager and its analyzers
        $this->analysisManager->setOptions($this->options);

        // import all auto-activated exporters thanks to filled CLI parameter(s)
        $this->importExporters();

        // apply system settings
        $this->importSystemSettings();
    }

    /**
     * Setup $this->options with options from Crawler and all founded exporters and analyzers
     * @return void
     */
    private function setupOptions(): void
    {
        $this->options = new Options();

        foreach (CoreOptions::getOptions()->getGroups() as $group) {
            $this->options->addGroup($group);
        }

        $exporterClasses = $this->getClassesOfExporters();
        foreach ($exporterClasses as $exporterClass) {
            /* @var $exporterClass Exporter */
            foreach ($exporterClass::getOptions()->getGroups() as $group) {
                $this->options->addGroup($group);
            }
        }

        $analyzerClasses = $this->analysisManager->getClassesOfAnalyzers();
        foreach ($analyzerClasses as $analyzerClass) {
            /* @var $analyzerClass Analyzer */
            foreach ($analyzerClass::getOptions()->getGroups() as $group) {
                $this->options->addGroup($group);
            }
        }
    }

    /**
     * @return void
     * @throws Exception
     */
    private function fillAllOptionsValues(): void
    {
        $extras = [];
        foreach ($this->options->getGroups() as $group) {
            foreach ($group->options as $option) {
                if (in_array($option->name, $this->knownOptions)) {
                    throw new Exception("Detected duplicated option '{$option->name}' in more exporters/analyzers.");
                } else {
                    $this->knownOptions[] = $option->name;
                }

                if ($option->altName && in_array($option->altName, $this->knownOptions)) {
                    throw new Exception("Detected duplicated option '{$option->altName}' in more exporters/analyzers.");
                } elseif ($option->altName) {
                    $this->knownOptions[] = $option->altName;
                }

                $option->setValueFromArgv($this->argv);

                // set domain for use in file/dir %domain% placeholder
                if ($option->propertyToFill === 'url') {
                    Option::setExtrasDomain(parse_url($option->getValue(), PHP_URL_HOST));
                }
            }
        }
    }

    /**
     * @return void
     * @throws Exception
     */
    private function checkUnknownOptions(): void
    {
        $scriptName = basename($_SERVER['PHP_SELF']);

        $unknownOptions = [];
        foreach ($this->argv as $arg) {
            if (trim($arg) === '' || str_starts_with($arg, '#')) {
                continue; // skip empty arguments and comments
            }
            $argWithoutValue = preg_replace('/\s*=.*/', '', $arg);
            if (!in_array($argWithoutValue, $this->knownOptions) && basename($arg) !== $scriptName) {
                $unknownOptions[] = $arg;
            }
        }

        if ($unknownOptions) {
            throw new Exception("Unknown options: " . implode(', ', $unknownOptions));
        }
    }

    /**
     * @return void
     * @throws Exception
     */
    private function importCoreOptions(): void
    {
        $this->coreOptions = new CoreOptions($this->options);
    }

    private function handleSpecialOptions(): void
    {
        $showVersion = false;
        $showHelp = false;

        foreach ($this->argv as $arg) {
            if ($arg === '--version' || $arg === '-v') {
                $showVersion = true;
            } elseif ($arg === '--help' || $arg === '-h') {
                $showHelp = true;
            }
        }

        if ($showHelp) {
            $this->printHelp();
            exit(2);
        } else if ($showVersion) {
            echo Utils::getColorText("Version: " . Version::CODE, 'blue') . "\n";
            exit(2);
        }
    }

    /**
     * Import all active exporters to $this->exporters based on filled CLI options
     * @return void
     */
    private function importExporters(): void
    {
        $this->exporters = [];

        $exporterClasses = $this->getClassesOfExporters();
        foreach ($exporterClasses as $exporterClass) {
            $exporter = new $exporterClass();
            /** @var Exporter $exporter */
            $exporter->setConfig($this->options);
            if ($exporter->shouldBeActivated()) {
                $this->exporters[] = $exporter;
            }
        }
    }

    private function importSystemSettings(): void
    {
        ini_set('memory_limit', $this->coreOptions->memoryLimit);
    }

    /**
     * @return string[]
     */
    private function getClassesOfExporters(): array
    {
        $classes = [];
        foreach (glob($this->crawlerClassDir . '/Export/*Exporter.php') as $file) {
            $classBaseName = basename($file, '.php');
            if ($classBaseName != 'Exporter' && $classBaseName != 'BaseExporter') {
                $classes[] = 'Crawler\\Export\\' . $classBaseName;
            }
        }
        return $classes;
    }

    public function getCoreOptions(): CoreOptions
    {
        return $this->coreOptions;
    }

    public function getAnalysisManager(): AnalysisManager
    {
        return $this->analysisManager;
    }

    /**
     * @return Exporter[]
     */
    public function getExporters(): array
    {
        return $this->exporters;
    }

    public function printHelp(): void
    {
        echo "\n";
        echo Utils::getColorText("Usage: ./crawler --url=https://mydomain.tld/ [options]", "yellow") . "\n";
        echo Utils::getColorText("Version: " . Version::CODE, "blue") . "\n";
        echo "\n";

        foreach ($this->options->getGroups() as $group) {
            echo Utils::getColorText("{$group->name}:", 'yellow') . "\n";
            echo Utils::getColorText(str_repeat('-', strlen($group->name) + 1), 'yellow') . "\n";
            foreach ($group->options as $option) {
                $nameAndValue = $option->name;
                if ($option->type === Type::INT) {
                    $nameAndValue .= '=<int>';
                } elseif ($option->type === Type::STRING) {
                    $nameAndValue .= '=<val>';
                } elseif ($option->type === Type::FLOAT) {
                    $nameAndValue .= '=<val>';
                } elseif ($option->type === Type::SIZE_M_G) {
                    $nameAndValue .= '=<size>';
                } elseif ($option->type === Type::REGEX) {
                    $nameAndValue .= '=<regex>';
                } elseif ($option->type === Type::EMAIL) {
                    $nameAndValue .= '=<email>';
                } elseif ($option->type === Type::URL) {
                    $nameAndValue .= '=<url>';
                } elseif ($option->type === Type::FILE) {
                    $nameAndValue .= '=<file>';
                } elseif ($option->type === Type::DIR) {
                    $nameAndValue .= '=<dir>';
                } elseif ($option->type === Type::HOST_AND_PORT) {
                    $nameAndValue .= '=<host:port>';
                } elseif ($option->type === Type::REPLACE_CONTENT) {
                    $nameAndValue .= '=<val>';
                } elseif ($option->type === Type::RESOLVE) {
                    $nameAndValue .= '=<domain:port:ip>';
                }

                echo str_pad($nameAndValue, 32) . " " . rtrim($option->description, '. ') . '.';
                if ($option->defaultValue != null) {
                    echo " Default value is `{$option->defaultValue}`.";
                }
                echo "\n";
            }
            echo "\n";
        }

        echo "\n";
        echo "For more detailed descriptions of parameters, see README.md.\n";
        echo "\n";
        echo Utils::getColorText("Created with ", 'gray') . Utils::getColorText('♥', 'red') . Utils::getColorText(" by Ján Regeš (jan.reges@siteone.cz) from www.SiteOne.io (Czech Republic) [11/2023]", 'gray') . "\n";
    }

}