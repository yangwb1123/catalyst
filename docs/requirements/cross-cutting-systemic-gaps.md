# ForgeOS — 跨层面系统性缺口：管线编排、资产演化与运行健康

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core（18+ Go 包 · cmd/forge 16+ 子命令 · ~35k LOC）·  
>    harness（39+ 模块 · ~10.5k LOC 执法层）·  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies · modes · routing）·  
>    examples/（url-shortener · go-taskd）· pi-batch.py · `.github/workflows/`  
> 2. Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 0 GAP）  
> 3. **逐篇通读 50+ 份 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md` + 其余核心文挡**（共 ~120 份），  
>    对每个候选方向在全部已有文档中做关键词检索 + 语义比对，确认**本文的 5 个方向从未作为独立扩展方向展开**  
> 4. **差异化证明**: 每个方向附代码级证据 + 与已有覆盖的精确边界区分  
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 已有 ~120 份文档的覆盖格局（本文不重复）

ForgeOS 经过 31 轮 sprint 迭代和 120+ 份分析文档的覆盖，功能域已被深度扫描：

| 已被充分覆盖的域 | 覆盖程度 | 代表性文档 |
|-----------------|---------|-----------|
| 编排引擎内核（串行/并行/loop-back/mode-gating/stop-condition） | 深度覆盖，15+ 方向 | `high-value-extension-directions.md`·各 sprint 文档 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | 深度覆盖，8+ 方向 | `seventh-wave-data-realism.md`·各 sprint 文档 |
| 记忆/学习（memory/persist/checkpoint/Supersedes/缓存） | 深度覆盖，10+ 方向 | `second-order-architectural-gaps.md` |
| 路由/调度（TierFor/多维 scorer/BudgetAdjust/HistoryTiebreak） | 深度覆盖，8+ 方向 | `novel-architectural-extensions-v40.md` |
| 安全护栏（recursion/budget/timeout/output-cap/四维） | 深度覆盖，5+ 方向 | `edgecases-and-perf.md`·`forgotten-five-foundations.md` |
| 治理/执法（arch-check 8 检查/secret-scan/check.py 10 检查） | 深度覆盖，12+ 方向 | `structural-gaps-v41.md`·`strategic-extensions-v33.md` |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | 各 sprint + `ARCHITECTURE.md` |
| 需求清单审计 | 已做，0 GAP | `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` |
| 编排运行时韧性（529/超时/退避/checkpoint/进程组）| 深度覆盖 | `edgecases-and-perf.md`·`strategic-extension-five-novel.md` |
| 真点火验证（multi-agent · converge MET · 成本三维）| 已坐实 | Sprint 24-26 + `docs/ignition.md` |
| Gate 执行经济学 / 记忆内容去重 / 墙钟预算 / forge plan | 已覆盖 | `novel-architectural-extensions-v40.md` |
| 工作流组合代数 / Provider 抽象 / 渐进式治理采纳 | 已覆盖 | `expansion-five-truly-uncovered-frontiers-v46.md` |
| 跨进程运行时守护 / 治理热加载 / Trace 查询 CLI / 可插拔扩展 | 已覆盖 | `forgotten-five-foundations.md` |
| Go 库 API 契约 / 测试质量元治理 / 混沌韧性验证 / Schema 版本化 | 已覆盖 | `structural-gaps-v41.md` |
| forge-core 二进制分发 / 状态目录健壮性 / 统一结构化输出 / 多会话协调 | 已覆盖 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` |
| 工作流黄金文件测试 / Agent 输出真实性闸门 / 并发状态隔离 | 已覆盖 | `expansion-five-truly-uncovered-frontiers-v46.md`·`five-uncovered-architectural-frontiers.md` |

---

## 本文的 5 个方向：跨层面系统性缺口

在通读全部 ~120 份已有分析后，以下缺口**从未作为独立方向被系统性展开**。它们的共同特征是：不是某个子系统的功能缺失，而是**两个或多个已有子系统之间的组合/接口/生命周期问题**——即「跨层面系统性缺口」。

每个方向都包含：
- **代码级证据**: 精确到 `file:line` 的现有代码引用
- **差异化证明**: 与已有覆盖的明确边界区分
- **实际影响评估**: 如果不管，什么场景会出问题
- **边界情况**: 实现时容易忽略的陷阱

---

## 目录

