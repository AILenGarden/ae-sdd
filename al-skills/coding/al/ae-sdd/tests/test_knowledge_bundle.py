"""Test private knowledge delivery through the ae-sdd public entry."""
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('bundle_knowledge', ROOT / 'scripts/bundle_knowledge.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class KnowledgeBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / 'ae-sdd/skills/ae-sdd/capabilities/al-knowledge'

    def snapshot(self):
        return {p.relative_to(self.output).as_posix(): p.read_bytes() for p in self.output.rglob('*') if p.is_file()}

    def test_packaged_internal_capability_has_no_discoverable_skill(self):
        builder.build(destination=self.output)
        self.assertTrue((self.output / 'CAPABILITY.md').is_file())
        self.assertFalse(list(self.output.rglob('SKILL.md')))
        self.assertFalse(list(self.output.rglob('plugin.json')))
        for doc in [self.output / 'CAPABILITY.md', *(self.output / 'references').glob('*.md')]:
            for link in re.findall(r'\]\(([^)\s]+)\)', doc.read_text(encoding='utf-8')):
                if not link.startswith(('http:', 'https:', '#')):
                    self.assertTrue((doc.parent / link.split('#')[0]).exists(), f'{doc}: {link}')

    def test_relocated_package_cli_needs_no_checkout(self):
        builder.build(destination=self.output)
        fixture = self.output / 'tests/fixtures/project'
        result = subprocess.run([sys.executable, '-X', 'utf8', str(self.output / 'scripts/knowledge.py'), 'check', str(fixture / '.al-knowledge'), '--repo', f'main={fixture}'], cwd=self.temp.name, capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(json.loads(result.stdout)['issues'], [])
        rendered = subprocess.run([sys.executable, '-X', 'utf8', str(self.output / 'scripts/knowledge.py'), 'render', str(fixture / '.al-knowledge'), '--repo', f'main={fixture}', '--write'], cwd=self.temp.name, capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(rendered.returncode, 0, rendered.stderr + rendered.stdout)
        self.assertTrue((fixture / '.al-knowledge-views/spec-code-map.md').is_file())

    def test_rebuild_is_byte_identical_and_verify_is_readonly(self):
        first = builder.build(destination=self.output)
        before = self.snapshot()
        self.assertTrue(first['changed'])
        self.assertEqual(builder.build(destination=self.output)['changed'], [])
        builder.build(destination=self.output, verify=True)
        self.assertEqual(before, self.snapshot())

    def test_edit_is_preserved_and_source_drift_reported(self):
        builder.build(destination=self.output)
        target = self.output / 'CAPABILITY.md'
        target.write_text(target.read_text(encoding='utf-8') + '\nHuman change\n', encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'edited'):
            builder.build(destination=self.output)
        with self.assertRaisesRegex(ValueError, 'differs'):
            builder.build(destination=self.output, verify=True)
        self.assertEqual(before, self.snapshot())

    def test_unknown_files_are_not_deleted(self):
        builder.build(destination=self.output)
        (self.output / 'notes.md').write_text('human notes', encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'unknown files'):
            builder.build(destination=self.output)
        self.assertEqual(before, self.snapshot())

    def test_source_change_can_refresh_owned_payload(self):
        source = Path(self.temp.name) / 'source'
        shutil.copytree(builder.registered_source(), source, ignore=shutil.ignore_patterns('__pycache__'))
        builder.build(source=source, destination=self.output)
        path = source / 'SKILL.md'
        path.write_text(path.read_text(encoding='utf-8') + '\nUpdated contract\n', encoding='utf-8')
        self.assertIn('CAPABILITY.md', builder.build(source=source, destination=self.output)['changed'])
        builder.build(source=source, destination=self.output, verify=True)

    def test_no_source_output_overlap(self):
        source = builder.registered_source()
        with self.assertRaisesRegex(ValueError, 'overlap'):
            builder.build(source=source, destination=source / 'generated')

    def test_missing_link_fails_before_writing(self):
        source = Path(self.temp.name) / 'source'
        shutil.copytree(builder.registered_source(), source, ignore=shutil.ignore_patterns('__pycache__'))
        entry = source / 'SKILL.md'
        entry.write_text(entry.read_text(encoding='utf-8') + '\nRead [required](references/missing.md).\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'missing capability resource'):
            builder.build(source=source, destination=self.output)
        self.assertFalse(self.output.exists())

    def test_escaping_link_fails_before_writing(self):
        source = Path(self.temp.name) / 'source'
        shutil.copytree(builder.registered_source(), source, ignore=shutil.ignore_patterns('__pycache__'))
        entry = source / 'SKILL.md'
        entry.write_text(entry.read_text(encoding='utf-8') + '\nRead [external](../private.md).\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'escapes'):
            builder.build(source=source, destination=self.output)
        self.assertFalse(self.output.exists())

    def test_manual_metadata_in_ownership_record_is_preserved(self):
        builder.build(destination=self.output)
        record = self.output / builder.MANIFEST
        meta = json.loads(record.read_text(encoding='utf-8'))
        meta['human_note'] = 'keep me'
        record.write_text(json.dumps(meta), encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'ownership'):
            builder.build(destination=self.output)
        self.assertEqual(before, self.snapshot())

    def test_line_endings_do_not_change_generated_payload(self):
        source = Path(self.temp.name) / 'source'
        shutil.copytree(builder.registered_source(), source, ignore=shutil.ignore_patterns('__pycache__'))
        before = builder.payload(source)
        skill = source / 'SKILL.md'
        skill.write_bytes(skill.read_bytes().replace(b'\r\n', b'\n').replace(b'\n', b'\r\n'))
        self.assertEqual(before, builder.payload(source))

    def test_zcode_package_has_same_private_payload(self):
        destination = Path(self.temp.name) / 'zcode'
        command = [sys.executable, '-X', 'utf8', str(ROOT / 'scripts/pack-zcode-plugin.py'), '--output', str(destination)]
        for extra in ([], ['--verify']):
            result = subprocess.run(command + extra, capture_output=True, text=True, encoding='utf-8')
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual([p.relative_to(destination).as_posix() for p in destination.rglob('SKILL.md')], ['skills/ae-sdd/SKILL.md'])
        builder.build(destination=destination / 'skills/ae-sdd/capabilities/al-knowledge', verify=True)

    @unittest.skipUnless(shutil.which('pwsh'), 'PowerShell adapter requires pwsh')
    def test_claude_adapter_with_parent_segments_is_self_contained(self):
        destination = str(Path(self.temp.name) / 'nested/../claude')
        result = subprocess.run(['pwsh', '-NoProfile', '-File', str(ROOT / 'scripts/pack-claude-plugin.ps1'), '-TargetRoot', destination, '-Python', sys.executable], capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        output = Path(destination).resolve()
        self.assertEqual([p.relative_to(output).as_posix() for p in output.rglob('SKILL.md')], ['SKILL.md'])
        builder.build(destination=output / 'capabilities/al-knowledge', verify=True)


if __name__ == '__main__':
    unittest.main()
