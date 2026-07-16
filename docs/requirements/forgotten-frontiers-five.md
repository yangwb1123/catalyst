# ForgeOS — 被遗忘的前沿：五个零覆盖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深度扫描 —— forge-core 18 Go 包 / 195+ 源文件 / 707+ 测试 /  
>   harness 39 模块（5 adapter + 18 test + 16 gate/arch/check/scorecard/sca/secret-scan）/  
>   `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 workflow + 10 prompt 模板）/  
>   Sprint 1–31 完整演进记录（每个 sprint 的 gap 诊断与修复）/  
>   docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md（90 DONE / 14 GAP 全部收口）/  
>   交叉核对 **40+ 篇 `docs/analysis/*.md` 和 14 篇 `docs/requirements/*.md`**（~60 个已有方向）  
> **核心承诺**: 每个方向在全部 ~60 个已有方向中**零覆盖**（附差异化证明）  
> **纪律**: 不编写任何代码。每方向附代码级证据 + 与已有分析的逐条对比  
> **日期**: 2026-07-09

---

## 已有 60+ 方向已覆盖的维度（本文不再重复）

| 维度 | 代表文档 | 方向数 |
|------|----------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/自适应装配） | `high-value-expansion-directions.md` + `v32` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | `expansion-horizon-three.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 双解析器） | `expansion-production-readiness.md` + `v3.md` | ~8 |
| 执行语义形式化（原子性/幂等性/因果一致性/版本演化/收敛定量） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md` + `v26` | ~10 |
| 系统性边界盲区（级联截断/YAML 分歧/信任边界/持久语义/可移植性） | `strategic-extensions-v22-v32.md` | ~10 |
| 安全/凭据/secret 生命周期/沙箱/SCA | `genuinely-novel-expansion-directions.md` | ~5 |
| CLI DX / shell 集成 / daemon 模式 / 增量采纳 / tutorial | `systemic-expansion-v26.md` + `expansion-self-governance.md` | ~5 |
| 并行编排 / 迭代跳过 / 收敛可见性 / YAML 差分测试 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 结构化输出协议 | `next-five-frontiers.md` + `architectural-expansion-perspectives.md` | ~8 |
| **总已有覆盖** | | **~60 方向** |

**本文瞄准的是这些方向之外的真正盲区**——不是「再加一个引擎」或「把这个功能优化一下」，而是 **ForgeOS 作为一个生产级治理平台，在成为「AI 软件工厂」的路上被系统性忽视的五个架构级空白**。

---

## 方向一：GitOps 控制器模式 —— 从「CLI 工具」到「平台控制器」

**类型**: 架构 · 部署模型  
**优先级**: P1（决定 ForgeOS 能否从「开发者工具」升级为「企业平台」）  
**代码影响**: 新 `forge-controller/` 目录（或 `internal/controller/`）· `cmd/forge` 的入口扩展 · 已有 `internal/orchestrator`/`internal/asset` 复用  
**当前代码**: 零实现

### 现状

ForgeOS 今天是一个**严格 CLI 驱动的工具**：

```
用户 SSH → 敲 forge run build → 等完成 → 看输出 → 敲 forge run evolve → ...
```

所有入口都在 `cmd/forge/main.go:69-76` 的 `subcommands` 表中：

```go
var subcommands = map[string]func([]string) int{
    "run":     cmdRun,
    "evolve":  cmdEvolve,
    "gate":    func(rest []string) int { return delegate(gate.Gate, rest) },
    // ... 全部是 CLI 子命令
}
```

生产部署方式被推给外部编排：

```
# operator 需要自己写：
systemd service → forge run build  （定时/开机启动）
cron job       → forge evolve      （每天凌晨执行）
git hook       → forge run review   （PR 触发）
```

没有：
- **常驻进程模式**（`forge controller`）：启动后持续运行，管理多个 repo、多个 workflow 的生命周期
- **Git 事件监听**：无需手动 `git pull && forge run`，控制器自动 watch repo changes
- **队列与调度**：多个 workflow 请求排队执行，而非进程级串行
- **Webhook API**：GitHub/GitLab webhook 直接触发 workflow run，带 HMAC 验证
- **状态持久化**：控制器自身有持久状态，重启后恢复所有正在运行/排队的 workflow

### 为什么已有分析未覆盖

| 相近方向 | 来自 | 与本文差异 |
|---------|------|-----------|
| **Daemon 模式**（forge daemon） | `high-value-extension-directions-v2.md` 方向一 | 关注的是「后台运行 CLI」的**进程管理**（health check / SIGHUP / 日志轮转）。本文关注的是**控制器架构**——事件驱动 + git watch + webhook + 队列 + 多 repo 管理。Daemon 是「把 CLI 放后台」，Controller 是「把 ForgeOS 变成平台」 |
| **事件驱动触发**（webhook/cron） | `expansion-directions-v14.md` 方向三 | 关注的是**单个触发事件**（webhook → forge run）。本文关注的是**完整的控制器生命周期**——注册 repo → watch → 事件队列 → 调度执行 → 状态报告 → 重试 → 清理。不是一个 `--webhook` flag，是一个新架构 |
| **Web UI**（v3） | `.agent/ARCHITECTURE.md` | Web UI 是**人看的界面**。本文的控制器是**程序调用的平台**。两者正交——控制器可以没有 UI（CLI + API 就够了） |
| **多项目工作区** | `systemic-expansion-v26.md` 方向三 | 关注的是**单 CLI 进程如何理解多项目结构**（workspace 概念）。本文关注的是**跨进程、常驻、事件驱动的平台控制器**——完全不同层的抽象 |

### 代码级证据

1. **`cmd/forge/main.go:69-76`** — 全部 17+ 子命令都是**一次性 CLI 调用**，无常驻循环
2. **`internal/persist/checkpoint.go`** — checkpoint 只保存单次 evolve 的迭代状态，不是「控制器世界状态」
3. **`internal/orchestrator/loop.go`** — LoopEngine 的循环是**单 workflow 内迭代**，不是外部事件循环
4. **`internal/asset/asset.go`** — `LoadWorkflowJSON` 每次从文件读，无「已注册 workflow」的运行时缓存
5. **`.github/workflows/forge.yml`** — CI 用 `forge run` / `forge accept` 的子进程调用，不是 API 调用

### 建议实现轮廓

```
forge controller                          # 启动控制器（常驻）
  --root /repos/my-project                # 管理的 repo 路径（可多个）
  --watch                                 # 文件变更自动触发
  --webhook-secret s3cr3t                 # webhook HMAC 验证密钥
  --port :8080                            # HTTP API 端口
  
# 工作流：
# 1. git push → GitHub webhook → forge controller（HTTP API）
# 2. 控制器验证 HMAC，解析 push 事件
# 3. 根据 .forge/triggers.yml 匹配事件到 workflow
# 4. 创建工作流 Run（状态=queued）
# 5. 调度器取出 Run，调用 orchestrator.RunFrom
# 6. 结果写回状态，触发回调（webhook / Slack / email）
# 7. forge status --controller http://forge:8080 查询远程状态
```

已有基础设施可复用：
- `internal/orchestrator` — 已有完整的 phase 执行引擎
- `internal/persist` — checkpoint 可扩展为 run-level 持久化
- `internal/gate` / `internal/converge` — 闸门和收敛不变
- `internal/routing` — 路由策略不变
- `harness/*.mjs` — 执法器通过 gate 接口调用，不变

**新代码量估计**: ~2000 行（HTTP server + webhook 处理 + 队列 + 状态机 + git watcher）

### 为什么需要

ForgeOS 的愿景是 **24h 无人值守的 AI 软件工厂**。但今天工厂的「门卫」是人——人 SSH 进去敲 `forge evolve`。只要入口还是 CLI，ForgeOS 就永远是一把需要人扣动的扳机，不是自动化的生产线。Controller 模式把 ForgeOS 从「工具」升级为「平台」——这正是 v3 Vision 中 `Gateway` 引擎要做的事，但架构设计从未落地。

---

## 方向二：工作流定义测试框架 —— 把 Workflow YAML 当作代码测试

**类型**: 功能 · 开发者体验  
**优先级**: P1（每次 workflow 变更都是无测试的盲改）  
**代码影响**: 新 `internal/workflowtest/` 包 · 已有 `internal/asset`/`internal/gate`/`internal/orchestrator` 扩展  
**当前代码**: 零实现

### 现状

今天修改一个 `.agent/workflows/*.yml` 文件，**没有任何自动化测试覆盖这个改动**：

```yaml
# build.yml — 如果我改了 on_fail.target_phase 或加了新 phase
on_fail:
  action: loop_back
  target_phase: implementer   # 如果我拼写成了 "implementor"？
```

现有验证工具：
- **`forge validate --models`** — 检查 phase 引用的 agent 卡和 template 是否存在（结构校验）
- **`check.py` 的 `check_workflow_control_flow`** — 检查 `target_phase` 引用是否存在于 phase 列表中
- **`internal/orchestrator` 的测试** — 测试 mock 编排器，但**不是给用户用的**

这些是**静态验证**。没有**动态行为测试**：

```
# 我想测试的「行为」：
# 1. 当 gate test FAIL 时，workflow 是否跳回 implementer？
# 2. 当 reviewer 输出 REQUEST_CHANGES 时，是否跳回 implementer 而不是 planner？
# 3. 当 3 次 loop-back 耗尽后，workflow 是否 fail-closed abort？
# 4. parallel 模式下，没有 depends_on 的 phase 是否并发执行？
```

### 为什么已有分析未覆盖

| 相近方向 | 来自 | 与本文差异 |
|---------|------|-----------|
| **YAML 双解析器差分测试** | `high-value-extension-directions-v3.md` 方向一 | 关注的是**YAML 解析器的一致性**（Python vs Go 解析结果是否相同）。本文关注的是**workflow 行为语义的一致性**（workflow 定义是否按预期产生正确的 phase 序列） |
| **forge validate** | 已有 | 只做**静态结构验证**（字段存在/引用有效）。不做**行为语义验证** |
| **编排器内部测试** | `internal/orchestrator/*_test.go` | 是 ForgeOS **自身**的测试，不是**给用户**测试自己 workflow 的工具。用户无法在 `examples/url-shortener` 中写一个 `workflow_test.yml` 声明「当 gate X 失败，验证 phase Y 被跳回」 |
| **回放/确定性测试** | `architectural-expansion-perspectives.md` 方向五 | 关注的是**回放已有 trace** 验证编排器行为。本文关注的是**在写 workflow 时提前验证预期行为**——TDD for workflows |

### 代码级证据

1. **`internal/orchestrator/orchestrator.go:RunFrom`** — 整个编排器接受 `asset.Workflow` 作为纯数据输入，无外部副作用（gate 是注入的 callback），天然可测
2. **`internal/orchestrator/orchestrator_test.go`** — 已有 700+ 行 mock-based 编排器测试，证明编排器是**可独立于真实 agent/gate 运行的**
3. **`internal/asset/asset.go:Workflow`** — 纯数据结构，可通过 `LoadWorkflowJSON` 从任意 YAML/JSON 加载——workflow 可来自测试文件
4. **`internal/gate/gate.go:Gate/Check/Accept`** — 三个门函数都是 `func(string) Result`，可轻松 mock
5. **`internal/converge/converge.go:Signals`** — 收敛信号也是纯数据结构，可合成

### 建议实现轮廓

```yaml
# .agent/workflows/tests/build_test.yml
# forge test workflow --file .agent/workflows/tests/build_test.yml
name: build workflow test
steps:
  - name: gate failure triggers loop-back to implementer
    given:
      workflow: build
      phase_results:
        implementer: { verdict: "", gate: PASS }
        gate: { verdict: "", gate: FAIL }   # test gate fails
    then:
      after_gate: FAIL
      next_phase_should_be: implementer     # loop back
      loop_back_count: 1
      
  - name: three loop-backs exhaust and abort
    given:
      workflow: build
      phase_results:
        implementer: { verdict: "", gate: PASS }
        gate: { verdict: "", gate: FAIL }   # repeats 3x
      max_loop_back: 3
    then:
      after_gate: FAIL_ABORT                # fail-closed
      convergence: NOT_MET
      exit_code: 1
```

其实就是把 `internal/orchestrator` 已有的 mock 测试能力**暴露为用户的声明式测试 DSL**。

**新代码量估计**: ~800 行（DSL 解析 + 编排器调用器 + 断言器 + `forge test-workflow` 命令）

### 为什么需要

ForgeOS 的核心承诺是**治理即代码**。Workflow YAML 是治理的最高表达。但今天，**治理代码本身不受治理**——没有一个测试框架来验证 workflow 定义的正确性。随着 ForgeOS 被更多项目采纳，workflow 定义会越来越复杂（多 phase / 条件跳转 / parallel / mode 矩阵 / lifecycle 迁移），**没有测试的 workflow 改动就是一个等待发生的静默失败**。

---

## 方向三：软件供应链信任与可验证治理（Attestation & SLSA）

**类型**: 安全 · 架构  
**优先级**: P1（治理 OS 需证明治理发生过）  
**代码影响**: 新 `internal/attestation/` 包 · `internal/persist/` · `internal/converge/` · `cmd/forge/`  
**当前代码**: 零实现

### 现状

ForgeOS 的治理系统是目前业界最完整的 AI 治理执法之一——8 项架构检查、10 项治理完整性检查、secret 扫描、SCA 框架。但所有这些治理证据**无法被第三方验证**：

```
forge accept: ACCEPTED   ← 谁信？只有你的终端信
```

治理证据的保存形态：
- **`.forge/checkpoint.json`** — 纯 JSON，无签名，可被任意篡改
- **`.forge/trace.jsonl`** — 事件日志，无防篡改保护
- **`harness/arch/arch-check.mjs` 的输出** — 只打印到 stdout，不持久化
- **`internal/doctor/status.go`** — 纯诊断，无加密证据

关键缺失：
1. **无 SLSA 证明**：无法回答「这个二进制的构建过程是否经过了 ForgeOS 治理？」
2. **无可验证审计线索**：`.forge/checkpoint.json` 可以被静默替换，无人能证明「这就是当初运行的结果」
3. **无治理凭据**：CI/CD 系统无法验证「这个 PR 是否通过了 ForgeOS 的安全评审」
4. **无密码学承诺**：workflow 运行的输入（workflow 定义 + agent 输出 + gate 结果）没有内容寻址哈希

### 为什么已有分析未覆盖

| 相近方向 | 来自 | 与本文差异 |
|---------|------|-----------|
| **安全检查/secret-scan** | 方向五（扩张五方向）+ ROADMAP.md | 关注的是**治理本身的内容**（有没有 secret、有没有安全漏洞）。本文关注的是**治理证据的可验证性**——你能证明治理发生过且结果未被篡改 |
| **审计回放** | `architectural-expansion-perspectives.md` 方向五 | 关注的是**回放 trace 诊断问题**（operator 调试）。本文关注的是**密码学级别的不可抵赖性**——第三方（审计员、监管机构）不信任你的终端也能验证 |
| **多仓库联邦的信任问题** | `expansion-horizon-three.md` 方向一 | 关注的是**仓库间如何共享治理策略**（策略推送/继承）。本文关注的是**治理结果如何被跨系统验证**（验证者不需要信任 ForgeOS 实例） |
| **SCA 安全扫描框架** | `genuine-architectural-gaps-v28.md` 方向四 | 关注的是**依赖漏洞检测**。本文关注的是**整个治理管道输出的密码学签名** |

### 代码级证据

1. **`internal/persist/checkpoint.go:Save`** 和 **`internal/persist/checkpoint.go:Load`** — checkpoint 读写是纯 JSON 序列化/反序列化，无签名验证：

```go
// forge-core/internal/persist/checkpoint.go
func Save(cp Checkpoint, path string) error {
    data, err := json.MarshalIndent(cp, "", "  ")  // ← 无签名
    // ...
    os.WriteFile(path+".tmp", data, 0644)            // ← 无加密校验
    os.Rename(path+".tmp", path)                     // ← 原子但不可验证
}
```

2. **`internal/trace/trace.go:WriteEvent`** — trace 事件是 append-only JSONL，无完整性保护

3. **`harness/arch/arch-check.mjs`** — 架构检查结果只输出到 stdout/stderr，不写回结构化持久化

4. **`cmd/forge/gates.go:gatherSignals`** — convergence 信号是纯内存结构，不写入带签名的 attestation

### 建议实现轮廓

```
# 层级 1：Checkpoint 签名（最小可行）
Checkpoint 加签名字段：
  - hash: sha256(content)     → 内容寻址
  - signature: ed25519(...)   → forge key 签名
  - public_key_fingerprint    → 用于验证的 key 标识

# 层级 2：Trace 完整性链
每个 trace event 包含：
  - prev_hash: sha256(上一个 event)  → hash chain
  - event_hash: sha256(当前 event)   → 自验证

# 层级 3：SLSA 证明（v2+）
ForgeOS 输出 in-toto attestation：
  - subject: github.com/org/repo@sha256:abc123
  - predicate: https://slsa.dev/verification/v1
  - 内容：governance_passed: true | false
         gates: [lint:PASS, test:PASS, arch-check:PASS, ...]
         builder: forge-core/v2.5.0
         run_id: forge-run-20260709-abc123
```

**新代码量估计**: ~1200 行（ed25519 签名 + hash chain + in-toto 格式 + `forge verify` 命令）

### 为什么需要

ForgeOS 的**最大卖点**是治理。但治理的价值取决于**可证明性**——如果治理结果不能被外部验证，那它和「开发者自称代码质量很好」没有本质区别。在受监管行业（金融、医疗、国防），可验证的治理证据不仅是好的，而且是**必须的**。SLSA/in-toto 是行业标准，ForgeOS 作为「AI 软件工厂的操作系统」，出厂就该支持。

---

## 方向四：组织级成本治理与分摊（Organizational Cost Governance）

**类型**: 功能 · 经济学  
**优先级**: P1（真点火后成本是采纳的第一障碍）  
**代码影响**: `internal/routing/` · `cmd/forge/cost.go` · `cmd/forge/engine_build.go` · 新 `internal/cost/` 包  
**当前代码**: 零实现

### 现状

当前成本模型完全是**单用户、单项目、per-call 级别的硬限额**：

```go
// forge-core/cmd/forge/engine_build.go:232-259
// 成本决策：
//   1. spendRatio < 0.80 → 不变
//   2. 0.80 <= spendRatio < 1.00 → 非安全角色降一档
//   3. spendRatio >= 1.00 → 硬停
```

CLI flags 全是单次运行的边界：
```
--max-agent-calls   → per-run 计数上限
--agent-max-budget-usd  → per-agent-call 美元上限
--run-budget-usd    → per-run 美元上限
```

数据存储也是单项目粒度：
- **`.forge/trace.jsonl`** — 按 repo 隔离，无全局视图
- **`cost.go` 的 `CostSummary`** — 内存结构，不持久化

组织级需求完全不存在：
- **无多团队预算分配**：团队 A 和团队 B 各有每月 $500 预算
- **无成本归因**：一个 evolve 迭代花了 $12.50，谁花的？哪个团队？哪个项目？
- **无预算滚动**：本月没用完的预算能否滚到下月？
- **无成本预警**：预算用掉 80% 时通知团队 lead
- **无费用分摊报告**：`forge cost-report --team platform --month 2026-06`
- **无成本优化建议**：`forge cost-optimize` → 「你的 review 阶段 60% 的 token 花在 Opus 上，但 40% 的 review 变更很小，建议降档到 Sonnet」

### 为什么已有分析未覆盖

| 相近方向 | 来自 | 与本文差异 |
|---------|------|-----------|
| **经济治理层（ROI-aware budget）** | `next-five-frontiers.md` 方向一 | 关注的是**单次运行的投入产出决策**（这个 task 值不值 $0.50）。本文关注的是**组织级的多团队成本管理**（月度预算、归因、分摊、预警） |
| **成本智能（budget guard 多维度）** | `expansion-production-readiness.md` 方向四 | 关注的是 per-gate/per-phase 的预算分配。本文关注的是 cross-project/cross-team 的成本治理 |
| **预算治理（PDP 策略引擎）** | FUNCTIONAL_REQUIREMENTS_AUDIT.md DEFERRED-BY-DESIGN | 关注的是「用 OPA/Rego 做策略引擎」。本文关注的是「成本数据的组织级可见性和控制」——预算不只是一个数字上限，是一个管理流程 |

### 代码级证据

1. **`cmd/forge/cost.go` 的 `CostSummary`** — 只有 per-run 的简单结构：

```go
type CostSummary struct {
    TotalCostUSD     float64
    TotalInputTokens  int
    TotalOutputTokens int
    NumPhases        int
}
```

没有 `Team`、`Project`、`CostCenter`、`BudgetPeriod` 字段。

2. **`cmd/forge/scorecard_wind.go`** — scorecard 按 `(model, task_type)` 聚合成本，没有组织维度

3. **`.agent/project.yml`** — 项目配置有 `mode`/`lifecycle`，没有 `team`/`cost_center`/`budget` 字段

4. **`internal/trace/trace.go:Event`** — trace 事件有 `Kind`/`Name`/`DurationMs`/`CostUsdMicros`，没有 `Team`/`CostCenter`

### 建议实现轮廓

```
# project.yml 扩展
team: platform
cost_center: eng-infra
monthly_budget_usd: 500

# 新数据结构
Budget {
    Team         string
    Period       BudgetPeriod  // monthly / quarterly
    LimitUSD     float64
    SpentUSD     float64
    Alerts       []BudgetAlert // 80% → warn, 100% → block
}

# forge commands
forge budget set --team platform --period monthly --limit 500
forge budget status                     # 当前消耗 vs 预算
forge cost-report --team platform       # 团队成本报告
forge cost-optimize                     # 自动优化建议
```

**新代码量估计**: ~1500 行（预算模型 + 持久化 + 报告生成 + CLI）

### 为什么需要

Sprint 24-26 的真点火证明 ForgeOS 是**真烧钱的**（每个 agent phase ~$0.18，一次完整 evolve 可能 $5-20）。在 throwaway 验证阶段这不是问题。但在组织采纳中，**成本是不可回避的第一障碍**——不是「能不能跑」，是「谁付钱、花多少、值不值」。没有组织级成本治理，ForgeOS 在企业中的采纳会被「不知道这会让云账单涨多少」一票否决。

---

## 方向五：人类开发者协作接口（Human-Developer Collaboration Layer）

**类型**: 功能 · 产品设计  
**优先级**: P2（不影响核心闭环，影响采纳深度）  
**代码影响**: 新 `internal/collaboration/` 包 · `cmd/forge/` · `internal/orchestrator/`  
**当前代码**: 零实现

### 现状

ForgeOS 的人机交互目前只有两个触点：

1. **`--approved` flag**（Human Approval gate）—— 二元批准/拒绝：

```yaml
# design.yml
stop_condition:
  type: human_gate
  human_approval: required
  on_approve:
    next_stage: review
```

2. **用户被动读日志**——看 stdout/stderr 了解运行情况

这两个触点有共同的假设：**人是「门卫」，不是「协作者」**。

真实世界的人在 AI 辅助开发中扮演的角色远比「批准/不批准」丰富：

| 角色 | 需要 | 当前支持 |
|------|------|---------|
| **开发者** | 审阅 agent 改了什么（diff + 上下文） | ❌ CLI 日志只有 phase 完成/失败，无 diff |
| **开发者** | 选择性批准 agent 的改动 | ❌ 只有「全接受/全拒绝」 |
| **开发者** | 在 workflow 运行中注入上下文 | ❌ 只能等下次迭代 |
| **开发者** | 收到需要人介入的通知 | ❌ 无 webhook/email/Slack |
| **开发者** | 暂停运行、检查状态、修改参数后继续 | ❌ 无 pause/resume 命令 |
| **架构师** | 审计 agent 的决策过程 | ❌ trace.jsonl 是机器格式 |
| **技术经理** | 看团队项目的治理健康度 | ❌ 只有 `forge doctor` 单点诊断 |

### 为什么已有分析未覆盖

| 相近方向 | 来自 | 与本文差异 |
|---------|------|-----------|
| **Human Approval 闸门** | Sprint 6/7 + ROADMAP.md | 关注的是**二进制门控**（批准/拒绝）。本文关注的是**开发者作为协作者的全流程参与**——审阅 diff、选择性批准、注入上下文、暂停恢复 |
| **CLI DX / Shell 集成** | `systemic-expansion-v26.md` 方向二 | 关注的是 CLI 本身的**操作体验**（补全/颜色/进度条）。本文关注的是**人与 AI 代理之间的协作协议**——不是一个更好的 CLI，是一个新的交互模型 |
| **跨运行审计回放** | `architectural-expansion-perspectives.md` 方向五 | 关注的是**operator 排错**（trace 回放诊断）。本文关注的是**开发者主动参与工作流决策**（pause/modify/resume/notify） |
| **交互式引导 / forge tutorial** | `expansion-self-governance.md` | 关注的是**初次使用者的学习路径**。本文关注的是**有经验的开发者在日常使用中的协作需求** |

### 代码级证据

1. **`internal/orchestrator/orchestrator.go:Engine`** — Engine 没有 `Pause`/`Resume`/`InjectContext` 方法：

```go
type Engine struct {
    Exec           AgentExecutor
    RunGate        func(name string) gate.Result
    // ... 15 个字段，没有 pause/resume/notify
}
```

2. **`cmd/forge/main.go:69-76 subcommands`** — 所有子命令都是**启动后等待完成**，无交互式子命令

3. **`internal/orchestrator/loop.go:LoopEngine.Run`** — 主循环不支持外部暂停/修改

4. **`cmd/forge/prompt_context.go:buildPrompt`** — 上下文注入是「全部注入」，不支持「开发者注入的临时上下文」

5. **`internal/converge/converge.go:Signals`** — 收敛信号是 quantitative（roadmap%/gates status），不含 human context（`developer_note`/`override_reason`）

### 建议实现轮廓

```
# 层次 1：Pause/Resume（最小可行）
forge run build
  ... → 开发者按 Ctrl+Z / 另开终端 forge pause build
  → Engine 暂停 phase 执行，子进程 SIGSTOP
  → forge status 显示 PAUSED
  → forge resume build — 从暂停点继续

# 层次 2：Diff Review（P1）
forge run build --phase implementer
  → implementer 完成后，自动显示 diff：
  ┌─────────────────────────────────────┐
  │ Forge review phase output:          │
  │                                     │
  │  M src/domain/task.go              │
  │  A src/interface/http.go           │
  │  D src/old/legacy.go               │
  │                                     │
  │ Approve? [y/n/d (diff)] > d         │
  │  ─── src/domain/task.go ───         │
  │  +func (t *Task) Validate() error   │
  │  ...                                │
  │  Approve? [y/n] > y                 │
  └─────────────────────────────────────┘

# 层次 3：通知集成（P2）
forge run evolve --notify slack:#forge-alerts
  → budget 80% → Slack 通知
  → human_gate 需要批准 → Slack 交互按钮
  → converge MET → Slack 通知 + 摘要

# 层次 4：协作上下文注入（v2+）
forge run build --context "注意：不要修改 auth 包，正在安全评审中"
  → 该上下文注入当前及后续所有 phase prompt
```

**新代码量估计**: ~2000 行（pause/resume 基础设施 + diff 呈现 + 通知抽象 + `forge collaborate` 命令）

### 为什么需要

ForgeOS 目前的假设是「AI 全自动，人只批准」。但现实世界的人-Al 协作不是这样的——人是**伙伴而不是门卫**。一个开发者可能同意 agent 的架构方案但不同意具体实现，可能想给 agent 补充上下文而不是等下次迭代，可能想暂停运行去检查一个异常而不是 kill 进程重来。没有这些协作能力，ForgeOS 会一直被当作「黑盒自动化工具」而不是「AI 协作平台」——这限制了它在需要深度人机协作的场景（设计评审、安全审计、架构决策）中的采纳。

---

## 优先级总览

| 方向 | 优先级 | 工作量估计 | 杠杆 | 依赖 |
|------|--------|-----------|------|------|
| **① GitOps 控制器** | P1 | ~2000 行 | 从工具→平台，解锁企业采纳 | 无（纯 Go stdlib） |
| **② 工作流测试框架** | P1 | ~800 行 | 治理代码本身可测试，防回归 | 已有编排器测试基础设施 |
| **③ 供应链信任与可验证治理** | P1 | ~1200 行 | 治理可证明，合规前提 | 需 ed25519 密钥管理 |
| **④ 组织级成本治理** | P1 | ~1500 行 | 消除采纳的「第一障碍」 | 基于已有 cost 框架 |
| **⑤ 人类开发者协作接口** | P2 | ~2000 行 | 从自动化→协作，提升采纳深度 | 需 pause/resume 机制 |

**如果只能做三件**：① + ③ + ② —— 分别解决「架构落地」「合规可证明」「治理质量自保」。这三件让 ForgeOS 从「一个很酷的 CLI 工具」变成「一个值得企业信任的平台」。

