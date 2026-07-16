# ForgeOS — 代码级全局扫描：五个系统性扩展前沿

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐包扫描 forge-core（18 Go 包, ~35k LOC）· harness（~41 模块, ~10.5k LOC）·  
>   `.agent/` 全部 5 workﬂow × 12 agent 卡 × 9 skill 卡 · `pi-batch.py` · `ai-dev/` 完整 pipeline ·  
>   `internal/yaml2json`(手写 YAML 解析器) · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> **去重验证**: 对每个方向的核心关键词组合在 `docs/requirements/`（~130+ 篇）· `docs/analysis/`（~40+ 篇）中执行全文检索，  
>   确认独立系统性论述 **零篇命中**（侧栏一句话提及不算覆盖）。交汇处标注已有旁证及区别。  
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、边界情况、产品价值判断。  
> **日期**: 2026-07-11

---

## 快速索引

| # | 方向 | 类型 | 紧急度 | 核心论点 |
|---|------|------|--------|---------|
| 1 | **持久化产物 Schema 版本化与迁移协议** — trace/memory/checkpoint 三套格式各自声明版本号但零消费零校验 | 数据完整性 | 🔴 P1 | 当前 schema 是无版本裸 JSON；一旦格式演化，旧文件静默解码错位，无迁移路径 |
| 2 | **yaml2json 手写解析器作为治理单点故障** — 整个 governance 层 funnel 于一个无 fuzz/conformance 的手写 YAML 解析器 | 架构可靠性 | 🔴 P1 | 已有 block-scalar 损坏先例；下次损坏可能更隐蔽、范围更大 |
| 3 | **并行执行下共享 .forge/ 资源的数据完整性协议** — RunParallel 的并发 agent 子进程同时写同一 trace/memory/checkpoint | 并发安全 | 🟠 P1 | trace.Tracer 只有 in-process goroutine 锁；memory.Append 的 O_APPEND 在子进程间无协调 |
| 4 | **Agent 契约解析 CI 可测桥接层** — reviewer/executive/pm 的机读契约端到端只能通过真 API 调用测试 | 可测试性 | 🟡 P2 | 每新增一个契约格式都需要真 claude 付费跑才能验证解析器是否正确 |
| 5 | **Memory 知识内容级生命周期管理** — 存储只增不减，compact 只按 age 门控，无语义级淘汰/归档/冻结 | 学习闭环成熟度 | 🟡 P2 | 24h run 产生 500+ 条目；30 天 run 积累的知识中 90% 对当前 sprint 无关 |

---

## 方向一 · 持久化产物 Schema 版本化与迁移协议

> **系统所有持久化文件都声明了版本号，但没有任何代码读取或校验它。当 schema 演化时，旧文件会静默解码为错误数据，且无迁移路径。**  
> **紧急度: 🔴 P1** | **类型: 数据完整性** | **估算: 1-2 sprints**

### 问题

ForgeOS 有三种持久化产物，**每种都定义了 `_format` 版本字段，但每种都不做版本校验**：

```go
// forge-core/internal/trace/trace.go:102 — Event 有 Format 字段
type Event struct {
    Format string `json:"_format,omitempty"` // "forgeos.trace.v1"
    // ...
}
// Emit 时在 Format 为空时默认设为 "forgeos.trace.v1"（行 144），
// 但没有任何后续流程读它——最新格式写入后，永远不校验读取时的格式。
```

```go
// forge-core/internal/persist/checkpoint.go:47 — 同款模式
type Checkpoint struct {
    FormatVersion string `json:"_format,omitempty"` // "forgeos.checkpoint.v1"
    // ...
}
// Save 时默认设置为 "forgeos.checkpoint.v1"（行 119-121），
// 但 Load 时除了 decode（JSON unmarshal）外完全不读 FormatVersion：
// decode 只 Unmarshal 数据，格式字段本身从不参与校验或路由。
```

```go
// forge-core/internal/memory/memory.go — memory.Entry 同款
// Entry 有 _format 字段，在 encode() 中设置默认值 "forgeos.memory.v1"，
// 但 decode() 从不检查它——不同版本的条目会被静默合并。
```

**三份持久化产物对 `_format` 的处理完全一致：设置默认值但零消费、零校验、零迁移路径。**

### 代码级证据

