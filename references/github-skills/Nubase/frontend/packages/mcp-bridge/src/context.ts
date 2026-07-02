import type { BridgeConfig } from './config.js';

export interface ScopeArgs {
  userId?: string;
  agentId?: string;
  runId?: string;
}

export interface ResolvedScope {
  userId?: string;
  agentId?: string;
  runId?: string;
}

export function resolveScope(config: BridgeConfig, args: ScopeArgs = {}): ResolvedScope {
  return {
    userId: args.userId || config.userId,
    agentId: args.agentId || config.agentId,
    runId: args.runId || config.runId,
  };
}

export function withScope<T extends Record<string, unknown>>(
  config: BridgeConfig,
  args: T & ScopeArgs
): T & ResolvedScope {
  const scope = resolveScope(config, args);
  return Object.fromEntries(
    Object.entries({ ...args, ...scope }).filter(([, value]) => value !== undefined && value !== '')
  ) as T & ResolvedScope;
}
