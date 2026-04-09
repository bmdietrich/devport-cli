#!/bin/bash
# DevPort Complete Installation Script

set -e

echo "🚢 DevPort Installer"
echo "===================="
echo ""

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install from https://rustup.rs/"
    exit 1
fi

# Check for Node.js
if ! command -v node &> /dev/null; then
    echo "⚠️  Node.js not found. MCP server will not be installed."
    SKIP_MCP=true
else
    NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
    if [ "$NODE_VERSION" -lt 18 ]; then
        echo "⚠️  Node.js 18+ required for MCP server. Found: $(node -v)"
        SKIP_MCP=true
    fi
fi

echo "🔨 Building DevPort CLI..."
cargo build --release

echo ""
echo "📦 Installing DevPort CLI to /usr/local/bin..."
sudo cp target/release/devport /usr/local/bin/devport

echo ""
echo "✅ DevPort CLI installed successfully!"
devport --version 2>/dev/null || echo "   Run: devport"

if [ "$SKIP_MCP" != "true" ]; then
    echo ""
    echo "🤖 Installing MCP Server..."
    cd mcp-server
    npm install

    echo ""
    read -p "Install MCP server globally (npm link)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        npm link
        echo ""
        echo "✅ MCP server installed as 'devport-mcp'"
        echo ""
        echo "📝 Add to Claude Desktop config:"
        echo '   {
     "mcpServers": {
       "devport": {
         "command": "devport-mcp"
       }
     }
   }'
    else
        echo ""
        echo "✅ MCP server installed locally"
        echo ""
        echo "📝 Add to Claude Desktop config:"
        echo "   {
     \"mcpServers\": {
       \"devport\": {
         \"command\": \"node\",
         \"args\": [\"$(pwd)/index.js\"]
       }
     }
   }"
    fi
    cd ..
fi

echo ""
echo "🎉 Installation complete!"
echo ""
echo "Usage:"
echo "  devport              # Launch TUI"
echo "  devport --list       # Show monitored ports"
echo "  devport --scan       # Debug mode"
echo "  devport --help       # More options"

if [ "$SKIP_MCP" != "true" ]; then
    echo ""
    echo "Claude Desktop config location:"
    echo "  ~/Library/Application Support/Claude/claude_desktop_config.json"
fi
