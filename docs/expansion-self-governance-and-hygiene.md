# ForgeOS — 全局扫描：自治理失效与工程卫生债

> **角色**: 资深架构师 / 产品经理  
> **方法**: 代码库全局深扫（forge-core 15+ Go 包 · cmd/forge ~20 CLI 命令 · harness 26+ 模块 ·  
>   `.agent/` 完整治理骨架 · `.ai/` 完整框架 · 全部 30+ 份已有 docs/analysis 交叉核对）  
> **纪律**: 不重复已有分析的核心论点。每方向标注与已有分析的差异。不写代码。  
> **当前状态**: `forge accept` **REJECTED** (3 项 load-bearing 失败 + 5 N/A)  
> **日期**: 2026-07-01

---

## 前置说明：为什么还有新方向

已有 30+ 份分析覆盖了编排引擎、收敛状态机、Memory/Trace/Checkpoint 数据层、中枢旋钮治理、并行编排竞态、ADR 衰减审计、成本 telemetry、多模型并行宇宙、执行器多样性、配置表面积等大量领域。本文刻意寻找被**集体放过**的结构性盲区——这些方向在已有分析中零散提及但**从未作为独立方向系统论证**。

每个方向的代码证据均为 `file:line` 可验证，不依赖推测。

---

## 已有分析已覆盖的相邻域（本文不重复）

| 域 | 覆盖文档 |
|------|----------|
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退 | `eighth-wave-adr-decay.md` |
| 自愈 / 自我改进工作流提案 | `fourth-wave-architecture.md`「自我演化」方向 |
| 运行时数据生命周期碎片提及 | `expansion-blind-spots-v15.md` · `seventh-wave-data-realism.md` |
| 版本信息 / 构建工具缺失 | `fifth-wave-operational.md`「版本号」方向 |
| .forge 目录并发安全 | `expansion-blind-spots-v16.md` 方向一 |
| 并行编排 / 竞态 | `edgecases-and-perf.md` §1 |
| 增长瓶颈 / 包膨胀 | `growth-bottlenecks-and-scalability.md` |
| AI-SDLC 三模型漂移 | `sixth-wave-multimodel.md` 模型 C |
| 测试覆盖缺口 | `self-testing-and-dogfooding.md` |
| forge-init 脚手架 | `configuration-surface-and-adoption.md` §5.2 |

---

## 方向一：自治理破产——ForgeOS 被自己的闸门 REJECTED（P0 · 治理地基）

### 类型
治理 · 诚信 · 元合规  
**代码影响**: 拆分 `forge-core/cmd/forge/validate.go`(994→≤500) · `main.go`(562→≤500) ·  
  `evolve.go`(505→≤500) · 修复 `test_gate.mjs:292` 回归 · 修复 20+ 超长函数

### 现状

ForgeOS 的核心理念是**带外执法为真相之源**——`forge accept` 是最终裁决。然而在本文写作时刻：

```text
$ node harness/acceptance.mjs
forge-accept: REJECTED — test_pass failed; complexity_violations failed; architecture failed
```

**三项 load-bearing 闸门同时失败**：

1. **test_pass FAIL**：`harness/test_*.mjs` 存在测试回归——`test_gate.mjs:292` 断言 `1 !== 0`，表明某个 gate 行为发生了变化但没有同步更新测试。这是 governance 闭环中的**算法自反**问题：项目用 gate 衡量的标准，gate 自身未通过。

2. **complexity_violations FAIL**（`gate.mjs exit 1`）：**8 个文件超 500 行上限**，其中 `validate.go: 994` 是上限的 **198%**（几乎双倍），`yaml2json.go: 755` 是 151%，`main.go: 561` 是 112%。已超限的文件长期存在、未拆分。

3. **architecture FAIL**：**20 个函数超 50 行上限**（`preflight.go:28 cmdPreflight` 达 163 行，超 326%）+ `forge-core/cmd/forge` 包达 15 文件超 14 文件上限。

