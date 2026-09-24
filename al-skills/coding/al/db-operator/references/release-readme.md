# DB Operator 发布包

发布目录同时提供两种完整包：Windows x64（`db-operator-*-windows-x86_64-*.zip`）与 macOS / Linux（`db-operator-*-{macos,linux}-{x86_64,aarch64}-*.tar.gz`，由 `scripts/package-unix.sh` 产出）。两者内容一致、仅平台不同：客户端、管理工具、daemon 服务端与连接管理 UI 都在同一个包里，**不需要另外下载 daemon 安装包**。运行这些二进制无需 Rust；管理 UI 另需 Node.js。服务安装需要管理员权限（Windows 管理员 PowerShell；macOS、Linux 的 root），普通客户端使用管理员签发的 capability。本文件随两种包一同分发，按平台看对应小节。

## 使用（Windows x64）

1. 将整个 ZIP 解压到独立目录，保留 `bin`、`scripts`、`references` 等子目录结构。此包不适用于原生 ARM64、macOS 或 Linux 服务安装。
2. **首次使用先安装服务。** 在解压目录打开管理员 PowerShell，按下面示例填写实际 Windows 账户、连接别名和客户端配置路径；注册时在本机输入连接信息，连接别名须与 `-Connection` 一致：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-service.ps1 `
  -AgentAccount "MACHINE\agent-user" `
  -Connection reporting-prod `
  -AgentConfigPath "C:\Users\agent-user\AppData\Roaming\db-operator\client.json" `
  -Register
```

3. 切回运行 Agent 的普通用户，运行 `.\bin\db-operator.exe status` 检查服务和授权。使用非默认配置路径时，在该用户环境中设置 `DB_OPERATOR_CLIENT_CONFIG` 指向签发的文件。
4. `scripts/install.ps1` **只安装客户端**，不安装 daemon、不注册连接、不生成 `client.json`。本包已含可运行的 `bin/db-operator.exe`。
5. 需要连接管理 UI 时，运行 `powershell -ExecutionPolicy Bypass -File .\scripts\open-registry.ps1`，并按提示以管理员权限启动。详细流程见 `references/connection-schema.md`。

## 使用（macOS / Linux）

1. 解压 `.tar.gz` 到独立目录，保留 `bin`、`scripts`、`references` 等结构。包内脚本为 LF 行尾，可直接执行。
2. **首次使用先安装服务**，以 root 运行：

```bash
sh scripts/install-service.sh \
  --agent-user agent-user \
  --connection reporting-prod \
  --agent-config /home/agent-user/.config/db-operator/client.json \
  --register
