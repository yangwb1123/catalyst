# ForgeOS — 第 16 轮扩展方向：未覆盖的结构性盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 15+ Go 包 / cmd/forge 18+ CLI / harness 26+ 模块 /  
>   `.agent/` 完整治理骨架 / examples/ / pi-batch.py / 全部 30+ 份已有 docs/analysis/ 交叉核对）  
> **基线**: Sprint 27 全状态（真点火正交验证完成、Adaptive Assembly/Reflect 已落地、  
>   parallel 完整交付含锁顺序契约、multi-candidate HistoryTiebreak v1.5 上线）  
> **纪律**: **绝不与任何已有分析文档的核心论点重叠**。每方向标注「已有覆盖」以证明新颖性。  
>   不写代码。仅列 5 个方向。  
> **日期**: 2026-07-01

---

## 已有 30+ 份分析已覆盖的域（本文不再重复）

全部已有覆盖域见 `fresh-perspectives-v14-five-novel-extensions.md` §已有分析覆盖域速查，  
另加：跨周期收敛状态机 · 配置面完整性守卫 · SCA 运行时 · Phase 级文件系统隔离 ·  
预算规划器 · 交互式工作流编排 · 检查点 Diff 浏览器 · Agent 输出 Schema 执法 ·  
策略模拟引擎 · 自适应 Assembly(已落地) · Reflect 自分析(已落地) ·  
冷启动分数卡(已落地) · 信号处理/优雅关闭 · 跨相位故障归因 · 执行器多样性 ·  
ForgeOS 自身治理差距 · 增长瓶颈/包膨胀 · 边界情况/性能（edgecases-and-perf.md） ·  
第五波运维分析（fifth-wave-operational.md） · 第七波数据现实（seventh-wave-data-realism.md） ·  
多模态第六波（sixth-wave-multimodel.md）

---

## 本文 5 个方向

以下方向均从**代码级微观证据**出发，与已有 30+ 份分析交叉确认无重叠。每个方向回答：
**「为什么这个盲区如此重要以至于必须单独成篇？」**

---

## 方向一：多实例并发安全——.forge 目录的竞态窗口（操作安全）

### 类型
操作安全 · 运维 · 数据完整性  
**紧急度: P0（数据损坏风险）**  
代码影响: `internal/persist/` · `internal/trace/` · `internal/memory/` · `cmd/forge/evolve.go`

### 已有覆盖
- `edgecases-and-perf.md` §2.2 提到 memory.jsonl 并发 append 风险  
- `sixth-wave-multimodel.md` 方向五 提到跨进程共享  
- **但无任何文档系统性地分析整个 `.forge/` 目录的跨进程并发安全**

### 代码级证据

`.forge/` 目录持有三个运行时状态文件，**全部在无跨进程锁的情况下被读写**：

**① checkpoint.json — 写写竞争（`internal/persist/save.go`）**

```go
func Save(path string, cp Checkpoint, retain int) error {
    // ...
    tmp := path + ".tmp"
    writeSynced(tmp, data)   // 写入 tmp
    os.Rename(tmp, path)     // 原子提交
}
```

两个进程同时 `forge evolve` → 同时 `Save`：
- 进程 A 写入 `checkpoint.json.tmp` → 刚写完，`Rename` 前被调度走
- 进程 B 写入 `checkpoint.json.tmp`（覆盖 A 的 tmp）
- 进程 A 恢复运行 → `Rename` 将 B 的 tmp 重命名为 checkpoint.json  
  → **A 的 checkpoint 数据永久丢失**

`rotateRetain` 也非原子：
```go
os.Rename(path, path+".1")  // 两个进程同时做，互相覆盖
```

**② memory.jsonl — 无保护的 O_APPEND（`internal/memory/memory.go`）**

```go
func Append(path string, e Entry) error {
    f, _ := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    f.Write(line)  // 两个进程同时写 → JSON 行可能交错: `{"kind":"gap"...}\n{"kind":"decision"...}\n`
    f.Close()
}
```

POSIX 保证 <= PIPE_BUF（通常 4096 字节）的 write(2) 在 O_APPEND 下原子。  
一条 JSON 行约 200-500 字节，大多数情况下安全。但：
- 行 > 4KB（极少见但可能：长 `Detail` 字段） → write 分裂 → **两进程的行交织成一个非法 JSON 对象**
- `invalidateLoadCache()` 只清本进程的 cache，对另一个进程无影响 → **一个进程看不见另一个刚写的条目**

