# ForgeOS — 五个尚被忽视的运行时前沿

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局扫描至 2026-07-11，覆盖 `forge-core/`(18 Go 包 / ~35k LOC)· `harness/`(15+ 闸门/执法器)· `.agent/`(完整声明骨架)· `examples/`。  
> **去重**: 对 `docs/requirements/`(~82 份) + `docs/analysis/`(~38 份) + `docs/` 根目录下现有分析进行了关键词交叉检索。以下每个方向的核心关键词组合在现有文档中 **零篇** 或 **仅 1 篇提及一次且未展开**，确保方向本身未被此前分析作为独立系统性方向深挖。  
> **纪律**: 不编写任何代码。只做判断。  
> **基线原则**: forge-core 零外部 Go 依赖 · harness 零外部 Node/Python 依赖 · 治理宪法不可违反。

---

## 现有 120+ 分析的饱和覆盖域（本文不重复）

| 饱和覆盖域 | 估篇 | 本文处理 |
|---|---|---|
| 编排状态机(串/并行/loop-back/resume/mode-gating/stop-condition) | ~35 | ✅ 跳过 |
| 生产韧性(529/超时/退避/递归守卫/预算护栏/输出上限) | ~20 | ✅ 跳过 |
| 学习闭环(trace/scorecard/converge/Memory/Context 注入) | ~16 | ✅ 跳过 |
| 安全纵深(secret-scan/SCA/risk 分类/进程组/注入防御) | ~14 | ✅ 跳过 |
| 治理执法(arch-check 8 检查/check.py/drift-guard/function-length) | ~12 | ✅ 跳过 |
| 执行语义(原子性/幂等/TOCTOU/因果一致性) | ~8 | ✅ 跳过 |
| CLI 体验(detect/preflight/doctor/status/migrate/config) | ~8 | ✅ 跳过 |
| 第三地平线(多仓库/Web UI/事件驱动/管道组合) | ~7 | ✅ 跳过 |

**本文 5 个方向全部落在上述饱和域的间隙中**，聚焦于运行时基座层面的系统性缺口。

---

## 方向一 · 多进程运行时安全 —— `.forge/` 目录无并发保护

> **优先级**: 🟠 **P1** | **类别**: 运行时韧性 · 数据完整性 | **关键词验证**: `multi.*process.*safety` → **0 篇命中**; `concurrent.*forge` → 5 篇提及但均为单次提及，无系统性分析

### 问题

整个 `.forge/` 运行时状态目录——`trace.jsonl`、`memory.jsonl`、`checkpoint.json`——假设**单进程独占访问**。没有任何进程级锁（advisory file lock 或 `flock`），当两个 `forge run`/`forge evolve` 进程并发操作同一仓库时，会发生：

```
┌──────────────────────────────────────────────────────────┐
│ 进程 A: forge evolve build (iteration 3)                  │
│ 进程 B: forge run detect (并发读取 ROADMAP+workflow)       │
├──────────────────────────────────────────────────────────┤
│ trace.jsonl:   A 写 "agent:phase=implementer"            │
│                写了一半 → B 读到了不完整的 JSON 行 → 解析失败 │
│ memory.jsonl:  A Append(kind=gap, topic=test-coverage)   │
│                B Append(kind=decision, topic=refactor)    │
│                O_APPEND 下 >4KB 的行可能分裂交织            │
│ checkpoint:    A Save(iteration=3, phaseIndex=2)          │
│                B Save(iteration=1, phaseIndex=0)          │
│                temp+rename 原子但互相覆盖，最终状态不确定      │
└──────────────────────────────────────────────────────────┘
```

### 代码证据

**trace.jsonl** — `internal/trace/trace.go`:
```go
// 第 47 行: sync.Mutex 保护 seq 和 writer，但锁是进程内互斥
// 两个进程各自有自己的 Tracer，未使用文件级锁
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
```

并发写 trace 的后果是：进程 A 的 `Emit` 写入一行 512 字节的 JSON，进程 B 的写入可能穿插其中（POSIX 保证 <= PIPE_BUF 的 write 是原子的，但对常规文件无此保证；Go 的 `os.File.Write` 最终是一个 `write(2)` 系统调用，内核保证 <= 4096 字节的 write 是原子的——但 trace 行可能超过这个长度，尤其是当 `Detail` 字段携带长错误消息时）。

**memory.jsonl** — `internal/memory/memory.go`:
```go
// 第 143 行: O_APPEND 写入，单行 write(2) 在 <= 4096 字节时原子
// 但 memory 条目有 Detail 字段，prompt 构建时可能拼接长内容
// 超过 4096 字节的行被内核分割 → 交织
```

**checkpoint.json** — `internal/persist/checkpoint.go`:
```go
// Save 使用 temp+rename 原子替换，但如果两个进程同时 Save，
// 每个都写各自的 .tmp 然后 rename：
//   进程 A: write .forge/checkpoint.json.tmp → rename checkpoint.json
//   进程 B: write .forge/checkpoint.json.tmp → rename checkpoint.json
// 最终谁是胜者取决于内核调度，且胜者的检查点可能包含 A 的 phaseIndex
// 和 B 的 iteration 的混合状态
// rotateRetain 在并发下更是竞态——A 重命名 checkpoint→checkpoint.1
// 的同时 B 可能正在写 checkpoint.json
```

**全仓无一处 `flock` 或 `LockFile`**:

```bash
$ grep -r "flock\|FLOCK\|LockFile\|lockfile\|syscall.Flock\|osutil.*Lock\|sync\.Mutex.*file" forge-core/ --include="*.go"
# → 零结果。sync.Mutex 仅用于进程内锁，不跨进程。
```

