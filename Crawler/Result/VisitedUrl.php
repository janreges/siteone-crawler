<?php

namespace Crawler\Result;

use Crawler\Utils;

class VisitedUrl
{
    const ERROR_CONNECTION_FAIL = -1;
    const ERROR_TIMEOUT = -2;
    const ERROR_SERVER_RESET = -3;
    const ERROR_SEND_ERROR = -4;

    /**
     * @var string Unique ID hash of this URL
     */
    public readonly string $uqId;

    /**
     * @var string Unique ID hash of the source URL where this URL was found
     */
    public readonly string $sourceUqId;

    /**
     * Full URL with scheme, domain, path and query
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
     * Content type ID
     * @see Crawler::CONTENT_TYPE_ID_*
     * @var int
     */
    public readonly int $contentType;

    /**
     * Extra data from the response
     * @var array|null
     */
    public readonly ?array $extras;

    /**
     * @var bool
     */
    public readonly bool $isExternal;

    /**
     * @var bool
     */
    public readonly bool $isAllowedForCrawling;

    /**
     * @param string $uqId
     * @param string $sourceUqId
     * @param string $url
     * @param int $statusCode
     * @param float $requestTime
     * @param int|null $size
     * @param int $contentType
     * @param array|null $extras
     * @param bool $isExternal
     * @param bool $isAllowedForCrawling
     */
    public function __construct(string $uqId, string $sourceUqId, string $url, int $statusCode, float $requestTime, ?int $size, int $contentType, ?array $extras, bool $isExternal, bool $isAllowedForCrawling)
    {
        $this->uqId = $uqId;
        $this->sourceUqId = $sourceUqId;
        $this->url = $url;
        $this->statusCode = $statusCode;
        $this->requestTime = $requestTime;
        $this->requestTimeFormatted = number_format($this->requestTime, 3);
        $this->size = $size;
        $this->sizeFormatted = $size !== null ? Utils::getFormattedSize($size) : null;
        $this->contentType = $contentType;
        $this->extras = $extras;
        $this->isExternal = $isExternal;
        $this->isAllowedForCrawling = $isAllowedForCrawling;
    }

}