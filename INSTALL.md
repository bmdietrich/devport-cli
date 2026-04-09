# Installation Guide

## Quick Install

```bash
./install.sh
```

The installer builds the CLI, installs MCP dependencies, and guides you through configuration.

## Manual Installation

### CLI

**Requirements:** macOS, Rust 1.70+

```bash
cargo build --release
sudo cp target/release/devport /usr/local/bin/
devport --version
```

### MCP Server

**Requirements:** Node.js 18+

```bash
cd mcp-server
npm install
npm link  # Global install
```

**Configuration** (`.mcp.json` or Claude Desktop config):
```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

**Without npm link**, use absolute path:
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

## Configuration

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

Restart Claude Desktop (Cmd+Q) and ask: "What dev servers are running?"

### Claude Code

Create `.mcp.json` in your project:
```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

Add to `~/.claude/settings.json`:
```json
{
  "enableAllProjectMcpServers": true
}
```

Reload with `/hooks`.

## Verification

```bash
devport --list              # List monitored ports
devport-mcp                 # Should print "DevPort MCP Server running on stdio"
```

In Claude, ask: "What dev servers are running?"

## Troubleshooting

**Command not found: devport**
```bash
sudo cp target/release/devport /usr/local/bin/
```

**Command not found: devport-mcp**
- Run `npm link` in `mcp-server/`
- Or use absolute path in config (run `pwd` in `mcp-server/`)

**Tools not showing in Claude**
- Restart Claude completely (Cmd+Q)
- Check config file location
- Verify `node -v` is 18+

**Claude Code MCP not loading**
- Type `/hooks` to reload
- Check `.mcp.json` exists in project root
- Verify `enableAllProjectMcpServers: true` in settings