**gate 层也无进程级互斥**：`internal/gate/gate.go` 的 `ProbeAll` 运行 `acceptance.mjs`，该脚本无锁，两个并发 `forge gate` 将同时 shell 出重复的 `node --test`、`secret-scan`、`arch-check` 进程，浪费 CPU 且可能产生冲突的输出。

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **数据完整性** | 并发写 trace/memory/checkpoint 可导致：trace 解析失败（`traceCheck` 报警）、memory 条目丢失（加载时 JSON 解析失败 → 整个文件不可读）、checkpoint 时间旅行（resume 时从旧 checkpoint 恢复，已完成的 phase 被重放并重新计费） |
| **CI/CD 场景** | 当 CI 流水线对同一个仓库并发运行 `forge check` 和 `forge run`（例如 CI 检查 PR 的同时，主分支在 evolve），如果它们指向同一个 `--root`，冲突必然发生。`.forge/` 被 `.gitignore` 排除但**不被 CI 隔离**。 |
| **多 workspace 问题** | 当前唯一的隔离机制是 `--root` 指向不同目录。但如果用户在同一 repo 的不同分支上运行 forge，`.forge/` 是共享的——Git 不隔离 `.forge/`。 |
| **经济损失** | checkpoint 被并发覆盖导致 resume 丢失已完成的 phase → 重跑 agent phase → 每个 phase 烧真实 API 费用。 |

### 建议方向

**v1 — 进程级 advisory 锁**：
- 在 `.forge/.lock` 创建一个 advisory file lock（`flock`/`LockFile`），在 `forge run`/`evolve`/`gate`/`check`/`accept` 入口处获取，退出时释放。
- 锁类型：`LOCK_NB`（非阻塞），获取失败时打印友好错误：`"another forge process is running in this repo (PID X locked .forge/.lock since Y)"`。
- 实现位置：`internal/gate/gate.go` 的 `RepoRoot()` 或 `cmd/forge/main.go` 的 `execEngine` 入口处——一个薄包装，不侵入现有逻辑。
- 不保护 `forge doctor`/`forge status`（只读，无害），不保护 `forge init`（无 `.forge/`）。

**v2 — 每资源列级锁**：
- trace 和 memory 使用独立的 append-only 锁文件（`.forge/.trace.lock`、`.forge/.memory.lock`），允许 checkpoint 写入与 trace 追加并行——比全局锁更细粒度。
- 读操作（如 `forge doctor` 读 trace）不需要锁，只阻塞写操作。

**v3 — 分支感知隔离**：
- 默认 `--root` 不变时，forge 自动附加当前 git 分支哈希：`.forge/main-abc1234/`、`.forge/feature-bran-xyz/`。分支切换不会污染状态。`--no-branch-isolation` 恢复旧行为。
- 对所有现有 forge 用户向后兼容：新行为仅在新目录结构中生效，旧 `.forge/` 在无分支哈希时按原有方式读取。

### 边界情况

- **`flock` 在 NFS/网络文件系统上不可靠**：v1 应在 `flock` 失败时降级为警告而非阻塞（fail-open，因为锁是完整性优化而非安全闸门）。
- **`forge evolve` 被 SIGKILL 后锁文件残留**：`flock` 在进程终止时由内核自动释放，不需要手动清理。
- **Windows 兼容性**：`flock` 是 POSIX 系统调用。Windows 上需要使用 `LockFileEx`/`CreateFile`。跨平台方案可考虑使用 Go 的 `osutil` 或 `.forge/.lock` 空文件上的 `os.OpenFile+O_EXCL` 作为轻量替代（虽然不完美，但有 crash-safe 问题）。
- **`forge run --parallel` 时子进程（goroutine 阶段）不应该持有锁**：锁在 CLI 进程级获取，并行 phase goroutine 共享同一个进程锁，不需要额外锁。

---

## 方向二 · ForgeOS 治理版本偏移 —— 项目模板无升级路径

> **优先级**: 🟠 **P1** | **类别**: 治理 · 生命周期管理 | **关键词验证**: `version.*skew.*governance` → **0 篇命中**; `template.*drift\|upgrade.*path.*forge` → 3 篇提及但均为产品级一句话提及，无架构分析

### 问题

`forge-init` 为项目创建一个静态的完整治理模板（`harness/` 全套执法器 + `.agent/` 资产 + CI 配置）。但当 ForgeOS 自身进化时——新增 gate、修改 agent 卡、更正 workflow 中的 `required_when` 引用或更新 `modes.yml` 的默认值——已初始化的项目**永远不会收到这些更新**。唯一的升级工具 `forge-upgrade.mjs` 尚存：

```javascript
// harness/scaffold/forge-upgrade.mjs
// 这是一个最小框架——目前只复制缺失的文件，不合并差异。
// TODO: diff-merge for existing files
```

后果：

```
forge-os v2.5.0 (初始)              forge-os v2.6.0 (三个月后)
  ┌─────────────────────┐             ┌─────────────────────┐
  │ modes.yml           │             │ modes.yml           │
  │  - lifecycle_floor  │◄─新增──     │  - lifecycle_floor  │
  │  - enforce_floor    │             │  - enforce_floor    │
  │  - evolve_depth     │             │  - evolve_depth     │
  │                     │             │  - review_depth     │◄─ 新增
  │ harness/secret-scan │             │ harness/secret-scan │
  │  - 15 patterns      │◄─ 新增──    │  - 22 patterns      │
  │ harness/arch/       │             │ harness/arch/       │
  │  - 8 checks         │             │  - 9 checks         │◄─ 新增 drift-docs
  └─────────────────────┘             └─────────────────────┘
         ↓ forge-init ↓                     ↓
  项目 A (静态快照)                 项目 B (同样是静态快照)
    modes.yml 无 review_depth       modes.yml 无 review_depth
    secret-scan 只有 15 模式        secret-scan 只有 15 模式
    arch-check 只有 8 检查          arch-check 只有 8 检查
    forge-upgrade 可复制新文件      forge-upgrade 可复制新文件
    但已有文件不合并                 但已有文件不合并
```

