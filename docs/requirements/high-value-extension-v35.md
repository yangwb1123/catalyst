# ForgeOS — 第 35 轮深扫：五个全新扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描 —— forge-core 19 Go 包 / 195+ 源文件 / 400+ 测试 /  
>   harness 34 模块 / `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 工作流 + policies）/  
>   Sprint 1–31 完整演进 / `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（约 90 DONE + GAP 全部收口）/  
>   **逐篇交叉核对 40+ 篇 `docs/analysis/*.md` + 19 篇 `docs/requirements/*.md`（~65+ 已有方向）**，  
>   确保每个方向的核心论点与已有分析**不重叠**。  
> **纪律**: 不编写任何代码。每个方向附具体代码级证据 + 与已有分析的差异证明。  
> **日期**: 2026-07-10

---

## 与 ~65+ 已有方向的全景差异表

本文**不重复**以下已被充分覆盖的域。每个新方向末尾附差异证明段落，引用已有文档中最接近的论点并解释为什么不重复。

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/信号/诊断） | `high-value-extension-directions.md` · `v3` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈层） | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/持久语义/可移植性） | `strategic-extensions-v22~v32.md` · `uncovered-frontiers-v25.md` | ~10 |
| 安全/凭据/secret 生命周期/SCA/沙箱 | `genuinely-novel-expansion-directions.md` | ~5 |
| CLI DX / shell 集成 / daemon 模式 / 增量采纳 | `extension-frontier-five.md` · `expansion-self-governance.md` | ~5 |
| 并行编排 / 迭代跳过 / 收敛可见性 / YAML 差分测试 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 结构化输出 | `next-five-frontiers.md` · `forgotten-frontiers-five.md` | ~8 |
| `.forge/` 并发写入隔离（三层模型：独立目录 + 分布式锁 + 可见性注册表） | `expansion-blind-spots-v15.md` 方向六 | ~1 |
| Go 原生 YAML 解析消除 Python shim | `eighth-wave-adr-decay.md` 方向五 · `edgecases-and-perf.md` §4.3 | ~2 |
| 跨阶段语义一致性守卫 / 合约注册表 | `novel-extensions-v12-architect-perspective.md` 方向二 | ~1 |
| **总计已有覆盖** | | **~68 方向** |

---

## 方向一：外部 SDLC 集成层——从本地执行到「软件工厂」

### 为什么这是高价值的

ForgeOS 当前完全运行在本地文件系统上：它读写本地 git 工作树、运行本地命令、检查本地 git 状态。**没有任何与外部 SDLC 工具（GitHub/GitLab/Bitbucket）的 API 集成。**

这意味着：

- ForgeOS 可以为你的仓库写代码，但**不能创建 PR/MR**。agent 产出代码变更后，你必须手动 `git commit` + `git push` + 打开 PR。
- **不能读取 CI 状态**。agent 可以本地跑 `forge accept`，但不能知道远程 CI 是否通过。
- **不能自动合并**。即使所有 gate 通过，也没有自动 merge 到 main 的路径。
- **不能审查 PR**。agent 可以做代码审查，但结果无法自动发布到 PR comment。
- **不能在 CI 中因果使用**。CI 触发 → forge 分析 → forge 创建 PR → 等待人类 → forge 更新 PR → … 这条链路完全不存在。

这使 ForgeOS 停留在「本地开发辅助工具」而非「AI 软件工厂」。北向架构（`north-star.md`）的 15 个服务中，`API Gateway`、`VCS/Artifact` 两个服务直接依赖外部 SDLC 集成，且 `Agent Registry`、`Observability` 也受益于它。

### 代码级证据

**证据 1：没有任何 HTTP/gRPC 客户端代码**

```bash
# forge-core 全部 19 个包——零 HTTP 客户端、零外部 API 调用
$ grep -rn "http\.\(Get\|Post\|Client\|NewRequest\)" forge-core/ --include="*.go" | grep -v _test | grep -v "net/http/pprof"
# 无输出 —— forge-core 不调任何外部 API
```

`go.mod` 确认零外部依赖（无 `require` 块），所有功能都基于本地文件系统和本地命令执行。

**证据 2：SDLC 相关的 agent 卡只有「本地」叙述**

检查 `reviewer.md`、`qa.md`、`cto.md` 等 agent 卡，它们全部以「写文件到本地目录」作为输出方式：

- `reviewer.md`: 输出 → `docs/review/` 目录
- `qa.md`: 输出 → `## QA Report`（文本报告）
- `cto.md`: 输出 → `executive-review.md` 文件

