import { EventEmitter } from 'node:events';
import { stdin, stdout } from 'node:process';

interface JsonRpcRequest {
  jsonrpc: '2.0';
  id?: string | number;
  method: string;
  params?: any;
}

type Handler = (request: JsonRpcRequest) => Promise<unknown> | unknown;

export class McpStdioServer {
  private readonly events = new EventEmitter();
  private buffer = Buffer.alloc(0);

  constructor(private readonly handler: Handler) {}

  start() {
    stdin.on('data', (chunk) => this.onData(chunk));
    stdin.resume();
  }

  private onData(chunk: Buffer) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const next = this.readMessage();
      if (!next) return;
      this.handleMessage(next).catch((error) => {
        this.write({
          jsonrpc: '2.0',
          id: next.id ?? null,
          error: { code: -32603, message: error instanceof Error ? error.message : String(error) },
        });
      });
    }
  }

  private async handleMessage(request: JsonRpcRequest) {
    if (request.id === undefined) {
      await this.handler(request);
      return;
    }
    try {
      const result = await this.handler(request);
      this.write({ jsonrpc: '2.0', id: request.id, result });
    } catch (error) {
      this.write({
        jsonrpc: '2.0',
        id: request.id,
        error: {
          code: -32603,
          message: error instanceof Error ? error.message : String(error),
        },
      });
    }
  }

  private readMessage(): JsonRpcRequest | null {
    // MCP stdio transport frames messages as newline-delimited JSON
    // (one JSON-RPC object per line, no embedded newlines).
    const newlineIndex = this.buffer.indexOf('\n');
    if (newlineIndex === -1) return null;
    const line = this.buffer.subarray(0, newlineIndex).toString('utf8').trim();
    this.buffer = this.buffer.subarray(newlineIndex + 1);
    if (!line) return this.readMessage();
    return JSON.parse(line) as JsonRpcRequest;
  }

  private write(message: unknown) {
    stdout.write(`${JSON.stringify(message)}\n`);
  }
}
