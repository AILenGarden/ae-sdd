#!/usr/bin/env python3
"""Read-only OKF/profile checks and explicit, owned navigation generation."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
from datetime import datetime, timezone
from urllib.parse import unquote, urlsplit, quote

try:
    import yaml
except ModuleNotFoundError as exc:
    raise SystemExit('PyYAML is required; install this skill\'s requirements.txt in your Python environment.') from exc

PROFILE = 'ae-sdd-knowledge/1'
STATES = {'confirmed', 'inferred', 'planned', 'unknown', 'stale'}
RELATIONS = {'contains', 'implements', 'planned-for', 'verifies', 'depends-on',
             'references', 'relates-to', 'supersedes'}
CALLS = {'calls', 'dispatches-to', 'publishes', 'consumed-by', 'remote-call'}
COVERAGE = {'full', 'partial', 'missing', 'unknown', 'not-applicable'}
MARKER = re.compile(r'<!-- al-knowledge-generated sha256=([a-f0-9]{64}) -->\n')
LINK = re.compile(r'(?<!!)\[[^\]\n]*\]\(([^)\s]+)\)')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def text_hash(text):
    return digest(text.replace('\r\n', '\n').encode('utf-8'))


def nonempty(value):
    return isinstance(value, str) and bool(value.strip())


def member(value, choices):
    return isinstance(value, str) and value in choices


def instant(value):
    try:
        parsed = value if isinstance(value, datetime) else datetime.fromisoformat(str(value))
        return parsed if parsed.tzinfo is not None else None
    except (ValueError, TypeError):
        return None


class UniqueLoader(yaml.SafeLoader):
    """A duplicate key must not silently replace evidence."""


def unique_mapping(loader, node, deep=False):
    result = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if not isinstance(key, str) or key in result:
            raise ValueError('YAML mapping keys must be unique strings')
        result[key] = loader.construct_object(value_node, deep=deep)
    return result


UniqueLoader.add_constructor(yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, unique_mapping)


def split_document(raw):
    text = raw.decode('utf-8-sig').replace('\r\n', '\n')
    if not text.startswith('---\n'):
        return None, text
    match = re.search(r'^---\s*$', text[4:], flags=re.M)
    if match is None:
        raise ValueError('frontmatter has no closing delimiter')
    end = 4 + match.start()
    data = yaml.load(text[4:end], Loader=UniqueLoader)
    if not isinstance(data, dict):
        raise ValueError('frontmatter must be a mapping')
    return data, text[4 + match.end():].lstrip('\n')


def inside(path, root):
    return path.resolve().is_relative_to(root.resolve())


def local_target(root, owner, value):
    """Return bundle path and explicit anchor; never fetch a URL."""
    if not isinstance(value, str):
        return None, ''
    uri = urlsplit(value)
    if uri.scheme or uri.netloc:
        return None, uri.fragment
    path_text = unquote(uri.path)
    if '\\' in path_text:
        raise ValueError('use / in bundle paths')
    target = (root / path_text.lstrip('/')) if path_text.startswith('/') else owner.parent / path_text
    if not path_text:
        target = owner
    if not inside(target, root):
        raise ValueError('path escapes bundle')
    return target.resolve(), unquote(uri.fragment)


class Bundle:
    def __init__(self, root, repositories=None):
        self.root = Path(root).resolve()
        self.repositories = {key: Path(value).resolve() for key, value in (repositories or {}).items()}
        self.docs = {}
        self.snapshots = {}
        self.issues = []
        self.ids = {}
        self.nodes = {}
        self.edges = []
        self.changed = set()
        self.uncertain = set()

    def issue(self, level, layer, path, code, detail):
        label = str(path)
        if isinstance(path, Path) and inside(path, self.root):
            label = path.relative_to(self.root).as_posix()
        self.issues.append(dict(level=level, layer=layer, path=label, code=code, detail=detail))

    def load(self):
        if not self.root.is_dir():
            raise ValueError('bundle must be an existing directory')
        for folder, dirs, names in os.walk(self.root, followlinks=False):
            for name in list(dirs):
                if (Path(folder) / name).is_symlink() or not inside(Path(folder) / name, self.root):
                    raise ValueError('linked directories are not supported by this local tool')
            for name in sorted(names):
                path = Path(folder) / name
                if path.suffix != '.md':
                    continue
                if path.is_symlink() or not inside(path, self.root):
                    raise ValueError('linked documents are not supported by this local tool')
                raw = path.read_bytes()
                self.snapshots[path] = raw
                try:
                    meta, body = split_document(raw)
                except (ValueError, UnicodeError, yaml.YAMLError, RecursionError) as exc:
                    self.issue('error', 'okf', path, 'frontmatter', str(exc))
                    continue
                if name in {'index.md', 'log.md'}:
                    if meta is not None and not (path == self.root / 'index.md' and set(meta) <= {'okf_version'}):
                        self.issue('error', 'okf', path, 'reserved-frontmatter', 'reserved file metadata is invalid')
                    if name == 'log.md':
                        for heading in re.findall(r'^## (.+)$', body, flags=re.M):
                            try:
                                datetime.strptime(heading, '%Y-%m-%d')
                            except ValueError:
                                self.issue('error', 'okf', path, 'log-date', heading)
                    continue
                if not meta or not nonempty(meta.get('type')):
                    self.issue('error', 'okf', path, 'type', 'concept requires a non-empty type')
                    continue
                al = meta.get('al', {})
                if not isinstance(al, dict):
                    self.issue('warning', 'profile', path, 'al', 'unrecognized extension; generic OKF remains readable')
                    al = {}
                doc = dict(path=path, meta=meta, body=body, al=al, raw=raw)
                self.docs[path] = doc
                if al.get('profile') == PROFILE:
                    ident = al.get('id')
                    if not nonempty(ident):
                        self.issue('error', 'profile', path, 'id', 'al.id is required')
                    elif ident in self.ids:
                        self.issue('error', 'profile', path, 'duplicate-id', ident)
                    else:
                        self.ids[ident] = doc
                else:
                    self.issue('warning', 'profile', path, 'profile-unavailable', 'generic OKF; engineering checks limited')
        for path in sorted(self.docs):
            self.validate_document(self.docs[path])
        self.validate_graph()
        self.check_sources()
        self.propagate_sources()
        return self

    def records(self, doc, key):
        value = doc['al'].get(key, [])
        if not isinstance(value, list) or any(not isinstance(row, dict) for row in value):
            self.issue('error', 'profile', doc['path'], key, 'expected list of mappings')
            return []
        if key != 'test_runs':
            ids = [row.get('id') for row in value]
            if any(not nonempty(item) for item in ids) or len(set(str(item) for item in ids)) != len(ids):
                self.issue('error', 'profile', doc['path'], key + '-ids', 'IDs must be non-empty and unique in list')
        return value

    def source_map(self, doc):
        values = doc['meta'].get('sources', [])
        if not isinstance(values, list):
            return {}
        return {s['id']: s for s in values if isinstance(s, dict) and nonempty(s.get('id'))}

    def resolved(self, doc, target):
        try:
            return local_target(self.root, doc['path'], target)
        except ValueError as exc:
            self.issue('warning', 'navigation', doc['path'], 'path', str(exc))
            return None, ''

    def evidence(self, doc, row, label):
        path = doc['path']
        state = row.get('evidence_state')
        if not member(state, STATES):
            self.issue('error', 'profile', path, 'evidence-state', label)
        refs = row.get('source_ids', [])
        sources = self.source_map(doc)
        if not isinstance(refs, list) or any(not nonempty(s) or s not in sources for s in refs):
            self.issue('error', 'profile', path, 'source-ids', label)
            return
        checked = row.get('checked')
        if state == 'confirmed' and (not refs or not isinstance(checked, dict)):
            self.issue('error', 'profile', path, 'confirmation', label + ' needs sources and checked')
            return
        if not isinstance(checked, dict):
            return
        if not nonempty(checked.get('by')) or not instant(checked.get('at')):
            self.issue('error', 'profile', path, 'checked-actor', label)
        hashes = checked.get('source_sha256', {})
        if not isinstance(hashes, dict):
            self.issue('error', 'profile', path, 'checked-hashes', label)
            return
        if not re.fullmatch(r'[a-f0-9]{64}', str(checked.get('body_sha256', ''))) or any(not re.fullmatch(r'[a-f0-9]{64}', str(hashes.get(key, ''))) for key in refs):
            self.issue('error', 'profile', path, 'checked-hashes', label + ' requires body and each source digest')
        if checked.get('body_sha256') != text_hash(doc['body']):
            self.issue('warning', 'freshness', path, 'body-changed', label)
            self.changed.add(path)
        for key in refs:
            target, _ = self.resolved(doc, sources[key].get('resource'))
            if target not in self.docs:
                self.issue('warning', 'freshness', path, 'source-unchecked', label + ':' + key)
                self.uncertain.add(path)
            elif hashes.get(key) != digest(self.docs[target]['raw']):
                self.issue('warning', 'freshness', path, 'evidence-changed', label + ':' + key)
                self.changed.add(path)

    def validate_document(self, doc):
        path, meta, al = doc['path'], doc['meta'], doc['al']
        # Optional OKF fields remain advisory for generic readers.
        for family in ('generated', 'verified'):
            events = meta.get(family, [])
            events = events if isinstance(events, list) else [events]
            for event in events:
                if not isinstance(event, dict) or not nonempty(event.get('by')) or (family == 'verified' or 'at' in event) and not instant(event.get('at')):
                    self.issue('warning', 'okf', path, 'actor-event', family)
        expiry = meta.get('stale_after')
        if expiry is not None:
            dt = instant(expiry)
            if dt is None:
                self.issue('warning', 'okf', path, 'stale-after', 'expected datetime with offset')
            elif dt <= datetime.now(timezone.utc):
                self.issue('warning', 'freshness', path, 'expired', 'definition needs recheck')
                self.changed.add(path)
        source_rows = meta.get('sources', [])
        if not isinstance(source_rows, list) or any(not isinstance(s, dict) or not nonempty(s.get('resource')) for s in source_rows):
            self.issue('warning', 'okf', path, 'sources', 'expected sources with resource')
        for value in LINK.findall(doc['body']):
            target, anchor = self.resolved(doc, value)
            if target is not None and (not target.exists() or anchor and target in self.docs and not self.has_anchor(self.docs[target], anchor)):
                self.issue('warning', 'navigation', path, 'broken-link', value)
        if al.get('profile') != PROFILE:
            return
        for field in ('title', 'description'):
            if not nonempty(meta.get(field)):
                self.issue('error', 'profile', path, field, 'required for ae-sdd concepts')
        if not member(meta.get('status', 'stable'), {'draft', 'stable', 'deprecated'}):
            self.issue('error', 'profile', path, 'status', 'unknown lifecycle')
        if not isinstance(source_rows, list) or len(self.source_map(doc)) != len(source_rows):
            self.issue('error', 'profile', path, 'source-identities', 'sources require unique IDs')
        if path == self.root / 'project.md':
            for field in ('project_id',):
                if not nonempty(al.get(field)):
                    self.issue('error', 'profile', path, field, 'required project field')
            for field in ('scan_scope', 'excluded_scope'):
                if not isinstance(al.get(field), list) or any(not nonempty(v) for v in al[field]):
                    self.issue('error', 'profile', path, field, 'expected string list')
            if not member(al.get('coverage'), {'partial', 'complete-in-scope'}) or not isinstance(al.get('repositories'), dict):
                self.issue('error', 'profile', path, 'project-scope', 'coverage and repositories required')
        if meta['type'] == 'API Endpoint':
            interface = al.get('interface', {})
            if not isinstance(interface, dict) or any(not nonempty(interface.get(k)) for k in ('service', 'protocol', 'signature')):
                self.issue('error', 'profile', path, 'interface', 'service, protocol, signature required')
        for claim in self.records(doc, 'claims'):
            if not nonempty(claim.get('anchor')) or not self.has_anchor(doc, claim['anchor']):
                self.issue('error', 'profile', path, 'claim-anchor', str(claim.get('id')))
            self.evidence(doc, claim, 'claim:' + str(claim.get('id')))
        for relation in self.records(doc, 'relations'):
            self.evidence(doc, relation, 'relation:' + str(relation.get('id')))
            kind = relation.get('type')
            if not nonempty(kind):
                self.issue('error', 'profile', path, 'relation-type', 'type must be a non-empty string')
            elif not member(kind, RELATIONS):
                self.issue('warning', 'profile', path, 'unknown-relation', str(kind))
            if kind == 'planned-for' and relation.get('evidence_state') != 'planned':
                self.issue('error', 'profile', path, 'planned-state', 'planned-for must be planned')
            if kind == 'implements' and not member(relation.get('coverage'), COVERAGE):
                self.issue('error', 'profile', path, 'coverage', 'implements requires coverage')
            target, anchor = self.resolved(doc, relation.get('target'))
            other = self.docs.get(target)
            level = 'error' if relation.get('evidence_state') == 'confirmed' else 'warning'
            if other is None or other['al'].get('id') != relation.get('target_id') or not nonempty(relation.get('target_id')) or anchor and not self.has_anchor(other, anchor):
                self.issue(level, 'profile', path, 'relation-target', str(relation.get('id')))
            else:
                if kind == 'contains' and (meta['type'] != 'Specification' or other['meta']['type'] != 'Specification'):
                    self.issue('error', 'profile', path, 'contains-type', 'contains is Spec hierarchy')
                if kind == 'implements' and (meta['type'] != 'API Endpoint' or other['meta']['type'] != 'Specification' or not anchor):
                    self.issue('error', 'profile', path, 'implements-contract', 'API Endpoint must implement an anchored Specification rule')
                if kind == 'planned-for' and (meta['type'] != 'Specification' or other['meta']['type'] != 'API Endpoint'):
                    self.issue('error', 'profile', path, 'planned-contract', 'Specification plans an API Endpoint')
                if kind == 'verifies' and (meta['type'] != 'Specification' or other['meta']['type'] not in {'Specification', 'API Endpoint'}):
                    self.issue('error', 'profile', path, 'verifies-contract', 'verification Specification targets Spec or API')
                self.edges.append((path, target, relation))
        for node in self.records(doc, 'nodes'):
            ident = node.get('id')
            if not nonempty(ident):
                continue
            if ident in self.nodes:
                self.issue('error', 'profile', path, 'duplicate-node', ident)
            self.nodes[ident] = (doc, node)
            if not nonempty(node.get('title')) or not member(node.get('kind'), {'method', 'boundary'}):
                self.issue('error', 'profile', path, 'node', ident)
            required = 'symbol' if node.get('kind') == 'method' else 'stop_reason'
            if not nonempty(node.get(required)):
                self.issue('error', 'profile', path, 'node-' + required, ident)
        for call in self.records(doc, 'calls'):
            self.evidence(doc, call, 'call:' + str(call.get('id')))
            if not member(call.get('type'), CALLS):
                self.issue('error', 'profile', path, 'call-type', str(call.get('id')))
        for run in self.records(doc, 'test_runs'):
            refs = run.get('source_ids', [])
            if not member(run.get('result'), {'passed', 'failed', 'not-run'}) or not isinstance(refs, list) or any(not nonempty(s) or s not in self.source_map(doc) for s in refs):
                self.issue('error', 'profile', path, 'test-run', 'result and source IDs invalid')
            if member(run.get('result'), {'passed', 'failed'}) and (not refs or not instant(run.get('at')) or not nonempty(run.get('by')) or not nonempty(run.get('command'))):
                self.issue('error', 'profile', path, 'test-evidence', 'executed run requires provenance')

    @staticmethod
    def has_anchor(doc, anchor):
        return any(match == anchor for match in re.findall(r'<a\s+id=["\']([^"\']+)["\']\s*>', doc['body']))

    def validate_graph(self):
        if self.ids:
            project = self.docs.get(self.root / 'project.md')
            if not project or project['meta']['type'] != 'Project Context' or project['al'].get('profile') != PROFILE:
                self.issue('error', 'profile', self.root, 'project', 'engineering bundle needs project.md')
        parents = {}
        for source, target, relation in self.edges:
            if relation.get('type') == 'contains' and relation.get('evidence_state') == 'confirmed':
                if target in parents:
                    self.issue('error', 'profile', target, 'multiple-parents', 'confirmed contains conflict')
                parents.setdefault(target, set()).add(source)
        # Remove roots repeatedly; retaining every parent avoids hiding a cycle
        # when a second parent is registered on the same child.
        remaining = {node: set(owners) for node, owners in parents.items()}
        while remaining:
            roots = set().union(*remaining.values()) - set(remaining)
            if not roots:
                for node in remaining:
                    self.issue('error', 'profile', node, 'contains-cycle', 'cycle or descendant blocked by cycle')
                break
            remaining = {node: owners - roots for node, owners in remaining.items() if owners - roots}
        for doc in self.docs.values():
            if doc['al'].get('profile') != PROFILE:
                continue
            entry = doc['al'].get('entry')
            if entry is not None and (not nonempty(entry) or entry not in self.nodes):
                self.issue('error', 'profile', doc['path'], 'entry', 'unresolved entry node')
            for call in self.records(doc, 'calls'):
                origin, target = call.get('from'), call.get('to')
                level = 'error' if call.get('evidence_state') == 'confirmed' else 'warning'
                if not nonempty(origin) or origin not in self.nodes or self.nodes[origin][0] is not doc:
                    self.issue('error', 'profile', doc['path'], 'call-owner', str(call.get('id')))
                if not nonempty(target) or target not in self.nodes:
                    self.issue('error' if not nonempty(target) else level, 'profile', doc['path'], 'call-target', str(call.get('id')))

    def check_sources(self):
        project = self.docs.get(self.root / 'project.md', {})
        aliases = project.get('al', {}).get('repositories', {})
        aliases = aliases if isinstance(aliases, dict) else {}
        for doc in self.docs.values():
            source = doc['al'].get('source')
            if source is None or doc['al'].get('profile') != PROFILE:
                continue
            path = doc['path']
            if not isinstance(source, dict) or not nonempty(source.get('path')) or not nonempty(source.get('repository')) or source['repository'] not in aliases or not re.fullmatch(r'[a-f0-9]{64}', str(source.get('sha256', ''))) or not instant(source.get('captured_at')):
                self.issue('error', 'profile', path, 'source-baseline', 'source needs registered repository, path, sha256, captured_at')
                continue
            rel = Path(source['path'])
            if rel.is_absolute() or '..' in rel.parts or '\\' in source['path']:
                self.issue('error', 'profile', path, 'source-path', 'source path must stay inside repository')
                continue
            repo = self.repositories.get(source['repository'])
            if repo is None:
                self.issue('warning', 'freshness', path, 'repository-unmapped', source['repository'])
                self.uncertain.add(path)
                continue
            target = repo / rel
            if not inside(target, repo):
                self.issue('error', 'profile', path, 'source-path', 'resolved source escapes repository')
                continue
            try:
                current = digest(target.read_bytes())
            except OSError:
                self.issue('warning', 'freshness', path, 'source-unavailable', source['path'])
                self.changed.add(path)
                continue
            if current != source['sha256']:
                self.issue('warning', 'freshness', path, 'source-changed', source['path'])
                self.changed.add(path)

    def propagate_sources(self):
        """Propagate source invalidation only; ordinary graph links are potential impact."""
        dependencies = []
        for doc in self.docs.values():
            for source in self.source_map(doc).values():
                target, _ = self.resolved(doc, source.get('resource'))
                if target in self.docs:
                    dependencies.append((doc['path'], target))
        for collection, code in ((self.changed, 'source-dependency-changed'), (self.uncertain, 'source-dependency-unchecked')):
            while True:
                newly = {a for a, b in dependencies if b in collection} - collection
                if not newly:
                    break
                for path in sorted(newly):
                    self.issue('warning', 'freshness', path, code, 'referenced evidence requires recheck')
                collection.update(newly)

    def impact(self, seeds):
        reached = set(seeds)
        dependencies = [(a, b) for a, b, _ in self.edges]
        for doc in self.docs.values():
            for source in self.source_map(doc).values():
                target, _ = self.resolved(doc, source.get('resource'))
                if target in self.docs:
                    dependencies.append((doc['path'], target))
            for call in doc['al'].get('calls', []) if isinstance(doc['al'].get('calls', []), list) else []:
                if isinstance(call, dict) and nonempty(call.get('to')) and call['to'] in self.nodes:
                    dependencies.append((doc['path'], self.nodes[call['to']][0]['path']))
            entry = doc['al'].get('entry')
            if nonempty(entry) and entry in self.nodes:
                dependencies.append((doc['path'], self.nodes[entry][0]['path']))
        while True:
            more = {a for a, b in dependencies if b in reached} | {b for a, b in dependencies if a in reached}
            if more <= reached:
                break
            reached |= more
        return sorted(p.relative_to(self.root).as_posix() for p in reached if p not in seeds)

    def report(self):
        return dict(okf_valid=not any(i['level'] == 'error' and i['layer'] == 'okf' for i in self.issues),
                    profile_valid=not any(i['level'] == 'error' and i['layer'] == 'profile' for i in self.issues),
                    concepts=len(self.docs), issues=self.issues,
                    directly_changed=sorted(p.relative_to(self.root).as_posix() for p in self.changed),
                    unchecked=sorted(p.relative_to(self.root).as_posix() for p in self.uncertain),
                    possibly_affected=self.impact(self.changed),
                    limitation='Structure and recorded evidence only; no claim of factual or test correctness.')


def label(doc):
    return str(doc['meta'].get('title', doc['path'].stem)).replace('\n', ' ')


def generated(body, prefix=''):
    return prefix + f'<!-- al-knowledge-generated sha256={text_hash(prefix + body)} -->\n' + body


def render(bundle):
    outputs = {}
    directories = {bundle.root} | {p.parent for p in bundle.docs}
    # Refresh old directories after concepts move. Human indexes still fail
    # write preflight; no old directory or user content is silently deleted.
    directories.update(p.parent for p in bundle.snapshots if p.name == 'index.md')
    for path in list(directories):
        directories.update(parent for parent in path.parents if inside(parent, bundle.root))
    for folder in sorted(directories):
        prefix = '---\nokf_version: "0.2"\n---\n\n' if folder == bundle.root else ''
        rows = ['# Knowledge index', '']
        for child in sorted(p for p in directories if p.parent == folder and p != folder):
            rows.append(f'- [{child.name}]({quote(child.name)}/index.md) - Knowledge group')
        for path, doc in sorted(bundle.docs.items()):
            if path.parent == folder:
                title = label(doc).replace('[', '\\[').replace(']', '\\]')
                rows.append(f'- [{title}]({quote(path.name)}) - {str(doc["meta"].get("description", "")).replace(chr(10), " ")}')
        # Root frontmatter must stay at byte zero.
        outputs[folder / 'index.md'] = generated('\n'.join(rows) + '\n', prefix)
    specs = {p: d for p, d in bundle.docs.items() if d['meta']['type'] == 'Specification'}
    children = {}
    targets = set()
    for a, b, rel in bundle.edges:
        if rel['type'] == 'contains' and rel.get('evidence_state') == 'confirmed' and a not in bundle.changed | bundle.uncertain and b not in bundle.changed | bundle.uncertain:
            children.setdefault(a, []).append((b, rel))
            targets.add(b)
    tree = ['# Spec tree', '', 'Only confirmed contains; grouping is not a Spec. Other edges remain below.', '', '```text']

    def walk_spec(path, prefix=''):
        tree.append(prefix + label(specs[path]) + ' [' + str(specs[path]['al'].get('id', path.stem)) + ']')
        for child, rel in sorted(children.get(path, []), key=lambda x: str(x[0])):
            tree.append(prefix + '└── contains / ' + str(rel['id']))
            walk_spec(child, prefix + '    ')

    for path in sorted(set(specs) - targets):
        walk_spec(path)
    if not specs:
        tree.append('No Specifications registered in this bundle.')
    tree += ['```', '', '## Registered relationships', '']
    mapping = ['# Spec / interface map', '', 'Static possible paths; sibling order is not execution order.', '', '```text']
    for path, spec in sorted(specs.items()):
        mapping.append(label(spec) + ' [' + str(spec['al'].get('id', path.stem)) + ']')
        matches = [(a, a, b, r) for a, b, r in bundle.edges if b == path and r['type'] in {'implements', 'verifies'}]
        matches += [(b, a, b, r) for a, b, r in bundle.edges if a == path and r['type'] == 'planned-for']
        for other, origin, target, rel in matches:
            mapping.append('└── ' + label(bundle.docs[other]) + ' / ' + relation_label(bundle, origin, target, rel))
        if not matches:
            mapping.append('└── No registered interface mapping [unknown; inspect source/scope]')
    for a, b, rel in bundle.edges:
        tree.append(f'- {label(bundle.docs[a])} → {label(bundle.docs[b])}: {relation_label(bundle, a, b, rel)}')
    mapping += ['```', '', '## Interface call trees', '']
    expanded = {}
    adjacency = {}
    for doc in bundle.docs.values():
        for call in bundle.records(doc, 'calls') if doc['al'].get('profile') == PROFILE else []:
            adjacency.setdefault(call.get('from'), []).append((doc, call))

    def walk_node(node_id, lines, prefix, active, owner):
        if node_id in active:
            lines.append(prefix + '↩ ' + node_id + ' [recursion; stop expansion]')
            return
        if node_id in expanded:
            lines.append(prefix + '↪ ' + node_id + ' [shared; see ' + expanded[node_id] + ']')
            return
        item = bundle.nodes.get(node_id)
        if item is None:
            lines.append(prefix + str(node_id) + ' [unknown target]')
            return
        expanded[node_id] = owner
        doc, node = item
        lines.append(prefix + node['title'] + ' [' + node_id + '] ' + str(node.get('stop_reason', '')))
        for origin, call in adjacency.get(node_id, []):
            status = effective_state(bundle, origin['path'], call)
            lines.append(prefix + '└── ' + str(call['type']) + '/' + str(call['id']) + ' [' + status + '] ' + str(call.get('condition', '')))
            walk_node(call.get('to'), lines, prefix + '    ', active | {node_id}, owner)

    for path, doc in sorted(bundle.docs.items()):
        if doc['meta']['type'] != 'API Endpoint':
            continue
        title = label(doc) + ' [' + str(doc['al'].get('id', path.stem)) + ']'
        mapping += ['### ' + title, '', '```text']
        if not any((a == path or b == path) and r['type'] in {'implements', 'planned-for', 'verifies'} for a, b, r in bundle.edges):
            mapping.append('No associated Spec [unknown; registered scope only]')
        if doc['al'].get('entry'):
            walk_node(doc['al']['entry'], mapping, '', set(), title)
        else:
            mapping.append('No registered implementation entry [unknown]')
        mapping += ['```', '']
    warnings = ['## Check limitations', ''] + [f'- {i["path"]}: {i["code"]} — {i["detail"]}' for i in bundle.issues]
    view_root = bundle.root.parent / '.al-knowledge-views'
    outputs[view_root / 'spec-tree.md'] = generated('\n'.join(tree + [''] + warnings) + '\n')
    outputs[view_root / 'spec-code-map.md'] = generated('\n'.join(mapping + warnings) + '\n')
    return outputs


def relation_label(bundle, source, target, rel):
    state = effective_state(bundle, source, rel)
    if target in bundle.changed:
        state = 'stale'
    return f'{rel["type"]}/{rel["id"]} [{state}; {rel.get("coverage", "n/a")}] {rel.get("condition", "")}'


def effective_state(bundle, path, record):
    if path in bundle.changed:
        return 'stale'
    state = record.get('evidence_state', 'unknown')
    if path in bundle.uncertain and state == 'confirmed':
        return 'confirmed-at-baseline; current source unchecked'
    return state


def owned(raw):
    text = raw.decode('utf-8').replace('\r\n', '\n')
    prefix = ''
    if text.startswith('---\n'):
        _, body = split_document(raw)
        prefix = text[:len(text) - len(body)]
        text = body
    match = MARKER.match(text)
    return bool(match and text_hash(prefix + text[match.end():]) == match.group(1))


def write_outputs(bundle, outputs):
    """Preflight all targets; lock writers; atomic per-file replace, report partial failure."""
    before = {}
    for path in outputs:
        if not inside(path, bundle.root.parent) or path.is_symlink():
            raise ValueError('output escapes project or is a link: ' + str(path))
        raw = path.read_bytes() if path.exists() else None
        if raw is not None and not owned(raw):
            raise ValueError('unmanaged or edited output: ' + str(path))
        before[path] = raw
    lock = bundle.root / '.render.lock'
    written = []
    try:
        fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError as exc:
        raise ValueError('render lock exists; check active writer before recovery') from exc
    try:
        os.close(fd)
        if {p for p in bundle.root.rglob('*.md')} != set(bundle.snapshots):
            raise ValueError('document set changed during render; reload bundle')
        for path, raw in bundle.snapshots.items():
            if path.read_bytes() != raw:
                raise ValueError('source changed during render: ' + str(path))
        for path, text in outputs.items():
            current = path.read_bytes() if path.exists() else None
            if current != before[path]:
                raise ValueError('output changed during render: ' + str(path))
            data = text.encode('utf-8')
            if current == data:
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            descriptor, temporary = tempfile.mkstemp(prefix='.knowledge-', dir=path.parent)
            try:
                with os.fdopen(descriptor, 'wb') as stream:
                    stream.write(data)
                os.replace(temporary, path)
                written.append(str(path))
            finally:
                if os.path.exists(temporary):
                    os.unlink(temporary)
    except (OSError, ValueError) as exc:
        raise ValueError(f'{exc}; already written: {written}') from exc
    finally:
        lock.unlink()
    return written


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['check', 'inspect', 'impact', 'render'])
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--repo', action='append', default=[], metavar='ALIAS=PATH')
    parser.add_argument('--id', action='append', default=[], help='engineering ID to seed impact')
    parser.add_argument('--write', action='store_true', help='render only; default is dry-run')
    args = parser.parse_args(argv)
    try:
        repos = {}
        for pair in args.repo:
            key, value = pair.split('=', 1)
            if not key or not value or key in repos:
                raise ValueError('repo bindings need unique ALIAS=PATH values')
            repos[key] = value
        if args.write and args.command != 'render':
            raise ValueError('--write is only available for render')
        bundle = Bundle(args.bundle, repos).load()
        result = bundle.report()
        if args.command == 'inspect':
            result['digests'] = {p.relative_to(bundle.root).as_posix(): dict(file_sha256=digest(d['raw']), body_sha256=text_hash(d['body'])) for p, d in sorted(bundle.docs.items())}
        if args.command == 'impact':
            unknown = set(args.id) - set(bundle.ids)
            if unknown:
                raise ValueError('unknown engineering IDs: ' + ', '.join(sorted(unknown)))
            seeds = bundle.changed | {bundle.ids[key]['path'] for key in args.id}
            result['seeds'] = sorted(p.relative_to(bundle.root).as_posix() for p in seeds)
            result['possibly_affected'] = bundle.impact(seeds)
        if args.command == 'render' and not any(i['level'] == 'error' for i in bundle.issues):
            outputs = render(bundle)
            result['outputs'] = [str(p) for p in outputs]
            result['written'] = write_outputs(bundle, outputs) if args.write else []
            result['dry_run'] = not args.write
        print(json.dumps(result, ensure_ascii=False, indent=2, default=str))
        return 1 if any(i['level'] == 'error' for i in bundle.issues) else 0
    except (OSError, ValueError, RecursionError) as exc:
        print(json.dumps({'error': str(exc)}, ensure_ascii=False))
        return 2


if __name__ == '__main__':
    sys.exit(main())