没有任何 agent 卡提及「创建 PR」「评论 PR」「检查 CI 状态」「合并分支」等外部 SDLC 操作。

**证据 3：工作流没有「发布」阶段**

5 个工作流（discover/design/build/review/evolve）都以本地文件变更结束。`build.yml` 的最终 phase（qa）输出是一个文件报告——代码已经在工作树上，但从未被推送到远程或变成 PR。

**证据 4：`forge-init` 生成的项目是孤岛**

`forge-init` 创建新项目时复制全套治理资产，但生成的 CI（`.github/workflows/forge.yml`）只跑本地检查。它不创建 PR、不评论、不合并。

### 核心设计思路

引入一个轻量的 **VCS 适配器层**，抽象 SDLC 操作：

```
internal/vcs/
  adapter.go    — VCSAdapter 接口
  github.go     — GitHub/GitHub Enterprise 实现
  gitlab.go     — GitLab/GitLab Self-Managed 实现
  local.go      — 当前行为：本地 git 操作，无 API 调用（向后兼容默认）
```

`VCSAdapter` 接口的核心操作：

```go
type VCSAdapter interface {
    CreateBranch(name, base string) error
    CreatePR(title, description, head, base string) (PRRef, error)
    GetCIStatus(ref string) (CIStatus, error)
    CommentOnPR(prRef, body string) error
    MergePR(prRef string) error
    GetChangedFiles(prRef string) ([]string, error)
    PostCheckRun(name, status, conclusion string, detailsURL string) error
}
```

各层的接法：

1. **`cmd/forge` 层**：新增 `--vcs` flag 选择适配器（默认 `local` 保持向后兼容），`--vcs-token` 提供 API token。
2. **工作流层**：`build.yml` 可声明 `emit: publish` 阶段，在 qa 通过后创建 PR。`review.yml` 可声明 `publish_review: true` 在 PR 上发布审查意见。
3. **Converge 层**：收敛信号可包含 `ci_status == green` 作为 gate 条件（不仅是本地 gate）。
4. **Agent 卡层**：新增 `vcs-agent` 角色卡（或扩展 `implementer`/`planner`），赋予创建/更新 PR 的能力。

### 边界场景

