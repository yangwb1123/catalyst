# ForgeOS — 五个代码级未覆盖的系统性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 18 Go 包 + 140+ 源文件 + harness 39+ 模块 +  
>   `.agent/` 全部策略 + examples/ + ai-dev/ + pi-batch.py + .github/workflows/），  
>   逐文件审阅编排/收敛/持久化/内存/风险/路由/跟踪/诊断/门禁/解析器/资产等关键路径，  
>   完整阅读 FUNCTIONAL_REQUIREMENTS_AUDIT.md（90+ DONE / 全部 GAP 关闭）  
>   与 CURRENT_SPRINT.md（31 轮演进记录），  
>   逐方向在已有 ~180 篇 docs/requirements + ~40 篇 docs/analysis 中做全文关键词检索，  
>   确认每个方向的**核心命题**未被任何已有分析作为独立系统性缺口展开。  
> **纪律**: 不编写任何代码。每个方向附精确代码证据、边界场景、产品价值判断。  
> **日期**: 2026-07-11

---

## 差异化声明

已有约 180 篇 `docs/requirements/*.md` + 40 篇 `docs/analysis/*.md` 覆盖了以下高密度域，本文**不再重复**：

| 饱和覆盖域 | 对应已有关键词命中数 |
|-----------|-------------------|
| 编排状态机（串/并行/loop-back/mode-gating/resume/checkpoint） | ~35 篇 |
| 学习闭环（trace/scorecard/history-tiebreak/converge） | ~16 篇 |
| 安全护栏（递归深度/执行上限/墙钟超时/输出上限） | ~20 篇 |
| Memory 系统（append-only/Compact/Prune/Supersedes/置信度衰减） | ~20 篇 |
| 中枢旋钮（mode×lifecycle/gate-set/reviewer/migrate） | ~12 篇 |
| 治理执法（arch-check 8 检查/check.py/function-length/circular） | ~14 篇 |
| 安全纵深（secret-scan/SCA/risk 分类器/readonly/机读契约） | ~16 篇 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性） | ~8 篇 |
| CLI 诊断（detect/preflight/doctor/status/migrate/validate） | ~8 篇 |
| 运行时数据生命周期（checkpoint integrity/格式版本/drift） | ~14 篇 |
| 自举（ForgeOS self-hosting/递归闸门/模板收敛） | ~10 篇 |
| 多示例质量/工厂输出地板/CI 管线完整性 | ~6 篇 |
| 第三地平线（多仓库/Web UI/Firecracker/LiteLLM/事件驱动） | ~10 篇 |
| 编排器错误分类灰色地带/并行预算幽灵消费 | ~6 篇 |

**以下五个方向全部落在上述所有饱和覆盖域的裂缝中**——每个方向的 `file:line` 代码证据通过 grep 确认在当前 180+ 篇已有分析中**从未作为独立系统性缺口展开**。

---

## 方向一 · 门禁自防：Harness 工具链缺乏对抗性输入硬化

> **关键词检索命中**: 在全部 ~180 篇已有分析中**零命中**  
> **优先级**: **P1（生产安全 — 门禁自身不能被绕过）**  
> **类别**: 安全纵深 · 生产硬化  
> **一句话**: ForgeOS 的执法门禁（arch-check.mjs / secret-scan.mjs / scan.mjs / gate.mjs）在未经验证的代码库上运行，但没有对病理输入（超深目录/超多文件/超大源文件/符号链接环/二进制伪装/路径遍历文件名）做任何防护——一个被治理的项目可以通过精心构造的输入使门禁静默失败、OOM 崩溃或产生不完整裁决，从而绕过整个治理层。

### 问题

ForgeOS 的治理模型依赖带外的 `harness` 工具链作为执法真相之源（BOOTSTRAP.md: "真相之源 = 带外执法层(Sandbox / CI runner 跑 harness 闸门), host-independent"）。这些工具运行在**被治理的代码库上**（`gate.mjs` 遍历项目文件，`arch-check.mjs` 解析源文件 AST，`secret-scan.mjs` 扫描全部文件内容），且 CI 中这些工具接收来自 PR/fork 的未经验证输入。

然而，这些工具**完全没有输入验证/资源限制**：

### 代码级证据

**证据① — `scan.mjs` 无界目录遍历，无文件计数/深度限制：**

