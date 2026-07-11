"""🆕 v3.9.3 CLI-level tests for work-item isolated state files.

BREAKING v3.9.3 变更：
  - 废除 v3.8.2 双段（{ID}--{name}）目录命名
  - 废除 --name 形参
  - 目录名统一走 R6 顶层名（PRD-/DR-/Story-/Task-）
  - session.json 与 state.json 共用 R6 顶层目录
  - cmd_state_new 强制 R2 向上归入（递归替父级建 state）

v3.8.2 旧测试已被替换为 v3.9.3 新行为测试。
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CLI = str(Path(__file__).resolve().parent.parent / "bin" / "ae-sdd")


def _setup_project() -> Path:
    tmp = Path(tempfile.mkdtemp())
    (tmp / ".ae-sdd" / "assets").mkdir(parents=True, exist_ok=True)
    (tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: life\n", encoding="utf-8")
    (tmp / ".ae-sdd" / "assets" / "life.assets.md").write_text(
        f"# §A §B §C §D §E §F §G\n\n| gitPath | `{tmp}` |\n| docWorkspacePath | `{tmp}` |\n",
        encoding="utf-8",
    )
    return tmp


def _run_cli(cwd: Path, *args: str) -> tuple[int, str, str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(Path(__file__).resolve().parent.parent.parent)
    r = subprocess.run(
        [sys.executable, CLI, *args],
        capture_output=True, text=True, cwd=str(cwd), env=env, encoding="utf-8",
    )
    return r.returncode, r.stdout, r.stderr


# ─── 🆕 v3.9.3 新行为测试 ────────────────────────────────────────────────────
class TestCmdStateNewV393(unittest.TestCase):
    """🆕 v3.9.3 cmd_state_new 走 R6 顶层名 + R2 向上归入。"""

    def test_state_new_story_no_name_uses_r6(self):
        """v3.9.3 state new --id STORY-006-BE --entry-node STORY -> 顶层 Story-006（无 --name）。
        🆕 v3.10.1 目录名带 UUID 前缀：{uuid}-Story-006。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # R6 业务名 = Story-006（workItemKey 仍返回纯业务名给用户）
        self.assertEqual(payload["workItemKey"], "Story-006")
        # 🆕 v3.10.1：statePath 含 UUID 前缀目录名，用后缀匹配验证
        self.assertTrue(payload["statePath"].endswith("-Story-006" + os.sep + "state.json"),
                        msg=f"statePath={payload['statePath']}")
        sp = Path(payload["statePath"])
        self.assertTrue(sp.is_file())
        # stateMachineId 带 UUID 前缀，stateMachineName 是纯业务名
        data = json.loads(sp.read_text(encoding="utf-8"))
        self.assertTrue(data["stateMachineId"].endswith("-Story-006"))
        self.assertEqual(data["stateMachineName"], "Story-006")
        self.assertIn("stateUuid", data)

    def test_state_new_dr_no_name_uses_r6(self):
        """v3.9.3 state new --id DR-005 --entry-node DR -> 顶层 DR-005。
        🆕 v3.10.1 目录名带 UUID 前缀：{uuid}-DR-005。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "DR-005",
            "--entry-node", "DR",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # 🆕 v3.10.1：用后缀匹配验证 statePath 含 -DR-005 目录
        self.assertTrue(payload["statePath"].endswith("-DR-005" + os.sep + "state.json"),
                        msg=f"statePath={payload['statePath']}")
        sp = Path(payload["statePath"])
        self.assertTrue(sp.is_file())

    def test_state_new_task_bug_uses_bug_prefix(self):
        """🆕 v3.10.0：state new --id BUG-LIFE-001 -> 顶层 Bug-BUG-LIFE-001（微任务无文档）。
        旧版用 Task- 前缀，v3.10.0 Route 下移后 BUG 走 Bug- 前缀。
        🆕 v3.10.1 目录名带 UUID 前缀：{uuid}-Bug-BUG-LIFE-001。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "BUG-LIFE-001",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # 🆕 v3.10.1：扁平 state 也带 UUID 前缀，用后缀匹配验证
        self.assertTrue(payload["statePath"].endswith("-Bug-BUG-LIFE-001" + os.sep + "state.json"),
                        msg=f"statePath={payload['statePath']}")
        sp = Path(payload["statePath"])
        self.assertTrue(sp.is_file())
        data = json.loads(sp.read_text(encoding="utf-8"))
        self.assertEqual(data["scale"], "微", "BUG/OPT/CONFIG 独立任务默认必须走微链")

    def test_state_new_story_defaults_to_medium_scale(self):
        """🆕 v3.10.0：Story 入口是中链（Route 下移：大=DR、中=Story、小=CodingPlan）。
        🆕 v3.10.1 目录名带 UUID 前缀。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        sp = Path(payload["statePath"])
        self.assertTrue(sp.is_file())
        data = json.loads(sp.read_text(encoding="utf-8"))
        self.assertEqual(data["scale"], "中")

    def test_state_new_legacy_name_flag_warns_and_ignores(self):
        """v3.9.3 --name 形参废除（传了被忽略，不报错）。
        🆕 v3.10.1 目录名带 UUID 前缀。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--name", "应被忽略",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # 不应创建双段目录
        legacy = tmp / ".auto-engineering" / "STORY-006-BE--应被忽略" / "state.json"
        self.assertFalse(legacy.is_file())
        # 仍走 R6（🆕 v3.10.1 带 UUID 前缀，用后缀匹配验证）
        sp = Path(payload["statePath"])
        self.assertTrue(sp.is_file())
        self.assertTrue(sp.parent.name.endswith("-Story-006"),
                        msg=f"dir name={sp.parent.name}")

    def test_state_new_story_with_parent_dr_doc_absorbs(self):
        """v3.9.3 Story 有父级 DR + DR 文档存在关联性对 → Story 嵌进 DR 嵌套 state。"""
        tmp = _setup_project()
        # 创建设计文档
        design = tmp / "design"
        design.mkdir()
        (design / "DR-005-some-title.md").write_text(
            "# DR-005\n\n## Story 拆分\n\n- STORY-006-BE\n- STORY-007-BE\n",
            encoding="utf-8",
        )
        (design / "STORY-006-BE-Story.md").write_text(
            "# STORY-006-BE\n\n## 元信息\n\n- Story ID: STORY-006-BE\n- 来源 DR: DR-005\n",
            encoding="utf-8",
        )
        # 配置 docWorkspacePath
        cfg = (tmp / ".ae-sdd" / "config.yaml")
        cfg.write_text(f"projectKey: life\ndocWorkspacePath: {tmp}\n", encoding="utf-8")

        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # Story 嵌进 DR-005 嵌套 state
        self.assertIn("DR-005", payload["statePath"])

    def test_state_new_story_with_dr_parent_prd_doc_absorbs_into_prd_root(self):
        """Story -> DR -> PRD resolves to the PRD root state, not a DR-local state."""
        tmp = _setup_project()
        design = tmp / "design"
        design.mkdir()
        (design / "PRD-001-product.md").write_text(
            "# PRD-001\n\n## DR split\n\n- DR-005\n",
            encoding="utf-8",
        )
        (design / "DR-005-some-title.md").write_text(
            "# DR-005\n\n## Meta\n\n- PRD: PRD-001\n\n## Story split\n\n- STORY-006-BE\n",
            encoding="utf-8",
        )
        (design / "STORY-006-BE-Story.md").write_text(
            "# STORY-006-BE\n\n## Meta\n\n- Story ID: STORY-006-BE\n- Source DR: DR-005\n- DR: DR-005\n",
            encoding="utf-8",
        )
        cfg = (tmp / ".ae-sdd" / "config.yaml")
        cfg.write_text(f"projectKey: life\ndocWorkspacePath: {tmp}\n", encoding="utf-8")

        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertIn("PRD-001", payload["statePath"])
        # 🆕 v3.10.1：DR-005 不再独立建顶层目录（嵌进 PRD state 的 drStates）
        auto_eng = tmp / ".auto-engineering"
        dr_dirs = [d for d in auto_eng.iterdir() if d.is_dir() and d.name.endswith("-DR-005")]
        self.assertEqual(len(dr_dirs), 0, msg=f"DR-005 should not have its own dir: {dr_dirs}")
        self.assertFalse((tmp / ".ae-sdd" / "state.json").exists())
        # PRD state 目录名带 UUID 前缀，用后缀匹配定位
        prd_dirs = [d for d in auto_eng.iterdir() if d.is_dir() and d.name.endswith("-PRD-001")]
        self.assertEqual(len(prd_dirs), 1, msg=f"expected 1 PRD-001 dir, got {prd_dirs}")
        prd_state = json.loads((prd_dirs[0] / "state.json").read_text(encoding="utf-8"))
        self.assertEqual(prd_state["prdState"]["prdId"], "PRD-001")
        self.assertIn("DR-005", prd_state.get("drStates", {}))
        self.assertIn("STORY-006-BE", prd_state["drStates"]["DR-005"].get("storyStates", {}))