| 场景 | 行为 |
|------|------|
| 无 VCS token/未配置 | `local` 适配器降级——当前行为不变，零回归 |
| PR 创建失败（重名/权限） | 失败日志 + 手动创建指路，不阻塞后续 phase |
| CI 一直 pending | 可配置超时 + 轮询间隔；超时视为 UNKNOWN 而非 FAIL |
| 多分支同时 evolve | 各分支独立 PR，VCS 适配器负责跟踪 |
| GitHub vs GitLab API 差异 | 适配器封装差异；`local` 适配器提供纯本地「伪 PR」（`.forge/pr-*.json`）供测试 |
| PR 审核发现需要修改 | `on_fail: loop_back → implementer` + 更新 PR commit（force push）|

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-horizon-three.md`：「第三地平线生态」涉及多仓库联邦和事件驱动管线。它聚焦**跨仓库编排**（仓库 A 的事件触发仓库 B 的 evolve），本文聚焦**单仓库的外部 SDLC 集成**（创建 PR/检查 CI/合并/评论）。
- `v2-to-northstar-gap.md`：列出了 15 个北极星服务的现状差距，包括 VCS/Artifact（❌ 不存在）。但它只做了差距评估，没有给出具体的设计思路、接口定义、边界场景和接法。

本文的独特性在于：给出了**完整的适配器接口定义、与现有工作流的接法、agent 卡扩展方向、边界场景表**——从「什么缺失」到「怎么接」的完整推理链。

---

## 方向二：Agent 输出行为回归检测——不止于「编译通过」

### 为什么这是高价值的

ForgeOS 当前的验证栈（harness gates）检查的是**结构属性**：

- 行数、文件数、目录结构（`gate.mjs`）
- 架构分层、循环依赖、扇入（`arch-check.mjs`）
- 硬编码 secret（`secret-scan.mjs`）
- 依赖漏洞（`sca.mjs`）
- 测试通过（`acceptance.mjs` 的 `test_pass` 和 `app_test_pass`）

但这些 gate **完全不检查 agent 代码的行为正确性**。在 `forge evolve` 的多迭代过程中，以下情况完全不被检测到：

1. **行为回归**：第 3 次迭代的 implementer 重构了一段代码，破坏了第 1 次迭代实现的功能。所有 gate 仍然通过——编译没问题、测试可能恰好没覆盖那条路径、架构检查也过了——但功能坏了。

2. **Agent 幻觉引入的错误**：agent 调用了一个不存在的方法签名、使用了错误的 API、或者实现了一个与 PRD 描述不一致的行为。`forge accept` 判 `ACCEPTED`，但代码是错的。

3. **跨迭代的架构侵蚀**：每次迭代只改一个局部，但 10 次迭代后系统的整体架构已经偏离了 `ARCHITECTURE.md` 的描述。没有 gate 能检测「架构漂移的累积效应」。

4. **PRD-代码追溯断裂**：agent 声称实现了某个需求，但没有 gate 验证需求到代码的可追溯性。PRD 中的 `MUST` 项可能从未被实现。

### 代码级证据

**证据 1：`internal/converge` 的收敛信号中没有行为指标**

`converge.go` 的 `Signals` 结构体包含 8 个字段：

```go
type Signals struct {
    RoadmapCompletion    float64     // roadmap 勾选比例
    GatesGreen           bool        // 所有 gate 通过
    RequirementConfidence float64    // discover 置信度
    ReviewStatus         string      // review 裁决
    FileDelta            float64     // git diff 文件匹配
    HumanApproved        bool        // 人工批准
    Criteria             map[string]string  // 各 criterion 裁决
    GateProof            GateProof
    CodeTestRatio        float64     // 测试代码占比
}
```

没有任何字段描述**行为的正确性**——没有 behavioral regression、没有 contract conformance、没有 requirements traceability。

**证据 2：`harness/acceptance.mjs` 的 criterion 全结构性的**

```bash
$ grep -A2 "criterion\|metric" harness/acceptance.mjs | head -20
# 实际检查的 metric：
# test_pass, app_test_pass, complexity_violations, arch_violations, 
# architecture, security_findings, dependency_vulnerabilities, lint_pass,
# typecheck_pass, build_pass, coverage_pass
# ——全是结构/编译/静态分析的，没有一个「行为」metric
```

**证据 3：`forge run build` 的真实执行路径没有差异化测试**

`engine_build.go` 中 `phaseTierResolver` 构建 prompt → 调 agent → 跑 gate，但 agent 产出的代码和上一迭代的代码之间的对比（`git diff`）只用于 `FileDelta`（粗糙的关键词匹配），不做任何语义分析。

**证据 4：没有 contract/property 测试框架**

ForgeOS 没有任何能力让用户声明「这个函数对输入 X 应返回 Y」或「API 端点的响应结构应符合 schema Z」——这些都是行为层面的契约，当前系统完全交给 agent 自行处理。

### 核心设计思路

在现有 gate 体系之上增加一个**轻量级行为验证层**——不替代现有 gate，而是在现有 gate 全部 PASS 后，额外验证代码的行为正确性。

```
当前 ── gate (结构) → converge (收敛判定)
目标 ── gate (结构) → behavior-verification (行为) → converge (收敛判定)
```

三种子策略（由简到繁，可增量采用）：

**策略 A：跨版本的差异化测试（`forge diff-test`）**

在每个 agent phase 完成后，自动跑 `git stash` → 跑一份测试快照 → `git stash pop` → 再跑测试 → 对比两轮测试输出。如果同一个测试在 agent 修改前 PASS、修改后 FAIL，说明引入了行为回归。

这不是「跑测试」——测试已经在 gate 中跑了。这是**跑同一套测试两次（基准 vs 新代码）并对比结果**，发现预存语义回归。

**策略 B：契约式测试断言（`forge contract`）**

用户（或 architect agent）在 `docs/design/CONTRACTS.md` 中声明关键接口的契约：

```yaml
contracts:
  - module: src/shortener-service.mjs
    property: "shorten(url) 返回的短码长度恒为 7"
    type: property-based
    command: "node -e '...'"
  - module: src/http-server.mjs
    property: "POST /shorten 返回 201 + JSON body"
    type: integration-test
    command: "node --test test/http.test.mjs"
