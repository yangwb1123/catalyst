# ForgeOS — 基于代码深扫的五个结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全库逐文件深扫（forge-core 18 Go 包/140+ 源文件 + harness 39+ 模块 + .agent/完整骨架 + examples/ + .github/workflows/），  
>   逐行审阅关键包的编排/收敛/持久化/内存/风险/路由/跟踪/诊断/提示上下文/成本/资产/门禁等实现。  
>   交叉校验 150+ 篇已有 docs/requirements + 40+ 篇 docs/analysis，确保每个方向的**核心命题**未在其他分析中作为独立子系统方向展开过。  
> **纪律**: 不编写任何代码。每个方向附精确 file:line 代码证据、边界场景、产品价值判断。  
> **日期**: 2026-07-11

---

## 与已有文档的差异声明

已有 190+ 篇分析文档覆盖了以下高密度领域，本文**不再重复**：

| 已被饱和覆盖的领域 | 代表文档（随机采样） |
|---|---|
| 编排状态机 / checkpoint / loop-back / resume / parallel / wave | ~35 篇 |
| 学习闭环 / scorecard / history-tiebreak / eval | ~16 篇 |
| 安全护栏 / recursion guard / budget guard / output cap / timeout | ~18 篇 |
| Memory / 价值衰减 / 置信度消费 / Compact / Prune / Supersedes | ~20 篇 |
| Prompt token 预算 / 上下文窗口仲裁 | ~8 篇 |
| 多厂商 Provider 抽象 / 非 Claude 路由 | ~6 篇 |
| 确定性回放 / 调试引擎 | ~5 篇 |
| 并行文件冲突检测 / 并发安全 | ~5 篇 |
| Checkpoint 完整性 / 格式版本执法 | ~5 篇 |
| 风险感知工作流适配 | ~4 篇 |
| 子进程生命周期管理 / 孤儿进程清理 | ~4 篇 |
| 跨项目舰队编排 / 联邦控制面 | ~6 篇 |

**本文的五个方向全部落在上述饱和域的裂缝/接口处**——不是已有抽象层的纵向深化，而是**横向裂缝**。

---

## 方向一 · 多实例隔离：当两个 forge 同时跑在同一个仓库

> **优先级**: P0（运维安全） | **类别**: 并发安全 · 状态管理  
> **一句话**: `.forge/` 的 checkpoint/trace/memory/cache 全部是单实例设计，并发运行导致静默数据损坏。

### 代码级证据

ForgeOS 的运行时状态全部集中在 `.forge/` 目录，但没有任何机制防止两个 forge 实例同时写入：

**1. checkpoint.json 竞争写入**

```go
// persist/checkpoint.go:71-94 — Save
func Save(path string, cp Checkpoint, retain int) error {
    // … write to <path>.tmp, fsync, rename atomically
}
```

原子 rename 保证**单次写入的原子性**，但不防止**两次写入的交错**。进程 A 写 iteration 5 的 checkpoint，进程 B（在另一个终端）同时写 iteration 3 的 checkpoint——后者 rename 覆盖前者。**operator 以为 resume 到 iteration 5，实际上 resume 到 iteration 3**。

更严重：如果 A 写 `checkpoint.json.tmp` 刚完成，B 开始写相同文件，A 的 rename 覆盖了 B 的 tmp 文件。没有进程级文件锁。

**2. trace.jsonl 交错行**

```go
// trace.go:75-97 — Emit
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    t.seq++
    // …
    t.w.Write(line)  // t.w 是 os.File，O_APPEND
}
```

`Emit` 的 `sync.Mutex` 保护**同一进程内**的并发 writer（并行 engine）。但两个不同进程的 `Tracer` 各自持有自己的 `os.File` 句柄，**mutex 不跨进程**。两个进程的 `O_APPEND write` 在内核层面交错→ trace.jsonl 中两行 JSON 互相穿插→ 整行不可解析 → `doctor` 报 `last line may be truncated`。

内核保证 `O_APPEND` 下单次 write 的原子性，但**不保证两次 write 之间的原子性**：A 写前半行 `{"seq":1,`，B 写前半行 `{"seq":1,`，A 写后半行 `"kind":"agent"}\n`，B 写后半行 `"kind":"gate"}\n`→ 文件内容：

```
{"seq":1,{"seq":1,"kind":"agent"}\n"kind":"gate"}\n
```

这是两行不可解析的垃圾。

**3. memory cache 在进程间失效**

```go
// memory.go:24-34 — loadCache
var loadCaches sync.Map  // per-path cache, per-PROCESS

func invalidateLoadCache() {
    loadCaches.Range(func(key, _ interface{}) bool {
        loadCaches.Delete(key)
        return true
    })
}
```

进程 A 的 `Append` 调用 `invalidateLoadCache`，**只清 A 自己的 cache**。进程 B 的 `loadFromCache` 仍命中过期 cache，拿到旧 memory 数据——memory 在不同进程间不同步。

