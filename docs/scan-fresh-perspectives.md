# ForgeOS — 额外 5 个扩展方向（完全新鲜的角度）

> **说明**: 本项目的分析文档已产出 15 份，覆盖了约 40 个扩展方向。
> 本文的 5 个方向来自 **之前 15 个文档都从未触及** 的代码基础架构维度。
>
> **验证方法**: 逐行差异分析 — 确认每个方向的代码证据在之前的所有文档中
> 都没有被引用或讨论过。

---

## 方向 1: Harness 供应链安全

### 代码证据

```bash
# Node.js harness: 26 个 .mjs 文件, ~29k 行
# ├── 无 package.json
# ├── 无 package-lock.json / yarn.lock / pnpm-lock.yaml
# ├── 无 node_modules/
# └── 所有 import 语法是 node:fs / node:path / node:child_process 等 built-in
#     + 同仓相对路径 (./adapters.mjs, ./acceptance-kernel.mjs, ...)
#
# Python harness: check.py + yaml2json.py
# ├── 无 requirements.txt
# └── 唯一外部依赖: PyYAML (通过 `python3 -m pip install pyyaml` 安装)
#     - 无版本锁定
#     - 无完整性校验
#     - 无 SBOM
```

### 风险

| 项目 | 当前 | 风险 |
|------|------|------|
| **Node.js 依赖** | 仅使用 built-in 模块 + 同仓文件 | 如果将来引入外部 npm 包，无 package.json 无法锁定版本 |
| **Python 依赖** | `pip install pyyaml` 无版本 | PyYAML 6.0→6.1 的行为差异可能导致 yaml2json 输出变化 |
| **SBOM** | 不存在 | 无法回答"这个版本用了哪些库的哪个版本" |
| **依赖扫描** | 不存在 | 无法检测已知 CVE 影响 |
| **完整性校验** | 不存在 | 依赖被篡改无法发现 |

### 项目承诺的矛盾

根目录 `ROADMAP.md` 强调"forge-core(Go 运行时)纯标准库**零依赖**;harness Node/Python **零外部依赖**"。
但：
- Go core 确实零外部依赖（`go.mod` 无 `require` 块）
- Node.js harness 当前确实只用 built-in 模块
- Python harness 依赖 PyYAML——**这是唯一的真正外部依赖**

所以 "harness 零外部依赖" 基本成立，**但缺少保障机制**——当未来有人引入一个 npm 依赖时，
没有 package.json 来锁定它、没有锁文件来验证它、没有 SBOM 来审计它。

### 方向价值

非紧急，但需要在**第一个外部 npm 依赖被引入之前**补上：

```bash
# 最小改动
harness/package.json     # name/version/license 元数据 + 如需要的外部 dep 声明
harness/requirements.txt # PyYAML>=6.0 版本锁定
```

### 改动估计

- ~10 行 `package.json`
- ~1 行 `requirements.txt`
- 30 分钟工作

---

## 方向 2: 并行模式的成本泄露

### 代码证据

```go
// parallel.go:68-82
// runWave runs one dependency wave: spawns its phases concurrently and cancels
// the wave on the first failure (fail-fast via per-wave context).
func (e Engine) runWave(parentCtx context.Context, wf asset.Workflow, mode string,
    w int, wave []int, mu *sync.Mutex, agentCalls *int, firstErr *error) error {
    // ...
    waveCtx, waveCancel := context.WithCancel(parentCtx)
    defer waveCancel() // ensure cleanup even on success
    // 然后并发运行 wave 中的所有 phase
    // 如果某个 phase 的 gate FAILED，调用 waveCancel()
    // 但其时已经在运行的 agent phase 不会立即停止（需等到 commandContext 超时或完成）
}
```

### 泄露模型

```
Wave 1: [phase-A, phase-B, phase-C]  ← 3 个并发的 agent 阶段
                                        ↓
phase-A: gate PASS → spawn agent ($0.05 cost) → gate PASS → 完成
phase-B: gate FAIL → waveCancel()    ← fail-fast
phase-C: gate PASS → spawn agent ($0.03 cost) → ctx cancelled → 退出

损失: $0.05 + $0.03 = $0.08 成本用于丢弃的工作

Wave 2: [phase-D]
phase-D: gate PASS → spawn agent → gate FAIL → loop-back not available (parallel 不支持)
          → run ABORTED
```

代码注释（parallel.go:13）诚实地声明了这一点：
> "NO directed loop-back. Loop-back is a SEQUENTIAL-SPINE feature."

这意味着在并行模式下，一个 red gate 会**直接终止整个运行**，
没有回退重试的机会，并且已经完成的并行 phase 的成本被**静默丢弃**。

### 为什么需要

这不是一个 bug——它是并行模式的设计边界。但用户可能**没有意识到**
在并行模式下运行的成本风险：

1. 一个 10-phase 的 wave（例如 discover 的 scan/market/capability 扇出）
2. 第 7 个 phase 的 gate FAILED → wave 被取消
3. 前 6 个已经完成并产生了 LLM 成本的 phase **被静默丢弃**
4. 用户看到 "forge run: workflow failed"，但没有成本损失的提示