```

新增 `forge contract` gate：解析契约文件，对每条契约执行验证命令。只有所有契约通过才算 `PASS`。

**策略 C：需求追溯性检查（`forge trace-requirements`）**

在 PRD 的 `MUST` 需求和代码之间建立追溯链。方法（轻量，非 ML）：

1. 解析 PRD 的 `MUST`/`SHOULD`/`COULD` 列表。
2. 从 ROADMAP 的 `- [x]` 已勾选项中提取关键词。
3. 从 git diff 中检查这些关键词是否出现在新增/修改的代码注释、测试名、函数名中。
4. 缺失的关键词 → 警告（非阻断，但记入收敛报告）。

### 边界场景

| 场景 | 行为 |
|------|------|
| 测试在 agent 修改前后都 FAIL | 不报告回归——gate 已经标记 FAIL |
| 契约测试所需的依赖未安装 | N/A（同现有 lint/coverage 适配器模式）|
| PRD 无结构化的 MUST 列表 | 追溯性检查跳过（N/A），不阻断 |
| property-based 测试超时或挂起 | 超时 → FAIL（阻断），fail-closed |
| 契约文件不存在 | 跳过（不影响任何现有 workflow）|
| 跨迭代多次修改同一函数 | 差异化测试对比相邻两次迭代；契约测试每次都跑 |

### 与已有分析的差异证明

最接近的已有分析：
- `novel-extensions-v12-architect-perspective.md` 方向二「跨阶段语义一致性守卫」：聚焦**跨 phase 的接口契约一致性**（planner 说要做 X，implementer 是否做了 X），使用合约注册表和关键词匹配。本文的策略 B（契约式测试断言）在终点上有相似性，但有三点本质差异：① 这里强调的是**可执行的 property testing**（agent 级的行为验证）而非跨 phase 文本匹配；② 策略 A 的**差异化测试**（stash 前后的测试结果对比）是已有分析从未提到过的思路；③ 策略 C 的**需求追溯性**从 PRD 出发而非从 planner 输出出发，弥补的是「spec→implementation」的断裂而非「plan→code」的断裂。
- `edgecases-and-perf.md` §3「收敛理论的隐藏陷阱」：讨论的是收敛判定本身的误判（零变更收敛），而非 agent 输出内容的正确性。
- `execution-semantic-gaps.md` 方向二「phase 输出契约的形状校验」：讨论 JSON Schema 级别的输出形状校验，而非行为的正确性。

---

## 方向三：并发工作树保护——双 Agent 互斥执行协议

### 为什么这是高价值的

ForgeOS 允许在不同终端启动多个 `forge run`/`forge evolve` 实例。当前没有任何机制阻止两个实例并发操作**同一个 git 工作树**。

`expansion-blind-spots-v15.md` 已经详细分析了 `.forge/` 目录（trace.jsonl/checkpoint.json/memory.jsonl）的并发写入问题，并提出了三层隔离模型。**本文的问题是不同且更危险的**：两个 evolve 循环各自 spawn 真 agent（`--executor=command --agent-cmd=claude`），两个 agent 同时编辑同一工作树的**源文件**。

这可能发生在：
- 开发者在终端 A 跑 `forge evolve build`，同时在终端 B 跑 `forge evolve build --resume`。
- CI/CD 管道和开发者本地实例同时操作同一 repo。
- 两个分支的 evolve 同时运行（如果工作树相同——比如 monorepo）。

后果：
- **合并冲突**：agent A 写 `src/api.ts` 第 50 行，agent B 写同一文件第 50 行——最后一个写的获胜，前一个的变更静默丢失。
- **破损的中间状态**：一个 gate phase 正在跑 `node --test`，另一个 agent 正在修改被测文件——测试结果不可靠。
- **gate 裁决不可靠**：gate.mjs 检查文件行数时，文件正在被另一个 agent 修改——一个 racing condition 产生假阴性或假阳性。

### 代码级证据

**证据 1：没有任何进程级互斥**

```bash
$ grep -rn "flock\|lockfile\|pidfile\|mutex.*file\|LockFile\|ExclusiveLock" forge-core/ --include="*.go" | grep -v _test
# 无输出 —— 不存在任何文件级互斥机制
```

`orchestrator` 包中的所有 `sync.Mutex` 都是**单进程内**的，保护的是共享内存中的数据结构（trace 写入、prompt ledgers、run budget）。它们对另一个进程完全不可见。

**证据 2：`CommandExecutor` 不检查工作树的所有权**

`command_executor.go` 的 `Run` 方法 spawn 子进程并直接在工作目录执行——它不知道是否有另一个 forge 实例在同一工作树中运行。

**证据 3：`evolve.go` 的 `cmdEvolve` 入口没有启动前检查**

```go
// evolve.go:57 cmdEvolve
func cmdEvolve(args []string) int {
    // ... 解析 flag、加载 workflow ...
    // 没有检查：是否有另一个 evolve 实例正在运行？
    return execLoop(ctx, wf, o, iter, src, *resume)
}
```

没有任何启动前检查（preflight check）验证当前工作树未被其他 forge 实例锁定。

**证据 4：`forge status` 不报告活跃实例**

`doctor/status.go` 收集 repo 状态（checkpoint 时效、trace 大小、最后运行时间），但不报告是否有其他 forge 进程正在活跃运行。

### 核心设计思路

在 `expansion-blind-spots-v15.md` 的 `.forge/` 存储隔离方案之上，增加一个**工作树级别的互斥协议**。两个方案是正交且互补的：存储隔离解决「多个 .forge 目录」问题，互斥协议解决「多个 agent 改同一文件」问题。

**轻量方案（v1）——乐观锁 + 启动检查：**

```
- forge run/evolve 启动时检测 .forge/exec.lock
- 如果锁文件不存在 → 创建（写入 pid + 工作流名称 + 启动时间）
- 如果锁文件存在且持有者存活 → 拒绝启动（打印：另一个 evolve 正在运行）
- 如果锁文件存在但持有者已死亡 → 警告 + 覆盖（可配置 --force）
- 进程退出时（正常/崩溃后清理）→ 删除锁文件
- 锁文件用 OS 级原子操作：Unix 上用 `flock(2)`, Node.js 上无直接等价
  故用 `O_CREAT|O_EXCL` open(2) + pid 文件
