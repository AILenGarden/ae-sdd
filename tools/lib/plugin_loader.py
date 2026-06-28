"""
plugin_loader.py — ae-sdd 三层 SKILL 注册表加载器（🆕 v3.5.0）

核心能力：
1. 三层注册表收集（L1 项目 / L2 全局 / L3 仓库根）
2. 优先级合成（L1 > L2 > L3 > L0 内置 fallback）
3. schema 校验 + 多层冲突检测（不阻断，warn）
4. 失败兜底（任何加载异常 → fallback 到内置 SKILL）

零外部依赖：自带 YAML 子集解析器（覆盖 registry.yaml 所需语法）。

权威设计文档：source/docs/plans/2026-06-26-plugin-registry-design.md
权威 schema 规范：source/standards/constraints/plugin-registry-spec.md
"""
from __future__ import annotations

import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


SCHEMA_VERSION_SUPPORTED = 1

# Layer 优先级（数字越小优先级越高）
LAYER_PROJECT = 1   # L1：<project>/.ae-sdd/plugins/registry.yaml
LAYER_GLOBAL = 2    # L2：~/.ae-sdd/plugins/registry.yaml
LAYER_MASTER = 3    # L3：<ae-sdd-master>/plugins/registry.yaml
LAYER_BUILTIN = 99  # L0：source/skills/ + source/templates/（fallback）

LAYER_NAMES = {
    LAYER_PROJECT: "L1-project",
    LAYER_GLOBAL: "L2-global",
    LAYER_MASTER: "L3-master",
    LAYER_BUILTIN: "L0-builtin",
}

LAYER_ORDERED = [LAYER_PROJECT, LAYER_GLOBAL, LAYER_MASTER, LAYER_BUILTIN]

VALID_TYPES = {"skill-override", "template-override", "skill-new", "template-new"}


# === 公共数据结构 ===

@dataclass
class Plugin:
    """单个插件声明（来自任一层的注册表）。"""
    name: str
    type: str
    version: str
    description: str
    path: str
    author: Optional[str] = None
    replaces: Optional[str] = None
    provides: Optional[str] = None
    tags: list = field(default_factory=list)
    compatibility: dict = field(default_factory=dict)
    dependencies: list = field(default_factory=list)
    # runtime 字段（loader 填充）
    layer: int = -1
    layer_label: str = ""
    registry_path: Optional[Path] = None       # 注册表文件绝对路径
    resolved_path: Optional[Path] = None        # path 字段解析后的绝对路径

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "type": self.type,
            "version": self.version,
            "author": self.author,
            "description": self.description,
            "path": self.path,
            "replaces": self.replaces,
            "provides": self.provides,
            "tags": list(self.tags),
            "compatibility": dict(self.compatibility),
            "dependencies": list(self.dependencies),
            "layer": self.layer,
            "layerLabel": self.layer_label,
            "registryPath": str(self.registry_path) if self.registry_path else None,
            "resolvedPath": str(self.resolved_path) if self.resolved_path else None,
        }


@dataclass
class Conflict:
    """多层冲突：同一 target 被多层覆盖。"""
    target: str
    winner: Plugin
    losers: list

    def to_dict(self) -> dict:
        return {
            "target": self.target,
            "winner": self.winner.name,
            "winnerLayer": self.winner.layer_label,
            "losers": [{"name": p.name, "layer": p.layer_label} for p in self.losers],
        }


@dataclass
class LoadResult:
    """加载结果。"""
    target: str                                  # 查的 SKILL 标识
    resolved_path: Optional[Path]                # 最终加载路径（None = fallback）
    layer: int                                   # 命中的层
    layer_label: str                             # 命中的层名
    plugin: Optional[Plugin]                     # 命中的插件
    conflicts: list = field(default_factory=list) # 多层冲突
    warnings: list = field(default_factory=list) # 加载过程中的警告

    def to_dict(self) -> dict:
        return {
            "target": self.target,
            "resolvedPath": str(self.resolved_path) if self.resolved_path else None,
            "layer": self.layer,
            "layerLabel": self.layer_label,
            "plugin": self.plugin.to_dict() if self.plugin else None,
            "conflicts": [c.to_dict() for c in self.conflicts],
            "warnings": list(self.warnings),
        }