**4. 无实例标识**

```go
// persist/checkpoint.go:44-54
type Checkpoint struct {
    FormatVersion     string  `json:"_format,omitempty"`
    Workflow          string  `json:"workflow"`
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    // … 没有 InstanceID / RunID / SessionID
}
```

如果两次 resume 指向同一个 checkpoint，无法区分是**同一实例的第二次 resume** 还是**不同实例的误恢复**。

### 为什么需要

| 维度 | 分析 |
|---|---|
| **运维安全** | 这不是理论问题。典型场景：CI pipeline（`forge accept` 在 CI 中跑）与开发者本地（`forge run build --mode explorer`）同时操作同一仓库。CI 的 checkpoint 被本地覆盖 → 下个 CI 构建恢复到了错误状态。 |
| **数据完整性** | trace.jsonl 是 scorecard/telemetry 的数据源。交错行导致 scorecard 数据不可靠，p95 延迟/平均成本统计失真。 |
| **信任基座** | 如果 forge-core 自己的持久化层不保证多实例安全，operator 会对「24h 自治运行说不会丢失进度」产生合理怀疑。 |
| **进化路径** | 当前是单实例 CLI 工具，但 ForgeOS 的 vision 包含「多 agent 工厂」——并行 evolve 是合理需求。v1 可以不做多实例安全，但必须**检测并防止**多实例并发。 |

### 边界场景

- 只读操作安全：`forge status` / `forge doctor` 是只读的，多实例并发无危害。
- 写写冲突窗口极窄（~几 ms），但在 24h 长跑中概率累积。
- 容器化环境（CI runner）每次启动在新容器 + 新 `.forge/`，不受影响。问题集中在**共享持久卷**或**本地开发机多终端**场景。

### 建议方向

**Phase A — 检测（~200 行，高杠杆）**:

1. 在 `persist` 包中增加 `InstanceID`：每次 `forge run`/`forge evolve` 启动时生成一个 UUID（`crypto/rand`，标准库）
2. Checkpoint 中记录 `InstanceID` + 启动时间戳
3. 每个写入操作（Save/Append/Emit）前尝试获取**建议性文件锁**（`flock` on Unix, `LockFileEx` on Windows）。获取失败→**打印警告**并继续（非阻断——不破坏现有行为）
4. `forge doctor` 增加 `--concurrent` 检查：`.forge/` 中是否存在其他活跃 `InstanceID` 的锁文件

**Phase B — 隔离（~400 行，可选）**:

- 为每个实例创建隔离的 `.forge/run-<ID>/` 子目录
- 在主 `.forge/locks/` 中记录运行状态
- 仍用 `.forge/checkpoint.json` 作为**唯一**可恢复的状态（通过锁协调）

### 诚实边界

- **flock 是建议性的**：不强制持有锁，防君子不防小人。运行中的 `forge` 进程监听 SIGTERM 清理锁文件（Sprint 27 信号处理已就绪），但 SIGKILL 不会清理——锁文件超时（TTL）留到 v2。
- **不做分布式锁**：v1 只做同一主机的文件锁。跨主机的 `.forge/` 共享（NFS）留给 v2。
- **不改变 trace/memory 的单文件追加格式**：定向到隔离目录即可，不改序列化格式。

---

## 方向二 · Agent 输出契约系统：从脆弱行尾解析到结构化输出合同

> **优先级**: P1（可靠性） | **类别**: 集成 · 可演化性  
> **一句话**: 系统用 3 种不同规则解析 agent 最后一行作为裁决/置信度，但无输出 schema、无契约声明、无降级路径，agent 格式变化即静默断裂。

### 代码级证据

当前系统有 3 个独立的「agent 最后一行解析器」，全部使用**精确字符串前缀匹配**：

```go
// cost.go:247-268 — parseReviewerVerdict
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:      // 精确匹配 "VERDICT: APPROVE"
    case "VERDICT: " + VerdictRequestChanges: // 精确匹配 "VERDICT: REQUEST_CHANGES"
    }
}
```

```go
// cost.go:271-301 — parseExecutiveVerdict
func parseExecutiveVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:              // 与 reviewer 相同的 token
    case "VERDICT: " + VerdictApproveWithSimplification:
    case "VERDICT: " + VerdictRedesign:
    case "VERDICT: " + VerdictDelay:
    case "VERDICT: " + VerdictReject:
    }
}
```

```go
// cost.go:304-326 — parseConfidenceScore
func parseConfidenceScore(output string) (score float64, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    numStr, hasPrefix := strings.CutPrefix(last, "CONFIDENCE: ")  // 精确前缀 "CONFIDENCE: "
    // … 解析数字
}
```

三个解析器共享同一个 `lastNonEmptyLine` 管道：