```

macOS 上 Agent 配置路径用 `~/Library/Application Support/db-operator/client.json`。安装器会创建独立服务身份（Linux `db-operator`、macOS `_dboperator`）、把注册数据锁进 `/var/lib/db-operator`、签发 capability，并安装 systemd unit 或 LaunchDaemon（`com.al.db-operator`）。daemon 通过 Unix socket 提供查询：Linux `/run/db-operator/db-operator.sock`，macOS `/var/run/db-operator/db-operator.sock`，socket 权限 0660、属组 `db-operator-agents`。

3. 切回 Agent 用户，运行 `bin/db-operator status` 检查服务与授权。
4. 需要连接管理 UI 时运行 `sh scripts/open-registry.sh`（需要 Node.js 与 root，未提权时脚本会自行 `sudo` 重入），详细流程见 `references/connection-schema.md`。UI 在 macOS、Linux 上以 Agent 账号确定 client.json 路径，并把签发结果归属到该账号（属主为该账号、权限 0600）。

## 提示 daemon 不存在时

先核对解压目录中是否包含以下文件：

- Windows：`bin/service/windows-x86_64/db-operator-daemon.exe`、`bin/service/windows-x86_64/db-operator-admin.exe`、`scripts/install-service.ps1`
- macOS / Linux：`bin/service/macos-aarch64/`、`bin/service/macos-x86_64/`、`bin/service/linux-x86_64/` 或 `linux-aarch64/` 下的 `db-operator-daemon`、`db-operator-admin`，以及 `scripts/install-service.sh`

文件齐全时，继续检查服务是否安装及 `client.json` 是否已签发，无需另找 daemon 包。文件缺失时，重新完整解压或获取同平台完整包；只复制 Skill 或客户端可能遗漏服务组件。客户端 `status` 失败本身不能证明安装包缺失。若仍失败，提供缺失的相对路径、执行命令、系统架构及脱敏后的错误文字，不发送凭据或 `client.json` 内容。

## UI 的实际能力

随包 UI 通过本机 HTTP 管理服务调用管理员工具，覆盖连接管理与 Agent 授权闭环：连接注册、编辑、删除、schema 缓存刷新、显示当前生效的查询授权，以及为选定连接签发客户端授权（可一并重启服务使其生效）。需要 Node.js 和管理员权限，不能直接打开 `ui/index.html` 使用：Windows 用 `scripts\open-registry.ps1`，macOS / Linux 用 `scripts/open-registry.sh`。注册新连接时可勾选"注册后自动签发 Agent 授权"：当前没有其他生效授权时会立即生成 `client.json` 并按需重启服务，无需再手动签发；已有其他连接的授权时自动跳过并在页面提示。保存前的连接测试失败（网络不通、凭据错误、超时）不会保存注册，页面会显示数据库驱动的原始错误文字。macOS / Linux 上签发时需填写 Agent 账号：client.json 路径由该账号家目录推导，签发结果以该账号为属主、权限 0600 落地。UI 签发的授权携带该连接注册时的写等级（`none`/`dml`/`ddl`），写等级为 `dml`/`ddl` 的连接在授权后仍可写，读语句始终可用。启动 UI 不等于安装查询 daemon；首次安装仍按服务安装流程完成。签发是单槽模型：每次签发都会替换 daemon 当前的唯一授权，被替换的连接需重新签发才能继续被查询。

写权限采用 `none / dml / ddl`，默认 `none`。写等级是连接的策略，必须在签发的授权里体现；数据库账号自身权限不会扩大范围。打包流程检查构建、CLI 版本及包文件完整性；这些检查不等于目标电脑上的服务安装或真实 MySQL/PostgreSQL 验收——真实库验收用 `tests/e2e-flow-windows.ps1`（配套 `tests/e2e-mysql-fixture.ps1` 建库建账号）。

## 注册信息存放位置

- 管理员手动运行时默认目录：Windows `%APPDATA%\db-operator`；Linux `/var/lib/db-operator`；macOS `/var/lib/db-operator`（服务私有目录）。`DB_OPERATOR_HOME` 可显式覆盖。
- Windows 服务默认私有目录：`%ProgramData%\db-operator`，由服务安装器限制访问；服务安装器同时把该目录的访问限制给服务身份。
- 客户端配置：管理员指定的 `client.json` 路径。Windows 通常位于 Agent 用户的 `%APPDATA%\db-operator`；Linux 为 `~/.config/db-operator/client.json`，macOS 为 `~/Library/Application Support/db-operator/client.json`（Agent 账号属主、0600）。也可通过 `DB_OPERATOR_CLIENT_CONFIG` 指定。
- 管理 UI：调用本机管理服务，连接注册数据由管理员工具存入服务私有目录；macOS / Linux 上每次签发都以服务身份执行，服务私有目录的证书、注册表、capability 记录不会被 Agent 账号写入。

以上目录不得指向源码仓库或本发布目录。本包不包含连接注册信息、用户凭据、capability、缓存或本机安装状态。不要把注册后的配置复制回仓库。

`release-manifest.json` 记录每个文件的 SHA-256。包旁的 `.sha256`（Windows 为 ZIP、macOS/Linux 为 `.tar.gz`）用于核对完整压缩包；这些是构建清单，不是注册数据。`prod/` 已被 Git 忽略。
