# ForgeOS — 扩展方向路线图（基于全局扫描）

> **生成日期**: 2026-06-29
> **分析方法**: 10 轮独立架构扫描 + MQTT/WASM 技术评估 + Sprint 27 可行性验证
> **原则**: 不编造缺口，只标注已验证的问题；每个方向有明确的"为什么需要"
>
> **当前状态**: v2 forge-core 已落地（13 包、7292 LOC 非测试代码、零外部 Go 依赖）
> **总扫描量**: ~29k 行（forge-core Go 7.3k LOC + 测试 11.2k LOC + harness ~10.5k LOC）

---

## 方向总览

| 优先级 | 方向 | 当前状态 | 目标 | Sprint 估计 | 紧急度 |
|--------|------|---------|------|------------|--------|
| 🔴 P0 | **韧性运行时** | 无信号处理，Ctrl+C 数据丢失 | 优雅关闭 + Context 传播 | 1 sprint (S27) | **最高** |
| 🟠 P1 | **诚实反馈闭环** | Scorecard 延迟写入，Memory 无置信度 | 实时聚合 + 记忆可溯源 | 1-2 sprints | 高 |
| 🟡 P2 | **可移植工具链** | 多语言 Gate 依赖主机工具，N/A 掩盖回归 | WASM 可移植 Gate 引擎 | 1-2 sprints | 中 |
| 🟢 P3 | **跨厂商模型路由** | Claude-only（3 档位） | LiteLLM 多厂商决策引擎 | 2-3 sprints | 低（按需触发） |
| 🔵 P4 | **治理完整性补完** | 5 个 Gate 因工具缺失降级为 N/A | CI 全链路覆盖 + 工具链补齐 | 1 sprint | 低 |

---

## 方向 1: 🔴 韧性运行时 — 优雅关闭 + Context 传播

### 现状问题

- **无 `os/signal` 导入** — Ctrl+C 直接杀死进程，无任何清理
- **无 `context.Context` 传播** — `CommandExecutor` 用 `context.Background()` 创建子 context，与父进程生命周期无关
- **并发风险** — `parallel.go` 的 goroutine 没有 panic-recover
- **子进程孤儿** — 进程被杀死后，agent 子进程变成孤儿

### 为什么需要

这是**最紧急**的方向。当前任何一个超过 5 分钟的 `forge evolve` 运行，
如果遇到以下情况之一，将丢失数据且不可恢复：

1. 用户误触 Ctrl+C
2. 系统 OOM killer 选中进程
3. CI 超时杀死进程
4. 网络中断导致 agent hang

**已验证的具体缺口**（分析④）:
- `internal/orchestrator/command_executor.go`: `commandContext()` 从 `context.Background()` 创建新 context，不继承任何取消信号
- `internal/orchestrator/loop.go`: `LoopEngine.Run()` 无 context 参数，无法被外部取消
- `cmd/forge/main.go`: 无 `signal.Notify` 注册
- `internal/orchestrator/parallel.go`: goroutine 无 panic-recover

### 实施方案

- 在 `main.go` 注册 `signal.NotifyContext`（Go 1.16+ 标准库）
- `Engine` 增加 `Ctx context.Context` 字段（向后兼容：零值 = `context.Background()`）
- `LoopEngine.Run()` 增加 `ctx` 参数，每次迭代检查 `ctx.Done()`
- `CommandExecutor.Execute()` 使用父 `ctx` 而非 `context.Background()`
- `parallel.go` 的 goroutine 添加 panic-recover + ctx 检查

### 风险

- **低**：纯标准库，无外部依赖；向后兼容设计保证现有测试逐位不变

### 产出 ADR

- `docs/adr/0004-forge-core-signal-handling.md`

---

## 方向 2: 🟠 诚实反馈闭环 — 实时 Scorecard + 可溯源 Memory

### 现状问题