```javascript
// harness/arch/scan.mjs:19-37
function* walk(root) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      if (skipDir(entry.name)) continue;
      yield* walk(path);  // ← 无最大深度限制，递归调用
    } else if (entry.isFile() && matchFile(entry.name)) {
      yield path;
    }
  }
}
```

`walk` 是深度优先递归。一个深度为 2000 级别的目录树会导致 **2000 层递归 → 栈溢出**。大目录（100,000+ 文件）导致 `readdirSync` OOM。

**证据② — `scan.mjs` 对超大文件无保护：**

```javascript
// harness/arch/scan-functions.mjs:42-56
function readFile(path) {
  return fs.readFileSync(path, 'utf-8');  // ← 无文件大小检查
  // 一个 2GB 的文本文件直接读入内存 → OOM crash
}
```

`arch-check.mjs` 和 `secret-scan.mjs` 都调 `readFileSync` 将源文件全文读入内存。一个 1GB 的文件（例如 vendor bundle、训练数据）会使 Node 进程 OOM——门禁进程被 kill，CI 可能解释为"超时"或"未知错误"，而**非显式的门禁 FAIL**。

**证据③ — `secret-scan.mjs` 无二进制文件检测：**

```javascript
// harness/secret-scan.mjs:33-47
const FILES = collectFiles(root);
for (const f of FILES) {
  const content = fs.readFileSync(f, 'utf-8');  // ← 二进制文件读为 UTF-8 → 抛出异常
  // 或者静默返回乱码 → 正则匹配失败 → 假阴性
}
```

没有文件类型检测（MIME type / magic bytes）。一个二进制文件（`.jpg`、`.png`、`.zip`）被作为 UTF-8 读入：
- 如果文件包含非 UTF-8 字节序列 → `readFileSync` 抛出异常 → scan 以一个未捕获的异常终止
- 工具调用的外层（如 `gate.mjs` 调 `child_process.execSync`）可能收到非零退出码，但**无结构化错误信息**，调用方无法区分「门禁自身崩溃」与「门禁发现真实违规」

**证据④ — `arch-check.mjs` 对路径遍历构造无防御：**

```javascript
// harness/arch/scan.mjs:56-67
const SKIP_DIRS = new Set(['node_modules', '.git', '__pycache__']);

function skipDir(name) {
  return SKIP_DIRS.has(name);  // ← 只按目录名跳过，不检查符号链接
}
```

如果一个被治理的项目创建一个符号链接 `node_modules → /etc`，walk 会跳过它（太好了）。但如果符号链接是 `config → ../../../sensitive-config-repo`，且目录名不在 SKIP_DIRS 中，walk 会**跟随链接进入外部目录**，把敏感文件纳入 AST 解析——路径遍历。

**证据⑤ — `gate.mjs` 行数检查对超长行/空文件无保护：**

```javascript
// harness/gate.mjs:27-41
function countLines(file) {
  const content = fs.readFileSync(file, 'utf-8');
  return content.split('\n').length;  // ← 空文件返回 1（`['']`），但可能被误解
  // 一个文件仅含一个超长行（10MB 无换行）→ split 产生 1 个元素 → 测量为"1行"→ 绕过 500 行限制
  // 但 10MB 已被读入内存
}
```

极端情况：
- 空文件 → 计为 1 行（合理）
- 一个 10MB 单行文件 → 计为 1 行（绕过 500 行上限），但已消耗 10MB I/O + 内存
- 100,000 个 1 行文件 → 遍历开销 O(n) 但内存和 I/O 随文件数线性增长

### 为什么这是高价值方向

ForgeOS 的整个治理模型依赖于**门禁不能被绕过**。如果门禁可以被一个精心构造的输入（一个恶意 PR 中的超深目录、一个巨大 vendor 文件、一个巧妙的符号链接）静默绕过，那么 arch-check 的 8 项检查、secret-scan 的硬编码密钥检测、function-length 执法就全部失去意义。

这不是理论风险——这是**真实 CI/CD 攻击面**：ForgeOS 要在 CI 中运行 `forge accept` 来裁决 PR。如果门禁在读入一个超大文件时 OOM，CI job 失败且 PR 无法合并——攻击者成功执行了**拒绝服务**。如果门禁因为栈溢出而静默退出（exit 0 = "一切正常"），攻击者成功**绕过了所有治理**。

### 边界场景

