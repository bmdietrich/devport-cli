# Changelog

## v1.0.0 (2026-04-08)

### CLI
- Scan common dev ports (3000-4200, 5000-5173, 8000-8081, databases)
- Smart framework and project detection
- Kill processes via TUI
- Color-coded interface with real-time refresh
- Custom port support

### MCP Server
- AI assistant integration (Claude, Cursor, etc.)
- Tools: `scan_ports`, `list_monitored_ports`, `kill_process`
- Works with any MCP-compatible client

### Technical
- Rust CLI with Ratatui TUI
- Node.js MCP server
- macOS only (uses `lsof`)
