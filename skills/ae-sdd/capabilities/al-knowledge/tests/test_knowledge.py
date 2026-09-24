"""Behavioral tests using isolated, executable project evidence."""
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import yaml

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('knowledge', ROOT / 'scripts/knowledge.py')
k = importlib.util.module_from_spec(spec)
spec.loader.exec_module(k)
NOW = '2026-09-22T10:00:00+08:00'


def put(path, meta, body='\n# Evidence\n'):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text('---\n' + yaml.safe_dump(meta, allow_unicode=True, sort_keys=False) + '---\n' + body,
                    encoding='utf-8', newline='\n')


def concept(kind, ident, **al):
    return dict(type=kind, title=ident, description='Fixture ' + ident,
                al=dict(profile=k.PROFILE, id=ident, **al))


def make_project(root):
    root.mkdir(parents=True, exist_ok=True)
    (root / 'app.py').write_text('def cancel(status):\n    if status != "draft":\n        raise ValueError("invalid state")\n    return "cancelled"\n', encoding='utf-8', newline='\n')
    (root / 'spec.md').write_text('# Cancellation\nOnly draft appointments can be cancelled.\n', encoding='utf-8', newline='\n')
    bundle = root / '.al-knowledge'
    put(bundle / 'project.md', concept('Project Context', 'PROJECT', project_id='fixture', scan_scope=['cancel'], excluded_scope=['other services'], coverage='partial', repositories={'main': 'Fixture source repository'}))
    for name in ('app', 'spec'):
        source = root / (name + ('.py' if name == 'app' else '.md'))
        put(bundle / 'references' / f'{name}.md', concept('Reference', 'REF-' + name, source=dict(repository='main', path=source.name, sha256=k.digest(source.read_bytes()), captured_at=NOW, symbol='cancel' if name == 'app' else 'Cancellation')))
    sources = [dict(id='spec', resource='../references/spec.md'), dict(id='code', resource='../references/app.md')]
    story = concept('Specification', 'STORY', spec_id='STORY-001', spec_type='Story', claims=[dict(id='rule', anchor='rule', evidence_state='unknown', source_ids=['spec'])])
    story['sources'] = sources[:1]
    put(bundle / 'specs/story.md', story, '\n<a id="rule"></a>\n# Cancellation rule\nOnly draft appointments can be cancelled.[^spec]\n\n[^spec]: Source specification.\n')
    api = concept('API Endpoint', 'API', interface=dict(service='appointments', protocol='HTTP', signature='POST /appointments/{id}/cancel'), entry='CANCEL', nodes=[dict(id='CANCEL', title='cancel(status)', kind='method', symbol='fixture.app.cancel(str)'), dict(id='EVENT', title='Cancellation event', kind='boundary', stop_reason='asynchronous fixture boundary')], calls=[dict(id='publish', type='publishes', **{'from': 'CANCEL', 'to': 'EVENT'}, condition='only if publishing is introduced', evidence_state='planned', source_ids=[])], relations=[dict(id='implementation', type='implements', target='../specs/story.md#rule', target_id='STORY', evidence_state='confirmed', coverage='full', source_ids=['spec', 'code'])])
    api['sources'] = sources
    body = '\n# Cancellation endpoint\nAccepts only draft. HTTP binding is a fictional fixture, not a deployed API.\n'
    checked = dict(by='process:fixture-author', at=NOW, body_sha256=k.text_hash(body.lstrip('\n')), source_sha256={s['id']: k.digest((bundle / 'interfaces' / s['resource']).resolve().read_bytes()) for s in sources})
    api['al']['relations'][0]['checked'] = checked
    put(bundle / 'interfaces/cancel.md', api, body)
    return bundle


class KnowledgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.project = Path(self.temp.name) / 'project'
        self.bundle = make_project(self.project)

    def load(self, repos=True):
        return k.Bundle(self.bundle, {'main': self.project} if repos else {}).load()

    def change(self, relative, fn):
        path = self.bundle / relative
        meta, body = k.split_document(path.read_bytes())
        fn(meta)
        put(path, meta, body)

    def codes(self, bundle):
        return {issue['code'] for issue in bundle.issues}

    def snapshot(self):
        return {str(p): p.read_bytes() for p in self.project.rglob('*') if p.is_file()}

    def test_valid_bundle_and_real_source_behavior(self):
        bundle = self.load()
        self.assertFalse([i for i in bundle.issues if i['level'] == 'error'])
        self.assertFalse(bundle.changed)
        ns = {}
        exec((self.project / 'app.py').read_text(), ns)
        self.assertEqual(ns['cancel']('draft'), 'cancelled')
        with self.assertRaises(ValueError):
            ns['cancel']('confirmed')

    def test_external_okf_is_readable_without_profile_or_index(self):
        other = self.project / 'external'
        put(other / 'custom.md', {'type': 'Unknown external type', 'vendor': {'preserve': True}}, '[not written](missing.md)\n')
        b = k.Bundle(other).load()
        self.assertTrue(b.report()['okf_valid'])
        self.assertIn('broken-link', self.codes(b))
        self.assertEqual(b.docs[other / 'custom.md']['meta']['vendor'], {'preserve': True})

    def test_external_al_extension_is_not_interpreted_as_our_profile(self):
        other = self.project / 'external'
        put(other / 'custom.md', {'type': 'External', 'al': ['vendor data']})
        b = k.Bundle(other).load()
        self.assertTrue(b.report()['okf_valid'])
        self.assertFalse([i for i in b.issues if i['level'] == 'error'])
        self.assertEqual(b.docs[other / 'custom.md']['meta']['al'], ['vendor data'])

    def test_yaml_duplicate_key_cannot_replace_evidence(self):
        (self.bundle / 'bad.md').write_text('---\ntype: Reference\ntype: Other\n---\n', encoding='utf-8')
        self.assertFalse(self.load().report()['okf_valid'])

    def test_malformed_yaml_and_missing_type_report(self):
        (self.bundle / 'bad.md').write_text('---\ntype: [\n---\n', encoding='utf-8')
        put(self.bundle / 'no-type.md', {'title': 'No type'})
        self.assertTrue({'frontmatter', 'type'} <= self.codes(self.load()))

    def test_reserved_files_and_datetime_constraints(self):
        put(self.bundle / 'specs/index.md', {'type': 'Bad index'})
        (self.bundle / 'log.md').write_text('# Log\n## 2026-02-31\n- changed\n', encoding='utf-8')
        self.assertTrue({'reserved-frontmatter', 'log-date'} <= self.codes(self.load()))

    def test_duplicate_ids_and_missing_project(self):
        put(self.bundle / 'copy.md', concept('Business Concept', 'STORY'))
        (self.bundle / 'project.md').unlink()
        self.assertTrue({'duplicate-id', 'project'} <= self.codes(self.load()))

    def test_local_claim_verification_does_not_make_document_verified(self):
        original = self.snapshot()
        b = self.load()
        self.assertNotIn('verified', b.ids['STORY']['meta'])
        self.assertEqual(original, self.snapshot())

    def test_false_business_claim_is_not_certified_by_structure(self):
        self.change('specs/story.md', lambda m: m.update(title='Every status can cancel'))
        report = self.load().report()
        self.assertTrue(report['okf_valid'])
        self.assertIn('no claim of factual', report['limitation'])

    def test_many_to_many_and_planned_mapping(self):
        api = copy.deepcopy(self.load().ids['API']['meta'])
        api['al']['id'] = 'API-SECOND'
        api['al']['nodes'] = []
        api['al']['calls'] = []
        put(self.bundle / 'interfaces/second.md', api)
        b = self.load()
        output = k.render(b)[self.project / '.al-knowledge-views/spec-code-map.md']
        self.assertIn('API-SECOND', output)
        self.assertIn('API / implements', output)

    def test_hierarchy_cycles_and_multiple_parents(self):
        for ident in ('A', 'B', 'C'):
            put(self.bundle / f'{ident}.md', concept('Specification', ident))
        for origin, target in [('A', 'B'), ('B', 'A'), ('C', 'B')]:
            self.change(origin + '.md', lambda m, t=target: m['al'].update(relations=[dict(id='owns', type='contains', target=t + '.md', target_id=t, evidence_state='confirmed', source_ids=[])]))
        self.assertTrue({'multiple-parents', 'contains-cycle'} <= self.codes(self.load()))

    def test_planned_for_never_counts_as_confirmed_implementation(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['relations'][0].update(type='planned-for'))
        self.assertIn('planned-state', self.codes(self.load()))

    def test_target_identity_and_anchor_are_checked(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['relations'][0].update(target='../specs/story.md#missing'))
        self.assertIn('relation-target', self.codes(self.load()))

    def test_source_changed_without_commit_or_expiry_change(self):
        self.change('references/app.md', lambda m: m.update(stale_after='2099-01-01T00:00:00Z'))
        (self.project / 'app.py').write_text('def cancel(status): return "cancelled"\n', encoding='utf-8')
        b = self.load()
        self.assertIn('source-changed', self.codes(b))
        self.assertIn('interfaces/cancel.md', b.report()['possibly_affected'] + b.report()['directly_changed'])

    def test_changed_body_invalidates_checked(self):
        path = self.bundle / 'interfaces/cancel.md'
        path.write_text(path.read_text(encoding='utf-8') + '\nNew claim.\n', encoding='utf-8')
        self.assertIn('body-changed', self.codes(self.load()))

    def test_source_unavailable_is_not_unchanged(self):
        (self.project / 'app.py').unlink()
        self.assertIn('source-unavailable', self.codes(self.load()))
        self.assertIn('repository-unmapped', self.codes(self.load(False)))

    def test_source_path_traversal_is_rejected(self):
        self.change('references/app.md', lambda m: m['al']['source'].update(path='../secret'))
        self.assertIn('source-path', self.codes(self.load()))

    def test_read_only_commands_preserve_every_byte(self):
        before = self.snapshot()
        for command in ('check', 'inspect', 'impact', 'render'):
            with contextlib.redirect_stdout(io.StringIO()):
                code = k.main([command, str(self.bundle), '--repo', 'main=' + str(self.project)])
            self.assertEqual(code, 0)
            self.assertEqual(before, self.snapshot())

    def test_generation_is_idempotent_and_source_preserving(self):
        sources = self.snapshot()
        b = self.load()
        first = k.write_outputs(b, k.render(b))
        self.assertTrue(first)
        b2 = self.load()
        self.assertEqual(k.write_outputs(b2, k.render(b2)), [])
        for path, content in sources.items():
            self.assertEqual(Path(path).read_bytes(), content)
        (self.project / '.al-knowledge-views/spec-tree.md').unlink()
        self.assertTrue(k.write_outputs(self.load(), k.render(self.load())))

    def test_unmanaged_or_edited_output_blocks_all_writes(self):
        (self.bundle / 'index.md').write_text('# My navigation\n', encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'unmanaged'):
            k.write_outputs(self.load(), k.render(self.load()))
        self.assertEqual(before, self.snapshot())

    def test_edited_generated_view_is_preserved(self):
        b = self.load()
        k.write_outputs(b, k.render(b))
        target = self.project / '.al-knowledge-views/spec-tree.md'
        target.write_text(target.read_text(encoding='utf-8') + 'human note\n', encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'edited'):
            k.write_outputs(self.load(), k.render(self.load()))
        self.assertEqual(before, self.snapshot())

    def test_edited_generated_frontmatter_is_preserved(self):
        b = self.load()
        k.write_outputs(b, k.render(b))
        path = self.bundle / 'index.md'
        path.write_text(path.read_text(encoding='utf-8').replace('"0.2"', '"0.1"'), encoding='utf-8')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'edited'):
            k.write_outputs(self.load(), k.render(self.load()))
        self.assertEqual(before, self.snapshot())

    def test_invalid_implements_type_and_missing_rule_anchor(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['relations'][0].update(target='../specs/story.md'))
        self.assertIn('implements-contract', self.codes(self.load()))
        self.change('interfaces/cancel.md', lambda m: m.update(type='Business Concept'))
        self.assertIn('implements-contract', self.codes(self.load()))

    def test_invalid_enum_types_and_numeric_test_source_report(self):
        self.change('interfaces/cancel.md', lambda m: m.update(status=[]))
        self.change('interfaces/cancel.md', lambda m: m['al'].update(test_runs=[dict(result='passed', source_ids=[123], at=NOW, by='tester', command='false')]))
        b = self.load()
        self.assertTrue({'status', 'test-run'} <= self.codes(b))

    def test_underlying_source_change_marks_generated_mapping_stale(self):
        (self.project / 'app.py').write_text('# changed\n', encoding='utf-8')
        b = self.load()
        self.assertIn(self.bundle / 'interfaces/cancel.md', b.changed)
        view = k.render(b)[self.project / '.al-knowledge-views/spec-code-map.md']
        self.assertIn('implements/implementation [stale', view)

    def test_new_document_during_render_is_detected(self):
        b = self.load()
        outputs = k.render(b)
        put(self.bundle / 'new.md', {'type': 'External'})
        with self.assertRaisesRegex(ValueError, 'document set changed'):
            k.write_outputs(b, outputs)

    def test_moved_concept_refreshes_old_index(self):
        put(self.bundle / 'old/topic.md', {'type': 'Other'})
        b = self.load()
        k.write_outputs(b, k.render(b))
        (self.bundle / 'new').mkdir()
        (self.bundle / 'old/topic.md').rename(self.bundle / 'new/topic.md')
        b = self.load()
        k.write_outputs(b, k.render(b))
        self.assertNotIn('(topic.md)', (self.bundle / 'old/index.md').read_text(encoding='utf-8'))
        self.assertIn('(topic.md)', (self.bundle / 'new/index.md').read_text(encoding='utf-8'))

    def test_shared_entry_impacts_all_registered_interfaces(self):
        put(self.bundle / 'interfaces/second.md', concept('API Endpoint', 'SECOND', interface=dict(service='s', protocol='CLI', signature='cancel'), entry='CANCEL'))
        b = self.load()
        self.assertIn('interfaces/second.md', b.impact({self.bundle / 'interfaces/cancel.md'}))

    def test_nonstring_relationship_type_blocks_render_with_json(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['relations'][0].update(type=[]))
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(k.main(['render', str(self.bundle)]), 1)
        self.assertFalse(json.loads(output.getvalue())['profile_valid'])

    def test_nonstring_call_target_blocks_render_with_json(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['calls'][0].update(to=[]))
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(k.main(['render', str(self.bundle)]), 1)
        self.assertIn('call-target', {i['code'] for i in json.loads(output.getvalue())['issues']})

    def test_unmapped_source_is_explicit_in_view(self):
        b = self.load(False)
        self.assertIn(self.bundle / 'interfaces/cancel.md', b.uncertain)
        output = k.render(b)[self.project / '.al-knowledge-views/spec-code-map.md']
        self.assertIn('current source unchecked', output)

    def test_bad_field_shapes_return_json_without_traceback(self):
        path = self.bundle / 'interfaces/cancel.md'
        original = path.read_bytes()
        for key in ('relations', 'claims', 'nodes', 'calls', 'test_runs'):
            for value in (None, 'bad', [123], {'id': 'bad'}):
                path.write_bytes(original)
                self.change('interfaces/cancel.md', lambda m, k=key, v=value: m['al'].update({k: v}))
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    code = k.main(['check', str(self.bundle)])
                self.assertEqual(code, 1, (key, value, output.getvalue()))
                self.assertFalse(json.loads(output.getvalue())['profile_valid'])

    def test_changed_source_between_read_and_write_is_detected(self):
        b = self.load()
        outputs = k.render(b)
        path = self.bundle / 'project.md'
        path.write_text(path.read_text(encoding='utf-8') + 'human note\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'source changed'):
            k.write_outputs(b, outputs)
        self.assertFalse((self.bundle / '.render.lock').exists())

    def test_lock_refuses_concurrent_writer(self):
        (self.bundle / '.render.lock').write_text('another writer', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'lock exists'):
            k.write_outputs(self.load(), k.render(self.load()))
        self.assertEqual((self.bundle / '.render.lock').read_text(), 'another writer')

    def test_partial_failure_reports_written_and_keeps_sources(self):
        b = self.load()
        real_replace = k.os.replace
        count = 0

        def fail_second(source, target):
            nonlocal count
            count += 1
            if count == 2:
                raise OSError('fixture disk failure')
            return real_replace(source, target)

        with mock.patch.object(k.os, 'replace', side_effect=fail_second):
            with self.assertRaisesRegex(ValueError, 'already written:.*index.md'):
                k.write_outputs(b, k.render(b))
        self.assertFalse((self.bundle / '.render.lock').exists())
        self.assertFalse(list(self.project.rglob('.knowledge-*')))
        self.assertTrue(k.write_outputs(self.load(), k.render(self.load())))

    def test_recursion_sharing_and_async_are_finite_and_visible(self):
        self.change('interfaces/cancel.md', lambda m: m['al']['calls'].append(dict(id='retry', type='calls', **{'from': 'CANCEL', 'to': 'CANCEL'}, evidence_state='inferred', source_ids=[], condition='recursive retry candidate')))
        put(self.bundle / 'interfaces/another.md', concept('API Endpoint', 'SECOND', interface=dict(service='appointments', protocol='CLI', signature='cancel'), entry='CANCEL'))
        b = self.load()
        output = k.render(b)[self.project / '.al-knowledge-views/spec-code-map.md']
        self.assertIn('↩ CANCEL', output)
        self.assertIn('↪ CANCEL', output)
        self.assertIn('publishes/publish [planned]', output)
        self.assertIn('asynchronous fixture boundary', output)

    def test_testcase_definition_is_not_test_success(self):
        self.change('specs/story.md', lambda m: m['al'].update(test_runs=[dict(result='passed', source_ids=[])]))
        self.assertIn('test-evidence', self.codes(self.load()))

    def test_cli_missing_bundle_and_invalid_write_are_errors(self):
        for args in (['check', str(self.project / 'missing')], ['check', str(self.bundle), '--write']):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(k.main(args), 2)
            self.assertIn('error', json.loads(output.getvalue()))

    def test_real_cli_subprocess_returns_json(self):
        result = subprocess.run([sys.executable, str(ROOT / 'scripts/knowledge.py'), 'check', str(self.bundle), '--repo', 'main=' + str(self.project)], capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['profile_valid'])


if __name__ == '__main__':
    unittest.main()
