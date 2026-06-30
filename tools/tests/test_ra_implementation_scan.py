import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCANNER = REPO_ROOT / "scripts" / "ra_implementation_scan.py"


def _write_project(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp())
    for rel, content in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    return root


def _complete_ra() -> str:
    return """# RA-IMPL-v1

## §9-quater 实现视角七要素

### §9-quater.1 数据源清单
| 类型 | 名称 | 读/写 | owner | 权威源 | 证据 |
|------|------|-------|-------|--------|------|
| DB 表 | im_session | 读写 | IM 服务 | DB | assets/db/schema.md |
| API 接口 | GET /api/session | 读 | IM 服务 | API | controller path |
| MQ 事件 | SessionUpdated | 写 | IM 服务 | MQ | topic definition |
| Redis 缓存 | session:{id} | 读写 | IM 服务 | DB | cache config |

### §9-quater.2 数据流链路
| 来源 | 入口 | 处理 | 落点 | 输出 | 事务/一致性 | 观测 |
|------|------|------|------|------|-------------|------|
| 前端客户端 -> API | GET /api/session | SessionService 领域处理 | DB im_session / Redis 缓存 / MQ | JSON 响应 | DB 事务内写，缓存失效后重建 | 日志/指标/审计 |

### §9-quater.3 术语/定义/不变量
| 术语 | 定义 | 字段/枚举/状态 | 不变量 | 单位/空值/ID | 权威源 |
|------|------|----------------|--------|--------------|--------|
| 会话 | 客服和用户的一次沟通 | status=open/closed | 同一 sessionId 唯一 | ID 为雪花；closedAt 可 null | im_session |

### §9-quater.4 现有实现/复用证据
| 对象 | 代码/路径/class/method/表/API/assets/git 证据 | 结论 |
|------|-----------------------------------------------|------|
| SessionService | src/main/java/SessionService.java, im_session 表, git grep session | 复用并改造 |
| RoutingAPI | controller path /api/session | 新建适配层 |

### §9-quater.5 高成本/难实现设计反驳
| 方案 | 成本/风险 | 不采用理由 | 替代/更低成本方案 |
|------|-----------|------------|-------------------|
| 重建实时数仓 | 高成本且难实现，影响 MQ 和缓存一致性 | 当前需求只需会话级查询 | 分阶段复用现有 DB + Redis，后续再异步扩展 |

### §9-quater.6 开发者疑问答复矩阵
| 开发者问题 | 答案/答复 | 证据 | 状态 | 是否阻断 DR |
|------------|-----------|------|------|------------|
| sessionId 从哪里来？ | 由现有 im_session 主键生成 | DB 表和代码路径 | 已解决 | 否 |
| 缓存何时失效？ | 更新事务提交后删除 Redis key | cache config | 已解决 | 否 |

### §9-quater.7 DR 生成交接包
| DR 输入 | 内容 |
|---------|------|
| 接口/API | GET /api/session |
| 数据模型/表 | im_session + Redis session:{id} |
| 状态/事务/一致性 | open/closed 状态，DB 事务后发 MQ |
| 非功能/性能/权限 | P95 200ms，客服权限校验 |
| 测试/验收 | API 测试、缓存失效测试、状态流转验收 |
| 迁移/回滚/灰度 | 无历史迁移，灰度按租户开关，异常回滚配置 |
"""


class TestRAImplementationScan(unittest.TestCase):
    def test_complete_ra_passes(self):
        root = _write_project({"design/RA-001-v1.md": _complete_ra()})
        result = subprocess.run(
            [sys.executable, str(SCANNER), "--root", str(root), "--format", "json"],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "PASS")
        self.assertEqual(payload["raFiles"], 1)
        self.assertEqual(payload["blockers"], 0)

    def test_missing_sections_block(self):
        root = _write_project({"design/RA-001-v1.md": "# RA\n\n## 数据源清单\n只有 DB 表。\n"})
        result = subprocess.run(
            [sys.executable, str(SCANNER), "--root", str(root), "--format", "json"],
            capture_output=True, text=True, check=False,
        )
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "FAIL")
        self.assertGreaterEqual(payload["blockers"], 6)
        rules = {f["rule"] for f in payload["findings"] if f["severity"] == "BLOCKER"}
        self.assertIn("I2", rules)
        self.assertIn("I7", rules)

    def test_placeholder_blocks(self):
        bad = _complete_ra().replace("src/main/java/SessionService.java, im_session 表, git grep session", "TODO")
        root = _write_project({"design/RA-001-v1.md": bad})
        result = subprocess.run(
            [sys.executable, str(SCANNER), "--root", str(root), "--format", "json"],
            capture_output=True, text=True, check=False,
        )
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "FAIL")
        rules = {f["rule"] for f in payload["findings"] if f["severity"] == "BLOCKER"}
        self.assertIn("I4", rules)

    def test_cli_help(self):
        result = subprocess.run(
            [sys.executable, str(SCANNER), "--help"],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("implementation-view", result.stdout)


if __name__ == "__main__":
    unittest.main()
