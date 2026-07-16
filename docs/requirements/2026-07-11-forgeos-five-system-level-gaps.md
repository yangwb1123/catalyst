# ForgeOS — 五个系统级结构性扩展方向（全局代码扫描）

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局逐包扫描 `forge-core/` (18 Go 包)· `harness/` (40+ 模块)· `scaffold/` · `examples/` · `.agent/` (5 workflow · 12 agent 卡 · 9 skill 卡 · 全部 ADR + DECISIONS)  
> **已审阅**: `docs/requirements/` (114+ 份)· `docs/analysis/` (24+ 份)· `CURRENT_SPRINT.md` (31 sprint 记录)· `FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> **去重验证**: 对每个方向在已有分析全文中进行关键词 URL 搜索和全文 grep,确认未被系统性论述  
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码级证据与产品价值判断  
> **日期**: 2026-07-11

---

## 核心发现:系统级缺口,非增量功能

经过 31 轮 sprint,ForgeOS 在**运行时引擎层**已高度成熟:编排引擎(串行/并行/loop-back/resume/checkpoint/mode-gating)、安全护栏(递归深度·执行次数·墙钟超时·输出上限)、学习闭环(trace→scorecard→memory→converge)、真点火验证(multi-agent 端到端 + 8 个真 bug 修复 + 三维成本遥测)均已落地。

但 114+ 篇扩展分析几乎全部聚焦在**已有引擎的增量功能**(加一个 knob、添一个新适配器、优化一个缓存)。以下 5 个方向全部落在这些分析的间隙中——它们触及的不是"还有什么可加",而是**当前架构中系统性地不存在或者将将萌芽的概念层**:

---

## 方向一 · 模板演化静默漂移——`forge-upgrade` 是一个孤立孤儿

> **类型**: 产品缺口 · 治理缺口 · **优先级**: P1 (紧急)  
> **关键词验证**: `forge.upgrade 0`(CLI 子命令零命中)· `template.evolution 0` · `drift.detect 0` · `governance.sync 0` · `harness.version 0`

### 现状

`harness/scaffold/forge-upgrade.mjs` 是一个**设计优秀但完全孤立的 Node 脚本**。它具备:

- 精确的 drift 分类(added/changed/unchanged/removed, `classifyDrift`)
- 安全的 backup-on-overwrite 机制(`backupTimestamp` + `copyFileSync`)
- 诚实的范围声明("I do NOT change forge-core binary behavior")
- DRY 默认、`--apply` 安全确认、`--prune` 预留

**但它没有连接到任何东西上:**

```bash
# 当前用户必须手动找到并运行:
node harness/forge-upgrade.mjs --from /path/to/forgeos --target /path/to/project [--apply]
# forge-core CLI 中没有任何子命令知道它的存在
```

| 证据 | 位置 | 说明 |
|------|------|------|
| `subcommands` 表 | `cmd/forge/main.go:51-66` | 15 个子命令,包含 `preflight`/`doctor`/`approve`,但**没有 `upgrade`** |
| `forge-upgrade.mjs` | `harness/scaffold/forge-upgrade.mjs` | 完整的独立实现,但**零消费代码路径**——没有 CLI 入口、没有 CI 检查、没有自动 drift 检测 |
| `forge-init` COPIED_FILES | `harness/scaffold/forge-init.mjs` | 复制治理资产的清单,但 upgrade 脚本本身不在清单中——项目升级后**无法自行检测到有新版本的升级器可用** |

### 产品价值

**这是一个随时间扩大且不可逆的债务**:每次 forge-core 的 `acceptance.mjs`、`arch-check.mjs`、`gate.mjs`、`check.py` 有更新(新检查、bug 修复、硬化)、所有通过 `forge-init` 创建的项目都落在后面。没有 drift 检测、没有通知、没有自动升级路径。3 个月后,这些项目的治理层已经和 forge-core 严重偏离——但没有任何迹象告诉负责人。

具体后果:
1. **安全补丁不达**:`secret-scan.mjs` 增加新 pattern 检测,旧项目永远得不到
2. **治理弱化**:`acceptance-kernel.mjs` 拆分后旧项目的 acceptance 仍是一个 499 行的单体文件
3. **不可审计**:组织不知道哪些项目跑了哪些版本的治理工具

### 建议方向

| 层 | 建议 |
|------|------|
| **CLI** | `forge upgrade` 子命令——shell 出 upgrade 脚本,使其与 `forge migrate`/`forge init` 并列 |
| **CI** | `forge check` 或 `forge doctor` 增加 governance-drift 检测 —— 项目与嵌入的 `.forge-version` 或 SOURCE SHA 对比 |
| **自动化** | `forge migrate --to engineering` 在升级 mode 时附带检查 governance 是否过时 |
| **检测** | 在 `forge upgrade` 之前,`forge doctor` 就可以有一个 `drift_check` 项——什么都不写,只报告"你当前有 X 个文件与 source 不同" |

---

## 方向二 · 冷启动引导——空 ROADMAP 下的计划瘫痪

> **类型**: 边界情况 · 产品体验 · **优先级**: P2 (高)  
> **关键词验证**: `cold.start 0` · `bootstrap 0` · `empty.roadmap 0` · `no.tasks 0` · `initial.*roadmap 0`

### 现状

当用户 `forge run build` 一个刚 `forge-init` 的空白项目时,ROADMAP.md 内容为空(或仅包含标题,无可勾选的 checklist 项):

| 组件 | 行为 | 证据 |
|------|------|------|
| `converge.RoadmapCompletion("")` | 返回 `0` (total==0) | `converge/converge.go:353-367` |
| `build.yml` stop condition | `roadmap_completion == 100` → **false** | 正确不收敛 |
| `prompt.currentTask()` | 返回 `""`(空字符串) | `prompt/prompt.go:99-104` |
| `buildPrompt` 构造的提示 | agent 收到空 task lane | `prompt_context.go` → `Gather` → `currentTask` |
| `gatherSignals` | `RoadmapCompletion: 0` | `gates.go` → `gatherSignals` |

**问题不在于收敛判定**(空 ROADMAP 正确不收敛),而在于 **agent 没有任务可执行**:

```
prompt 中 inject 的内容:
  ## Role card
  (agent 角色描述——正确)

  ## Project context
  Current task — implement what .agent/ROADMAP.md describes:
  [空]

  Engineering constraints (hard, non-negotiable):
  - ... (正确)
