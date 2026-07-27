# 安全规范

## 摘要

本文件定义 ae-sdd 用户级 daemon、本地 IPC、角色 capability、文件、外部进程、插件和审计的安全边界。
适用场景：协议、session/delegation、路径读写、build/install、宿主适配和日志实现。

---

## 一、本地 endpoint 与身份

- daemon 默认只监听本地 IPC，禁止默认开放 TCP/HTTP。
- Windows Named Pipe DACL 必须只允许当前用户 SID；Unix runtime 目录权限必须为 `0700`，socket/manifest 为 `0600`。
- endpoint secret 必须由 OS CSPRNG 生成至少 256 bit，restart/upgrade 时轮换；明文只允许存在于 DACL/`0600` 保护的 manifest 与 daemon/client 当前进程内存，SQLite、普通日志和审计只记录 digest。
- client 从同一次原子 manifest snapshot 读取 `endpointToken + bootId + policyDigest`，handshake 请求携带 token 与 `expectedBootId/expectedPolicyDigest`；daemon 验证 OS peer 用户、constant-time token、protocol range 和两个 expected 值，client 再核对响应。token/OS mismatch 返回 `ENDPOINT_AUTH_FAILED`，boot/policy mismatch 返回 `ENDPOINT_STALE` 并重新读取 manifest。
- client 自报的 user、role、rootSessionId、parentSessionId、delegationId 和 childSessionId 均不可信，必须由 daemon/认证宿主上下文派生或核对。

## 二、角色、委派与宿主动作

- capability 必须绑定 workspace、session、turn、role、lineage、allowed operations、allowed paths、required deliverables、deadline、bootId 和 policy digest。
- 短期 capability 使用 daemon 每次 boot 生成并轮换的 Ed25519 private key 签名；private key 只驻留 daemon 内存，manifest 仅发布 keyId/public key。client/Hook 只读验签，不得持有可签名共享密钥；endpoint HMAC/token 不得复用为 capability key。
- child capability 只能是 parent grant 的真子集；root/series/task/reviewer 越权请求必须返回稳定拒绝码并记录审计。
- one-time claim 只保存 hash，必须 single-use、短 TTL，并与 host adapter、delegation、预期 role/lineage 关联。
- Host ACK 只证明命令接收；没有 trusted child claim/attestation 时禁止标记物理 session active。
- reviewer 必须与被审 worker 使用不同的 physical session；task/reviewer 不得继续派生 child。
- compact ACK 必须匹配 authenticated adapter、sessionId、previousGeneration、nextGeneration 和 hostActionId；缺失或错误 ACK 不得推进 generation。

## 三、文件与路径

- 所有项目路径必须先定位 approved root，再 canonicalize 已存在父级，并逐段拒绝越界 symlink、junction、mount/reparse point。
- 新建目标必须验证 canonical parent；禁止仅通过字符串前缀判断 containment。
- 所有跨平台 relative path 必须在 dry-run/plan 阶段拒绝 control character、`:`/Windows ADS、drive/UNC、空/`.`/`..` segment、trailing dot/space，以及 `CON/PRN/AUX/NUL/CLOCK$/COM1..9/LPT1..9` 等设备名；Apply 不得首次发现这些错误。
- mutation 必须使用 staging/同目录临时文件、显式权限、fsync 和 atomic promote；禁止在验证完成前覆盖原文件。
- plugin/artifact 路径禁止 `..`、绝对路径和跨 root link；content hash 必须在使用和 commit 前复核。
- prompt、transcript、claim、endpoint secret、完整 stdout/stderr 和用户 credential 不得写入普通日志、SQLite 或 root ContextProjection。

## 四、外部进程与输入

- 必须使用 `std::process::Command`/Tokio process 的 program + args 形式，禁止 `sh -c`、`cmd /c`、PowerShell string interpolation 或 `shell=true`。
- executable 必须来自 allowlist 或经 canonical path 验证；工作目录必须位于 approved root。
- 子进程必须有 deadline、输出字节上限、环境变量 allowlist、取消和 process-tree 清理；超限/超时返回 typed error，不得截断后伪 PASS。
- SQL 必须参数化；RPC/JSON/YAML 必须有 frame、深度、集合长度和字符串长度上限。
- 对用户可控文本执行日志转义，禁止 ANSI/control sequence 注入终端。

## 五、日志与审计

- requestId、workspaceId、sessionId、turnId、delegationId、hostActionId、compactId、workItemId、revision、eventSeq、fencingToken、policyDigest 和 outcome 必须可关联。
- 不记录 secret 正文；路径对普通 client 只返回 workspace-relative form，越界失败不得泄露其他 root。
- break lease、external conflict reconcile、canary/rollback、install/uninstall、Python runtime 删除属于高权限操作，必须记录 actor、reason、confirmation 和前后 digest。
- security finding、plugin scan、role denial 与 invalid ACK 必须保留不可变 evidence reference。

## 六、供应链与禁止事项

- 依赖/许可证/advisory 规则由 `technology-stack.md` 定义；release 必须通过 `cargo audit` 与 `cargo deny check`。
- 禁止在源码、fixture、配置、CLI 参数或环境默认值中硬编码 token、password、private key 或 endpoint secret。
- 禁止将 daemon 作为提权服务；所有写入必须受当前 OS 用户权限和 allowed-root 限制。
- 禁止在错误、timeout、scanner panic 或 host capability 缺失时 fail open。
