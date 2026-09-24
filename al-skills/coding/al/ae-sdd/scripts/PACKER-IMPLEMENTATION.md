# ae-sdd 打包实现

ae-sdd 的维护源由 [系统注册表](../../../../registry.yaml) 登记。Claude 与 ZCode 适配包由脚本生成；修改维护源后再构建，不手工双维护生成内容。

## 当前交付边界

- ae-sdd 包只暴露一个 `SKILL.md` 入口。安装、卸载是 `references/install.md` 和 `references/uninstall.md` 中的流程，不生成独立管理 Skill。
- `al-knowledge` 由注册表指定的源目录构建，包内入口为 `capabilities/al-knowledge/CAPABILITY.md`，同时携带脚本、模板、参考文档和归属清单。
- `al-ra`、`al-spec`、`al-coding`、`db-operator` 保持各自的安装边界；适配包不复制这些能力的实现。
- `scripts/manage_agents.py` 与 `references/agents-guidance.md` 随包提供，供安装流程同步托管的 AGENTS.md 段落。打包本身不安装到 Agent 运行时。

## 脚本与产物

| 脚本 | 默认输出 | 作用 |
| --- | --- | --- |
| [bundle_knowledge.py](bundle_knowledge.py) | `../skills/ae-sdd/capabilities/al-knowledge/` | 从注册源生成私有知识资源；`--verify` 只校验 |
| [pack-claude-plugin.ps1](pack-claude-plugin.ps1) | `../../ae-sdd-claude/` | 适配入口路径，复制参考文档和管理工具，生成 README 与 manifest，打包知识资源 |
| [pack-zcode-plugin.py](pack-zcode-plugin.py) | `../../ae-sdd-zcode/` | 生成 ZCode 插件及市场清单，复制映射资源并校验；`--verify` 只校验 |

Claude 输出的公开入口位于根目录 `SKILL.md`；ZCode 输出位于 `skills/ae-sdd/SKILL.md`。两个打包器均检查输出中是否存在额外的过时 Skill 入口。知识 payload 按归属清单维护，未知文件或人工修改的冲突应保留并报告。

## 构建与校验

在 `al-skills/coding/al/ae-sdd/` 下执行，Python 环境需要安装知识工具依赖 PyYAML：

```powershell
python scripts/bundle_knowledge.py --verify
python -m unittest discover -s tests -p 'test_*.py' -v
```

需要生成平台包时，优先使用全新的临时输出目录，验证后再按授权处理交付目标：

```powershell
.\scripts\pack-claude-plugin.ps1 -TargetRoot '<新的 Claude 输出目录>' -Python '<python 可执行文件>'
python scripts/pack-zcode-plugin.py --output '<新的 ZCode 输出目录>'
python scripts/pack-zcode-plugin.py --output '<同一 ZCode 输出目录>' --verify
```

Claude 打包器支持 `-SourceRoot`、`-TargetRoot`、`-Version` 和 `-Python`；版本默认来自源插件清单。`-Clean` 会删除整个输出目录，只能用于已核实范围且允许重建的目标。

验证以当次执行结果为准：检查单一公开入口、引用闭包、知识资源与源的一致性、平台清单，以及 AGENTS.md 管理工具。文件数量和包大小随资源变化，不使用历史固定值作为验收条件。生成内容带有时间戳，不保证两次构建全包字节相同。

安装、更新与卸载另遵循 [安装合同](../references/installation-contract.md)。打包不会执行 Git 提交、推送或打 tag。
