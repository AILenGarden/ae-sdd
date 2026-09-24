"""Installation-owned guidance round trips without editing user rules."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('manage_agents', ROOT / 'scripts/manage_agents.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class GuidanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.primary = self.root / 'runtime/AGENTS.md'
        self.mirror = self.root / 'workspace/AGENTS.md'
        self.record = self.root / 'runtime/.ae-sdd-install/guidance.json'
        self.backups = self.root / 'backups'
        self.template = self.root / 'template.md'
        self.template.write_bytes(m.TEMPLATE.read_bytes())
        self.original = '# User rules\n\n## ALSDD\nPreserve this.\n\n## Git\nNever push.\n'.encode('utf-8')
        for path in (self.primary, self.mirror):
            path.parent.mkdir(parents=True)
            path.write_bytes(self.original)

    def plan(self, action='install', **kwargs):
        return m.plan(action, self.primary, [self.mirror], self.record, self.template, **kwargs)

    def install(self):
        return m.apply(self.plan(), self.backups)

    def test_dry_run_creates_nothing(self):
        planned = self.plan()
        self.assertEqual(len(planned['changes']), 3)
        self.assertEqual(self.primary.read_bytes(), self.original)
        self.assertFalse(self.record.exists())
        self.assertFalse(self.backups.exists())

    def test_install_mirror_and_idempotence(self):
        self.install()
        self.assertEqual(self.primary.read_bytes(), self.mirror.read_bytes())
        self.assertEqual(self.primary.read_bytes().count(b'ae-sdd:managed:start'), 1)
        state = self.record.read_bytes()
        self.assertEqual(self.plan()['changes'], [])
        self.assertEqual(m.apply(self.plan(), self.backups)['written'], [])
        self.assertEqual(self.record.read_bytes(), state)

    def test_uninstall_preserves_all_original_bytes(self):
        self.install()
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), self.original)
        self.assertEqual(self.mirror.read_bytes(), self.original)
        self.assertFalse(self.record.exists())
        self.assertEqual(self.plan('uninstall')['changes'], [])

    def test_crlf_and_bom_are_preserved(self):
        original = b'\xef\xbb\xbf' + self.original.replace(b'\n', b'\r\n')
        self.primary.write_bytes(original)
        self.mirror.write_bytes(original)
        self.install()
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), original)

    def test_update_replaces_only_owned_region(self):
        self.install()
        self.primary.write_bytes(b'# Later user rule\n' + self.primary.read_bytes())
        self.template.write_bytes(self.template.read_bytes().replace('查询只读'.encode(), '查询保持只读'.encode()))
        planned = self.plan()
        self.assertIn('update', [item['action'] for item in planned['targets']])
        m.apply(planned, self.backups)
        self.assertTrue(self.primary.read_bytes().startswith(b'# Later user rule\n' + self.original))
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), b'# Later user rule\n' + self.original)

    def test_manual_edit_inside_region_blocks_update_and_uninstall(self):
        self.install()
        current = self.primary.read_bytes().replace('查询只读'.encode(), '人工编辑'.encode())
        self.primary.write_bytes(current)
        for action in ('install', 'uninstall'):
            with self.assertRaisesRegex(ValueError, 'edited'):
                self.plan(action)
            self.assertEqual(self.primary.read_bytes(), current)

    def test_unmarked_section_requires_exact_reviewed_hash(self):
        raw = self.original + b'\n' + m.TITLE + '\n\n用户明确指定的入口。\n\n## Extra\nKeep extra.\n'.encode()
        for p in (self.primary, self.mirror):
            p.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, 'adopt-sha256'):
            self.plan()
        start, end = m.legacy_span(raw)
        planned = self.plan(adopt_hash=m.sha(raw[start:end]))
        m.apply(planned, self.backups)
        installed = self.primary.read_bytes()
        a, b = m.managed_span(installed)
        self.assertEqual(installed[:a], raw[:start])
        self.assertEqual(installed[b:], raw[end:])
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), raw[:start] + raw[end:])

    def test_bom_before_unmarked_title_does_not_duplicate_entry(self):
        raw = b'\xef\xbb\xbf' + m.TITLE + b'\nReviewed entry\n\n## Git\nKeep.\n'
        for path in (self.primary, self.mirror):
            path.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, 'adopt-sha256'):
            self.plan()
        start, end = m.legacy_span(raw)
        m.apply(self.plan(adopt_hash=m.sha(raw[start:end])), self.backups)
        self.assertEqual(self.primary.read_bytes().count(m.TITLE), 1)
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), raw[:start] + raw[end:])

    def test_duplicate_and_partial_markers_preserved(self):
        self.install()
        original = self.primary.read_bytes()
        for raw in (original + original, original.replace(b'ae-sdd:managed:end', b'bad:end')):
            self.primary.write_bytes(raw)
            with self.assertRaisesRegex(ValueError, 'markers'):
                self.plan('uninstall')
            self.assertEqual(self.primary.read_bytes(), raw)

    def test_marked_file_without_record_is_not_taken_over(self):
        self.install()
        self.record.unlink()
        with self.assertRaisesRegex(ValueError, 'without ownership'):
            self.plan()

    def test_uninstall_without_record_preserves_unmarked_section(self):
        raw = self.original + b'\n' + m.TITLE + b'\nUser section\n'
        self.primary.write_bytes(raw)
        self.assertEqual(self.plan('uninstall')['changes'], [])
        self.assertEqual(self.primary.read_bytes(), raw)

    def test_mirror_different_outer_rules_are_preserved(self):
        self.mirror.write_bytes(b'# Mirror rules\n')
        self.install()
        m.apply(self.plan('uninstall'), self.backups)
        self.assertEqual(self.primary.read_bytes(), self.original)
        self.assertEqual(self.mirror.read_bytes(), b'# Mirror rules\n')

    def test_changed_target_set_is_refused(self):
        self.install()
        with self.assertRaisesRegex(ValueError, 'target set'):
            m.plan('uninstall', self.primary, [], self.record, self.template)

    def test_record_cannot_claim_arbitrary_outer_text(self):
        self.install()
        state = json.loads(self.record.read_bytes())
        state['targets'][str(self.primary)]['prefix'] = self.original.hex()
        self.record.write_text(json.dumps(state), encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'whitespace'):
            self.plan('uninstall')

    def test_concurrent_change_refuses_apply(self):
        planned = self.plan()
        self.mirror.write_bytes(b'New edit\n')
        with self.assertRaisesRegex(ValueError, 'concurrent'):
            m.apply(planned, self.backups)
        self.assertEqual(self.primary.read_bytes(), self.original)
        self.assertEqual(self.mirror.read_bytes(), b'New edit\n')

    def test_partial_failure_restores_only_this_operations_writes(self):
        real_replace = m.replace
        count = 0

        def fail_once(path, value):
            nonlocal count
            count += 1
            if count == 2:
                raise OSError('fixture disk error')
            return real_replace(path, value)

        with mock.patch.object(m, 'replace', side_effect=fail_once):
            with self.assertRaisesRegex(ValueError, r'unrecovered=\[\]'):
                self.install()
        self.assertEqual(self.primary.read_bytes(), self.original)
        self.assertEqual(self.mirror.read_bytes(), self.original)
        self.assertFalse(self.record.exists())
        self.assertTrue(list(self.backups.rglob('actions.json')))

    def test_missing_marker_with_remaining_section_keeps_record(self):
        self.install()
        raw = self.primary.read_bytes()
        raw = re_sub_markers(raw)
        self.primary.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, 'marker missing'):
            self.plan('uninstall')
        self.assertTrue(self.record.exists())


def re_sub_markers(raw):
    import re
    return re.sub(rb'^<!-- ae-sdd:managed:[^\n]+\n?', b'', raw, flags=re.M)


if __name__ == '__main__':
    unittest.main()