- **Scorecard 延迟写入** — 直到 evolve 循环结束才 wind-down 写入，中间无法观察路由决策质量
- **Memory 无置信度** — `memory.jsonl` 是 append-only 的，错误的洞察被当作与正确洞察同等对待
- **自我强化风险** — 收敛判断基于 agent 自报告的 RoadmapCompletion，系统信任 agent 的诚实度
- **N/A 与 PASS 不可区分** — 在 prompt 上下文中，N/A 渲染与 PASS 相同，agent 无法判断某个 gate 是真的通过还是根本没被检查

### 为什么需要

这是**最危险的系统性风险**（分析⑤）。当前的反馈循环存在以下隐患：

1. **记忆污染** — 一个错误的 insight 被写入 memory 后，后续所有迭代都会读取它并可能被其误导
2. **延迟反馈** — 如果 evolve 运行 100 次迭代后才写入 scorecard，前 99 次的路由决策都没有被评估
3. **自我报告偏差** — 系统信任 agent 说"Roadmap 完成"就完成，没有独立验证机制

**已验证的具体缺口**（分析⑤）:
- `internal/memory/memory.go`: `Append` 和 `BuildPrompt` 无置信度字段，所有记录权重相同
- `forge-core/cmd/forge/scorecard_wind.go`: `windDownScorecards()` 只在 `execEngine` 结束时被 defer 调用
- `internal/converge/converge.go`: `roadmapCompletion` 表达式完全信任 agent 的自报告
- prompt 模板中 N/A 与 PASS 渲染不可区分

### 实施方案

**Phase A（低改动）**:
- `memory.jsonl` 增加 `confidence` 字段（默认 1.0，agent 可自标注或系统可降权）
- `BuildPrompt` 按 confidence 排序，低置信度记录排在后面
- Scorecard 在每次迭代结束时写入（而非整个 evolve 结束时），增加 `iteration` 字段

**Phase B（中改动）**:
- 为 convergence 增加独立验证：`roadmapCompletion` 不仅看 agent 自报告，还看实际文件变更（`git diff --stat` 是否覆盖 Roadmap 条目）
- N/A 与 PASS 在 prompt 中用不同标记区分（`[N/A]` vs `[PASS]`）

### 风险

- **中**：memory confidence 需要设计合理的默认值（不能因为没标注就默认不可信）
- **中**：convergence 独立验证可能引入与 agent 不一致的判断

### 依赖

- 方向 1（Context 传播）完成后，scorecard 的迭代写入可在 ctx 取消时优雅 flush

---

## 方向 3: 🟡 可移植工具链 — WASM Gate 引擎

### 现状问题

- **主机工具链依赖** — `harness/adapters/{go,python,typescript}.yml` 要求主机安装 eslint、ruff、golangci-lint 等
- **N/A 掩盖无声回归** — 如果主机缺少某个工具，gate 降级为 N/A，不会 FAIL 也不会 PASS，回归被隐藏
- **工具版本漂移** — 不同 CI runner 安装的工具版本不同，gate 结果不可重现
- **零外部依赖承诺被违背** — python3 + PyYAML 是 harness 的必需外部依赖

### 为什么需要

这是一个**工程完整性**问题（分析③、分析⑦）。当前：

1. 5 个 gate 在 ForgeOS 自身的 `forge accept` 中因工具缺失降级为 N/A（coverage、typecheck、build、app_test、architecture）
2. N/A 与 PASS 在 prompt 中渲染相同，agent 无法区分
3. 新项目初始化后（`forge init`）需要手动安装所有 lint 工具才能 gate 不降级

**已验证的具体缺口**（分析⑦）:
- `harness/acceptance.mjs`: 5/14 项 gate 在 ForgeOS 自己上为 N/A
- `harness/adapters/go.yml`: 声明需要 `golangci-lint`，但主机可能未安装
- `harness/adapters/python.yml`: 声明需要 `ruff` + `pytest`
- `harness/adapters/typescript.yml`: 声明需要 `eslint` + `tsc`

### 实施方案