**③ trace.jsonl — 同 memory 一样的 O_APPEND 问题**

`internal/trace/trace.go`:
```go
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()      // 保护本进程内的 goroutine 并发
    // ...
    t.w.Write(line)  // 但不保护跨进程的 write(2) 交织
    t.mu.Unlock()
}
```

**④ Load 的 mtime 缓存假设独占性**

`memory.go` 的 `loadCache` 用 mtime 做缓存有效判断：
```go
func loadFromCache(path string) ([]Entry, bool, error) {
    // ...
    if !st.ModTime().Equal(ce.modTime) {
        return nil, false, nil   // mtime 变了，缓存失效
    }
}
```

进程 A 写入了新条目（`Append` 修改了文件 mtime）。  
进程 B 在下一次 `Load` 时缓存失效，重新读取全文件——正确。  
但**如果进程 B 的 `Load` 与进程 A 的 `Append` 同时发生**：  
B 读到部分写入的行（A 的 write(2) 只写了前一半）→ `json.Unmarshal` 失败 → **返回硬错误**

```go
func Load(path string) ([]Entry, error) {
    data, err := os.ReadFile(path)  // 读到 A 未写完的 content
    // ...
    entries, err := decode(data)    // 解析失败 → 返回 err
    return nil, err                 // 进程 B 的行为完全被 A 的写入时序破坏
}
```

### 为什么需要它

这是**操作安全的基本问题**。ForgeOS 当前假设「一个 repo 一次只有一个 forge 进程」——  
这在以下场景中全部被违反：
- **CI runner 并发**：同一个 repo 被两个 CI job 同时 checkout，各自执行 `forge evolve`
- **开发者 + CI 同时跑**：开发者本地跑 `forge run`，CI 也在跑
- **无人值守 + 手动干预**：24h evolve 运行时运维人员手动 `forge status` 或 `forge approve`

修复方向不复杂（`flock` / 文件锁 / `.forge/.lock`），但**缺失意味着数据损坏不是「可能」而是「何时」**。

### 边界情况
- 跨容器文件系统（NFS、FUSE）下 `flock(2)` 可能不支持 → 需 fallback 机制
- 死锁：进程被 SIGKILL 时持有的文件锁不会自动释放（POSIX 规定如此）→ 需要 `O_CLOEXEC` + 心跳超时释放
- `forge doctor --anomaly` 如果读到被并发损坏的 checkpoint，可能误报

---

## 方向二：Agent 输出 Schema 强制——从「自由文本约定」到「结构化合约」

### 类型
架构 · 治理 · 可靠性  
**紧急度: P1（随 agent 种类增长风险递增）**  
代码影响: `cmd/forge/cost.go`（契约解析） · `internal/orchestrator/` · 工作流 YAML schema · 全部 agent 卡

### 已有覆盖
- `novel-extensions-v12.md` 方向二「Agent 输出 Schema 执法」提出概念  
  — **但未分析当前代码库的「已经部署的自由文本契约」具体有多少、各自脆弱性如何**  
- `expansion-directions-v6.md` 方向一「跨 Agent Prompt 注入防护」关注输入安全  
  — **不覆盖输出端 schema 强制**

### 代码级证据

目前系统中有**至少 5 个不同位置的自由文本契约**，全部通过字符串模式匹配（最脆弱的机制）提取语义信号：

**① Reviewer VERDICT（关键裁决，P0）**

`cost.go`:
```go
const VerdictApprove        = "APPROVE"
const VerdictRequestChanges = "REQUEST_CHANGES"

func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:
        return VerdictApprove, true
    case "VERDICT: " + VerdictRequestChanges:
        return VerdictRequestChanges, true
    default:
        return "", false
    }
}
```

依赖条件：「reviewer 的**最后一行非空行**恰好匹配预设格式」。  
- agent 在多轮对话后在末尾加了额外行 → 格式被"埋没"
- agent 被 prompt 注入说服输出 `VERDICT: APPROVE` 但实际内容有问题 → 无法检测
- 输出格式微调（如 `VERDICT:REQUEST_CHANGES` 少了空格）→ `ok=false` → fail-open（proceed），**关键裁决无声降级**

**② Claude cost JSON 解析（核心计费，P1）**

`cost.go`:
```go
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    var env struct {
        TotalCostUsd *float64 `json:"total_cost_usd"`
    }
    if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
        return 0, false
    }
```

