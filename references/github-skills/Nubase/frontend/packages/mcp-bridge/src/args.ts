// Shared argument validation for the CLI command modules, the MCP tool
// dispatch, and the HTTP client. One copy keeps the error message format
// ("<name> is required") identical everywhere.

// Required positional CLI argument (already parsed to string | undefined).
export function required(value: string | undefined, name: string): string {
  if (!value) throw new Error(`${name} is required`);
  return value;
}

// Required non-blank string from untyped tool/client args.
export function requiredString(value: unknown, name: string): string {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${name} is required`);
  return value;
}

// Required plain object (not null/array) from untyped tool args.
export function requiredObject(value: unknown, name: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${name} object is required`);
  return value as Record<string, unknown>;
}

// Strict integer flag parsing shared by all CLI namespaces. `Number('600s')`
// is NaN, which survives a `typeof x === 'number'` filter and serializes to
// null — the server then silently applies its default. Reject loudly instead.
export function intOption(
  options: Record<string, string | boolean>,
  name: string,
  { min, max }: { min?: number; max?: number } = {}
): number | undefined {
  const value = options[name];
  if (value === undefined) return undefined;
  const parsed = typeof value === 'string' && /^-?\d+$/.test(value.trim()) ? Number(value.trim()) : NaN;
  if (!Number.isSafeInteger(parsed)) throw new Error(`--${name} must be an integer`);
  if (min !== undefined && parsed < min) throw new Error(`--${name} must be an integer >= ${min}`);
  if (max !== undefined && parsed > max) throw new Error(`--${name} must be an integer <= ${max}`);
  return parsed;
}
