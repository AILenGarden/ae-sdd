"""Regression checks for the public Skill surface and management references."""
import importlib.util
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class SkillDiscoveryTests(unittest.TestCase):
    def test_only_entry_is_discoverable(self):
        self.assertEqual(
            [p.relative_to(ROOT).as_posix() for p in (ROOT / 'skills').rglob('SKILL.md')],
            ['skills/ae-sdd/SKILL.md'],
        )
        self.assertFalse(list((ROOT / 'skills').rglob('plugin.json')))

    def test_entry_management_links_resolve(self):
        entry = ROOT / 'skills/ae-sdd/SKILL.md'
        for target in re.findall(r'\]\(([^)]+)\)', entry.read_text(encoding='utf-8')):
            self.assertTrue((entry.parent / target).is_file(), target)
        for operation in ('install', 'uninstall'):
            text = (ROOT / 'references' / f'{operation}.md').read_text(encoding='utf-8')
            self.assertFalse(text.startswith('---'))
            self.assertIn('## Procedure', text)

    def test_registry_has_no_management_skills(self):
        text = (ROOT.parents[2] / 'registry.yaml').read_text(encoding='utf-8')
        self.assertNotRegex(text, r'(?m)^\s*- id: ae-sdd-(?:install|uninstall)\s*$')

    def test_zcode_payload_has_no_management_entrypoints(self):
        spec = importlib.util.spec_from_file_location('packer', ROOT / 'scripts/pack-zcode-plugin.py')
        packer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packer)
        for source, target in packer.BYTE_COPIES:
            self.assertTrue(source.is_file(), str(source))
            self.assertFalse(target.endswith('/SKILL.md'), target)
        adapted = packer.build_adapted_text().decode('utf-8')
        self.assertNotIn('ae-sdd-install', adapted)
        self.assertNotIn('ae-sdd-uninstall', adapted)
        self.assertIn('(references/install.md)', adapted)


if __name__ == '__main__':
    unittest.main()