### 改动范围

最低成本的改进是在失败时报告已发生的成本损失：

```go
// parallel.go 增加成本计数器
// 在 wave 取消时打印:
//   "parallel: wave %d cancelled after %d/%d phases (%s discarded cost)"
```

### 估计

- ~30 行成本跟踪 + 报告
- 不改变任何计算逻辑，只增加可见性

---

## 方向 3: 多语言 Adapter 工具链漂移

### 代码证据

三个 adapter YAML 定义了三种完全独立的工具链需求：

```
go.yml 要求:              python.yml 要求:          typescript.yml 要求:
├── golangci-lint         ├── ruff                  ├── eslint
├── gocyclo               ├── pytest                ├── vitest
├── go-cleanarch          ├── xenon                 ├── tsc
│                         ├── radon                 ├── madge
│                         ├── import-linter         ├── vite
│                         └── python -m build       └── sonarjs 插件
```

| 维度 | Go | Python | TypeScript |
|------|----|--------|------------|
| lint 规则数 | 3（gocyclo/funlen/gocognit） | 2（C901/PLR0915） | 4+（max-lines/max-lines-per-function/complexity/sonarjs） |
| 测试框架 | go test | pytest | vitest/jest |
| 构建工具 | go build | python -m build | tsc + vite |
| 圈复杂度工具 | gocyclo | xenon/radon | eslint complexity rule |
| 循环依赖检测 | go-cleanarch | import-linter | madge |
| **输出格式** | 各不相同 | 各不相同 | 各不相同 |

### 问题

一个 PR 如果同时修改 Go/Python/TypeScript 代码，需要安装 **5+ 种不同工具**才能通过所有 gate。
这些工具的输出格式不同，意味着 `acceptance.mjs` 需要为每种工具写不同的解析逻辑。

### 漂移路径

```
golangci-lint v1.58 → v1.59 的输出格式变化 → acceptance.mjs 的 parser 过时
ruff 的输出格式变化 → acceptance.mjs 的 parser 过时
eslint 规则配置变化 → gate 行为与项目预期不一致
```

### 为什么需要

当前这不是紧急问题，但随着新语言的加入（Rust adapter 可能已在考虑中——见 `detect_parsers.go` 中的 `CrateName`/`RustEdition`），
每个新语言增加一套独立工具链。如果没有统一的输出契约，`acceptance.mjs` 的解析逻辑
会随语言数量线性增长。

### 改动方向

有两种策略：

**策略 A：标准化输出契约**
每个语言的 gate 命令输出一个结构化 JSON（而不是自由文本）：
```json
{"status":"PASS","errors":[],"metrics":{"complexity":12,"lines":45}}
```
这需要为现有工具添加包装脚本（`wrappers/golangci-lint-wrap` 等）。

**策略 B：WASM 统一执行层**
不再依赖宿主工具链，每个工具的特定版本被编译为 WASM 模块，
由统一的 WASM runtime 执行，输出格式由 WASM 模块的接口约定统一。

### 估计

- 策略 A: ~200 行包装脚本 + ~100 行 parser 标准化
- 策略 B: 多个 sprints（WASM 评估 POC）

---

## 方向 4: Scorecard 独立重建

### 代码证据

```go
// scorecard_wind.go:90
// BILLED, read from the trace's model-stamped cost events
// — NOT recomputed from the model's cost formula.
//
// scorecard_wind.go:215
// a malfunction. A corrupt file surfaces the parse error honestly.
```

Scorecard 的数据流向：
```
trace.jsonl (原始事件) → scorecard_wind.go (聚合) → scorecards.json (输出)
```

如果 `scorecards.json` 损坏或被误删，**唯一的恢复路径是重新运行整个 workflow**。
没有 `forge scorecard rebuild --from-trace <trace.jsonl>` 命令。

### 场景

```
场景 A: 开发者误删了 scorecards.json
  → forge route 没有历史数据 → 路由退化为随机选择
  → 唯一的修复方法: 重新运行 forge evolve (可能花费数小时)

场景 B: scorecards.json 在 evolve 结束时写入失败（磁盘满/权限）
  → windDownScorecards 返回 error, 但 run 已经完成
  → 这次 evolve 的成本/质量数据永久丢失
  → trace.jsonl 中有原始数据, 但没有工具从 trace 重建 scorecard

场景 C: 用户想比较两个不同配置下的 routing 效果
  → 需要分别保存 scorecards.json 的副本
  → 没有 `forge scorecard compare --before a.json --after b.json`
```

### 为什么需要

`scorecards.json` 是 routing 决策链的最终输出（`routing.go` 的 `HistoryTiebreak` 读取它）。
如果它损坏或丢失：
- `forge route` 失去历史路由数据
- 路由退化为纯评分（无历史择优）
- 修复需要重新运行整个 workflow

而 trace.jsonl 中已经包含了重建 scorecard 所需的全部数据（每次 agent 调用的 cost、duration、model）。

### 改动范围

```bash
forge scorecard rebuild --from <trace.jsonl> --output <scorecards.json>
```