@dataclass
class RegistryLayer:
    """单层注册表加载结果。"""
    layer: int
    layer_label: str
    registry_path: Optional[Path]
    exists: bool
    plugins: list = field(default_factory=list)        # list[Plugin]
    errors: list = field(default_factory=list)          # 加载错误（解析/校验失败）
    warnings: list = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "layer": self.layer,
            "layerLabel": self.layer_label,
            "registryPath": str(self.registry_path) if self.registry_path else None,
            "exists": self.exists,
            "plugins": [p.to_dict() for p in self.plugins],
            "errors": list(self.errors),
            "warnings": list(self.warnings),
        }


# === 路径解析 ===

def plugin_registry_path_project(ade_sdd: Optional[Path]) -> Optional[Path]:
    """L1 项目层注册表路径。"""
    if ade_sdd is None:
        return None
    return ade_sdd / "plugins" / "registry.yaml"


def plugin_registry_path_global() -> Path:
    """L2 用户全局层注册表路径。

    优先级：
    1. AE_SDD_GLOBAL_HOME 环境变量（测试 / 多用户隔离用）
    2. Path.home() / .ae-sdd/plugins/registry.yaml
    """
    custom = os.environ.get("AE_SDD_GLOBAL_HOME")
    if custom:
        return Path(custom) / ".ae-sdd" / "plugins" / "registry.yaml"
    return Path.home() / ".ae-sdd" / "plugins" / "registry.yaml"


def plugin_registry_path_master(master: Optional[Path]) -> Optional[Path]:
    """L3 仓库根层注册表路径。

    注意：locate_master_source() 返回的是 source/ 目录（含 SKILL.md），
    而 L3 注册表按设计在仓库根 plugins/registry.yaml（不是 source/plugins/）。
    所以这里要取 master.parent 作为仓库根。
    """
    if master is None:
        return None
    # master 是 source/，所以仓库根 = master.parent
    repo_root = master.parent if master.name == "source" else master
    return repo_root / "plugins" / "registry.yaml"


# === YAML 子集解析器（零依赖） ===

