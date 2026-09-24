#!/usr/bin/env python3
"""ae-sdd → ae-sdd-zcode 打包器（ZCode 插件市场形态）。

唯一真源：脚本所在的 ae-sdd/（版本读自 .codex-plugin/plugin.json）+ ../../registry.yaml
输出：../ae-sdd-zcode/（自含本地插件市场 al-local）。

用法（任意目录执行，路径自解析）：
    python al-skills/coding/al/ae-sdd/scripts/pack-zcode-plugin.py            重建 + 自检
    python al-skills/coding/al/ae-sdd/scripts/pack-zcode-plugin.py --verify   只校验不写入

脚本拥有输出包的 skills/ 映射产物、.claude-plugin/plugin.json、marketplace.json；
BUILD.md 手维护；映射外文件只报告不删。幂等：不写时间戳，重复执行输出字节不变。
"""
import argparse
import importlib.util
import json
import re
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent      # al-skills/coding/al/ae-sdd（唯一真源）
AL = SRC.parent                                   # al-skills/coding/al
PKG = AL / "ae-sdd-zcode"                         # 输出：ZCode 分发包
ALSK = SRC.parents[2]                             # al-skills
REGISTRY = ALSK / "registry.yaml"
CACHE = Path.home() / ".zcode" / "cli" / "plugins" / "cache" / "al-local" / "ae-sdd-suite"
SOURCE_MANIFEST = SRC / ".codex-plugin" / "plugin.json"
ADAPTED_SRC = SRC / "skills" / "ae-sdd" / "SKILL.md"
_bundle_spec = importlib.util.spec_from_file_location('ae_sdd_knowledge_bundle', SRC / 'scripts/bundle_knowledge.py')
knowledge_bundle = importlib.util.module_from_spec(_bundle_spec)
_bundle_spec.loader.exec_module(knowledge_bundle)

# 字节级拷贝映射：（源绝对路径, 包内相对路径）
BYTE_COPIES = [
    (SRC / "references" / name, "skills/ae-sdd/references/" + name)
    for name in ("install.md", "uninstall.md", "installation-contract.md", "agents-guidance.md")
] + [
    (SRC / "capability-catalog.md", "skills/ae-sdd/references/capability-catalog.md"),
    (REGISTRY, "skills/ae-sdd/references/registry.yaml"),
    (SRC / "scripts/manage_agents.py", "skills/ae-sdd/scripts/manage_agents.py"),
    (SRC / "ui" / "icon.png", "icon.png"),
]

# 入口 SKILL.md 锚点替换：（标签, 源锚点, 目标文本）——每处必须恰好命中一次，锚点漂移即失败
ANCHORS = [
    ("registry 解析",
     "In the central source tree resolve it as `../../../../registry.yaml`; in an installed runtime use the host's `al-skills/registry.yaml` or its explicitly packaged copy.",
     "In this packaged ZCode build resolve it as `references/registry.yaml` in this Skill directory; in the central source tree resolve it as `../../../../registry.yaml`."),
    ("catalog 路径",
     "Then read `../../capability-catalog.md` for routing context",
     "Then read `references/capability-catalog.md` for routing context"),
    ("install 链接", "../../references/install.md", "references/install.md"),
    ("uninstall 链接", "../../references/uninstall.md", "references/uninstall.md"),
]

# Tooling is not part of the Skill payload. Management documents are copied as references.
SRC_EXCLUDED_PREFIXES = ("scripts/", "ui/", "tests/", "skills/ae-sdd/capabilities/")
SRC_EXCLUDED_FILES = {"references/gui-management.md", ".codex-plugin/plugin.json", "skills/ae-sdd/SKILL.md"}
SRC_DUPLICATE_CHECKS = []

HAND_KEPT = {"BUILD.md"}  # 输出包内脚本不改不删的手维护文件


def plugin_json_text(version):
    obj = {
        "name": "ae-sdd-suite",
        "version": version,
        "description": "ae-sdd ZCode build - capability routing entry (ae-sdd) plus install/uninstall management reference documents for the AL engineering capability family (al-ra, al-spec, al-coding, al-knowledge, db-operator).",
        "author": {"name": "AILenGarden"},
        "license": "MIT",
        "skills": "skills",
        "icon": "icon.png",
        "keywords": ["ae-sdd", "zcode", "skill", "capability-registry", "al"],
    }
    return json.dumps(obj, ensure_ascii=False, indent=2) + "\n"


