<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

use Crawler\Initiator;
use Crawler\Manager;
use Crawler\Output\OutputType;
use Crawler\Utils;
use Crawler\Version;

$startTime = microtime(true);

// class loader
define('BASE_DIR', getBaseDir());
spl_autoload_register(function ($class) {
    $classFile = BASE_DIR . '/src/' . str_replace('\\', '/', $class) . '.php';
    require_once($classFile);
});

// increase PCRE limits for large HTML/CSS/JS parsing
ini_set('pcre.backtrack_limit', '100000000');
ini_set('pcre.recursion_limit', '10000000');

// initiator
try {
    $initiator = new Initiator($argv, BASE_DIR . '/src/Crawler');
} catch (Exception $e) {
    echo Utils::getColorText("ERROR: Unable to initialize crawler: {$e->getMessage()}", 'red');
    exit(100);
}

// validate input parameters and init analyzers/exporters
try {
    $initiator->validateAndInit();
} catch (Exception $e) {
    echo Utils::getColorText("ERROR: {$e->getMessage()}", 'red');
    $initiator->printHelp();
    echo Utils::getColorText("\nERROR: {$e->getMessage()}\n\n", 'red');
    exit(101);
}

// parse input parameters with internal error handling and output
$options = $initiator->getCoreOptions();
if ($options->noColor) {
    Utils::disableColors();
} elseif ($options->forceColor === true) {
    Utils::forceEnabledColors();
}

// set forced console width if specified (used by Electron GUI app)
if ($options->consoleWidth && $options->consoleWidth > 0) {
    Utils::setForcedConsoleWidth($options->consoleWidth);
}

// run crawler
try {
    $manager = new Manager(
        Version::CODE,
        $startTime,
        $options,
        implode(' ', $argv),
        $initiator->getExporters(),
        $initiator->getAnalysisManager(),
        BASE_DIR
    );

    $manager->run();
    exit(0);
} catch (Exception $e) {
    if ($options->outputType === OutputType::TEXT) {
        Utils::getColorText("ERROR: Unable to run crawler: {$e->getMessage()}", 'red');
    } elseif ($options->outputType === OutputType::JSON) {
        echo json_encode(['error' => $e->getMessage()], JSON_PRETTY_PRINT | JSON_INVALID_UTF8_IGNORE);
    } else {
        echo "ERROR: Unable to run crawler: {$e->getMessage()}\n";
    }
    exit(1);
}

function getBaseDir(): string
{
    if (stripos(PHP_OS, 'CYGWIN') !== false) {
        if (isset($_SERVER['SCRIPT_DIR']) && $_SERVER['SCRIPT_DIR'] && (str_contains($_SERVER['SCRIPT_DIR'], ':/') || str_contains($_SERVER['SCRIPT_DIR'], ':\\'))) {
            return rtrim(platformCompatiblePath($_SERVER['SCRIPT_DIR']), '/ ');
        } else {
            return dirname(platformCompatiblePath($_SERVER['SCRIPT_FILENAME']), 2);
        }
    } else {
        $scriptRealPath = str_starts_with($_SERVER['SCRIPT_FILENAME'], '/') ? $_SERVER['SCRIPT_FILENAME'] : realpath($_SERVER['SCRIPT_FILENAME']);
        return dirname(platformCompatiblePath($scriptRealPath), 2);
    }
}

function platformCompatiblePath(string $path): string
{
    if (stripos(PHP_OS, 'CYGWIN') !== false) {
        // convert drive letter to lowercase - C:/ -> c:/
        if (preg_match('/^([a-z]+):$/i', $path) === 1) {
            $path = lcfirst($path);
        }
        $path = preg_replace('/^([a-z]):/i', '/cygdrive/\1', $path);
        $path = str_replace('\\', '/', $path);
    }
    return $path;
}
