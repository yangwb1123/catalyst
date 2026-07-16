# ForgeOS — 五个隐性的产品/质量缺口（资深架构师 × 产品经理视角）

> **角色**: 资深架构师 + 产品经理  
> **方法**:
> 1. 全局深扫: forge-core（19 Go 包 · 63+ 非测试源文件 · 纯 stdlib 零依赖）+
>    cmd/forge（40+ 子命令 · ~12k LOC）+ harness（39+ 模块 · ~10.5k LOC 执法层）+
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies/ADR/DECISIONS）+
>    `examples/`（url-shortener · go-taskd）+ `pi-batch.py` + `.github/workflows/forge.yml`
> 2. 通读 Sprint 1–31 全部演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（200+ 条目，0 GAP）
> 3. **差异化验证**: 对每个方向在 93+ 份已有分析文档（`docs/requirements/`）中做全文关键词检索 +
>    语义比对，确认该方向的**核心论点从未被作为独立系统性方向展开**。
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、实际影响、边界场景。
> **日期**: 2026-07-10

---

## 全景判断

ForgeOS 经过 31 轮 sprint，几乎所有功能引擎、安全护栏、治理执法、学习闭环都已被深度覆盖。
93+ 份已有分析文档已覆盖的主题（本文不重复）：

| 覆盖域 | 约方向数 |
|--------|---------|
| 引擎补齐（编排/路由/记忆/收敛/信号/并行/回灌/ADR 消费/配置管理） | ~35 |
| 生产可靠性（超时/重试/护栏/进程组/自愈/熔断/输出限流/budget 降档） | ~18 |
| 学习闭环（trace/telemetry/scorecard/自适应收敛/知识反哺） | ~15 |
| 安全纵深（secret-scan/递归/预算/SCA/Sandbox/readonly 强制/深度守卫） | ~15 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/漂移守卫） | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 |
| 结构债（Phase 膨胀/Context Engine/非结构化日志/cmd/forge 包/函数长度） | ~10 |
| 执行语义（原子性/幂等/回滚/因果一致性/undo/workspace side-effect） | ~12 |
| 运营可信度（Run Identity/状态隔离/审计/健康检查/自检/binary 生命周期） | ~10 |
| 二阶伴生（配置爆炸/知识衰减/TOCTOU/无声数据丢失/配置散落） | ~12 |
| 第三地平线（多仓库联邦/事件驱动/Web UI/LiteLLM/Firecracker 沙箱） | ~8 |
| 二进制/发布/输出/会话/数据生命周期 | ~5 |

**以下五个方向落在 93+ 份已有分析的间隙中**。它们不是「缺失的组件」，而是 **「系统已达到 ~95% 功能完备度，但缺少让剩下 5% 成为真正可靠产品的关键桥梁」** —— 每个方向都对应具体的代码级证据，且其核心命题在已有分析中从未被作为独立方向展开。

---

## 方向一 · `.forge/` 运行时状态目录缺乏运行隔离与生命周期管理

**优先级**: 🔴 P1（数据完整性） | **类别**: 运行时基础设施 · 可靠性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 将其运行时状态全部写入 `<root>/.forge/`，包括 checkpoint（恢复点）、trace（遥测日志）、memory（跨 session 知识）、scorecards（学习记分卡）、approval/rejection 标记。**但这些状态没有运行隔离、没有并发锁、没有垃圾回收**。在宣称「24h 无人值守自治运行」的产品愿景下，这是一个静默数据损坏的定时炸弹。

### 代码级证据

**证据 A: 三个核心状态文件共享同一目录，无 run-ID 隔离**

```go
// forge-core/internal/persist/checkpoint.go — 写入 .forge/checkpoint.json
// forge-core/internal/trace/trace.go       — 写入 .forge/trace.jsonl
// forge-core/internal/memory/memory.go     — 写入 .forge/memory.jsonl
// forge-core/cmd/forge/scorecard_wind.go   — 写入 .forge/scorecards.json
// forge-core/cmd/forge/gates.go            — 读/写 .forge/<stage>.approved / .rejected
```

