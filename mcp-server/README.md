# DevPort MCP Server

MCP server for DevPort CLI. Lets AI assistants scan and manage local dev ports.

## Installation

**Requirements:** Node.js 18+, DevPort CLI in PATH

```bash
npm install
npm link
```

## Configuration

```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

See [AI_SETUP.md](AI_SETUP.md) for Claude Desktop, Cursor, VS Code, and others.

## Tools

### `scan_ports`
Returns running processes on monitored ports. Optional: specify additional ports.

### `list_monitored_ports`
Lists all monitored ports with descriptions.

### `kill_process`
Terminates a process by PID.

## Examples

- "What dev servers are running?"
- "Kill the process on port 5173"
- "What ports does DevPort monitor?"

## Environment

- `DEVPORT_BIN`: Path to devport binary (default: `devport`)