- 使用 `wazero`（Go 原生 WASM 运行时，**零外部依赖**）
- 将关键 lint 工具预编译为 WASM 模块（如 ESLint 已有 WASM 版本）
- `adapters/*.yml` 增加 `wasm_gate` 字段，检测到 WASM 模块时优先使用
- 无 WASM 模块时退回到 host 命令（向后兼容）

### 风险

- **高**：很多 lint 工具没有 WASM 构建路径（golangci-lint、ruff）
- **高**：WASM 模块无法访问宿主文件系统（需要 WASI 配置）
- **中**：`wazero` 是第三方 Go 库，打破"零外部依赖"原则（但可 vendored）

### 建议

先从**已有 WASM 构建的工具**开始（ESLint WASM 版本可用），作为试点。
Go lint 工具链的 WASM 化需要上游支持，不在 ForgeOS 控制范围内。

---

## 方向 4: 🟢 跨厂商模型路由 — 多厂商决策引擎

### 现状问题

- **Claude-only** — `internal/routing/tiers.go` 只定义了 haiku/sonnet/opus 三档
- **单厂商风险** — Anthropic API 故障时整个系统不可用
- **成本优化受限** — 无法利用不同厂商的价格差异
- **`policy.yml` 已有占位** — `cross_vendor_pool_v3` 字段已声明但未激活

### 为什么需要

这是**成本与可用性**问题。当前：

1. 所有 agent 调用必须经过 Anthropic，无故障转移路径
2. `policy.yml` 的 `provider_pool: claude-only` 硬编码了单一厂商
3. 不同任务类型适合不同模型（代码生成 vs 架构评审 vs 安全审计），当前路由维度有限

**已验证的具体缺口**（分析⑧、分析⑩）:
- `internal/routing/tiers.go`: 只有 3 个 Claude model，无多厂商扩展点
- `.agent/routing/policy.yml`: `cross_vendor_pool_v3` 声明 `status: not_active_in_v1`
- `scorecard.json` 的 `(model, task_type)` 主键已为多厂商场景设计好

### 实施方案

- 保持当前 `internal/routing` 的评分框架（6 维权重 + task_type 下限已足够）
- 增加 LiteLLM 客户端作为 `tier` 的抽象层
- 扩展 `tiers.go` 支持 `(vendor, model)` 复合键
- scorecard 聚合增加厂商维度比较

### 风险

- **中**：LiteLLM 是外部依赖（Python），需要在 Go 侧通过 subprocess 或 HTTP 调用
- **中**：不同厂商的成本模型不同（按 token、按输出、按推理步骤），预算守卫需适配
- **低**：`policy.yml` 已有占位，改动是增量式的

### 触发条件

- 有第二厂商接入需求时（成本或可用性）
- 当前 `claude-only` 模式下，三个档位已足够覆盖大部分场景

---

## 方向 5: 🔵 治理完整性补完 — CI 全链路覆盖 + 工具链补齐

### 现状问题

- **CI 不完整** — `forge accept` 自身有 5 个 gate 为 N/A（coverage、typecheck、build、app_test、architecture）
- **`go test -race` 不在 CI 中** — 分析⑦ 确认 CI 不跑 `-race`
- **`node --test harness/` 不在 CI 中** — CI 只跑 `node harness/acceptance.mjs`，不直接跑 harness 单元测试
- **Build gate 为 N/A** — `go build ./...` 失败不会被 CI 捕获

### 为什么需要

这是一个**最低成本、最高确定性回报**的方向。当前 CI 的盲区意味着：

1. 一个数据竞争 bug 可能在开发环境被发现，但 CI 不会拦截
2. 一个 `go build` 失败会直接进入 main 分支
3. harness 的单元测试变更不会独立验证（只通过 acceptance.mjs 间接测试）

**已验证的具体缺口**（分析⑦、分析⑧）:
- `.github/workflows/forge.yml`: 不跑 `-race`，不跑 `node --test harness/`
- `forge accept`: 5 N/A 项中 build gate 最危险（意味着 broken build 不被拦截）
- 无 `go build ./...` 步骤在 CI 中

