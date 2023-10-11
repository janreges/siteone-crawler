<?php

namespace Crawler\Result;

use Crawler\Utils;

class VisitedUrl
{
    const ERROR_CONNECTION_FAIL = -1;
    const ERROR_TIMEOUT = -2;
    const ERROR_SERVER_RESET = -3;
    const ERROR_SEND_ERROR = -4;

    const TYPE_HTML = 1;
    const TYPE_SCRIPT = 2;
    const TYPE_STYLESHEET = 3;
    const TYPE_IMAGE = 4;
    const TYPE_FONT = 5;
    const TYPE_DOCUMENT = 6;
    const TYPE_JSON = 7;
    const TYPE_OTHER_FILE = 8;

    /**
     * @var string Unique ID hash of this URL
     */
    public readonly string $uqId;

    /**
     * @var string Unique ID hash of the source URL where this URL was found
     */
    public readonly string $sourceUqId;

    /**
     * @var string URL
     */
    public readonly string $url;

    /**
     * @var int HTTP status code of the request
     * Negative values are errors - see self:ERROR_* constants
     */
    public readonly int $statusCode;

    /**
     * Request time in seconds
     * @var float
     */
    public readonly float $requestTime;

    /**
     * Request time formatted as "1.234s"
     * @var string
     */
    public readonly string $requestTimeFormatted;

    /**
     * Size of the response in bytes
     * @var int|null
     */
    public readonly ?int $size;

    /**
     * Size of the response formatted as "1.23 MB"
     * @var string|null
     */
    public readonly ?string $sizeFormatted;

    /**
     * Type of the response - see self::TYPE_* constants
     * @var int|null
     */
    public readonly ?int $type;

    /**
     * Extra data from the response
     * @var array|null
     */
    public readonly ?array $extras;

    /**
     * @param string $uqId
     * @param string $sourceUqId
     * @param string $url
     * @param int $statusCode
     * @param float $requestTime
     * @param int|null $size
     * @param int|null $type
     * @param array|null $extras
     */
    public function __construct(string $uqId, string $sourceUqId, string $url, int $statusCode, float $requestTime, ?int $size, ?int $type, ?array $extras)
    {
        $this->uqId = $uqId;
        $this->sourceUqId = $sourceUqId;
        $this->url = $url;
        $this->statusCode = $statusCode;
        $this->requestTime = $requestTime;
        $this->requestTimeFormatted = number_format($this->requestTime, 3);
        $this->size = $size;
        $this->sizeFormatted = $size !== null ? Utils::getFormattedSize($size) : null;
        $this->type = $type;
        $this->extras = $extras;
    }

}