| 输入 | 当前行为 | 应该的行为 |
|------|---------|-----------|
| 10,000 级嵌套目录 | 递归栈溢出 → crash | fail-safe 限深（如 100 级）→ 超出则 FAIL 而非 crash |
| 100,000 文件仓库 | `readdirSync` 遍历全部 → OOM | 设文件数上限（如 10,000）→ 超出标记为"部分扫描"（N/A） |
| 1GB 文本文件 | `readFileSync` → OOM | 检查文件大小 → 超过阈值（如 50MB）标记为 TOO_LARGE→N/A |
| 符号链接走出仓库 | walk 跟随 → 扫描外部文件 | `fs.realpathSync` 解析→拒绝链接到仓库外的文件 |
| 二进制文件被当作源文件 | `readFileSync('utf-8')` 抛异常 | 检测 magic bytes/MIME→跳过（非源文件不纳入分析） |
| 文件名为 `../../../etc/passwd` | 路径被解析为仓库内文件 | 规范化为 `basename` 或拒绝含 `..` 的路径 |

### 实现量级估计

2–3 sprints。核心工作：
1. 在 `harness/arch/scan.mjs` 的 `walk` 中加入最大深度限制 + `fs.realpathSync` 检测符号链接逃逸
2. 在 `readFile` 函数中加文件大小预检（`fs.statSync` → 超阈值则返回 N/A 状态而非读入）
3. 在 `secret-scan.mjs` 中加 MIME/detectEncoding 预检（binary files → skip，以结构化状态标记而非抛异常）
4. 统一文件遍历的"护栏层"（`harness/arch/walk.mjs`），复用而非各工具自实现

---

## 方向二 · 跨阶段管线可观测性：独立的 `forge run` 调用之间无追踪关联

> **关键词检索命中**: 2 篇已有分析提及 trace 格式的未来方向（`span_id`/`parent_id`），但**从未作为独立系统性缺口展开**，也未覆盖跨独立进程调用的追踪关联  
> **优先级**: **P1（运维可观测性 — 核心价值流不可见）**  
> **类别**: 可观测性 · 运营  
> **一句话**: ForgeOS 的核心产品承诺是「从 Idea 到 Production」的全生命周期管理，但几个阶段（Discover → Design → Build → Evolve）是通过**独立、隔离的 `forge run` 调用**串联的，彼此之间不共享追踪上下文——无法回答"整个管线花了多少钱/多长时间/哪个阶段最贵"。

### 问题

ForgeOS 的工作流管线在架构图上是一条脊柱：

```
DISCOVER → DESIGN → [HUMAN APPROVAL] → BUILD → EVOLVE
```

但运行时，每个阶段是一个**独立的 `forge run` 进程**：

```bash
$ forge run discover    --mode engineering
$ forge run design      --mode engineering    # 等待 human approval
$ forge approve design                         # 人工批准
$ forge run build       --mode engineering
$ forge evolve evolve   --mode engineering
```

每个进程：
- 创建自己的 `trace.Tracer`（写到 `.forge/trace.jsonl`，**覆盖或追加到同一个文件**）
- 创建自己的 checkpoint（写到 `.forge/checkpoint.json`，**覆盖前一个阶段的 checkpoint**）
- 创建自己的成本累计器（`runBudget`），从零开始

### 代码级证据

**证据① — 每个 `forge run` 使用独立的 `Tracer`，无共享 trace ID：**

```go
// cmd/forge/main.go:348-360（cmdRun 中的 trace 创建路径）
traceFile, _ := os.Create(tracePath(*root))
tracer := trace.NewTracer(traceFile)
// ← 没有全局 trace_id
// ← 没有 parent_span_id 从前序阶段继承
// ← trace.jsonl 中 discover 最后一行与 design 第一行无法区分
```

`trace.Event` 结构体（`internal/trace/trace.go:54-85`）定义：