def _parse_yaml_subset(text: str) -> dict:
    """极简 YAML 解析器，仅覆盖 registry.yaml schema。

    支持：
    - # 行注释
    - 顶层 key: scalar
    - | literal block（多行字符串）
    - 嵌套 dict（key: 后跟缩进 key: value）
    - list of dict（- 开头 + 缩进属性）
    - list of scalar（- value）
    - int / bool / str 类型自动识别
    - "..." 或 "---" 文档分隔符忽略

    不支持（registry.yaml 不会用）：
    - & 锚点 / * 引用
    - flow style ({}, [] inline)
    - !!type 标签
    - 多层嵌套 dict（compatibility 只有一层）
    """
    # 预处理：去掉 BOM + 文档分隔符
    text = text.lstrip("\ufeff")
    lines = []
    for raw in text.splitlines():
        # 去掉行尾注释（保留缩进）
        # 注：split("#", 1) 遇到字符串内的 # 也会被切——但 registry.yaml schema 不含行内字符串含 # 的情况
        line_no_comment = raw.split("#", 1)[0]
        lines.append(line_no_comment.rstrip())

    # 第一遍：解析为节点列表
    nodes = []  # (indent, key_or_None, type, value)
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue

        # 文档分隔符
        if line.strip() in ("---", "...", "..."):
            i += 1
            continue

        indent = len(line) - len(line.lstrip())
        stripped = line.strip()

        # | literal block
        if stripped.endswith("|") or stripped.endswith("|+") or stripped.endswith("|-"):
            chomp = stripped[-1]
            key = stripped[:-1].rstrip().rstrip(":")
            i += 1
            block_indent = None
            content = []
            while i < len(lines):
                l = lines[i]
                if not l.strip():
                    content.append("")
                    i += 1
                    continue
                cur_indent = len(l) - len(l.lstrip())
                if block_indent is None:
                    block_indent = cur_indent
                if cur_indent < block_indent:
                    break
                content.append(l[block_indent:])
                i += 1
            value = "\n".join(content)
            if chomp == "-":
                value = value.rstrip("\n")
            elif chomp == "+":
                pass
            else:
                value = value.rstrip("\n") + "\n" if value else ""
            nodes.append((indent, key, "literal", value.strip()))
            continue

        # > folded block
        if stripped.endswith(">") or stripped.endswith(">+") or stripped.endswith(">-"):
            key = stripped[:-1].rstrip().rstrip(":")
            i += 1
            block_indent = None
            content = []
            while i < len(lines):
                l = lines[i]
                if not l.strip():
                    content.append("")
                    i += 1
                    continue
                cur_indent = len(l) - len(l.lstrip())
                if block_indent is None:
                    block_indent = cur_indent
                if cur_indent < block_indent:
                    break
                content.append(l[block_indent:])
                i += 1
            folded = []
            current_para = []
            for c in content:
                if not c.strip():
                    if current_para:
                        folded.append(" ".join(current_para))
                        current_para = []
                else:
                    current_para.append(c.strip())
            if current_para:
                folded.append(" ".join(current_para))
            nodes.append((indent, key, "literal", "\n".join(folded)))
            continue

        # list 元素（以 - 开头）
        if stripped.startswith("- "):
            payload = stripped[2:].strip()
            nodes.append((indent, None, "list_item", payload))
            i += 1
            continue
        if stripped == "-":
            nodes.append((indent, None, "list_item", ""))
            i += 1
            continue

        # key: value 或 key:（dict 起始）
        if ":" in stripped:
            key_part, _, val_part = stripped.partition(":")
            key = key_part.strip()
            val = val_part.strip()
            if val:
                nodes.append((indent, key, "scalar", val))
            else:
                nodes.append((indent, key, "dict_or_list_start", None))
            i += 1
            continue

        # 其他（异常结构，跳过）
        nodes.append((indent, None, "scalar", stripped))
        i += 1

    # 第二遍：构建嵌套结构
    return _parse_yaml_subset_build(nodes, 0, -1)[0]


def _parse_yaml_subset_build(nodes: list, i: int, parent_indent: int) -> tuple:
    """从 nodes[i] 开始构建一个 dict。返回 (dict, new_i)。

    parent_indent 是当前 dict 的"父级"缩进；本 dict 内节点的 indent 都 > parent_indent。
    """
    result = {}
    while i < len(nodes):
        indent, key, typ, val = nodes[i]
        # 缩进 ≤ parent_indent → 跳出（回到父级）
        if indent <= parent_indent:
            break

        if typ == "list_item":
            # dict 内不应该直接出现 list_item（应该是 dict_or_list_start 的 value）
            # 异常结构，跳过
            i += 1
            continue

        if key is None:
            i += 1
            continue

        if typ == "scalar":
            result[key] = _coerce_scalar(val)
            i += 1
        elif typ == "literal":
            result[key] = val
            i += 1
        elif typ == "dict_or_list_start":
            # 收集子节点（缩进 > 当前 indent）
            sub_nodes = []
            j = i + 1
            while j < len(nodes) and nodes[j][0] > indent:
                sub_nodes.append(nodes[j])
                j += 1

            # 判断是 dict 还是 list
            has_list_item = any(n[2] == "list_item" for n in sub_nodes)
            if has_list_item:
                # list of dict / list of scalar
                items, end_j = _build_list(nodes, i + 1, indent)
                result[key] = items
            else:
                # 嵌套 dict
                sub, end_j = _parse_yaml_subset_build(nodes, i + 1, indent)
                result[key] = sub
            i = j
        else:
            i += 1

    return result, i