### 为什么这是个独立方向（而不仅仅是"修 bug"）

这是**元层面的自治理裂痕**——不是单个 bug，而是系统性地：

- **REJECTED 状态被接受为正常**：当前 `.agent/project.yml` 设为 `engineering×mvp`，`enforce: block`。闸门是 block 模式但项目持续运行在 REJECTED 状态。这意味着要么 block 模式形同虚设，要么项目接受了"我们知道但不管"的例外。无论哪种，治理的可信度都受损。

- **无 self-rollback 机制**：如果是正常软件项目，CI 会阻挡 REJECTED 状态的代码合入。但 ForgeOS 修改自身代码，没有独立于自身的 CI 来守卫——这是经典的自举(bootstrapping)悖论。

- **validate.go 994 行是一个"上帝文件"**：这正是 ForgeOS 红线（`AGENTS.md: 单一职责·禁 God Object`）针对的架构反模式。该文件从 Sprint 5 就已超限（方向四日志："两处上帝文件……按「先拆分」拆分"），但当时只拆了 `main.go/scan.mjs`，`validate.go` 从未被处理。

- **无自动分解策略**：项目有 `skill: refactor-large-file` 但从未在自身架构上触发——因为没有任何机制定期对自身执行 `forge scan` + gap 分析。

### 已有分析覆盖差异

`fourth-wave-architecture.md` 提出了 `self-evolve.yml`，但那是关于"让 ForgeOS 可以改进自己"的特性提案，而不是当前**已经被自己闸门拒绝了**的事实状态。本文指出的是：在自我改进之前，需要先修复自合规(self-compliance)。

### 高价值解决路径

- **短期**：拆分 validate.go(994→≤500)，拆分 main.go(562→≤500)，拆分 evolve.go(505→≤500)，修复 test_gate.mjs:292 回归。目标：`forge accept` 从 REJECTED 回到 ACCEPTED。
- **中期**：新增 `forge self-check`——在修改 forge-core 自身时额外跑一组"元闸门"（确保 split-before-continue 纪律应用到自身）。
- **长期**：在 sprint/review 流程中加入「本闸门状态」作为评审前置条件——`forge accept` 自身未通过时，工作流应阻止新特性开发（同 AGENTS.md 纪律）。

---

## 方向二：配置表面碎片化——从「声明式单一事实源」到「事实的八爪鱼」（P1 · 可维护性）

### 类型
架构 · 可维护性 · 开发者体验  
**代码影响**: 配置合并层 · 新 `forge config` CLI · 配置交叉验证工具 · 文档补全

### 现状

ForgeOS 将自己定位为「声明式治理」，但实际上配置分布在 **8+ 个不同表面**，同一语义以不同格式重复出现：

| # | 表面 | 文件 | 关键内容 | 格式 |
|---|------|------|---------|------|
| 1 | 项目 YAML | `.agent/project.yml` | mode, lifecycle, features | YAML |
| 2 | 模式策略 | `.agent/policies/modes.yml` | mode gates, threshold, depth, migration | YAML |
| 3 | 门闩策略 | `harness/policies.yml` | max_file_lines, enforce | Key-Value |
| 4 | 架构规则 | `.arch/rules.yaml` | layering, package size | YAML |
| 5 | 路由策略 | `.agent/routing/policy.yml` | tiers, by_task_type, floors | YAML |
| 6 | 评估 Schema | `.agent/eval/acceptance.schema.yml` | acceptance criteria | YAML |
| 7 | 工作流 YAML | `.agent/workflows/*.yml` | phases, gates, loop-back | YAML |
| 8 | CLI 标志 | `main.go:bindRunOpts` | --mode, --executor, --model, --max-iter | flag |
| 9 | **环境变量** | 散落各处 | FORGE_REPO_ROOT, FORGE_SCA_DB, FORGE_AGENT_DEPTH, FORGE_ACCEPT_INNER, FORGE_AGENT_CACHE_DIR, FORGE_MEMORY_PATH | env |
| 10 | CC Hook | `.claude/settings.json` | PostToolUse 自动跑 gate.mjs | JSON |