`project.yml` 没有存储创建它的 ForgeOS 版本。没有一个版本记录说 "this project was bootstrapped from forge-os v2.5.0"——所以没有任何机器可读的方式知道一个项目离最新治理有多远。

### 代码证据

**forge-init 不记录版本** — `harness/scaffold/forge-init.mjs`:
```javascript
// 不写版本标记。project.yml 中无 "forgeos_version" 或类似字段。
// 新项目无法追踪它初始化时使用的 ForgeOS 版本。
```

**forge-upgrade 是纯复制，不是 diff-merge** — `harness/scaffold/forge-upgrade.mjs`:
```javascript
// 现有工作文件跳过不更新:
//  "跳过已存在的文件: .agent/agents/architect.md"
// 没有 diff/patch 机制，没有冲突解决。
```

**`.agent/` 文件无版本头** — 每个 `modes.yml`、`policies.yml`、`routing/policy.yml`、agent 卡在初始创建后不会收到 ForgeOS 上游的更新。`check.py` 验证治理完整性（交叉引用、引用完整性），但不验证**版本漂移**——即一个 `modes.yml` 与上游的差异。

**版本差异的治理影响**：
- 新 gate（如 `architecture` 检查 8→9）不会自动出现在现有项目的 `workflows/build.yml` 的 `required_gates` 中。
- 现有 `project.yml` 的 `lifecycle:` 默认值（"mvp"）在 2.6.0 中可能改为 "growth"，但旧项目继续使用 "mvp"。
- 如果 `secret-scan.mjs` 新增了 7 个关键模式（如 `AI keys`、`JWT tokens`），旧项目的安全评估是**不完整的**，但 `forge accept` 仍然 PASS——因为没有版本漂移检测器。

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **治理完整性随时间衰减** | 项目 3 个月前通过了 `forge accept` 且 ACCEPTED，但当时的检查集比现在少 20%。没有坏掉的东西看起来是好的——但长期信任度在衰减。 |
| **安全问题** | secret-scan 模式库随着新的凭证泄露模式而增长。项目使用旧版本 ForgeOS 的扫描器会产生漏报。 |
| **升级恐惧症** | 没有安全升级路径意味着用户不敢升级。"万一 upgrade 破坏了什么"是完全合理的担忧，因为 forge-upgrade 除了复制新文件什么安全保证也没有。 |
| **ForgeOS 自身的 dogfood 崩溃** | 当 ForgeOS 仓库自身运行 `forge-init` 创建测试项目时，那些项目立即就过时了（它们使用当前发布版的模板，而当前开发版已经演化）。 |

### 建议方向

**v1 — 版本标记 + 漂移报告（只读）**：
- 在 `project.yml` 中添加 `forgeos_version` 字段（`forge-init` 在创建时注入，例如 `forgeos_version: 2.5.0`）。
- 新增子命令 `forge governance drift`（只读，不修改文件）：
  - 读取 `project.yml` 中的记录版本。
  - 将当前 `.agent/` 和 `harness/` 的**结构签名**与预期基线对比（基线由版本标记选择的内置模板快照定义）。
  - 输出差异分类：`new_file`（新执法器未被复制）、`signature_changed`（现有文件语义改变）、`removed`（上游删除了项目仍有的文件）。
  - **不写入任何内容**——只是告知用户存在偏移。

**v2 — 可升级性证明（安全应用）**：
- `forge governance upgrade --diff` 模式：生成特定于项目的升级计划，类似于 `forge migrate` 的 `--apply` 语义：先打印计划，再 `--apply` 才改。
- 变更按兼容性分类：
  - **向后兼容**（新模式/门/执法器）：合并到现有项目中——本质上是 *更严格* 的治理。
  - **破坏性**（`modes.yml` 结构变更、删除的 gate）：需要用户确认，默认跳过。
- 升级后运行完整的 `forge gate` + `forge check` 以验证没有损坏。
- `forge evolve` 在启动时可选地检查版本偏移（`forge evolve --check-governance-upgrade`）。

**v3 — 持续治理订阅**：
- 项目可选择一个"治理频道"：`stable`、`candidate`、`edge`。
- `forge evolve` 每隔 N 次迭代自动检查上游模板是否有更新的谐波，如果有，建议通过 `forge governance upgrade` 应用它们。

### 边界情况

- **fork/定制化项目**：项目故意定制了 `.agent/` 文件（新 agent 卡、自定义 workflow）。升级机制必须识别故意偏离并避免覆盖。与 `forge migrate` 的 `Plan` 比较一样，升级计划应标注 `[custom]` 文件并跳过它们。
- **模块化升级**：项目只想升级 `secret-scan.mjs` 但不改变 `modes.yml`。版本标记应该是每个组件的（`harness_version`、`agent_version`），而不是一个全局标记加上特定覆盖。
- **ForgeOS 自身**：当 ForgeOS 仓库 `forge migrate` 自身时，它不应报告 `forgeos_version` 漂移（因为当前版本是开发 HEAD）。快速检查：如果 `project.yml` 的 `forgeos_version` 等于当前 ForgeOS 版本（或 `dev`），跳过漂移报告。

---

## 方向三 · 模型/执行器无关抽象层 —— 从克劳德耦合到供应商无关编排