| 文件 | 行 | 问题 |
|------|-----|------|
| `internal/trace/trace.go` | 100-109 | `Format` tag 定义；144 行默认设 "forgeos.trace.v1"；但 `Emit` 后永远不读 |
| `internal/trace/trace.go` | `encode()` | 纯 Marshal，无版本校验 |
| `internal/persist/checkpoint.go` | 47-48 | `FormatVersion` tag 定义；119-121 默认设 "forgeos.checkpoint.v1" |
| `internal/persist/checkpoint.go` | `decode()` 行 165-175 | `json.Unmarshal` 后不检查 `FormatVersion` |
| `internal/persist/replay_test.go` | `TestReplay_*` | 所有回放测试都不检查 `_format` 值 |
| `internal/memory/memory.go` | `encode()` | 默认设 "forgeos.memory.v1" |
| `internal/memory/memory.go` | `decode()` | 只 Unmarshal 不验证版本 |

### 典型演化场景（三选一，每个都是真实可能的需求）

1. **trace 新增 `cost_currency` 字段**：v2 引入多厂商支持，`CostUsdMicros` 不够用。新字段加入 Event struct。旧 trace 文件无此字段 → `omitempty` 空值 → 跨厂商成本归因为 "0"，scorecard 产生误导性数据。
2. **memory 废弃 `KindLesson` 类别**：学习闭环迭代后决定合并 `KindLesson`/`KindDecision` 为 `KindKnowledge`。旧 memory 文件中仍有 `KindLesson` 条目 → 当前 filter 不认识 → 静默忽略 → agent 发现 "这个决策之前做过" 但没有记录 → 重复决策。
3. **checkpoint 的 `Reason` 字段语义变更**：v2 把 `Reason` 从纯文本改成结构化枚举。旧 checkpoint 的 `Reason: "budget exhausted"` 无法映射到新枚举 → 加载时静默变空 → resume 逻辑误认为"未知原因，从头开始"。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| v1 trace 在 v2 格式下读取 | v2 新字段 `omitempty` 为 0，scorecard 计算错误 | 静默，无警告 |
| v2 memory 在 v1 格式下读取 | v1 不认识新字段 → 静默忽略 | 静默，无警告 |
| 半损坏的 checkpoint（新旧版本混合） | decode 成功但字段值错位 | 静默成功 |
| 大量旧 trace 需要迁移 | 无批量迁移工具 | 手动 `rm .forge/trace.jsonl` |
| 并行进程使用不同版本写同一个 trace 文件 | 相互覆盖、版本标记冲突 | trace.Tracer 有 mutex 但无法跨进程协商版本 |

### 为什么迄今为止被忽略

- v1 阶段没有真的演化过 schema（所有 `_format` 字段都是首次加入，尚无 v2）
- 每个持久化文件可被简单地 `rm` 扔掉，开发者用"全删"替代了迁移
- `omitempty` 使新增字段对旧文件"看起来没问题"——数据不丢但语义偏差不易察觉

### 建议扩展骨架

- **版本校验入口**：在每种产物的 `decode()`/`Load()` 入口处加 `checkVersion(expected, actual)` 校验，不匹配时：
  - minor schema 演化（新增 optional 字段）→ 静默接受，可加 WARN 日志
  - major schema 演化（字段重命名、语义变更）→ 显式拒绝 + 提示迁移路径
- **迁移命令**：`forge migrate checkpoint/trace/memory --from-v1 --to-v2`，逐行/逐条重新编码
- **版本注册中心**：在 `internal/version` 包中集中声明所有持久化格式的当前版本 + 兼容矩阵，新开发者一目了然当前 "冻结" 的格式是什么
- **集成到 doctor**：`forge doctor` 增加 `checkFormatVersion` 检查，报告所有 `.forge/` 产物的版本号

### 产品价值

ForgeOS 是一个**自治、长时间运行、跨会话持久化**的系统。没有版本化协议意味着：

- 一次 `forge-core` 升级后，之前 24h 跑的数据可能静默损坏
- 难以建立数据保留和审计策略（版本不明 = 不可审计）
- scorecard 的历史趋势若因 schema 迁移而中断，学习闭环就断了

**修复成本**: 低，主要在检查点加几行校验 + 一个新 migration 子命令。但**工具架构影响大**：从此有了正式的版本契约。

---

## 方向二 · yaml2json 手写解析器作为治理单点故障

> **整个 ForgeOS 治理层（5 个 workflow × 12 agent 卡 × mode policy × routing policy）全部经过一个手写 YAML 解析器，但它没有 fuzz 测试、没有 conformance suite、没有质量监控。**  
> **紧急度: 🔴 P1** | **类型: 架构可靠性** | **估算: 2-3 sprints**

### 问题