### 实施方案

- `.github/workflows/forge.yml` 增加 `go test -race ./...` 步骤
- 增加 `go build ./...` 步骤
- 增加 `node --test harness/` 步骤
- 补充 harness 工具链：安装 coverage 工具使 coverage gate 不再 N/A

### 风险

- **极低**：纯 CI 配置改动，不触及任何运行时代码

### 为什么优先级最低

因为它**不需要架构决策**——只是把已有的工具链补齐。但它的**实际风险最高**，
因为当前 CI 的盲区可能已经让 bug 流入 main 分支。

---

## 方向之间的关系

```
方向 1 (韧性运行时)          方向 2 (诚实反馈闭环)        方向 3 (可移植工具链)
     │                            │                          │
     ▼                            ▼                          ▼
  Context 传播 ──────────────→  Scorecard 实时写入        WASM Gate 减少 N/A
  优雅关闭                    Memory 可溯源               确定性工具版本
  并行安全                    收敛独立验证                  │
     │                            │                          │
     └────────────┬───────────────┘                          │
                  ▼                                          ▼
         方向 4 (跨厂商模型路由) ←──────────────────── 方向 5 (治理完整性补完)
              │
              ▼
     多厂商路由 → 成本优化 → 故障转移
```

**依赖关系**:
- 方向 1 是**所有方向的前提**：没有 context.Context 传播，方向 2 的 scorecard 实时写入
  在取消时无法优雅 flush；方向 3 的 WASM Gate 执行也需要 ctx 超时控制
- 方向 5 是**最低成本起点**：纯 CI 改动，1 天完成，应立即执行
- 方向 2 和方向 3 可以**并行推进**：一个改内存/反馈逻辑，一个改外部工具链
- 方向 4 **依赖现有路由框架成熟**：等方向 1-3 完成后，路由框架已足够稳定，
  再增加厂商维度风险最低

---

## 推荐执行顺序

| 阶段 | 方向 | 为什么 | 时间估计 |
|------|------|--------|---------|
| **立即** (Sprint 27) | 方向 5 → 方向 1 | CI 补完（1 天） + 韧性运行时（1 sprint）是最高回报组合 | 1 sprint |
| **近期** (Sprint 28-29) | 方向 2 (Phase A) | Scorecard 实时写入 + Memory confidence，低改动高价值 | 1-2 sprints |
| **中期** (Sprint 30+) | 方向 2 (Phase B) | 收敛独立验证，需要设计独立验证逻辑 | 1 sprint |
| **评估中** | 方向 3 | WASM 可行性取决于工具链的 WASM 支持度，需要先做 POC | 不确定 |
| **按需触发** | 方向 4 | 第二厂商接入需求出现时再启动 | 按需 |

---

## 总结

ForgeOS v2 是一个**架构干净但韧性不足**的系统。核心循环（discover → design → implement → gate → review → converge）已经端到端跑通，
但**缺少生产级系统必备的基础设施**：

1. **没有优雅关闭** — Ctrl+C 数据丢失（方向 1）
2. **没有诚实反馈** — 记忆不可溯源、收敛自我报告（方向 2）
3. **没有可移植工具链** — Gate 结果依赖主机环境（方向 3）
4. **没有厂商冗余** — 单点 Anthropic 故障导致不可用（方向 4）
5. **CI 不完整** — 5 个 gate 为 N/A，race detector 不跑（方向 5）

**最优先的行动**：在下一个 sprint 中完成方向 5（CI 补完，1 天）+ 方向 1（韧性运行时，1 sprint）。
这两个方向合计改动量约 300 行代码（含测试），零外部依赖，但将系统的**数据安全性**从"不可接受"提升到"生产就绪"。

*生成日期: 2026-06-29 | 基于 10 轮架构扫描 + MQTT/WASM 评估的综合发现*
