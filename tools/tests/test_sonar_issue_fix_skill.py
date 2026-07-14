"""Contract tests for the Sonar issue remediation child skill."""
from __future__ import annotations

import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SONAR_SKILL = (
    REPO_ROOT
    / "source"
    / "skill-fallbacks"
    / "skills"
    / "phase3-review"
    / "sonar-issue-fix-skill.full.md"
)
SONAR_RULES = (
    REPO_ROOT
    / "source"
    / "standards"
    / "review"
    / "sonar-issue-fix-rules.md"
)
CODE_REVIEW = (
    REPO_ROOT
    / "source"
    / "skill-fallbacks"
    / "skills"
    / "phase3-review"
    / "code-review-skill.full.md"
)
ROOT_SKILL = REPO_ROOT / "source" / "skill-fallbacks" / "SKILL.full.md"
UPDATE_SKILL = (
    REPO_ROOT
    / "source"
    / "skill-fallbacks"
    / "skills"
    / "orchestration"
    / "ae-sdd-update-skill.full.md"
)
DESIGN = REPO_ROOT / "source" / "docs" / "ae-sdd-design.md"
README = REPO_ROOT / "README.md"


def _read(path: Path) -> str:
    if not path.is_file():
        raise AssertionError(f"required Sonar contract artifact is missing: {path}")
    return path.read_text(encoding="utf-8")


class TestSonarIssueFixSkill(unittest.TestCase):
    """TC-01 through TC-19 are enforced as source-level runtime contracts."""

    def test_child_declares_modes_deduplication_and_unknown_rule_fallback(self):
        text = _read(SONAR_SKILL)
        self.assertRegex(text, r"(?m)^name:\s*sonar-issue-fix\s*$")
        for mode in ("upstream-edit", "registry", "reasoned", "manual"):
            self.assertIn(f"`{mode}`", text)
        self.assertIn("issueKey", text)
        self.assertIn("去重", text)
        self.assertRegex(text, r"未知规则.*(?:reasoned|manual)")

    def test_edit_plan_is_bounded_stale_safe_and_atomic(self):
        text = _read(SONAR_SKILL)
        for field in ("baseSha256", "range", "newText", "ruleKey", "provenance"):
            self.assertIn(field, text)
        for guard in ("路径逃逸", "陈旧", "重叠", "多文件"):
            self.assertIn(guard, text)
        self.assertRegex(text, r"(?s)baseSha256.*(?:不一致|失配).*skip")
        self.assertRegex(text, r"(?s)多文件.*(?:原子|全部拒绝|整批拒绝)")

    def test_upstream_quick_fix_requires_actual_payload(self):
        text = _read(SONAR_SKILL)
        self.assertIn("quickFix", text)
        self.assertRegex(text, r"(?s)quickFix.*(?:提示|标志).*(?:不等于|不能替代).*TextEdit")
        self.assertRegex(text, r"(?s)upstream-edit.*TextEdit")

    def test_registry_contains_only_evidenced_safe_recipe(self):
        text = _read(SONAR_RULES)
        self.assertIn("java:S1128", text)
        self.assertIn("unused import", text.lower())
        self.assertIn("analyzer", text.lower())
        self.assertIn("前置条件", text)
        self.assertIn("负例", text)
        self.assertIn("skip", text)
        self.assertRegex(text, r"(?s)java:S1128.*(?:单条|一条).*import")

    def test_security_and_license_boundaries_forbid_blind_or_copied_fixes(self):
        skill = _read(SONAR_SKILL)
        rules = _read(SONAR_RULES)
        combined = skill + "\n" + rules
        for category in ("taint", "hotspot", "认证", "密码学", "并发", "事务", "公共 API"):
            self.assertIn(category, combined)
        self.assertRegex(combined, r"(?:安全|security).*(?:manual|禁止自动|不得盲改)")
        self.assertIn("Sonar Source-Available License v1.0", combined)
        self.assertRegex(combined, r"(?:不得|禁止).*(?:复制|移植).*(?:SonarJava|分析器)")
        self.assertRegex(combined, r"(?:token|令牌).*(?:环境变量|env)")

    def test_closure_requires_compile_tests_rescan_and_regression_checks(self):
        text = _read(SONAR_SKILL)
        for evidence in ("compile", "test", "Sonar"):
            self.assertIn(evidence, text)
        self.assertRegex(text, r"原 issue.*(?:消失|不存在)")
        self.assertRegex(text, r"(?:新增|回归).*(?:Blocker|Critical|阻断)")
        self.assertRegex(text, r"(?:无法验证|验证失败).*(?:unverified|failed|不得报.*fixed)")

    def test_code_review_invokes_sonar_exactly_once_at_the_closeout_boundary(self):
        text = _read(CODE_REVIEW)
        marker = "SONAR_CLOSEOUT_CALL"
        self.assertEqual(text.count(marker), 1)
        step_six = text.index("## 第六步：循环判定")
        call = text.index(marker)
        step_seven = text.index("## 第七步：", step_six)
        self.assertLess(step_six, call)
        self.assertLess(call, step_seven)
        window = text[call : call + 1800]
        self.assertIn("sonar-issue-fix-skill.md", window)
        self.assertRegex(window, r"(?:恰好|只能|仅调用)\s*一次")
        self.assertIn("N/A", window)
        self.assertRegex(window, r"同一.*评审会话.*(?:不得|不再).*第二次")

    def test_indexes_counts_and_design_document_the_new_child(self):
        root = _read(ROOT_SKILL)
        update = _read(UPDATE_SKILL)
        readme = _read(README)
        design = _read(DESIGN)
        for text in (root, update, readme, design):
            self.assertIn("sonar-issue-fix-skill", text)
        self.assertIn("29 个子 SKILL", readme)
        self.assertIn("source/skills/`(29 个子 SKILL)", update)
        self.assertRegex(update, r"sonar-issue-fix-skill\.md.*Sonar")
        self.assertRegex(design, r"(?s)Sonar.*第六步.*第七步.*(?:一次|恰好一次)")

    def test_changed_source_reopens_verification_without_recursive_reentry(self):
        skill = _read(SONAR_SKILL)
        review = _read(CODE_REVIEW)
        combined = skill + "\n" + review
        self.assertRegex(combined, r"(?:改动|修改).*源码.*(?:重新|重开).*(?:测试|验证|review|评审)")
        self.assertRegex(combined, r"同一.*评审会话.*(?:不得|不再).*第二次")


if __name__ == "__main__":
    unittest.main(verbosity=2)
