<?php
/**
 * JsonRpcHandler Test
 * 
 * Unit test for the JsonRpcHandler class.
 */
declare(strict_types=1);

namespace SiteOne\Tests\Unit;

use PHPUnit\Framework\TestCase;
use SiteOne\Mcp\JsonRpcHandler;

class JsonRpcHandlerTest extends TestCase
{
    /**
     * @var JsonRpcHandler
     */
    private JsonRpcHandler $handler;
    
    /**
     * Set up the test case
     */
    protected function setUp(): void
    {
        $this->handler = new JsonRpcHandler();
    }
    
    /**
     * Test that parseRequest correctly parses a valid JSON-RPC request
     */
    public function testParseRequestWithValidRequest(): void
    {
        $json = json_encode([
            'jsonrpc' => '2.0',
            'method' => 'test.method',
            'params' => ['param1' => 'value1'],
            'id' => '123'
        ]);
        
        $request = $this->handler->parseRequest($json);
        
        $this->assertEquals('2.0', $request['jsonrpc']);
        $this->assertEquals('test.method', $request['method']);
        $this->assertEquals(['param1' => 'value1'], $request['params']);
        $this->assertEquals('123', $request['id']);
    }
    
    /**
     * Test that parseRequest throws an exception when given invalid JSON
     */
    public function testParseRequestWithInvalidJson(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionCode(-32700);
        
        $this->handler->parseRequest('invalid json');
    }
    
    /**
     * Test that parseRequest throws an exception when given a request without jsonrpc property
     */
    public function testParseRequestWithMissingJsonrpc(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionCode(-32600);
        
        $json = json_encode([
            'method' => 'test.method',
            'params' => ['param1' => 'value1'],
            'id' => '123'
        ]);
        
        $this->handler->parseRequest($json);
    }
    
    /**
     * Test that parseRequest throws an exception when given a request without method property
     */
    public function testParseRequestWithMissingMethod(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionCode(-32600);
        
        $json = json_encode([
            'jsonrpc' => '2.0',
            'params' => ['param1' => 'value1'],
            'id' => '123'
        ]);
        
        $this->handler->parseRequest($json);
    }
    
    /**
     * Test that parseRequest throws an exception when given a request with invalid params
     */
    public function testParseRequestWithInvalidParams(): void
    {
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionCode(-32600);
        
        $json = json_encode([
            'jsonrpc' => '2.0',
            'method' => 'test.method',
            'params' => 'invalid params', // Should be an array or object
            'id' => '123'
        ]);
        
        $this->handler->parseRequest($json);
    }
    
    /**
     * Test that createResponse creates a valid JSON-RPC response
     */
    public function testCreateResponse(): void
    {
        $response = $this->handler->createResponse('123', ['result' => 'value']);
        $decodedResponse = json_decode($response, true);
        
        $this->assertEquals('2.0', $decodedResponse['jsonrpc']);
        $this->assertEquals(['result' => 'value'], $decodedResponse['result']);
        $this->assertEquals('123', $decodedResponse['id']);
    }
    
    /**
     * Test that createErrorResponse creates a valid JSON-RPC error response
     */
    public function testCreateErrorResponse(): void
    {
        $response = $this->handler->createErrorResponse('123', -32600, 'Invalid Request');
        $decodedResponse = json_decode($response, true);
        
        $this->assertEquals('2.0', $decodedResponse['jsonrpc']);
        $this->assertEquals(-32600, $decodedResponse['error']['code']);
        $this->assertEquals('Invalid Request', $decodedResponse['error']['message']);
        $this->assertEquals('123', $decodedResponse['id']);
    }
    
    /**
     * Test that createErrorResponse includes data when provided
     */
    public function testCreateErrorResponseWithData(): void
    {
        $response = $this->handler->createErrorResponse('123', -32600, 'Invalid Request', ['detail' => 'error detail']);
        $decodedResponse = json_decode($response, true);
        
        $this->assertEquals('2.0', $decodedResponse['jsonrpc']);
        $this->assertEquals(-32600, $decodedResponse['error']['code']);
        $this->assertEquals('Invalid Request', $decodedResponse['error']['message']);
        $this->assertEquals(['detail' => 'error detail'], $decodedResponse['error']['data']);
        $this->assertEquals('123', $decodedResponse['id']);
    }
} 