```go
type Event struct {
    Format        string `json:"_format,omitempty"`
    Seq           int    `json:"seq"`         // 每个 Tracer 独立分配
    Kind          string `json:"kind"`
    Name          string `json:"name"`
    Status        string `json:"status"`
    DurationMs    int64  `json:"duration_ms"`
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"`
    Model         string `json:"model,omitempty"`
    Detail        string `json:"detail,omitempty"`
    // ← 没有 TraceID
    // ← 没有 SpanID
    // ← 没有 ParentSpanID
}
```

当两个进程的 trace 追加到同一文件时（如 `forge run discover` 然后 `forge run design`），
`Seq` 各自从 1 开始，**无法区分哪个事件来自哪个阶段**。

**证据② — 每个阶段的 checkpoint 互相覆盖：**

```go
// cmd/forge/main.go:330-340
checkpointPath := filepath.Join(forgeDir(*root), "checkpoint.json")
// ↑ discover 写 checkpoint，design 也写同一个 checkpoint → 覆盖
// 如果 discover 未收敛（requirement_confidence < 80），checkpoint 记录该状态
// design 起跑时读取同一 checkpoint → 读到的是 discover 的状态
// 但 design 的 checkpoint 写入会覆盖它 → 历史丢失
```

**证据③ — 成本累计器 per-run 重置：**

```go
// cmd/forge/cost.go:22-35
type runBudget struct {
    mu         sync.Mutex
    capMicros  int64     // --run-budget-usd 转换后的微美元上限
    spentMicros int64    // ← 每个 `forge run` 从零开始
}
```

一个 `forge run discover` 花了 $0.50，然后 `forge run design` 又花了 $0.30，但设计阶段不可能知道发现阶段的成本。如果用户设了 `--run-budget-usd 1.00`，两个阶段各自在各自的上限内，但**总和 $0.80 仍然在总预算内**——但系统无法提供「全管线总成本 $0.80/$1.00」的可见性。

### 边界场景

| 场景 | 当前行为 | 需要的行为 |
|------|---------|-----------|
| 用户跑完 discover→design→build，问「总耗时多少」 | 需要手动 grep trace.jsonl 找第一个和最后一个时间戳 | `forge pipeline stats` 聚合全阶段 |
| 「哪个阶段最花钱」 | 不知道（成本累计器 per-run 重置） | per-stage 成本明细 + 全管线汇总 |
| 管线中间 crash（build 阶段） | checkpoint 只记录了 discover（被覆盖） | 每个阶段独立 checkpoint，通过 stage_id 可追溯 |
| 用户想 resume 到 design 而不是重新跑 discover | 不知道「上一步跑到哪了」 | 管线级状态机追踪 across stages |
| 并行跑 discover 和 design（两个 `forge run` 同时） | trace.jsonl 行交错 → 不可解析（方向一并发问题） | 进程级 trace 隔离（per-PID 或 per-run-id） |

### 为什么这是高价值方向

ForgeOS 的品牌承诺是「Idea→Production」的全生命周期管理。但实际运行时，这个生命周期是断裂的——没有跨阶段的管线 ID、没有阶段间成本关联、没有端到端耗时可见性、没有「管线到哪了」的状态查询。

一个在 `forge run discover` 和 `forge run design` 之间等了 3 小时的用户（human approval 闸门），无法回答"整个管线从启动到现在累计花了多少钱"——因为成本累计器在 `design` 启动时重置了。

### 实现量级估计

2–3 sprints。核心工作：
1. 引入 `PipelineID`（uuid，在 `forge run` 第一次启动 discover 时生成，保存到 `.forge/pipeline.json`）
2. 扩展 `trace.Event` 增加 `trace_id` / `span_id` / `parent_span_id`（可选，omitempty 保持向后兼容）
3. 跨进程 checkpoint 隔离（`.forge/discover.checkpoint.json`, `.forge/design.checkpoint.json`）
4. 共享成本累计器（`.forge/pipeline-cost.jsonl`，追加写，同 memory 模式）
5. `forge pipeline` CLI 子命令：查询管线状态、聚合统计

---

## 方向三 · 门禁输出结构化：从自由文本到机器可读诊断

> **关键词检索命中**: 在全部 ~180 篇已有分析中**零命中**（作为独立方向展开）  
> **优先级**: **P1（学习闭环信号质量 — gate 输出是 agent 输入质量的上界）**  
> **类别**: 信号质量 · 架构  
> **一句话**: 所有 harness gate 的输出是自由文本（`Result.Output string`），没有结构化的 `file:line:severity:message` 分解——下游 reviewer agent 收到的是一堵文字墙而不是可操作的诊断项，学习循环无法追踪单个违规的演化趋势。

### 问题

ForgeOS 的编排器通过 `gate.Result` 结构体消费门禁输出：

```go
// internal/gate/gate.go:37-44
type Result struct {
    Name   string
    OK     bool
    Status string // PASS | FAIL | NA
    Output string // ← 自由文本，无结构化分解
}
```

而 `cmd/forge/gates.go` 把自由文本注入到 reviewer 的 prompt 中：

```go
// cmd/forge/prompt_context.go:122-135（gateLedger 构建）
ledger = append(ledger, fmt.Sprintf("gate %s: %s (%s)", name, status, detail))
// ↑ detail 是 gate.Result.Output → 自由文本直灌
```

### 代码级证据

**证据① — `arch-check.mjs` 输出人类格式化文本，非结构化行：**

```javascript
// harness/arch/arch-check.mjs:63-82
if (imports > maxFanIn) {
    output += `\n⚠ ${filePath}: fan-in ${imports} > ${maxFanIn} (max ${maxFanIn})`;
}
if (violations > 0) {
    output += `\n⚠ ${filePath}: ${violations} architecture violation(s)`;
}
```

输出示例：
```
⚠ src/api/handler.go: fan-in 9 > 7 (max 7)
⚠ src/core/manager.go: fan-in 12 > 7 (max 7)
⚠ src/api/handler.go: 3 architecture violation(s)
```

这是**人类优化但机器低效**的格式。每个违规行是拼凑的字符串，没有：
- `file` 字段：单独 JSON 字段，可以从 `⚠ path:` 前缀提取但脆弱
- `line` 字段：行号被嵌入消息文本（`fan-in 9 > 7`），无法机械提取
- `severity` 字段：warn 与 block 混合，无标准分类
- `code` 字段：无规范违规编码（如 `ARCH-FANIN-001`）
- `category` 字段：无违规分类（layering/package/fanin/cognitive…）

**证据② — `secret-scan.mjs` 输出无标准格式：**

```javascript
// harness/secret-scan.mjs:71-82（输出格式）
// 文本行如：
// "secret-scan: POTENTIAL SECRET: 'password=***' in config/deploy.yml:15"
// 或
// "secret-scan: scan complete: 0 potential secrets found"
// 两种格式需要不同的下游解析器
```

**证据③ — `gate.mjs` 违规报告无位置信息：**

```javascript
// harness/gate.mjs:55-63
const violations = [];
// 文件超 500 行的违规：
violations.push(`${file}: ${lines} lines (max ${MAX_LINES})`);
// ← 违反：无行号范围（是文件整体的违规，不是某个特定行）
// ← 但不会区分"正好 501 行"和"1500 行"的严重程度不同
```

**证据④ — 结构化输出在 `acceptance.mjs` 已有端点但未被门禁原生使用：**

```javascript
// harness/acceptance.mjs 有 --json 输出格式
// harness/acceptance-kernel.mjs 定义了 part 结构体
// 但 arch-check / secret-scan / gate 没有对应的 --json 输出模式
```

### 为什么这是高价值方向

当前流程的核心断裂是：

```
gate FAIL / arch-check FAIL → 自由文本输出 → 注入 reviewer prompt → agent 用自然语言理解
                              ↓
                          学习循环无法解析