```go
// cost.go:337-344
func lastNonEmptyLine(s string) string {
    lines := strings.Split(s, "\n")
    for i := len(lines) - 1; i >= 0; i-- {
        if t := strings.TrimSpace(lines[i]); t != "" {
            return t
        }
    }
    return ""
}
```

以及同一个 `unwrapClaudeResult`：

```go
// cost.go:347-355
func unwrapClaudeResult(output string) string {
    var env struct { Result *string `json:"result"` }
    if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil || env.Result == nil {
        return output
    }
    return *env.Result
}
```

**关键脆弱点**：

1. **格式假设耦合在 agent 卡的内容中**：`VERDICT: APPROVE` 这个格式写在 `.agent/agents/reviewer.md` 的角色卡里。没有 format spec，没有 machine-readable schema。如果某个 future model 的 agent 在输出末尾加了 `---` 分隔线或 markdown 脚注，`lastNonEmptyLine` 返回的不是 VERDICT 行。

2. **三套解析规则，重复实现**：reviewer、executive、confidence 三者共享结构但各自实现。未来加入一个新的输出 token（如 `ESTIMATE: 5 story points`）需要写第 4 个解析器。

3. **没有契约声明**：workflow YAML 可知「此 phase 的 agent 是 reviewer」，但不知道「reviewer 的输出契约是什么」。契约散落在角色卡文本和 cost.go 的解析器中——修改角色卡格式时，不会忘记改 cost.go，因为没有编译时检查。

4. **静默失败**：解析失败（ok=false）被 orchestrator 解释为「fail-open → proceed」。这意味着如果 reviewer 的格式无意中变了，not APPROVE but not DISAPPROVE → **继续向前走，评审被静默跳过**。

5. **confidence 解析与 reviewer 解析不同步**：`parseConfidenceScore` 不认识 `VERDICT:` 前缀。如果某个 phase 的 agent 同时输出 verdict 和 confidence（planner 可能两者都出），一个解析器只能拿到一个。

### 为什么需要

| 维度 | 分析 |
|---|---|
| **从脆弱到健壮** | 三个精确行尾解析器是系统的信号神经——它们决定了「是否继续」、「是否需要修改」、「置信度够不够」。如果神经容易断裂，整个自治循环的信任就建立不起来。 |
| **模型升级风险** | 当 Anthropic 升级 Claude 模型时，新模型可能在输出中添加额外格式（自动 markdown、自动摘要、后置思考 token）。当前系统对这类变化零韧性。 |
| **扩展性** | 加入新 agent 角色（如 `security-reviewer`、`performance-reviewer`）需要重复实现第四/五个解析器。每个新解析器的 bug 可能导致误判。 |
| **诚实度** | 当前合同是「agent 卡说：输出最后一行用这个格式」。但系统不验证 agent 真的遵守了合同。输出完全不匹配时不失败，静默 proceed。 |

### 边界场景

- 空输出：agent 崩溃返回空字符串——`lastNonEmptyLine` 返回 `""` → 所有解析器 ok=false → proceed。崩溃被静默忽略。
- 多行重复：agent 输出了两行 `VERDICT: APPROVE\nCONFIDENCE: 85` → `lastNonEmptyLine` 只取最后一行（85），`VERDICT: APPROVE` 丢失。
- 包装格式：agent 输出在 markdown 代码块中——`lastNonEmptyLine` 返回 `` ``` `` 而不是 verdict。
- `unwrapClaudeResult` 只解一层 JSON：如果 claude 的 output-format 在未来的版本中改变 envelope 结构，解析器需要重写。
- **时序问题**：reviewer 的输出先经过 `parseReviewerVerdict` 尝试匹配（不匹配→ok=false），然后尝试 `parseExecutiveVerdict`——这是在尝试「双派解析」。如果 reviewer 的输出恰好匹配 executive 的某个罕见的 token，就被误分类了。

### 建议方向

**Phase A — 声明式输出契约（~400 行）**:

在 `asset.Phase` 中增加输出契约字段：

```go
type OutputContract struct {
    // Kind 标识 agent 应该返回的令牌类型
    Kind string `json:"kind"` // "verdict" | "confidence" | "estimate" | "structured"
    // Token 是精确的字符串常量（verdict 场景）
    Tokens []string `json:"tokens,omitempty"` // ["APPROVE", "REQUEST_CHANGES"]
    // Prefix 是行前缀（confidence 场景，如 "CONFIDENCE: "）
    Prefix string `json:"prefix,omitempty"`
    // Format 指定输出格式（未来扩展）
    Format string `json:"format,omitempty"` // "last_line_token" | "json" | "yaml"
}
```

在 workflow YAML 中声明：

```yaml
phases:
  - name: cto-review
    agent: cto
    output_contract:
      kind: verdict
      tokens:
        - APPROVE
        - APPROVE_WITH_SIMPLIFICATION
        - REDESIGN
        - DELAY
        - REJECT