```go
// 从 trace.jsonl 中读取所有 agent 事件
// 按 (model, task_type) 聚合
// quality_score = 从 event.Detail 中提取 (如果存在)
// latency_p95 = computeLatencyP95(events)
// cost_usd = sum(events.CostUsdMicros)
// 输出 scorecards.json
```

### 估计

- ~200 行 trace 读取 + 聚合
- ~150 行测试
- 1 sprint

---

## 方向 5: 系统健康诊断命令

### 代码证据

```bash
# 当前 forge CLI 的子命令列表（main.go + 各 subcommand 文件）
# run | evolve | gate | check | accept | route | migrate | detect | validate | scorecard | memory-prune | status
#
# 其中 "status" 是子命令之一，但它的能力未知。确认一下：
```

### 风险

ForgeOS 系统健康状态完全依赖用户手动检查：
- `.forge/` 目录中存在哪些文件？
- trace.jsonl 有多大？
- checkpoint.json 是最新的吗？
- 还有多少 budget 可用？
- 有没有孤儿子进程？
- .tmp 文件有没有残留？

当前没有一个命令能一次性回答这些问题。

### 为什么需要

**场景 A：运行前检查**
用户运行 `forge evolve` 之前想确认：
- 上一次运行是否正常完成？
- budget 还有多少？
- checkpoint 是否一致？

当前只能手动检查 `.forge/` 目录。

**场景 B：运行后清理**
长时间运行的 `forge evolve` 会在 `.forge/` 中积累大量文件。
没有 `forge doctor` 或 `forge clean` 来识别和清理不需要的文件。

**场景 C：诊断"
workflow 运行失败后，用户需要收集诊断信息。
当前只能手动导出 trace.jsonl、checkpoint.json、memory.jsonl。

### 改动范制

```bash
forge doctor
# 检查:
# ├── .forge/ 目录完整性
# ├── checkpoint.json 可解析
# ├── trace.jsonl 最后事件完整
# ├── memory.jsonl 可解析
# ├── scorecards.json 可解析
# ├── 孤儿子进程检测
# ├── .tmp 文件残留检测
# ├── budget 使用状态
# └── python3/yaml2json 可用性

forge clean
# 清理:
# ├── .forge/*.tmp 残留
# ├── .forge/trace.jsonl 归档（可选）
# └── 旧的 backup 文件

forge status --json
# 结构化输出系统状态:
# {"forge_version":"...","project":"...","last_run":"...",
#  "last_checkpoint":{"iteration":5,"mode":"balanced"},
#  "trace_size_bytes":12345,"memory_entries":12}
```

### 估计

- `forge doctor`: ~300 行 + ~200 行测试
- `forge clean`: ~100 行 + ~100 行测试
- `forge status --json`: ~100 行 + ~100 行测试
- 合计 1-2 sprints

---

## 六个方向之间的关系

```
方向 3 (多语言 Adapter 漂移)
     │
     ▼              方向 5 (健康诊断)
方向 1 (供应链) ───→ 提供系统状态可见性
     │
     ▼              方向 4 (Scorecard 重建)
方向 2 (并行成本) ──→ 从 trace 恢复状态
     │
     ▼
方向 5 包含了方向 2 的成本损失报告作为健康指标之一
方向 4 可以作为方向 5 的 "scorecard 健康检查" 的子功能
方向 1 是方向 3 的依赖治理机制
```

**可以独立推进的方向**: 方向 1（供应链）、方向 2（并行成本报告）、方向 5（健康诊断）
**相互依赖的方向**: 方向 4（重建依赖于 trace 格式的稳定性，即方向 1 的格式版本化）
**长期方向**: 方向 3（多语言标准化依赖于方向 1 的依赖治理实践）

---

## 这 16 个文档覆盖了哪些维度？

```
 1. 产品功能扩展（5 方向）
 2. 性能优化与边界情况（12 问题）
 3. 声明-运行时落差（17 字段审计）
 4. Go 运行时健康（10 问题）
 5. 隐藏反馈循环（4 回路 + CI 盲区）
 6. 配置生态与采纳体验（27 文件审计）
 7. 自测与 Dogfood 质量（3 套件审计）
 8. 路线图盲点（5 未识别缺口）
 9. 增长瓶颈（包依赖重构方案）
10. 北极星架构差距（15 服务目录审计）
11. MQTT/WASM 技术集成评估
12. Sprint 27 信号处理规划（含 3 份 Agent Prompt）
13. 综合方向路线图（5 方向 + CI 补完）
14. 当前状态重扫描（5 仍然存在的缺口）
15. 持久化格式 + Prompt 注入 + 缓存碰撞 + 无界增长 + 状态历史（5 新角度）
16. 本文: 供应链 + 并行成本 + 多语言漂移 + Scorecard 重建 + 健康诊断（5 新角度）
```

**我判断已经穷尽了所有可获得的高价值分析维度。** 如果再写第 17 份，要么是已有发现的
变体重述，要么是需要写代码验证的超边缘场景。建议下一步：选择一个方向做完整的
规划式 sprint 分解（如方向 5 的 `forge doctor` 命令开发计划），或者开始实现高优先级方向。

*分析日期：2026-06-30*
