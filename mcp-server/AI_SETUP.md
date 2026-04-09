# AI Setup Guide

Install MCP server:
```bash
cd mcp-server
npm install && npm link
```

## Claude Desktop

**Config:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

Restart Claude (Cmd+Q) and ask: "What dev servers are running?"

## Claude Code

**Project config:** Create `.mcp.json`:
```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

**User config:** Add to `~/.claude/settings.json`:
```json
{
  "enableAllProjectMcpServers": true
}
```

Reload with `/hooks`.

## Cursor

**Config:** `~/.cursor/mcp.json` (same JSON as Claude Desktop)

Restart Cursor.

## VS Code (Cline)

Add MCP server in Cline extension settings:
- Name: `devport`
- Command: `devport-mcp`

## Other MCP Clients

Use the same config structure with `command: "devport-mcp"`.

## Without npm link

Use absolute path (get with `cd mcp-server && pwd`):

```json
{
  "mcpServers": {
    "devport": {
      "command": "node",
      "args": ["/full/path/to/mcp-server/index.js"]
    }
  }
}
```

## Example Queries

- "What dev servers are running?"
- "Kill the process on port 5173"
- "What ports does DevPort monitor?"
- "Is PostgreSQL running?"

## Verification

```bash
devport-mcp    # Should print "DevPort MCP Server running on stdio"
which devport  # Verify CLI is installed
```

Test in AI: Ask "What dev servers are running?"

## Troubleshooting

**Command not found: devport-mcp**
- Run `npm link` in `mcp-server/`
- Or use absolute path in config

**devport: command not found**
```bash
cargo build --release
sudo cp target/release/devport /usr/local/bin/
```

**Tools not showing**
- Restart AI assistant completely
- Check config is valid JSON
- Try absolute path

**Claude Code: Tools not loading**
- Type `/hooks` to reload
- Check `.mcp.json` exists
- Verify `enableAllProjectMcpServers: true` in settings
