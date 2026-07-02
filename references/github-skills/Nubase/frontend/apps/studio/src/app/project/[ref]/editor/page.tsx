'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Plus,
  RefreshCw,
  Filter,
  Search,
  X,
  Trash2,
  Pencil,
} from 'lucide-react';
import {
  Button,
  Input,
  Label,
  Badge,
  Dialog,
  DialogHeader,
  DialogBody,
  DialogFooter,
  cn,
} from '@nubase/ui';
import { apiFetch, API_BASE, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef } from '@/lib/route-params';

interface TableRef {
  schema: string;
  name: string;
}

interface ColumnInfo {
  name: string;
  type: string;
  nullable: boolean;
  isPk: boolean;
  hasDefault: boolean;
}

interface SqlExecutionResponse {
  success: boolean;
  results?: Array<{ index: number; type: 'query' | 'update'; rows?: Array<Record<string, unknown>> }>;
  execution_time_ms?: number;
  error?: string;
}

interface FilterRule {
  id: string;
  column: string;
  op: string;
  value: string;
}

const OPERATORS: Array<{ value: string; label: string; takesValue: boolean }> = [
  { value: 'eq', label: '=', takesValue: true },
  { value: 'neq', label: '≠', takesValue: true },
  { value: 'gt', label: '>', takesValue: true },
  { value: 'gte', label: '≥', takesValue: true },
  { value: 'lt', label: '<', takesValue: true },
  { value: 'lte', label: '≤', takesValue: true },
  { value: 'like', label: 'like', takesValue: true },
  { value: 'ilike', label: 'ilike', takesValue: true },
  { value: 'in', label: 'in (csv)', takesValue: true },
  { value: 'is', label: 'is (null/true/false)', takesValue: true },
];

const PAGE_SIZE = 50;

const LIST_TABLES_SQL = `
  select c.table_schema, c.table_name
  from information_schema.tables c
  where c.table_schema not in ('pg_catalog','information_schema','pg_toast')
    and c.table_type = 'BASE TABLE'
  order by c.table_schema, c.table_name
`;

const COLUMNS_SQL = (schema: string, table: string) => `
  with pk as (
    select kcu.column_name
    from information_schema.table_constraints tc
    join information_schema.key_column_usage kcu
      on tc.constraint_name = kcu.constraint_name
      and tc.table_schema = kcu.table_schema
    where tc.constraint_type = 'PRIMARY KEY'
      and tc.table_schema = '${schema}'
      and tc.table_name = '${table}'
  )
  select c.column_name as name,
         c.data_type as type,
         (c.is_nullable = 'YES') as nullable,
         (c.column_name in (select column_name from pk)) as is_pk,
         (c.column_default is not null) as has_default
  from information_schema.columns c
  where c.table_schema = '${schema}' and c.table_name = '${table}'
  order by c.ordinal_position
`;

export default function TableEditorPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <TableEditorInner />;
}

