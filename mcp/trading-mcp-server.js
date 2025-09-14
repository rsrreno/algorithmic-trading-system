#!/usr/bin/env node

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import sqlite3 from 'sqlite3';
import { exec } from 'child_process';
import { promisify } from 'util';
import path from 'path';

const execAsync = promisify(exec);

// Initialize SQLite database connection
const dbPath = process.env.DB_PATH || '/home/bill/bot/trading-system/data/trading.db';
const projectRoot = '/home/bill/bot/trading-system';

class TradingMCPServer {
  constructor() {
    this.server = new Server(
      {
        name: 'trading-system-mcp',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.setupTools();
  }

  setupTools() {
    // Docker command tool
    this.server.setRequestHandler('tools/list', async () => ({
      tools: [
        {
          name: 'docker_command',
          description: 'Execute docker commands in the trading system directory',
          inputSchema: z.object({
            command: z.string().describe('Docker command to execute (e.g., "compose up --build -d")'),
          }),
        },
        {
          name: 'docker_logs',
          description: 'Get docker compose logs',
          inputSchema: z.object({
            service: z.string().optional().describe('Service name (optional)'),
            lines: z.number().default(50).describe('Number of lines to show'),
          }),
        },
        {
          name: 'sqlite_query',
          description: 'Query the trading SQLite database',
          inputSchema: z.object({
            query: z.string().describe('SQL query to execute'),
          }),
        },
        {
          name: 'rebuild_and_deploy',
          description: 'Rebuild and deploy the trading system',
          inputSchema: z.object({
            detached: z.boolean().default(true).describe('Run in detached mode'),
          }),
        },
      ],
    }));

    // Handle tool calls
    this.server.setRequestHandler('tools/call', async (request) => {
      const { name, arguments: args } = request.params;

      switch (name) {
        case 'docker_command':
          return this.executeDockerCommand(args);
        
        case 'docker_logs':
          return this.getDockerLogs(args);
        
        case 'sqlite_query':
          return this.executeSQLQuery(args);
        
        case 'rebuild_and_deploy':
          return this.rebuildAndDeploy(args);
        
        default:
          throw new Error(`Unknown tool: ${name}`);
      }
    });
  }

  async executeDockerCommand(args) {
    try {
      const { stdout, stderr } = await execAsync(`docker ${args.command}`, {
        cwd: projectRoot,
      });
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              success: true,
              stdout: stdout || '',
              stderr: stderr || '',
              command: `docker ${args.command}`,
            }, null, 2),
          },
        ],
      };
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              success: false,
              error: error.message,
              stdout: error.stdout || '',
              stderr: error.stderr || '',
              command: `docker ${args.command}`,
            }, null, 2),
          },
        ],
      };
    }
  }

  async getDockerLogs(args) {
    try {
      const service = args.service || '';
      const lines = args.lines || 50;
      const { stdout, stderr } = await execAsync(
        `docker compose logs --tail ${lines} ${service}`,
        { cwd: projectRoot }
      );
      
      return {
        content: [
          {
            type: 'text',
            text: stdout || stderr || 'No logs available',
          },
        ],
      };
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: `Error getting logs: ${error.message}`,
          },
        ],
      };
    }
  }

  async executeSQLQuery(args) {
    return new Promise((resolve) => {
      const db = new sqlite3.Database(dbPath, sqlite3.OPEN_READONLY);
      
      db.all(args.query, (err, rows) => {
        db.close();
        
        if (err) {
          resolve({
            content: [
              {
                type: 'text',
                text: `SQL Error: ${err.message}`,
              },
            ],
          });
        } else {
          resolve({
            content: [
              {
                type: 'text',
                text: JSON.stringify(rows, null, 2),
              },
            ],
          });
        }
      });
    });
  }

  async rebuildAndDeploy(args) {
    try {
      const detached = args.detached ? '-d' : '';
      
      // Stop existing containers
      await execAsync('docker compose down', { cwd: projectRoot });
      
      // Rebuild and start
      const { stdout, stderr } = await execAsync(
        `docker compose up --build ${detached}`,
        { cwd: projectRoot }
      );
      
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              success: true,
              message: 'Trading system rebuilt and deployed',
              stdout: stdout || '',
              stderr: stderr || '',
            }, null, 2),
          },
        ],
      };
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: JSON.stringify({
              success: false,
              error: error.message,
              stdout: error.stdout || '',
              stderr: error.stderr || '',
            }, null, 2),
          },
        ],
      };
    }
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
  }
}

const server = new TradingMCPServer();
server.run();