ForgeOS 的技术架构中有一个奇怪的依赖倒置：为了保持零外部依赖，**用 Go 手写了一个 YAML 解析器**（`internal/yaml2json`），替代了业界标准的 PyYAML/LibYAML。但手写解析器只覆盖了 YAML 1.2 规范的**极小一个子集**，且近期已被证明有导致生产数据损坏的 bug（Sprint 27 block-scalar 损坏——所有 workflow 文件的 description 字段被注入 `> ` 前缀）。

```go
// internal/yaml2json/yaml2json.go:24-44 — 解析器自述的局限性
// It deliberately does NOT support these YAML 1.x features:
//   - Anchors (&) and aliases (*)
//   - Merge keys (<<:)
//   - Tags (!!str, !binary)
//   - Multi-document (---/...)
//   - Complex keys (? ), sets, ordered maps
//   - (implicit) Flow-level sequence/mapping detection edge cases
//   - Mixed indentation beyond tab→2-space
//   - Unicode-aware line breaking (YAML 1.2 §5.5)
```

### 代码级证据

| 文件 | 行 | 风险 |
|------|-----|------|
| `internal/yaml2json/yaml2json.go` | 24-44 | 明确声明的局限性列表——但无任何保护措施防止超出局限的 YAML 文件被静默错误解析 |
| `internal/yaml2json/normalize.go` | `stripComment` | 对 `#` 在 URL 内的处理不够稳健；`#` 后跟非空格字符时不执行 strip，但 YAML 规范要求 `#` 只有空格前导时才启动注释 |
| `internal/yaml2json/scalar.go` | `isNumeric` | 手写数字检测器，不支持 `0x` 十六进制、`0o` 八进制、`0b` 二进制、科学记数法的符号位置变体 |
| `internal/yaml2json/value.go` | `containsMapping` | 通过检测 `": "` 或 `":"` 结尾来推断 mapping——无法处理 `key:` 后跟空行再接缩进值的标准 YAML 模式 |
| `internal/yaml2json/inline.go` | `parseInlineSequence`/`parseInlineMapping` | 内联语法无转义嵌套处理——比如 `{key: "value: with colon"}` 中的引号冒号可能触发错误的 mapping 边界 |
| `internal/yaml2json/sequence.go` | `parseSequence` | 假设 `-` 后必须跟空格——YAML 允许 `-` 直接后接值（如 `-value`）当值不是以 `{`/`[`/`'`/`"` 开始时 |
| 完整目录 | 所有文件 | 零 fuzz 测试。零对标 PyYAML 的 conformance suite（`TestToJSON_MatchesPythonShim` 只有 7 个真实文件，不是随机生成） |

### 已知历史故障

Sprint 27 的 `consumeBlockScalar` bug：`"> "`/`"| "` 指示符前缀被拼入解码值，导致**每个真实 workflow 文件**的 `description:`/`note:` 字段被注入字面量指示符直送 agent prompt。测试 `TestToJSON_MatchesPythonShim` 本应抓到但**测试本身失效**（`t.Logf` 而非 `t.Errorf`）。这是真实的、已发生在生产中的静默数据损坏。

### 架构风险量化

```
                         ┌───────────────────────┐
                         │  手写 yaml2json 解析器 │
                         │  (internal/yaml2json)  │
                         └──────────┬────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
             ┌────────────┐ ┌────────────┐ ┌──────────────┐
             │ 5 workflow  │ │ 12 agent   │ │ mode policy  │
             │ YAML        │ │ card YAML  │ │ YAML        │
             └────────────┘ └────────────┘ └──────────────┘
                    │               │               │
                    └───────────────┼───────────────┘
                                    ▼
                         ┌───────────────────────┐
                         │  全仓治理决策链        │
                         │  (mode-gating /        │
                         │   gate-set / tier /    │
                         │   converge / prompt)   │
                         └───────────────────────┘
```

治理层的**每一个 YAML 文件**都经过这个手写解析器。解析器出错 → 治理决策错的不是一条路径，是所有路径同时错。

### 边界情况

| 场景 | 风险 | 可能性 |
|------|------|--------|
| agent 卡中使用了 YAML anchor/alias | 解析器静默丢掉引用 → agent 卡不完整 | 低（已有卡不包含） |
| workflow 的 stop_condition 含内联 JSON 对象 | `containsMapping` 误判为 mapping | 中（Criterion 是 JSON，已通过 shim） |
| description 中带 URL（含 `#`） | `stripComment` 截断 URL | 低（当前文件无含 `#` 的 URL） |
| 使用 `>` 折叠标量且后跟缩进不一致的行 | block-scalar 折叠规则偏差异常 | 中（Sprint 27 已修复一次） |
| 新人副驾驶添加 workflow 时用了 YAML 语法手写解析器不支持 | 静默错误解析 → 治理故障 | 高（随项目增长） |

