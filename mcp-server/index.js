#!/usr/bin/env node

/**
 * DevPort MCP Server
 *
 * Model Context Protocol server for DevPort CLI.
 * Allows AI assistants to scan, monitor, and manage local development ports.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

const DEVPORT_BIN = process.env.DEVPORT_BIN || 'devport';

/**
 * Execute devport command and return parsed JSON output
 */
async function executeDevport(args) {
  try {
    const { stdout, stderr } = await execAsync(`${DEVPORT_BIN} ${args}`);
    if (stderr) {
      console.error('DevPort stderr:', stderr);
    }
    return stdout.trim();
  } catch (error) {
    throw new Error(`DevPort command failed: ${error.message}`);
  }
}

/**
 * Parse devport --scan output into structured data
 */
function parseScanOutput(output) {
  const processes = [];
  const lines = output.split('\n');

  let currentProcess = null;

  for (const line of lines) {
    if (line.startsWith('Port:')) {
      if (currentProcess) {
        processes.push(currentProcess);
      }
      currentProcess = {
        port: parseInt(line.split(':')[1].trim()),
      };
    } else if (line.includes('Description:') && currentProcess) {
      currentProcess.description = line.split('Description:')[1].trim();
    } else if (line.includes('PID:') && currentProcess) {
      currentProcess.pid = parseInt(line.split('PID:')[1].trim());
    } else if (line.includes('Process:') && currentProcess) {
      currentProcess.process = line.split('Process:')[1].trim();
    } else if (line.includes('Command:') && currentProcess) {
      currentProcess.command = line.split('Command:')[1].trim();
    }
  }

  if (currentProcess) {
    processes.push(currentProcess);
  }

  return processes;
}

/**
 * Create and configure the MCP server
 */
const server = new Server(
  {
    name: 'devport-server',
    version: '1.0.0',
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

/**
 * List available tools
 */
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: 'scan_ports',
        description: 'Scan all monitored development ports and return running processes',
        inputSchema: {
          type: 'object',
          properties: {
            ports: {
              type: 'array',
              items: { type: 'number' },
              description: 'Optional additional ports to scan',
            },
          },
        },
      },
      {
        name: 'list_monitored_ports',
        description: 'List all ports that DevPort monitors by default',
        inputSchema: {
          type: 'object',
          properties: {},
        },
      },
      {
        name: 'kill_process',
        description: 'Kill a process by PID (requires user confirmation in most contexts)',
        inputSchema: {
          type: 'object',
          properties: {
            pid: {
              type: 'number',
              description: 'Process ID to kill',
            },
          },
          required: ['pid'],
        },
      },
    ],
  };
});

/**
 * Handle tool execution
 */
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case 'scan_ports': {
        const portsArg = args?.ports?.length
          ? `--ports ${args.ports.join(',')}`
          : '';
        const output = await executeDevport(`--scan ${portsArg} 2>/dev/null`);
        const processes = parseScanOutput(output);

        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(processes, null, 2),
            },
          ],
        };
      }

      case 'list_monitored_ports': {
        const output = await executeDevport('--list');
        return {
          content: [
            {
              type: 'text',
              text: output,
            },
          ],
        };
      }

      case 'kill_process': {
        if (!args?.pid) {
          throw new Error('PID is required');
        }

        // Use kill command directly (devport CLI doesn't expose kill via flags)
        try {
          await execAsync(`kill -TERM ${args.pid}`);
          return {
            content: [
              {
                type: 'text',
                text: `Successfully sent SIGTERM to process ${args.pid}`,
              },
            ],
          };
        } catch (error) {
          throw new Error(`Failed to kill process ${args.pid}: ${error.message}`);
        }
      }

      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  } catch (error) {
    return {
      content: [
        {
          type: 'text',
          text: `Error: ${error.message}`,
        },
      ],
      isError: true,
    };
  }
});

/**
 * Start the server
 */
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error('DevPort MCP Server running on stdio');
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