> **优先级**: 🟢 **P2** | **类别**: 架构 · 可扩展性 | **关键词验证**: `model.*agnostic\|executor.*abstraction\|vendor.*neutral` → **0 篇命中**; `multi.*provider\|LLM.*pool\|model.*router.*abstract` → 2 篇提及但为 ROADMAP 级的"将来做"，无代码级分析

### 问题

当前 forge-core 在多个层面对 `claude` CLI 有硬编码知识，形成一个跨越 4 个包的隐式耦合：

```
                    cmd/forge/
                     cost.go            ─── 解析 claude JSON 信封
                     engine_build.go    ─── 构造 claude CLI 参数（--model --permission-mode --allowed-tools）
                     prompt_context.go  ─── 注入特定于 claude 的上下文格式
                     prompt_artifacts.go─── 模板引用（.ai/prompts/）
                    internal/
                     orchestrator/
                      command_executor.go── shell 出 claude 命令
                      executor.go        ─── Action.Strings() 输出 "claude" 作为默认命令
```

具体来说：

**1. `cost.go` 解析 claude 特有的 JSON 输出格式**：
```go
// cmd/forge/cost.go:25-40
// 知道 claude -p --output-format json 的返回结构
// 解析 result.total_cost_usd、result.usage.output_tokens 等字段
type claudeResult struct {
    TotalCostUSD *float64 `json:"total_cost_usd"` // claude 特有的字段名
    ...
}
```

**2. `engine_build.go` 构造 claude CLI 参数**：
```go
// cmd/forge/engine_build.go:90-120
// agentExecutor 的 Exec.Execute 对 claude-family 添加:
//   --model <tier>           ← claude 特定的模型名
//   --permission-mode acceptEdits  ← claude 特有的权限模型
//   --max-budget-usd <n>     ← claude 特有的预算参数
//   --allowed-tools "..."    ← claude 特有的工具白名单格式
```

**3. `routing.go` 输出 claude 特定模型名**：
```go
// internal/routing/routing.go:15-20
const (
    TierHaiku  = "claude-sonnet-4-20250514"  // 硬编码完整的模型标识符
    TierSonnet = "claude-sonnet-4-20250514"
    TierOpus   = "claude-opus-4-20250514"
)
```

这种耦合使得以下场景不可能实现或非常困难：
- 使用 Codex CLI 代替 claude
- 使用 Gemini CLI 代替 claude
- 使用本地开源模型（ollama + continue.dev 等）
- 使用 OpenCode 或 OpenHands 作为后端
- 跨供应商成本比较——切换供应商需要修改 4 个包的代码

### 代码证据

**路由系统输出模型标识符而非抽象层级** — `internal/routing/routing.go:62-72`:
```go
func TierFor(mode string, agentTier string) string {
    // 返回 "claude-sonnet-4-20250514" 等——不是 "sonnet" 而是完整模型名
}
```

**agentExecutor 组装供应商特定的 CLI 标志** — `cmd/forge/engine_build.go:Executor`:
```go
// agentCmd 硬编码为 "claude"，通过 --agent-cmd flag 可覆盖，但：
// 1) 非 claude 的 CLI 很可能有不同的标志名称
// 2) --model 标志是 claude 特定的
// 3) --permission-mode 是 claude 特有的
// 4) --allowed-tools 语法是 claude 特有的
```

**`orchestrator/command_executor.go` 的 `Execute` 方法**：
```go
// 构建 os/exec.Command，硬编码将 argv 解释为 claude CLI 参数
// 如果 --agent-cmd 被设为 "codex" 或 "gemini"，标志会被静默发送给不理解它们的 CLI
```

**ROADMAP.md 的 v3 描述说"跨厂商池(LiteLLM)"**——但通往那里的架构步骤是空的。从今天直接跳到 LiteLLM 是一个巨大的跳跃，没有任何中间抽象。

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **供应商锁定** | ForgeOS 的核心编排层硬编码了对 Anthropic claude 的依赖。如果 claude API 不再可用（定价变更、服务终止、出口管制），目前没有干净的替代方案。 |
| **经济性** | 不同的 LLM 提供商对各种任务有不同的成本/质量曲线。路由（`internal/routing`）在抽象层级工作（haiku/sonnet/opus），但执行器将它们具体化为 claude 模型名称。映射"haiku 级成本"→"gemini-2.0-flash"或"gpt-4o-mini"需要重新布线。 |
| **本地/离线 LLM** | 许多组织不能将源代码发送给外部 API。Ollama/vLLM 本地推理是必须的。目前，`--agent-cmd` 可以被覆盖，但参数格式仍然与 claude 绑定。 |
| **v3 ROADMAP 的准备** | LiteLLM 不是一个魔术开关——它需要一个抽象层来放置它。目前要到达那里需要重写 4 个包。 |

### 建议方向

**v1 — 抽象模型层（现有结构可附加）**：
- 引入 `internal/model` 包，定义：
  - `Tier` 类型："haiku"|"sonnet"|"opus"（抽象层）。
  - `Provider` 类型："claude"|"codex"|"gemini"|"ollama"（由配置选择）。
  - `CLISpec` 接口：给定一个 `Tier` 和 `Provider`，产生供应商特定的 CLI 参数。
- 将 `routing.TierFor` 改为返回 `Tier`（"sonnet"）而非完整模型名称。下游的 `CLISpec` 解析器将其映射到供应商特定的标识符。
- 将 `engine_build.go` 中的 claude 特定参数构造提取到 `internal/model/claude.go`（第一个 `CLISpec` 实现）。
- **默认值「claude」保持不变**，因此所有现有行为逐字节保留。