所有这些路径是**硬编码常量**，没有 run-ID/session-ID 前缀。两次 `forge evolve` 并行运行（或同一项目两个终端窗口）的 checkpoint、trace、memory 会被**静默覆盖/交错**。

**证据 B: trace.jsonl 的 seq 跨运行重置（实证 91 行 84 个 seq 冲突）**

```go
// forge-core/internal/trace/trace.go:105-106
func NewTracer(w io.Writer) *Tracer {
    return &Tracer{w: w, Now: time.Now}  // seq 从 0 开始
}
// trace.go:118-119
t.seq++           // 每次 Emit 递增
ev.Seq = t.seq    // 赋值
```

Tracer 的 `seq` 是**进程内变量**。每次 `forge run` 创建一个新 Tracer（打开已有 `trace.jsonl` 做 O_APPEND），seq 都从 0 开始重新计数。实证：

```
$ node -e "const t=require('fs').readFileSync('.forge/trace.jsonl','utf8').split('\n').filter(Boolean); \
  const seqs=t.map(l=>JSON.parse(l).seq); const u=new Set(seqs); \
  console.log('total lines:',t.length,'unique seq:',u.size,'max seq:',Math.max(...seqs),'seq collisions:',t.length-u.size)"
total lines: 91  unique seq: 7  max seq: 7  seq collisions: 84
```

**91 行 trace 只有 7 个不重复的 seq** —— 84 次 seq 冲突。`seq=1` 对应多个完全不同的运行事件，你无法通过 seq 唯一标识或排序任意一条 trace 记录。

**证据 C: memory JSONL 与 checkpoint 同目录，无并发保护**

```go
// forge-core/internal/memory/memory.go — 打开 .forge/memory.jsonl 做 O_APPEND
f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
```

O_APPEND 在同一进程内是原子的，但**两个进程同时 append 同一个文件**会产生交错行。checkpoint 使用 `O_TRUNC` 覆盖写，没有原子 rename 之前的 `O_APPEND` 竞争窗口。且 checkpoint 格式声明了 `_format` 字段（`"forgeos.checkpoint.v1"`），但 `Save` 和 `Load` **从不校验该字段**——一个未来格式的 checkpoint 被静默误解析。

### 实际影响

1. **并发运行损坏**: 两个终端同时 `forge run build --executor dry`，memory 交错了、checkpoint 被覆盖了、trace 的 seq 全乱了。当前没有保护机制。
2. **跨 session 可审计性丧失**: 持续 24h 的运行轨迹存入 `trace.jsonl`，但 seq 在每次 `forge accept` / `forge run` 间重置。`tail -f` 想看最新事件变成了在全文件里找 seq=1。
3. **状态堆积无回收**: 没有 `.forge/` 清理机制。`memory.jsonl` 是 append-only（永不 compact 已过时知识），`trace.jsonl` 随着时间线性增长。24h 无人值守运行时，磁盘可能写满。

### 边界场景 (Edge Cases)

- **同一项目两个 evolve 并行**: 一个跑 `build.yml`，另一个跑 `evolve.yml` → checkpoint 覆盖对方恢复点 → 崩后 resume 跳到错的 workflow。
- **`forge run` 途中 `forge accept`**: acceptance 还读 `.forge/` 状态 → 读到半写的 trace 行 → 静默解析失败。
- **深拷贝脚手架**: `forge-init` 新项目时，`.forge/` 状态文件被不慎复制（git 忽略但文件复制不忽略）→ 新项目继承了上一个运行的全部 memory。

### 推荐解决思路