```

然后用一个统一的解析器 `ParseOutput(output string, contract OutputContract) (token string, ok bool)` 替代三个独立的解析器。

**Phase B — 输出验证层（~250 行）**:

1. 在 `prompt.go:Build` 中，当注入 agent 卡时，**同时在 prompt 开头注入**machine-readable 的输出合同（agent 卡中写 human-readable 格式说明，输出合同写 machine-readable 的 schema）
2. 在 agent 输出后，**先验证契约遵守情况**，再解析 token。不匹配→写入 trace event（`kind:"output_contract_violation"`）+ 根据 `phase.on_contract_violation`（workflow 配置）决定 degrade（proceed）、retry（重发 prompt 明确格式）、或 fail（阻断）

**Phase C — Agent 卡与解析器的双源一致性验证（~150 行）**:

- `forge validate --contracts`：扫描所有 agent 卡，检查卡中描述的裁决格式是否与 `phase.output_contract` 匹配
- 如果不匹配→ `forge doctor` 报告「agent card and output contract out of sync」

### 诚实边界

- **不做手写 prompt 的自动化验证**：输出合同只覆盖 agent 输出的**最后一行**。prompt 的其他部分和 agent 的其他输出仍由 agent 卡管理。合同不是 prompt 的替代品，是**交互边界的契约**。
- **不改变现有 agent 卡的 작성方式**：agent 卡仍然用自然语言描述格式。`output_contract` 是**额外的 machine-readable 声明**，不是卡片的替代。
- 向后兼容：`output_contract` 缺失时，退回到当前的三解析器行为。
- 不能防止 agent 撒谎：如果合同说输出 APPROVE 但 agent 输出了 APPROVE 并做了坏代码，合同验证不会捕获。

---

## 方向三 · Gate N/A 生命周期管理：从不复检到治理保鲜

> **优先级**: P1（治理质量） | **类别**: 治理 · 可观测性  
> **一句话**: 报告 N/A 的门禁被永远当作「没问题」从不复检，系统无法感知「工具安装好了，现在可以执法了」或「N/A 原来是有问题」。

### 代码级证据

Gate 的 tri-state（PASS/FAIL/NA）中，N/A 是没有新鲜度概念的：

```go
// gate/gate.go:111-125 — ProbeAll
func ProbeAll(root string) (statuses map[string]string, categories map[string]string, err error) {
    // … 跑一圈 accept 探测，返回每个 gate 的当前状态
    // N/A 表示「无工具可检查此项」
}
```

```go
// gates.go:198-226 — GatesGreen
func GatesGreen(required []string, statuses, categories map[string]string, lifecycle string) (green bool, proof converge.GateProof) {
    for _, name := range required {
        switch resolve(name, statuses, categories, lifecycle) {
        case gate.StatusPass:
            // …
        case gate.StatusNA:
            // N/A → 算 green？！ 用 exemption matrix 判定
            if exempt(name, categories, lifecycle) {
                continue  // 被豁免 → 算 green
            }
            // 不被豁免 → 仍算 green？！
            // 看 lifecycle: production 下 N/A 是 FAIL，非 production 下是 N/A→算 green
        }
    }
}
```

关键路径：`exempt` 判断一个 N/A 是否能被生命周期豁免矩阵放过。

```go
// gate/resolve.go:36-58 — exemptsNoTool
func exemptsNoTool(gate, lifecycle string) bool {
    // lifecycle mvp 使用豁免率: 某些 gate 在 mvp 下就是 N/A
    // 但一旦判定为 N/A → 永远 N/A → 直到下次 forge run 重新 ProbeAll
}
```

**问题不在一次运行中，而在跨运行之间**：

1. **N/A 没有 TTL**：今天 `coverage` gate 报告 N/A（`go test -cover` 没权限）。下个月 DevOp 装好了测试工具。forge 不会自动重新探测 coverage gate → 仍然 N/A → 在 enforcement matrix 中被豁免 → **代码覆盖率仍然没有被执法**，没有人知道。

2. **N/A 与环境漂移不可区分**：今天 `security` gate 报告 N/A（secret-scan 工具没装）。一个月后装了但出 bug 了——secret-scan 安装正确但扫描失败 → security 的 N/A 变成了 FAIL。但 operator 无法区分「N/A→FAIL 因为工具出了问题」和「N/A→FAIL 因为有新 secret」。现有 `forge doctor` 不报告 gate 状态趋势。

3. **没有 N/A 审计报告**：没有命令或 trace 事件记录「哪些 gate 长期 N/A」。「长期 N/A 的数量」是治理质量的指标——越多 gate 长期 N/A，治理越表面化。但系统不暴露这个数字。

4. **exemption matrix 不持久化**：每次 `forge run` 重新计算豁免。如果一次运行中 `coverage gate` 被豁免（因为 mvp lifecycle），这次豁免不会写进任何持久化记录。下次 `forge doctor` 看不到「coverage was waived in sprint-7 run-3」。

### 为什么需要

| 维度 | 分析 |
|---|---|
| **治理质量** | N/A 是诚实信号——「我无法检查这个」。但如果不设 TTL、不追踪趋势、不复检，「诚实」退化为「永远忽略」。 |
| **采纳障碍** | 新用户 `forge init` 后可能大量 gate N/A（无 coverage/lint/security 工具）。用户不知道「哪些 N/A 是正常的」vs「哪些 N/A 是应该装工具的」。 |
| **升级路径** | `forge migrate --to engineering` 升级治理严格度，但**不知道当前有多少 gate 是 N/A**——这是迁移的风险输入（迁移后 N/A 可能变成 FAIL）。 |
| **Dogfood 自身** | ForgeOS 对自己的治理要求应该包含「N/A 门禁数量 ≤ 0」——如果系统允许自己的 lint gate 长期 N/A 也不觉得有问题，那对外部项目的治理承诺就不可信。 |

### 边界场景

- 干净的 N/A：「这个项目用 Rust，没有 Go coverage 工具」→ 永远的 N/A。正确的 N/A 不应该报警。
- 可控的 N/A：「项目中 eslint 没装」→ 装了 eslint 后应该自动从 N/A 到 PASS/FAIL。这是 N/A 治理的核心场景。
- 模糊的 N/A：「安全扫描需要 API key，当前环境没有」→ 永久 N/A 除非有 API key。这是配置问题，不是工具安装问题，需要不同的处理。
- **N/A 数量是动态的**：新 gate（compile gate、api-contract gate）加入后，所有项目初始状态为 N/A。不应该在未配置时就告警。

### 建议方向

**Phase A — N/A 观测性（~200 行）**:

1. 在 `forge status` 的 governance 报告中增加 N/A 保鲜度行：
   ```
   Gate N/A freshness:
     coverage    N/A since 2026-06-01 (41 days)  ← 永久的 N/A 要标注时长
     lint        N/A since 2026-07-10 (1 day)
     security    N/A since unknown (first run)
   ```

2. `converge.Signals` 增加 `NAGatesAge` 字段：最长 N/A 持续天数。在 converge 报告中作为**非阻断预警**输出。

3. 在 `forge doctor` 中增加长期 N/A 警告：`[WARN] coverage gate has been N/A for 41 days — consider configuring a coverage tool`

**Phase B — N/A 自动复检（~350 行）**:

1. 为 `ProbeAll` 增加判断：如果上次运行某 gate 是 N/A 且存活了 N 天（configurable via `project.yml: max_na_days: 30`），在 run 开始时**自动生成一个「Install gate tooling」跟踪任务**追加到 ROADMAP（使用 `forge migrate` 的补债任务模式）

2. 在 `mode.Policy` 中增加 `NAAgeWarn` 和 `NAAgeBlock` 阈值：
   - `engineering` mode：`na_age_warn: 14 days, na_age_block: 60 days`
   - `explorer` mode：`na_age_warn: 60 days`

**Phase C — N/A 豁免持久化（~150 行）**:

- 将每次 `forge run` 的豁免决策记录到 trace event（`kind:"gate_waiver"`），附带 duration（该 gate N/A 了多久）
- 增加 `forge status --na-history`：显示每个 gate 的 N/A 趋势曲线

### 诚实边界

- N/A 的自动复检**只建议、不强制**。`max_na_days` 默认 0 = 永不自动复检（向后兼容）。
- 「long N/A」不意味着「工具应该装」。有些 gate 的语言/框架就是不适用的。必须支持白名单：`project.yml: na_permanent: [coverage, security]`。
- 不改变现有 gate 的 PASS/FAIL/NA 语义。N/A 仍然是 N/A，只是加上了保鲜度标签。

---

## 方向四 · 工作流模块化：从单体 YAML 到可组合工作流片段

> **优先级**: P1（架构可扩展性） | **类别**: 可组合性 · 可复用性  
> **一句话**: 系统的编排能力在单工作流内已极成熟，但无法复用、组合、装配工作流——跨工作流的代码全是复制粘贴。

### 代码级证据

ForgeOS 的工作流是**完全扁平的**——每个工作流是一个独立的 YAML 文件，无任何组合机制：

```yaml
# .agent/workflows/build.yml — 一个独立的、不可分割的文件
phases:
  - name: planner
  - name: implementer-1
  - name: implementer-2
  - name: harness-gates
  - name: reviewer
  - name: fix
  - name: qa