def _build_value(nodes: list, i: int, parent_indent: int) -> tuple:
    """构建一个 value（list / 嵌套 dict / scalar）。返回 (value, new_i)。

    parent_indent 是调用方的缩进；本 value 内节点的 indent 都 > parent_indent。
    """
    if i >= len(nodes):
        return None, i

    # 收集第一个有效节点之前的同 indent 的 list_items（list 开头）
    # 简单起见：直接看 i 处的类型
    first = nodes[i]
    indent = first[0]
    typ = first[2]

    if typ == "list_item":
        return _build_list(nodes, i, parent_indent)
    elif typ in ("scalar", "literal"):
        # 不应该在 dict_or_list_start 后出现（应该是 dict 的 key: value）
        return (first[3] if typ == "literal" else _coerce_scalar(first[3])), i + 1
    elif typ == "dict_or_list_start":
        return _parse_yaml_subset_build(nodes, i, parent_indent)
    else:
        return None, i + 1


def _build_list(nodes: list, i: int, parent_indent: int) -> tuple:
    """构建 list（list of dict 或 list of scalar）。返回 (list, new_i)。"""
    items = []
    cur = None       # 当前正在累积的 dict（list_item 的属性）
    cur_indent = None

    while i < len(nodes):
        indent, key, typ, val = nodes[i]

        # 缩进 ≤ parent_indent 表示回到父级
        if indent <= parent_indent:
            break

        if typ == "list_item":
            # 完成上一个
            if cur is not None:
                items.append(cur)
                cur = None

            payload = val
            if ":" in payload:
                # - key: value → 开始新 dict
                k, _, v = payload.partition(":")
                cur = {k.strip(): _coerce_scalar(v.strip())}
                cur_indent = indent
            elif payload:
                # - value（裸 scalar） → list of scalar 元素
                items.append(_coerce_scalar(payload))
                cur = None
                cur_indent = None
            else:
                # - 空 → 后面跟嵌套内容（罕见）
                cur = {}
                cur_indent = indent
            i += 1

        elif cur is not None and typ == "scalar" and key is not None:
            # list item 的属性
            if indent > cur_indent:
                cur[key] = _coerce_scalar(val)
            i += 1

        elif cur is not None and typ == "literal" and key is not None:
            if indent > cur_indent:
                cur[key] = val
            i += 1

        elif cur is not None and typ == "dict_or_list_start" and key is not None:
            # 嵌套 dict / list of dict / list of scalar
            # 收集子节点判断
            sub_nodes = []
            j = i + 1
            while j < len(nodes) and nodes[j][0] > indent:
                sub_nodes.append(nodes[j])
                j += 1
            has_list_item = any(n[2] == "list_item" for n in sub_nodes)
            if has_list_item:
                sub_items, end_j = _build_list(nodes, i + 1, indent)
                cur[key] = sub_items
            else:
                sub_dict, end_j = _parse_yaml_subset_build(nodes, i + 1, indent)
                cur[key] = sub_dict
            i = j

        else:
            i += 1

    if cur is not None:
        items.append(cur)

    return items, i


def _coerce_scalar(val: str):
    """自动识别 scalar 类型。"""
    if not val:
        return ""
    if val.lower() in ("true", "yes"):
        return True
    if val.lower() in ("false", "no"):
        return False
    # 去掉引号
    if (val.startswith('"') and val.endswith('"')) or (val.startswith("'") and val.endswith("'")):
        return val[1:-1]
    # int
    try:
        return int(val)
    except ValueError:
        pass
    return val


# === 注册表加载与校验 ===