1. 引入 `run_id`（UUIDv7 或 timestamp+hostname+pid），所有 `.forge/` 文件以 `run_id` 为前缀或子目录
2. trace 文件的 seq 改为读取已有最后 seq 续号（`LOAD max(seq) + 1`）或其他跨运行唯一标识
3. 添加 `.forge/` 的 `forge state gc --keep-runs N` 命令，设定保留最近 N 次运行的状态
4. checkpoint reader 严格校验 `_format` 字段，不匹配则拒绝加载（失败声明的 fail-closed，而非静默误解析）

---

## 方向二 · 遥测基础设施的查询性与可审计性缺口

**优先级**: 🔴 P1（可观测性） | **类别**: 可观测性 · 运维 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 已经构建了扎实的遥测基础设施 —— trace JSONL、scorecard、telemetry 框架 —— 但**写了一堆数据却没人读**。没有查询工具、没有可视化、没有跨运行聚合。这导致 scorecard 回灌只对当前运行有效，而跨运行的模式发现退化给人眼翻 JSONL。

### 代码级证据

**证据 A: trace 事件的 `Detail` 字段混合机器数据与人类散文**

```go
// forge-core/internal/trace/trace.go:62-63
Detail string `json:"detail,omitempty"`  // free-text description
```

trace 核心结构中的 `Detail` 是纯字符串，没有结构化子字段。`forge doctor` 的 anomaly 诊断、convergence 判据、scorecard update 都往 `Detail` 里写机器学习可解析的内容（如 `"roadmap=100% gates_green=false"`），但这些字符串没有标准模式 —— 上层消费者必须做字符串匹配/正则解析，脆弱且版本敏感。

**证据 B: trace 文件 O_APPEND 写入，没有索引、没有时间范围查询**

```go
// forge-core/cmd/forge/evolve.go — trace.Tracer 写入 .forge/trace.jsonl
// 只有 Emit 写入，没有任何 Open/Query/Filter API
```

`trace.Tracer` 只暴露了 `Emit` 和 `Span` 两个写入接口。**零个读取接口**。整个 trace 包没有 `Open`, `Query`, `Iterate`, `Filter`, `Aggregate` 等函数。如果你想知道「过去 12h 哪些 phase 触发了 overload backoff」，需要：
1. 手动 `cat .forge/trace.jsonl`
2. `jq 'select(.kind=="overload_backoff")'`
3. 自己写脚本聚合

**证据 C: scorecards 跨运行被静默覆盖**

```go
// forge-core/cmd/forge/scorecard_wind.go:88 — runScorecardUpdate
// 写入 .forge/scorecards.json (覆盖写)
```

每轮 `forge run/evolve` 自动执行 `scorecard-update.mjs`，它输出 `scorecards.json`（覆盖写）。**不保留历史**。你今天看 scorecard 只能看到**上次运行**的数据，看不到性能是做平还是做差了。

**证据 D: `forge doctor` status 是快照，不是趋势**

```go
// forge-core/internal/doctor/status.go — Status 只返回当前 .forge/ 文件状态
// 无历史对比、无变化率、无异常检测
```

`forge doctor` 可以告诉你当前 trace 文件多大，但**不告诉你它比昨天大了 10 倍**。没有查询差异的基线。

### 实际影响

1. **调试困难**: 用户报告「forge 24h run 性能下降了」，但没有查询工具找到「第一次 green 是什么时候」「overload 频率变化」。可观测性是找问题的入口，当前这个入口不存在。
2. **跨运行归因丢失**: trace 没有 run_id，seq 冲突，scorecard 覆盖写。想对比「上个月和前天的平均 phase 耗时有变化吗」—— 做不到。
3. **诊断退化为手动**: `forge doctor` 能检查状态文件完整性和异常模式，但不能回答任何「趋势」问题。运维人员需要自己写脚本。

### 边界场景 (Edge Cases)

- **scorecard 覆盖写 + 崩溃**: 在 `runScorecardUpdate` 写一半时崩溃 → `scorecards.json` 为空或半写 → 下次 `forge run` 读不到历史
- **多用户同项目**: CI runner 跑 `forge accept` 写入 trace，开发者本地也跑 → trace 交错

### 推荐解决思路