**v2 — 可插拔提供者**：
- `project.yml` 新增 `provider:` 字段（`provider: claude|codex|gemini|ollama`）。
- 根据提供商选择在运行时切换到不同的 `CLISpec` 实现。
- `cost.go` 通用化——每个 `CLISpec` 也提供一个 `CostParser`，用于解析该提供商的 JSON 输出并提取成本/使用量。
- `forge detect` 可以探测环境中有哪些 CLI 并建议供应商。

**v3 — 多供应商池**：
- 在 `project.yml` 中声明多个提供者（`providers: [{claude: weight 0.7}, {gemini: weight 0.3}]`。
- 评分卡附加一个 `provider` 维度以跟踪每个提供商每层级的成本/质量。
- 路由器考虑提供商可用性并相应分配。

### 边界情况

- **供应商特定能力**：Codex 可能有 `--edit-format` 而 claude 没有；Gemini 可能有 `--safety-thresholds`。`CLISpec` 应遵循功能检测/降级模式，而非假设了一组通用超集标志。
- **成本格式差异**：不同的 API 以不同的格式返回成本（claude = `total_cost_usd` float · OpenAI = `usage.total_tokens` + 每 token 定价表 · Gemini = `usageMetadata.promptTokenCount`）。`CostParser` 接口必须能够处理完全不同的输入形状。
- **架构约束**：`internal/routing` 目前是零依赖的叶子包。引入 `internal/model` 必须保持相同的叶子不 import 状态。`routing.TierFor` 保留返回 `Tier`（字符串），model 包负责 `Tier → CLI 参数` 映射。

---

## 方向四 · 确定性 Trace 回放引擎 —— 过去运行的模拟调试

> **优先级**: 🔵 **P3** | **类别**: 可观测性 · 调试 | **关键词验证**: `deterministic.*replay` → 6 篇提及（但均为"需要回放能力"的一句话，无架构）; `trace.*analytics\|converge.*forensic` → 3 篇提及（浅层）; `simulat.*engine` → 1 篇（提及仿真但作为投机性 v3）; **无人提出从已记录的 trace.jsonl 构建确定性回放引擎的代码级方案**

### 问题

ForgeOS 拥有极其丰富的事件日志——`trace.jsonl` 记录了每一次迭代、gate 裁决、agent 调用、收敛检查和运行时决策——以及 `checkpoint.json` + `.N` 历史记录，再加上 `memory.jsonl` 的加速日志。这些数据目前仅用于：
1. 实时可观测性（`forge doctor` 检查其健康状态）。
2. 事后审计（手动 `jq` 查询 trace）。
3. 评分卡构建（`scorecard_wind.go` 扫描 trace 以获取 cost/latency 指标）。

**没有人使用的功能**：通过这些数据**重新运行** past evolve 循环，以理解**为什么**收敛——或没有收敛——发生在它发生的时候。具体来说，今天无法回答这些问题：

```text
forge evolve 跑了 43 次迭代然后收敛于 iteration 43。

Q1: 在 iteration 23 发生了什么导致了 roadmap_completion 从 55% 跳升到 82%？
     → trace 有事件，但你必须 grep、解析，然后推断来源

Q2: 如果我将 lifecycle 从 "mvp" 改为 "production"，相同的 trace 会在 iteration 36 收敛还是 52？
     → 不可能知道。你必须重新运行整个 evolve，花费真实的 API 费用

Q3: iteration 17 时 FileDelta 与 RoadmapCompletion 偏差 40% ——是 agent 夸大了进展还是 FileDelta 启发式漏了匹配？
     → trace 有 FileDelta 的数值，但没有逐项调试

Q4: 我升级了收敛引擎（internal/converge）——它是否会对过去一个月所有可用的 trace 输出相同的结果？
     → 无法验证。你必须重新运行所有的 evolve，但 agent 调用不可重复。
```

### 代码证据

**Trace 已经包含了重建所需的所有信号** — `internal/trace/trace.go` `Event` 结构体：
```go
type Event struct {
    Seq          int    // 单调递增：控制流排序
    Kind         string // "iteration" "agent" "gate" "decision" "converge" "error"
    Name         string // phase/gate 名称
    Status       string // PASS/FAIL/MET/NOT_MET/ok/stale/…
    DurationMs   int64  // 墙钟
    CostUsdMicros int64 // 实际 API 成本
    Model        string // 使用的模型
    Detail       string // 自由文本：裁决输出、决策依据
}
```

**Checkpoint 历史记录保存迭代间状态** — `internal/persist/checkpoint.go`:
```go
// 保留=5 保存了 checkpoints 1-5 的旋转链，因此迭代收敛趋势可重建
type Checkpoint struct {
    Iteration         int     // 哪个迭代
    RoadmapCompletion float64 // 当时的完成度
    GatesGreen        bool    // 当时的 gate 绿色状态
    PhaseIndex        int     // 当时的 phase 索引
    SpentUsdMicros    int64   // 当时的成本
    Reason            string  // 为什么停止/继续
}
```

**Memory 记录因果背景** — `internal/memory/memory.go`:
```go
type Entry struct {
    Kind      string // "gap" "decision" "lesson"
    Topic     string // 主题（可查询）
    Detail    string // 为什么记录此条目
    Iteration int    // 哪个迭代
}
```