def marketplace_json_text(version):
    # icon 用 file:// 绝对 URL：ZCode 客户端只渲染 URL 形式的图标字段（官方三市场均为绝对 URL），
    # 相对路径 "icon.png" 不被识别。此值绑定本机路径，al-local 市场本就是单机本地市场。
    icon_url = PKG.resolve().as_uri() + "/icon.png"
    obj = {
        "name": "al-local",
        "owner": {"name": "AILenGarden"},
        "description": "Local marketplace for AL engineering plugins in al-agent-workspace.",
        "plugins": [{
            "name": "ae-sdd-suite",
            "source": "./",
            "version": version,
            "description": "ae-sdd ZCode build - capability routing entry (ae-sdd) plus install/uninstall management reference documents for the AL engineering capability family.",
            "category": "development",
            "icon": icon_url,
        }],
    }
    return json.dumps(obj, ensure_ascii=False, indent=2) + "\n"


class Report:
    def __init__(self):
        self.fails = 0

    def line(self, tag, msg):
        print(f"[{tag}] {msg}")
        if tag == "FAIL":
            self.fails += 1

    ok = lambda self, m: self.line("OK", m)
    warn = lambda self, m: self.line("WARN", m)
    info = lambda self, m: self.line("INFO", m)
    fail = lambda self, m: self.line("FAIL", m)


def read_source_version():
    if not SOURCE_MANIFEST.is_file():
        sys.exit(f"[FAIL] 源清单缺失: {SOURCE_MANIFEST}")
    version = json.loads(SOURCE_MANIFEST.read_text(encoding="utf-8")).get("version")
    if not version:
        sys.exit(f"[FAIL] 源清单无 version 字段: {SOURCE_MANIFEST}")
    return version


def build_adapted_text():
    """读源入口 SKILL.md，执行锚点替换；任一锚点命中数 != 1 直接失败。"""
    text = ADAPTED_SRC.read_text(encoding="utf-8", newline="")
    for label, old, new in ANCHORS:
        n = text.count(old)
        if n != (2 if label in ("install 链接", "uninstall 链接") else 1):
            sys.exit(f"[FAIL] 锚点「{label}」命中 {n} 次（与预期次数不符）——源措辞已变，请更新本脚本的 ANCHORS")
        text = text.replace(old, new)
    return text.encode("utf-8")


def frontmatter_name(skill_md):
    m = re.search(r"^name:\s*(\S+)", skill_md.read_text(encoding="utf-8")[:400], re.M)
    return m.group(1) if m else None