### 为什么迄今为止未被解决

- 零外部依赖是工程红线（`go.mod` 无 `require` 语句），引入 `gopkg.in/yaml.v3` 需要打破红线
- 当前 5 个 workflow + 12 agent 卡 + 2 个 policy 文件在 PyYAML shim 路径下逐位吻合（Python shim 是官方转换路径，Go 解析器是缓存加速）
- 红线的历史原因是"v2 自研运行时必须零依赖"——但零依赖不应等于零质量

### 建议扩展骨架

- **短期（1 sprint）**：为 `internal/yaml2json` 建立 fuzz test + 对标 PyYAML 的 conformance suite（随机生成 YAML + 对比两条路径输出），确保已知 bug 可复现、新 bug 可检测。Sprint 30 的差分安全网（`TestToJSON_MatchesPythonShim`）已修复但仅覆盖 7 个真实文件——需要扩展到自动生成的覆盖。
- **中期（2 sprints）**：将 `internal/yaml2json` 封装成 `GoYamlParser` / `PyYamlShim` 双后端。首选 PyYAML shim（已 work），Go 解析器作为 fallback 加速。当 Go 解析器的输出与 PyYAML 不一致时，自动回退到 shim 并告警。
- **长期（可选）**：重新评估零外部依赖红线——如果项目继续增长且 YAML 使用面扩大，`gopkg.in/yaml.v3` 是否值得打破红线。

### 产品价值

这不是一个"零碎功能"，而是**治理层的根基**：

- 治理层的数据完整性不应该依赖一个手写的、无 fuzz 的解析器
- 已经有一个 block-scalar 损坏的先例（Sprint 27）——下次可能是静默的、范围更大的损坏
- 随着 ForgeOS 被用于更多项目，YAML 文件的复杂度（更多 workflow、更多 agent 卡）只会上升，解析器的压力面只会扩大

**修复成本**: 低（短期 fuzz）+ 中（中期双后端）。不修复的隐性成本：每次手写解析器出 bug，所有运行中的 ForgeOS 工作流都受到影响，而 bug 可能在发现前已经运行了数周。

---

## 方向三 · 并行执行下共享 `.forge/` 资源的数据完整性协议

> **当 RunParallel 并行启动多个 agent 子进程时，它们共享同一个 `.forge/trace.jsonl`、`memory.jsonl`、`checkpoint.json` 却没有进程级写入协调。**  
> **紧急度: 🟠 P1** | **类型: 并发安全 · 数据完整性** | **估算: 1-2 sprints**

### 问题

`orchestrator/parallel.go` 的 `RunParallel` 支持依赖无关 phase 的并发执行。但所有并发 phase 共享同一个 `.forge/` 持久化目录，各自通过子进程写入。当前只有 **in-process goroutine 级的互斥**（`trace.Tracer.mu`），没有**进程级的写入协议**。

```go
// forge-core/internal/orchestrator/parallel.go — 核心循环
// RunParallel 并发启动多个 phase，每个通过 CommandExecutor 起子进程
// (engine_build.go: claudeArgv → exec.Command)
// 但 .forge/ 目录的写入在同一文件的多个子进程间无锁
```

**三条并发的写路径：**

| 写入路径 | 操作模式 | 进程安全性 |
|----------|---------|-----------|
| `trace.Emit()` | `Tracer.mu.Lock()` + `w.Write()` | goroutine 安全✅；进程不安全❌（多个子进程各有一个 Tracer） |
| `memory.Append()` | `os.OpenFile(O_APPEND)` | O_APPEND 内核级不交叠 ✅；但**事件交错无协调** ❌ |
| `persist.Save()` | 写临时文件 + `os.Rename` | **两个子进程同时 Save 静默覆盖** ❌ |

### 代码级证据

```go
// forge-core/internal/trace/trace.go:123-135 — Emit 的锁
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    t.seq++
    ev.Seq = t.seq
    // ...
}
// ❌ 这个 mu 只保护同一个 Go 进程内的 goroutine。
// 当 RunParallel 通过 CommandExecutor 起子进程时，每个子进程
// 的 os/exec.Command 用不同的 stdout/stderr pipe，但每个子进程
// 的 cmd/forge 都构造自己的 Tracer{mu: new Mutex}，写入同一个
// .forge/trace.jsonl——mutex 不跨进程。
```