stop:
  type: conjunction
  all_of:
    - metric: roadmap_completion
```

```yaml
# .agent/workflows/evolve.yml — 几乎与 build.yml 相同的结构但手动复制
phases:
  - name: planner
  - name: implementer-1
  # … 几乎相同，但少了 qa、加了 scan/gap
```

**跨工作流重用 = 手动复制粘贴**：

```bash
# 两个 workflow 共享的大部分 phase 定义是冗余的
# build.yml 的 planner 和 evolve.yml 的 planner 几乎相同
# 无 import/ref/include 机制
```

**next_stage 存在但消费有限**：

```go
// converge.go:100-109 — Converge 中的 human_gate 分支
if IsHumanGate(stop) {
    return humanGate(sig)  // 只消费 HumanApproved 信号
}
// converge.go 不消费者 stop.OnApproved.NextStage
```

```go
// loop.go:180-200 — nextStartPhase
func (l LoopEngine) nextStartPhase(wf asset.Workflow) int {
    ou := l.Stop.OnUnmet
    // 只处理 loop_to_next_roadmap_item
    // 不处理 next_stage（跳转到另一个 workflow）
}
```

```go
// asset.go:130-150 — StopCondition
type StopCondition struct {
    Type       string               `json:"type"`
    AllOf      []Criterion          `json:"all_of"`
    OnApproved *OnApprovedAction    `json:"on_approved,omitempty"`  // 声明了但仅 human_gate 消费
    OnUnmet    *OnUnmetAction       `json:"on_unmet,omitempty"`     // 声明了但仅 loop_to_next_roadmap_item 消费
}
// OnApproved.NextStage 字段存在，但没有任何代码：
// 1. 在工作流 A 收敛后自动发现并加载工作流 B
// 2. 将 B 的 phase 追加到 A 之后执行
// 3. 跨工作流传递 signals（roadmap_completion）
```

**没有「子工作流」概念**：

```go
// asset.go:Phase 结构体没有 SubWorkflow / Include / Template 字段
// 没有「从另一个 YAML 文件导入 phase 列表」的能力
```

### 为什么需要

| 维度 | 分析 |
|---|---|
| **治理一致性** | build.yml 和 evolve.yml 在手动维护，容易 drift。一个 worklfow 的改进（新 phase、新 gate、新 on_fail）不会自动传播到另一个。 |
| **模板化** | 未来加入 `deploy.yml`、`security.yml`、`compliance.yml`，每个从 build.yml 复制粘贴 → 维护成本线性增长。 |
| **第三方扩展** | `forge-init` 给新项目的治理继承是**复制文件**。如果有模块化工具体系，第三方 can 提供 `forge-workflow-security.yml` 作为「安全审计工作流片段」被任何项目 import。 |
| **架构清晰性** | 一个 10+ phase 的工作流已经很难阅读。`import phases from "security-scan.yml"` 比 30 行 YAML 更清晰。 |

### 边界场景

- 循环 import：A import B，B import A → 无限递归。静态检测必须在加载时拒绝。
- 参数化 override：A import B，想 override B 中某个 phase 的 `model_tier`。需要类似 `extends` + `override` 的机制。
- 条件化 import：production lifecycle 下 import `security-audit.yml`，mvp 下不 import。
- 导入后 phase 重命名：两个不同的 import 不能引入同名 phase。需要 namespacing 或自动前缀。

### 建议方向

**Phase A — `include` 指令（~300 行）**:

在 workflow YAML 中增加 `include`：

```yaml
# .agent/workflows/ci.yml
include:
  - security-scan.yml    # 从同一目录 include
  - ./shared/test-gate.yml