def load_registry(registry_path: Path, layer: int, layer_label: str) -> RegistryLayer:
    """加载单层注册表。

    返回 RegistryLayer（即使文件不存在也返回 exists=False 的实例，不抛异常）。
    """
    rl = RegistryLayer(layer=layer, layer_label=layer_label, registry_path=registry_path, exists=False)

    if registry_path is None:
        rl.warnings.append(f"{layer_label}: 注册表路径未提供（跳过）")
        return rl

    if not registry_path.is_file():
        rl.warnings.append(f"{layer_label}: 注册表不存在（{registry_path}，跳过）")
        return rl

    rl.exists = True
    try:
        text = registry_path.read_text(encoding="utf-8")
    except (OSError, IOError, UnicodeDecodeError) as e:
        rl.errors.append(f"{layer_label}: 读取失败：{e}")
        return rl

    try:
        data = _parse_yaml_subset(text)
    except Exception as e:
        rl.errors.append(f"{layer_label}: YAML 解析失败：{e}")
        return rl

    # schema_version 校验
    schema_version = data.get("schema_version")
    if schema_version != SCHEMA_VERSION_SUPPORTED:
        rl.errors.append(f"{layer_label}: schema_version 必须为 {SCHEMA_VERSION_SUPPORTED}，实际为 {schema_version}")
        return rl

    # plugins 数组
    plugins_raw = data.get("plugins", [])
    if not isinstance(plugins_raw, list):
        rl.errors.append(f"{layer_label}: plugins 字段必须是数组，实际为 {type(plugins_raw).__name__}")
        return rl

    # 单层内 name 唯一性 + replaces 唯一性检测
    seen_names = set()
    seen_replaces = set()

    registry_dir = registry_path.parent

    for idx, p_raw in enumerate(plugins_raw):
        # 🆕 v3.5.10：enabled: false 的 plugin 跳过（不加载、不计入 count、不校验）
        # 用途：L3 默认注册表的 example 插件默认禁用，避免污染 plugin count / 测试隔离
        if isinstance(p_raw, dict) and p_raw.get("enabled") is False:
            continue
        try:
            p = _parse_plugin(p_raw, idx, registry_path, registry_dir)
        except ValueError as e:
            rl.errors.append(f"{layer_label}[{idx}]: {e}")
            continue

        # name 唯一性
        if p.name in seen_names:
            rl.errors.append(f"{layer_label}: name '{p.name}' 在该层注册表内重复")
            continue
        seen_names.add(p.name)

        # replaces 唯一性
        if p.replaces:
            if p.replaces in seen_replaces:
                rl.errors.append(f"{layer_label}: replaces '{p.replaces}' 在该层注册表内被多次覆盖")
                continue
            seen_replaces.add(p.replaces)

        # path 存在性 + 越权检测
        if ".." in p.path.split("/"):
            rl.errors.append(f"{plugin_repr(p)}: path 含 '..' 越权")
            continue

        resolved = (registry_dir / p.path).resolve()
        if not resolved.is_file():
            rl.errors.append(f"{plugin_repr(p)}: path 指向的文件不存在：{resolved}")
            continue

        p.resolved_path = resolved
        p.layer = layer
        p.layer_label = layer_label
        p.registry_path = registry_path

        # replaces 内置路径校验（只对 skill-override / template-override）
        if p.type in ("skill-override", "template-override"):
            builtin_base = registry_dir
            # master 路径下：source/skills/phase2-coding/coding-skill.md 是相对 master 的
            # 但 ae-sdd 装到 ~/.claude/skills/ae-sdd/ 时 master 不一定有 source/
            # 这里我们宽容处理：只校验 path 解析后存在（已经在上面做了）
            # replaces 的内置路径校验留给 trace/load 时做（因为 builtin 路径依赖安装状态）

        # 规则 #16：外挂内容安全扫描（🆕 B4 增强，分层阻断）
        # L2 全局层 BLOCKER → 阻断；L1 项目层 BLOCKER → 告警；L3 仓库根 → 跳过（git 兜底）
        if layer == LAYER_MASTER:
            pass  # L3 git tracked，跳过
        else:
            scan_result = _scan_plugin_content(resolved, p.name)
            if scan_result:
                if scan_result.skipped:
                    rl.warnings.append(
                        f"{plugin_repr(p)}: 内容扫描跳过（{scan_result.skip_reason}）"
                    )
                else:
                    for f in scan_result.findings:
                        # 分层：L2 BLOCKER 阻断，L1 仅告警
                        should_block = (layer == LAYER_GLOBAL and f.severity == "BLOCKER")
                        prefix = "🔴" if should_block else "🟡" if f.severity != "INFO" else "🔵"
                        line = (f"{prefix} {plugin_repr(p)}: 内容扫描 {f.rule} @ L{f.line} "
                                f"— {f.snippet}")
                        if should_block:
                            rl.errors.append(line + f"（{f.message}）")
                        elif f.severity == "INFO":
                            pass  # INFO 不入 warnings 列表，避免噪音（仅 scan_result 里可见）
                        else:
                            rl.warnings.append(line + f"（{f.message}）")
                    # L2 BLOCKER 命中 → 不加入 plugins（阻断加载）
                    if layer == LAYER_GLOBAL and scan_result.blockers > 0:
                        continue

        rl.plugins.append(p)

    return rl