# ─── v3.8.2 旧测试全部 SKIPPED（BREAKING 变更）────────────────────────────────
class TestStateActiveMirrorRegression(unittest.TestCase):

    def test_state_read_uses_existing_legacy_active_work_item(self):
        """activeWorkItem may contain an existing v3.8 two-segment directory key."""
        tmp = _setup_project()
        work_item = "STORY-004-BE--车主端预约单操作-BE"
        state_file = tmp / ".auto-engineering" / work_item / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        nested = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": "STORY-004-BE",
            "storyStates": {"STORY-004-BE": {"phase": "story-generated"}},
            "workItemKey": work_item,
        }
        state_file.write_text(json.dumps(nested, ensure_ascii=False), encoding="utf-8")
        mirror = dict(nested)
        mirror["activeWorkItem"] = work_item
        mirror["activeStatePath"] = str(state_file)
        (tmp / ".ae-sdd" / "state.json").write_text(
            json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--json")

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertEqual(payload["activeStory"], "STORY-004-BE")

    def test_state_write_without_story_updates_nested_active_story(self):
        """state write should advance activeStory in nested legacy work-item states."""
        tmp = _setup_project()
        work_item = "STORY-004-BE--车主端预约单操作-BE"
        story_id = "STORY-004-BE"
        state_file = tmp / ".auto-engineering" / work_item / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        nested = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": story_id,
            "storyStates": {story_id: {"phase": "story-generated"}},
            "workItemKey": work_item,
            "scale": "中",
        }
        state_file.write_text(json.dumps(nested, ensure_ascii=False), encoding="utf-8")
        mirror = dict(nested)
        mirror["activeWorkItem"] = work_item
        mirror["activeStatePath"] = str(state_file)
        (tmp / ".ae-sdd" / "state.json").write_text(
            json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(
            tmp, "state", "write",
            "--phase", "story-reviewed",
            "--allow-empty-memory",
        )

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        updated = json.loads(state_file.read_text(encoding="utf-8"))
        self.assertEqual(updated["storyStates"][story_id]["phase"], "story-reviewed")
        self.assertNotEqual(updated.get("phase"), "story-reviewed")


# ─── 🆕 v3.9.7 fallback-trap-fix ─────────────────────────────────────────────

class TestStateMirrorMissingFallback(unittest.TestCase):
    """🆕 v3.9.7: 镜像文件不存在时 CLI 仍能读 work-item 源。

    背景：life 项目 owner 实测追问 '.ae-sdd/state.json 作为镜像的反模式'，
    决定删掉镜像让 .auto-engineering/Story-004/state.json 当唯一源。
    v3.9.7 fallback-trap-fix 改造 _active_state_from_mirror：
    - 镜像缺失时不立即 return None
    - 改扫 .auto-engineering/*/state.json，按 mtime 选最近活跃
    - health 'state.json 可读' → 'state.json 可定位'（镜像或源任一即可）
    """

    def _setup_v397_work_item(self, work_item_dir: str, story_id: str = "STORY-004-BE") -> tuple[Path, Path]:
        """构造镜像缺失、源存在的最小项目。返回 (tmp, work_item_state_path)。"""
        tmp = _setup_project()
        # 构造 work-item 源（R6 顶层名）
        work_item_dir_path = tmp / ".auto-engineering" / work_item_dir
        work_item_dir_path.mkdir(parents=True, exist_ok=True)
        state_file = work_item_dir_path / "state.json"
        nested = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": story_id,
            "workItemKey": work_item_dir,
            "stateMachineId": work_item_dir,
            "scale": "中",
            "history": [
                {"phase": "nested-state-init(entryNode=STORY)",
                 "timestamp": "2026-07-08T00:00:00Z", "by": "ae-sdd"},
            ],
            "storyStates": {story_id: {
                "phase": "story-generated",
                "completedSteps": [],
                "codingRound": 0,
                "lastUpdated": "2026-07-08T00:00:00Z",
                "resetHistory": [],
            }},
        }
        state_file.write_text(json.dumps(nested, ensure_ascii=False), encoding="utf-8")
        # 🔑 故意不创建 .ae-sdd/state.json（模拟镜像已删场景）
        (tmp / ".ae-sdd" / "state.json").unlink(missing_ok=True)
        assert not (tmp / ".ae-sdd" / "state.json").exists()
        return tmp, state_file

    def test_state_read_falls_back_to_work_item_source_when_mirror_missing(self):
        """核心回归：镜像缺失时 'state read' 应返回 work-item 源的 state.json"""
        tmp, state_file = self._setup_v397_work_item("Story-004")
        code, out, err = _run_cli(tmp, "state", "read", "--json")
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # 应从源读到 story-generated，而不是 default v1 state (phase=initialized)
        self.assertEqual(payload["version"], "2", "应读嵌套 state 而非 v1 default")
        self.assertEqual(payload["stateMachineId"], "Story-004")
        self.assertEqual(payload["activeStory"], "STORY-004-BE")
        nested_phase = payload["storyStates"]["STORY-004-BE"]["phase"]
        self.assertEqual(nested_phase, "story-generated",
                         "应读到 Story-004 子状态的真实 phase，不是 default 'initialized'")

    def test_state_next_step_uses_work_item_source_when_mirror_missing(self):
        """镜像缺失时 next-step 应按 work-item 源的 phase 推荐下一步"""
        tmp, state_file = self._setup_v397_work_item("Story-004")
        code, out, err = _run_cli(tmp, "state", "next-step", "--json")
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # 真值是 story-generated → next=story-reviewed
        # 不是 'initialized → ra-generated'（default v1 推荐）
        self.assertEqual(payload["current"], "story-generated",
                         "应读 work-item 源的当前 phase 而非 default 'initialized'")
        self.assertEqual(payload["next"], "testcase-generated")  # v3.10.1 子系列合并

    def test_health_passes_when_mirror_missing_but_source_exists(self):
        """镜像缺失但 work-item 源存在时 'state.json 可定位' 应 pass（即便其他 1 项 fail）"""
        tmp, state_file = self._setup_v397_work_item("Story-004")
        code, out, err = _run_cli(tmp, "health", "--json")
        payload = json.loads(out)
        state_check = next((it for it in payload["items"] if it["name"] == "work-item state.json 可定位"), None)
        self.assertIsNotNone(state_check, "应改名后 'work-item state.json 可定位' 检查项")
        self.assertTrue(state_check["pass"],
                        f"'state.json 可定位' 应通过；msg={state_check.get('message')}")
        self.assertIn("work-item", state_check.get("message", "").lower())
        # 其他项（如 master-freshness）可能因测试 project 缺 .githooks/ 而 fail，
        # 但与镜像 fallback 无关——只断言目标项，不强求 code==0

    def test_health_fails_when_both_mirror_and_source_missing(self):
        """镜像 + 源都缺失时 health 仍合理 fail（项目未初始化）"""
        tmp = _setup_project()
        # 没创建任何 work-item 源
        (tmp / ".ae-sdd" / "state.json").unlink(missing_ok=True)
        code, out, err = _run_cli(tmp, "health", "--json")
        payload = json.loads(out)
        # health 仍可能 1 项 fail（master-freshness），但 work-item state 检查应 fail
        state_check = next((it for it in payload["items"] if it["name"] == "work-item state.json 可定位"), None)
        self.assertIsNotNone(state_check)
        self.assertFalse(state_check["pass"],
                         "镜像和源都缺失时，'state.json 可定位' 应 fail")