依赖条件：claude 的 `--output-format json` 输出是**单行完整 JSON 对象**。  
- 如果 agent 输出了多行（claude 自己加了 prose），`json.Unmarshal` 失败 → cost 丢失
- trace 中的 `cost_usd_micros` 恒 0 → telemetry 层无数据 → Learning loop 收不到反馈
- **实际发生过**（Sprint 26 日志）：claude JSON 解析不稳定，需要 `unwrapClaudeResult` 预处理

**③ RoadmapCompletion（收敛信号，P1）**

`internal/converge/converge.go`:
```go
func RoadmapCompletion(markdown string) float64 {
    for _, line := range strings.Split(markdown, "\n") {
        switch t := strings.TrimSpace(line); {
        case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
            done++
            total++
```

依赖条件：ROADMAP.md 的 checklist 格式 `- [ ]` / `- [x]` 严格一致。  
- agent 用了不同的 checklist 格式（`* [ ]` 或 `- [X] ` 多了一个空格）→ 条目不被计数
- checklist 被写到列表嵌套中（`- 子项 - [x] done`）→ 格式解析漏掉
- **收敛决策基于一个可能不准确的百分比**

**④ Memory confidence / supersedes（跨 session 知识，P1）**

`internal/memory/memory.go`:
```go
type Entry struct {
    Kind       string  `json:"kind"`
    Topic      string  `json:"topic"`
    Confidence float64 `json:"confidence,omitempty"`
    Supersedes string  `json:"supersedes,omitempty"`
}
```

Entry 有结构化的 JSON schema——这是**好的例子**。但 `Kind` 只有三个固定值  
（`"gap"` / `"decision"` / `"lesson"`），无扩展点且无验证。  
- 代码里无处验证一个 Entry 的 Kind 必须是这三者之一（decode 后直接接受）
- `Confidence` 由调用方自由填写，无标准差、无校准——高低置信度的含义因 agent 而异

**⑤ Agent card 的 FINISH → VERDICT 隐含约定（agent 卡中声明但机器不执行）**

`.agent/agents/reviewer.md` 声明 reviewer 应在末尾输出 `VERDICT: …`。  
但**没有任何代码在 agent 卡的 prompt + 输出格式之间建立机器可读的契约**。  
- 写新的 agent 卡（security-engineer / distributed-engineer）只用文字约定输出格式  
- 新 agent 的输出没有 schema 验证 → 解析层需要为每个 agent 单独写字符串匹配代码

### 为什么需要它

当前系统有 **5 个关键信号全部依赖自由文本的模式匹配**。每增加一种 agent 角色，  
就增加一个「agent 输出了正确的格式吗？」的运维焦虑。随着系统扩展到 15+ agent 角色  
（当前 9 张卡，Sprint 12 审计标记的 `distributed-engineer` / `performance-engineer`  
等尚在路线图上），自由文本契约的可维护性**线性退化**。

一个简单的输出 schema 验证层（Agent Output Contract）可以：
- 定义每类 agent 输出必须包含的 JSON 字段（verdict, confidence, rationale, findings[]）
- 在 `Observe` sink 中自动验证 agent 输出是否符合 schema  
- 不符合 schema → 重试 / 降级 / 报告，**而不是 fail-open 地忽略**

---

## 方向三：Phase 原子工作区——从「直接写工作树」到「阶段性隔离提交」

### 类型
架构 · 可靠性 · 安全  
**紧急度: P1（影响生产可用性）**  
代码影响: `internal/orchestrator/` · `cmd/forge/` · 新增 phase-workspace 包

### 已有覆盖
- `novel-directions-v13.md` 方向一「Phase 级文件系统隔离」提出了类似概念  
  — **但该方向聚焦安全隔离（容器/jail），而非事务性回滚完整性**  
- `edgecases-and-perf.md` §1.1 提到并发相位互相影响  
  — **只有一句话，未深入分析文件系统状态污染**  
- `expansion-directions-v4.md` 方向四「确定性 Replay 引擎」  
  — **关注 replay 而非原子性**

### 代码级证据

**① 相位在无隔离的工作目录内运行**

`engine_build.go`:
```go
ex := orchestrator.CommandExecutor{
    Build: func(p asset.Phase, mode string) []string {
        // ... 构建 claude -p "<prompt>"
    },
    Dir: o.root,  // 直接指向 repo 根目录！
}
```

每个 agent 直接在 repo 工作树内写文件（`claude --permission-mode acceptEdits`）。  
如果一个相位中途失败（超时、529、budget 烧穿），工作树上的文件状态是**部分完成**的：