def _parse_plugin(raw: dict, idx: int, registry_path: Path, registry_dir: Path) -> Plugin:
    """解析单个 plugin 字典 + 校验必填字段。"""
    errors = []
    for field_name in ("name", "type", "version", "description", "path"):
        if field_name not in raw or raw[field_name] in (None, ""):
            errors.append(f"plugins[{idx}]: 缺必填字段 '{field_name}'")

    name = raw.get("name", "")
    ptype = raw.get("type", "")

    # type 校验
    if ptype not in VALID_TYPES:
        errors.append(f"plugins[{idx}] '{name}': type 必须是 {VALID_TYPES} 之一，实际为 '{ptype}'")

    # type vs replaces/provides 关系
    if ptype in ("skill-override", "template-override"):
        if not raw.get("replaces"):
            errors.append(f"plugins[{idx}] '{name}': type={ptype} 时 'replaces' 必填")
    elif ptype in ("skill-new", "template-new"):
        if not raw.get("provides"):
            errors.append(f"plugins[{idx}] '{name}': type={ptype} 时 'provides' 必填")

    # name 正则
    if name and not re.match(r"^[a-z0-9][a-z0-9-]*$", str(name)):
        errors.append(f"plugins[{idx}] '{name}': name 必须匹配 ^[a-z0-9][a-z0-9-]*$（kebab-case）")

    # version semver 简单校验
    version = str(raw.get("version", ""))
    if version and not re.match(r"^\d+\.\d+\.\d+(-[\w.]+)?$", version):
        errors.append(f"plugins[{idx}] '{name}': version 必须是 semver（X.Y.Z），实际为 '{version}'")

    if errors:
        raise ValueError("; ".join(errors))

    return Plugin(
        name=str(name),
        type=str(ptype),
        version=str(version),
        description=str(raw.get("description", "")),
        path=str(raw.get("path", "")),
        author=raw.get("author"),
        replaces=raw.get("replaces"),
        provides=raw.get("provides"),
        tags=list(raw.get("tags") or []),
        compatibility=dict(raw.get("compatibility") or {}),
        dependencies=list(raw.get("dependencies") or []),
    )


def plugin_repr(p: Plugin) -> str:
    """插件的可读表示。"""
    return f"plugin '{p.name}' ({p.layer_label})"


def _scan_plugin_content(path: Path, plugin_name: str):
    """🆕 B4 增强：调用 scripts/plugin_content_scan.py 扫描外挂内容。

    失败优先：扫描器不可用/异常 → 返回 None（调用方按"跳过"处理，不阻断主流程）。
    返回 ScanResult 或 None。
    """
    try:
        import sys
        from pathlib import Path as _Path
        # plugin_loader 在 tools/lib/，扫描器在 scripts/（仓库根下）
        repo_root = _Path(__file__).resolve().parent.parent.parent
        scripts_dir = repo_root / "scripts"
        if str(scripts_dir) not in sys.path:
            sys.path.insert(0, str(scripts_dir))
        import plugin_content_scan
        return plugin_content_scan.scan_plugin_file(path, plugin_name)
    except Exception:
        # 扫描器异常不阻断主流程（与 prompt_inject / drift 探测同模式）
        return None


# === 三层加载 + 优先级合成 ===