1. trace 包增加 `Reader` / `Query` / `Filter` 接口（按 kind、name、时间范围、status 过滤）
2. 每个 trace event 加入 `run_id` 和 `wall_clock`（人类可读 RFC3339，不只 unix 秒）
3. scorecards 改为 append-only JSONL + 聚合命令 `forge scorecard trend`，支持跨运行对比（p95 latency 变化趋势、cost 变化率）
4. `forge doctor` 增加 `--trend` flag，自动对比上次运行的状态文件大小/事件数差异

---

## 方向三 · 核心解析器的测试成熟度缺口——从「正确性」到「可靠性」

**优先级**: 🔴 P1（产品质量） | **类别**: 测试基础设施 · 质量保障 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 的核心依赖多个手写解析器（YAML、TOML、Go mod、package.json 等），它们处理用户输入的 workflow 定义、项目配置、依赖分析。但**这些解析器的测试以纯单元测试为主，缺乏 fuzz、property-based testing、差分回归测试**。这些解析器的任何 bug 都可能导致静默的配置误读、路由误判、或阶段跳过。

### 代码级证据

**证据 A: 全仓只有 1 个 fuzz test**

```
$ grep -r "func Fuzz\|f.Fuzz" forge-core/ --include="*.go" | head -5
forge-core/internal/routing/routing_test.go:func FuzzTierForScore(f *testing.F) {
forge-core/internal/routing/routing_test.go:	f.Fuzz(func(t *testing.T, score float64, taskType string, risk string, spendRatio float64) {
```

整个 forge-core （19 Go 包）只有 1 个 fuzz test。关键解析器缺失:

| 解析器 | 文件 | 行数 | 风险 | 有 fuzz? |
|--------|------|------|------|----------|
| yaml2json 手写 YAML 解析器 | `internal/yaml2json/` | ~44+ 文件 | **P0** 输入是用户 workflow YAML | ❌ |
| pyproject.toml 手写 TOML 解析器 | `cmd/forge/detect_parsers.go` | 363 行 | **P0** 项目检测路径 | ❌ |
| semver 匹配引擎 | `harness/sca.mjs` | ~300 行 | **P1** SCA 漏洞匹配 | ❌ |
| Go mod 解析器（手写行扫描） | `cmd/forge/detect_parsers.go` | 同文件 | **P1** 项目检测 | ❌ |
| risk diff 启发式扫描 | `internal/risk/risk_diff.go` | 102 行 | **P1** 路由安全判定 | ❌ |
| scorecard JSONL 解析 | `internal/routing/scorecard.go` | ~200 行 | **P1** 学习回路 | ❌ |

**证据 B: yaml2json 已有「block scalar 损坏」类 bug 历史**

Sprint 27 发现并修了一个 **blocking bug**: `consumeBlockScalar` 把 YAML 的 `>` / `|` 指示符拼进解码值，导致**每个 workflow 的 `description:` 字段注入字面量前缀直送 agent prompt**。这个 bug 从 yaml2json 诞生之日起就存在，存活了至少 6 个 sprint，被多位 reviewer 和两次测试都漏过。

原因：差分安全网测试（`TestToJSON_MatchesPythonShim`）存在但**只用 `t.Logf` 不用 `t.Errorf`**——测试本身失效。这是一个**测试基础设施**的失败，不是解析器本身的失败。

**证据 C: detect_parsers.go 是 363 行手写 TOML 解析器**

```go
// forge-core/cmd/forge/detect_parsers.go — parsePyprojectToml 手工扫描 pyproject.toml
// 没有使用 encoding/toml，因为 forge-core 是零外部依赖
// 手工处理: table 行、key=value、数组值、行延续…
```

TOML 规范支持数组 `[dependencies]`、`[[array-of-tables]]`（两个括号）、inline table、多行字符串。手写扫描器不可能正确处理所有这些边缘情况。一个畸形的 `pyproject.toml` 可以让 `forge detect` 输出不正确的 project type → workflow suggestion → 用户选错 workflow。