```

**增强方案（v2）——租约式互斥：**

```
- 锁文件含到期时间戳（租约），默认 30 分钟
- forge evolve 的后台 goroutine 每 5 分钟刷新租约
- 崩溃后，锁在 30 分钟后自动过期（无需手动清理）
- 新实例检测到旧锁 → 判断过期 → 过期则覆盖，未过期则等待/拒绝
```

**接入点：**

| 接入层 | 改动 | 复杂度 |
|--------|------|--------|
| `internal/gate/gate.go` | 新增 `TryAcquireExecLock(root string) (release func(), err error)` | ~80 行 |
| `evolve.go` `cmdEvolve` | 调用 `TryAcquireExecLock`，defer release | ~5 行 |
| `main.go` `cmdRun` | 同上 | ~5 行 |
| `doctor/status.go` | 读取 exec.lock 信息加入 `Status` | ~10 行 |
| `forge status` 输出 | 显示「当前有活跃 evolve: pid=12345, workflow=build」 | ~5 行 |

### 边界场景

| 场景 | 行为 |
|------|------|
| 同一 repo 两个不同工作流（一个 discover、一个 build） | 互斥锁不区分工作流：第二个启动就拒绝（简单安全）→ 或 v2 支持 read-only 工作流共享 |
| 同一 repo 两个不同分支 | 默认互斥（同一工作树）；如果工作树不同（`git worktree`）则不影响 |
| 崩溃后锁文件残留 | v1: 自动检测 pid 不存在 → 覆盖（`--force` 可跳过确认）。v2: 租约到期自动过期 |
| 两个 `forge run`（非 evolve）同时跑 | 同样适用：run 也修改工作树 |
| `forge status --watch` 常驻进程 | 持有一个 watch-only 锁（不阻塞读写操作），不干扰 evolve |

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-blind-spots-v15.md` 方向六「`.forge/` 并发写入隔离」：解决的是**存储层**的并发写入问题（trace.jsonl/checkpoint.json/memory.jsonl 因两进程交错写入而损坏），提出了「独立运行目录 + 分布式锁 + 可见性注册表」的三层模型。**本文方向的焦点完全不同**——解决的是**工作树层**的并发编辑问题（两个 agent 同时修改同一源文件造成的静默数据丢失）。这两个问题是正交的：存储隔离方案即使完美实施，工作树竞争仍然存在。两个方案互补，需要同时部署才能实现真正的多实例安全。
- `edgecases-and-perf.md` §2「长运行资源泄漏」：讨论子进程资源泄漏（孤儿进程、文件描述符泄漏）而非并发编辑。
- `expansion-strategic-v21.md`/`strategic-extensions-v24.md`：讨论子进程生命周期管理和沙箱隔离，不涉及多实例互斥。

---

## 方向四：跨项目治理继承与知识联邦

### 为什么这是高价值的

ForgeOS 当前的项目管理模型是**完全扁平化的孤岛**：

1. `forge-init` 创建的新项目复制全套治理资产——但所有副本是独立的。仓库 A 修改了 `policies.yml`，仓库 B 完全不知道。
2. `forge-upgrade` 可以从源仓库同步治理资产——但这是**推送**模式，不是**继承**模式。被治理项目无法主动选择「继承哪些策略、覆盖哪些策略」。
3. 没有**组织级策略**的概念。如果公司规定所有项目必须 coverage ≥ 80%，每个项目必须手动配置。
4. **跨项目知识无法共享**。项目 A 学到的「这个 Go 版本有已知 bug」无法自动传递给项目 B。各项目的 memory.jsonl 是完全独立的。
5. **联邦记分卡**不存在。项目 A 的 router 不知道项目 B 对同一模型/任务的评分。