function TableEditorInner() {
  const { project } = useSession();
  const apikey = project!.apikey;
  const [tables, setTables] = useState<TableRef[]>([]);
  const [tablesLoading, setTablesLoading] = useState(false);
  const [tablesError, setTablesError] = useState<string | null>(null);
  const [tableQuery, setTableQuery] = useState('');

  const [selected, setSelected] = useState<TableRef | null>(null);
  const [columns, setColumns] = useState<ColumnInfo[]>([]);
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  const [tableLoading, setTableLoading] = useState(false);
  const [tableError, setTableError] = useState<string | null>(null);

  const [filters, setFilters] = useState<FilterRule[]>([]);
  const [filterPanelOpen, setFilterPanelOpen] = useState(false);

  const [editing, setEditing] = useState<
    | { mode: 'insert'; initial: Record<string, unknown> | null }
    | { mode: 'edit'; initial: Record<string, unknown> }
    | null
  >(null);

  const [schemaFilter, setSchemaFilter] = useState<string>('all');
  const [createTableOpen, setCreateTableOpen] = useState(false);

  const runSql = useCallback(
    async (query: string): Promise<Array<Record<string, unknown>>> => {
      const res = await apiFetch<SqlExecutionResponse>('/auth/v1/admin/sql/execute', {
        method: 'POST',
        body: { query },
        apikey,
      });
      if (!res.success) throw new Error(res.error ?? 'Query failed');
      const lastQuery = res.results?.findLast?.((r) => r.type === 'query');
      return lastQuery?.rows ?? [];
    },
    [apikey]
  );

  const loadTables = useCallback(async () => {
    setTablesLoading(true);
    setTablesError(null);
    try {
      const data = await runSql(LIST_TABLES_SQL);
      const list = data.map((r) => ({ schema: String(r.table_schema), name: String(r.table_name) }));
      setTables(list);
      if (list.length > 0 && !selected) {
        const first = list.find((t) => t.schema === 'public') ?? list[0]!;
        setSelected(first);
      }
    } catch (err) {
      setTablesError((err as ApiError | Error).message ?? 'Failed to load tables.');
    } finally {
      setTablesLoading(false);
    }
  }, [runSql, selected]);

  const loadTable = useCallback(
    async (t: TableRef, currentFilters: FilterRule[]) => {
      setTableLoading(true);
      setTableError(null);
      setColumns([]);
      setRows([]);
      try {
        const colsData = await runSql(COLUMNS_SQL(t.schema, t.name));
        const cols: ColumnInfo[] = colsData.map((r) => ({
          name: String(r.name),
          type: String(r.type),
          nullable: Boolean(r.nullable),
          isPk: Boolean(r.is_pk),
          hasDefault: Boolean(r.has_default),
        }));
        setColumns(cols);

        const params = new URLSearchParams();
        params.set('limit', String(PAGE_SIZE));
        for (const f of currentFilters) {
          if (f.column && f.op && f.value !== '') {
            params.append(f.column, `${f.op}.${f.value}`);
          }
        }
        const url = `${API_BASE}/rest/v1/${encodeURIComponent(t.name)}?${params.toString()}`;
        const res = await fetch(url, {
          headers: {
            apikey,
            Authorization: `Bearer ${apikey}`,
            'Accept-Profile': t.schema,
          },
        });
        if (!res.ok) {
          const body = await res.text().catch(() => res.statusText);
          throw new Error(`REST ${res.status}: ${body}`);
        }
        const rowsData = (await res.json()) as Array<Record<string, unknown>>;
        setRows(Array.isArray(rowsData) ? rowsData : []);
      } catch (err) {
        setTableError((err as Error).message ?? 'Failed to load table.');
      } finally {
        setTableLoading(false);
      }
    },
    [apikey, runSql]
  );

  useEffect(() => {
    loadTables();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [apikey]);

  useEffect(() => {
    if (selected) loadTable(selected, filters);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.schema, selected?.name, apikey]);

  const availableSchemas = useMemo(() => {
    const s = new Set<string>();
    for (const t of tables) s.add(t.schema);
    return Array.from(s).sort();
  }, [tables]);

  const grouped = useMemo(() => {
    const out: Record<string, TableRef[]> = {};
    for (const t of tables) {
      const passesSchema = schemaFilter === 'all' || t.schema === schemaFilter;
      if (!passesSchema) continue;
      const passesSearch = tableQuery ? t.name.toLowerCase().includes(tableQuery.toLowerCase()) : true;
      if (!passesSearch) continue;
      const list = out[t.schema] ?? [];
      list.push(t);
      out[t.schema] = list;
    }
    return out;
  }, [tables, tableQuery, schemaFilter]);

  function applyFilters() {
    if (selected) loadTable(selected, filters);
  }
  function clearFilters() {
    setFilters([]);
    if (selected) loadTable(selected, []);
  }

  return (
    <div className="flex h-full">
      <aside className="flex w-64 flex-col border-r border-border">
        <div className="flex items-center justify-between border-b border-border px-3 py-2">
          <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Tables {tables.length > 0 ? `· ${tables.length}` : ''}
          </span>
          <div className="flex gap-0.5">
            <Button size="icon" variant="ghost" aria-label="New table" onClick={() => setCreateTableOpen(true)}>
              <Plus className="h-4 w-4" />
            </Button>
            <Button size="icon" variant="ghost" aria-label="Refresh" onClick={loadTables}>
              <RefreshCw className="h-4 w-4" />
            </Button>
          </div>
        </div>
        <div className="space-y-2 px-3 py-2">
          <select
            value={schemaFilter}
            onChange={(e) => setSchemaFilter(e.target.value)}
            className="flex h-8 w-full rounded-md border border-input bg-transparent px-2 text-xs"
          >
            <option value="all">All schemas</option>
            {availableSchemas.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
          <div className="relative">
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search tables"
              value={tableQuery}
              onChange={(e) => setTableQuery(e.target.value)}
              className="h-8 pl-7 text-xs"
            />
          </div>
        </div>
        <div className="flex-1 overflow-y-auto px-2 pb-3">
          {tablesError ? (
            <p className="px-2 text-xs text-destructive">{tablesError}</p>
          ) : tablesLoading ? (
            <p className="px-2 text-xs text-muted-foreground">Loading…</p>
          ) : Object.keys(grouped).length === 0 ? (
            <p className="px-2 text-xs text-muted-foreground">No tables.</p>
          ) : (
            Object.entries(grouped).map(([schema, list]) => (
              <div key={schema} className="mb-3">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  {schema}
                </div>
                {list.map((t) => {
                  const isSelected = selected?.schema === t.schema && selected?.name === t.name;
                  return (
                    <button
                      key={`${t.schema}.${t.name}`}
                      onClick={() => {
                        setSelected(t);
                        setFilters([]);
                        setFilterPanelOpen(false);
                      }}
                      className={cn(
                        'flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-sm',
                        isSelected ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/60'
                      )}
                    >
                      <span className="truncate">{t.name}</span>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </aside>

      <section className="flex flex-1 flex-col overflow-hidden">
        <header className="flex items-center justify-between border-b border-border px-4 py-2">
          <div className="flex items-center gap-2">
            {selected ? (
              <>
                <span className="text-xs text-muted-foreground">{selected.schema}.</span>
                <h2 className="text-sm font-semibold">{selected.name}</h2>
                <Badge variant="outline">{rows.length} rows</Badge>
              </>
            ) : (
              <h2 className="text-sm font-semibold text-muted-foreground">No table selected</h2>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant={filterPanelOpen || filters.length > 0 ? 'secondary' : 'outline'}
              onClick={() => setFilterPanelOpen((v) => !v)}
              disabled={!selected || columns.length === 0}
            >
              <Filter className="h-3.5 w-3.5" />
              Filter {filters.length > 0 ? `(${filters.length})` : ''}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => selected && loadTable(selected, filters)}
              disabled={!selected}
            >
              <RefreshCw className="h-3.5 w-3.5" /> Refresh
            </Button>
            <Button
              size="sm"
              onClick={() => setEditing({ mode: 'insert', initial: null })}
              disabled={!selected || columns.length === 0}
            >
              <Plus className="h-3.5 w-3.5" /> Insert
            </Button>
          </div>
        </header>

        {filterPanelOpen && selected ? (
          <FilterPanel
            columns={columns}
            filters={filters}
            setFilters={setFilters}
            onApply={applyFilters}
            onClear={clearFilters}
            onClose={() => setFilterPanelOpen(false)}
          />
        ) : null}

        {tableError ? (
          <pre className="m-4 whitespace-pre-wrap rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
            {tableError}
          </pre>
        ) : tableLoading ? (
          <p className="p-4 text-sm text-muted-foreground">Loading table…</p>
        ) : !selected ? (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            Pick a table from the sidebar.
          </div>
        ) : (
          <div className="flex-1 overflow-auto">
            <table className="border-collapse text-sm" style={{ minWidth: '100%' }}>
              <thead className="sticky top-0 z-10 bg-card">
                <tr className="border-b border-border">
                  {columns.map((c) => (
                    <th
                      key={c.name}
                      className="border-r border-border/60 px-3 py-2 text-left align-top last:border-r-0"
                      style={{ minWidth: '180px' }}
                    >
                      <div className="flex items-center gap-1.5">
                        <span className="truncate font-medium" title={c.name}>{c.name}</span>
                        {c.isPk ? (
                          <Badge variant="outline" className="px-1 py-0 text-[9px] leading-tight">
                            PK
                          </Badge>
                        ) : null}
                      </div>
                      <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-muted-foreground">
                        <span className="truncate font-mono" title={c.type}>{c.type}</span>
                        {!c.nullable && !c.isPk ? (
                          <span className="shrink-0">· not null</span>
                        ) : null}
                      </div>
                    </th>
                  ))}
                  <th style={{ minWidth: '60px' }} className="px-2 py-2 align-top" />
                </tr>
              </thead>
              <tbody>
                {rows.length === 0 ? (
                  <tr>
                    <td
                      colSpan={Math.max(1, columns.length + 1)}
                      className="px-3 py-12 text-center text-sm text-muted-foreground"
                    >
                      No rows.
                    </td>
                  </tr>
                ) : (
                  rows.map((row, i) => (
                    <tr
                      key={i}
                      className="border-b border-border/50 hover:bg-accent/30"
                    >
                      {columns.map((c) => {
                        const formatted = formatCell(row[c.name]);
                        return (
                          <td
                            key={c.name}
                            className="max-w-[320px] truncate border-r border-border/40 px-3 py-2 font-mono text-xs last:border-r-0"
                            style={{ minWidth: '180px' }}
                            title={formatted}
                          >
                            {formatted}
                          </td>
                        );
                      })}
                      <td className="px-2 py-1.5">
                        <button
                          onClick={() => setEditing({ mode: 'edit', initial: row })}
                          className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
                          title="Edit row"
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {editing && selected ? (
        <RowEditorDialog
          mode={editing.mode}
          table={selected}
          columns={columns}
          initial={editing.mode === 'edit' ? editing.initial : null}
          apikey={apikey}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            if (selected) loadTable(selected, filters);
          }}
        />
      ) : null}

      <CreateTableDialog
        open={createTableOpen}
        defaultSchema={schemaFilter !== 'all' ? schemaFilter : 'public'}
        runSql={runSql}
        onClose={() => setCreateTableOpen(false)}
        onCreated={(newRef) => {
          setCreateTableOpen(false);
          loadTables().then(() => setSelected(newRef));
        }}
      />
    </div>
  );
}

interface NewColumn {
  id: string;
  name: string;
  type: string;
  pk: boolean;
  notNull: boolean;
  defaultExpr: string;
}

const COLUMN_TYPE_OPTIONS = [
  'bigserial',
  'serial',
  'uuid',
  'text',
  'varchar(255)',
  'integer',
  'bigint',
  'numeric',
  'real',
  'double precision',
  'boolean',
  'jsonb',
  'json',
  'date',
  'timestamptz',
  'timestamp',
];

function CreateTableDialog({
  open,
  defaultSchema,
  runSql,
  onClose,
  onCreated,
}: {
  open: boolean;
  defaultSchema: string;
  runSql: (query: string) => Promise<Array<Record<string, unknown>>>;
  onClose: () => void;
  onCreated: (created: TableRef) => void;
}) {
  const [schema, setSchema] = useState(defaultSchema);
  const [name, setName] = useState('');
  const [cols, setCols] = useState<NewColumn[]>(() => defaultColumns());
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setSchema(defaultSchema);
      setName('');
      setCols(defaultColumns());
      setError(null);
    }
  }, [open, defaultSchema]);

  function addColumn() {
    setCols((c) => [
      ...c,
      { id: Math.random().toString(36).slice(2), name: '', type: 'text', pk: false, notNull: false, defaultExpr: '' },
    ]);
  }
  function removeColumn(id: string) {
    setCols((c) => c.filter((x) => x.id !== id));
  }
  function update(id: string, patch: Partial<NewColumn>) {
    setCols((c) => c.map((x) => (x.id === id ? { ...x, ...patch } : x)));
  }

  function buildSql(): string {
    const qSchema = quoteIdent(schema);
    const qTable = quoteIdent(name);
    const pkCols = cols.filter((c) => c.pk && c.name.trim()).map((c) => quoteIdent(c.name));
    const colDefs = cols
      .filter((c) => c.name.trim())
      .map((c) => {
        let def = `${quoteIdent(c.name)} ${c.type}`;
        if (c.notNull && !c.pk) def += ' NOT NULL';
        if (c.defaultExpr.trim()) def += ` DEFAULT ${c.defaultExpr.trim()}`;
        return def;
      });
    if (pkCols.length > 0) {
      colDefs.push(`PRIMARY KEY (${pkCols.join(', ')})`);
    }
    return `CREATE TABLE ${qSchema}.${qTable} (\n  ${colDefs.join(',\n  ')}\n);`;
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Table name is required.');
      return;
    }
    if (cols.filter((c) => c.name.trim()).length === 0) {
      setError('At least one column is required.');
      return;
    }
    setSubmitting(true);
    try {
      await runSql(buildSql());
      onCreated({ schema, name: trimmedName });
    } catch (err) {
      setError((err as Error).message ?? 'Create failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose} size="max-w-3xl">
      <DialogHeader title="New table" onClose={onClose} />
      <form onSubmit={submit}>
        <DialogBody className="max-h-[60vh] space-y-3 overflow-y-auto">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="ct-schema">Schema</Label>
              <Input
                id="ct-schema"
                value={schema}
                onChange={(e) => setSchema(e.target.value)}
                required
                pattern="[A-Za-z_][A-Za-z0-9_]*"
                className="font-mono"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ct-name">Table name</Label>
              <Input
                id="ct-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                pattern="[A-Za-z_][A-Za-z0-9_]*"
                placeholder="my_table"
                className="font-mono"
              />
            </div>
          </div>

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <Label>Columns</Label>
              <Button type="button" size="sm" variant="outline" onClick={addColumn}>
                <Plus className="h-3.5 w-3.5" /> Add column
              </Button>
            </div>
            <div className="space-y-1.5">
              <div className="grid grid-cols-[1.5fr_1.5fr_36px_36px_1fr_28px] gap-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                <span>Name</span>
                <span>Type</span>
                <span title="Primary key">PK</span>
                <span title="Not null">NN</span>
                <span>Default</span>
                <span></span>
              </div>
              {cols.map((c) => (
                <div key={c.id} className="grid grid-cols-[1.5fr_1.5fr_36px_36px_1fr_28px] items-center gap-1">
                  <Input
                    value={c.name}
                    onChange={(e) => update(c.id, { name: e.target.value })}
                    placeholder="column"
                    className="h-8 font-mono text-xs"
                  />
                  <select
                    value={c.type}
                    onChange={(e) => update(c.id, { type: e.target.value })}
                    className="flex h-8 w-full rounded-md border border-input bg-transparent px-2 font-mono text-xs"
                  >
                    {COLUMN_TYPE_OPTIONS.map((t) => (
                      <option key={t} value={t}>
                        {t}
                      </option>
                    ))}
                  </select>
                  <input
                    type="checkbox"
                    checked={c.pk}
                    onChange={(e) => update(c.id, { pk: e.target.checked })}
                    className="mx-auto"
                    aria-label="Primary key"
                  />
                  <input
                    type="checkbox"
                    checked={c.notNull}
                    onChange={(e) => update(c.id, { notNull: e.target.checked })}
                    className="mx-auto"
                    aria-label="Not null"
                  />
                  <Input
                    value={c.defaultExpr}
                    onChange={(e) => update(c.id, { defaultExpr: e.target.value })}
                    placeholder="now()"
                    className="h-8 font-mono text-xs"
                  />
                  <button
                    type="button"
                    onClick={() => removeColumn(c.id)}
                    className="rounded-md p-1 text-muted-foreground hover:bg-accent"
                    aria-label="Remove column"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>
          </div>

          <details className="rounded-md border border-border p-2">
            <summary className="cursor-pointer text-xs text-muted-foreground">Preview SQL</summary>
            <pre className="mt-2 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px]">
              {buildSql()}
            </pre>
          </details>

          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Creating…' : 'Create table'}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function defaultColumns(): NewColumn[] {
  return [
    { id: 'c1', name: 'id', type: 'bigserial', pk: true, notNull: false, defaultExpr: '' },
    { id: 'c2', name: 'created_at', type: 'timestamptz', pk: false, notNull: false, defaultExpr: 'now()' },
  ];
}

function quoteIdent(s: string): string {
  // Match Postgres identifier quoting — wrap in "" and escape any inner ".
  return '"' + s.replace(/"/g, '""') + '"';
}

function FilterPanel({
  columns,
  filters,
  setFilters,
  onApply,
  onClear,
  onClose,
}: {
  columns: ColumnInfo[];
  filters: FilterRule[];
  setFilters: React.Dispatch<React.SetStateAction<FilterRule[]>>;
  onApply: () => void;
  onClear: () => void;
  onClose: () => void;
}) {
  function addFilter() {
    const col = columns[0];
    if (!col) return;
    setFilters((f) => [
      ...f,
      { id: Math.random().toString(36).slice(2), column: col.name, op: 'eq', value: '' },
    ]);
  }
  function removeFilter(id: string) {
    setFilters((f) => f.filter((x) => x.id !== id));
  }
  function update(id: string, patch: Partial<FilterRule>) {
    setFilters((f) => f.map((x) => (x.id === id ? { ...x, ...patch } : x)));
  }
  return (
    <div className="space-y-2 border-b border-border bg-card/40 px-4 py-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">Filters</span>
        <div className="flex gap-1">
          <Button size="sm" variant="ghost" onClick={onClose}>
            <X className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
      {filters.length === 0 ? (
        <p className="text-xs text-muted-foreground">No filters. Click <strong>Add filter</strong> to add one.</p>
      ) : (
        <div className="space-y-1.5">
          {filters.map((f) => (
            <div key={f.id} className="flex items-center gap-1.5">
              <select
                value={f.column}
                onChange={(e) => update(f.id, { column: e.target.value })}
                className="h-7 rounded-md border border-input bg-transparent px-2 text-xs"
              >
                {columns.map((c) => (
                  <option key={c.name} value={c.name}>
                    {c.name}
                  </option>
                ))}
              </select>
              <select
                value={f.op}
                onChange={(e) => update(f.id, { op: e.target.value })}
                className="h-7 rounded-md border border-input bg-transparent px-2 text-xs"
              >
                {OPERATORS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              <Input
                value={f.value}
                onChange={(e) => update(f.id, { value: e.target.value })}
                placeholder="value"
                className="h-7 flex-1 text-xs"
              />
              <button
                onClick={() => removeFilter(f.id)}
                className="rounded-md p-1 text-muted-foreground hover:bg-accent"
                aria-label="Remove filter"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="flex items-center justify-between pt-1">
        <Button size="sm" variant="outline" onClick={addFilter}>
          <Plus className="h-3.5 w-3.5" /> Add filter
        </Button>
        <div className="flex gap-1.5">
          {filters.length > 0 ? (
            <Button size="sm" variant="ghost" onClick={onClear}>
              Clear
            </Button>
          ) : null}
          <Button size="sm" onClick={onApply}>
            Apply
          </Button>
        </div>
      </div>
      <p className="pt-1 text-[10px] text-muted-foreground">
        Operators map to PostgREST query syntax: <code>eq, neq, gt, gte, lt, lte, like, ilike, in, is</code>.
        For <code>in</code>: <code>(a,b,c)</code>. For <code>is</code>: <code>null</code>, <code>true</code>, <code>false</code>.
      </p>
    </div>
  );
}

interface RowEditorProps {
  mode: 'insert' | 'edit';
  table: TableRef;
  columns: ColumnInfo[];
  initial: Record<string, unknown> | null;
  apikey: string;
  onClose: () => void;
  onSaved: () => void;
}

function RowEditorDialog({ mode, table, columns, initial, apikey, onClose, onSaved }: RowEditorProps) {
  const [values, setValues] = useState<Record<string, string>>(() => initialValues(columns, initial));
  const [nullFlags, setNullFlags] = useState<Record<string, boolean>>(() => initialNullFlags(columns, initial));
  const [submitting, setSubmitting] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pk = columns.find((c) => c.isPk);
  const pkValue = pk && initial ? String(initial[pk.name] ?? '') : '';

  function set(col: string, v: string) {
    setValues((vs) => ({ ...vs, [col]: v }));
  }
  function setNull(col: string, isNull: boolean) {
    setNullFlags((f) => ({ ...f, [col]: isNull }));
  }

  function buildBody(): { body: Record<string, unknown>; error?: string } {
    const body: Record<string, unknown> = {};
    for (const c of columns) {
      if (mode === 'insert' && c.isPk && c.hasDefault) continue; // auto-generated PK
      const raw = values[c.name] ?? '';
      const isNull = nullFlags[c.name];
      if (isNull) {
        body[c.name] = null;
        continue;
      }
      if (raw === '') {
        // Insert: omit (let default kick in); Edit: skip
        continue;
      }
      try {
        body[c.name] = parseValue(c, raw);
      } catch (e) {
        return { body, error: `${c.name}: ${(e as Error).message}` };
      }
    }
    return { body };
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const { body, error: parseError } = buildBody();
    if (parseError) {
      setError(parseError);
      return;
    }
    setSubmitting(true);
    try {
      if (mode === 'insert') {
        await restCall(`/rest/v1/${encodeURIComponent(table.name)}`, {
          method: 'POST',
          schema: table.schema,
          apikey,
          body,
          prefer: 'return=representation',
        });
      } else {
        if (!pk) throw new Error('Table has no primary key — cannot update.');
        await restCall(
          `/rest/v1/${encodeURIComponent(table.name)}?${encodeURIComponent(pk.name)}=eq.${encodeURIComponent(pkValue)}`,
          { method: 'PATCH', schema: table.schema, apikey, body, prefer: 'return=representation' }
        );
      }
      onSaved();
    } catch (err) {
      setError((err as Error).message ?? 'Save failed.');
    } finally {
      setSubmitting(false);
    }
  }

  async function doDelete() {
    if (!pk) {
      setError('Table has no primary key — cannot delete.');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await restCall(
        `/rest/v1/${encodeURIComponent(table.name)}?${encodeURIComponent(pk.name)}=eq.${encodeURIComponent(pkValue)}`,
        { method: 'DELETE', schema: table.schema, apikey }
      );
      onSaved();
    } catch (err) {
      setError((err as Error).message ?? 'Delete failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open onClose={onClose} size="max-w-2xl">
      <DialogHeader
        title={mode === 'insert' ? `Insert into ${table.name}` : `Edit row in ${table.name}`}
        description={mode === 'edit' && pk ? `${pk.name} = ${pkValue}` : undefined}
        onClose={onClose}
      />
      <form onSubmit={submit}>
        <DialogBody className="max-h-[60vh] space-y-3 overflow-y-auto">
          {columns.map((c) => {
            const readOnly = mode === 'edit' && c.isPk;
            const isNull = !!nullFlags[c.name];
            return (
              <div key={c.name} className="space-y-1">
                <div className="flex items-center justify-between gap-2">
                  <Label htmlFor={`f-${c.name}`} className="flex items-center gap-1.5">
                    <span>{c.name}</span>
                    <span className="font-mono text-[10px] text-muted-foreground">{c.type}</span>
                    {c.isPk ? <Badge variant="outline" className="px-1 py-0 text-[9px]">PK</Badge> : null}
                  </Label>
                  {!readOnly && c.nullable ? (
                    <label className="flex items-center gap-1 text-[10px] text-muted-foreground">
                      <input
                        type="checkbox"
                        checked={isNull}
                        onChange={(e) => setNull(c.name, e.target.checked)}
                      />
                      null
                    </label>
                  ) : null}
                </div>
                <FieldInput
                  id={`f-${c.name}`}
                  col={c}
                  value={values[c.name] ?? ''}
                  onChange={(v) => set(c.name, v)}
                  disabled={readOnly || isNull}
                />
                {mode === 'insert' && c.hasDefault ? (
                  <p className="text-[10px] text-muted-foreground">Leave blank to use column default.</p>
                ) : null}
              </div>
            );
          })}
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          {mode === 'edit' ? (
            confirmingDelete ? (
              <>
                <span className="mr-auto text-xs text-destructive">Delete this row?</span>
                <Button type="button" variant="outline" size="sm" onClick={() => setConfirmingDelete(false)} disabled={submitting}>
                  Cancel
                </Button>
                <Button type="button" variant="destructive" size="sm" onClick={doDelete} disabled={submitting}>
                  {submitting ? 'Deleting…' : 'Yes, delete'}
                </Button>
              </>
            ) : (
              <Button
                type="button"
                variant="destructive"
                size="sm"
                onClick={() => setConfirmingDelete(true)}
                disabled={submitting || !pk}
                className="mr-auto"
              >
                <Trash2 className="h-3.5 w-3.5" /> Delete
              </Button>
            )
          ) : null}
          {!confirmingDelete ? (
            <>
              <Button type="button" variant="outline" size="sm" onClick={onClose} disabled={submitting}>
                Cancel
              </Button>
              <Button type="submit" size="sm" disabled={submitting}>
                {submitting ? 'Saving…' : mode === 'insert' ? 'Insert' : 'Save'}
              </Button>
            </>
          ) : null}
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function FieldInput({
  id,
  col,
  value,
  onChange,
  disabled,
}: {
  id: string;
  col: ColumnInfo;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}) {
  const t = col.type.toLowerCase();
  if (t === 'boolean') {
    return (
      <select
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm disabled:opacity-50"
      >
        <option value="">(unset)</option>
        <option value="true">true</option>
        <option value="false">false</option>
      </select>
    );
  }
  if (t === 'json' || t === 'jsonb' || t.endsWith('[]')) {
    return (
      <textarea
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        rows={3}
        placeholder={t.endsWith('[]') ? 'JSON array, e.g. ["a","b"]' : '{"key": "value"}'}
        className="flex w-full rounded-md border border-input bg-transparent px-3 py-2 font-mono text-xs shadow-sm disabled:opacity-50"
      />
    );
  }
  const numeric = ['smallint', 'integer', 'bigint', 'real', 'double precision', 'numeric', 'decimal'];
  if (numeric.some((n) => t.includes(n))) {
    return (
      <Input
        id={id}
        type="number"
        step="any"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
      />
    );
  }
  return <Input id={id} value={value} onChange={(e) => onChange(e.target.value)} disabled={disabled} />;
}

function initialValues(columns: ColumnInfo[], row: Record<string, unknown> | null): Record<string, string> {
  const out: Record<string, string> = {};
  for (const c of columns) {
    const v = row ? row[c.name] : undefined;
    if (v === null || v === undefined) {
      out[c.name] = '';
    } else if (typeof v === 'object') {
      out[c.name] = JSON.stringify(v);
    } else {
      out[c.name] = String(v);
    }
  }
  return out;
}

function initialNullFlags(columns: ColumnInfo[], row: Record<string, unknown> | null): Record<string, boolean> {
  const out: Record<string, boolean> = {};
  if (!row) return out;
  for (const c of columns) {
    if (row[c.name] === null) out[c.name] = true;
  }
  return out;
}

function parseValue(c: ColumnInfo, raw: string): unknown {
  const t = c.type.toLowerCase();
  if (t === 'boolean') {
    if (raw === 'true') return true;
    if (raw === 'false') return false;
    throw new Error('expected true or false');
  }
  if (t === 'json' || t === 'jsonb' || t.endsWith('[]')) {
    try {
      return JSON.parse(raw);
    } catch {
      throw new Error('invalid JSON');
    }
  }
  const numeric = ['smallint', 'integer', 'bigint'];
  if (numeric.some((n) => t.includes(n))) {
    const n = Number(raw);
    if (!Number.isInteger(n)) throw new Error('expected integer');
    return n;
  }
  const floats = ['real', 'double precision', 'numeric', 'decimal'];
  if (floats.some((n) => t.includes(n))) {
    const n = Number(raw);
    if (Number.isNaN(n)) throw new Error('expected number');
    return n;
  }
  return raw;
}

interface RestCallOptions {
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE';
  schema: string;
  apikey: string;
  body?: unknown;
  prefer?: string;
}

async function restCall(path: string, opts: RestCallOptions): Promise<unknown> {
  const isWrite = opts.method !== 'GET';
  const headers: Record<string, string> = {
    apikey: opts.apikey,
    Authorization: `Bearer ${opts.apikey}`,
  };
  if (opts.body !== undefined) headers['Content-Type'] = 'application/json';
  if (opts.prefer) headers['Prefer'] = opts.prefer;
  if (isWrite) headers['Content-Profile'] = opts.schema;
  else headers['Accept-Profile'] = opts.schema;

  const res = await fetch(`${API_BASE}${path}`, {
    method: opts.method,
    headers,
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`REST ${res.status}: ${text}`);
  }
  if (res.status === 204) return null;
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) return res.json();
  return res.text();
}

function formatCell(v: unknown): string {
  if (v === null || v === undefined) return '';
  if (typeof v === 'object') return JSON.stringify(v);
  return String(v);
}