phases:
  - name: planner
  # … 自己的 phase
```

实现：
1. `loadWorkflow` 解析 YAML 前，递归处理 `include` 字段：找到被引用的文件，加载其 phases，合并进来
2. 循环检测：维护已加载路径的 set，发现循环→return error（不静默跳过）
3. 直接合并不做 namespace：include 的 phases 与本文件 phases 合并成一个扁平数组，保持声明顺序

**Phase B — `next_stage` 消费（~200 行）**:

1. 在 `LoopEngine`（或新的 `WorkflowChain`）中消费 `stop.on_approved.next_stage`：
   - 当 workflow A 收敛后，检查是否声明了 `next_stage`（如 `build → deploy`）
   - 自动加载 `deploy.yml`，将其 phases 作为「后续阶段」追加到当前 LoopEngine 的迭代循环中
   - 在 convergence 报告中显示 `converged: build → next_stage deploy (loaded)`

2. `on_rejected` 同理：如果 security-review 被 reject，跳到 `remediate.yml`

**Phase C — 参数化工作流片段（v2 方向）**:

```yaml
# .agent/workflows/security-scan.yml — 可参数化片段
parameters:
  severity: critical  # 默认值
phases:
  - name: security-{{parameters.severity}}
    agent: security-engineer
```

### 诚实边界

- `include` 是**编译时合并**不是运行时动态导入。所有 phase 在工作流加载时已确定，不做运行时动态路由。
- 不改变 `asset.LoadWorkflowJSON` 的返回值结构——合并后仍是扁平 `[]Phase`，所有下游代码无感知。
- `next_stage` 消费是**可选的、声明式的**。不声明时，现有行为逐字节不变。
- 参数化是 v2 方向。Phase A+B 纯做静态组合，不做动态模板化。

---

## 方向五 · 元学习闭环：从「学什么」到「学怎么学得更好」

> **优先级**: P2（长期杠杆） | **类别**: 适应性 · 持续改进  
> **一句话**: 系统在「学什么任务该做」（ROADMAP + memory），但不学「怎么让做任务的 agent 更高效」——agent 卡是静态的。

### 代码级证据

ForgeOS 的学习闭环覆盖了**任务层**和**控制层**，但不覆盖**行为层**：

**已经存在的学习机制**：

```go
// memory.go — 学「发现了什么 gap、做了什么决策」
// Append/Load/Query — 跨迭代知识传递