这在大型组织中是不可持续的。每个新项目都启动为一个完全独立的治理实例，策略变更需要全量重新部署。

### 代码级证据

**证据 1：`forge-init` 复制而不继承**

```javascript
// harness/scaffold/forge-init.mjs
// GOVERNANCE_DIRS 和 COPIED_FILES 是文件级复制，不是引用
export const GOVERNANCE_DIRS = [
  join('.agent', 'agents'),    // 复制
  join('.agent', 'skills'),    // 复制
  join('.agent', 'workflows'), // 复制
  join('.agent', 'eval'),      // 复制
  join('.agent', 'routing'),   // 复制
  join('.agent', 'policies'),  // 复制
];
```

所有治理资产被**复制（copy）**而非**引用（reference）**。没有任何机制说「项目 B 继承项目 A 的 `harness/policies.yml`，并覆盖其中 3 行」。

**证据 2：`forge-upgrade` 是一对一的字节同步**

```javascript
// harness/scaffold/forge-upgrade.mjs
// 对比 SOURCE 仓库和目标项目的逐文件差异，然后同步
// 没有「组织级策略服务器」的概念
```

升级方向是 SOURCE → TARGET，其中 SOURCE 是另一个 ForgeOS 仓库的副本。没有「组织策略仓库」→「多个项目」的继承关系。

**证据 3：`internal/memory` 是项目本地的 JSONL 文件**

`memory.go` 的 `Append`/`Load` 操作的是项目本地路径（`<root>/.forge/memory.jsonl`）。没有任何跨项目 memory 合并或共享机制。

**证据 4：`internal/routing/scorecard.go` 是项目本地的 JSON**

`LoadScorecards` 从项目本地 `<root>/scorecards.json` 读取。没有联合（federated）记分卡的概念。

**证据 5：`project.yml` 只有本地属性**

```yaml
# .agent/project.yml（项目级）
mode: balanced
lifecycle: mvp
```
没有 `extends:`、`inherit:`、`parent_policy:` 字段来指向组织级策略。

### 核心设计思路

引入一个**三层治理继承模型**：

```
层 1 — 组织级（Organization Policy）
  只读真理源，由平台团队维护
  存放在中央仓库（如 org-forgeos-policies）
  内容：强制 gate 集合 · 安全基线 · 合规要求 · 成本上限 · 路由下限

层 2 — 团队级（Team Overrides）
  可选叠加，由 tech lead 维护
  存放在团队仓库或团队目录
  内容：代码规范 · 测试要求 · 自定义 gate · 模型偏好

层 3 — 项目级（Project Identity）
  forge-init 生成的唯一身份
  内容：PROJECT.md · ROADMAP.md · CURRENT_SPRINT.md · local overrides
```

**继承解析算法：**

```go
// internal/gov/inherit.go
type PolicyResolver struct {
    OrgRoot    string   // 组织级策略路径（可远程 URL）
    TeamRoot   string   // 团队级策略路径（可选）
    ProjectRoot string  // 项目路径
}

func (r *PolicyResolver) Resolve(name string) (string, error) {
    // 1. 尝试项目级 → 找到则返回
    // 2. 尝试团队级 → 找到则返回
    // 3. 尝试组织级 → 找到则返回
    // 4. 未找到 → 用内置默认值（不阻断）
}
```

**跨项目知识共享：**

```
- 新增 forge memory share <topic> <detail>   → 向组织级知识库发布一条经验
- 新增 forge memory feed                      → 拉取组织级相关经验
- 组织级知识库存放在中央位置（共享文件系统或简单 HTTP）
- 各项目在 prompt 构建时可选择是否注入组织级知识
```

**联邦记分卡：**