```
# 一个 implementer phase：
  - 已创建：src/payment/new-flow.go        ← 完成
  - 已修改：src/payment/handler.go           ← 完成
  - 已修改：src/payment/types.go             ← 写到一半也被 SIGKILL 了！
  - 已创建：test/payment/new-flow_test.go     ← agent 没来得及写
```

→ **工作树留下了一个不一致的「半实现」**。当前重试或下一个相位看到的是一个无法编译的代码库。

**② 无相位级快照/回滚**

当 loop-back 从 gate FAIL 跳回 implementer 时：
```go
func (e Engine) loopBackTo(...) {
    // 跳回 implementer，但上一个 implementer run 留下的文件还在
    // 重试的 agent 看到的是「被改过但改错了」的文件状态
}
```

新的 implementer 跑在一个**已经被污染的工作树**上。它可能：
- 尝试修正但被旧改动的残余错误信息误导
- 直接忽略（gate 还是失败——浪费一次 loop-back budget）

**③ 并发相位（parallel 模式）的文件冲突**

`parallel.go`:
```go
go func(i int) {
    defer wg.Done()
    e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls)
}(idx)
```

两个并发的 implementer 可能：
- 修改同一个文件（git merge 冲突）
- 创建同名文件（一个覆盖另一个）
- 都尝试添加同一 import（重复 import）

当前**完全没有防护**，因为：
- 工作是串行设计的（asset.Phase 没有 `depends_on` 用于文件级别依赖）
- 文件系统没有按相位隔离

**④ 相位产物影响 git diff（跨轮污染）**

`risk_diff.go` 的 `FromChangedPaths` 读 git diff：
```go
func FromChangedPaths(paths []string) (Signals, []string) {
    for _, p := range paths {
        // ...
    }
}
```

如果一个相位修改了文件但未收敛，这些修改进入 git diff → 影响**下一个 evolve 迭代**的
risk auto-detection → **前一个失败相位的脏数据影响后一个迭代的模型路由决策**。

### 为什么需要它

当前 ForgeOS 在**文件系统层没有事务性**。相位失败 → 工作树被污染 → 重试不干净 →  
浪费 budget（gate 重复失败）。这是一个让 24h 无人值守不敢在真·生产项目中运行的薄弱环节。

简单方案（v1）：
- 每个相位在工作前创建一个 git stash / 快照
- 成功 → 丢弃 stash
- 失败 / loop-back → `git stash pop` 完全恢复

复杂方案（v2）：
- OverlayFS 或 tmpfs 工作区：相位写 overlay，成功才 commit 到工作树
- `Phase.OutputFiles` 声明式列出相位产出文件（已有 `Phase.Emits` 字段雏形）

---

## 方向四：工具相位——非 LLM 阶段的声明式执行

### 类型
功能 · 编排模型  
**紧急度: P2（高杠杆但非安全关键）**  
代码影响: `internal/asset/`（Phase 类型） · `internal/orchestrator/` · workflow YAML schema

### 已有覆盖
- `novel-directions-v13.md` 方向四「执行器多样性」提出了多执行器的概念  
  — **但未分析「非 LLM 相位」作为一级公民的编排需求**  
- `fresh-perspectives-v14.md` 方向三「相位级前置条件验证」  
  — **关注检查而非执行**  
- `high-value-extensions.md` 方向三「增量式治理执行」  
  — **关注 git-diff 级别的 gate 优化，不是非 LLM 相位**

### 代码级证据

**① 每个相位强制要求 agent card**

`prompt_context.go`:
```go
func readCard(repoRoot, agent string) string {
    b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agent+".md"))
    // ...
}
```

每个相位需要一个 agent 名 → 一个 `.agent/agents/<name>.md` 卡。  
这意味着一个只跑 `gofmt`、`eslint --fix`、或 `node --test --watch` 的纯工具步骤  
也**必须占用一个 agent 卡槽**，即使它完全不涉及 LLM。

**② gate phase 是唯一的「无 LLM」相位，但它的语义固定为「闸门检查」**

```go
if len(p.RequiredGates) > 0 {
    if err := e.runGates(p, e.gatesFor(p)); err != nil {
        // gate phase: 检查→通过或失败
    }
}
```

`gate phase` 的语义是「检查、阻断、或 loop-back」。它不能用来做：
- 格式化代码（`gofmt -w`）
- 生成文档
- 编译验证
- 部署
- 数据迁移

