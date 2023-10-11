<?php

use Crawler\Initiator;
use Crawler\Manager;
use Crawler\Output\OutputType;
use Crawler\Utils;

const VERSION = '2023.10.5';
$startTime = microtime(true);

// class loader
spl_autoload_register(function ($class) {
    $classFile = dirname(platformCompatiblePath($_SERVER['PHP_SELF'])) . '/' . str_replace('\\', '/', $class) . '.php';
    require_once($classFile);
});

// initiator
try {
    $initiator = new Initiator($argv, dirname(platformCompatiblePath($_SERVER['PHP_SELF'])) . '/Crawler');
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
    exit(101);
}

// parse input parameters with internal error handling and output
$options = $initiator->getCoreOptions();
if ($options->noColor) {
    Utils::disableColors();
}

// run crawler
try {
    $manager = new Manager(
        VERSION,
        $startTime,
        $options,
        implode(' ', $argv),
        $initiator->getExporters(),
        $initiator->getAnalyzers(),
        dirname(platformCompatiblePath($_SERVER['PHP_SELF']))
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


function platformCompatiblePath(string $path): string
{
    if (stripos(PHP_OS, 'CYGWIN') !== false) {
        $path = preg_replace('/^([A-Z]):/i', '/cygdrive/\1', $path);
        $path = str_replace('\\', '/', $path);
    }
    return $path;
}