def collect_all_layers(ade_sdd: Optional[Path], master: Optional[Path]) -> list:
    """收集三层注册表（含 builtin fallback 标识）。"""
    layers = []

    # L1 项目层
    p1 = plugin_registry_path_project(ade_sdd)
    layers.append(load_registry(p1, LAYER_PROJECT, LAYER_NAMES[LAYER_PROJECT]))

    # L2 全局层
    p2 = plugin_registry_path_global()
    layers.append(load_registry(p2, LAYER_GLOBAL, LAYER_NAMES[LAYER_GLOBAL]))

    # L3 仓库根层
    p3 = plugin_registry_path_master(master)
    layers.append(load_registry(p3, LAYER_MASTER, LAYER_NAMES[LAYER_MASTER]))

    return layers


def list_plugins(ade_sdd: Optional[Path], master: Optional[Path]) -> dict:
    """列所有已加载的插件（按层分组 + 冲突检测）。

    返回结构：
    {
      "layers": [RegistryLayer.to_dict() for ...],
      "allPlugins": [Plugin.to_dict() for ...],
      "conflicts": [Conflict.to_dict() for ...],
    }
    """
    layers = collect_all_layers(ade_sdd, master)
    all_plugins = [p for rl in layers for p in rl.plugins]
    conflicts = detect_conflicts(all_plugins)

    return {
        "layers": [rl.to_dict() for rl in layers],
        "allPlugins": [p.to_dict() for p in all_plugins],
        "conflicts": [c.to_dict() for c in conflicts],
        "totalPlugins": len(all_plugins),
        "totalConflicts": len(conflicts),
    }


def detect_conflicts(plugins: list) -> list:
    """检测多层冲突：同一 target（replaces 或 provides）被多层覆盖。

    返回 Conflict 列表。
    """
    # 按 target 分组
    by_target = {}
    for p in plugins:
        target = p.replaces or p.provides
        if not target:
            continue
        by_target.setdefault(target, []).append(p)

    conflicts = []
    for target, plist in by_target.items():
        if len(plist) <= 1:
            continue
        # 按 layer 排序（layer 数字越小优先级越高）
        plist_sorted = sorted(plist, key=lambda p: (p.layer, p.name))
        winner = plist_sorted[0]
        losers = plist_sorted[1:]
        conflicts.append(Conflict(target=target, winner=winner, losers=losers))

    return conflicts


def resolve_skill(target: str, ade_sdd: Optional[Path], master: Optional[Path]) -> LoadResult:
    """解析目标 SKILL 的实际加载路径（按三层优先级合成）。

    target 可以是：
    - "replaces 内置路径"（如 "source/skills/phase2-coding/coding-skill.md"）
    - "provides key"（如 "finance-coding-skill"）
    - 任何 registry.yaml 里的 replaces/provides 值

    返回 LoadResult：
    - resolved_path = 外挂 SKILL 绝对路径（命中三层任一层）
    - resolved_path = None + layer = LAYER_BUILTIN（fallback 到内置）
    """
    layers = collect_all_layers(ade_sdd, master)

    # 收集所有匹配的 plugin（按 target 匹配）
    matched = []
    for rl in layers:
        for p in rl.plugins:
            if p.replaces == target or p.provides == target:
                matched.append(p)

    warnings = []
    conflicts = []

    if matched:
        # 按 layer 排序（layer 数字越小优先级越高）
        matched_sorted = sorted(matched, key=lambda p: (p.layer, p.name))
        winner = matched_sorted[0]
        losers = matched_sorted[1:]

        if losers:
            conflict = Conflict(target=target, winner=winner, losers=losers)
            conflicts.append(conflict)
            warnings.append(
                f"plugin '{winner.name}' ({winner.layer_label}) 与 "
                f"{[p.name + ' (' + p.layer_label + ')' for p in losers]} "
                f"都覆盖了 '{target}'；{winner.layer_label} 胜出"
            )

        # 兼容性检查
        master_version = _read_master_version(master)
        compat_version = winner.compatibility.get("ae_sdd_version") if winner.compatibility else None
        if compat_version and master_version:
            if not _version_satisfies(master_version, compat_version):
                warnings.append(
                    f"plugin '{winner.name}' 要求 ae_sdd_version {compat_version}，"
                    f"当前 master 版本 {master_version} 不满足"
                )

        return LoadResult(
            target=target,
            resolved_path=winner.resolved_path,
            layer=winner.layer,
            layer_label=winner.layer_label,
            plugin=winner,
            conflicts=conflicts,
            warnings=warnings,
        )

    # fallback 到内置
    return LoadResult(
        target=target,
        resolved_path=None,
        layer=LAYER_BUILTIN,
        layer_label=LAYER_NAMES[LAYER_BUILTIN],
        plugin=None,
        conflicts=[],
        warnings=[f"target '{target}' 未在任何注册表命中，fallback 到 L0 内置"],
    )