```
- 各项目定期向中央汇总上报 scorecard 快照
- forge route --federated 优先使用联邦数据做 history tiebreak
- 小样本项目受益于大项目的历史数据
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 项目级策略未定义任何 gate-set | 继承团队级/组织级 |
| 组织级策略 URL 不可达（网络问题） | 降级到纯本地策略，日志告警（不阻断运行）|
| 组织级策略版本高于项目兼容版本 | 自动回退到上次已知兼容版本，标记告警 |
| 两个项目向同一知识库发布矛盾的经验 | 基于置信度（confidence）和时效（recency）择优 |
| 团队策略覆盖了组织策略的关键安全 gate | **拒绝覆盖**：安全/合规类策略标记为 `immutable: true` |
| 项目离线运行 | 使用本地缓存的策略副本，无网络不阻塞 |

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-horizon-three.md`：「第三地平线生态」涉及多仓库编排和事件驱动管线。它的焦点是**跨仓库的编排自动化和事件传递**（仓库 A 的 evolve 完成后触发仓库 B 的工作流），本文的焦点是**治理资产继承和知识共享**（策略从组织传到项目、记分卡从项目汇集到组织）。两者是架构中不同的维度：一个控制**数据流**（事件），一个控制**策略和知识**（配置 + 经验）。
- `expansion-gaps-v7-novel.md`：涉及「跨项目知识联邦/组织学习」，与本文的方向四有表面相似。但它的焦点是**联邦学习**（多个项目的评分数据联合训练更好的路由模型），知识维度偏向 ML 模型训练。本文的焦点是：① **策略的声明式继承**（组织→团队→项目的配置传递）；② **经验的去中心化发布/订阅**（开发者主动发布教训、其他项目主动消费）；③ **联邦记分卡用于冷启动路由**（不等同于 ML 训练）。具体接口设计（`PolicyResolver` 解析链、`forge memory share/feed` 命令）在已有分析中不存在。
- `expansion-forgeos-meta-governance.md`：讨论 ForgeOS 自身治理的元层面，不涉及跨项目策略继承。

---

## 方向五：渐进式治理启动——按需引导而非全量拷贝

### 为什么这是高价值的

当前 `forge-init` 执行一次**全量拷贝**：

```
约 200+ 文件被复制到新项目：
  .agent/agents/   → 12 个角色卡
  .agent/skills/   → 9 个技能卡
  .agent/workflows/ → 5 个完整工作流
  .agent/policies/ → modes.yml, routing 策略
  .agent/eval/     → 评分 schema
  .agent/routing/  → 路由策略
  harness/         → ~30 个工具脚本 + 自测
  .github/         → CI 配置
  examples/starter/ → 种子应用
```

对于：① 一个刚接触 ForgeOS 的新手；② 一个只想「试一下」的原型项目；③ 一个已有 CI/CD 不想被全量治理覆盖的成熟项目

**全量拷贝是进入门槛**。它暗示「要接受所有治理才能开始」，而实际上 ForgeOS 的 `mode×lifecycle` 中枢旋钮可以在运行时调节严格度——但**启动表面积（bootstrap surface area）没有对应的旋钮**。

结果：
- 新用户被 200+ 文件吓退（「ForgeOS 太重了」）
- 成熟项目迁移时，全量治理覆盖现有工作流造成摩擦
- `examples/starter/` 的种子应用包含真实测试和代码，但几乎没人真用它——它更多是 `forge accept` 的占位测试而非实际的脚手架

### 代码级证据

**证据 1：`forge-init` 的 `COPIED_FILES` 清单包含约 100+ 条目**

```javascript
// harness/scaffold/forge-init.mjs
export const COPIED_FILES = [
  // 约 100+ 条目，从 CLAUDE.md 到每个 harness 工具
  // ...
];
```

没有「最小集」——要么全复制，要么不动。

**证据 2：没有 `--minimal` 或 `--tier` flag**

```bash
$ grep -n "minimal\|lite\|basic\|starter\|tier\|profile" harness/scaffold/forge-init.mjs
# 无匹配 —— 没有最小/轻量/分档选项
```

**证据 3：`project.yml` 的 `mode`/`lifecycle` 只调运行时严格度，不影响启动表面积**

```yaml
# mode: explorer, lifecycle: idea → 运行时很宽松（skip discover, haiku router, few gates）
# 但 agent 卡 / 工作流 / harness 工具仍是全量复制
```

启动后的「宽松模式」只影响执行路径，不影响文件数量。

**证据 4：没有「渐进式采纳」文档或向导**

搜索 ForgeOS 文档，没有「从零开始——第一步只加 `gate.mjs`，第二步加 `arch-check.mjs`……」的分步指南。

### 核心设计思路

引入四个 bootstrap 档位（profile），每个逐步增加治理深度：

```yaml
profiles:
  nano:      # 只想「试试」
    gates: [gate.mjs]
    agents: 0
    workflows: 0
    files: ~5  # CLAUDE.md + gate.mjs + .gitignore + README
    intent: "零治理，只有体积门"

  micro:     # 想加基本的质量控制
    gates: [gate.mjs, arch-check, secret-scan]
    agents: [implementer, reviewer]
    workflows: [build]
    files: ~40
    intent: "单工作流（build）+ 核心 gate"

  standard:  # 完整 ForgeOS（当前 forge-init 的行为）
    gates: [全部]
    agents: [全部 12 个]
    workflows: [全部 5 个]
    files: ~200+
    intent: "金标准——完整治理栈"

  enterprise: # standard + 可选增强（SCA DB/cross-vendor 路由/沙箱等）
    gates: [全部 + SCA（有 DB 时）]
    agents: [全部 + 扩展]
    workflows: [全部 + 定制]
    files: ~200+ + 外部依赖
    intent: "生产级——需要外部资源"
```