class TestParallelWorkItemIsolation(unittest.TestCase):
    """多 work-item 时禁止默认消费全局 mirror，避免 A/B 会话串状态。"""

    def _write_work_item(self, tmp: Path, name: str, story: str, phase: str) -> Path:
        state_file = tmp / ".auto-engineering" / name / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "stateMachineId": name,
            "workItemKey": name,
            "currentWorkItem": name,
            "scale": "中",
            "activeStory": story,
            "storyStates": {story: {
                "phase": phase,
                "completedSteps": [],
                "codingRound": 0,
                "lastUpdated": "2026-07-09T00:00:00Z",
                "resetHistory": [],
            }},
            "history": [],
        }
        state_file.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        return state_file

    def test_state_read_without_work_item_resolves_single_active_ignoring_completed(self):
        """completed 的 work-item 不应再占据隐式候选池：仅 1 个活跃态时应直接解析，
        不应被已完成的 Story-005 拖成假性歧义（life 项目 2026-07-08/09 故障复现场景）。"""
        tmp = _setup_project()
        sp_a = self._write_work_item(tmp, "Story-004", "STORY-004-BE", "testcase-generated")
        self._write_work_item(tmp, "Story-005", "STORY-005-BE", "completed")
        mirror = json.loads(sp_a.read_text(encoding="utf-8"))
        mirror["activeWorkItem"] = "Story-004"
        mirror["activeStatePath"] = str(sp_a)
        (tmp / ".ae-sdd" / "state.json").write_text(json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--json")

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertEqual(payload["stateMachineId"], "Story-004")

    def test_state_read_resolves_work_item_with_completed_active_story_and_pending_sibling(self):
        """同一 work-item 内 activeStory 指向的 Story 已 completed，但还有别的 Story
        未完成时，隐式解析仍应命中该 work-item，不应因误判"整体已完结"而报
        NoWorkItemStateError（v3.9.18 修复，复现 Fix A 的多 Story 场景）。"""
        tmp = _setup_project()
        state_file = self._write_work_item(tmp, "Story-004", "STORY-004-A", "completed")
        data = json.loads(state_file.read_text(encoding="utf-8"))
        data["storyStates"]["STORY-004-B"] = {
            "phase": "coding",
            "completedSteps": [],
            "codingRound": 0,
            "lastUpdated": "2026-07-09T00:00:00Z",
            "resetHistory": [],
        }
        state_file.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--json")

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertEqual(payload["stateMachineId"], "Story-004")

    def test_state_read_without_work_item_blocks_when_multiple_active_sources_exist(self):
        """两个都还在跑（非 completed）的 work-item 仍应保持歧义拒绝，不允许隐式猜测。"""
        tmp = _setup_project()
        sp_a = self._write_work_item(tmp, "Story-004", "STORY-004-BE", "testcase-generated")
        self._write_work_item(tmp, "Story-005", "STORY-005-BE", "story-generated")
        mirror = json.loads(sp_a.read_text(encoding="utf-8"))
        mirror["activeWorkItem"] = "Story-004"
        mirror["activeStatePath"] = str(sp_a)
        (tmp / ".ae-sdd" / "state.json").write_text(json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--json")

        self.assertNotEqual(code, 0, msg="多个活跃 work-item 时不应静默读取全局 active mirror")
        self.assertIn("--work-item", err + out)
        self.assertIn("Story-004", err + out)
        self.assertIn("Story-005", err + out)

    def test_state_read_explicit_work_item_reads_target_not_mirror(self):
        tmp = _setup_project()
        sp_a = self._write_work_item(tmp, "Story-004", "STORY-004-BE", "testcase-generated")
        self._write_work_item(tmp, "Story-005", "STORY-005-BE", "completed")
        mirror = json.loads(sp_a.read_text(encoding="utf-8"))
        mirror["activeWorkItem"] = "Story-004"
        mirror["activeStatePath"] = str(sp_a)
        (tmp / ".ae-sdd" / "state.json").write_text(json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--work-item", "Story-005", "--json")

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertEqual(payload["stateMachineId"], "Story-005")
        self.assertEqual(payload["activeStory"], "STORY-005-BE")

    def test_state_write_completed_syncs_story_projection(self):
        tmp = _setup_project()
        state_file = self._write_work_item(tmp, "Story-005", "STORY-005-BE", "code-reviewed")
        data = json.loads(state_file.read_text(encoding="utf-8"))
        sub = data["storyStates"]["STORY-005-BE"]
        sub["currentPhase"] = "coding"
        sub["currentStep"] = "step-5-task-review-passed-awaiting-human-confirm"
        sub["pendingOutputs"] = {"humanConfirm": True}
        sub["codingRound"] = "r0"
        state_file.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(
            tmp,
            "state", "write",
            "--work-item", "Story-005",
            "--phase", "completed",
            "--allow-empty-memory",
        )

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        updated = json.loads(state_file.read_text(encoding="utf-8"))
        sub = updated["storyStates"]["STORY-005-BE"]
        self.assertEqual(sub["phase"], "completed")
        self.assertEqual(sub["currentPhase"], "completed")
        self.assertEqual(sub["currentStep"], "completed")
        self.assertEqual(sub["pendingOutputs"], {})
        self.assertEqual(sub["codingRound"], 1)
        self.assertIn("step-5-task-review-passed-awaiting-human-confirm", sub["completedSteps"])


class TestStateWorkItemIsolationLegacy(unittest.TestCase):
    """🆕 v3.9.3 BREAKING：v3.8.2 双段 + --name 形参已废除。

    旧测试全部 SKIPPED，新行为见 TestCmdStateNewV393。
    """

    def test_legacy_double_segment(self):
        self.skipTest("v3.9.3 BREAKING: v3.8.2 双段目录已废除")

    def test_legacy_name_param(self):
        self.skipTest("v3.9.3 BREAKING: --name 形参已废除")

    def test_legacy_project_state_behavior(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除，新行为见 TestCmdStateNewV393")

    def test_legacy_two_work_items(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除")

    def test_legacy_state_read_work_item(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除")


class TestStateNewUuidPrefix(unittest.TestCase):
    """🆕 v3.10.1 state new 创建的 state 带 UUID 前缀 + 防重复创建。"""

    def test_state_new_produces_uuid_prefixed_fields(self):
        """state new 创建后 stateMachineId 带 UUID 前缀，stateMachineName 纯业务名。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-007-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-007-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        sp = Path(payload["statePath"])
        data = json.loads(sp.read_text(encoding="utf-8"))
        # stateMachineId = {uuid}-Story-007
        self.assertTrue(data["stateMachineId"].endswith("-Story-007"))
        # stateMachineName = 纯业务名
        self.assertEqual(data["stateMachineName"], "Story-007")
        # stateUuid 存在且是 36 字符
        self.assertEqual(len(data["stateUuid"]), 36)
        # 目录名 == stateMachineId
        self.assertEqual(sp.parent.name, data["stateMachineId"])

    def test_state_new_same_business_name_blocked(self):
        """🆕 v3.10.1 同业务名重复创建被防重复机制拦截（UUID 前缀不影响撞名检测）。"""
        tmp = _setup_project()
        # 第一次创建
        code1, out1, err1 = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-008-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-008-BE",
            "--json",
        )
        self.assertEqual(code1, 0, msg=f"stdout={out1}\nstderr={err1}")
        # 第二次同业务名创建 -> 应被拦截
        code2, out2, err2 = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-008-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-008-BE",
        )
        self.assertNotEqual(code2, 0, msg=f"应被拦截 stdout={out2}\nstderr={err2}")

    def test_state_new_force_overwrites(self):
        """🆕 v3.10.1 --force 可覆盖已存在的同业务名 state。"""
        tmp = _setup_project()
        _run_cli(tmp, "state", "new",
                 "--id", "STORY-009-BE",
                 "--entry-node", "STORY",
                 "--story-ids", "STORY-009-BE",
                 "--json")
        # --force 覆盖
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-009-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-009-BE",
            "--force",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")

    def test_state_read_finds_uuid_prefixed_by_business_name(self):
        """🆕 v3.10.1 state read 按业务名能找到 UUID 前缀目录的 state。"""
        tmp = _setup_project()
        # 创建
        code1, out1, err1 = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-010-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-010-BE",
            "--json",
        )
        self.assertEqual(code1, 0, msg=f"stdout={out1}\nstderr={err1}")
        # 按业务名 Story-010 读取
        code2, out2, err2 = _run_cli(
            tmp, "state", "read",
            "--work-item", "Story-010",
            "--json",
        )
        self.assertEqual(code2, 0, msg=f"stdout={out2}\nstderr={err2}")
        payload = json.loads(out2)
        self.assertEqual(payload["stateMachineName"], "Story-010")


if __name__ == "__main__":
    unittest.main(verbosity=2)
