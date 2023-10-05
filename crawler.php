<?php

use Crawler\Manager;
use Crawler\Output\OutputType;
use Crawler\Utils;
use Crawler\Options;

spl_autoload_register(function ($class) {
    require_once(__DIR__ . '/' . str_replace('\\', '/', $class) . '.php');
});

const VERSION = '2023.10.2';
$startTime = microtime(true);

// parse input parameters with internal error handling and output
$options = Options::parse($argv);

// run crawler
try {
    $manager = new Manager(VERSION, $startTime, $options, implode(' ', $argv));
    $manager->run();
    exit(0);
} catch (\Exception $e) {
    if ($options->outputType === OutputType::FORMATTED_TEXT) {
        Utils::getColorText("ERROR: Unable to run crawler: {$e->getMessage()}", 'red');
    } elseif ($options->outputType === OutputType::JSON) {
        echo json_encode(['error' => $e->getMessage()], JSON_PRETTY_PRINT | JSON_INVALID_UTF8_IGNORE);
    } else {
        echo "ERROR: Unable to run crawler: {$e->getMessage()}\n";
    }
    exit(1);
}