**但没有任何东西将它们连接成一个可重放的时间线**。`scorecard_wind.go` 是唯一读取 trace 的东西，它只提取 cost/latency，而不是状态转换。

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **调试复杂收敛问题** | 当 evolve 收敛时间过长或过早，或产生意外结果时，回放引擎可以让开发者有洞察力地"逐步执行"过去运行的迭代，看到每次迭代的状态信号如何变化以及收敛检查如何裁决。 |
| **回归检测** | 当 `internal/converge` 或 `internal/routing` 中的逻辑发生变化时，针对过去 N 次真实运行的 trace 回放，可以验证新逻辑是否与旧逻辑做出相同的决定——在部署之前。实际上就是收敛引擎的 CI/CD 测试。 |
| **"what-if" 分析** | 如果用户探索不同的 mode/lifecycle 设置："如果我在那次运行时使用 `engineering` mode 而不是 `balanced`，我会收敛得更快吗？" 回放器可以在相同的 trace 事件上模拟不同的 mode 策略——不产生 LLM 成本。 |
| **审计 / 合规** | "证明 AI 编码系统的收敛决策是合理的。" 回放器可以严格证明，给定事件序列 X，收敛检查正确地在迭代 43 停止，因为（并且只因为）标准 Y 得到了满足。 |

### 建议方向

**v1 — 纯回放分析器（只读、离线）**：
- 工具位于 `harness/trace-replay.mjs`，读取 `trace.jsonl` + `checkpoint.json` + `memory.jsonl`，输出迭代间信号变化的结构化摘要。
- 输出格式示例：

  ```json
  [
    {
      "iteration": 17,
      "roadmap_completion": 0.55,
      "gates_green": true,
      "agents": [
        {"phase": "planner", "model": "sonnet", "cost_usd": 0.12, "duration_s": 45},
        {"phase": "implementer", "model": "haiku", "cost_usd": 0.18, "duration_s": 120}
      ],
      "converge_verdict": "NOT_MET",
      "converge_reason": "roadmap_completion 55% < 100%",
      "events": [
        {"seq": 142, "kind": "gate", "name": "test", "status": "PASS"},
        {"seq": 150, "kind": "decision", "detail": "budget adjust: tier down to haiku"}
      ]
    }
  ]
  ```

- **不合成事件**，不重放 agent 调用，不修改任何文件。纯粹的增量分析。
- 不包括未来成本预测或 mode 模拟——只保留面的事实摘要。

**v2 — 决定模拟**：
- 给定原始 trace 事件（从 `Kind:"converge"` 和 `Kind:"decision"` 事件捕获的收敛检查输入），针对**当前**版本的 `internal/converge` 重新计算 `Converge(stop, signals)`。
- 报告差异：`"iteration 23: original verdict = NOT_MET, re-simulated = MET (roadmap=100% now satisfies stop condition)"`——解释模式或收敛逻辑变化的影响。
- 模拟输出可以写入分片（trace-like）文件，不修改原始运行记录。

**v3 — 假设模拟（供应商/模式选择器）**：
- 给定原始 trace 事件（包括从 trace 中重建的 `Signals` 输入），模拟不同的 mode（`balanced`→`engineering`）或 lifecycle（`mvp`→`production`）设置如何影响收敛进度。
- 报告：`"如果使用 engineering mode，迭代 23 时的 review_status 会被考虑，并且迭代 23 就会收敛（而不是迭代 43）"`——无成本的 what-if 分析。
- 警告：仅当 mode 变化不影响 agent 行为（只影响 gating）时才有效。如果 engineering mode 运行额外的 reviewer phase（这会改变 agent 输出），模拟是不完整的——对此诚实标注。

### 边界情况

- **Trace 格式变更**：回放器必须处理同一文件中的多个 _format 版本（v1 与 v2 事件一起）。回放开始时读取第一个事件的 `_format`，并使用它来选择解析器路径。
- **不完整的 trace**：崩溃会导致 trace 以事件丢失结束。回放器应报告最后完整事件的序列号，并诚实标注为 `[trace truncated: seq 142 is the last complete event]`。
- **Agent 调用不可重复**：自动特征提取（`FromChangedPaths`）可以由模拟器重新计算。Agent 输出是 LLM API 调用产物，永远无法重建。回放器从不假装重放 agent 阶段——它只回放**控制流决策**（gate 裁决、收敛检查、预算守卫、模式门控）。
- **隐私**：trace 可能包含来自 agent `Detail` 字段的敏感信息。回放器输出应该是可配置的以屏蔽/省略自由文本字段。

---

## 方向五 · 优雅降级架构 —— 从全有或全无可用性模型到偏序系统

> **优先级**: 🔵 **P3** | **类别**: 韧性 · 运行时自治 | **关键词验证**: `graceful.*degrad\|partial.*avail\|degraded.*mode\|fail.*soft` → 4 篇（浅层提及，无架构）; `fail.*open.*strategy\|degraded.*operation` → 1 篇（仅在注释中提到具体 gate）; **无系统级降级模型分析**

### 问题

ForgeOS 当前在其每个子系统上采用**基本上二元**的可用性模型——一个资源要么可用要么不可用，没有中间状态。在实践中，修复工具/依赖所需的可用性程度通常是**部分**的：

| 子系统 | 二元模型 | 现实可能性 |
|---|---|---|
| Harness gates（lint/coverage/sca） | 可用（PASS）或 N/A | lint 引擎已安装但配置不完整（eslint 存在但无 `.eslintrc`）→ coverage 有工具但特定安装路径因权限被拒→ sca 有 DB 但数据已过时 2 个月 |
| Agent executor（claude） | 可用（`claude --version` 成功）或不可用 | claude CLI 存在但 API rate-limited（529）→ OAuth token 即将过期（设备代码流）→ claude CLI 存在但指向已弃用的 API 版本 |
| YAML→JSON 转换 | Go 原生解析器成功或回退到 Python shim | Python shim 存在但 PyYAML 版本太旧不支持 workflow 使用的 YAML 特性→ PATH 中有 Python 但 import 失败 |
| `.forge/` 运行时目录 | 可写或不可写 | 磁盘可用空间低（仍有 200MB 但可写）→ NFS 挂载延迟大但可用→ 目录存在但权限只允许读取/追加但不允许重命名（checkpoint 的 Save 会失败） |
| Git 操作 | 仓库可用或不可用 | 并非 git 仓库（`forge init` 后立即阶段）→ `git diff` 有冲突→ 浅克隆没有完整历史（影响 FileDelta 计算） |

