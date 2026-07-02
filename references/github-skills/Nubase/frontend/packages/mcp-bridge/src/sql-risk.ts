export type SqlRisk = 'UNKNOWN' | 'READ' | 'DATA_WRITE' | 'SCHEMA_WRITE' | 'DANGEROUS';

export function classifySql(sql: string | undefined): SqlRisk {
  const statements = splitStatements(sql);
  if (statements.length === 0) return 'UNKNOWN';
  return statements.reduce<SqlRisk>((highest, statement) => maxRisk(highest, classifyStatement(statement)), 'UNKNOWN');
}

export function countStatements(sql: string | undefined): number {
  return splitStatements(sql).length;
}

function classifyStatement(statement: string): SqlRisk {
  const normalized = statement
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/--.*$/gm, ' ')
    .trim()
    .replace(/\s+/g, ' ')
    .toLowerCase();
  if (!normalized) return 'UNKNOWN';
  if (startsWithAny(normalized, ['drop ', 'truncate ', 'reindex ', 'vacuum full', 'cluster '])) return 'DANGEROUS';
  if (/^delete\s+from\s+[^\s;]+\s*$/.test(normalized)) return 'DANGEROUS';
  if (startsWithAny(normalized, ['create ', 'alter ', 'grant ', 'revoke ', 'comment ', 'security label '])) {
    return 'SCHEMA_WRITE';
  }
  if (startsWithAny(normalized, ['insert ', 'update ', 'delete ', 'merge ', 'copy ', 'call '])) return 'DATA_WRITE';
  if (startsWithAny(normalized, ['select ', 'with ', 'show ', 'explain ', 'describe '])) return 'READ';
  return 'UNKNOWN';
}

function splitStatements(sql: string | undefined) {
  if (!sql || !sql.trim()) return [];
  return sql.split(';').map((s) => s.trim()).filter(Boolean);
}

function startsWithAny(value: string, prefixes: string[]) {
  return prefixes.some((prefix) => value.startsWith(prefix));
}

function maxRisk(left: SqlRisk, right: SqlRisk): SqlRisk {
  return severity(left) >= severity(right) ? left : right;
}

function severity(risk: SqlRisk) {
  return {
    UNKNOWN: 0,
    READ: 1,
    DATA_WRITE: 2,
    SCHEMA_WRITE: 3,
    DANGEROUS: 4,
  }[risk];
}