# === 工具函数 ===

def _read_master_version(master: Optional[Path]) -> Optional[str]:
    """从 master/source/SKILL.md 的 frontmatter 读 version。"""
    if master is None:
        return None
    skill_md = master / "SKILL.md"
    if not skill_md.is_file():
        return None
    try:
        text = skill_md.read_text(encoding="utf-8")
    except (OSError, IOError, UnicodeDecodeError):
        return None
    # 简单解析 frontmatter
    if not text.startswith("---"):
        return None
    end = text.find("\n---", 3)
    if end == -1:
        return None
    front = text[3:end]
    for line in front.splitlines():
        if line.strip().startswith("version:"):
            return line.split(":", 1)[1].strip().strip('"').strip("'")
    return None


def _version_satisfies(master: str, requirement: str) -> bool:
    """简单 semver range 检查：支持 '>=X.Y.Z' 和 'X.Y.Z'。"""
    # 解析 master 版本
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)", master)
    if not m:
        return True  # 无法解析 → 放行
    m_tuple = (int(m.group(1)), int(m.group(2)), int(m.group(3)))

    # 解析 requirement
    req = requirement.strip()
    if req.startswith(">="):
        target = req[2:].strip()
        t = re.match(r"^(\d+)\.(\d+)\.(\d+)", target)
        if not t:
            return True
        return m_tuple >= (int(t.group(1)), int(t.group(2)), int(t.group(3)))
    if req.startswith(">"):
        target = req[1:].strip()
        t = re.match(r"^(\d+)\.(\d+)\.(\d+)", target)
        if not t:
            return True
        return m_tuple > (int(t.group(1)), int(t.group(2)), int(t.group(3)))
    if req.startswith("=="):
        target = req[2:].strip()
        return master.startswith(target)
    # 默认 = X.Y.Z → 必须相等
    t = re.match(r"^(\d+)\.(\d+)\.(\d+)", req)
    if not t:
        return True
    return m_tuple == (int(t.group(1)), int(t.group(2)), int(t.group(3)))


# === 入口（CLI 调用）===

def validate(ade_sdd: Optional[Path], master: Optional[Path]) -> dict:
    """校验三层注册表 + 每个 plugin 的 sanity check。

    返回：
    {
      "valid": bool,
      "errors": [str, ...],
      "warnings": [str, ...],
      "layers": [RegistryLayer.to_dict(), ...],
    }
    """
    layers = collect_all_layers(ade_sdd, master)
    errors = []
    warnings = []

    for rl in layers:
        errors.extend(rl.errors)
        warnings.extend(rl.warnings)

    # 多层冲突检测
    all_plugins = [p for rl in layers for p in rl.plugins]
    conflicts = detect_conflicts(all_plugins)
    for c in conflicts:
        warnings.append(
            f"多层冲突：target '{c.target}' 被 "
            f"winner={c.winner.name}({c.winner.layer_label}) + "
            f"losers={[p.name for p in c.losers]} 覆盖；{c.winner.layer_label} 胜出"
        )

    return {
        "valid": len(errors) == 0,
        "errors": errors,
        "warnings": warnings,
        "layers": [rl.to_dict() for rl in layers],
        "totalPlugins": len(all_plugins),
        "totalConflicts": len(conflicts),
    }