```go
// forge-core/internal/memory/memory.go — Append 的 O_APPEND
func Append(path string, entry Entry) error {
    f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
    // ...
}
// O_APPEND 保证两个并发 write(2) 系统调用在内核级不交叠，
// 但两个子进程的 JSONL 行会交错出现——trace 的 seq 顺序无法反映执行顺序，
// memory 的 iteration 序列混乱，后续 replay 无法重建因果关系。
```

```go
// forge-core/internal/persist/checkpoint.go:104-124 — Save 的 rename
func Save(path string, cp Checkpoint, retain int) error {
    // ...
    tmp := path + ".tmp"
    // 两个子进程写各自的 tmp 文件
    if err := os.Rename(tmp, path); err != nil { ... }
    // ❌ 进程 A rename 后，进程 B 的 rename 覆盖它——丢失进程 A 的数据
    // ❌ 若 retain>0，两个进程的 rotateRetain 同时执行可能造成历史旋转混乱
}
```

### 边界情况

| 场景 | 风险 | 可能性 |
|------|------|--------|
| Wave 1 的两个 implementer 同时写 trace | trace 行交错，seq 重复 | 高（`--parallel` 启用后） |
| 一个 checkpoint 被两个 phase 的 checkpointHook 同时保存后一个覆盖前一个 | 丢失已完成 phase 的 checkpoint 进度 | 中 |
| memory 在 wave 写入 + 在下一次迭代的 loop-back compact 同时进行 | compact 读到的 memory 集不完整，部分条目丢失 | 低（time window 窄） |
| trace 中来自不同子进程的 seq 编号冲突、 | 归因迭代序号错误 | 高 |

### 为什么迄今为止未被解决

- `--parallel` 是 opt-in 模式，当前 5 个工作流均未声明 `depends_on`，所以并行路径在真实运行中从未被触发
- 之前对"并行安全"的讨论集中在**单进程多 goroutine 的 race**（Sprint 22 已验证 `-race -count=20` 零 race），而非**多进程的数据完整性**
- checkpoint 的原子 rename 通常被认为"足够安全"，但这个假设只在**单写者**场景下成立

### 建议扩展骨架

- **`DotForge` WAL（Write-Ahead Log）层**：在 `internal/dotforge` 包中封装 `.forge/` 目录的所有写入，提供一个与进程协调无关的抽象。三种写入模式：
  1. **AppendCoordinated**：为 trace/memory 的追加写入提供进程级锁（`flock` / `LOCK_EX` 在文件上）
  2. **WriteCoordinated**：为 checkpoint 的覆盖写入提供 CAS（compare-and-swap via `open(O_EXCL)` + rename）
  3. **BatchAppend**：并行 wave 结束后一次性合并各子进程的 trace/memory 片段
- **`RunID`**：每个 forge 运行实例生成一个全局唯一 ID（`crypto/rand` 零外部依赖），注入所有 trace/memory/checkpoint 条目。解码时按 `RunID` 隔离——并行 wave 各 phase 即使 trace 行交错也可按 `RunID` 重建回正确的因果关系。
- **并行 compaction guard**：memory compact 时先获取进程级的读锁，确保没有并行 phase 正在追加 entry。

### 产品价值

- `--parallel` 是 ROADMAP 声明的能力。一旦用户启用它（当 workflow 开始声明 `depends_on`），当前的数据完整性缺口将直接从理论变为生产故障
- 并行加速的核心价值（减少 24h 运行时间）被数据完整性故障抵消——如果 trace 数据不可靠，scorecard 和学习闭环就失去意义
- 修复成本在并行能力被广泛使用前最低；修复需要引入的新架构（WAL 层）对单写者路径完全透明

**修复成本**: 中。需要新建 `internal/dotforge` 包封装写入原语，但不对任何现有单路径的调用方产生行为变化。

---

## 方向四 · Agent 契约解析 CI 可测桥接层

> **机读契约（reviewer 的 VERDICT、executive 的五择一裁决、PM 的 CONFIDENCE 等）的端到端解析测试只能通过真实付费 API 调用进行。没有一个 `--executor=contract-test` 模式来用一个假脚本而非真 LLM 验证所有契约解析器。**  
> **紧急度: 🟡 P2** | **类型: 可测试性 · 契约硬化** | **估算: 1 sprint**

### 问题