1. [方向一：跨工作流编排管线守卫——从声明到强制执行](#方向一跨工作流编排管线守卫)
2. [方向二：Agent 治理资产的版本化生命周期管理](#方向二agent-治理资产的版本化生命周期管理)
3. [方向三：外部工具链版本契约与环境钉扎](#方向三外部工具链版本契约与环境钉扎)
4. [方向四：跨运行状态隔离与运行身份](#方向四跨运行状态隔离与运行身份)
5. [方向五：编排引擎自适应性——从硬截止到可配置降级策略](#方向五编排引擎自适应性)

---

## 方向一：跨工作流编排管线守卫——从声明到强制执行

**优先级**: 🔴 P0 | **类别**: 架构 · 治理 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 定义了一条完整的脊柱管线：`DISCOVER → DESIGN → REVIEW → BUILD → EVOLVE`（`ARCHITECTURE.md` 第 16 行），且每个工作流的 stop_condition 声明了 `on_approved.next_stage` 指向下一个阶段：

```yaml
# .agent/workflows/design.yml:62-69
stop_condition:
  type: human_gate
  human_approval: required
  on_approved:
    next_stage: review
```

```yaml
# .agent/workflows/review.yml:138-141
stop_condition:
  type: human_gate
  human_approval: required
  on_approved:
    next_stage: build
```

但 **ZERO 代码读取 `on_approved.next_stage`**。整个管线顺序没有任何形式的强制执行：

- 用户可以在完成 discovery 之前直接 `forge run build`——系统不会阻止
- 用户可以在 design 被拒绝之后 `forge run build`——系统不会阻止
- 用户可以在 review 还在进行中时 `forge run evolve`——系统不会阻止
- 一个 workflow 的产物（如 PRD、架构方案）对下游 worklow 的可用性从未被验证

### 代码级证据

**证据 A：`asset.OnApproved` 结构体声明了 `NextStage`，但 forge-core 零消费**：

```go
// forge-core/internal/asset/asset.go:193-199
type OnApproved struct {
    NextStage string `json:"next_stage"`
}
```

搜索全仓对这个字段的引用：

```bash
$ grep -rn "NextStage\|next_stage" forge-core/ --include="*.go"
forge-core/internal/asset/asset.go:198:  NextStage string `json:"next_stage"`
forge-core/cmd/forge/main.go:155:       // on_approved.next_stage surface
```

`main.go:155` 只打印了它，从未用作守卫逻辑。`NextStage` 是纯声明，零强制执行。

**证据 B：`forge run` 可以通过 `--workflow`（位置参数）加载任何工作流文件，不做前置检查**：

```go
// forge-core/cmd/forge/main.go:180-205
func cmdRun(args []string) int {
    // ...
    wf, err := loadWorkflow(o.root, args[0]) // 加载任何指定的 workflow
    // ...
    return execEngine(ctx, wf, o) // 直接执行，不检查管线顺序
}
```

**证据 C：`forge evolve` 同样没有管线前置检查**：

```go
// forge-core/cmd/forge/evolve.go:~80-120
func cmdEvolve(args []string) int {
    // 只检查 human_gate 拒绝，不检查前一阶段是否批准
    // 不检查 discover/design/review 是否完成
}
```

**证据 D：标注了 `next_stage` 路径的 `human_gate` 批准标记 `.forge/<stage>.approved` 存在，但只有当前 stage 用它——从不指向下一个 stage**：

```go
// forge-core/cmd/forge/approve.go:~30-55
func cmdApprove(args []string) int {
    // 写 .forge/<stage>.approved 标记
    // 但从不读取 worklow 的 on_approved.next_stage 来验证「要批准的 stage 是否符合管线顺序」
}
```

### 与已有覆盖的区别

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---------|---------|------------|
| `execution-semantic-gaps.md` | 单个工作流内的执行语义（原子性/幂等/回滚） | 不讨论工作流之间的顺序约束 |
| `expansion-horizon-three.md` | 多仓库联邦编排 | 讨论跨仓库的编排，不讨论同一仓库内的管线顺序 |
| `forgotten-five-foundations.md` | 跨进程运行时守护 | 讨论并发 forge 进程间的文件锁，不讨论工作流执行顺序 |
| `five-uncovered-horizontal-frontiers.md` | 工作流组合代数 | 讨论工作流之间的**数据流**组合（一个的输出到另一个的输入），不讨论**顺序强制执行** |
| `ARCHITECTURE.md` | 脊柱管线声明 | 只是文字描述，不是代码强制执行 |

**核心差异**: 本方向不解决「如何组合工作流」，而是解决「如何防止工作流被乱序执行」——这是一个**治理闸门**而非**编排机制**。

### 为什么需要它

**场景 1：生产事故**。一个直接在裸仓库上运行 `forge run build` 的用户，跳过了 discover/design/review，AI 直接按照对需求的猜测写出代码并部署——缺失了需求验证、架构审批和安全评审。这是 ForgeOS 声称要防止的核心反模式，而当前架构完全没有防范。

**场景 2：协作混乱**。团队 A 在 review 阶段要求 redesign，团队 B 在不知情的情况下基于已拒绝的设计方案开始 build——两个团队各自产生矛盾的工作，预算和时间双重浪费。

**场景 3：审计失败**。合规审计要求证明「每个进入 build 的特性都经过了设计审批」。如果 `forge run build` 可以在 design 批准前执行，这个证明不可能给出。

### 边界情况

- **向后兼容**: 现有项目可能没有完整的管线文件（只有 build.yml）。守卫必须允许「部分管线」模式，只在声明了 `next_stage` 的 worklow 之间强制顺序。
- **Evolve 循环**: `forge evolve` 本身就是一个跨迭代循环，不应被管线守卫阻塞——需要区分「首次进入 evolve」和「evolve 内的后续迭代」。
- **多项目同一仓库**: 当仓库中有多个独立模块（如 examples/ 下的 url-shortener 和 go-taskd），管线守卫应作用于模块级别而非全局。
- **管线重置**: 如果 design 被 redesign 驳回后重置，之前通过的设计批准应失效，阻止基于旧设计的 build。

### 具体方向建议

- **`.forge/stage-machine.json`**: 新增运行状态文件，追踪当前管线中的阶段位置（当前阶段、已批准阶段、管线历史）
- **`forge run` 前置检查**: `RunFrom` 在加载 worklow 后、执行 phase 前，检查 `.forge/stage-machine.json` 中声明的先决阶段状态
- **`on_approved.next_stage` 消费**: `cmdApprove` 写入 `.forge/<stage>.approved` 时，读取 worklow 的 `next_stage` 验证顺序
- **`forge status --pipeline`**: CLI 子命令显示当前管线状态，哪些阶段已完成/已批准/待执行
- **可选性机制**: `project.yml > pipeline.guard: optional | enforced` 控制是否启用管线守卫，默认 `enforced` 在 production lifecycle
- **`--force` 标志**: `forge run build --force` 跳过管线守卫（需记录审计日志）

---

## 方向二：Agent 治理资产的版本化生命周期管理

**优先级**: 🟡 P1 | **类别**: 治理 · 可演化性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的治理资产——agent 卡（`.agent/agents/*.md`）、skill 卡（`.agent/skills/*.md`）、workflow（`.agent/workflows/*.yml`）、policy（`.agent/policies/*.yml`）、routing 策略（`.agent/routing/policy.yml`）、AI 提示模板（`.ai/prompts/*.md`）——都是**纯文本文件，没有任何版本化机制**。

这意味着：

1. **修改一条 agent 卡的 prompt 会影响该 agent 的所有历史 run**。如果一个新的 prompt 实际上效果更差（但 gate 通过了），没有机制定位是「agent 卡的第 3 版」引入了回归。
2. **没有「agent 卡版本」的概念**。每个 evolve 迭代产生的 trace 标注了 model（`claude-opus-4`）但没有标注使用的 agent 卡版本。如果 reviewer 卡在 Sprint 20 和 Sprint 25 之间被修改，分析 reviewer 效果变化时无法归因。
3. **Workflow 的 YAML 修改无法原子化回滚**。如果 `build.yml` 增加了一个 phase，但这个 phase 有 bug，`git revert` 是唯一的方式——但没有运行时机制防止回滚后的版本冲突。
4. **跨项目共享治理资产时无法追踪版本**（ADR-0003 的 submodule 机制已设计但未落地）。当上游 `.agent/` 更新时，下游项目无法选择性采纳变更。

### 代码级证据

**证据 A：trace.Event 没有任何「治理资产版本」字段**：

```go
// forge-core/internal/trace/trace.go:81-99
type Event struct {
    Format        string `json:"_format,omitempty"`
    Seq           int    `json:"seq"`
    Kind          string `json:"kind"`
    Name          string `json:"name"`
    Status        string `json:"status"`
    DurationMs    int64  `json:"duration_ms"`
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"`
    Model         string `json:"model,omitempty"`
    Detail        string `json:"detail,omitempty"`
    // ← 没有 AgentCardVersion / WorkflowVersion / PolicyVersion
}
```

每个 trace event 记录了 `model`（如 `claude-opus-4`），但没有任何信息记录「当时用的是哪个版本的 architect 卡」「哪个版本的 build.yml」。这使得 trace 数据无法完整归因效果变化。

**证据 B：`prompt.ContextCache` 缓存了 agent 卡内容，但不追踪版本**：

```go
// forge-core/internal/prompt/cache.go:141-148
func (c *ContextCache) invariants(repoRoot string) ([]Doc, string) {
    c.mu.Lock()
    defer c.mu.Unlock()
    if !c.built {
        c.adrDocs = adrDocs(repoRoot)
        c.constraintsBlock = constraints(repoRoot)
        c.cardText = loadCards(repoRoot) // 加载卡内容，但不用版本标识
        c.built = true
    }
    return c.adrDocs, c.constraintsBlock
}
```

当 agent 卡 md 文件变化后，缓存自动失效（`built==false`），但**没有记录新旧版本的 hash 或版本号**，因此无法在 trace 中追溯哪个版本的卡产生了哪个阶段的输出。

**证据 C：Scorecard 记录了 (model, task_type) 但不记录 (agent_card_version, workflow_version)**：

```go
// forge-core/internal/attribution/attribution.go:28-36
type ScorecardPair struct {
    Model    string
    TaskType string
    // ← 没有 AgentCardVersion / WorkflowVersion / PolicyVersion
}
```

这意味着：如果改变 `architect.md` 的 prompt 后架构设计质量下降，scorecard 会显示 `model=opus, task_type=architecture, quality_score=0.6`，但无法判断这个 quality 的下降是因为模型变化、routing 变差、还是 agent 卡 prompt 改坏了。

**证据 D：forge-init 复制治理资产时不保留来源版本信息**：

```go
// harness/scaffold/forge-init.mjs 无版本追踪代码
// 复制 .agent/ 和 harness/ 到新项目，但不生成 manifest.json
```

### 与已有覆盖的区别

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---------|---------|------------|
| `structural-gaps-v41.md`「配置 Schema 版本化」 | Workflow/policies 的 YAML schema 版本演进 | 讨论**格式版本**（字段增删），不讨论**内容版本**（同一 schema 下的 prompt 变化） |
| `novel-extensions-v36-deep-architectural.md`「Agent 版本化」 | Agent 运行时协议的版本化（接口契约） | 讨论**agent 与引擎之间的通信协议版本**，不讨论**agent 卡 prompt 内容版本** |
| `forgotten-five-foundations.md`「治理热加载」 | 治理资产变更后的动态重载机制 | 讨论**运行时重载**，不讨论**版本追踪与回滚** |
| `expansion-horizon-three.md`「多仓库联邦」 | 跨仓库共享治理资产的机制 | 讨论**分发拓扑**，不讨论**版本化本身** |
| `docs/adr/0003-agent-os-repo-extraction.md` | 将 `.agent/` 提取为独立 submodule | 讨论**仓库位置**，不讨论**版本化管理** |

**核心差异**: 本方向不讨论「如何存放/分发治理资产」，而是讨论**如何对治理资产的变化进行版本标记、可观测追溯和选择性回滚**——将 `.agent/` 从一个文件目录升级为**版本化的资产仓库**。

### 为什么需要它

**场景 1：Prompt 回归不可归因**。团队修改了 `reviewer.md` 的 prompt，加入了新的审查维度。之后几个 build 循环中 reviewer 开始误报——但不确定是因为 prompt 改了还是模型变了。没有版本信息，归因取决于人的记忆。

**场景 2：选择性采纳上游治理更新**。一个派生项目收到上游 `.agent/` 的更新：workflow 增加了一个安全 gate，但同时也改动了某个 agent 卡的 prompt。项目希望只采纳 workflow 更新而不接受 prompt 改动——当前只能全盘接受或手动 diff。

**场景 3：合规审计的「运行时视角」**。审计要求：「请列出上周三的 build 运行中，使用的 security-engineer.md 是哪个版本」。当前不可能回答——log 只记录了 `model` 和 `phase`，不记录资产版本。

### 边界情况

- **性能开销**: 每次 run 计算所有 agent 卡/workflow 的 git hash 会引入启动延迟。建议：`prompt.ContextCache` 扩展为在 `invariants()` 构建时也捕获当前 git hash（或目录内容的 sha256 树），存入缓存。
- **非 git 仓库工作负载**: 当仓库不在 git 中时（`forge run` 的可能场景），版本回退为 mtime+size。
- **版本膨胀**: trace event 里不应嵌入完整 hash（128 位），建议用 `git describe --always --dirty` 的短格式（7 字符）。
- **Submodule 分层**: 当 ADR-0003 落地后，`.agent/` 可能是 submodule，版本应同时记录 submodule commit 和主仓库 commit。

### 具体方向建议

- **`asset.VersionInfo`**: 新增结构体承载当前治理资产的版本快照（agent 卡 git hash / mtime、workflow hash、policies hash）
- **`GatherCached` 版本注入**: 在 `invariants()` 构建时捕获版本快照并返回
- **`trace.Event.ToolVersions`**: 新增 `tool_versions` 字段（omitempty），记录 `{"agents/reviewer": "abc123f", "workflows/build": "def4567"}` 格式
- **`scorecard.GovVersion`**: Scorecard schema 扩展 `agent_card_version` / `workflow_version` 字段
- **`forge status --assets`**: CLI 子命令显示当前治理资产的版本和最近修改时间
- **`forge migrate --from-gov-version`**: 迁移时基于治理资产版本做差异化的 `derive_tasks`

---

## 方向三：外部工具链版本契约与环境钉扎

**优先级**: 🟡 P1 | **类别**: 可靠性 · DevOps | **预估**: ~1–2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 依赖大量外部工具来执行其治理功能：

| 工具 | 用途 | 最小版本 | 当前验证方式 |
|------|------|---------|------------|
| `python3` | `check.py` 治理检查 + yaml2json 回退 | 3.8+？ | `doctor.Run` 只检查 PATH 存在 |
| `node` | `gate.mjs` / `acceptance.mjs` / 适配器 | 18+？ | 隐式依赖，无版本检查 |
| `claude` CLI | agent 执行器 (`--agent-cmd`) | 未知 | `preflight.go` 只检查 PATH 存在 |
| `go` | 构建 forge-core | 1.21+？ | 无 |
| `golangci-lint` | Go lint (adapter) | 1.55+？ | `probeLint` 检查安装但无版本 |
| `eslint` | TS/JS lint (adapter) | 8+？ | `probeLint` 检查安装但无版本 |
| `ruff` | Python lint (adapter) | 0.3+？ | 无检查 |
| `git` | risk auto-detection / file delta | 任意 | 无检查 |

这些依赖散布在多个地方：
- `forge doctor` 检查 `python3` 和 `go`
- `forge preflight` 检查 `claude` 和 `python3`
- `adapters/` 框架的 `probeLint`/`probeCoverage` 检查单个 linter 是否安装
- 绝大多数工具**没有任何版本检查**

当某个工具的版本变化导致行为变化时（如 `eslint` 从 8→9 规则大变），forge 的行为会静默变化——更糟的是，**不同开发者可能因为本地工具版本不同而得到不同的 gate 结果**。

### 代码级证据

**证据 A：`forge doctor` 只检查 `python3` 是否在 PATH，不检查版本**：

```go
// forge-core/internal/doctor/doctor.go:161-165
func python3Check() Check {
    if _, err := exec.LookPath("python3"); err != nil {
        return Check{Name: "python3 on PATH", OK: false, Detail: "required for check.py governance check"}
    }
    return Check{Name: "python3 on PATH", OK: true}
}
```

**证据 B：`forge preflight` 对 `claude` 也只检查 PATH**：

```go
// forge-core/cmd/forge/preflight.go:94-101
func checkClaudeCLI(executor, agentCmd string, rep *preflightReport) {
    if executor != "command" {
        return
    }
    if _, err := exec.LookPath(agentCmd); err == nil {
        rep.pass("%s on PATH", agentCmd)
    } else {
        rep.fail("%s not found on PATH", agentCmd)
    }
    // 不检查版本
}
```

**证据 C：adapters 的 `probeLint` 只探测「是否安装」不探测「版本是否匹配」**：

```mjs
// harness/adapters.mjs:~210-240
export function probeLint(root, lang, log) {
    // 解析 adapter → 获取 lint 命令 → 检查二进制是否存在
    // 但不检查版本号是否满足 adapter.yml 中声明的要求
}
```

**证据 D：policies.yml 声明了工具要求，但不是版本化合约**：

```yaml
# harness/policies.yml — 不声明工具版本
# .agent/policies/modes.yml — 不声明工具版本
# 没有任何地方声明 "这个库依赖 golangci-lint >= 1.55.0"
```

### 与已有覆盖的区别

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---------|---------|------------|
| SCA/CVE（Sprint 19） | 项目自身依赖的漏洞扫描（npm/go/pip） | 讨论**项目的第三方库**，不讨论**ForgeOS 工具链** |
| `doctor.Run` / `forge doctor` | 诊断 `python3` 和运行时状态 | 被动诊断，非版本化合约 |
| `forge preflight` | 运行前环境检查 | 只检查存在性，不检查版本匹配 |
| `probeLint` / `probeCoverage` | 单个 linter/coverage 工具可用性 | 分散在每个 command 中，无集中版本声明 |

**核心差异**: 本方向要求一个**集中的、版本化的、可验证的外部工具依赖合约**——类似于 `package.json` 的 `devDependencies` 或 `go.mod` 的 `toolchain` 指令——并集成到 `forge run` 的前置检查中。

### 为什么需要它

**场景 1：CI 与本地不一致**。开发者本地用 `eslint 8.56` 通过 lint，CI 用 `eslint 9.0` 报 50 个新错。没有任何 pre-commit / pre-run 检查会提前警告。

**场景 2：安全回归**。项目依赖 `golangci-lint 1.55` 的某个安全检查器，CI 中升级到 `1.60` 后该检查器的默认行为变化——安全违规未被捕获。因为没有版本声明，团队不知道 CI 环境工具版本已漂移。

**场景 3：新成员加入**。新开发者按照 README 装好工具后运行 `forge run build`，因为 `node` 版本是 16（需 ≥18）导致 `gate.mjs` 语法错误——报错信息不友好。

### 边界情况

- **跨平台版本差异**: Linux 的 `ruff` 版本与 macOS 版本可能不同步。合约应允许 `>=` 语义版本范围。
- **工具不存在时的行为**: 合约应区分「此工具对 forge 运行是必需的」（如 node、python3）和「可选的但启用特定 gate」（如 eslint、golangci-lint）。
- **向后兼容**: 已有项目没有 `.forge-toolchain.yml` 合约文件。引擎应优雅降级——无合约时保持当前行为（只检查 PATH 存在），有合约时严格执行版本检查。
- **Docker 容器内运行**: 当在容器内运行时，工具版本是固定的，版本检查可以绕过或自动满足。

### 具体方向建议

- **`.agent/toolchain.yml`**: 新增版本化工具依赖声明文件，格式类似：
  ```yaml
  # 必须的工具（forge 运行的前提）
  required:
    python3: ">=3.8"
    node: ">=18.0"
    git: "*"
  # 网关工具（启用特定 gate）
  gates:
    lint:
      golangci-lint: ">=1.55"
      eslint: ">=8.0"
    test:
      vitest: ">=1.0"
  # agent 执行器（至少一个可用）
  executors:
    claude: ">=0.3"
  ```
- **`forge doctor --toolchain`**: 新增检查模式，对照 `.agent/toolchain.yml` 验证所有声明工具的存在性和版本
- **`forge preflight` 集成**: 版本检查作为 preflight 的一部分
- **`forge-init` 输出**: `forge-init` 为新项目生成默认 `.agent/toolchain.yml`
- **版本检查工具**: 一个可扩展的版本比对引擎，支持 `>=`、`^`、`~`、`*`、`<`、`=` 等语义版本操作符
- **环境快照**: `forge doctor --snapshot` 记录当前工具版本的 JSON 快照，与 trace 一同存档

---

## 方向四：跨运行状态隔离与运行身份

**优先级**: 🟠 P1 | **类别**: 可靠性 · 状态管理 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当前 ForgeOS 的运行时状态目录 `.forge/` 是一个**全局单例**。无论多少并发进程、多少独立运行，都共享同一组文件：

```
.forge/
  checkpoint.json     ← 所有进程共享，最后写入者覆盖
  trace.jsonl         ← 所有进程追加写入，行交错
  memory.jsonl        ← 所有进程追加写入，条目混合
  checkpoint.json.1..5  ← 历史混杂
```

这导致三个严重问题：

1. **状态静默覆盖**: 两个终端同时跑 `forge run build`，第二个进程的 `checkpoint.json` 覆盖第一个的进度——第一个进程的 checkpoint 静默丢失。
2. **Trace 污染**: 两个 `forge run` 的 trace 事件在 `trace.jsonl` 中交错，seq 号重复，下游 scorecard 无法区分哪个事件属于哪个 run。
3. **Memory 幻觉**: 一个 evolve 循环的 memory 条目被另一个 evolve 循环读取，产生「我的前置分析认为 API 协议是 REST，但 memory 说你之前决定用 GraphQL」的幻觉。

### 代码级证据

**证据 A：`persist.Save` 是全局的——无运行身份参数**：

```go
// forge-core/internal/persist/checkpoint.go:95-102
func Save(path string, cp Checkpoint, retain int) error {
    // path 是固定的 .forge/checkpoint.json
    // 无 run_id 参数区分不同运行
}
```

**证据 B：`memory.Append` 和 `trace.Emit` 同样无运行身份**：

```go
// forge-core/internal/memory/memory.go:152-170
func Append(path string, e Entry) error {
    // path 固定为 .forge/memory.jsonl
    // 如果一个 evolve 循环正在写入，另一个 forge run 不会知道
}

// forge-core/internal/trace/trace.go:102-120
type Tracer struct {
    mu  sync.Mutex
    w   io.Writer
    seq int
    // 无 run_id
}
```

**证据 C：LoopEngine 不设置隔离上下文**：

```go
// forge-core/internal/orchestrator/loop.go:~50-70
type LoopEngine struct {
    Engine Engine
    // 无 RunID / SessionID 字段
    // 无隔离策略（独占/共享/只读）
}
```

**证据 D：`openTracer` 和 `openRunResources` 不产生隔离文件路径**：

```go
// forge-core/cmd/forge/main.go:~290-310
func openTracer(root string) (*trace.Tracer, func(), error) {
    tracePath := filepath.Join(root, ".forge", "trace.jsonl")
    // 零隔离——每个进程写同一个文件
}
```

### 与已有覆盖的区别

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---------|---------|------------|
| `forgotten-five-foundations.md`「跨进程运行时守护」 | 文件锁 + PID 文件防止并发放问 | 讨论**并发互斥**（防止两个进程同时写入 checkpoint），不讨论**运行身份**（区分谁写了什么） |
| `five-uncovered-architectural-frontiers.md`「并发状态隔离」 | 并行编排下的内存隔离（锁顺序、竞态） | 讨论**线程级并发安全**，不讨论**进程级运行隔离** |
| `second-order-architectural-gaps.md`「memory 信噪比」 | memory 条目数量增长导致信号稀释 | 讨论**条目质量**，不讨论**条目来源归属** |
| `strategic-extensions-v22.md`「无声数据丢失」 | 写入失败时的数据完整性问题 | 讨论**写入安全性**，不讨论**写入归属** |

**核心差异**: 本方向不解决「两个进程同时写入会冲突」，而是解决「两个进程各自写入后，下游工具无法区分数据来源」。前者是并发控制，后者是**数据血缘（data provenance）**。

### 为什么需要它

**场景 1：IDE 插件 + CLI 同时运行**。VSCode 插件自动运行 `forge run build` 做预检查，同时终端用户也在跑 `forge run build`——两个 trace 交错，scorecard 报告 p95=45s（实际上是因为合并了两个独立运行的数据）。

**场景 2：CI 并行矩阵**。CI 中 matrix 策略同时跑 4 个 `forge evolve`（不同 model 组合对比），所有 trace 写入同一文件，事后分析无法区分每个 matrix 变体的独立表现。

**场景 3：夜间批量 + 手动触发冲突**。定时任务每晚跑 `forge evolve`，当开发者白天手动运行时中断了夜间进程——checkpoint 继续累积夜间进度还是重置为手动进度，完全取决于竞态时序。

### 边界情况

- **运行身份的粒度**: 每个 `forge run`/`forge evolve` 应获得唯一 `run_id`（UUID v7 带时间戳），在进程启动时生成并保持到运行结束。
- **隔离级别**: 运行身份应支持不同隔离策略：`isolated`（完全独立的 `.forge/<run_id>/` 子目录）、`shared`（当前行为，所有跑共享同一状态）、`advisory`（单独 trace 文件但共享 checkpoint）。
- **旧数据兼容**: 已有 `.forge/trace.jsonl` 中没有 run_id 的行应视为属于一个匿名运行，不被丢弃。
- **运行身份持久化**: loop checkpoint/resume 需要保存 `run_id`，以便一个 evolve 循环的多次迭代使用同一身份。
- **运行清理**: 需要 `forge clean --run <run_id>` 或保留最近 N 个运行的程序，防止磁盘膨胀。

### 具体方向建议

- **`internal/session` 新包**: 管理运行身份（`SessionID` 生成、持久化、隔离策略）
- **`SessionID` 注入**: `forge run`/`evolve` 启动时生成 UUID v7，注入到所有子系统中（trace、persist、memory、prompt cache）
- **隔离的文件路径**: 隔离模式下 `trace.jsonl` → `.forge/runs/<run_id>/trace.jsonl`、`checkpoint.json`→`.forge/runs/<run_id>/checkpoint.json`
- **跨运行内存读取治理**: memory 条目增加 `session_id` 字段，query 时默认只读当前 session 的数据，交叉引用需显式 flag
- **`forge list-runs`**: 新子命令列出 `.forge/runs/` 下的历史运行及元数据（workflow、mode、时间、结果）
- **`.forge/runs/.last` 符号链接**: 指向最近一次运行，兼容当前读取 `.forge/checkpoint.json` 的路径

---

## 方向五：编排引擎自适应性——从硬截止到可配置降级策略

**优先级**: 🟠 P1 | **类别**: 韧性 · 运行时 | **预估**: ~2–3 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当前 ForgeOS 引擎在面对资源压力时，只提供**二进制响应**——要么通过（预算充足），要么硬截止（`checkRunBudget` → abort、`MaxAgentCalls` → fail-closed）。没有一个**渐进的降级路径**。

目前存在的硬截止点：

| 截止点 | 触发条件 | 响应 |
|--------|---------|------|
| `checkAgentBudget` | agent 调用次数超过 `MaxAgentCalls` | **fail-closed**，abort 整个 run |
| `checkRunBudget` | 累积花费超过 `--run-budget-usd` | **fail-closed**，abort 整个 run |
| `MaxLoopBack` | gate 循环回跳次数超限 | **fail-closed**，abort |
| `MaxOutputBytes` | agent 输出超限 | **truncate**（但 agent 不知道输出被截断） |
| `MaxDepth` | agent 递归深度超限 | **fail-closed**，拒绝 spawn |
| `NoProgress` tripwire | 连续无进展迭代 | **fail-closed**，stop evolve |
| `MaxIter` | 迭代数超限 | **clean stop**（但浪费预算） |

但这些硬截止点之间没有协同：
- 预算即将用尽时，不会提前降级 tier（`BudgetAdjustTier` 在 `spendRatio>=0.80` 时降级——但只对 routing 生效，不会触发 checkpoint 频率降低、memory 裁剪或 trace 采样率降低）
- Agent 输出达到 `maxOutputBytes` 的 80% 时，不会先警告再截断
- Memory 条目数达到阈值（如 1000 条）时，不会先 prune 再继续 append
- Trace 文件大小达到阈值时，不会先 rotate 再继续写入

这种「悬崖式截止」在长期运行的 evolve 循环中尤其危险：一个持续 24 小时的循环可能在最后 10% 的预算内突然中止，之前的投入全部浪费。

### 代码级证据

**证据 A：五个资源阈值都是硬边界，无预警/降级路径**：

```go
// forge-core/internal/orchestrator/orchestrator.go:248-262
func (e Engine) runAgentPhaseBudgeted(ctx context.Context, p asset.Phase, mode string, calls *int) error {
    if err := e.checkAgentBudget(calls); err != nil {
        return err // 超过硬上限 → fail-closed，无降级选项
    }
    if err := e.checkRunBudget(*calls - 1); err != nil {
        return err // 超过预算 → fail-closed，无降级选项
    }
    return e.runAgentPhase(ctx, p, mode)
}
```

**证据 B：trace.jsonl 写入失败是 fail-closed——无降级到「静默丢弃」的路径**：

```go
// forge-core/internal/trace/trace.go:107-120
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    // ...
    if _, err := t.w.Write(line); err != nil {
        return fmt.Errorf("trace: writing event seq=%d: %w", ev.Seq, err)
    }
    // 如果磁盘满、权限不足或文件系统只读，整个 phase 会因 trace 写入失败而中断
}
```

**证据 C：memory.Load 对整个文件解码——无流式读取或部分容错**：

```go
// forge-core/internal/memory/memory.go:184-205
func Load(path string) ([]Entry, error) {
    data, err := os.ReadFile(path) // 如果 memory.jsonl 是 200MB，这行分配 200MB 内存
    // ...
    entries, err := decode(data) // 一次性解码全部条目
}
```

当 memory.jsonl 膨胀到 200MB+ 时，Load 本身就可能 OOM。

**证据 D：CommandExecutor 的 `cappedBuffer` 静默截断 agent 输出，但 agent 不知道**：

```go
// forge-core/internal/orchestrator/command_executor.go:~130-160
// cappedBuffer 保留 ≤cap 字节，超出部分 drain 但不告知 agent
// agent 可能写了一部分关键代码被截断，但接下来的 harness gate 认为代码完整
```

### 与已有覆盖的区别

| 已有分析 | 覆盖内容 | 与本文的区别 |
|--------|---------|------------|
| `strategic-extension-five-novel.md`「梯度安全响应」 | 从硬截止到梯度响应（soft warning → rate limit → hard stop） | 覆盖**资源维度的梯度**（budget/memory/time），但不覆盖**功能维度的降级**（降低 trace 精度→关闭非核心 gate→降级到只读模式） |
| `novel-architectural-extensions-v40.md`「墙钟预算」 | 工作级整体执行时间上限 | 讨论**单一维度的时间控制**，不讨论**多维资源协同降级** |
| `edgecases-and-perf.md`「trace 无限增长」 | trace.jsonl 文件轮换 | 讨论**单个文件的轮换策略**，不讨论**系统级的资源感知自适应** |
| `second-order-architectural-gaps.md`「memory 压缩」 | memory 条目的合并与修剪 | 讨论**记忆压缩的算法**，不讨论**触发压缩的条件与系统级调度** |

**核心差异**: 本方向要求一个系统级的**资源感知自适应框架**——不是单个维度的优化（trace 轮换、memory 修剪），而是当引擎检测到资源压力时，协调多个子系统做**协同降级**，尽量延长有意义的运行时间而不是硬截止。

### 为什么需要它

**场景 1：24h evolve 过程中磁盘空间下降**。evolve 循环已运行 12 小时，累积了 500MB trace + 200MB memory + checkpoint 历史。磁盘从 60% → 95%。当前行为：trace.Emit 遇到 ENOSPC 导致 phase fail，整个 evolve 回滚——12 小时的 LLM 花费全部浪费。

**理想行为**：引擎检测到磁盘使用 >80% → 通知各子系统做降级：trace 降低 Event.Detail 详细度 → 触发 memory.Compact → 丢弃最老的 checkpoint 历史 → 如果仍 >90% 则切换到高压缩率的 trace 格式 → >95% 则优雅终止（保留已有进度）。

**场景 2：预算敏感期的高效运行**。用户设 `--run-budget-usd=50`，运行到 $40（80%）时触发近预算信号。当前只做 `BudgetAdjustTier`（降级 model tier）。自适应框架应该同时：启用 memory prune（节省下次加载时间）、降低 trace 采样率（只记录 phase start/end 不记录 detail）、跳过 checkpoint 历史备份（减少 IO）、在 converge 报告中标注「预算接近上限，建议增加 budget 或调整 scope」。

**场景 3：高负载下的自我保护**。CI 环境中多个 `forge run` 同时运行，总并发度过高导致 claude API 返回 529（overload）。当前：每个 run 各自 backoff 重试，但重试加剧了负载。自适应：引擎检测到连续 529 → 本机协调退避或降级并行度。

### 边界情况

- **降级可观测性**: 每个降级动作必须在 trace 中记录（`DecisionEvent`），并在 `forge run`/`evolve` 的日志中显式标注 `[DEGRADE]`，以便用户在事后分析中看到「不是因为 run 出了问题，而是因为系统自适应降级了 X」
- **降级回退**: 当资源压力解除后，部分降级应自动回退（如磁盘空间释放后恢复 full trace 记录）
- **降级策略声明**: 应支持用户通过 `project.yml` 声明策略——`degradation: graceful | fail-fast | conservative`，而不是硬编码
- **降级不绕过 safety**: 安全 gate（critical→Opus、production→full review）绝不应被降级策略越过
- **测试**: 降级路径比 happy-path 更难正确测试。需要 `--simulate-disk-pressure` 或 `--simulate-budget-exhaustion` 测试 flag

### 具体方向建议

- **`internal/health` 新包**: 系统级健康监控与资源感知，监控以下信号：
  - 磁盘空间（`.forge/` 所在文件系统的可用空间百分比）
  - 内存压力（`runtime.MemStats` 或 `/proc/meminfo`）
  - trace/memory 文件大小趋势
  - API 529 频率
  - 预算消耗率（`spend / iteration`）
- **自适应 degrader 接口**:
  ```go
  type Degrader interface {
      Level() DegradeLevel       // Normal | Caution | Critical
      Suggest() []DegradeAction  // 当前压力下的建议降级动作
  }
  ```
- **预装的降级动作**:
  - `DegradeTrace`: 降低 trace Event.Detail 详细度或采样率
  - `DegradeMemory`: 触发 `memory.Compact` 或 `memory.Prune`
  - `DegradeCheckpoint`: 降低 checkpoint 备份历史数（`retain: 5 → 2`）
  - `DegradeGate`: 跳过非载重 gate（在 CI 压力下跳过 `complexity` gate）
  - `DegradeParallel`: 降低并行度
  - `Stop`: 优雅终止（保存 checkpoint + 归档 trace）
- **`forge status --health`**: CLI 子命令显示当前系统资源状态和活跃降级
- **`project.yml > degradation`**: 新增配置段，允许用户设定降级偏好

---

## 优先级矩阵

| # | 方向 | 优先级 | 类别 | 预估 | 依赖性 | 杠杆 |
|---|------|--------|------|------|--------|------|
| 1 | 管线顺序守卫 | **P0** | 架构·治理 | ~2 sprints | 依赖 `asset.OnApproved.NextStage`（已有，扩展消费） | ⭐⭐⭐⭐⭐ |
| 2 | 治理资产版本化 | P1 | 治理·可演化 | ~3 sprints | 依赖 trace 格式扩展、scorecard 扩展 | ⭐⭐⭐⭐ |
| 3 | 工具链版本契约 | P1 | 可靠性·DevOps | ~1–2 sprints | 独立，可增量实现 | ⭐⭐⭐⭐ |
| 4 | 运行身份隔离 | P1 | 可靠性·状态 | ~2 sprints | 影响 persist/trace/memory 三个子系统的文件路径 | ⭐⭐⭐⭐ |
| 5 | 降级策略框架 | P1 | 韧性·运行时 | ~2–3 sprints | 依赖新的 `internal/health` 包，侵入性中等 | ⭐⭐⭐⭐ |

**建议实施顺序**:
1. **管线顺序守卫**（P0）——零侵入（仅装配已有声明字段），解决架构级的管线完整性缺口
2. **运行身份隔离**（P1）——解决真实的多进程冲突问题，为所有后续可观测性改进提供基础
3. **工具链版本契约**（P1）——快速实现（独立新文件），高杠杆的可靠性改进
4. **降级策略框架**（P1）——需要最多的设计工作，但解决长期运行的生存性问题
5. **治理资产版本化**（P1）——最深的改动（trace/scorecard 格式），建议在其他四个方向稳定后实施

---

## 总结

本文提出的五个方向不是「ForgeOS 缺什么功能」的传统扩展分析，而是**跨层面系统性缺口**——它们位于两个或多个已有子系统的交界处，被已有 ~120 份分析的垂直覆盖所遗漏。

| 方向 | 跨越的子系统 | 已有覆盖的盲区原因 |
|------|------------|-----------------|
| 管线顺序守卫 | 5 个工作流 + stop_condition + approve 命令 | `next_stage` 被所有人看到但没人消费——声明字段的「视觉欺骗」 |
| 治理资产版本化 | agent 卡 + workflow + trace + scorecard | 每个子系统内部都有版本概念，但跨子系统的版本归属从未被讨论 |
| 工具链版本契约 | forge doctor + preflight + adapters | doctor 和 preflight 的诊断逻辑是过程式的、非声明式的，没有上升为合约 |
| 运行身份隔离 | trace + persist + memory + orcherator | 所有子系统都是「单进程视角」设计的，并发运行场景从未被纳入设计 |
| 降级策略框架 | orcherator + trace + memory + gate + routing | 每个引擎的截止点是独立设计的，系统层面的资源感知协调从未被考虑 |

ForgeOS 经过 31 轮 sprint 的构建和 120+ 份文档的覆盖，已经是一个功能极其完备的系统。但要让它从「能运行」变成**「能在生产环境中长期可靠运行」**，上述五个跨层面缺口是必须填补的结构性债务。