**证据 D: CI 没有矩阵测试、没有 presubmit 性能回退检查**

```yaml
# .github/workflows/forge.yml
# - 只在一个 Go 版本上跑（1.26）
# - 只在一个 Node 版本上跑（22）
# - 只在一个 Python 版本上跑（3.12）
# - 没有跨版本矩阵
# - 没有 benchmark 回归检测
# - 没有 `-race` 测试超时设置（并行测试可能饿死）
```

### 实际影响

1. **用户配置被静默误读**: yaml2json 的历史证明了 —— 解析器有 bug 时，agent 收到错的 prompt，所有下游决策都基于错误输入，但没有任何人知道。
2. **「零外部依赖」以手写解析器 bug 为代价**: 这是 deliberate trade-off。但要诚实承认手写解析器需要更严格的测试。
3. **CI 不能捕捉性能回退**: 并行编排、memory compaction、trace IO 这些路径没有任何 benchmark gate。一次意外的复杂度增加可以悄悄把 `forge evolve` 的 phase 开销翻倍而不被发现。

### 边界场景 (Edge Cases)

- **不常见 TOML 语法**: `pyproject.toml` 含 inline table `requires = { "python" = ">=3.9" }` 或 `[[tool.ruff.lint]]` 双括号 → 手写扫描器误解析
- **YAML 多文档**: 用户不小心在 workflow 文件里写 `---` 分割 → yaml2json 只读第一个文档，静默丢弃后续
- **Go mod 间接依赖**: `go.mod` 有 `require (` 块 + 间接注释 → 行扫描解析器可能把间接依赖算进 direct 依赖
- **畸形的 OSV 数据库**: semver 匹配引擎处理 `>= 1.0.0, < 2.0.0` 范围表达式（当前只支持 `introduced` / `fixed`） → 静默不匹配

### 推荐解决思路

1. 在 `internal/yaml2json` 添加 fuzz test，随机生成合法 YAML 输入，与 PyYAML 做差分对比（复用已有的 fixture 框架）
2. 在 `cmd/forge/detect_parsers_test.go` 添加 property-based test：随机生成合法的 go.mod/package.json/pyproject.toml → 验证 round-trip（序列化→检测→结果与输入一致）
3. 在 `harness/sca.mjs` 对 semver 匹配添加结构化 fuzz：随机 version × random OSV range → 匹配不 panic、不无限循环
4. CI 添加 benchmark gate：`go test -bench . -benchtime=1x` → 与基线对比，超过阈值则 flag
5. CI 添加跨版本矩阵（Go 1.25/1.26/1.27，Node 20/22，Python 3.11/3.12）

---

## 方向四 · 工作流模板的人机工程学与发现性问题（首次用户体验）

**优先级**: 🟡 P2（产品采纳） | **类别**: 产品体验 · CLI 可用性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 的「中枢旋钮」和「工作流编排」是强大的架构抽象，但对新用户来说是**巨大的认知负担**。用户需要知道：（1）有 5 个 workflow YAML 文件存在于 `.agent/workflows/`；（2）它们各自做什么（discover/design/build/evolve/review）；（3）`mode` 和 `lifecycle` 是什么意思；（4）怎么选正确的 workflow。当前没有任何引导、解释或发现性工具。

### 代码级证据

**证据 A: 没有 `forge workflow list` 或 `forge new` 命令**

```go
// forge-core/cmd/forge/main.go:69-76 — 完整命令清单
//   run / evolve / gate / check / accept / migrate / route
//   detect / validate / status / scorecard / doctor / preflight / approve
// 没有: workflow / new / start / template / init (forge-init 是 scaffold)
```

全仓 grep 无 `workflow list`、`workflow describe`、`new workflow` 子命令。

**证据 B: `forge detect` 输出建议但不持久化**

```go
// forge-core/cmd/forge/detect.go — cmdDetect
// 输出 workflow suggestion + command，但不执行、不修改 project.yml
// 用户必须自己复制建议命令并粘贴
```

