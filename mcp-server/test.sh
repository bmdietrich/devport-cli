#!/bin/bash
# Test MCP server manually

echo "Testing DevPort MCP Server..."
echo ""

# Test scan_ports
echo "1. Testing scan_ports tool..."
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scan_ports","arguments":{}}}' | node index.js 2>/dev/null | tail -1

echo ""
echo "2. Testing list_monitored_ports tool..."
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | node index.js 2>/dev/null | tail -1

echo ""
echo "Done! Use MCP Inspector for interactive testing:"
echo "  npx @modelcontextprotocol/inspector node index.js"