// converge/roadmap.go — 学「ROADMAP 进展到哪了」
// RoadmapCompletion — checklist 完成度

// scorecard.go — 学「哪个 model 对哪个任务类型更好」
// HistoryTiebreak — 按 (model, task_type) 聚合质量排名

// routing/budget.go — 学「成本阈值下怎么调整 tier」
// BudgetAdjustTier — 根据 spend ratio 降级
```

**不存在但应有的学习机制**：

**1. Agent 卡是纯静态的**：

```go
// .agent/agents/implementer.md — 手工编写，从不自动修改
// 内容：角色定义、工程约束、输出格式
// 系统从不根据历史表现优化这个 prompt
```

没有代码读取并反思 agent 卡的质量。所有 agent 卡在 `prompt.go` 中被注入，但从无反馈回路。

**2. 没有「phase 效果归因」**：

scorecard 能回答「model X 在 task type Y 上的 avg cost + p95 latency」。但 scorecard 不回答「就 agent prompt A 而言，approve 率是多少？REQUEST_CHANGES 的典型原因是什么？」。

```go
// scoring/scoring.go — Score 函数
func Score(dims map[string]float64, weights map[string]float64) float64 {
    // 做复杂度/风险/上下文/依赖/安全的评分
    // 不包含「历史 approve 率」或「card quality」维度
}
```

**3. 没有 prompt 效果追踪**：

```go
// prompt.go:Build — Build prompt
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    // 拼接角色卡 + context
    // card 是静态的，ctx 是动态的
    // 不记录「每次输出用的 prompt 的 hash」
    // 不能回答「在这个 prompt hash 下，输出质量如何？」
}
```

**4. 没有 auto-debug 机制**：

当一个 implementer phase 连续 3 次被 reviewer 要求修改（REQUEST_CHANGES），系统**什么都不做**——继续跑第 4 次相同的 prompt、相同的 agent card、期望不同结果。没有「发现问题→调整 prompt→重试」的循环。

```go
// loop.go:135-180 — loopBackTo
func (e Engine) loopBackTo(wf asset.Workflow, p asset.Phase, loopBacks *int, reason string) (target int, jumped bool) {
    // 跳回 target phase
    // 但不改变 prompt 内容——agent 收到和上次完全相同的 instruction
    // 期望 LLM 这次能做得更好（非确定性）
}
```

### 为什么需要

| 维度 | 分析 |
|---|---|
| **AI-SDLC 的终极闭环** | Stage 6（Evolve）在 ForgeOS 中 = scan → gap → roadmap → implement → review → evaluate。但「implement」的 prompt 是固定的，系统不自己改进 prompt。真正的 AI-SDLC Stage 7 应该是「改进改进者本身」。 |
| **减少人工调优** | agent 卡目前是最需要人工调整的部分。项目初期 prompt 可能写得不好（agent 行为不符合预期），operator 需要手动编辑 card 文件。元学习应该自动建议/执行 prompt 改进。 |
| **长期成本优化** | 一个更精准的 implementer prompt = 更少 REQUEST_CHANGES = 更少循环 = 更少 token 消耗。边际收益随迭代增加。 |
| **差异化** | 大多数「AI 软件工厂」产品只做编排+记忆。能做「元学习——自改进 agent prompt」的产品是下一代差异化竞争点。 |

### 边界场景

- prompt 优化可能使 agent 行为变得更严格（更多 REJECT）或更松散（更多 APPROVE）。需要**安全护栏**确保优化方向是收敛的，不是发散的。
- **不要自动修改 agent 卡**。元学习的输出应该是**建议**（PR on agent card），不是自动应用。human-in-the-loop 保持。
- 归因困难：「这次 reviewer 发了 REQUEST_CHANGES，是因为 implementer 的 prompt 写得太模糊，还是因为 task 太复杂？」元学习系统需要足够多的样本才能归因。
- 不能优化到「agent 卡只包含 'approve everything'」——这是 reward hacking。元学习的评估指标必须多维度（quality + cost + safety）。

### 建议方向

**Phase A — 可观测性（~300 行）**:

1. 对每次 agent phase，记录 **prompt hash**（用 `sha256` 对完整 prompt 做 hash）到 trace event 的 `Detail` 字段
2. scorecard schema 扩展：`prompt_hash string`, `approve_rate float`, `avg_cost_by_prompt map[string]float64`
3. 增加 `forge scorecard --by-prompt-hash`：按 prompt hash 聚合 approve rate、cost、latency
4. 增加 `forge route --prompt-diff`：比较两个 prompt hash 对应的 scorecard 指标，判断哪个 prompt 更优

**Phase B — 信号采集（~200 行）**:

1. 在 loop-back 循环中，当 `agentOutcome` 返回 REQUEST_CHANGES，记录**被驳回的 phase 的 prompt hash** 到 memory（`kind: "lesson"`, `topic: "prompt_quality"`）
2. 在 scorecard 的 `HistoryTiebreak` 中，增加 prompt hash 作为辅助维度（主-key 仍是 model, task_type，但同 model+task 下比较 prompt hash 的 approve rate）

**Phase C — 元学习建议（v2/P3 方向）**:

- 当同一 prompt hash 的 REQUEST_CHANGES 率超过阈值（如 > 40% over 5 runs），自动生成**一个 ROADMAP item**：`Review and improve [agent] card — high REQUEST_CHANGES rate (N%)`
- 建议包括：从 reviewer 的 Findings 中提取高频反馈词（如「missing error handling」「no tests」），汇总为卡片改进方向

### 诚实边界

- **永远不自动修改 agent 卡**。元学习的输出是 ROADMAP item（operator 可审阅后决定是否修改）或自然语言建议。human-in-the-loop 不可绕过的设计原则。
- Prompt hash 是内容敏感的：即使增加一个空格，hash 也会完全不同。需要 fuzzy matching 或归一化（去除空白/注释）才能做有意义的聚合。
- 「approve rate」是 proxy metric，不是 ground truth。approve rate 高不意味着质量好——一个偏好 approve 的 reviewer 可能让坏代码通过，被 test gate 在后面捕获。
- 元学习的冷启动问题：前 N 次运行（N ≈ 10-20）样本太少，无法做统计有效的归因。建议在此前不显示元学习指标。

---

## 优先级与收敛执行建议

| # | 方向 | 优先级 | 投入 | 收益类型 | 一句话杠杆 |
|---|------|--------|------|---------|-----------|
| 1 | 多实例隔离 | **P0** | Phase A ~200 行 | 运维安全 · 数据完整性 | 两个 forge 同时跑=静默损坏；检测锁是零成本的保险 |
| 2 | Agent 输出契约 | **P1** | Phase A ~400 行 | 可靠性 · 可演化性 | 三个脆弱解析器=信号神经断裂的风险；统一合同降低未来断裂概率 |
| 3 | Gate N/A 保鲜 | P1 | Phase A ~200 行 | 治理质量 · 可观测性 | 长期 N/A=治理幻觉；保鲜度标签让「诚实」不退化到「忽略」 |
| 4 | 工作流模块化 | P1 | Phase A ~300 行 | 架构可扩展性 · 可复用性 | 跨工作流复制粘贴=治理 drift；include 是模块化的原子步 |
| 5 | 元学习闭环 | **P2** | Phase A ~300 行 | 长期差异化 · 成本优化 | agent 卡静态=人工调优瓶颈；prompt hash 追踪是迈出自改进的第一步 |

**收敛执行顺序**：

```
Sprint N:   方向一 Phase A（锁检测） + 方向三 Phase A（N/A 保鲜报告）——两项可观测性增强，无运行时破坏
Sprint N+1: 方向二 Phase A（输出契约声明 + 统一解析器）——替换三个独立解析器，改进可测试性
Sprint N+2: 方向四 Phase A（include 指令）——工作流组合能力的第一步
Sprint N+3: 方向五 Phase A（prompt hash 追踪 + scorecard 扩展）——元学习的观测基础
Sprint N+4: 方向一 Phase B（隔离子目录） + 方向三 Phase B（自动复检）——主动防护 + 治理修复
```

**明确排除（反镀金）**：

- 不做跨主机的分布式锁（多实例隔离只做同一主机的 `flock`）
- 不做 agent 输出 schema 的 JSON Schema 验证（输出合同目前只覆盖最后一行 token）
- 不做 N/A 的自动工具安装（只检测和报告，不自动部署工具）
- 不做 workflow 运行时的动态 import（include 是编译时合并，不是运行时插件）
- 不做自动 agent 卡修改（元学习只产出 ROADMAP item，永远 human-in-the-loop）