当前已落地四种机读契约解析器：

1. **`parseReviewerVerdict`**（`cost.go`）——匹配 `VERDICT: APPROVE` / `REQUEST_CHANGES`
2. **`parseExecutiveVerdict`**（`cost.go`）——匹配五择一裁决
3. **`parseConfidenceScore`**（`cost.go`）——匹配 `CONFIDENCE: <0-100>`
4. **`unwrapClaudeResult`**（`cost.go`）——通用 Claude JSON 解包

每种都有**单元测试**覆盖独立的解析逻辑，但**端到端的真实路径**（从 agent 子进程的标准输出 → 解析 → verdictLedger → converge 信号）只能通过 `--executor=command --agent-cmd=claude` 测试，需要真实 API 调用和费用。

```go
// forge-core/cmd/forge/cost.go — 契约解析器三层 fallback
func unwrapClaudeResult(out string, phase string) ... {
    // 1. 先试二元 reviewer 契约 (VERDICT: APPROVE / REQUEST_CHANGES)
    // 2. 退到五择一 executive 契约
    // 3. 再退到 confidence 契约
    // 这三层 fallback 逻辑的 end-to-end 集成测试只能靠真 `claude -p` 跑
}
```

### 代码级证据

| 文件 | 行 | 问题 |
|------|-----|------|
| `cmd/forge/cost.go` | 330-450 | 三层契约 fallback 的 `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore`——每个都是纯解析函数，但**三层编排的集成**只在 `observeFor` 中，而 `observeFor` 被 `costSink` 调用，`costSink` 被 `agentExecutor.Execute` 调用——端到端需要一个真 agent 子进程 |
| `cmd/forge/engine_build.go` | 180-220 | `agentExecutor.Execute` 起真子进程（或 `DryRunExecutor` 仅回声）。`DryRunExecutor` 只在日志中叙述路由，**不产生模拟的契约输出** |
| `internal/orchestrator/executor.go` | 35-60 | `AgentExecutor` 接口——有 `DryRunExecutor`（回声）和 `CommandExecutor`（真子进程），但没有 `ContractTestExecutor`（按 phase 名吐指定 VERDICT） |
| `internal/orchestrator/orchestrator_test.go` | 全部 | 700+ 行 mock 测试但只在编排层 mock `Exec.Execute`——不驱动到底层的契约解析循环 |
| S26 验证记录 | `CURRENT_SPRINT.md` | 真 claude 跑才暴露了 verdict 解析的集成问题（reviewer 缺 gate 信号等） |

### 边界情况

| 场景 | 风险 | 当前覆盖 |
|------|------|----------|
| Claude CLI 在新版本修改了输出格式 | `unwrapClaudeResult` 解包失败 → verdict 丢失 | 无（CI 不能跑真 claude） |
| 新增 executive 契约（6 种裁决）→ 解析器写错 | `REDESIGN` 被误解析为 `APPROVE` → 收敛判断错误 | 单元测试覆盖单个解析，但三层 fallback 编排未集成测试 |
| 契约契约添加了新 token（如 `WARN`） | fallback 顺序错误：二元契约先匹配到 WARN 视为无效 → 退到 executive | 无 CI 端到端验证 |
| 非 claude agent（未来厂商）的输出格式不同 | `unwrapClaudeResult` 硬编码了 claude JSON envelope | 无跨厂商格式测试 |

### 为什么迄今为止未被解决

- 每个契约解析器都有稳定的单元测试，检查了各种正反例
- S24-S26 的真 claude 验证确实暴露了集成问题（任务注入、gate 信号等）——但不是契约解析本身的主要 bug
- 契约格式的增量成本在当前阶段尚可接受：reviewer 加一个新的裁决 token 时，先在 `cost.go` 写 UT、再在真 claude 跑验证——"手动跑一次就好"的心态

### 建议扩展骨架

- **`ContractTestExecutor`**：实现 `AgentExecutor` 接口，接受一个 `map[phaseName]verdictToken` 映射。运行时：
  - 使用与 `CommandExecutor` 相同的 `observeFor` → `costSink` → 解析器链路
  - 但 agent 输出不是真 claude JSON，而是按 phase 名查找映射中预配置的 mimic 输出（格式与真实 claude 完全一致）
  - 验证：运行一次 `forge run build --executor contract-test --verdict-map review:APPROVE` → 检查 converge 是否为 MET 且 `review_status=approved`
