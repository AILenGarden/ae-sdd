import importlib.util
import re
import tempfile
import unittest
from pathlib import Path
from shutil import copytree


ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


VALIDATOR = load_module("al_spec_validate_registry", ROOT / "scripts" / "validate_registry.py")
REGISTRY = load_module("al_spec_registry", ROOT / "scripts" / "registry.py")


class RegistryTests(unittest.TestCase):
    def test_built_in_registry_is_valid(self):
        self.assertEqual(VALIDATOR.main([str(ROOT)]), 0)

    def test_built_in_registry_contains_the_three_current_default_specs(self):
        index = REGISTRY.build_index(ROOT)
        self.assertEqual(
            [item["id"] for spec_type in ("dr", "story", "testcase")
             for item in index["categories"][spec_type]],
            ["dr", "story", "testcase"],
        )
        self.assertTrue(all(item["scope"] == "global" for values in index["categories"].values() for item in values))

    def test_each_spec_type_has_an_independent_skill_and_project_templates_are_mapped(self):
        expected_global = {"dr", "story", "testcase"}
        expected_project = set()
        registered = {entry["id"]: entry for entry in REGISTRY.load_registry(ROOT)["specs"]}
        self.assertEqual(set(registered), expected_global | expected_project)
        self.assertTrue(expected_global.issubset(registered))
        self.assertTrue(expected_project.issubset(registered))
        self.assertEqual(registered["story"]["scope"], "global")
        self.assertEqual(registered["dr"]["scope"], "global")
        self.assertEqual(registered["testcase"]["scope"], "global")
        self.assertNotIn("life-story", registered)
        self.assertNotIn("life-dr", registered)
        for skill_id in expected_global | expected_project:
            skill_path = ROOT / registered[skill_id]["path"]
            self.assertTrue(skill_path.is_file(), skill_path)
            self.assertTrue(skill_path.read_text(encoding="utf-8").startswith("---\n"))
        for skill_id in expected_project:
            self.assertEqual(registered[skill_id]["scope"], "project")
            self.assertEqual(registered[skill_id]["project_id"], "icec-cloud-life")
            self.assertIn("template", registered[skill_id]["metadata"])
            self.assertIn("reference_standard", registered[skill_id]["metadata"])
            self.assertIn("transcription", registered[skill_id]["metadata"])

    def test_each_skill_is_self_contained_and_requires_its_template_context(self):
        registered = {entry["id"]: entry for entry in REGISTRY.load_registry(ROOT)["specs"]}
        expected = {
            "dr": ["templates/global/dr.md"],
            "story": ["templates/global/story.md"],
            "testcase": ["templates/global/testcase.md"],
        }
        required_anchors = {
            "dr": ["写前必须加载的上下文", "DR", "DEC-*", "数据与状态", "交付前审查"],
            "story": ["写前必须加载的上下文", "写作约束", "Story", "Given/When/Then", "交付前检查"],
            "testcase": ["写前必须加载的上下文", "TestCase", "L1 单元", "L2 接口", "TC-*", "交付前检查"],
        }
        for spec_id, templates in expected.items():
            entry = registered[spec_id]
            content = (ROOT / entry["path"]).read_text(encoding="utf-8")
            self.assertTrue(
                "Required context" in content
                or "Required context and template selection" in content
                or "Context and template" in content
                or "写前必须加载的上下文" in content
            )
            self.assertTrue(
                "writing guidance" in content
                or "authoritative" in content
                or "This skill defines" in content
                or "defines how" in content
                or "behavior contract" in content
                or "behavior specification" in content
                or "Extract the behavior" in content
                or "写作约束" in content
                or "Story 写作方法" in content
                or "写作约束" in content
            )
            if "## Required context" in content:
                context_marker = "## Required context"
            elif "## 1. Required context and source selection" in content:
                context_marker = "## 1. Required context and source selection"
            elif "## 写前必须加载的上下文" in content:
                context_marker = "## 写前必须加载的上下文"
            else:
                context_marker = "## Context and template"
            required_context = content.split(context_marker, 1)[1].split("##", 1)[0]
            self.assertNotRegex(required_context, r"standards/(global|project)/[^`\\s]+.*(?:must|required|first)")
            for template in templates:
                self.assertTrue((ROOT / template).is_file(), template)
                self.assertIn(template, content)
            if spec_id in {"dr", "story", "testcase"}:
                self.assertTrue("Project template match" in content or "If the registry maps the project" in content or "mapped project template" in content or "templates/project/" in content)
                self.assertTrue("Global template fallback" in content or "Otherwise load" in content or "otherwise" in content or "templates/global/" in content)
            for anchor in required_anchors[spec_id]:
                self.assertIn(anchor, content, f"{spec_id} missing anchor {anchor}")
        self.assertEqual(
            registered["story"]["metadata"]["project_templates"]["icec-cloud-life"],
            "templates/project/life-story.md",
        )
        self.assertEqual(
            registered["dr"]["metadata"]["project_templates"]["icec-cloud-life"],
            "templates/project/life-dr.md",
        )
        self.assertTrue((ROOT / "templates/project/life-story.md").is_file())
        self.assertTrue((ROOT / "templates/project/life-dr.md").is_file())
        self.assertTrue((ROOT / "templates/project/life-testcase.md").is_file())
        self.assertFalse((ROOT / "skills/life-story/SKILL.md").exists())
        self.assertFalse((ROOT / "skills/life-dr/SKILL.md").exists())
        self.assertFalse((ROOT / "skills/life-testcase/SKILL.md").exists())
        self.assertNotIn("life-story", registered)
        self.assertNotIn("life-dr", registered)
        self.assertNotIn("life-testcase", registered)
        self.assertEqual(
            registered["testcase"]["metadata"]["project_templates"]["icec-cloud-life"],
            "templates/project/life-testcase.md",
        )

    def test_templates_do_not_require_external_standards(self):
        for path in list((ROOT / "templates" / "global").glob("*.md")) + list((ROOT / "templates" / "project").glob("life-*.md")):
            content = path.read_text(encoding="utf-8")
            self.assertNotRegex(content, r"Load [`']?standards/")

    def test_templates_do_not_contain_skill_loading_instructions(self):
        paths = list((ROOT / "templates" / "global").glob("*.md")) + list((ROOT / "templates" / "project").glob("*.md"))
        forbidden = re.compile(r"(?i)\bSKILL\b|使用说明|fallback template|required context|load the invoking|selected by .*project")
        for path in paths:
            self.assertIsNone(forbidden.search(path.read_text(encoding="utf-8")), path)

    def test_current_scope_excludes_deferred_prd_and_coding_plan(self):
        registry = REGISTRY.load_registry(ROOT)
        ids = {entry["id"] for entry in registry["specs"]}
        self.assertNotIn("prd", ids)
        self.assertNotIn("coding-plan", ids)
        self.assertFalse((ROOT / "skills" / "prd").exists())
        self.assertFalse((ROOT / "skills" / "coding-plan").exists())
        self.assertFalse((ROOT / "templates" / "global" / "prd.md").exists())
        self.assertFalse((ROOT / "templates" / "global" / "coding-plan.md").exists())

    def test_project_entries_require_exact_project_filter(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            copytree(ROOT / "framework", root / "framework")
            REGISTRY.register_spec(
                root,
                spec_id="billing-story",
                name="Billing Story",
                spec_type="story",
                scope="project",
                project_id="billing",
                path="specs/billing/story.md",
            )
            global_index = REGISTRY.build_index(root)
            self.assertEqual([item["id"] for item in global_index["categories"]["story"]], ["story"])
            billing_index = REGISTRY.build_index(root, project_id="billing")
            self.assertEqual([item["id"] for item in billing_index["categories"]["story"]], ["story", "billing-story"])
            other_index = REGISTRY.build_index(root, project_id="other")
            self.assertEqual([item["id"] for item in other_index["categories"]["story"]], ["story"])

    def test_register_update_disable_remove_round_trip(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            copytree(ROOT / "framework", root / "framework")
            entry = REGISTRY.register_spec(
                root, spec_id="checkout-story", name="Checkout Story", spec_type="story",
                scope="project", project_id="checkout", path="specs/checkout/story.md", tags=["checkout"],
            )
            self.assertEqual(entry["project_id"], "checkout")
            REGISTRY.update_spec(root, "checkout-story", {"description": "Story", "enabled": False})
            self.assertEqual(
                [item["id"] for item in REGISTRY.build_index(root, project_id="checkout")["categories"]["story"]],
                ["story"],
            )
            self.assertEqual(
                [item["id"] for item in REGISTRY.build_index(root, project_id="checkout", include_disabled=True)["categories"]["story"]],
                ["story", "checkout-story"],
            )
            removed = REGISTRY.remove_spec(root, "checkout-story")
            self.assertEqual(removed["name"], "Checkout Story")

    def test_duplicate_identity_is_rejected_without_write(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            copytree(ROOT / "framework", root / "framework")
            before = (root / "framework" / "registry.yaml").read_text(encoding="utf-8")
            with self.assertRaises(REGISTRY.RegistryError):
                REGISTRY.register_spec(root, spec_id="another-id", name="Story", spec_type="story", path="x.md")
            self.assertEqual(before, (root / "framework" / "registry.yaml").read_text(encoding="utf-8"))

    def test_invalid_scope_and_project_id_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            copytree(ROOT / "framework", root / "framework")
            with self.assertRaises(REGISTRY.RegistryError):
                REGISTRY.register_spec(root, spec_id="bad-project", name="Bad", spec_type="dr", scope="project", path="x.md")
            with self.assertRaises(REGISTRY.RegistryError):
                REGISTRY.register_spec(root, spec_id="bad-id", name="Bad 2", spec_type="dr", scope="global", project_id="billing", path="x.md")

    def test_uri_reference_is_not_checked_as_a_local_file(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            copytree(ROOT / "framework", root / "framework")
            REGISTRY.register_spec(root, spec_id="external-dr", name="External DR", spec_type="dr", path="https://example.test/dr.md")
            item = next(item for item in REGISTRY.build_index(root)["categories"]["dr"] if item["id"] == "external-dr")
            self.assertEqual(item["reference_kind"], "reference")
            self.assertIsNone(item["available"])


if __name__ == "__main__":
    unittest.main()
