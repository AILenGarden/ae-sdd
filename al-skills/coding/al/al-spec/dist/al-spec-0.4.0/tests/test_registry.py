import importlib.util
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

    def test_each_spec_type_has_an_independent_skill_and_project_adapters_are_registered(self):
        expected_global = {"dr", "story", "testcase"}
        expected_project = {"life-dr", "life-story", "life-testcase"}
        registered = {entry["id"]: entry for entry in REGISTRY.load_registry(ROOT)["specs"]}
        self.assertEqual(set(registered), expected_global | expected_project)
        self.assertTrue(expected_global.issubset(registered))
        self.assertTrue(expected_project.issubset(registered))
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
            "life-dr": ["templates/global/dr.md", "templates/project/life-dr.md"],
            "life-story": ["templates/global/story.md", "templates/project/life-story.md"],
            "life-testcase": ["templates/global/testcase.md", "templates/project/life-testcase.md"],
        }
        required_anchors = {
            "dr": ["Document contract", "DEC-*", "state transitions", "rollback", "ready"],
            "story": ["Required context and source selection", "STORY-*", "field", "Given/When/Then", "ready"],
            "testcase": ["Document contract", "L1 unit", "L2 interface", "TC-*", "ready"],
            "life-dr": ["life project rules", "transcription boundary", "DEC-*", "ready"],
            "life-story": ["life boundary and contract rules", "high semantic density", "Story", "ready"],
            "life-testcase": ["life project test rules", "TC-*", "compensating evidence", "ready"],
        }
        for spec_id, templates in expected.items():
            entry = registered[spec_id]
            content = (ROOT / entry["path"]).read_text(encoding="utf-8")
            self.assertIn("Required context", content)
            self.assertTrue("writing guidance" in content or "authoritative" in content)
            context_marker = "## Required context" if "## Required context" in content else "## 1. Required context and source selection"
            required_context = content.split(context_marker, 1)[1].split("##", 1)[0]
            self.assertNotRegex(required_context, r"standards/(global|project)/[^`\\s]+.*(?:must|required|first)")
            for template in templates:
                self.assertTrue((ROOT / template).is_file(), template)
                self.assertIn(template, content)
            for anchor in required_anchors[spec_id]:
                self.assertIn(anchor, content, f"{spec_id} missing anchor {anchor}")

    def test_templates_do_not_require_external_standards(self):
        for path in list((ROOT / "templates" / "global").glob("*.md")) + list((ROOT / "templates" / "project").glob("life-*.md")):
            content = path.read_text(encoding="utf-8")
            self.assertNotRegex(content, r"Load [`']?standards/")

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