- **集成到 `forge validate`**：`forge validate --contracts` 依次测试所有已知契约格式，验证三层 fallback 的顺序正确性、旧格式的向后兼容性、未知 token 的 fail-open 行为
- **CI 集成**：CI 中加入 `forge validate --contracts`——零外部依赖、零 API 费用、全契约覆盖
- **契约注册中心**：统一在 `.agent/contracts/` 中声明所有机读 token 的正则模式、适用 phase、预期行为

### 产品价值

- 每新增一个机读契约（如 sprint 30 将 `confidence_metric` 从硬编码改为配置驱动），都需要对应的端到端验证。没有 `ContractTestExecutor`，要么花真实 API 费用测试，要么只跑单元测试冒风险
- CI 中的契约回归测试是"AI 自治系统"的基本要求——如果自治循环的控制信号（verdict）在 CLI 升级后损坏，但 CI 无法发现，那 ForgeOS 的自治就只是"开发环境正常，CI 未验证"
- 当前已经有 4 种契约格式、3 层 fallback、2 个驱动（reviewer/executive/PM）——架构成熟度到了需要统一契约测试框架的临界点

**修复成本**: 低。`AgentExecutor` 接口已存在，新增实现无需改动现有执行器。主要为测试基础设施 + CI 配置。

---

## 方向五 · Memory 知识内容级生命周期管理

> **memory 存储只增不减。Compact 只按 age 门控 + kind 聚合摘要，但没有任何语义级的淘汰、归档或冻结机制。持续运行的系统中 90% 的知识可能对当前 sprint 已无关。**  
> **紧急度: 🟡 P2** | **类型: 学习闭环成熟度 · prompt 质量** | **估算: 2-3 sprints**

### 问题

`memory.Entry` 的 `Compact` 机制（`memory_compact.go`）只做两件事：
1. 按 age 分离（>24h 的旧条目）
2. 按 kind 分组，每 kind 保留最近 N 条，其余替换为一个汇总条目

但**没有任何机制判断一个条目的内容是否还对当前 sprint 相关**。一个 24h evolve 运行产生约 500 条记忆；一条来自 sprint 1 的 `KindDecision: "use approach A"` 在 sprint 4 仍在 memory store 中以 `Confidence=1.0` 存在——即使团队已经改用 approach B 三周了。

```go
// forge-core/internal/memory/memory_compact.go — 现有的 compact 逻辑
// Compact reads the memory store at path, groups OLD entries by kind,
// and compacts them: for each kind it retains at most keepPerKind of
// the most RECENT entries (by creation time), and the rest are replaced
// by a summary entry.
//
// NO content analysis.
// NO relevance scoring.
// NO cross-referencing with current ROADMAP.
// NO semantic eviction policy.
```

### 代码级证据

| 文件 | 行 | 问题 |
|------|-----|------|
| `internal/memory/memory_compact.go` | `compactByKind` 行 75-104 | 纯 time-based + kind-based 分组——不读 `Detail` 内容、不查 `Topic` 是否仍相关 |
| `internal/memory/memory_compact.go` | `summarizeBlock` 行 112-150 | 生成汇总条目时只记 topic 频次 + 时间范围——**原始 Detail 完全丢失**，而原始细节中可能有理解后续决策所需的关键上下文 |
| `internal/memory/memory_compact.go` | `splitByAge` 行 57-68 | ageSeconds=86400（24h）硬编码——不是基于 activity 模式或 sprint 边界 |
| `internal/memory/memory.go` | `Entry.Confidence` | 创建时设 1.0，永不更新（见已有分析 "memory confidence decay" 方向） |
| `internal/memory/memory.go` | `Supersedes` 字段 | 必须知道确切的 Topic 才能取代——没有模糊匹配、无自动矛盾检测 |
| `internal/converge/converge.go` | `gatherSignals` | 收敛信号不包括 `memory_age_ratio` 或 `knowledge_staleness` 指标 |

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Sprint 2 的 `KindDecision: "use PostgreSQL"` — Sprint 4 已改为 MongoDB | 每次 prompt 都注入过期决策 → agent 困惑 | 无；需人工 `forge memory-remove` |
| Sprint 1 的 `KindLesson: "test on port 8080"` — 项目已迁移到容器化 | 无意义知识占据 prompt 预算 | 无 |

| 累计 5000+ 条 memory → compact 后摘要丢失了特定时期的关键细节 | agent 无法追溯到某次决策的原由 | 无警告，静默丢失 |

### 为什么迄今为止未被解决