`forge detect` 可以输出完美的建议命令「`forge run .agent/workflows/build.yml --mode balanced --lifecycle mvp`」，但用户看到的是一个文本建议，而不是直接被引导执行。

**证据 C: 错误信息不引导用户选正确的 workflow**

```go
// forge-core/cmd/forge/main.go:210-220 — bindRunOpts
// 如果 workflow 文件不存在：返回 "cannot find workflow at <path>"
// 不会说 "发现你是 Go 项目，推荐用 build.yml，或者先跑 forge detect"
```

```go
// forge-core/cmd/forge/evolve.go — cmdEvolve
// 如果 workflow 没有 on_unmet/human_gate：返回 "workflow cannot be evolved"
// 不会说 "试试 forge run build 单次运行"
```

当前几乎所有错误消息都是声明式的（"something is wrong"），不是引导式的（"try this instead"）。

**证据 D: 用户配置散落六个文件**

| 配置 | 文件路径 | 用户需知 |
|------|---------|---------|
| 项目 mode/lifecycle | `.agent/project.yml` | 知道文件位置、YAML 语法 |
| 模式矩阵 | `.agent/policies/modes.yml` | 同上 |
| 路由策略 | `.agent/routing/policy.yml` | 同上 |
| 执法策略 | `harness/policies.yml` | 同上 |
| 评估 schema | `.agent/eval/acceptance.schema.yml` | 同上 |
| 工作流定义 | `.agent/workflows/*.yml` | 5 个文件各自的含义 |

没有 `forge config` 统一入口（这本身是另一个方向，已有分析部分覆盖，不在此重复）。

### 实际影响

1. **首用体验断裂**: 用户安装 ForgeOS，跑 `forge run` 报错「workflow not specified」，用户不知道什么 workflow 可用。这是最大采用障碍。
2. **Hello World 太远**: 运行 ForgeOS 需要理解 5 种 YAML 格式、mode/lifecycle 概念、中枢旋钮。「只是想试试」的用户在第一步就放弃了。
3. **用户选了错的 workflow**: 新手选了 `evolve.yml`（适合持续演化）而不是 `build.yml`（适合单次构建），evolve 循环空转或报「no roadmap items」—— 用户不知道选错了。

### 边界场景 (Edge Cases)

- **用户 `forge run build.yml` 但项目没有 ROADMAP** → build.yml 的 gate phases 跑不过 → 用户困惑「为什么 build 失败」
- **用户 `forge run evolve.yml --mode explorer`** → discover stage 被 skip → gate 空跑 → 用户觉得「什么都没发生」
- **用户从 Claude Code 的 README 直接复制命令** → 运行了一个与项目不匹配的 workflow

### 推荐解决思路

1. `forge start` 或 `forge run` 不加参数时调用 `forge detect` 做 workflow 建议 + 确认执行
2. `forge workflow list` 列出所有可用 workflow + 一句话说明 + 典型使用场景
3. 错误消息转换为引导式：如果 `build.yml` 不满足 stop_condition，错误消息增加「试试 `forge evolve` 用于持续演化，或者调整 ROADMAP.md」
4. 在 `forge-init` 的 scaffold starter 中增加一个交互式教程路径：`forge start --tutorial`

---

## 方向五 · `pi-batch.py` 分析工具的产品质量缺口

**优先级**: 🟡 P2（工具链质量） | **类别**: 工具链 · 分析基础设施 | **预估**: ~0.5 sprint | **杠杆**: ⭐⭐⭐

### 为什么需要

`pi-batch.py` 是当前用于生成本代码库 93+ 份需求分析文档的**核心工具**。它是一个 499 行的 Python 脚本，承担了「批量向 AI agent 发送 prompt → 收集输出 → 并行串行编排」的关键角色。**但这个工具没有测试、没有 CI 集成、有已知 bug**。分析工具本身的质量决定了分析产物的可信度。