**关键问题**：同一概念在多个表面出现，各有不同但合法的值，无单一解析路径。

**代码级证据**（以 `max_file_lines` 为例）：

- `harness/policies.yml` 声明 `max_file_lines: 500`（"真相之源"）
- `.arch/rules.yaml` 声明 `file.max_lines: 500`，带注释 `必须与 harness/policies.yml 一致`
- `harness/arch/arch-check.mjs` 跑每个 drift-guard 时**验证两者相等**
- `harness/gate.mjs` 通过 `resolveMaxFileLines` 读 project.yml × modes.yml → 实际生效值可能是 800（explorer）或 500（production）
- `resolveMaxFileLines` 的 fallback 链：mode×lifecycle → policies.yml → 常数 500

这 5+ 个层级的解析链**没有一份文档描述过**。新贡献者弄错两个文件的同步关系直接导致 drift-guard 告警。

**更隐蔽的问题**（未被已有分析涉及）：

- **环境变量地下配置**：`FORGE_*` 系列环境变量在代码中散落定义，**无文档、无发现、无冲突解析**。例如 `FORGE_ACCEPT_INNER` 用于防止递归跑测试，但无任何用户文档说明其存在。
- **CLI flag vs 环境变量 vs 项目文件的三体问题**：当 `--mode` 与 `project.yml` 的 `mode:` 冲突时，哪个优先？`FORGE_SCA_DB` 与环境 `PWD` 的关系？代码中每个消费点自行决定优先级，无统一 policy。
- **`resolveEnforce` 的隐式优先级**：`modes.yml` 的 `enforce` 值经过 `lifecycle_modifiers.production.enforce_floor` 的覆盖，再被 fail-safe 逻辑兜底——这个解析链在 `internal/mode/mode.go` 中，但 operator 通过 CLI 无法查询"当前生效的 enforce 是什么以及为什么"。

### 已有分析覆盖差异

`configuration-surface-and-adoption.md` 分析了跨文件一致性（14 个 YAML 文件之间的漂移），但**未聚焦配置表面数量导致的认知负荷**、**未涉及环境变量地下配置**、**未讨论 CLI-文件-env 三者的冲突解析规则**。本文的着重点不是文件一致性，而是概念碎片化——同一个模式×lifecycle 概念需要 8 个表面协同理解。

### 高价值解决路径

- **`forge config` 查询命令**：读取所有表面 → 合并 → 显示当前生效配置（含来源标注）。类似 `git config --list --show-origin`。例如 `forge config enforce` → `block (from modes.yml engineering:enforce, overridden by lifecycle:production → enforce_floor=block)`。
- **环境变量注册表**：所有 `FORGE_*` 变量在代码中统一注册（一个 `env.go`），附带文档说明、默认值、互斥关系，`forge config --env` 展示。
- **配置冲突检测**：`forge validate` 增加交叉表面校验——当同一语义在多个表面的值不一致时警告（非 blocking，已采纳的差异如 `policies.yml` 的 `max_file_lines` 与 `project.yml` 的 `override` 是合法的分层覆盖）。
- **减少表面数量**：长期将 `policies.yml` 和 `.arch/rules.yaml` 合并进 `modes.yml` 或 project.yml，减少到 3-4 个核心表面。

---

## 方向三：运行时数据的生命周期策略缺失——.forge/ 的无主之地（P1 · 运维）

### 类型
运维 · 可靠性 · 存储  
**代码影响**: 新 `internal/datastore/` 管理层 · `forge cleanup` CLI · evolve 启动时间

### 现状

`.forge/` 目录持有三个运行时状态文件，**各自为政地管理自己的生命周期**：