**③ YAML workaround 需要用 implementer + 特制的 prompt 来假装工具相位**

当前如果想在 workflow 中插入一个「go mod tidy」步骤，必须：
1. 创建一个 `.agent/agents/tidy.md` agent 卡
2. 在该卡中写 prompt：「你唯一要做的是运行 `go mod tidy`」
3. 用 implementer 相位执行，消耗一次 LLM 调用（~$0.05-0.18）  
   **即使这个步骤完全不需要 LLM 判断**

这是**架构上的不诚实**——一个 deterministic 操作被迫走 LLM 路径。

**④ workflow YAML 无 executor hint**

当前 `Phase` schema（`asset.go`）无执行器类型字段：
```go
type Phase struct {
    Name          string     `json:"name"`
    Agent         string     `json:"agent"`
    // ... 无 Executor 类型字段
    // ... 无 Command 字段
    // ... 无 RunInline 字段
}
```

一个工具相位理想的工作流声明：
```yaml
phases:
  - name: format-code
    agent: gofmt          # 「gofmt」本身不是 agent 卡名，但可以被解析为「tool:gofmt」
    executor: tool        # 新增语义：非 LLM，直接运行命令
    command: "gofmt -w src/"
```

或更灵活的：
```yaml
  - name: build-check
    executor: shell            # 新增 executor type
    run: "go build ./..."
    fail_on_error: true
```

### 为什么需要它

这不是一个花哨的功能，而是一个**架构完整性问题**。当前的「万物皆 agent」模型：
1. **浪费钱**：纯工具步骤烧 LLM token
2. **减慢流程**：LLM 启动 ~2-10 秒，工具步骤应该 <500ms
3. **限制表达力**：workflow YAML 只能表达「agent 步骤」和「gate 检查」，不能表达「工具执行」
4. **污染 telemetry**：工具步骤的 cost / latency 计入 agent trace → 学习数据含噪声

加上工具相位后，ForgeOS 的编排模型从：
```
Agent Phase  ↔  Gate Phase
```
变成：
```
Agent Phase  ↔  Tool Phase  ↔  Gate Phase
```
这是一个小的 schema 扩展，但大幅拓宽了 workflow 所能表达的自动化范围。

---

## 方向五：收敛信号交叉验证——从「agent 自评」到「多源归因」

### 类型
治理 · 可靠性 · Honesty  
**紧急度: P1（长期收敛可信度）**  
代码影响: `internal/converge/` · `internal/orchestrator/loop.go` · `cmd/forge/evolve.go`  
  · 新增 cross-validation layer

### 已有覆盖
- `edgecases-and-perf.md` §3.1 门闩效应用 GatesGreen 辅助  
  — **仅覆盖一个子场景，未提出系统框架**  
- `expansion-core-five-2026-07-01.md` 方向一「跨周期收敛状态机」  
  — **关注状态机记忆趋势，而非信号来源多样性**  
- `seventh-wave-data-realism.md` §3「Realism in convergence evidence」  
  — **提出了需要现实信号但没做系统分析**  
- `loop.go` 已有 `FileDelta` 和 `CodeTestRatio` 警告日志  
  — **但这些信号仅用于日志警告，未参与收敛决策**

### 代码级证据

**① 当前收敛信号只有两个决定性的**

`converge.go` 的 `Evaluate` 从 `Signals` 结构体读取决定性信号：
```go
type Signals struct {
    RoadmapCompletion float64  // agent 自评（主观）
    GatesGreen        bool     // 客观门（硬事实）
    // ... 其余信号只用于 Detail 日志，不影响 met bool
}
```

`RoadmapCompletion` 是**agent 自评**（agent 写了 `- [x]` 就认为自己完成了）。  
`GatesGreen` 是**客观门**（ci runner 的 exit code）。

两者之间有一个巨大的信任空隙：

```
RoadmapCompletion = 100%    agent 说「我全做完了」
GatesGreen        = true    测试都过了
→ 收敛判定：MET
```
但：
- 代码可能**缺少测试**（CodeTestRatio=0 但只记录警告）
- 代码可能**实现了错误的功能**（reviewer 没发现的功能性偏差）
- 代码可能**引入了技术债**（认知负荷超标、扇入超限——arch-check 已有的检查但未纳入收敛）

**② 已有但未使用的验证信号**

代码中已收集但**仅用于日志**、不参与收敛判定的信号：

