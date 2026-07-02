/**
 * Shared TypeScript types for the Memory management pages.
 *
 * <p>Mirrors the Java DTOs in {@code ai.nubase.mem.dto.*}. Keep field names in sync —
 * Jackson on the backend uses default camelCase serialization, so the JSON shape matches
 * these declarations one-to-one.
 */

export interface MemoryItem {
  id: string;
  userId?: string | null;
  agentId?: string | null;
  runId?: string | null;
  memory: string;
  metadata?: Record<string, unknown> | null;
  actorId?: string | null;
  role?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  /** Only populated by /search responses. */
  score?: number | null;
}

export interface MemoryEvent {
  id?: string | null;
  memory?: string | null;
  /** ADD | UPDATE | DELETE | NONE | SKIPPED */
  event: string;
  /** For UPDATE: previous text. For SKIPPED: human-readable reason. */
  previousMemory?: string | null;
}

export interface MemoryHistoryEntry {
  id: string;
  memoryId: string;
  oldValue?: string | null;
  newValue?: string | null;
  /** ADD | UPDATE | DELETE */
  event: string;
  actorId?: string | null;
  createdAt?: string | null;
}

export interface EntityItem {
  id: string;
  userId?: string | null;
  agentId?: string | null;
  runId?: string | null;
  text: string;
  entityType?: string | null;
  linkedMemoryIds?: string[] | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface PagedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface MemoryStats {
  totalMemories: number;
  totalEntities: number;
  last24h: { add: number; update: number; delete: number };
  topUsers?: Array<{ userId: string; count: number }>;
}

export interface SearchRequest {
  query: string;
  userId?: string;
  agentId?: string;
  runId?: string;
  topK?: number;
  threshold?: number;
  metadataFilters?: Record<string, unknown>;
}

export interface AddMemoryRequest {
  messages: Array<{ role: string; content: string; name?: string }>;
  userId?: string;
  agentId?: string;
  runId?: string;
  metadata?: Record<string, unknown>;
  infer?: boolean;
}