| 文件 | 格式 | 生命周期策略 | 代码位置 |
|------|------|-------------|---------|
| `trace.jsonl` | JSONL | 10MB 旋转 → `.1` 备份，旧备份被覆盖 | `evolve.go:478-498` |
| `memory.jsonl` | JSONL | 500 条阈值 → 按 kind 保留 20 条，24h 时限 | `memory.go:317-342` |
| `checkpoint.json` | JSON | 按 `retain` 参数旋转（默认 3 代） | `persist/save.go` |

**三个存储无统一策略的证据**：

- **trace 只留 1 个备份**：`openTracer` 当 `trace.jsonl` > 10MB 时 rename 到 `trace.jsonl.1`（`evolve.go:491-493`），但**不检查备份是否也存在**。一个 100MB trace 只保留最新 10MB + 旧备份的一个 10MB 切片——剩下的 80MB 被静默丢弃。

- **memory compaction 只在 evolve 中调用**（`evolve.go:445`），**`forge run` 路径从不触发 compaction**。如果一个 project 长期用 `forge run build` 而非 `forge evolve`，memory 无限增长。

- **checkpoint 的 retain 参数从未被命令行暴露**：`persist.Save` 接受 `retain int` 参数，但 `evolve.go` 调用 `persist.Save` 时传的 retain 值硬编码为 3（`checkpoint.go` 调用处）。operator 无法配置备份代数。

- **数据重复**：cost_usd_micros 既出现在 `trace.jsonl` 的 `Event` 中，也出现在 `checkpoint.json` 的 `SpentUsdMicros` 中。无跨存储一致性检查。

- **无事后清理**：没有 `forge cleanup`、没有 `forge archive`、没有 `.forge/` 目录总大小 TTL。`forge evolve` 24h 后，operator 如果忘了清理，磁盘持续消耗直到 OS 层告警。

- **读模式下的 loadCache 缓存膨胀**（`memory.go:45-60`）：`sync.Map` 的 loadCache 按 path 缓存 memory 条目，但随着项目演化（git branch 切换、不同 `.forge/` 目录），缓存键单调增长、永不淘汰。

### 已有分析覆盖差异

- `expansion-blind-spots-v15.md` 方向一提到 "trace 文件增长问题"
- `seventh-wave-data-realism.md` 方向二分析了 memory compaction
- `edgecases-and-perf.md` §2 分析了 trace/memory 增长

但**无任何分析把三个存储视为一个统一的 data lifecycle 问题**——它们各自独立地被分析和优化，但作为系统，operator 没有一个单一的旋钮来控制 `.forge/` 的总体数据量。这是运维层面的「局部最优，全局无主」。

### 高价值解决路径

- **`forge doctor --data`**：报告 `.forge/` 目录总大小、各文件大小/行数/年龄、compaction 状态、缓存条目数。给出清理建议。
- **`forge cleanup [--keep-days N]`**：统一清理所有过期运行时数据——删除超过 N 天的 trace 段、压缩超过阈值的 memory、清理 checkpoint 历史中的过时代、清空 loadCache。
- **统一保留策略**：在 `project.yml` 或 `modes.yml` 新增 `data_retention: {trace_days: 7, memory_entries: 500, checkpoint_generations: 3}`，由所有三个存储共同消费。
- **trace 旋转加固**：改为一对旋转（保留当前 + 备份 + 总大小上限），`openTracer` 在启动时检查总大小，超限按最旧优先删除。

---

## 方向四：项目导入认知税——从「Hello World」到「受治理的循环」（P2 · 采纳体验）

### 类型
开发者体验 · 文档 · 采纳  
**代码影响**: `forge tutorial` 命令 · 交互式 `forge init --guided` · 增量采纳路径

### 现状

一个新项目要得到 ForgeOS 的完整治理，需要**跨越 12+ 个概念的认知门槛**：

