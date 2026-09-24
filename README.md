# AL Skill Suite (ae-sdd)

AL 工程能力套件。内容与 `al-agent-workspace` 仓库的 `al-skills/` 子树**同路径、逐字节一致**，
因此可以直接按包内文档声明的相对路径解析，无需任何布局调整。

## 布局

```text
al-skills/
├── registry.yaml              系统级能力注册表（packageRoot / entrypoint / 发现机制）
└── coding/al/
    ├── ae-sdd/                入口包：能力发现与路由；内置知识能力 capabilities/al-knowledge
    ├── al-ra/                 需求分析
    ├── al-spec/               类型化规格：DR / Story / TestCase
    ├── al-coding/             编码设计、实现、评审与实现验证
    ├── al-knowledge/          OKF 项目知识正本
    └── db-operator/           严格只读的数据库检查与证据采集
```

只发布 AL 技能套件这一部分；`al-skills/` 在源工作区里还收纳各 Agent 的资产与镜像，
不属于本仓库范围。

## 使用

- **入口**：`al-skills/coding/al/ae-sdd/skills/ae-sdd/SKILL.md`（套件只暴露这一个可发现入口）。
- **能力解析**：读 `al-skills/registry.yaml`，按各条目的 `packageRoot` / `entrypoint` 显式定位；
  不要按目录名或目录顺序猜测，也不要把 `dist/`、`_agents/`、运行时副本当作源。
- **安装 / 更新 / 卸载**：`al-skills/coding/al/ae-sdd/references/install.md`、`uninstall.md`；
  文件归属、备份与恢复契约见同目录 `installation-contract.md`。

### `al-knowledge` 为什么有两处

不是重复维护，而是「正本 + 生成副本」：

- `al-skills/coding/al/al-knowledge/` 是唯一正本；
- `al-skills/coding/al/ae-sdd/skills/ae-sdd/capabilities/al-knowledge/` 是
  `al-skills/coding/al/ae-sdd/scripts/bundle_knowledge.py` 生成的交付副本，
  `.bundle.json` 记录每个文件的归属哈希（入口由 `SKILL.md` 重命名为 `CAPABILITY.md`）。

`registry.yaml` 中该能力登记为 `owner: ae-sdd`、`delivery: bundled`、`installable: false`，
即入口插件自带知识能力，不要求单独安装。**禁止手工双维护**：改正本 → 重新生成 → `--verify`。

## 构建与校验

在 `al-skills/coding/al/ae-sdd/` 下执行（Python 需 PyYAML）：

```text
python scripts/bundle_knowledge.py --verify      # 生成副本与正本一致性
python -m unittest discover -s tests -p 'test_*.py'
python -m unittest discover -s skills/ae-sdd/capabilities/al-knowledge/tests -p 'test_*.py'
```

平台包生成：`scripts/pack-zcode-plugin.py`、`scripts/pack-claude-plugin.ps1`。
`dist/` 与 `db-operator/bin/` 是源工作区里已跟踪的构建产物，仅保留当前版本，便于按版本取用；
历史版本快照不进本仓库（在源工作区的归档与 git 历史中）。

## 贡献方式

改动在 `al-agent-workspace` 的 `al-skills/` 里进行，校验通过后同步到此仓库；
不要只在本仓库单向修改后再回灌，以免出现第二份正本。
