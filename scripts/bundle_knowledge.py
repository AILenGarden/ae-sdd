#!/usr/bin/env python3
"""Build ae-sdd's private knowledge capability from its registered source."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
from pathlib import Path
import re
import tempfile
from urllib.parse import unquote, urlsplit

import yaml

SUITE = Path(__file__).resolve().parents[1]
REGISTRY = SUITE.parents[2] / 'registry.yaml'
DESTINATION = SUITE / 'skills/ae-sdd/capabilities/al-knowledge'
MANIFEST = '.bundle.json'
EXCLUDED = {'.git', '.codex-plugin', 'agents', '__pycache__', '.pytest_cache'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def registered_source(registry=REGISTRY):
    registry = Path(registry).resolve()
    data = yaml.safe_load(registry.read_text(encoding='utf-8-sig'))
    entries = [e for e in data['capabilities'] if e['id'] == 'al-knowledge' and e.get('enabled')]
    if len(entries) != 1:
        raise ValueError('one enabled al-knowledge source is required')
    source = (registry.parent / entries[0]['packageRoot']).resolve()
    if not source.is_relative_to(registry.parent) or not (source / 'SKILL.md').is_file():
        raise ValueError('invalid canonical knowledge source')
    return source


def payload(source):
    source = Path(source).resolve()
    files = {}
    for path in sorted(source.rglob('*')):
        relative = path.relative_to(source)
        if any(part in EXCLUDED for part in relative.parts) or path.suffix == '.pyc':
            continue
        if path.is_symlink() or not path.resolve().is_relative_to(source):
            raise ValueError('linked source is not supported: ' + str(relative))
        if not path.is_file():
            continue
        name = 'CAPABILITY.md' if relative.as_posix() == 'SKILL.md' else relative.as_posix()
        if name == MANIFEST or name.endswith('/SKILL.md'):
            raise ValueError('unexpected nested public entry in knowledge source')
        data = path.read_bytes()
        if path.suffix in {'.md', '.py', '.yaml', '.yml', '.json', '.txt'} or name == '.gitattributes':
            data = data.replace(b'\r\n', b'\n')
        files[name] = data
    if 'CAPABILITY.md' not in files or 'scripts/knowledge.py' not in files:
        raise ValueError('incomplete knowledge capability')
    files['.gitattributes'] = b'* text=auto eol=lf\n' + files.get('.gitattributes', b'')
    validate_links(files)
    files[MANIFEST] = (json.dumps({'schema_version': 1, 'owner': 'ae-sdd',
                                 'source_capability': 'al-knowledge',
                                 'files': {name: sha(data) for name, data in files.items()}},
                                ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    return files


def validate_links(files):
    """Reject missing instruction resources before any generated file is written."""
    for name, content in files.items():
        if name != 'CAPABILITY.md' and not (name.startswith('references/') and name.endswith('.md')):
            continue
        for target in re.findall(r'\]\(([^)\s]+)\)', content.decode('utf-8-sig')):
            link = urlsplit(target)
            if link.scheme or link.netloc or not link.path:
                continue
            path = unquote(link.path)
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(name), path))
            if path.startswith('/') or '\\' in path or resolved == '..' or resolved.startswith('../'):
                raise ValueError(f'capability resource escapes package: {name} -> {target}')
            if resolved not in files and not any(other.startswith(resolved.rstrip('/') + '/') for other in files):
                raise ValueError(f'missing capability resource: {name} -> {target}')


def targets(root, files):
    result = {}
    for name in files:
        path = root / name
        if Path(name).is_absolute() or '..' in Path(name).parts or not path.resolve().is_relative_to(root) or path.is_symlink():
            raise ValueError('unsafe generated path: ' + name)
        result[name] = path
    return result


def build(source=None, destination=DESTINATION, verify=False):
    source = Path(source or registered_source()).resolve()
    root = Path(destination).resolve()
    if root == source or root.is_relative_to(source) or source.is_relative_to(root):
        raise ValueError('generated output must not overlap source')
    files = payload(source)
    paths = targets(root, files)
    previous = {}
    record = root / MANIFEST
    if record.exists():
        meta = json.loads(record.read_text(encoding='utf-8'))
        if set(meta) != {'schema_version', 'owner', 'source_capability', 'files'} or meta.get('schema_version') != 1 or meta.get('owner') != 'ae-sdd' or meta.get('source_capability') != 'al-knowledge' or not isinstance(meta.get('files'), dict):
            raise ValueError('invalid bundle ownership record')
        previous = meta['files']
        targets(root, previous)
    existing = {p.relative_to(root).as_posix(): p for p in root.rglob('*') if p.is_file() and '__pycache__' not in p.parts} if root.exists() else {}
    unknown = set(existing) - set(files) - set(previous)
    if unknown:
        raise ValueError('unknown files preserved; reconcile output: ' + ', '.join(sorted(unknown)))
    obsolete = set(previous) - set(files)
    if obsolete:
        raise ValueError('obsolete generated files preserved; use a reviewed fresh destination: ' + ', '.join(sorted(obsolete)))
    changed = []
    snapshots = {}
    for name, path in paths.items():
        current = path.read_bytes() if path.exists() else None
        snapshots[name] = current
        if current != files[name]:
            changed.append(name)
        if not verify and current is not None and name != MANIFEST and current != files[name] and sha(current) != previous.get(name):
            raise ValueError('edited or unowned generated file preserved: ' + name)
    if verify:
        if changed:
            raise ValueError('bundle missing or differs from canonical source: ' + ', '.join(changed))
        return {'verified': len(files), 'changed': []}
    if not changed:
        return {'verified': len(files), 'changed': []}
    # Validate all before writing. Each replacement is atomic, not the bundle.
    written = []
    try:
        for name in changed:
            path = paths[name]
            if (path.read_bytes() if path.exists() else None) != snapshots[name]:
                raise ValueError('concurrent output change: ' + name)
            path.parent.mkdir(parents=True, exist_ok=True)
            fd, temp = tempfile.mkstemp(prefix='.bundle-', dir=path.parent)
            try:
                with os.fdopen(fd, 'wb') as stream:
                    stream.write(files[name])
                os.replace(temp, path)
                written.append(name)
            finally:
                if os.path.exists(temp):
                    os.unlink(temp)
    except (OSError, ValueError) as exc:
        raise ValueError(f'{exc}; already written: {written}; preserve and reconcile before retry') from exc
    return {'verified': len(files), 'changed': written}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=DESTINATION)
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(build(destination=args.output, verify=args.verify), ensure_ascii=False, indent=2))
        return 0
    except (ValueError, OSError) as exc:
        print(json.dumps({'error': str(exc)}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