| 信号 | 收集位置 | 当前使用 | 应如何使用 |
|------|---------|---------|-----------|
| `FileDelta` | `loop.go:reportConvergence` | 仅日志警告 `⚠ honesty: roadmap=90% but file-change=20%` | **RoadmapCompletion 置信度调降因子** |
| `CodeTestRatio` | `loop.go:reportConvergence` | 仅日志警告 `⚠ test gap: code-to-test ratio=0%` | **GatesGreen 的后备校验：测试覆盖不足时标记「已退化收敛」** |
| `arch-check` 结果 | `probeArchitecture()` | 作为 `architecture` gate（绿/红） | **加入收敛信号：架构违规数超过阈值时 NOT MET** |
| `review_status` | `converge.go:114` | 已定义为信号但**未被任何 workflow stop condition 使用** | **CTO review 的审批结果应直接参与收敛判定** |
| `RequirementConfidence` | `converge.go:110` | 已定义为信号但**无数据来源**（discover workflow 未接入） | **需求探索阶段的置信度决定「是否能进入 build」** |
| `CycleTime` (wall clock) | `trace.jsonl` 可计算 | **未被任何地方读取** | **超时的 evolve 即使 MET 也应标记「已临界」** |

**③ 跨信号矛盾检测的缺失**

`loop.go:reportConvergence` 虽有 FileDelta 警告，但无正式的矛盾检测协议：

```go
// 当前：仅警告
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    logln("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%%", ...)
}

// 应做的：形式化矛盾检测
type SignalDiscrepancy struct {
    Level    string  // "warn" | "downgrade" | "block"
    Reason   string
    Affected string  // 受影响的收敛指标
}
```

缺少的规则（代码中无对应逻辑但可从数据推导）：
- **`roadmap_completion = 100% 但 file_delta < 20%`** → agent 可能在虚报（prompt injection / 幻觉）
- **`gates_green = true 但 code_test_ratio < 0.05`** → 代码质量可能在下降（退化性收敛）
- **`roadmap_completion +10%` 但 `iterations_since_last_gate_change > 3`** → 进度集中在非验证区域
- **`cost_spent > budget/2` 但 `roadmap_completion < 20%`** → 成本失控，需要调低 tier

### 为什么需要它

当前的收敛判定是**二值化的信任模型**：
- agent 说做了 → RoadmapCompletion 增加
- gate 通过了 → GatesGreen = true
- 两者都满足 → 收敛

但在真实世界中：
- agent 可能幻觉完成度（ROADMAP checklist 只是 tick，没有语义验证）
- gate 可能不完整（coverage 是 N/A、typecheck 是 N/A）
- 系统可能在「退化性收敛」——门都绿了但代码质量在下滑

一个正式的多信号交叉验证层，让 `RoadmapCompletion` 的权重被 `FileDelta` 和  
`CycleTime` 调节、让 `CodeTestRatio` 补 `GatesGreen` 之不足、让 `review_status`  
成为真正的收敛输入——**这是 ForgeOS「honesty-first」原则在收敛层的最后一次落地**。

### 边界情况
- 新项目开头：`FileDelta=0` 但 `RoadmapCompletion=0` → 不是矛盾，是冷启动
- 纯 docs 改动：`CodeTestRatio 未定义` → 跳过检查
- 多迭代累积：已迭代 10 轮的 converge 不应因 FileDelta 而否决（旧的 file changes 已被 git commit 清除）

---

## 总结：优先级与归属

| # | 方向 | 紧急度 | 类别 | 一句话杠杆 |
|---|------|--------|------|-----------|
| 1 | 多实例并发安全 | **P0** 🚨 | 操作安全 | 数据损坏风险。两个 `forge evolve` 同时跑=数据损坏「不是可能而是何时」 |
| 2 | Agent 输出 Schema 强制 | P1 | 架构 · 治理 | 5 个关键信号全凭自由文本匹配。每增一个 agent 角色，脆弱性线性增长 |
| 3 | Phase 原子工作区 | P1 | 可靠性 | 相位失败直接污染工作树。24h 无人值守不敢真用 |
| 4 | 工具相位 | P2 | 编排模型 | 「万物皆 agent」浪费钱、减慢速度、限制表达能力 |
| 5 | 收敛信号交叉验证 | P1 | 治理 · Honesty | 当前二值信任（agent 说+gate 绿）→ 容易被幻觉/N/A 绕过 |

---

*分析日期: 2026-07-01 | 第 16 轮全局扫描 | 基于 forge-core + harness + .agent 全量源码*
