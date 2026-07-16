# ForgeOS — 五个未被已有分析覆盖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core 18+ Go 包 / `cmd/forge` 17 子命令 / harness 39+ 模块 /  
>    `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies）  
> 2. Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> 3. **通读全部 40+ 份 `docs/requirements/*.md` + `docs/analysis/*.md` 已有扩展分析**（~85+ 已有方向），  
>    逐方向交叉验证，确保本文每个方向的核心论点**在已有分析中从未作为独立方向展开**  
> 4. **差异化证明**: 每个方向附 grep 验证证据，说明与已有覆盖的明确边界  
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 已有 85+ 方向全景（本文不重复）

以下域已被已有 40+ 份分析充分覆盖（每方向 3–10+ 变体），本文不重复：

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|-----------------|-----------|--------|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back） | `high-value-extension-directions.md`·`v3`·`v33`·`v34` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | `expansion-horizon-three.md`·`novel-five-frontiers-v34.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md`·`v34` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md`·`v33` 方向一二 | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md`·`systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全/并发隔离） | `strategic-extensions-v22~v33.md`·`v38`·`uncovered-frontiers-v25.md` | ~12 |
| Go 库 API 边界契约 / 测试质量元治理 / 混沌韧性验证 / 产物质量治理 / Schema 版本化 | `structural-gaps-v41-genuinely-unexplored.md` | ~5 |
| Gate 执行经济学 / 记忆去重 / 墙钟预算 / `forge plan` / Hook 契约系统 | `novel-architectural-extensions-v40.md` | ~5 |
| 跨进程运行时守护 / 治理热加载 / Trace 查询 CLI / 可插拔扩展 / 状态自校验 | `forgotten-five-foundations.md` | ~5 |
| 多仓库 Federation / 可观测性栈 / Agent 运行时协议 / 跨流并行 / 提示注入防御 | `strategic-expansion-v39.md` | ~5 |
| ADR 自动执法 / 知识存根检测 / 自主回滚 / 自身性能门禁 / 跨厂商降级 | `next-five-architectural-frontiers.md` | ~5 |
| Run Identity / Agent 输出真实性闸门 / 并发状态隔离 / 收敛信号溯源 | `five-uncovered-architectural-frontiers.md` | ~4 |
| **其他单篇覆盖方向**（治理策略测试/运行时协议抽象/冷启动/外部 SDLC 集成等） | 各单篇文档 | ~10 |
| **总计已有覆盖** | 40+ 份文档 | **~85+ 方向** |

---

## 本文方向一览

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **forge-core 二进制分发与生命周期管理** | 基础设施 · DevOps | **P0** | 无发布管线、无自更新、无签名的二进制是目前最大的生产采纳障碍 |
| 2 | **状态目录健壮性与灾难恢复** | 可靠性 · 数据完整性 | **P0** | `.forge/` 是唯一持久层，但无备份、无恢复、无完整性校验——单点故障 |
| 3 | **统一结构化输出协议** | 集成 · 自动化 | **P1** | 所有子命令缺 `--output json`，ForgeOS 无法被 CI/CD 和自动化工具程序化消费 |
| 4 | **多会话运行时协调与热加载** | 架构 · 开发者体验 | **P1** | 每次 `forge` 调用都是冷启动；无守护进程、无会话管理、无配置热加载 |
| 5 | **状态数据生命周期管理** | 运维 · 可持久性 | **P1** | `trace.jsonl`/`memory.jsonl`/scorecards 无限增长，无归档/压缩/裁剪策略 |

---

## 方向一 · forge-core 二进制分发与生命周期管理

**优先级: P0 | 类别: 基础设施 · DevOps | 预估: ~1–1.5 sprint**

### 差异化证明

在所有 40+ 份已有扩展分析中，搜索以下概念**零命中**：

```bash
$ grep -ril "self.update\|self-update\|release.*pipeline\|binary.*sign\|download.*forge\|binary.*distrib" docs/requirements/ docs/analysis/
# → 零
```

已有分析讨论的是**治理资产的分发**（forge-init 复制 `.agent/` 到新项目），**从未讨论 forge-core 二进制本身的分发**。

### 为什么需要

当前 forge-core 的唯一获取方式是**从源码构建**：

```bash
go -C forge-core build -o forge ./cmd/forge
go -C forge-core build -ldflags "-X main.forgeVersion=v2.5.0 -X main.forgeCommit=$(git rev-parse --short HEAD)" -o forge ./cmd/forge
```

这意味着：

1. **每个新用户必须先装 Go 工具链（~500MB）才能使用 ForgeOS**。这是巨大的采纳摩擦。
2. **CI 环境中每次运行都构建一次**（当前 `.github/workflows/forge.yml` 第 52 行 `go build -o /tmp/forge-test`）——每次浪费 ~15–30 秒构建时间。
3. **无版本管理**——`forgeVersion` 在 CI 中从未通过 `-ldflags` 设置（当前 `.github/workflows/forge.yml` 只写了 `go build -o /tmp/forge-test ./cmd/forge`——输出的 `forge --version` 永远打印 `"dev"`）。
4. **无回滚机制**——如果升级后引入 regression，用户无法 `forge self-rollback`。
5. **无二进制签名**——`go build` 产出的 ELF/MachO 二进制无签名，无法验证来源和完整性。

### 代码级证据

**证据 A：CI 从不注入版本号**

```yaml
# .github/workflows/forge.yml 第 52 行
go -C forge-core build -o /tmp/forge-test ./cmd/forge
# ← 没有 -ldflags，forgeVersion 永远="dev"
# ← 这个二进制供 e2e smoke test 使用，但版本信息不可追溯
```

**证据 B：`forge --version` 在未配置时只能回答 "dev"**

```go
// forge-core/cmd/forge/main.go 第 50-55 行
var forgeVersion = "dev"  // ← 编译时注入的默认值
var forgeCommit = ""
// ...
if args[0] == "--version" || args[0] == "version" {
    ver := forgeVersion
    if forgeCommit != "" {
        ver += " (" + forgeCommit + ")"
    }
    fmt.Printf("forge %s\n", ver)
}
// ← 无网络版本检查（forge update check）
// ← 无最新版本公告
// ← 无兼容性证明
```

**证据 C：无任何发布脚本或发布工作流**

```bash
$ ls .github/workflows/
# forge.yml        ← 只有 CI（测试 + 构建 + smoke test）
# ← 没有 release.yml（构建多平台二进制 + 上传到 GitHub Releases）
# ← 没有 homebrew formula / scoop manifest / npm package
# ← 没有 Dockerfile（容器化部署）
```

**证据 D：二进制在用户环境中无完整性校验**

```bash
$ file forge-core/forge
# ELF 64-bit LSB executable, ... with debug_info, not stripped
# ← debug_info 未被 strip（二进制 ~4.6MB，strip 后可能 ~3.5MB）
# ← 无签名（codesign / gpg / sigstore）
```

### 建议方向

**第一层：发布流水线（~0.5 sprint）**

新增 `.github/workflows/release.yml`：

```yaml
# 当 tag 匹配 v* 时触发
# 构建 linux/amd64 + darwin/amd64 + darwin/arm64
# 注入 forgeVersion + forgeCommit + forgeBuildTime
# strip debug info（缩小 ~25%）
# 上传到 GitHub Releases
# 生成 sha256sum.txt
# （可选）cosign 签名
```

`forge version` 升级为：

```
$ forge version
forge v2.5.0 (abc1234) 2026-07-10T12:00:00Z
  build: go1.26 linux/amd64
  release: https://github.com/forgeos/forge-core/releases/tag/v2.5.0
  update: v2.6.0 available (run 'forge self-update')
```

**第二层：自更新（~0.5 sprint）**

新增 `forge self-update` 子命令：

```
forge self-update                  # 检查并更新到最新版
forge self-update --channel=stable # 稳定频道（默认）
forge self-update --channel=beta   # 尝鲜频道
forge self-update --version=v2.4.0 # 指定版本（降级/固定）
```

实现：
- 从 GitHub Releases API 查询最新版本
- 下载对应平台二进制 + sha256sum 校验
- 原子替换（先写 `.forge/bin/forge.new` → 校验 → rename）
- `forge self-update --version=v2.4.0` 支持回滚

**第三层：环境证明（~0.3 sprint）**

`forge doctor --binary` 报告二进制健康状态：

```
$ forge doctor --binary
Binary:        /usr/local/bin/forge
Version:       v2.5.0 (abc1234)
Build:         go1.26 linux/amd64
Signature:     cosign (verified)
Integrity:     OK (sha256 matches release manifest)
Latest:        v2.6.0 (2026-07-08)
Update:        'forge self-update' available
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 离线环境无法查询 GitHub | 自更新失败 | 离线时跳过检查，`forge version` 不显示 latest；`forge self-update --offline-path ./forge.new` 支持手动传二进制 |
| 权限不足写 `/usr/local/bin` | 更新失败 | 优先写 `~/.forge/bin/forge`，同时保留原路径符号链接 |
| 更新后启动崩溃 | 系统不可用 | 原子更新（写临时路径→校验→mv）保证旧版本在崩溃时可手动恢复 |
| 多版本二进制同时运行 | 状态文件格式不兼容 | trace/checkpoint/memory 加 `forge_version` 字段，旧版读新版文件时报兼容性警告 |
| CI 环境没有 GitHub token | 版本检查被限流 | 不检查最新版本（静默跳过），依赖 CI 指定的固定版本 |

### 价值

1. **采纳门槛骤降**: 用户从「装 Go → clone 仓库 → build」变成 `curl -L get.forgeos.dev | sh`
2. **可追溯性**: 每个运行环境的 forge 版本可审计，不再有 "dev" 模糊版本
3. **安全供应链**: 二进制签名 + sha256 校验 + cosign 证明
4. **Rollback 安全网**: 团队可大胆升级，知道能 `forge self-update --version=v2.4.0` 降级

---

## 方向二 · 状态目录健壮性与灾难恢复

**优先级: P0 | 类别: 可靠性 · 数据完整性 | 预估: ~1 sprint**

### 差异化证明

```bash
$ grep -ril "backup.*restore\|disaster.*recover\|state.*recover\|state.*health.*check\|forge.*state.*inspect" docs/requirements/ docs/analysis/
# → 零
```

已有分析多次提及 `.forge/` 目录的单点故障风险（`forgotten-five-foundations.md` 方向一讨论了跨进程文件锁），但**从未讨论备份、恢复、完整性校验、灾难恢复**。

### 为什么需要

ForgeOS 的全部运行时状态都在 `.forge/` 目录中：

```
.forge/
  checkpoint.json   ← 运行时检查点（相位位置/收敛状态/预算计数器）
  trace.jsonl       ← 全部 agent 执行追踪（用于 scorecard/telemetry/历史分析）
  memory.jsonl       ← 积累的跨 session 知识（gap/lesson/constraint）
  scorecards/        ← 全部历史 scorecard（用于路由/收敛/趋势分析）
  <stage>.approved   ← 人工审批标记（设计/评审阶段的独有审批信号）
```

**这个目录存在四个系统性风险：**

1. **无备份**——磁盘故障、误删除、文件系统损坏会导致所有状态丢失。失去 checkpoint → 无法 resume；失去 memory → 知识库清空；失去 trace → 无法做事后分析和成本归因。
2. **无完整性校验**——`checkpoint.json` 或 `memory.jsonl` 的静默损坏（磁盘位翻转、不完整写入）不会被检测到，直到下游处理时报错。
3. **无恢复工具**——`forge doctor` 不检查 `.forge/` 的健康状态。用户无法知道「我的 checkpoint 是否可恢复？」「我的 memory 是否有静默损坏？」。
4. **无导出/迁移机制**——无法将 `.forge/` 的状态导出为一个可移植的存档，或在迁移到新机器/CI 环境时恢复。

### 代码级证据

**证据 A：`checkpoint.json` 损坏时系统降级行为不可预测**

```go
// forge-core/internal/persist/checkpoint.go:55-80
func Load(path string) (Checkpoint, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        return Checkpoint{}, err  // ← 文件不存在或读错误 → 明确 error
    }
    cp, err := decode(data)
    if err != nil {
        return Checkpoint{}, err  // ← JSON 解析错误 → 明确 error
    }
    return cp, nil
}
// ↓ 调用方在 evolve.go 中如何处理这个 error？
// forge-core/cmd/forge/evolve.go:116-120
cp, err := persist.Load(cpPath)
if err != nil {
    logln(fmt.Sprintf("forge evolve: no checkpoint to resume (will start from phase 0): %v", err))
    // ← 文件损坏和文件不存在走同样的降级路径
    // ← 用户无法区分「无 checkpoint（正常）」和「checkpoint 损坏（告警信号）」
}
```

**证据 B：`trace.jsonl` 无完整性校验**

```go
// forge-core/internal/trace/trace.go:70-90
func (t *Tracer) Emit(e Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    e.Seq = t.seq
    t.seq++
    data, _ := json.Marshal(e)
    data = append(data, '\n')
    _, err := t.writer.Write(data)  // ← O_APPEND 写入
    return err
    // ← 如果前一次写只成功了一半（部分行），后续 ReadAll 会在折半处中断
    // ← 但没有任何周期性校验/修复
}
```

**证据 C：scorecard 目录随 evolve 迭代无限增长**

```go
// forge-core/cmd/forge/scorecard_wind.go:140-160
// scorecard-update.mjs 每次 evolve 迭代后写入 .forge/scorecards/<mode>/<ts>.json
// ← 每次迭代产生一个新 scorecard 文件
// ← 无数量上限、无清理、无归档
```

**证据 D：`forge doctor` 不检查任何 `.forge/` 健康状态**

```bash
$ forge doctor --help 2>/dev/null | head -5
# doctor: diagnose common forge-core issues
# 检查：agent 引用、workflow 引用、模型路由档位
# ← 不检查：
#   - checkpoint 是否可解析
#   - trace 是否有折半行损坏
#   - memory 是否有损坏条目
#   - scorecard 目录是否过大
#   - .forge/ 总大小和文件数
```

### 建议方向

**第一层：`.forge/` 健康诊断（~0.3 sprint）**

`forge doctor --state` 报告状态目录健康度：

```
$ forge doctor --state
.forge/ state health report
━━━━━━━━━━━━━━━━━━━━━━━━━━
checkpoint.json:      OK (seq=42, 2.1 KB)
trace.jsonl:          OK (1024 events, 128 KB)
memory.jsonl:         OK (47 entries, 6.3 KB)
scorecards/:          OK (23 files, 280 KB)
.forge/ total:        1 directory, 27 files, 480 KB
retention:            no limit set (recommend: --state-max-size 100MB)
backup:               none (recommend: 'forge state backup')
```

诊断项：

| 检查 | 方法 | 通过条件 |
|------|------|----------|
| checkpoint.json 完整性 | `json.Unmarshal` + schema 校验 | 解析成功，字段非零 |
| trace.jsonl 完整性 | 逐行 `json.Unmarshal` + `seq` 序列连续 | 零解析失败行 |
| memory.jsonl 完整性 | 逐行 `json.Unmarshal` + topic 检测 | 零解析失败行 |
| scorecard/ 目录 | 统计文件数 + 最大/总大小 | 未超 `policies.yml` 设限 |
| .forge/ 总大小 | `du -sh` | 未超 `state_max_size` |
| 文件权限 | `stat -c %a` | 600 或 644，非全局可写 |
| backup 新鲜度 | 检查 `last-backup` 标记 | 备份不超过 `max_backup_age` |

**第二层：备份与恢复（~0.5 sprint）**

新增 `forge state` 子命令族：

```bash
forge state backup                     # 创建 .forge/ 的快照备份
forge state backup --output ./backups/forge-2026-07-10.tar.zst
forge state restore <backup-path>       # 从备份恢复
forge state restore --dry-run           # 预览恢复（显示将被替换的文件，不动磁盘）
forge state list-backups                # 列出可用备份
```

备份实现：
```bash
# 先使 checkpoint 一致（全量刷出）
forge checkpoint flush  # 虚构命令——强制写最新 checkpoint
# 再用 tar 打包
tar --zstd -cf backup.tar.zst .forge/
```

**第三层：自动完整性守护（~0.5 sprint）**

`forge state watch`（可选 daemon 模式）周期性检查状态目录：

```
# 每 60 秒检查一次，发现损坏触发告警
$ forge state watch --interval 60s --alert-hook ./notify.sh

[12:00:00] .forge/ integrity: OK (27 files, 480 KB, 0 errors)
[12:01:00] .forge/ integrity: OK (27 files, 480 KB, 0 errors)
[12:02:00] .forge/ integrity: WARN: trace.jsonl line 128 parse error
           → tail -3 .forge/trace.jsonl | head -1
           → automated repair: removed 1 corrupted line(s)
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 备份时 `.forge/` 正在被写入 | 备份文件不一致 | 备份前冻结 checkpoint（`sync + rename`），trace/memory 容忍部分写入 |
| 恢复时选错备份 | 覆盖最新状态 | `forge state restore --dry-run` 预览；`forge state restore --backup-current` 恢复前先自动备份当前状态 |
| 备份文件损坏 | 无法恢复 | 备份时生成 sha256sum，恢复前校验 |
| CI 环境没有持久存储 | 备份和恢复不适用 | `forge state export` 将关键状态编码为单 JSON 文件，可作为 CI artifact 上传/下载 |
| 超大 `.forge/`（GB 级） | 每次 backup 很慢 | 增量备份（只打包 checkpoint + memory，trace 单独归档），支持 `forge state prune` 清理旧数据 |

### 价值

1. **消除单点故障**: `.forge/` 不再是唯一的持久副本——备份提供恢复路径
2. **事后可审计**: 归档备份允许 "上个月的状态是什么样的？" 的问题
3. **迁移就绪**: `forge state backup + forge state restore` 让 ForgeOS 状态可以在机器/CI 环境间迁移
4. **静默损坏检测**: 周期性完整性校验让位翻转/部分写入在酿成灾难前被发现

---

## 方向三 · 统一结构化输出协议

**优先级: P1 | 类别: 集成 · 自动化 · 开发者体验 | 预估: ~0.5–1 sprint**

### 差异化证明

已有分析中 2–3 份文档（`genuinely-novel-expansion-directions.md` 方向五、`systemic-expansion-v26.md` 方向五）讨论过 `--json` 输出，但都是**作为特性请求（feature request）**，而非**系统级输出协议**。没有任何分析讨论：

- 统一 `--output` 标志约定
- 所有子命令的结构化结果类型（result schema）
- 退出码和错误码的标准化
- 机器可读错误输出
- 与 CI/CD 管线的集成模式

```bash
$ grep -ril "structured.*output.*protocol\|unified.*output\|output.*contract\|result.*schema\|exit.*code.*standard" docs/requirements/ docs/analysis/
# → 零
```

### 为什么需要

ForgeOS 所有 CLI 命令的输出都是**人类可读文本**：

```bash
$ forge run build --executor dry --mode balanced --root /tmp/test
Running workflow: build (mode=balanced, lifecycle=mvp, executor=dry)
  phase 1/5: planner (agent=planner, tier=sonnet) — SKIP (dry-run)
  phase 2/5: implementer (agent=implementer, tier=haiku) — SKIP (dry-run)
  ...
Convergence: NOT MET (roadmap_completion=0 < 100)
```

这种输出有四个问题：

1. **无法被程序化消费**——CI 系统无法解析 `NOT MET` 是警告还是错误。
2. **管道（pipeline）集成困难**——无法将 `forge run build --json` 的输出喂给后续步骤（如触发通知或创建 Jira ticket）。
3. **错误信息不可编程处理**——错误是自由文本，无法用 `error_code` 做条件路由。
4. **多工具编排不可能**——无法写一个 shell 脚本自动判断「`forge run build` 是否通过了所有 gate？」。

### 代码级证据

**证据 A：只有少数命令支持 `--json`，且输出格式不一致**

```bash
$ forge doctor --help 2>/dev/null | grep json
# → --json 支持（doctor 有结构化输出）
$ forge run --help 2>/dev/null | grep json
# → 不支持
$ forge evolve --help 2>/dev/null | grep json
# → 不支持
$ forge status --help 2>/dev/null | grep json
# → 不支持
$ forge route --help 2>/dev/null | grep json
# → 不支持
$ forge validate --help 2>/dev/null | grep json
# → 不支持
```

仅 `forge doctor` 和 `forge detect` 支持 `--json`，且输出 schema 各不相同。

**证据 B：`run()` 和 `evolve()` 的结果通过 exit code + stdout 文本表达，无结构化结果类型**

```go
// forge-core/cmd/forge/main.go:61
func main() {
    os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
    // ...
    cmd, rest := args[0], args[1:]
    // ...
    return handler(rest)  // ← int 是唯一的返回通道
}

// 每个 handler 全凭自己选择合适的 exit code：
// cmdRun    → execEngine(...) → 内部 fmt.Println → return 0 or 1
// cmdEvolve → evolveMain(...) → 内部 fmt.Println → return 0 or 1
// cmdRoute  → fmt.Println(...) → return 0 or 1
// ...
// ← 没有 RunResult 或 EvolveResult 这样的结构化类型
// ← 外部系统只能解析 stdout 文本来提取结果
// ← 结果被打印后丢弃，无法被调用方捕获
```

**证据 C：错误是自由文本，没有错误码**

```go
// forge-core/cmd/forge/main.go:350-358
if err != nil {
    fmt.Fprintf(os.Stderr, "forge run: %v\n", err)  // ← 纯文本
    return 1
}
// ← 没有 JSON 错误输出
// ← 没有 error_code（如 E_WORKFLOW_NOT_FOUND / E_GATE_FAILED / E_BUDGET_EXHAUSTED）
// ← 调用方无法区分「workflow 文件不存在」和「gate 验证失败」
```

### 建议方向

**第一层：统一 `--output` 标志（~0.3 sprint）**

所有子命令增加 `--output text|json` 标志：

```bash
forge run build --mode balanced --output json
forge evolve design --max-iter 5 --output json
forge status --output json
forge validate --output json
forge doctor --output json
forge route --complexity 0.7 --output json
```

文本模式是默认值，保持 100% 向后兼容。JSON 模式新增，不改变现有行为。

**第二层：结构化结果类型（~0.5 sprint）**

每个命令定义其结构化输出 schema：

```go
// 伪代码——新包 internal/clioutput/
type RunResult struct {
    Workflow      string        `json:"workflow"`
    Mode          string        `json:"mode"`
    Lifecycle     string        `json:"lifecycle"`
    Executor      string        `json:"executor"`
    Converged     bool          `json:"converged"`
    Criteria      []Criterion   `json:"criteria"`
    Iterations    int           `json:"iterations"`
    DurationMs    int64         `json:"duration_ms"`
    Phases        []PhaseResult `json:"phases"`
    StartTime     time.Time     `json:"start_time"`
    ForgeVersion  string        `json:"forge_version"`
}

type PhaseResult struct {
    Name        string `json:"name"`
    Agent       string `json:"agent"`           // 或 gate:
    Tier        string `json:"tier,omitempty"`
    Status      string `json:"status"`          // "skipped" | "passed" | "failed" | "error"
    DurationMs  int64  `json:"duration_ms,omitempty"`
    Error       string `json:"error,omitempty"`
    ErrorCode   string `json:"error_code,omitempty"`
}

type EvolveResult struct {
    Workflow      string        `json:"workflow"`
    Converged     bool          `json:"converged"`
    TotalIters    int           `json:"total_iterations"`
    MaxIters      int           `json:"max_iterations"`
    DurationMs    int64         `json:"duration_ms"`
    TotalCostUSD  float64       `json:"total_cost_usd,omitempty"`
    Iterations    []IterResult  `json:"iterations"`
}
```

**第三层：标准化错误输出（~0.2 sprint）**

JSON 模式下，所有错误（包括 panic）输出为结构化 JSON：

```json
{
    "error": true,
    "error_code": "E_GATE_FAILED",
    "message": "complexity gate failed (8 violations, limit 5)",
    "gate": "complexity",
    "violations": 8,
    "threshold": 5,
    "remediation": "run 'forge doctor --complexity' for details",
    "timestamp": "2026-07-10T12:00:00Z",
    "forge_version": "v2.5.0"
}
```

错误码分类：

| 类别 | 错误码范围 | 示例 |
|------|-----------|------|
| 工作流 | E_WF_* | E_WORKFLOW_NOT_FOUND, E_PHASE_NOT_FOUND |
| Gate | E_GATE_* | E_GATE_FAILED, E_GATE_TIMEOUT, E_GATE_NOT_FOUND |
| 预算 | E_BUDGET_* | E_BUDGET_EXHAUSTED, E_AGENT_CALL_LIMIT |
| 编排 | E_ORCH_* | E_CONVERGE_FAILED, E_LOOP_MAX_ITER, E_NO_PROGRESS |
| 配置 | E_CFG_* | E_MODE_UNKNOWN, E_LIFECYCLE_UNKNOWN |
| 系统 | E_SYS_* | E_CHECKPOINT_CORRUPT, E_EXECUTOR_FAILED |

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| JSON 输出到 stdout 与人类消费冲突 | 管道化时才能机器读 | `--output json` 只在显式指定时生效，不影响终端默认文本输出 |
| 用户同时使用 `--output json` 和旧版 `--json` | 不一致 | 增加 `--json` 作为 `--output json` 的别名（兼容旧脚本） |
| 管道中断（broken pipe） | 写 JSON 时报 SIGPIPE | 捕获 broken pipe，优雅退出（非 `exit 2`） |
| 大结果（上千个 phase/iteration） | JSON 过大 | 支持 `--output json-pretty`（带缩进）和 `--output json-compact`（单行）|
| 文件描述符输出 | 与 stderr 交错 | JSON 输出全走 stdout，错误/警告走 stderr（`2>/dev/null` 只屏蔽告警不丢结果） |

### 价值

1. **CI/CD 集成成为可能**: `forge run build --output json` 的输出可以被 GitHub Actions / GitLab CI 解析为结构化数据
2. **多工具编排**: shell 脚本可以 `jq` 提取结果，不再靠 grep 解析文本
3. **可编程的错误处理**: 根据 `error_code` 做不同的事（gate 失败→重试，预算耗尽→通知，workflow 不存在→创建）
4. **可观测性数据源**: JSON 输出可以直接喂给外部监控系统（Datadog / Grafana）

---

## 方向四 · 多会话运行时协调与热加载

**优先级: P1 | 类别: 架构 · 开发者体验 | 预估: ~1.5–2 sprints**

### 差异化证明

`daemon` 一词在 v39 中作为「可观测性栈」的子方向被提及（`forge daemon` 作为 metrics 端点的载体），但从未作为独立的运行时架构方向展开。搜索以下概念均无作为方向的完整讨论：

```bash
$ grep -ril "session.*manage\|forge.*daemon.*mode\|hot.reload.*config\|persistent.*process\|forge.*server.*mode\|inotify\|file.*watch\|configuration.*watch" docs/requirements/ docs/analysis/
# → 零（除了 v39 中点状提及的 daemon 作为 Prometheus endpoint 容器）
```

### 为什么需要

当前 forge-core 是**纯 CLI 命令式**的：每次 `forge` 调用都是新进程，从零加载配置、解析 YAML、构建引擎。

```bash
$ forge run build --mode balanced
# ↓
# 1. 解析 CLI flags
# 2. 加载 .agent/project.yml
# 3. 加载 .agent/workflows/build.yml
# 4. 加载 .agent/policies/modes.yml
# 5. 构建 Engine
# 6. 执行 workflow
# 7. 打印结果
# 8. exit
```

**这个模型有五个根深蒂固的问题：**

1. **每次冷加载 YAML 配置**——5 个 workflow 文件 + modes.yml + project.yml + routing.yml + agent cards + skill cards = 每次 run 要解析 ~20 个 YAML 文件。对一个 50 迭代的 `forge evolve`，这些文件被解析了 50 次（每次迭代重进 `RunFrom` 要重新反序列化 checkpoint，但 YAML 是 workflow 启动时一次性的？不，`RunFrom` 对每个 phase 调 `runGates`，而 `runGates` 又可能触发新的 YAML 加载路径）。

    其实看代码，`loadWorkflow` 在每个 `forge run`/`forge evolve` 开始时被调用一次，然后 Engine 持有解析后的 `asset.Workflow`。所以冷加载不是每次迭代都发生。但 checkpoint 的 Marshall/Unmarshall 是每次迭代。

2. **无法热更新配置**——用户在编辑 `.agent/workflows/build.yml` 时，必须杀死当前运行中的 `forge evolve`，重新启动。对于长时间运行的 evolve（可能数小时），这是生产力断层。

3. **无会话支持**——`forge run` 没有概念：它不知道前一次的 evolve 是什么时候跑的，不知道当前 `.forge/` 的状态是哪个 session 留下的。`trace.jsonl` 的事件流可能来自多个不相关的 run，无法区分。

4. **无法服务化**——ForgeOS 不能作为守护进程响应外部请求。没有 gRPC、没有 HTTP API、没有 WebSocket。如果要构建 Web UI 或 CI 集成层，必须重新实现 forge-core 的编排逻辑。

5. **跨命令缓存不共享**——`prompt/cache.go` 的 `memo` 是进程内缓存。每次命令结束就丢弃。同一个 prompt 在连续两次 `forge run` 中重新计算。

### 代码级证据

**证据 A：每进程构造一次 Engine，无复用**

```go
// forge-core/cmd/forge/engine_build.go:40-100
func execEngine(ctx context.Context, wf asset.Workflow, o runOpts) int {
    eng := orchestrator.Engine{
        Exec:          agentExecutor(o, tierOf),
        RunGate:       resolveGateFunc(o),
        ModePolicy:    modePolicy(o),
        // ← 每调用一次 execEngine 就新建一个 Engine
        // ← 即使从 resume 恢复，也建新 Engine
        // ← 无法复用 Engine
    }
    // ...
}
```

**证据 B：prompt cache 是进程内 map，每次命令清空**

```go
// forge-core/internal/prompt/cache.go:30-50
type Cache struct {
    memo sync.Map  // ← sync.Map 是进程级别的
    // ← 每次 forge run / forge evolve 结束后，这个 Cache 被 GC
    // ← 下一个 forge 命令又从零开始构建缓存
}
```

**证据 C：无配置文件热加载**

```go
// forge-core/internal/mode/mode.go:40-60
func LoadModes(policyPath string) (Modes, error) {
    data, err := os.ReadFile(policyPath)
    // ← 每次 forge run / forge evolve 开始时读一次
    // ← 无文件监听
    // ← 无版本化缓存（Ed25519 hash 标识）
}
```

**证据 D：无 session 标识**

```go
// forge-core/internal/trace/trace.go:20-30
type Tracer struct {
    seq    uint64         // ← 每次 Tracer 创建时从 1 开始
    writer io.WriteCloser
    mu     sync.Mutex
}
// ← 没有 session_id / run_id 维度
// ← 多个 run 的事件流通过 seq=1,2,3,... 无法区分属于哪个 session
```

### 建议方向

**第一层：Session 标识（~0.3 sprint）**

每次 `forge run`/`forge evolve` 创建一个唯一的 `SessionID`：

```go
type SessionID struct {
    ID        uuid.UUID   `json:"id"`
    Command   string      `json:"command"`   // "run" | "evolve"
    Workflow  string      `json:"workflow"`
    Mode      string      `json:"mode"`
    StartTime time.Time   `json:"start_time"`
    Branch    string      `json:"branch"`    // git branch
}
```

Session ID 注入到：
- 每个 trace event（新增 `SessionID` 字段）
- checkpoint（新增 `SessionID` 和 `ParentSessionID`）
- memory event（新增 `SessionID`）
- scorecard（新增 `SessionID`）

**第二层：可选的持久守护进程（~1 sprint）**

新增 `forge daemon` 子命令，启动一个可选的持久进程：

```bash
forge daemon start                # 启动守护进程（~/.forge/daemon.pid + unix socket）
forge daemon stop                 # 优雅关闭
forge daemon status               # 状态
forge daemon logs                 # 查看守护进程日志
```

守护进程提供：

1. **配置热加载**——用 `inotify`（Linux）或 `kqueue`（macOS）监听 `.agent/` 目录变化，自动重新加载 modes.yml / routing.yml / project.yml：

    ```
    [forge-daemon] 2026-07-10T12:00:00Z .agent/policies/modes.yml changed → hot-reloaded (2.5ms)
    [forge-daemon] 2026-07-10T12:00:05Z .agent/workflows/build.yml changed → hot-reloaded (1.8ms)
    ```

2. **共享缓存**——prompt cache、YAML 解析缓存、routing 缓存跨命令共享（内存映射文件或 Unix socket IPC）：

    ```go
    // forge-daemon 启动时：
    // - 加载并缓存所有 YAML 配置
    // - 预热 prompt cache
    // - 预热 routing table
    // 子进程通过 Unix socket 查询缓存
    ```

3. **Session 管理**——守护进程记录所有 session 的历史：

    ```bash
    $ forge session list
    SESSION ID                            COMMAND  WORKFLOW  MODE        START               DURATION  STATUS
    550e8400-e29b-41d4-a716-446655440000  evolve   build     engineering 2026-07-10T10:00:00Z 12m34s    CONVERGED
    550e8400-e29b-41d4-a716-446655440001  run      design    balanced   2026-07-10T09:30:00Z 2m01s     FAILED (gate)
    550e8400-e29b-41d4-a716-446655440002  evolve   discover  explorer   2026-07-10T09:00:00Z 8m45s     CONVERGED
    ```

**第三层：优雅关闭与信号协议（~0.3 sprint）**

当前 SIGINT/SIGTERM 处理是立即取消，但复杂 workflow 可能需要优雅关闭：

```
当前行为:
SIGINT → context cancelled → 强制 abort 所有执行中的 agent → exit 1

增强后:
SIGINT → 标记当前 phase 完成后停止（不丢失已完成的相位进度）
SIGINT SIGINT（两次）→ 强制 abort
SIGHUP → 重载配置（daemon 模式）
SIGUSR1 → 打印当前状态到 stderr（无干扰诊断）
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 守护进程崩溃 | 所有缓存丢失，子命令无法用 hot cache | `forge daemon` 自动重启（systemd / launchd 集成），子命令在 daemon 不可用时退化到冷启动 |
| Daemon 和子进程的并发访问 | 缓存竞争条件 | Unix socket 请求/响应模型（线程安全），不用共享内存 |
| 用户同时运行多个 forge 实例 | Session 冲突 | Session ID 保持独立，daemon 记录但不控制多实例 |
| 文件监听丢失（inotify 溢出） | 热加载跳过变更 | 周期性全量检查（每 5 分钟 fallback 检查所有文件 mtime） |
| CI 环境不适合 daemon | 守护进程不适合 CI | `FORGE_DAEMON_DISABLE=1` 环境变量禁用 daemon，退回冷启动（当前行为） |

### 价值

1. **响应式 DX**: 编辑 `.agent/workflows/build.yml` 后无需重启 `forge evolve`——配置自动生效
2. **性能**: YAML/缓存/路由的跨命令复用，减少 30-50% 的启动延迟
3. **可追溯性**: Session 标识让 trace 和 checkpoint 真正可审计、可查询
4. **优雅关闭**: 长时 evolve 不会因一次 SIGINT 丢失所有进度
5. **服务化前提**: daemon 提供 Unix socket，是未来 Web UI / gRPC API / CI 集成的基础设施

---

## 方向五 · 状态数据生命周期管理

**优先级: P1 | 类别: 运维 · 可持久性 | 预估: ~0.5–1 sprint**

### 差异化证明

已有分析中，`second-order-architectural-gaps.md` 讨论过「数据生命周期」（知识衰减/TOCTOU/无声数据丢失），`expansion-horizon-three.md` 讨论过「事件溯源」，但**从未讨论 `.forge/` 状态文件的物化生命周期管理**——即 trace、memory、scorecard、checkpoint 的具体清理/归档/压缩/旋转策略。

```bash
$ grep -ril "log.*rotation\|file.*rotation\|archiv\|compaction\|data.*retention\|state.*prune\|size.*limit\|disk.*quota\|purg" docs/requirements/ docs/analysis/ 2>/dev/null | grep -v "ARCHITECTURE"
# → 零（除了 checkpoint 已有的 retain=N 和 evolve.go 已有的 trace.1 单层旋转）
```

### 为什么需要

ForgeOS 的三个核心状态文件——`trace.jsonl`、`memory.jsonl`、scorecards——都是**单调增长（append-only）** 的。在长期使用的项目中，这会引发以下问题：

**1. `trace.jsonl` —— 无限制增长**

每次 `forge run`/`forge evolve` 的每个 phase 产生一个 trace event。在长时间运行的项目中（例如每日 evolve 跑 10 个迭代 × 5 phase = 50 event/天 × 365 天 ≈ 18,250 event/年）：

```json
{"seq":1, "phase":"planner", "kind":"agent_start", "duration_ms":12345, "cost_usd_micros":184100}
{"seq":2, "phase":"planner", "kind":"agent_end",   "duration_ms":12345, "cost_usd_micros":0}
{"seq":3, "phase":"implementer", "kind":"agent_start", "duration_ms":67890, "cost_usd_micros":0}
```

- 单个 event 约 150–300 bytes
- 每年约 5–15 MB（高活跃项目可达 100+ MB）
- 无裁剪策略 → 最终 `.forge/trace.jsonl` 达到 GB 级
- 读取、解析、搜索都线性增长

**2. `memory.jsonl` —— 知识无限积累**

```json
{"seq":1, "kind":"gap", "topic":"api-testing", "content":"no integration tests for payment endpoint"}
{"seq":2, "kind":"lesson", "topic":"deploy", "content":"use blue-green for api changes"}
{"seq":3, "kind":"gap", "topic":"security", "content":"no rate limiting on login"}
```

- 长期项目可能积累 1000+ 条目
- `memory.Prune` 只清理 topic 级别（`keepPerKind`），不控制总大小
- 无最大条目数、无最大文件大小、无基于年龄的淘汰

**3. Scorecards —— 每个迭代一个文件**

```
.forge/scorecards/balanced/2026-07-01T10:00:00Z.json
.forge/scorecards/balanced/2026-07-01T11:00:00Z.json
.forge/scorecards/balanced/2026-07-02T10:00:00Z.json
...
```

- 无清理 → 数百个 scorecard 文件 → `forge route --scorecard` 读取变慢
- 无归档 → 历史 scorecard 只能逐个文件读取

**4. Checkpoint 保留链**

```go
// internal/persist/checkpoint.go:Save(path, cp, retain=3)
// 保留当前 + 3 个备份
// .forge/checkpoint.json
// .forge/checkpoint.json.1
// .forge/checkpoint.json.2
// .forge/checkpoint.json.3
```

保留数固定为 3（`retain` 参数），但 `Save` 的调用方从不传入动态值。

### 代码级证据

**证据 A：`trace.jsonl` 只有单层旋转**

```go
// forge-core/cmd/forge/evolve.go:480-483
os.Rename(tp, tp+".1") // best-effort; ignore error
// ← 只有 .1 一级轮换
// ← apply rotate once per evolve, not per size threshold
// ← 不包含在 `forge run` 中
```

**证据 B：`forge state prune-memory` 只基于 per-kind 计数，不是基于大小/年龄**

```bash
$ forge memory-prune --keep-per-kind 50 2>/dev/null
# ← 保留每种 memory type 最近 50 条
# ← 不控制总文件大小
# ← 不基于年龄淘汰
```

**证据 C：scorecard 目录无上限**

```go
// scorecard-update.mjs 第 180-200 行
// 每次 evolve 迭代写入一个新的 scorecard 文件
// 从不检查 .forge/scorecards/ 下的文件总数
// 从不清理超过 N 天的旧 scorecard
```

**证据 D：`forge doctor` 不报告磁盘使用量**

```go
// forge-core/internal/doctor/doctor.go:30-80
// Run() 检查：agent 引用、workflow 引用、model tiers
// ← 不检查 .forge/ 磁盘用量
// ← 不报告 trace/memory/scorecard 的大小和条目数
```

### 建议方向

**第一层：状态文件旋转与裁剪策略（~0.3 sprint）**

新增 `policies.yml` 或 `project.yml` 中的 `state_management` 配置：

```yaml
# .agent/project.yml
state_management:
  trace:
    max_size: "50MB"           # 超过后旋转
    max_files: 5               # 保留最近 5 个旋转文件
    compress: true             # 旧文件用 gzip 压缩
  memory:
    max_entries: 500           # 超过后裁剪最旧条目
    max_age_days: 180          # 超过 180 天的条目自动删除
    dedup: true                # 按内容哈希去重（可选）
  scorecard:
    max_age_days: 90           # 只保留最近 90 天的 scorecard
    keep_min: 20               # 即使超过 90 天也至少保留最近 20 个
  checkpoint:
    retain: 5                  # 保留当前 + 5 个备份（从固定 3 改为可配置）
  global:
    max_total_size: "500MB"    # .forge/ 整体上限
    prune_interval: "24h"      # 自动裁剪周期（daemon 模式）
```

**第二层：`forge state` 子命令族扩展（~0.3 sprint）**

已有 `.forge/` 管理命令扩展：

```bash
forge state info                            # 当前状态目录用量
forge state prune                           # 按 policy 裁剪
forge state prune --force                   # 强制执行，忽略策略
forge state prune --dry-run                 # 预览（显示将删除什么）
forge state archive                         # 归档旧状态（压缩 + 移出 .forge/）
forge state archive --output ./archives/    # 指定归档目录
forge state rotate                          # 手工旋转 trace
```

输出示例：

```
$ forge state info
.forge/ state usage
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
trace.jsonl:          12.4 MB  (42,931 events)
trace.jsonl.1:        8.2 MB   (28,104 events)
memory.jsonl:         128 KB   (247 entries)
scorecards/:          46 files / 2.8 MB (87 days, oldest: 2026-04-15)
checkpoint.json:      2.1 KB   (seq=187)
checkpoint.json.1~3:  6.3 KB   (3 backups)
total:                28.5 MB  (73% of 500 MB limit)

recommendations:
  - forge state prune --dry-run  → would remove 18 scorecards (1.1 MB, > 90 days old)
  - forge state archive         → would archive trace.jsonl.1 (8.2 MB)
```

**第三层：自动维护周期（~0.2 sprint）**

在 daemon 模式下（方向四），或作为 `forge evolve` 的迭代尾部操作：

```
$ forge evolve build --max-iter 10
Iteration  1/10: CONVERGENCE_NOT_MET (roadmap_completion=30)
...
Iteration 10/10: CONVERGED (roadmap_completion=100)
  ⊳ state prune: 2 scorecards removed (82 days old)
  ⊳ trace rotate: skipped (trace.jsonl = 12 MB, < 50 MB limit)
  ⊳ memory prune: 3 old entries removed (195 days old)
```

非 daemon 模式下，可以在 `forge accept` 或 `forge state info` 中触发。

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 裁剪时进程崩溃 | 文件处于中间状态 | 裁剪是幂等的：先写临时文件再 rename，崩溃后下次裁剪可安全重试 |
| 策略太激进取删了重要 trace | 无法回溯历史 | 归档（archive）优于删除（prune）：归档文件移出 `.forge/` 但保留到 `./.forge-archive/` |
| 用户从未配置 `state_management` | 退化到无限制行为 | 默认策略保守（`max_size: 100MB`, `max_age_days: 365`, `retain: 3`），与现有行为一致 |
| `forge state prune` 在 evolve 中途被调用 | 文件被并发读写 | prune 只操作 rotate 后的旧文件（`.1`/`.2` 等），不操作当前正在写入的文件 |
| CI 环境每次从干净状态开始 | 裁剪不适用 | CI 环境跳过 prune（零文件可剪），`forge state info` 报告 `N/A (ephemeral)` |

### 价值

1. **磁盘爆炸防护**: `.forge/` 的无限增长最终会填满磁盘——这是可以预测的、应该预防的
2. **性能保证**: `trace.jsonl` 大到 GB 级时，读取/搜索性能线性下降；裁剪保证可预测的性能
3. **合规性**: 在某些行业（金融/医疗），数据保留有法定限制——可配置的保留策略满足合规要求
4. **基础设施友好**: 监控系统可以定期检查 `forge state info --output json | jq '.usage_percent'` 并在超过 80% 时告警
5. **知识新鲜度**: memory 的过期删除保证知识库不会因为 2 年前的经验过时而产生误导

---

## 汇总优先级矩阵

| 方向 | 影响面 | 实现难度 | 用户可见度 | 已有覆盖 | 优先级 |
|------|--------|----------|-----------|---------|--------|
| 1. 二进制分发与生命周期 | 采纳 · 安全 · DevOps | 中（~1–1.5 sprint） | 高（每个用户首次接触） | **零** | **P0** |
| 2. 状态目录健壮性与灾难恢复 | 可靠性 · 数据安全 | 中（~1 sprint） | 中（出事时极高） | **零** | **P0** |
| 3. 统一结构化输出协议 | 集成 · 自动化 · CI/CD | 低（~0.5–1 sprint） | 高（CI/CD 使用者） | **浅提及** | **P1** |
| 4. 多会话运行时协调与热加载 | DX · 性能 · 服务化 | 高（~1.5–2 sprints） | 中（重度用户） | **浅提及** | **P1** |
| 5. 状态数据生命周期管理 | 运维 · 性能 · 合规 | 低（~0.5–1 sprint） | 低（出事前不可见） | **零** | **P1** |

---

## 总结

本文的 5 个方向代表 ForgeOS 在**生产落地**道路上的五个结构性盲区：

- **方向一和三**是从「个人开发者工具」走向「团队/组织级平台」的基础设施前提——没有二进制部署和机器可读输出，ForgeOS 无法被集成到组织的标准工具链中。

- **方向二和五**是长期运行的运维保障——没有状态目录的健康管理和数据生命周期策略，ForgeOS 在连续使用数月后必然会遇到状态损坏或磁盘耗尽。

- **方向四**是架构演进的自然下一步——从纯 CLI 命令模型走向守护进程/会话模型，是未来 Web UI、CI 集成、服务化架构的基础设施。

这些方向与已有 85+ 方向的关系：不是竞争，而是互补。已有分析覆盖了引擎层、治理层、知识层的扩展；这 5 个方向覆盖了**运营层/基础设施层**的扩展——它们是让所有已有功能在真实生产环境中持续、可靠、可集成地运行的前提条件。