当前每个子系统在此处返回一个**二元**答案：
- `gate.Result.Status` = `PASS` | `FAIL` | `N/A` —— 第三态存在但只适用于"探测完全不运行"的情况，而非"部分运行"
- `doctor.Check.OK` = `true` | `false` —— 健康检查：PASS 或 FAIL，无降级
- `asset.LoadWorkflowJSON` = 负载或错误 —— 无"部分负载"概念
- `memory.Load` = `[]Entry, error` —— 一个坏行使整个文件不可读

缺乏降级模型在 24h 自主运行时引发级联故障：

```
🌆 凌晨 3 点，forge evolve 运行到迭代 18
  ↓
📉 磁盘使用率 95%（checkpoint 因 ENOSPC 写入失败）
  ↓
⚠️ checkpointSave("写入失败，继续") = 打印错误并继续
  ↓
⏰ 上午 7 点：断电
  ↓
🔄 `forge evolve --resume`：最后可读的 checkpoint 来自迭代 15
     迭代 16-18 丢失，3 次迭代的 agent 费用（约 3x ~$1.20 = $3.60）被浪费
  ↓
❌ 用户发现财务浪费是在 3 天后，当 API 账单出账时
```

如果 forge 在 95% 磁盘时警告并主动降级到**仅保留状态**（在内存中运行收敛检查，将检查点写入临时备份），损失就可避免。当前的"写入失败，继续"方法意味着在第一个指标发出红色信号后，所有后续可观测性都是不可靠的。

### 代码证据

**checkpoint 是 fail-loud-and-continue** — `cmd/forge/evolve.go` 的 `checkpointHook`:
```go
// 记录错误但从不中止 evolve：
if err := persist.Save(...); err != nil {
    fmt.Fprintf(os.Stderr, "⚠ checkpoint save failed: %v (continuing)\n", err)
}
// 没有降级操作——没有切换到后备路径，没有状态变化
```

**doctor 运行在所有/无的基础上** — `internal/doctor/doctor.go`:
```go
func Run(root string) Report {
    if _, err := os.Stat(dotForge); os.IsNotExist(err) {
        return Report{NoForgeDir: true}
        // 报告全部缺失——没有"部分 forge 目录"的概念（例如 .forge/ 存在但 checkpoint.json 缺失）
    }
}
```

**trace 写入失败是 fail-closed** — `internal/trace/trace.go`:
```go
func (t *Tracer) Emit(ev Event) error {
    if _, err := t.w.Write(line); err != nil {
        return fmt.Errorf("trace: writing event seq=%d: %w", ev.Seq, err)
        // 如果 trace.jsonl 不可写，错误冒泡到上层，可能 abort 整个 phase
    }
}
```

**memory 有一个坏行就致命** — `internal/memory/memory.go`:
```go
func decode(data []byte) ([]Entry, error) {
    // 一行格式错误 → 整个文件不可读（全有或全无）
    if err := json.Unmarshal(raw, &e); err != nil {
        return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
    }
}
```

**没有子系统健康/降级注册表**：
```bash
$ grep -r "health\|degraded\|status\|alive\|readiness\|liveness" internal/ --include="*.go" | grep -v test | grep -v "_test" | wc -l
# 约 12 条结果——大多数是 gate result.Status，没有一个系统注册表
```

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **24h 自主韧性** | 24h 运行中工具/API/磁盘/网络退化的概率接近 1。没有降级模型意味着每次退化都是潜在的运行终止。 |
| **成本浪费** | 发生在不可恢复的子系统故障后的运行（磁盘满→checkpoint 丢失→断电→3 次迭代的 agent 成本浪费）。降级可以防止这种浪费。 |
| **用户信任** | 当 forge 遇到可恢复问题时输出"错误：… 继续"但随后崩溃，用户会失去信心。降级模型提供可预测的行为："磁盘使用率 >90%：forge 进入"保留"模式——状态保留但不启动新的 agent 阶段"。 |
| **N/A 治理不是降级模型** | 现有 N/A/FAIL/PASS 的三态 gate 模型是诚实但不完整的。它说"此检查未运行"。它没有说"此检查未运行因为……这里的建议是……"。降级模型应携带可操作的建议，而不仅仅是缺少结果。 |

### 建议方向

**v1 — 子系统健康注册表**：
- 新增包 `internal/health`（纯数据，不 I/O），定义：
  ```go
  type Status int
  const (
      StatusHealthy Status = iota   // 完全运行：所有必需的依赖都可用
      StatusDegraded                // 部分运行：核心功能工作，非核心降级
      StatusUnavailable             // 不可运行：核心依赖缺失
  )
  type Report map[string]SubsystemHealth
  type SubsystemHealth struct {
      Status   Status
      Detail   string   // 为什么在此状态
      Advice   string   // 用户可操作的建议以恢复健康
      CanRun   bool     // forge 是否可以以当前 SubsystemHealth 启动 run？
  }
  ```
- 已知的子系统注册：`harness_gates`、`agent_executor`、`yaml2json`、`forge_dir`、`git`、`python_shim`、`sca_db`。
- `forge preflight` 运行健康注册表并报告每个子系统的状态。

