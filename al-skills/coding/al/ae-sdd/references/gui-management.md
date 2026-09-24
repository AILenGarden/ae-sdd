# GUI 管理验收（2026-09-13）

ae-sdd 负责发现和启动各能力现有的管理入口，不保存数据库信息，不代理连接 CRUD。
ALCoding 打开自身 Web 注册器；ALSpec 打开自身 registry.yaml 供编辑；无独立注册器的 al-ra、al-knowledge 显示说明入口。入口缺失时明确标记未安装。

DB Operator 负责自己的连接管理：列表、新建、编辑、保存前可选 SELECT 1 测试、删除。
编辑保持别名不变；密码不回显，留空保留；TLS 证书路径等未展示的配置保持原值。
删除流程是点击删除、取得短期一次性确认凭证、输入完整别名并再次确认。未经确认、凭证过期、重放或确认后修改连接均被拒绝。
删除清理连接配置、加密凭据、该连接 schema 缓存和对应 capability；新签发 capability 记录其 client 配置位置，只删除仍与签发内容相符的 client 文件。旧版没有位置记录的 client 文件无法定位，但其授权仍撤销。保留其他连接、共用 master key 和审计记录。删除不执行数据库 DROP/DELETE。
daemon 检查持久化授权是否仍有效；同名连接重建不会恢复旧授权。删除失败保留连接入口供重试，不报告成功。

运行时必须使用同一 Windows 服务数据目录；注册器启动脚本请求标准 Windows UAC，授权失败如实返回，不能将错误退出解释为宿主限制。

验证：native/tests/management.rs、cache/security/daemon/cli 回归；双方 tests/registry-ui.test.mjs 使用临时目录调用真实 admin 程序。真实业务连接不参与删除测试。

## 执行证据

- 当前运行包：0.1.0+codex.20260913064024；Codex plugin add 成功。
- Rust management/security/daemon/cache/cli 回归：19 passed；双方 Node HTTP 与页面脚本测试：3 passed。
- 插件 validate_plugin.py 通过。与操作前备份做 git diff --no-index 审核；diff --check 无空白问题。未执行 Git commit、push、merge、tag。
- 实际端口 17843、17842、8765：HTTP 200，标题分别为 ae-sdd 能力管理、DB Operator 连接管理、ALCoding SKILL Registry。
- Windows 服务 Running；管理员诊断确认服务二进制 SHA-256 与本次构建一致；现有连接状态 warm。
- CUA 浏览器通道 nodeRepl.fetch 失败，未完成真实浏览器点击与截图验收。
- 备份：C:/Users/EDY/AppData/Local/Temp/ae-sdd-gui-20260913-141848。
