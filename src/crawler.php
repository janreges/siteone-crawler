<?php

use Crawler\Initiator;
use Crawler\Manager;
use Crawler\Output\OutputType;
use Crawler\Utils;
use Crawler\Version;

$startTime = microtime(true);

// class loader
define('BASE_DIR', dirname(platformCompatiblePath($_SERVER['PHP_SELF']), 2));
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


function platformCompatiblePath(string $path): string
{
    if (stripos(PHP_OS, 'CYGWIN') !== false) {
        $path = preg_replace('/^([A-Z]):/i', '/cygdrive/\1', $path);
        $path = str_replace('\\', '/', $path);
    }
    return $path;
}