```

一个 `planner` agent 收到空任务指令后,在 print-mode (`claude -p`)下会:
- 直接什么也不输出(遵守"只回答被问的内容"→ 一句话带过)
- 或虚构任务(产生幻觉,给用户一个从未讨论过的 feature 的 task plan)

**两种结果都不是"好的首次体验"**。

### 产品价值

ForgeOS 的核心理念是"需求探索 > 代码实现"(G1,G4)。但目前的冷启动流程是:

1. 用户 `forge init my-project`
2. 用户 `forge run build`(想试试看能做什么)
3. 系统运行 5 个 phase,每个 phase 的 agent 都收到空任务 → 什么都没做 → converge 不满足 → 用户困惑

**更好的设计**:`forge init` 应该触发一个短交互(或读取 `project.yml` 中的 goals),**自动推导并写入一个初始 ROADMAP**。具体地:

| 阶段 | 当前行为 | 建议行为 |
|------|----------|----------|
| `forge init` | 复制模板文件,写 project.yml | 同上,外加:检测 project.yml 的 `description`/`goals`,生成 1-3 条初始 ROADMAP 项 |
| `forge run discover`(冷启动) | discover phase 收空 prompt | product-manager agent 收到 "这是一个新项目,请生成初始 ROADMAP" 的指令 |
| `forge run build`(冷启动) | planner 收空 task | **拒绝运行**: "ROADMAP is empty. Run `forge run discover` first, or manually add checklist items to `.agent/ROADMAP.md`" |

### 边界情况

1. **内容已存在但无 checklist 格式**:ROADMAP 有描述性文本但没有 `- [ ]` 行 → `RoadmapCompletion` 也是 0 → 同样问题。系统应检查文件是否存在且包含 checklist 语法,而非仅非空。
2. **用户手工创建了 ROADMAP**:用户自己写了 3 个 `- [ ]` 项 → 一切正常。冷启动检测只在 checklist 项数为 0 时触发。
3. **非 build 阶段可以绕过**:discover 和 design 阶段天然不需要 ROADMAP——它们生成 ROADMAP。检测应仅限依赖 ROADMAP 的阶段(planner/implementer)。

---

## 方向三 · 语言生态模板抽象——`CODE_EXTS` 是有偏默认

> **类型**: 架构缺口 · 产品扩展性 · **优先级**: P2 (高)  
> **关键词验证**: `language.template 0` · `--lang 0` · `polyglot.template 0` · `ecosystem.default 0` · `workspace.template 0`

### 现状

`forge-init` 创建一个强 Node.js 有偏的项目骨架:

| 治理元素 | 当前默认 | Node.js | Python | Go | Rust |
|----------|----------|---------|--------|-----|------|
| `gate.mjs`: `CODE_EXTS` | `.ts,.tsx,.js,.mjs,.cjs,.jsx,.py,.go,.rs,.java` | ✅ | ✅ | ✅ | ✅ |
| `gate.mjs`: `SKIP_DIRS` | `node_modules,.git,dist,build,.next,coverage,vendor,.forge` | ✅ | ❌ `__pycache__` 漏了 | ❌ | ❌ `target` 漏了 |
| `adapters/go.yml` | `test: go test ./...` | ❌ | ❌ | ✅ | ❌ |
| `adapters/python.yml` | 存在但测试命令空 | ❌ | 部分 | ❌ | ❌ |
| seed app | Node.js `.mjs` 文件 | ✅ | ❌ | ❌ | ❌ |
| `policies.yml` | `max_root_files:15` | 通用 | 通用 | 通用 | 通用 |

**关键证据:**

| 文件 | 行 | 证据 |
|------|-----|------|
| `harness/gate.mjs` | `CODE_EXTS` | 硬编码文件扩展集合,不是通过 adapter 扫描或用户配置驱动的 |
| `harness/gate.mjs` | `SKIP_DIRS` | `node_modules` 硬编码,但 **`__pycache__`/`target`/`venv` 缺失** |
| `harness/scaffold/forge-init.mjs` | 全部 | `--lang` 或 `--ecosystem` 参数不存在 |
| `examples/go-taskd/` | 全部 | Go 项目示例**物理上存在于 `examples/` 中**,但 `forge-init` **完全不使用它作为模板** |
| `harness/policies.yml` | 全部 | 无语言特定策略 |

### 产品价值

ForgeOS 的愿景是 **polyglot 软件工厂**(BOOTSTRAP.md: "Go-核心 polyglot")。但新项目的引导体验是强 Node.js 偏好的——如果一个 Python/Go/Rust 团队 `forge-init` 一个项目:

1. `SKIP_DIRS` 漏掉了 `__pycache__`/`target`/`venv` → gate.mjs 会扫描缓存二进制文件 → 计数噪音/误报
2. `adapters/go.yml` 的 `test:` 已正确配置,但 `forge-init` 生成的 seed app 是 Node.js \`.mjs\` 文件 → 用户需要手动删除 starter app 并替换
3. 项目第一周:用户花时间"让 forge 适配我的语言环境",而非"用 forge 构建软件"

**建议**:增加 `forge init --lang python|go|rust|ts`(或更抽象的 `--ecosystem`)参数:

| `--lang` | `CODE_EXTS` 子集 | `SKIP_DIRS` 追加 | seed app | `adapters/*.yml` 激活 | 默认测试命令 |
|----------|------------------|-------------------|----------|----------------------|--------------|
| `python` | `.py` | `__pycache__`, `venv`, `.tox` | `src/` + `test/` Python 文件 | `python.yml` | `pytest` |
| `go` | `.go` | `vendor`(已在) | `main.go` + `internal/` | `go.yml` | `go test ./...` |
| `rust` | `.rs` | `target` | `src/main.rs` + `Cargo.toml` | 新建 | `cargo test` |
| `node`(默认) | `.ts,.js,.mjs` | `node_modules` | `src/` + `test/` mjs 文件 | `typescript.yml` | `vitest` |

---

## 方向四 · 多仓库编排——一个没有服务边界的 OS

> **类型**: 架构缺口 · 产品天花板 · **优先级**: P3 (中,但战略重要)  
> **关键词验证**: `multi.repo 0` · `cross.repo 0` · `cross.project 0` · `service.dependency 0` · `monorepo 0` · `microservice 0`

### 现状

ForgeOS 的所有概念模型都是**单一仓库导向**的:

```go
// 每个路径解析都是 repo-scoped:
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
```

| 组件 | 范围 | 证据 |
|------|------|------|
| `.agent/` | 每个项目一个 | 没有跨项目引用语法 |
| `.forge/` | 本地到 repo | checkpoint/trace/memory 都在 repo 本地 |
| `memory.jsonl` | 每个 repo 独立 | 没有跨项目知识共享 |
| `scorecards.json` | `.agent/routing/` | 路由决策基于单项目历史 |
| `forge evolve` | 单 workflow | 没有"在项目 A 中发现问题,在项目 B 中创建 ticket"的通道 |
| `roadmap.md` | 单项目 | 没有跨项目依赖阻塞跟踪 |

**这不是一个设计缺陷**——ForgeOS v0-v2 的目标是单一仓库场景(ROADMAP.md 明确声明)。**但这是一个随时间会变成产品天花板的限制**:

### 产品价值

真实企业的软件不是单仓库:

```
Service A (repo-a) ──depends-on──→ Service B (repo-b) ──depends-on──→ Library C (repo-c)
                                     ↑
Service D (repo-d) ──────────────────┘
```

在这种拓扑中,ForgeOS 的"单项目自动 ROADMAP" 会错过:

1. **跨项目依赖阻塞**:项目 A 的 Roadmap 90% 完成,但阻塞在项目 B 的 API 变更上——ForgeOS 完全感知不到
2. **API 契约变更的级联效应**:项目 B 改 proto/OpenAPI spec → 需要自动触发项目 A 的 adapt 工作流
3. **共享知识**:项目 C 的安全漏洞修复经验 → 应该传播到项目 A/B/D 的知识库

**这不是 v3 必须做的,但在设计决策中需要预留接入点**。具体地:

| 所需预留 | 当前状态 | 建议 |
|----------|----------|------|
| `memory.jsonl` 的远程同步 | 纯本地文件 | 增加可选的 `memory_sync_url` 或 `memory_sync_command`——forge-core 本身不实现同步,但开放钩子 |
| `checkpoint.json` 的跨项目可见性 | 纯本地 | 在 `.forge/` 中增加可选 `shared/` 子目录或 socket 接口 |
| `route` 的跨项目路由 | `forge route` 单项目 | 增加 `forge route --from-service`/`--to-service` 框架(CLI 层,非引擎层) |
| `forge evolve` 的触角 | 单 workflow | `workflow.yml` 中增加可选 `triggers: [repo-a/ci, repo-b/deploy]` 声明字段(asset 层) |

**关键原则**:不实现多仓库编排,而是**预留概念空间**,使 v3 的多仓库服务(`forge-fleet`)可以在不破坏 v2 单仓库假设的情况下接入。

---

## 方向五 · 故障注入与韧性验证基础设施——最复杂错误逻辑零系统测试

> **类型**: 测试缺口 · 质量风险 · **优先级**: P2 (高)  
> **关键词验证**: `fault.injection 0` · `chaos 0` · `resilience.test 0` · `overload.test 0` · `529.test 0` · `inject.failure 0` · `fake.agent 0`

### 现状

ForgeOS 拥有整个项目中**最复杂的错误处理逻辑链条**之一:

```
529 过载检测:
  classifyClaudeOverload → hasOverloadMarker → containsToken529
  → 影响: KindOverloaded → overloadBackoff → timed backoff → retry

超时:
  timeout (flag) → CommandExecutor.Timeout → exec.CommandContext
  → 影响: KindTimeout → classifyRunErr → isOverload? → MaxRetries

重试:
  runAgentPhase → classifyRunErr → retryable → overloadBackoff → MaxRetries
  → 影响: 是否重试?是否退避?是否放弃?

预算守卫:
  checkRunBudget → BudgetExhausted → runBudget.exhausted
  → 影响: 是否拒绝 spawn?是否降级模型?

循环依赖:
  orchestrator checkCircularDependencies → DFS 环检测
  → 影响: 是否接受 workflow?
```

**但没有任何一条代码路径是经过系统性故障注入测试的:**

| 错误处理 | 实现位置 | 测试方式 | 风险 |
|----------|----------|----------|------|
| 529 过载检测 | `cmd/forge/cost.go:classifyClaudeOverload` | 单元测试(静态 JSON fixture) | **静态 fixture 通过,但"真实 claude 进程输出 529"从未被验证**——没有 fake claude 进程生成真实的 529 响应 |
| 超时杀死子进程 | `internal/orchestrator/command_executor.go` | 单元测试(快速超时) | "超时后子进程确实被杀死"已验证,但"超时后子进程遗留的 side effect"未测试 |
| 重试退避 | `internal/orchestrator/backoff.go` | 单元测试(时间压缩) | **退避算法正确,但"重试后 agent 状态一致"未测试——重试的 implementer 看到的是之前写到一半的文件还是干净状态?** |
| 预算耗尽 | `cmd/forge/cost.go:runBudget` | 单元测试(算术) | **预算耗尽后 claude 确实停下来了?** ——预算逻辑正确但下游没有 fake claude 确认 |
| 递归深度保护 | `internal/orchestrator/command_executor.go` | 单元测试(env 注入) | Fork-bomb 防护逻辑正确,但**真实二进制行为未在集成测试中验证** |

### 产品价值

**这不是"多写几个单元测试"的问题**——问题在于 ForgeOS 最复杂的运行时行为(backoff、retry、overload recovery、timeout、budget guard)完全依赖于外部进程(真实 `claude` CLI)的特定输出来验证。而这些外部进程:

1. **按需付费**:每次真实 claude 调用花费真金白银
2. **不可控**:API 返回 529 是随机事件,不能在 CI 中可靠重现
3. **慢**:真实 LLM 调用以秒计,迭代 50 次的测试不可行

**后果**:最需要测试的路径要么没被测试(529 恢复),要么只测了逻辑没测集成(超时后进程确实被杀了但"agent 写到一半的文件"怎么办?)

### 建议:轻量故障注入框架

在 `harness/` 或 `cmd/forge` 中增加一个 `--executor=fake` 模式——NOT 生产用户可见,而是测试和 CI 专用:

```go
// FakeAgentExecutor 模拟一个真实 agent 的输出,在指定条件下注入故障
type FakeAgentExecutor struct {
    // 输出模板——正常返回的内容
    OutputTemplate string
    
    // 故障注入
    FailOnPhases    []string   // 哪些 phase 触发故障
    FailWithError   string     // timeout / overload / config / generic
    FailOnIteration int        // 指定 iteration 触发
    LatencySimMs    int        // 模拟的延迟
    
    // 生成 verdict
    Verdict         string     // APPROVE / REQUEST_CHANGES / 或空
    CostUsd         float64    // 模拟的账单
    Confidence      float64    // 模拟的置信度
}
```

这个 fake executor 应该:

1. 继承现有的 `AgentExecutor` 接口——零架构改动
2. 可组合——可以从 YAML fixture 加载,使测试 fixture 可读
3. 注入 trace event——使 scorecard 验证路径也走过全链路

**关键测试场景(fixture 举例)**:

```yaml
# tests/fixtures/overload-recovery.yaml
phases:
  - name: implementer
    agent: implementer
    executor:
      fake:
        template: "normal output"
        latency_ms: 2000
        cost_usd: 0.05
  - name: harness-gates
    required_gates: [test]
    executor:
      fake:
        fail_on_attempt: 1  # 第一次失败→触发 retry
        fail_with: overload  # 返回 529 信号
        succeed_on_attempt: 2  # 第二次成功
```

### 边界情况

1. **fake executor 必须有诚实标记**:不可被用户误用于生产环境。执行器名 `fake` 在所有 CLI 输出和 trace 中必须明确标注,使其不可被混用于真实运行的计费/审计。
2. **与 dry-run 的关系**:`--executor=dry` 现在完全不调 LLM,也不产生输出。`--executor=fake` 是 "dry + 模拟响应"——产生 agent 输出但不调 LLM,可用于端到端测试 loop-back/retry/converge。
3. **不能破坏 load-bearing 路径**:fake executor 必须与真实 executor 走同一条 `observeFor` → `cost.go` → `trace.Emit` 管道,以便验证 telemetry 全链路。
4. **升级友好**:fake executor 的 fixture 格式应被视为测试基础设施,不承诺向后兼容。每个版本可能有不同的故障注入维度,无需保持 fixture 的稳定性。

---

## 优先级总览

| 方向 | 标题 | 优先级 | 影响面 | 当前风险等级 |
|------|------|--------|--------|-------------|
| 一 | 模板演化静默漂移 | **P1** | 安全+治理 | 随时间线性增长 |
| 二 | 冷启动引导 | P2 | 产品体验 | 首次使用即触发 |
| 三 | 语言生态模板抽象 | P2 | 产品扩展性 | 非 Node 团队必然触及 |
| 四 | 多仓库编排预备 | P3 | 产品天花板 | 当前不致命,v3 前需要 |
| 五 | 故障注入测试基础设施 | **P1** | 质量+信心 | 最复杂错误逻辑未经系统性验证 |

---

## 与现有分析的差异化总结

| 本文方向 | 被 114+ 份已有分析覆盖? | 关键差异 |
|----------|------------------------|----------|
| 模板演化漂移 | **否** — 无任何分析涉及 `forge-upgrade` 的孤立状态 | 已有分析都假设模板是一次性的,忽略了演化场景 |
| 冷启动 ROADMAP | **否** — 无分析检查空 ROADMAP 下 agent 收到什么 prompt | 已有分析关注边界(空 workflow、vacuous run),但未追踪 prompt 内容 |
| 语言模板抽象 | **部分** — `expansion-core-five.md` 提到 polyglot,但不是从 `forge-init` 模板角度 | 已有分析的 polyglot 讨论都是运行时层(跨厂商模型池),非项目引导层 |
| 多仓库编排 | **否** — 无分析讨论跨项目拓扑 | 已有分析都假设单仓库,ROADMAP 也诚实标注了这个限制 |
| 故障注入 | **否** — `edgecases-and-perf.md` 分析过竞态/资源,但无系统故障注入 | 已有分析关注"代码是否正确",而非"错误处理路径是否被测试覆盖" |