**v2 — 降级感知执行**：
- 在入口点（`execEngine` 前，`cmd/forge/main.go`）进行健康检查，构建运行前健康快照。
- 健康状态影响行为：
  - `CanRun=false` → `forge run` 拒绝启动，ful 报告"在启动前解决这些问题"。
  - `StatusDegraded` → `forge run` 启动但抑制非关键功能：
    - 如果 `sca_db` 降级：跳过 SCA 门控（已经是 N/A，但降级模式使其清晰）。
    - 如果 `forge_dir` 降级（磁盘 >90%）：绕过 checkpoint 写入（或切换到内存后备）。
    - 如果 `agent_executor` 降级但可用（529）：增加退避，降低最大重试。
- `forge status` 输出运行中 run 的当前健康快照。

**v3 — 弹性配置**：
- `project.yml` 新增 `resilience:` 块，允许用户配置降级策略：
  ```yaml
  resilience:
    disk_space:
      warning_threshold: 85%         # 在此处开始记录警告
      degrade_threshold: 92%         # 在此处进入降级模式：停止 checkpoint 写入
      abort_threshold: 97%           # 在此处中止运行（安全失败）
    agent_rate_limit:
      max_retries: 5                 # 超过默认值 3，适用于易变 API
      fallback_provider: codex       # 如果 claude 持续 529，切换到 codex
    missing_gate:
      default_action: warn           # 而不是 N/A，对缺失门控发出警告
  ```

### 边界情况

- **亚健康检测成本**：健康检查本身不应显著增加延迟或 API 成本。`claude --version` 调用是即时的；`df` 是微秒级的；但检查 SCA DB 新鲜度可能需要远程获取——应被缓存或延迟。
- **回环健康状态**：如果健康注册表本身是降级的（健康子系统的磁盘已满会怎样？）？健康检查应该是最小依赖的——纯 Go，无外部文件写入。
- **v1 健康注册表与 `forge doctor` 重叠**：`doctor` 已经执行了其中一些检查。健康注册表应**重用** `doctor` 的检查逻辑，而不是复制它。自然的演进是 `doctor` 成为健康注册表的 CLI 前端。

---

## 跨方向协同效应

| 方向 | 对现有基础设施的依赖性 | 所需的新基础设施 | 与其他方向的协同效应 |
|---|---|---|---|
| 方向一：多进程安全 | `internal/persist` · `internal/trace` · `internal/memory` | 进程级 `flock`, `.forge/.lock`, 分支隔离解析器 | 方向五的健康注册表应报告锁状态 |
| 方向二：治理版本偏移 | `forge-init` · `forge-upgrade` · `project.yml` | `project.yml` 中新增 `forgeos_version`, `internal/versionskew` 包 | 方向五的健康注册表应报告版本漂移状态 |
| 方向三：模型抽象 | `internal/routing` · `engine_build.go` · `cost.go` | `internal/model` 包, `CLISpec` 接口 | 与方向五的"代理执行器健康"交织 |
| 方向四：Trace 回放 | `trace.jsonl` · `checkpoint.json` · `memory.jsonl` | `harness/trace-replay.mjs`, 收敛模拟器 | 方向二的版本偏移升级可用方向四验证新收敛逻辑对旧 trace 的兼容性 |
| 方向五：优雅降级 | `forge doctor` · `forge preflight` · `internal/doctor` | `internal/health` 包, `project.yml` 中的 `resilience:` 块 | 方向一的锁状态, 方向二的版本漂移, 方向三的提供者健康都是健康注册表的输入 |

---

## 总结：优先级与投资回报

| 方向 | 类别 | 代码复杂度 | 用户价值 | 风险 |
|---|---|---|---|---|
| 方向一：多进程运行时安全 | **韧性/数据完整性** | 低（~80 行 Go：一个锁包装器 + CLI 入口集成） | 高（防止数据损坏、经济损失和 CI/CD 碰撞） | 低；`flock` 是标准且稳定的；备用方案适用于 NFS |
| 方向二：治理版本偏移 | **治理/生命周期** | 中（~300 行：`project.yml` 版本记录 + 漂移检测器 + 升级规划器） | 高（解锁信任升级，防止安全退化） | 中；差异合并复杂，v1 只读版本降低风险 |
| 方向三：模型抽象 | **架构/解耦** | 中高（~500 行：`internal/model` 包 + `CLISpec` 接口 + 重构两个现有包） | 高（解锁 v3 ROADMAP 的多提供商支持，消除供应商锁定） | 中；接口设计需要谨慎，否则会限制未来的提供者 |
| 方向四：Trace 回放 | **可观测性/调试** | 中（~400 行 JS：trace 分析器 + 收敛模拟器） | 中高（加速调试，启用回归检测，无成本"what-if"分析） | 低；纯离线，无写入，无状态修改 |
| 方向五：优雅降级 | **韧性/运行时自治** | 高（~800 行：健康注册表 + 降级策略 + 配置模式 + 跨 6+ 包集成） | 高（24h 自主运行的核心，防止级联故障） | 中；接口需要跨包断裂，可能引入微妙的路径 |

**短期投资（S—2 sprints）**：方向一（多进程安全）——交付风险比最高。~80 行 Go，解决一个可证明的数据完整性问题，已成为 CI/CD 障碍。

**中期投资（S—4 sprints）**：方向二（版本偏移）+ 方向四（Trace 回放）——共同创建"可升级且可审计的 ForgeOS"治理。版本漂移检测识别问题；Trace 回放验证升级安全。

**长期投资（S—6 sprints）**：方向三（模型抽象）+ 方向五（降级）——架构性投资，直接支持 ROADMAP v3（多提供商、生产级 24h 运行时）的交付。

---

*分析日期：2026-07-11 | 基于 forge-core 全量源码扫描（第五轮，聚焦运行时基座的 5 个未被当前 120+ 份分析覆盖的缺口）*