**实现方式：**

```
- forge-init --profile nano|micro|standard|enterprise
- 默认 = micro（不吓跑新用户；standard 仍可通过 --profile standard 获得）
- nano/micro 的 project.yml 自动设 mode=explorer, lifecyle=idea
- micro 升级到 standard：forge migrate --to standard（类似已有 explorer→engineering 迁移）
```

**渐进式治理升级路径：**

```
nano  → micro  → standard  → enterprise
         ↑          ↑
    (新用户入口)  (成熟项目入口)
```

每一步有对应的 `forge migrate --profile <target>` 命令，自动：
1. 复制新增的治理文件（不覆盖已有 identity）。
2. 更新 `project.yml` 的 mode/lifecycle。
3. 可选：派发补债任务（类似 `forge migrate --to engineering` 派生 backfill-tasks）。

### 边界场景

| 场景 | 行为 |
|------|------|
| micro 项目收到一个需要 full governance 的 PR | `forge status` 显示「当前 profile=micro，建议升级到 standard 以启用 REVIEW 工作流」|
| nano 项目想跳到 standard | `forge migrate --profile standard` 一次性完成 |
| 已有项目不想升级 | 保持当前 profile，零压力 |
| micro 下的 forge accept 判据 | 比 standard 少（无 review/coverage/adr 判据），但诚实 N/A 不伪造 |
| CI 开始要求 full governance | 修改 CI 的 `forge accept` 调用 → 项目需要先 migrate 才能通过 |
| 用户说「我不知道该用什么 profile」| 默认 micro（有基础保护但不重）；README 加决策树 |

### 与已有分析的差异证明

最接近的已有分析：
- `expansion-self-governance-and-hygiene.md`：讨论 ForgeOS 自身治理的自我施加和自我维护（`forge` 自身是否通过自己的 gate），而非用户的启动体验。
- `configuration-surface-and-adoption.md`：讨论 ForgeOS 的配置表面积（有多少配置项、哪些被消费者真正读取了），关注的是配置的**运行时覆盖率**。本文关注的是**启动时的文件数量门槛**——这是两个不同的采纳障碍。
- `expansion-production-readiness.md`：讨论生产环境下的可靠性增强（prompt QA、信号硬化、环境验证），属于已经接受 ForgeOS 之后的优化。本文关注的是**第一次接触**的采纳门槛。
- `fifth-wave-operational.md`：方向一提到 `forge init --interactive` 交互式引导生成项目文件——但那是指引用户填写 `--name`/`--mode` 等参数的 UX 改进，不是本文讨论的**启动文件体积分档**。

本文的独特性在于：
1. 明确把「启动表面积」定义为独立的采纳障碍，与运行时严格度正交。
2. 提出 `nano/micro/standard/enterprise` 四档 profile 而非全量复制。
3. 给出渐进式治理升级路径（`forge migrate --profile`），复用已有 migration 模式。
4. 每个 profile 有明确的文件计数、gate 集合、工作流集合和适用场景。

---

## 总结

| 方向 | 核心问题 | 与 65+ 已有方向的不重叠证明 | 预估复杂度 |
|------|----------|---------------------------|-----------|
| **1. 外部 SDLC 集成** | ForgeOS 不能创建 PR/检查 CI/评论/合并 | SDLC API 集成在已有 65+ 方向中零覆盖 | 大（新包 + VCS 适配器） |
| **2. 行为回归检测** | 结构 gate 不检查 agent 输出是否正确 | 差异化测试策略在已有分析中零覆盖 | 中（新增 gate 策略） |
| **3. 工作树互斥** | 两 agent 同时编辑同一文件→静默丢数据 | 与 .forge/ 存储隔离正交（不同层的问题） | 小（锁文件协议） |
| **4. 跨项目治理继承** | 每项目独立复制→策略无法统一管理 | 策略继承链在已有分析中零覆盖 | 中-大（解析栈） |
| **5. 渐进式启动** | 200+ 文件吓退新用户和原型项目 | 启动表面积作为独立采纳障碍的思路在已有分析中零覆盖 | 中（重构 forge-init） |

每个方向都是可独立验证、可增量交付的——没有要求一次做完，也没有要求跨方向依赖。