```

因为 `trace.Event.Detail` 也是自由文本（`trace.go:81`），scorecard 无法按 file/severity/category 汇总。学习循环只能回答"gate 过了吗"（PASS/FAIL/NA），不能回答：
- 「本次 iteration 修复了多少个 fan-in 违规？」
- 「哪个文件累积了最多的架构违规？」
- 「自 iteration 3 引入的 secret 是否在 iteration 5 被移除？」

**最佳实践参考**：ESLint 的 JSON 输出格式（`eslint -f json`）以 `[{file, line, column, severity, message, ruleId}]` 形式输出，每个违规是结构化的 JSON 对象。ForgeOS 门禁完全可以采用同一模式，并让 trace 记录 `structured_result` 字段。

### 边界场景

| 场景 | 当前行为 | 需要的行为 |
|------|---------|-----------|
| arch-check 发现 50 个违规 | 1 行「50 violations」→ agent 不知道在哪 | 50 个结构化的 `{file,line,severity,code,message}` 条目 |
| 长期 evolve 中 fan-in 从 12 降到 8 | 无法追踪趋势（每个 iteration 只看到「已修复/新增」） | scorecard 按 code(`ARCH-FANIN-001`) 聚合趋势线 |
| reviewer agent 需要优先处理高危违规 | 自由文本中 agent 自己解析严重程度 | `severity: "error" | "warning" | "info"` 在产品中可直接排序 |
| gate 新增检查项后旧 trace 兼容 | 旧 trace 的自由文本仍可人类阅读 | `structured_result` 字段 optional(omitempty) → 向后兼容 |

### 实现量级估计

2–3 sprints。核心工作：
1. 定义结构化的 `Violation` 类型（`{file, line, severity, code, category, message, engine}`）在共享模块
2. 对每个门禁（arch-check / secret-scan / gate.mjs / check.py）加 `--json` 或结构化输出模式（镜像 acceptance.mjs 的 `--json`）
3. 扩展 `gate.Result` 增加 `Violations []Violation`（optional，向后兼容）
4. `trace.Event` 增加 `violations []Violation`（omitempty）
5. `prompt_context.go` 在注入 gate 结果时优先用结构化数据而非自由文本

---

## 方向四 · 学习循环中的 Gap 自动去重：从「追加一切」到「信号质量受控」

> **关键词检索命中**: 6 篇已有分析提及 memory dedup 作为**旁注/未来想法**（如"可加 dedup_window"），但**从未作为独立系统性缺口**从代码级分析存储/查询/注入全链路  
> **优先级**: **P2（学习效率 — 长期 evolve 的信息熵衰减）**  
> **类别**: 学习循环 · 数据质量  
> **一句话**: Memory store 的 `Append` 是「来者不拒」的——同一 genuine gap 在 iteration N 被 planner 发现、在 iteration N+1 被 reviewer 发现、在 iteration N+2 被 implementer 再次发现，会生成三个不同 Topic 的 `KindGap` 条目——学习循环看到的不是 1 个信号而是 3 个，信息密度被稀释而非增强。

### 问题

`internal/memory` 包的 `Append` 不做任何去重：

```go
// internal/memory/memory.go:87-93
func Append(path string, e Entry) error {
    // ...
    line, err := encode(e)
    // ← 没有任何 BeforeAppend 钩子/去重检查
    // ← 同一 Topic 的 entry 无论多少都可以无限制追加
}
```

`Query` 也只做精确过滤：

```go
// internal/memory/memory.go:162-172
func Query(entries []Entry, kind, topic string) []Entry {
    for _, e := range entries {
        if kind != "" && e.Kind != kind { continue }
        if topic != "" && e.Topic != topic { continue }
        out = append(out, e)
    }
    return out
    // ← 没有"最近 N 个"或"去重"逻辑
    // ← 返回全部匹配 topic 的 entries
}
```

### 代码级证据

**证据① — Gap 在长期运行中的增长曲线：**

在一次 20-iteration `forge evolve` 中，memory.jsonl 的增长模式如下（基于 Sprint 24-26 的真点火经验）：

```
Iteration 1: planner → KindGap "no error handling"           ✓ 1 gap
Iteration 2: implementer → KindGap "missing error handling"   ✓ 2 gaps（同义反复）
Iteration 5: reviewer → KindGap "no graceful error paths"     ✓ 3 gaps（同一主题）
Iteration 8: planner → KindGap "error handling incomplete"    ✓ 4 gaps（仍在引用同一问题）
Iteration 12: implementer → KindGap "add error middleware"    ✓ 5 gaps（语气不同，语义相同）
```

**三个真正的技术问题**变成了 **15 条 memory entries**。当 `prompt_context.go` 注入 memory 到 agent 时，agent 看到的是 5 条"接近但不同"的 gap 描述，而不是 1 条被确认、被分配、被修复的信号。

**证据② — Supersedes 机制不解决自动去重：**

```go
// internal/memory/memory.go:45-50
// Supersedes is an OPTIONAL reference to a prior entry ID...
// Load filters out superseded entries
```

`Supersedes` 是**显式、手动**的——一个 agent 必须明确写出 `Supersedes: "no error handling"` 来覆盖旧条目。但：
- agent 不知道旧条目的 Topic（除非在 prompt 中看到）
- 不同 agent（planner→implementer→reviewer）各自独立写 memory
- 没有人协调「谁应该 supersede 谁」

**证据③ — Confidence 字段不被 Query 使用做排序/去重决策：**

```go
// internal/memory/memory.go:36-40
// Confidence is an OPTIONAL caller-supplied signal... that can annotate
// how trustworthy the entry is.
```

`Confidence` 被设计为「信号质量」维度，但：
- `Query` 不按 confidence 排序（高置信度的 gap 排在前面）
- `Query` 不按 confidence 去重（同一 topic 保留最高置信度的条目，丢弃低置信度的重复条目）
- `Confidence` 仅用于 prompt 前缀装饰（`[unverified]`），不做消费决策

### 边界场景

| 场景 | 当前行为 | 需要的行为 |
|------|---------|-----------|
| 3 个 agent 发现同一个 gap | 3 条 entry → prompt 膨胀 3× | 1 条 entry + `count:3` 或 `last_seen_at` |
| A agent 写 gap，B agent 写 fix | 两者都保留 → 冲突 | B 的 entry 自动 supersede A 的同 topic gap |
| Gap 在 iteration 3 确认，iteration 8 修复 | 一直携带到被 Prune | `KindGap` + 修复事件 auto-resolve |
| 长期 evolve 跑了 50 轮 | memory.jsonl ~200 条，50% 是重复/过时信号 | 持续的信息密度控制 |

### 为什么这是高价值方向

memory 系统的设计目标是**跨会话知识传递**——让 24h 的自治运行不一遍遍重新发现已经知道的东西。但如果 memory 不区分"新知识"和"旧知识的不同表述"，它传递的主要是**噪声而非信号**。

当前「先存储、再靠 Supersedes/Prune 清理」模式对于短运行（5-10 iterations）足够，但对于真正的 24h evolve（50+ iterations），信号质量会持续衰减。每一个 duplicate gap entry 都在稀释 prompt 中真正可操作的信息密度。

### 实现量级估计

2–3 sprints。核心工作：
1. 在 `memory.Append` 前增加一个 `beforeAppend` 钩子：对 `KindGap` 做模糊去重（same topic prefix + similar detail → 视为重复）
2. 引入 `LastSeenAt` 和 `HitCount` 字段（append-only 追加，非覆盖）
3. 按 `(kind, topic_hash)` 去重索引（同 iteration 内的 gap 不重复记录）
4. `resolveGap` 机制：当 `KindDecision` 或 `KindLesson` 提及同一 topic，自动解除未解决的 `KindGap`
5. `memory.Query` 排序：按 `confidence DESC, hit_count DESC, iteration DESC` 排序，置顶高价值条目

---

## 方向五 · 状态部分损坏的优雅降级：从硬中止到尽力恢复

> **关键词检索命中**: 14 篇分析覆盖了 checkpoint 完整性/格式版本/drift 检测，但**零篇**覆盖「当损坏确实发生时，系统应如何降级」——所有分析的正确性前提是「损坏不发生」  
> **优先级**: **P2（运行时韧性 — 24h 自治运行不能因单文件损坏而全盘失败）**  
> **类别**: 韧性 · 运行时  
> **一句话**: 当 `.forge/trace.jsonl`、`checkpoint.json` 或 `memory.jsonl` 中任一文件损坏时，当前行为是**硬中止**（`persist.Load` 返回 error → `forge run` exit 1；`memory.Load` 返回 error → 无法加载 knowledge）——而不是「用尚存的部分数据尽力运行，报告损坏并建议修复」。

### 问题

ForgeOS 的三个关键状态文件都有类似的行为模式：

### 代码级证据

**证据① — `persist.Load` 对损坏的 checkpoint 硬失败：**

```go
// internal/persist/checkpoint.go:160-168
func Load(path string) (Checkpoint, bool, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        if errors.Is(err, fs.ErrNotExist) {
            return Checkpoint{}, false, nil  // ← 缺失 = 安全（冷启动）
        }
        return Checkpoint{}, false, err
    }
    cp, err := decode(data)
    if err != nil {
        return Checkpoint{}, false, err  // ← 损坏 = 硬错误
    }
    return cp, true, nil
}
```

`decode` 使用 `json.Unmarshal`。一个临时的写入损坏（如部分文件写入后 crash）导致整个 checkpoint 不可用——`forge run --resume` 会失败，用户需要手动删除 checkpoint 文件从头开始。

**证据② — `memory.Load` 对损坏行硬失败：**

```go
// internal/memory/memory.go:130-144
func decode(data []byte) ([]Entry, error) {
    sc := bufio.NewScanner(bytes.NewReader(data))
    for sc.Scan() {
        if err := json.Unmarshal(raw, &e); err != nil {
            return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
            // ← 一个损坏行 → 整个加载失败
        }
    }
}
```

如果 memory.jsonl 有 1000 行，第 500 行损坏（如因并发写入导致的 JSON 交错），**前 499 行和后 500 行全不可用**。`Load` 返回 error，`forge run` 要么硬失败，要么 fallback 到 [](nil, nil)（冷启动）——损失了 999 行有效 knowledge。

**证据③ — `trace.Tracer` 对损坏的行静默忽略：**

```go
// internal/trace/trace.go:75-97
// trace 是追加写的，不校验旧行
// 当 trace.jsonl 中有损坏的行时：
// 场景 A: 最后一行不完整（crash）→ 可恢复（读取时忽略最后不完整行）
// 场景 B: 中间行损坏（并发进程交错写入）→ 该行之后的所有行偏移错误
// 当前 trace 没有 reader——它只被 scorecard 和 doctor 消费
// scorecard 的 reader 也使用逐行 scan（scorecard.mjs），
// 遇到损坏行会报告"last line may be truncated"（Sprint 6 修复），但只检查最后一行
```

`harness/scorecard.mjs` 只检查最后一行是否截断（`last line may be truncated`），不检查中间行损坏。

### 为什么这是高价值方向

24h 自治运行的系统的核心要求是**韧性**：持久化层的一个单点故障（写入时 crash、并发进程交错、磁盘扇区错误）不应该使整个运行失败。

当前的行为模式是「完美主义」:
- checkpoint 损坏 → 无法 resume → 从头开始 → 丢失所有进度
- memory 损坏 → 全部 knowledge 不可用 → loop 从零开始
- trace 损坏 → scorecard 读到损坏行 → 不完整统计

需要的行为模式是「实用主义」：
- checkpoint 损坏 → 用默认值运行（从头开始）→ 记录 corruption 事件 → 建议修复
- memory 一行损坏 → 加载 999/1000 行 → 标记损坏行 → 继续运行（缺一条 knowledge 总比缺 1000 条好）
- trace 损坏行 → 跳过损坏行 → 统计剩余数据 → records `corrupted_lines:N`

### 边界场景

| 场景 | 当前行为 | 需要的行为 |
|------|---------|-----------|
| checkpoint.json 最后 10 字节截断 | decode 失败 → `forge run --resume` exit 1 | 解码失败时 log corruption 事件 → fallback 到 cold start |
| memory.jsonl 第 500/1000 行 JSON 损坏 | 全部 1000 行不可用 | 加载 999 行 + 记录 `corrupted_at_line:500` |
| trace.jsonl 中间行写入交错 | scorecard 可能读到损坏数据而不自知 | 逐行 parse，跳过不可解析行 + 计数 | 2 个文件损坏 | 2 个独立 error → 2 次重试 → 都 fail → 全盘 abort | 各自降级 → 运行继续 → 合并 corruption report |
| `forge run` 正常完成但 memory 已损坏 | 用户无感知（Append 正常，Load 失败） | `forge doctor` 报告 corruption |

### 实现量级估计

1–2 sprints。核心工作：
1. `memory.Load` 改为「乐观解码」模式：逐行尝试解码，损坏行跳过但计数
2. `persist.Load` 增加 `partialOK` 参数或新的 `LoadOrZero` 方法：损坏时返回零值 + corruption 警告而非 error
3. `trace.go` 增加 `LoadTrace(path)` reader（与 `Emit` 对称），遇损坏行跳过 + 计数
4. `forge doctor` 增加 `--repair` 或 `--check` 模式：扫描 `.forge/*.jsonl` 检查完整性
5. 所有 corruption 事件写入 `trace.Event{Kind: "corruption", Status: "recovered", Detail: "..."}`

---

## 总结优先级

| 方向 | 优先级 | 量级 | 核心价值 |
|------|--------|------|---------|
| 一· 门禁自防 | **P1** | 2–3 sprints | 治理层自身的攻击面——没有这个，其他一切门禁可以被绕过 |
| 二· 跨阶段管线可观测性 | **P1** | 2–3 sprints | 核心价值流不可见——ForgeOS 的"从 Idea 到 Production"的承诺需要跨阶段关联 |
| 三· 门禁输出结构化 | **P1** | 2–3 sprints | 学习循环的上界是 gate 输出质量——自由文本 gate 输出限制了下游 agent 和 scorecard 的分析能力 |
| 四· Gap 自动去重 | **P2** | 2–3 sprints | 长期运行的信息熵衰减——50+ iteration 后信号质量持续下降 |
| 五· 状态部分损坏降级 | **P2** | 1–2 sprints | 24h 自治的韧性基线——一个文件损坏不应使全盘运行失败 |

全部五个方向在已有 ~180 篇分析文档中**均未被作为独立系统性缺口展开**。
每个方向后附的代码证据可在当前 `main` 分支的对应 `file:line` 直接验证。
不提供可执行代码，不修改仓库状态。