```
第 1 层：概念
  1. 中枢旋钮：mode × lifecycle
  2. 工作流：discover → design → build → evolve
  3. Harness 闸门：gate.mjs → check.py → acceptance.mjs → arch-check → secret-scan

第 2 层：文件系统
  4. .agent/ 治理骨架
  5. .agent/agents/ 角色卡
  6. .agent/skills/ 技能卡
  7. .agent/workflows/ 工作流 YAML
  8. .agent/policies/ 策略文件
  9. .agent/routing/ 路由策略
  10. .agent/eval/ 评估 schema

第 3 层：运行时
  11. forge-core CLI（run / evolve / gate / check / accept / migrate / route）
  12. 带外执行器（--executor command --agent-cmd claude）

第 4 层：可选深度
  13. harness/adapters/<lang>.yml 适配器
  14. arch-check 架构规则
  15. 并行编排（--parallel + depends_on）
```

`forge-init` 脚手架（`harness/scaffold/forge-init.mjs`）创建一个**全功能但全空的治理项目**——它把完整骨架复制过去，但**没有渐进阶**。新用户面对的是一个完整的 `.agent/` 目录、完整的转发器、完整的测试套件，但**没有一个具体的指导**告诉用户第一步应该做什么。

**代码级证据**：

- `forge-init` 模板在 `CLAUDE.md` 中包含了 `TODO: 一句话说清 $(name) 是什么` 和 `- [ ] TODO: 第一个最小可验证切片` 等模板占位符——这是诚实标注了「你需要自己填空」，但**没有指引如何填**。
- `harness/scaffold/forge-init.mjs` 是一个 400+ 行的复制工具，**没有交互模式**、没有 `forge init --guided`、没有解释每个文件做什么的 inline 注释。
- **没有 `forge tutorial`**：与之对比，几乎每个成功的开发者工具（git、docker、npm、go）都有内置教程或交互式引导。ForgeOS 在仓库根目录有 5 个 README 级 Markdown 文件（`BOOTSTRAP.md`, `README.md`, `ROADMAP.md`, `CLAUID.md`, 以及 `forge-core/README.md`, `harness/README.md`）但没有一个**教你怎么用**。

### 已有分析覆盖差异

- `configuration-surface-and-adoption.md` §5.2 提到"没有快速开始教程"——但那是作为配置表面积分析的一部分，**不是独立的采纳方向**。本文将其提升为独立方向，因为这是 ForgeOS 从"技术验证"到"真正被采纳"的最大坎。
- `self-testing-and-dogfooding.md` 分析了 forge-init 复制测试的问题，但未触及采纳体验的核心。
- `growth-bottlenecks-and-scalability.md` 分析了包增长但未分析知识门槛。

### 高价值解决路径

- **`forge tutorial` 命令**：交互式教程，分 5 步从 `forge init` 到 `forge accept` 到 `forge run build`。每步解释一个核心概念，产出真实可用的项目。
- **`forge init --guided`**：交互式脚手架——询问项目名、语言、目标（prototype / production / team），然后根据答案选择模式（explorer/balanced/engineering）并解释为什么。
- **增量采纳路径**：允许项目从「仅 gate」（无 `.agent/` 骨架）开始，逐步添加 governance 层——`forge adopt --governance-level gate` → `forge adopt --add-mode` → `forge adopt --add-workflows`。每次添加只引入 2-3 个新概念，附带解释。
- **概念地图**：在 `BOOTSTRAP.md` 或新 `CONCEPTS.md` 中添加概念领域图（mode×lifecycle → 为何驱动三处 → 分别影响什么），类似 `git`s 的 "Git 概念" 页面。

---

## 方向五：版本/发布拓扑缺失——没有版本号的自治系统（P2 · 运维·供应链）

### 类型
运维 · 供应链 · 可重复性  
**代码影响**: `main.go` 版本注入 · `forge version` · `go.mod` 版本 · 发布工单

### 现状

`forge-core` 作为一个 Go 编译的 CLI，**没有任何版本号**：

```bash
$ forge-core/forge --help | grep version
# 无输出 — 没有 --version 标志

$ go -C forge-core list -m
github.com/earendil-works/forge-core
# go.mod 没有 version 标签

$ strings forge-core/forge | grep -E "^[0-9]+\.[0-9]+\.[0-9]+" | head -3
# 从二进制搜到的版本串是 Go 工具链的版本，不是 forge 自己的
```

