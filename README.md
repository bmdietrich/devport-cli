# DevPort CLI

Terminal UI for managing local development ports and services. Like htop for your dev environment.

## Features

- Scans common dev ports (3000-3003, 4200, 5000-5001, 5173, 8000-8081, databases)
- Smart service detection (Vite, Next.js, Nx, Docker, databases with project names)
- Kill processes from the TUI
- Color-coded interface with real-time refresh
- Custom port support
- **MCP Server** for AI assistant integration (Claude, Cursor, etc.)

## Installation

```bash
# Clone the repository, then:
./install.sh
```

See [INSTALL.md](INSTALL.md) for manual installation and troubleshooting.

## Usage

Simply run:

```bash
devport
```

### CLI Options

```bash
# Run with default ports
devport

# Monitor additional custom ports
devport --ports 9000,9001,7777

# List all monitored ports
devport --list

# Debug mode - see detailed scanning info
devport --scan
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate |
| `d` / `Delete` | Kill process |
| `r` | Refresh |
| `q` / `Esc` | Quit |

### Smart Detection

Automatically identifies:
- **Frameworks**: Vite, Next.js, Nx, Webpack, Express, Flask, Django, FastAPI
- **Project names** from file paths (e.g., "Vite dev server (myapp)")
- **System services**: macOS services, Docker containers
- **Databases**: PostgreSQL, MySQL, Redis, MongoDB (with Docker detection)

## Monitored Ports

| Ports | Service |
|-------|---------|
| 3000-3003, 4200, 5173 | Frontend dev servers |
| 5000-5001, 8000, 8080-8081 | Backend servers |
| 5432, 3306, 6379, 27017 | Databases |
| 9229 | Node debugger |

## Requirements

- macOS (uses `lsof`)
- Rust 1.70+ (for building)
- Node.js 18+ (for MCP server)

## MCP Server

Let AI assistants manage your dev ports. Works with Claude Desktop, Cursor, VS Code (Cline), and other MCP clients.

### Setup

```bash
cd mcp-server
npm install && npm link
```

Add to your AI config (e.g., `.mcp.json` for Claude Code):

```json
{
  "mcpServers": {
    "devport": {
      "command": "devport-mcp"
    }
  }
}
```

See [AI_SETUP.md](mcp-server/AI_SETUP.md) for full configuration.

### Example Queries

- "What dev servers are running?"
- "Kill the process on port 5173"
- "Is PostgreSQL running?"

## Project Structure

```
devport-cli/
├── src/           # Rust TUI source code
├── mcp-server/    # MCP server for AI integration
├── target/        # Build artifacts
└── README.md      # This file
```

## License

MIT
