# GitHub Skills 下载报告

- **目标目录**:`D:\Item\ae-sdd\references\github-skills\`
- **日期**:2026-07-02
- **预期**:14 个仓库(Spring Boot + Spring Cloud + DDD 维度的 skill 候选)
- **实际落地**:13 / 14 ✅,1 个因网络受限未达 ⚠️
- **总大小**:211 MB
- **更新时间**:2026-07-02 15:40(二次拉取后)

---

## ✅ 已下载(12 个)

| 仓库 | Star | 文件数 | Manifest MD5 | HEAD commit |
|---|---:|---:|---|---|
| `rrezartprebreza/spring-boot-skills` | 128 | 88 | `52b6105b479f` | `d7204ce9` Add spring-batch skill |
| `a-pavithraa/springboot-skills-marketplace` | 61 | 58 | `05a178890774` | `efa55466` Migrate from Spring Retry |
| `giuseppe-trisciuoglio/developer-kit` | 296 | 1014 | `0cc021ed36af` | `306f428f` Merge origin/main |
| `jed1978/ddd-architecture-coach` | 5 | 20 | `222c14c652d5` | `c87ec9b9` parallel multi-BC dev |
| `cosbort/agent-skills` | 3 | 16 | `fec6d9d1cc23` | `3e707e85` Update author name |
| `helmedeiros/clean-code-skills` | 3 | 48 | `f1fa9d857f0d` | `b9805c90` branch quality review |
| `JhonatanMota/claude-skills` | 1 | 32 | `fae96c3c0fa6` | `633bdedf` Add Java/Spring Skill set |
| `joshipurvang/ai-agent-skills-microservices-assistant` | 1 | 21 | `eb2d0ac24bdc` | `7be0e522` AI Agent skills |
| `OtterMind/Nubase` | 445 | 901 | `e1e1c41e8a87` | `5b43c0a3` 兼容 Studio apikey JWT |
| `superheromeZzh/java-ut-coverage-loop` | 4 | 33 | `041d442c1478` | `50b5bbdc` agent-injection coverage |
| `vekzz-dev/opencode-skills` | 2 | 74 | `7ebd31512405` | `c0ecde43` align skill frontmatter |
| `ciembor/agent-rules-books` | 2063 | 201 | `207ddbdad231` | `9c876361` Update README.md |

### 下载方式说明
- 前 9 个:HTTPS `git clone --depth 1`(443 直连成功)
- 后 3 个(`java-ut-coverage-loop` `opencode-skills` `Nubase`):HTTPS 443 被网关限速,改走 SSH `git@github.com:`
- 所有仓库都是浅克隆(单 commit + working tree),无历史

---

## ⚠️ 未下载(2 个,网络受限)

| 仓库 | Star | 原因 | 大小 |
|---|---:|---|---:|
| `modu-ai/moai-adk` | 1110 | HTTPS 443 直连被掐 / SSH 中段 EOF / codeload tarball 30+MB 4 分钟没拉完 | ~36 MB |
| `jabrena/plinth` | 413 | 同上,大仓库出站持续丢包 | ~7 MB |

### 重试策略(已穷举)
1. HTTPS `git clone` × 4 次 — 前两次成功,后两次 `Connection reset` / `Failed to connect`
2. SSH `git clone` × 3 次 — 小仓库成功,大仓库 `early EOF`
3. codeload tarball × 多次 — 30+ MB 的仓库下载到 30+ MB 处持续被掐(速率从 1.7MB/s 跌到 0.5MB/s)

### 建议补救方案(任选其一)
- **断点续传**:用 `curl -C -` 续传 plinth/moai-adk 的 tarball,改分多次重连
- **换时段**:这台机当前出站到 github.com 被限速,深夜/早上可能放开
- **走代理**:如果你有可用的 HTTP 代理,我可以配 `git config http.proxy`
- **手工下载**:你浏览器直接下 tarball 放进 `github-skills/` 下,我帮解包

---

## 🗺️ 目录结构建议(本机下一步)

```
D:\Item\ae-sdd\references\github-skills\
├── spring-boot-skills\           ← 骨架层 1 (128★,Spring Boot 多 skill)
├── springboot-skills-marketplace\← 骨架层 2 (61★,Claude+Codex 双端)
├── ddd-architecture-coach\       ← DDD 战略层 (Context Map)
├── claude-skills\                ← DDD 战术补强 (Java/Spring 微服务 9 skills)
├── clean-code-skills\            ← TDD/SOLID 通用补强
├── agent-skills\                 ← Spring Boot enterprise best practices
├── ai-agent-skills-microservices-assistant\ ← Java 21 微服务助手
├── developer-kit\                ← 上层插件市场(1014 文件,Java 支持)
├── agent-rules-books\            ← 通用规则库(最大,2063★)
├── Nubase\                       ← 上层"AI 代码 → 真应用"框架
├── java-ut-coverage-loop\        ← 单测(Spring JUnit 5 + Mockito)
├── opencode-skills\              ← 单测(OpenCode Spring Boot testing)
└── (DOWNLOAD-REPORT.md ← 本文件)
```

---

## ⚙️ 已知环境问题(给后续操作参考)

这台机的 GitHub 网络状况(实测 2026-07-02):
- **HTTPS 443 直连**:间歇可用,大仓库(< 5MB)基本 OK,大仓库(> 10MB)会被掐
- **SSH 22 (git@github.com)**:稳定可用,本机已有 `id_ed25519` 配 `known_hosts`
- **codeload.github.com tarball**:和 HTTPS git 走同一通道,同样间歇限速

→ **后续克隆推荐直接走 SSH**:
```bash
GIT_SSH_COMMAND="ssh -o StrictHostKeyChecking=no -o BatchMode=yes" \
  git clone --depth 1 git@github.com:USER/REPO.git
```

---

## 🔄 复验命令

```bash
cd "/d/Item/ae-sdd/references/github-skills"
echo "=== 目录数 ==="
ls -d */ | wc -l
echo "=== 总大小 ==="
du -sh .
echo "=== 各仓库 HEAD commit ==="
for d in */; do
  cd "$d" 2>/dev/null
  printf "%-42s %s\n" "${d%/}" "$(git log --format='%H %s' -1 2>/dev/null | cut -c1-100)"
  cd ..
done
```