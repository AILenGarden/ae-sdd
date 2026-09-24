#!/usr/bin/env python3
"""Synchronize only ae-sdd-owned AGENTS.md regions; dry-run by default."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import uuid

MARKER = 'ae-sdd-entry'
TITLE = '## 项目工程入口：ae-sdd'.encode('utf-8')
START = re.compile(rb'^<!-- ae-sdd:managed:start id="ae-sdd-entry" version="[0-9]+" -->\r?$', re.M)
END = re.compile(rb'^<!-- ae-sdd:managed:end id="ae-sdd-entry" -->\r?$', re.M)
TEMPLATE = Path(__file__).resolve().parents[1] / 'references/agents-guidance.md'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return path.read_bytes() if path.exists() else None


def managed_span(raw):
    raw = raw or b''
    offset = 3 if raw.startswith(b'\xef\xbb\xbf') else 0
    starts, ends = list(START.finditer(raw[offset:])), list(END.finditer(raw[offset:]))
    markers = re.findall(rb'<!--\s*ae-sdd:managed:', raw)
    if not markers:
        return None
    if len(markers) != 2 or len(starts) != 1 or len(ends) != 1 or starts[0].start() >= ends[0].start():
        raise ValueError('malformed, duplicated or legacy ae-sdd markers; preserve and review')
    return offset + starts[0].start(), offset + ends[0].end()


def legacy_span(raw):
    offset = 3 if raw.startswith(b'\xef\xbb\xbf') else 0
    matches = list(re.finditer(rb'^' + re.escape(TITLE) + rb'\r?$', raw[offset:], flags=re.M))
    if not matches:
        return None
    if len(matches) != 1:
        raise ValueError('duplicate unmarked ae-sdd headings; preserve and review')
    start = offset + matches[0].start()
    match_end = offset + matches[0].end()
    following = re.search(rb'^## ', raw[match_end:], flags=re.M)
    end = match_end + following.start() if following else len(raw)
    end = start + len(raw[start:end].rstrip(b'\r\n'))
    return start, end


def template_block(path):
    raw = path.read_bytes().replace(b'\r\n', b'\n')
    span = managed_span(raw)
    if span is None:
        raise ValueError('template has no owned ae-sdd entry')
    return raw[span[0]:span[1]]


def plan(action, agents, mirrors, record, template=TEMPLATE, adopt_hash=None):
    targets = [Path(agents).resolve(), *(Path(p).resolve() for p in mirrors)]
    record = Path(record).resolve()
    if len(set(targets)) != len(targets) or record in targets:
        raise ValueError('targets and record must be distinct')
    if any(p.name != 'AGENTS.md' for p in targets):
        raise ValueError('explicit targets must be AGENTS.md files')
    record_before = read(record)
    previous = {}
    if record_before is not None:
        state = json.loads(record_before.decode('utf-8'))
        if not isinstance(state, dict) or state.get('schemaVersion') != 1 or state.get('suite') != 'ae-sdd' or state.get('markerId') != MARKER or not isinstance(state.get('targets'), dict):
            raise ValueError('invalid guidance ownership record')
        previous = state['targets']
        if set(previous) != {str(p) for p in targets}:
            raise ValueError('explicit target set differs from recorded targets; preserve ownership')
    block = template_block(Path(template)) if action == 'install' else None
    changes, entries, descriptions = [], {}, []
    snapshots = {record: record_before}
    for path in targets:
        before = read(path)
        snapshots[path] = before
        raw = before or b''
        raw.decode('utf-8-sig')
        span = managed_span(raw)
        entry = previous.get(str(path))
        if entry is not None:
            if not isinstance(entry, dict) or entry.get('markerId') != MARKER:
                raise ValueError('invalid target ownership: ' + str(path))
            if not isinstance(entry.get('prefix'), str) or not isinstance(entry.get('suffix'), str) or entry['prefix'] not in {'', '0a0a'} or entry['suffix'] not in {'', '0a'}:
                raise ValueError('invalid owned whitespace: ' + str(path))
            if span is None:
                if action == 'install' or legacy_span(raw) is not None:
                    raise ValueError('recorded marker missing; preserve and review: ' + str(path))
            elif sha(raw[span[0]:span[1]]) != entry.get('sha256'):
                raise ValueError('managed region was edited; preserve: ' + str(path))
        elif span is not None:
            raise ValueError('marked content without ownership record; preserve: ' + str(path))

        after = before
        label = 'unchanged'
        if action == 'install':
            if span is not None:
                after = raw[:span[0]] + block + raw[span[1]:]
                new_entry = dict(entry)
                label = 'update' if after != before else 'unchanged'
            else:
                legacy = legacy_span(raw)
                if legacy is not None:
                    section = raw[legacy[0]:legacy[1]]
                    if not adopt_hash or sha(section) != adopt_hash:
                        raise ValueError('unmarked ae-sdd section requires explicit reviewed --adopt-sha256; found ' + sha(section) + ': ' + str(path))
                    after = raw[:legacy[0]] + block + raw[legacy[1]:]
                    new_entry = {'prefix': '', 'suffix': '', 'adopted': True}
                    label = 'adopt-reviewed-section'
                else:
                    prefix = b'\n\n' if raw else b''
                    suffix = b'\n'
                    after = raw + prefix + block + suffix
                    new_entry = {'prefix': prefix.hex(), 'suffix': suffix.hex(), 'adopted': False}
                    label = 'insert'
                new_entry['preInstallFileHash'] = sha(raw)
            new_entry.update(markerId=MARKER, sha256=sha(block))
            entries[str(path)] = new_entry
        elif action == 'uninstall' and entry is not None and span is not None:
            start, end = span
            prefix = bytes.fromhex(entry.get('prefix', ''))
            suffix = bytes.fromhex(entry.get('suffix', ''))
            if prefix and raw[:start].endswith(prefix):
                start -= len(prefix)
            if suffix and raw[end:].startswith(suffix):
                end += len(suffix)
            after = raw[:start] + raw[end:]
            label = 'remove-owned-region'
        elif action == 'uninstall' and entry is None:
            label = 'preserve-unowned'
        elif action not in {'install', 'uninstall'}:
            raise ValueError('unsupported action')
        if before != after:
            changes.append((path, before, after))
        descriptions.append({'path': str(path), 'action': label})
    if action == 'install':
        state = dict(schemaVersion=1, suite='ae-sdd', markerId=MARKER, targets=entries)
        record_after = (json.dumps(state, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    else:
        record_after = None
    if record_before != record_after:
        changes.append((record, record_before, record_after))
    return dict(action=action, targets=descriptions, record=str(record), changes=changes,
                snapshots=snapshots)


def replace(path, data):
    if data is None:
        path.unlink(missing_ok=True)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.ae-sdd-guidance-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def apply(prepared, backup_dir):
    if not prepared['changes']:
        return {'written': [], 'backup': None}
    record = Path(prepared['record'])
    backup_dir = Path(backup_dir).resolve()
    if any(backup_dir == p or backup_dir.is_relative_to(p) for p in prepared['snapshots']):
        raise ValueError('backup directory overlaps target files')
    record.parent.mkdir(parents=True, exist_ok=True)
    lock = record.with_suffix('.lock')
    try:
        descriptor = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError as exc:
        raise ValueError('guidance update already locked; inspect active writer') from exc
    written = []
    backup = backup_dir / ('ae-sdd-guidance-' + uuid.uuid4().hex)
    try:
        os.close(descriptor)
        for path, before in prepared['snapshots'].items():
            if read(path) != before:
                raise ValueError('concurrent change; no apply: ' + str(path))
        backup.mkdir(parents=True)
        actions = []
        for index, (path, before, after) in enumerate(prepared['changes']):
            backup_file = backup / f'{index}.bak'
            if before is not None:
                backup_file.write_bytes(before)
                if backup_file.read_bytes() != before:
                    raise ValueError('backup verification failed')
            actions.append(dict(path=str(path), backup=backup_file.name if before is not None else None,
                                before=sha(before) if before is not None else None,
                                after=sha(after) if after is not None else None))
        (backup / 'actions.json').write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
        for path, before, after in prepared['changes']:
            if read(path) != before:
                raise ValueError('concurrent change during apply: ' + str(path))
            replace(path, after)
            written.append((path, before, after))
    except (OSError, ValueError) as exc:
        conflicts = []
        for path, before, after in reversed(written):
            try:
                if read(path) != after:
                    conflicts.append(str(path))
                else:
                    replace(path, before)
            except OSError:
                conflicts.append(str(path))
        raise ValueError(f'{exc}; backup={backup}; unrecovered={conflicts}') from exc
    finally:
        lock.unlink()
    return {'written': [str(path) for path, _, _ in written], 'backup': str(backup)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'uninstall'])
    parser.add_argument('--agents', required=True, type=Path)
    parser.add_argument('--mirror', action='append', default=[], type=Path)
    parser.add_argument('--record', required=True, type=Path)
    parser.add_argument('--template', type=Path, default=TEMPLATE)
    parser.add_argument('--adopt-sha256', help='explicitly take ownership of the reviewed unmarked ae-sdd section')
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--backup-dir', type=Path)
    args = parser.parse_args()
    try:
        prepared = plan(args.action, args.agents, args.mirror, args.record, args.template, args.adopt_sha256)
        result = {key: prepared[key] for key in ('action', 'targets', 'record')}
        result['dryRun'] = not args.apply
        result['changedFiles'] = [str(path) for path, _, _ in prepared['changes']]
        if args.apply:
            if args.backup_dir is None:
                raise ValueError('--apply requires an explicit external --backup-dir')
            result.update(apply(prepared, args.backup_dir))
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (OSError, ValueError) as exc:
        print(json.dumps({'error': str(exc)}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
