import { App } from "obsidian";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
} from "./types";
import {
  PARSE_ERROR,
  METHOD_NOT_FOUND,
  INVALID_PARAMS,
  INTERNAL_ERROR,
  PROTOCOL_VERSION,
  SERVER_NAME,
  SERVER_VERSION,
} from "./types";
import { getToolDefinitions, callTool } from "./tools";
import type { VaultTreeSettings } from "../settings";

const WEBSOCKET_PORT = 22365;
const HTTP_PORT = 22366;

export class McpServer {
  private app: App;
  private getSettings: () => VaultTreeSettings;
  private wsServer: WebSocketServer | null = null;
  private httpServer: HttpServer | null = null;
  private initialized = false;

  constructor(app: App, getSettings: () => VaultTreeSettings) {
    this.app = app;
    this.getSettings = getSettings;
  }

  async start(): Promise<void> {
    // WebSocket server for Claude Code CLI
    this.wsServer = new WebSocketServer(this.app, WEBSOCKET_PORT, this.handleRequest.bind(this));
    await this.wsServer.start();

    // HTTP/SSE server for Claude Desktop
    this.httpServer = new HttpServer(this.app, HTTP_PORT, this.handleRequest.bind(this));
    await this.httpServer.start();

    console.log(`MCP server started on WebSocket:${WEBSOCKET_PORT} and HTTP:${HTTP_PORT}`);
  }

  async stop(): Promise<void> {
    if (this.wsServer) {
      await this.wsServer.stop();
      this.wsServer = null;
    }
    if (this.httpServer) {
      await this.httpServer.stop();
      this.httpServer = null;
    }
    this.initialized = false;
  }

  private async handleRequest(input: string): Promise<string | null> {
    let request: JsonRpcRequest;

    try {
      request = JSON.parse(input);
    } catch {
      return JSON.stringify(this.errorResponse(null, PARSE_ERROR, "Parse error"));
    }

    const response = await this.processRequest(request);
    return response ? JSON.stringify(response) : null;
  }

  private async processRequest(request: JsonRpcRequest): Promise<JsonRpcResponse | null> {
    switch (request.method) {
      case "initialize":
        return this.handleInitialize(request);

      case "initialized":
        this.initialized = true;
        return null;

      case "tools/list":
        return this.handleToolsList(request);

      case "tools/call":
        return this.handleToolsCall(request);

      case "ping":
        return this.successResponse(request.id, {});

      default:
        return this.errorResponse(
          request.id,
          METHOD_NOT_FOUND,
          `Method not found: ${request.method}`
        );
    }
  }

  private handleInitialize(request: JsonRpcRequest): JsonRpcResponse {
    return this.successResponse(request.id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: {
        tools: {},
      },
      serverInfo: {
        name: SERVER_NAME,
        version: SERVER_VERSION,
      },
    });
  }

  private handleToolsList(request: JsonRpcRequest): JsonRpcResponse {
    const tools = getToolDefinitions();
    return this.successResponse(request.id, { tools });
  }

  private async handleToolsCall(request: JsonRpcRequest): Promise<JsonRpcResponse> {
    const params = request.params as { name?: string; arguments?: Record<string, unknown> } | undefined;

    if (!params?.name) {
      return this.errorResponse(request.id, INVALID_PARAMS, "Missing tool name");
    }

    const toolName = params.name;
    const toolArgs = params.arguments ?? {};

    try {
      const result = await callTool(this.app, this.getSettings(), toolName, toolArgs);
      return this.successResponse(request.id, result);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      return this.errorResponse(request.id, INTERNAL_ERROR, message);
    }
  }

  private successResponse(id: string | number | null | undefined, result: unknown): JsonRpcResponse {
    return {
      jsonrpc: "2.0",
      id: id ?? null,
      result,
    };
  }

  private errorResponse(
    id: string | number | null | undefined,
    code: number,
    message: string
  ): JsonRpcResponse {
    return {
      jsonrpc: "2.0",
      id: id ?? null,
      error: { code, message },
    };
  }
}

// WebSocket Server implementation using Obsidian's capabilities
class WebSocketServer {
  private app: App;
  private port: number;
  private handler: (input: string) => Promise<string | null>;
  private server: any = null;
  private connections: Set<any> = new Set();

  constructor(app: App, port: number, handler: (input: string) => Promise<string | null>) {
    this.app = app;
    this.port = port;
    this.handler = handler;
  }

  async start(): Promise<void> {
    // In Obsidian, we use Node.js WebSocket server via electron
    try {
      const { WebSocketServer: WsServer } = require("ws");
      this.server = new WsServer({ port: this.port });

      this.server.on("connection", (ws: any) => {
        this.connections.add(ws);

        ws.on("message", async (data: Buffer) => {
          const input = data.toString();
          const response = await this.handler(input);
          if (response) {
            ws.send(response);
          }
        });

        ws.on("close", () => {
          this.connections.delete(ws);
        });

        ws.on("error", (error: Error) => {
          console.error("WebSocket error:", error);
          this.connections.delete(ws);
        });
      });

      this.server.on("error", (error: Error) => {
        console.error("WebSocket server error:", error);
      });
    } catch (error) {
      console.warn("WebSocket server not available:", error);
    }
  }

  async stop(): Promise<void> {
    if (this.server) {
      for (const ws of this.connections) {
        ws.close();
      }
      this.connections.clear();
      this.server.close();
      this.server = null;
    }
  }
}

// HTTP Server for SSE transport (Claude Desktop compatibility)
class HttpServer {
  private app: App;
  private port: number;
  private handler: (input: string) => Promise<string | null>;
  private server: any = null;

  constructor(app: App, port: number, handler: (input: string) => Promise<string | null>) {
    this.app = app;
    this.port = port;
    this.handler = handler;
  }

  async start(): Promise<void> {
    try {
      const http = require("http");

      this.server = http.createServer(async (req: any, res: any) => {
        // CORS headers
        res.setHeader("Access-Control-Allow-Origin", "*");
        res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
        res.setHeader("Access-Control-Allow-Headers", "Content-Type");

        if (req.method === "OPTIONS") {
          res.writeHead(204);
          res.end();
          return;
        }

        if (req.method === "POST") {
          let body = "";
          req.on("data", (chunk: Buffer) => {
            body += chunk.toString();
          });

          req.on("end", async () => {
            const response = await this.handler(body);
            res.setHeader("Content-Type", "application/json");
            res.writeHead(200);
            res.end(response ?? "");
          });
          return;
        }

        // SSE endpoint for streaming
        if (req.url === "/sse" && req.method === "GET") {
          res.setHeader("Content-Type", "text/event-stream");
          res.setHeader("Cache-Control", "no-cache");
          res.setHeader("Connection", "keep-alive");
          res.writeHead(200);

          // Keep connection alive
          const keepAlive = setInterval(() => {
            res.write(": keepalive\n\n");
          }, 30000);

          req.on("close", () => {
            clearInterval(keepAlive);
          });
          return;
        }

        res.writeHead(404);
        res.end("Not found");
      });

      this.server.listen(this.port);
    } catch (error) {
      console.warn("HTTP server not available:", error);
    }
  }

  async stop(): Promise<void> {
    if (this.server) {
      this.server.close();
      this.server = null;
    }
  }
}