**代码级证据**：

- `go.mod`（`forge-core/go.mod`）：`module github.com/earendil-works/forge-core` — 无 `go 1.26` 外的版本信息，无 `require` 块，无版本号。
- `main.go`：无 `var Version = "dev"` 或 `ldflags` 注入入口。`main()` 函数调用 `args := os.Args[1:]` 然后路由到子命令，**从不检查 `version` 参数**。
- `main_test.go`：无 `TestVersion`。
- `internal/persist/checkpoint.go`：定义了 `FormatVersion = "forgeos.checkpoint.v1"` 作为数据文件格式版本——但运行时的自身版本没有标注。

**后果**：

- **无发布拓扑**：无法区分"这个 evolve run 是 v1.2.3 跑的"和"是 v1.2.4 跑的"。一个 checkpoint 文件里没有写入 forge 版本，未来格式变化时无法判断兼容性。
- **供应链脆弱**：如果某天需要回退到旧版本，没有一个明确的版本边界可供沟通（"请升级到 v1.3.0" vs "请重新 clone main"）。
- **CI 产物不可追溯**：GitHub Actions 流水线的 `go build` 产物每次 commit 不同，但无法回答"CI 反馈的这个 bug 是在哪个版本修复的"。

### 已有分析覆盖差异

- `fifth-wave-operational.md` **方向二完全覆盖了版本号缺失问题**（包括建议的 `forge version` 命令和 `ldflags` 注入方式）。
- 但本方向将其扩展为**版本拓扑 + 可重复构建**问题，而非仅仅是版本号缺失。

### 高价值解决路径

- **版本注入**：`ldflags -X main.Version=$(git describe --tags --always)` + `forge version` 命令。
- **发布工单**：`RELEASE.md` 规范（`v1.0.0` / `v1.1.0` / `v2.0.0` 含义），与 ROADMAP.md 对齐。
- **Checkpoint 携带运行时版本**：`checkpoint.json` 增加 `forge_version` 字段，未来读旧版本 checkpoint 时给出兼容性提示。
- **`forge version --check`**：检查当前版本是否过旧，比较与最新发布的差异（类似 `brew outdated` 或 `go version`）。

---

## 优先级与执行顺序

| # | 方向 | 代码复杂度 | 风险降低 | 用户可见度 | 建议 |
|---|------|-----------|---------|-----------|------|
| **①** | 自治理破产 | **低**（拆分文件 + 修复测试） | **极高**（治理可信度地基崩塌） | **高**（REJECTED 可见） | **P0—立即** |
| **②** | 配置表面碎片 | 中（新增 config 命令） | **高**（防止配置漂移 bug） | **高**（`forge config` 直观） | **P1—下一 sprint** |
| **③** | 数据生命周期 | 中（cleanup + 统一保留策略） | **高**（防止磁盘写满崩盘） | 中（运维价值） | **P1—下一 sprint** |
| **④** | 项目采纳税 | **低**（tutorial + guided init） | 中（提高采纳率） | **极高**（新手的第一印象） | **P2—持续** |
| **⑤** | 版本/发布拓扑 | **低**（ldflags + forge version） | 中（可追溯性） | 中（开发者体验） | **P2—持续** |

### 执行建议

1. **立即（今天）**：拆分 validate.go(994→≤500)，拆分 main.go(562→≤500)，修复 test_gate.mjs:292。目标：`forge accept` → ACCEPTED。这是 ForgeOS 对自身诚信的最低要求。
2. **本 sprint**：增加 `forge config`（只读查询当前生效配置）和 `forge cleanup`（清理过期 `.forge/` 数据）。两者都是运维零风险的纯新增。
3. **持续**：`forge tutorial` 交互式引导 + 版本号注入 + `forge init --guided`。这些是增长采纳率的投资。