### 代码级证据

**证据 A: 零测试覆盖**

```bash
$ grep -r "pi-batch\|batch" forge-core/ harness/ —include="*.go" —include="*.mjs" | head -3
# 无输出 — forge-core 和 harness 都不引用 pi-batch.py
$ ls test_pi-batch* 2>/dev/null  # 无输出 — 没有测试文件
$ grep -n "def test\|unittest\|pytest" pi-batch.py 2>/dev/null  # 无输出
```

499 行，21 个函数，2 个 dataclass，**零行测试代码**。没有任何单元测试、集成测试、doctest。没有 `__main__` 入口的防御性检查（直接 `if __name__ == "__main__": main()`）。

**证据 B: 已知的超时 bug（CURRENT_SPRINT.md Sprint 27 记录）**

> `pi-batch.py`（独立批处理脚本，零测试覆盖）超时机制对 stdout/stderr 两个 reader 线程
> 分别给满额 timeout 预算，实际杀进程延迟可达 ~2× 配置值（命中脚本自身目标场景：详细
> stdout+安静 stderr 的流式 CLI）；`FileNotFoundError` 一律误报「pi not found in PATH」，
> 不区分二进制缺失与 `cwd` 不存在。

这是一个 **已记录、未修复** 的 bug。超时保护的实际延迟是配置值的 2 倍——当你的 subprocess 是 `claude` 这种长运行命令行时，2× 超时意味着用户等待加倍。而 `FileNotFoundError` 的诊断误导使任何被 `cwd` 问题导致的异常被统一归因到「PATH 找不到 pi」。

**证据 C: 依赖 `PyYAML` 为可选（但核心功能依赖它）**

```python
try:
    import yaml
except ImportError:
    yaml = None
```

如果 YAML task 文件存在但 `PyYAML` 未安装，脚本成功导入但随后的 `yaml.safe_load()` 会 panic（`AttributeError: 'NoneType' object has no attribute 'safe_load'`），不是友好的错误消息。

**证据 D: 没有与 forge-core 的集成**

```bash
$ grep -r "pi-batch\|batch" .github/ 2>/dev/null
# 无输出 — CI 不涉及 pi-batch.py
```

`pi-batch.py` 是**完全脱离 ForgeOS 治理体系的工具**。它不使用 forge-core 的编排引擎、不被 `forge accept` 验证、不受 `gate.mjs` 的约束检查。它本质上是一个「寄生」脚本，但产出大量影响项目路标的分析文档。

### 实际影响

1. **超过 93 份分析文档由一个有已知 bug 的工具生成**: 文档本身的可靠性被工具质量打了折扣。如果 `pi-batch.py` 的并行执行导致 prompt 交错，分析输出是混合的。
2. **修复需手工**: 超时 bug 的修复没有自动化回归——改代码的人无法验证「修复没坏别的」。
3. **治理盲区**: ForgeOS 宣称「治理 OS」、「所有工具被同标准约束」，但 `pi-batch.py` 是治理法则的例外。

### 边界场景 (Edge Cases)

- **长提示词 10k+ tokens**: stdout/stderr 读到 CPU 绑定，timer 线程调度的精度抖动导致 ~2× 超时的实际值进一步恶化
- **YAML task 文件不含 task 列表**: 空 `tasks: []` → 脚本成功退出但什么都没做，exit 0 给用户假阳性
- **模型不可用时**: 循环重试逻辑没有用户可见进度 → 用户在 `--mode parallel --workers 8` 时以为系统挂起

### 推荐解决思路

1. 增加 `test_pi-batch.py`：覆盖 `Task.to_cmd()`、`parse_args()`、`read_tasks()` 等纯函数路径
2. 修复已知超时 bug：两个 reader 线程合并为一个 timeout 预算，或改用 `asyncio` 统一 IO 与超时
3. 将 `PyYAML` 从可选升级为硬依赖（或提供更优雅的降级——如只支持 `-p` 模式）
4. 将 `pi-batch.py` 纳入 `forge accept` 的治理范围（至少加 `gate.mjs` 文件体积检查）
5. 增加 `--quiet` / `--progress` 模式，长运行时给用户可见进度（当前只有 logging 到 stderr）