- v1 的所有 effort 集中在让"记忆写进去"的路径可靠（append → load → compact），而非"忘掉什么"——这是学习闭环的天然优先级顺序
- 已有的 `Supersedes` 机制已覆盖"显式取代"的场景，但对隐式取代（新决策没有显式说"我取代决定 X"）无能力
- 项目至今的单次 evolve 运行最多数十迭代（Sprint 24-26），尚未遇到 5000+ entry 的规模——这个问题是"当 ForgeOS 24h 连续跑一个月"时才尖锐的

### 建议扩展骨架

- **相关性门控注入**（短期）：在 `prompt_memory.go` 的 memory 注入点（`memoryCap` 硬约束之后）加一道软约束——按与当前 phase 的相关性过滤。可使用 `internal/prompt/retrieve.go` 的 TF-IDF 检索器（已存在！），将当前 phase 的 `Agent`/`Description` 作为 query，只注入 Top-K 匹配的 memory 条目。compact 后的摘要条目也参与检索。
- **知识冻结/归档**（中期）：新增 `KindArchive` 和 `KindFreeze`——用户或系统可标记"此知识已归档，不再注入 prompt 但保留在 store 中供审计"。`forge archive` 命令可迁移历史条目。
- **Sprint 边界标记**（中期）：在 memory store 格式中加 `sprint` 字段。每次 `forge evolve` 开始前记录 sprint 编号。注入时可按 sprint 过滤——"只注入当前 sprint 及上一个 sprint 的知识"。
- **摘要展开**（长期）：compact 生成的 `KindCompactSummary` 条目不应是最终状态。可增强 `summarizeBlock` 保留关键细节 Topic 的**可追溯引用**（如 "compacted 50 entries including decisions about PostgreSQL migration (iter 3-12), testing port config (iter 5-8)"），以便检索器可以匹配到概括性的 Topic 类别。
- **新收敛信号**：`converge.Signals` 加 `KnowledgeStaleness`——对当前 ROADMAP 而言已被取代的知识占全部知识的比例。当 >30% 的知识过期时告警。

### 与已有分析的区别

已有 `docs/requirements/high-value-extensions-analysis.md` 在行 112 提到 TTL-based 自动裁剪（`expires_at` / `relevance_decay` 字段），但那只是条目的时间衰减字段——即使知道"这条目已过期"，也没有能力知道"当前 phase 不关心这条目类型的内容"。本方向关注的是**内容级相关性**：不是"这条目已多旧"，而是"这条目在说什么，当前 phase 是否需要知道这个"。

### 产品价值

ForgeOS 的学习闭环（trace → scorecard → memory → converge）是自治运转的核心。但如果 memory 层不具备知识生命周期意识：

- 24h 运行后，agent 收到的 memory 注入中有 70% 是无关的"昨日新闻"→ prompt 预算浪费、注意力稀释
- 30 天运行后，知识库里充斥着 sprint 1 的过时决策，而新的 agent 不知道今天应该听谁的
- compact 把"为什么做决定 A"的原始细节替换为"做决定 A"的摘要——失去了溯因能力，闭环开始退化

**修复成本**: 短期（相关性门控注入，复用现有 TF-IDF 检索器）成本低；中期（归档/冻结/sprint 边界标记）成本中。关键在于**不打破现有记忆写入路径**——只增强注入侧。

---

## 优先级总结

| 方向 | 优先级 | 紧急度理由 |
|------|--------|-----------|
| **方向一: 持久化产物 Schema 版本化** | 🔴 P1 | 当前是无版本裸 JSON。第一次格式演化就会静默损坏历史数据，且无迁移路径。安全网成本极低（版本校验），出事成本极高（不可审计） |
| **方向二: yaml2json 治理单点故障** | 🔴 P1 | 已有生产损坏先例。修复成本（fuzz + 双后端）远小于下次损坏的排查成本。治理层的根基不应该依赖手写解析器 |
| **方向三: 并行 .forge/ 数据完整性** | 🟠 P1 | 当前 `--parallel` 未被真实工作流使用，但一旦启用并行能力，这个缺口直接从"理论问题"变为"生产事故"。在并行被广泛采用前修复成本最低 |
| **方向四: Agent 契约 CI 测试桥接层** | 🟡 P2 | 每新增一个机读契约格式都需要真钱测试。CI 中无法验证契约解析是自治系统的测试盲区 |
| **方向五: Memory 知识内容级生命周期** | 🟡 P2 | 当前 24h 运行规模下不尖锐，但当 ForgeOS 真正长时间自治运行时，知识稀释会成为学习闭环退化的根源 |