def main():
    global PKG
    ap = argparse.ArgumentParser(description="从 ae-sdd 源打包 ae-sdd-zcode")
    ap.add_argument("--verify", action="store_true", help="只校验，不写入")
    ap.add_argument("--output", type=Path, default=PKG, help="显式输出目录；可用于隔离验证")
    args = ap.parse_args()
    PKG = args.output.resolve()
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    r = Report()
    version = read_source_version()
    adapted = build_adapted_text()
    knowledge_target = PKG / 'skills/ae-sdd/capabilities/al-knowledge'
    try:
        knowledge_bundle.build(destination=knowledge_target, verify=args.verify)
    except (ValueError, OSError) as exc:
        r.fail(str(exc))
        sys.exit(1)

    if not args.verify:
        for src_path, rel in BYTE_COPIES:
            dst = PKG / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src_path.read_bytes())
        (PKG / "skills/ae-sdd/SKILL.md").write_bytes(adapted)
        (PKG / ".claude-plugin").mkdir(exist_ok=True)
        (PKG / ".claude-plugin/plugin.json").write_text(plugin_json_text(version), encoding="utf-8", newline="\n")
        (PKG / "marketplace.json").write_text(marketplace_json_text(version), encoding="utf-8", newline="\n")

    # --- 自检：字节拷贝 ---
    for src_path, rel in BYTE_COPIES:
        dst = PKG / rel
        if not dst.is_file() or dst.read_bytes() != src_path.read_bytes():
            r.fail(f"字节不一致: {rel}")
    # --- 自检：适配文件与 frontmatter ---
    adapted_dst = PKG / "skills/ae-sdd/SKILL.md"
    if adapted_dst.read_bytes() != adapted:
        r.fail("skills/ae-sdd/SKILL.md 与锚点适配结果不一致")
    for d in ("ae-sdd",):
        name = frontmatter_name(PKG / "skills" / d / "SKILL.md")
        if name != d:
            r.fail(f"frontmatter name={name} != 目录名 {d}")
    if sorted(p.relative_to(PKG).as_posix() for p in PKG.rglob("SKILL.md")) != ["skills/ae-sdd/SKILL.md"]:
        r.fail("输出包包含过期 Skill 入口；请检查旧产物并使用干净目标重新打包")
    # --- 自检：生成清单 ---
    for rel, want in ((".claude-plugin/plugin.json", plugin_json_text(version)),
                      ("marketplace.json", marketplace_json_text(version))):
        got = (PKG / rel).read_text(encoding="utf-8")
        if got != want:
            r.fail(f"{rel} 与生成内容不一致（版本应为 {version}）")
    # --- 源侧副本一致性（源维护的嵌套副本若与顶层漂移，映射来源就不可信） ---
    for nested, top in SRC_DUPLICATE_CHECKS:
        if nested.is_file() and nested.read_bytes() != top.read_bytes():
            r.warn(f"源内副本漂移: {nested.relative_to(SRC).as_posix()} != 顶层 {top.name}")
    # --- 源未映射文件（防新增悄悄漏掉） ---
    consumed = {str(p.resolve()) for p, _ in BYTE_COPIES} | {str(ADAPTED_SRC.resolve()), str(SOURCE_MANIFEST.resolve())}
    for p in sorted(SRC.rglob("*")):
        if not p.is_file() or str(p.resolve()) in consumed:
            continue
        rel = p.relative_to(SRC).as_posix()
        if rel.startswith(SRC_EXCLUDED_PREFIXES) or rel in SRC_EXCLUDED_FILES:
            continue
        r.warn(f"源文件未被映射: ae-sdd/{rel}——若是新增内容，请更新本脚本映射")
    # --- 包内非脚本所有文件（只报告） ---
    knowledge_owned = {'skills/ae-sdd/capabilities/al-knowledge/' + name for name in knowledge_bundle.payload(knowledge_bundle.registered_source())}
    owned = {rel for _, rel in BYTE_COPIES} | knowledge_owned | {"skills/ae-sdd/SKILL.md", ".claude-plugin/plugin.json", "marketplace.json"} | HAND_KEPT
    for p in sorted(PKG.rglob("*")):
        if p.is_file() and p.relative_to(PKG).as_posix() not in owned:
            r.warn(f"包内未纳管文件: {p.relative_to(PKG).as_posix()}（脚本不改动它）")
    # --- 已装缓存对比 ---
    if CACHE.is_dir():
        for vd in sorted(d for d in CACHE.iterdir() if d.is_dir()):
            diff = miss = 0
            for f in PKG.rglob("*"):
                if not f.is_file():
                    continue
                cf = vd / f.relative_to(PKG)
                if not cf.exists():
                    miss += 1
                elif cf.read_bytes() != f.read_bytes():
                    diff += 1
            if diff:
                r.warn(f"已装缓存 {vd.name} 有 {diff} 个文件与包不一致 → 插件市场刷新并重装")
            elif miss:
                r.info(f"已装缓存 {vd.name} 缺 {miss} 个包内文件（重装后自动同步）")
            else:
                r.ok(f"已装缓存 {vd.name} 与包完全一致")
    else:
        r.info("未检测到已安装缓存（ae-sdd-suite 未安装）")

    print(f"源版本: {version} | 模式: {'仅校验' if args.verify else '重建+自检'} | 失败项: {r.fails}")
    sys.exit(1 if r.fails else 0)


if __name__ == "__main__":
    main()