---

## 优先级收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|------|--------|------|-----------|
| 一 · `.forge/` 运行隔离 | 🔴 P1 | 数据完整性 | 并行运行会静默损坏 state；修复成本低（加 run_id + 目录前缀） |
| 二 · 遥测查询性 | 🔴 P1 | 可观测性 | 写了大量 trace 但无人能读；增加 Reader API 和趋势分析即可解锁跨运行审计 |
| 三 · 解析器测试成熟度 | 🔴 P1 | 产品质量 | 手动 YAML/TOML 解析器已有前科（block scalar bug 存活 6 sprint）；fuzz 是最高杠杆加固 |
| 四 · 工作流模板人机工程 | 🟡 P2 | 产品采纳 | 新用户首个命令就迷茫；`forge start` 引导式入口可能翻倍初次试用率 |
| 五 · pi-batch.py 质量 | 🟡 P2 | 工具链健康 | 核心分析工具有已知 bug 零测试；修复超时 bug 是具体可量化的改善点 |

### 建议执行次序

**[第一优先级] 方向一 + 方向三**（可并行，无依赖）：
- 方向一是**最低成本最高回报**：加 run_id 前缀、checkpoint 格式版本校验、`forge state gc`。一周内可以交付。
- 方向三是**测试基础设施投资**：yaml2json fuzz、detect_parsers property-based test、CI 矩阵。两周内可交付。两条并行跑道都触及产品质量的地基。

**[第二优先级] 方向二 + 方向四**（可并行）：
- 方向二的 trace Reader + `forge doctor --trend` 是一到两周的工作，完成度已达到标准产品级可观测性。
- 方向四的 `forge start` 和 `forge workflow list` 是两个月内的采用推动力。

**[第三优先级] 方向五**：0.5 sprint 修复已知 bug 和加测试，单独交付，不和其他方向抢跑道。

---

## 附录：已有 93+ 分析文档去重依据

本文件每个方向做差异化验证的方法：

| 方向 | 检索词 | 检索范围 | 已有覆盖判断 |
|------|--------|---------|-------------|
| 一 | `.forge` 运行隔离 / run_id / 状态目录 / 并发写入 | 全部 93+ 文档 | 零匹配 |
| 一 | trace seq 跨运行冲突 / seq 重置 | 全部 93+ 文档 | 零匹配 |
| 二 | trace 查询 / Reader / Query / 遥测工具 | 全部 93+ 文档 | `genuinely-uncovered-five-deep-runtime-gaps.md` 方向三提到「trace 数据生命周期」但聚焦于其作为遥测源的消费端，不是缺失的查询接口 |
| 二 | scorecard 趋势 / 跨运行对比 | 全部 93+ 文档 | `expansion-five-systemic-learning-loop-gaps.md` 方向一讨论了 scorecard 回灌但未提及历史保留 |
| 三 | fuzz / property-based / quickcheck / 随机测试 | 全部 93+ 文档 | `expansion-production-readiness.md` 方向三脚注提及 fuzz 但未展开为独立方向；全仓仅有 1 个 fuzz test 的事实未被任何文档作为独立缺口分析 |
| 三 | 解析器测试 / yaml2json 测试 / detect_parsers 测试 | 全部 93+ 文档 | 零匹配 |
| 四 | forge start / workflow list / 新用户体验 / 引导式入口 | 全部 93+ 文档 | 零匹配 |
| 五 | pi-batch / 批处理工具质量 | 全部 93+ 文档 | 零匹配 |

以上检索在 93+ 份 `docs/requirements/*.md` + `docs/analysis/*.md` +
`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 根目录分析文档中执行，
逐方向确认核心论点未被覆盖。
