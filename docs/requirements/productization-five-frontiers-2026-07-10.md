# ForgeOS — 产品化五方向：从「可运行的工程系统」到「可采纳的产品」

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局扫描完整代码库：forge-core（19 Go 包 · ~35k LOC · 纯 stdlib 零依赖）+
>    cmd/forge（40+ 子命令 · ~12k LOC）+ harness（39+ 模块 · ~10.5k LOC 执法层）+
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies/ADR/DECISIONS）+
>    `examples/`（url-shortener · go-taskd）+ `pi-batch.py` + CI 流水线
> 2. 通读 Sprint 1–31 全部演进记录、`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`、
>    .agent/ 全部核心文档、ADR-0001~0004
> 3. **差异化验证**: 在 98+ 份已有需求分析文档（`docs/requirements/*.md`）中逐方向检索，
>    确认每个方向的**核心论点未被已分析作为独立系统性方向展开**。已有文档中出现的
>    相邻概念在本文中明确标注出处和差异。  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据。
> **日期**: 2026-07-10

---

## 全景判断

ForgeOS 经过 31 轮 sprint，工程完备度已达到极高水平——`forge accept: ACCEPTED` 在任何改动后
真实运行多维度验证，98+ 份分析文档覆盖了约 150+ 方向。技术核心（编排/路由/记忆/收敛/安全/
治理/执行语义）几乎无可挑剔。

但站在**产品采纳**的角度，有一个未解决的矛盾：

> **技术完备度 ≈ 95%，采纳就绪度 ≈ 30%。**

ForgeOS 可以自治地构建软件，但要让一个没有参与 31 轮 sprint 的真实团队来使用它、信任它、
集成到自己的流程中，还需要一些关键的「产品化桥梁」。下面的 5 个方向正是这些桥梁——
每个都基于当前代码的确切证据，解决的都不是「加什么新功能」，而是 **「已有功能如何被真实团队采用」**。

---

## 方向一 · `forge approve` 只读不写：半成品的人机交互

**优先级**: 🔴 P0 | **类别**: 产品 · 人机交互 | **预估**: ~0.5 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖验证**: 在 98+ 份分析文档中检索 `approve`/`approval`/`CLI approval`，所有文档讨论的是
**human gate 的概念和机制**（Sprint 6/CURRENT_SPRINT.md），但没有任何文档指出 `forge approve` CLI
本身只有 `list` 命令、写路径标记为 "Future" 这一具体产品缺口。

### 问题描述

`forge-core/cmd/forge/approve.go:13-15` 的第一行注释写着：

```
// Future: `forge approve <stage> --yes` to approve, `forge reject <stage> --reason "..."`.
```

当前 `forge approve` 的实现只有：

```go
func cmdApprove(args []string) int {
    // ...
    if len(rest) > 0 && rest[0] == "list" {
        return cmdApproveList(root)
    }
    // --help or no args: show usage
    fmt.Fprint(os.Stderr, `usage: forge approve list [--root DIR]
  List pending human-gate approvals by scanning .forge/*.approved markers.
`)
    return 2
}
```

整个 `approve.go` 是**只读的**：只能 `forge approve list` 查看有哪些待审批项，但没有任何
`forge approve design --yes` 或 `forge reject design --reason "架构需要重审"` 命令。

用户被迫手动创建 `.forge/<stage>.approved` 文件：

```go
// forge-core/cmd/forge/gates.go:humanApproved
func humanApproved(root, stage string, approvedFlag bool) bool {
    if approvedFlag {
        return true
    }
    _, err := os.Stat(approvalPath(root, stage))
    return err == nil
}
```

这对于一个宣称「Human Approval 是全系统最高杠杆闸门」（PROJECT.md:23）的系统来说，是一个
**产品层面的明显缺口**。审批是全系统中最重要的安全闸门（Design→Build 之间的人控点），
但它的 CLI 交互却是一行注释里的 "Future"。

### 边界情况

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| 新用户看到 human_gate | convergence 输出提示 `pass --approved or create .forge/design.approved` | 应该提示更清晰的 CLI：`forge approve design --yes` |
| 审批后反悔 | 无 API，只能手动删文件 | `forge reject design --reason "..."` 应生成 `.forge/design.rejected` 并触发 on_rejected 路径 |
| 多个待审批 workflow | `forge approve list` 已经支持 | `forge approve list --json` 待支持 |
| 审批备注 | 无 | `forge approve design --note "ADR reviewed, LGTM"` |

### 为什么现在做

Sprint 6 实现了 human_gate 的概念（`.forge/<stage>.approved` 标记），Sprint 31 实现了
`on_rejected` 路径。但这两端之间的 CLI 层——真正的用户交互——从未被连接。这是一个
**薄胶水层缺口**，只需要 0.5 sprint 就能完成，杠杆极高：hunan_gate 是项目最高论点的
核心安全闸门，其 CLI 交互不该停留在 "Future" 注释上。

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| CURRENT_SPRINT.md Sprint 6 | human_gate 概念和收敛信号 | `forge approve/reject` CLI 不完整的具体代码证据 |
| `genuine-five-product-architectural-frontiers.md` | approval-mark 读写框架 | 用户侧 CLI 交互（write path + UX） |
| `forgotten-five-foundations.md` 方向一 | 跨进程锁 | approve 命令的人机交互层 |

---

## 方向二 · `project.yml overrides` 段：无声配置失效

**优先级**: 🔴 P0 | **类别**: 配置 · 可靠性 | **预估**: <0.5 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖验证**: 检索 `overrides`/`max_file_lines`/`project.yml` 在 98+ 份文档中，
有文档讨论 `project.yml` 的 `mode`/`lifecycle` 读取（这是已实现的），但没有任何文档
指出 `overrides` 段被完全忽略是一个配置静默失效问题。

### 问题描述

`.agent/project.yml` 的 `overrides` 段是用户可自定义的配置入口：

```yaml
overrides:
  max_file_lines: 500             # 对齐 harness/policies.yml(真相之源)
  max_root_files: 15
```

但 **forge-core 和 harness 中没有一行代码读取这些值**。

`forge-core/cmd/forge/main.go:projectYAMLValue` 只读取 `mode` 和 `lifecycle`：

```go
func projectYAMLValue(root, key string) string {
    // ...逐行扫描，只返回 key 匹配的第一行值
    // 被调用于 resolveLifecycle() 和 resolveMode()
    // 从不被调用于 overrides
}
```

`harness/gate.mjs` 的 `resolveEnforce` 和大小检查阈值硬编码读取 `harness/policies.yml`：

```javascript
// harness/gate.mjs:75-97 — 读取的是 policies.yml，不是 project.yml
const rules = parseRules(readFileSync(join(HARNESS_DIR, 'policies.yml'), 'utf8'));
```

用户在 `project.yml` 中设定的 `overrides` 会被**静默忽略**。如果用户把 `max_file_lines` 改为 600，
gate.mjs 依然用 policies.yml 的 500；如果改为 400，gate.mjs 依然用 500。没有警告，没有错误——
配置生效为假。

### 边界情况

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| 用户设 max_file_lines: 400（收紧） | 被忽略，仍用 500 | gate.mjs 读到更严值，告警线降低 |
| 用户设 max_file_lines: 600（放宽） | 被忽略，仍用 500 | gate.mjs 读到更松值，告警线升高 |
| 用户设 max_root_files: 10 | 被忽略，仍用 15 | gate.mjs 根文件数上限收紧 |
| 没有 project.yml | gate.mjs 正常用 policies.yml | 保持现有行为（向后兼容） |
| overrides 与 policies.yml 冲突 | 静默忽略 override | 应记录并选择更严格的值 |

### 为什么现在做

这是一个 <0.5 sprint 的修复：`projectYAMLValue` 已经在 main.go 中，只需要扩展它来读取
`overrides` 字段，并在 gate.mjs 中增加 `resolveProjectOverrides()` 来与 policies.yml 合并。
现有的 `resolveEnforce`（Sprint 18）已经示范了「取更严格者」的模式——同样可用于 override 合并。
杠杆高：当前 `overrides` 配置会**误导**用户以为自己设了阈值，实际上从未生效。

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `expansion-production-blindspots-v36.md` | 配置散落问题 | `overrides` 段完全不被读取的代码级证据 |
| `architect-product-perspective-five-novel-directions.md` | project.yml 的 mode/lifecycle 读取 | overrides 段的 silent failure |
| Sprint 18 | mode×lifecycle enforce 解析 | project.yml overrides 的完全缺失 |

---

## 方向三 · 第一次运行体验缺失：没有 `forge init` 快速通道

**优先级**: 🔴 P1 | **类别**: 产品 · 采纳 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖验证**: 在 98+ 份文档中检索 `init`/`onboarding`/`quickstart`/`tutorial`/`hello world` 等关键词，
已有的 `forge-init` 相关讨论（如 `governance-prod-five-frontiers.md`、`expansion-horizon-three.md`）关注的是
**harness 模板复制机制**（scaffold 的正确性和完整性），而非**用户第一次使用的认知路径**。

### 问题描述

当前 ForgeOS 的「从零开始」流程：

```
1. 用户安装 forge（无安装脚本，README 无安装指南）
2. 用户需要知道 `forge init <dir>`（有，在 harness/scaffold/forge-init.mjs）
3. 用户需要知道 `cd <dir> && forge run build` 来验证
4. 但在第 3 步之前，用户需要理解 workflow、mode、lifecycle 等概念
```

没有 `forge demo`、`forge tutorial`、或 `forge init --demo` 来展示一个**端到端可工作的示例**。

具体代码证据：

```go
// forge-core/cmd/forge/main.go:69-82 — 子命令表
var subcommands = map[string]func([]string) int{
    "run":          cmdRun,
    "gate":         func(rest []string) int { return delegate(gate.Gate, rest) },
    "evolve":       cmdEvolve,
    "route":        cmdRoute,
    "migrate":      cmdMigrate,
    "detect":       cmdDetect,
    "scorecard":    cmdScorecard,
    "validate":     cmdValidate,
    "memory-prune": cmdMemoryPrune,
    "status":       cmdStatus,
    "doctor":       cmdDoctor,
    "preflight":    cmdPreflight,
    "approve":      cmdApprove,
    // ★ 注意:没有 "init"、"demo"、"tutorial" 或 "example"
}
```

`forge init` 是一个独立的 Node.js 脚本（`harness/scaffold/forge-init.mjs`），不是 forge-core CLI
的一部分。用户需要知道它的存在并通过 `node harness/scaffold/forge-init.mjs` 调用。

`README.md`（根目录）只包含 12 行简介和一页参考，没有任何「三分钟快速上手」引导。

`examples/url-shortener` 和 `examples/go-taskd` 存在，但它们需要在项目内部运行——用户无法
通过一个简单的 `forge demo` 命令来零认知成本体验 ForgeOS。

### 边界情况

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| 新用户下载 forge | 无安装指南，需要自己发现 `forge` 二进制 | `forge` 无参数时给出清晰的第一条命令建议 |
| 用户想快速体验 | 需要阅读 BOOTSTRAP → PROJECT → ARCHITECTURE → ... | `forge demo` 创建一个临时目录，运行最小 workflow，然后销毁 |
| 用户想初始化项目 | 需要知道 `node harness/scaffold/forge-init.mjs` | `forge init <dir>` 作为 forge-core 子命令 |
| 用户想升版 governance | `forge-upgrade.mjs` 存在 | `forge upgrade` 作为 forge-core 子命令 |
| CI 中首次验证 | 需要手动配置 | `forge init --ci` 一步生成 CI 配置 |

### 为什么现在做

ForgeOS 的技术复杂度要求新用户投入显著的认知成本。没有快速通道的情况下，大多数潜在
采纳者会在「看懂之前」放弃。一个 `forge demo` 或 `forge init --demo` 命令，可以在 10 秒内
创建一个临时项目、运行最小 build workflow、输出 ACCEPTED——新用户全程零配置，直接体验
核心价值。

当前 `detect.go:detectProject` + `suggestWorkflow` + `autoSelectWorkflow`（Sprint 27）已经能
自动检测项目类型并建议 workflow。这条能力线已经存在，只需要一个 `forge init` 薄包装就能
形成完整的新用户到可运行项目的闭环。

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `forgotten-five-foundations.md` 方向五 | 运行时状态自校验 | 用户第一次使用体验的缺失 |
| `operational-product-five-gaps.md` 方向一 | 二进制生命周期 | 没有 `forge init`/`forge demo` CLI 的采纳缺口 |
| `five-genuinely-uncovered-frontiers.md` 方向四 | 多仓库联邦 | `forge init --multi-repo` 场景扩展 |

---

## 方向四 · trace 格式无版本标记：未来迁移的定时炸弹

**优先级**: 🟡 P2 | **类别**: 数据完整性 · 可演化 | **预估**: ~0.5 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖验证**: 在 98+ 份文档中检索 `trace version`/`trace format`/`trace migration`/
`schema version`/`data migration` 等关键词，没有任何文档将 trace 格式的无版本化作为
独立方向展开。`forgotten-five-foundations.md` 方向三（结构化 trace 查询）讨论的是
**查询能力**而非**格式版本化**。

### 问题描述

`internal/trace/trace.go` 的 `TraceEvent` 结构体是 forge-core 学习闭环的核心数据格式——
它承载 cost、latency、model attribution、scorecard rebuild 所需的一切信息。

但该结构体**没有任何版本字段**：

```go
// forge-core/internal/trace/trace.go:40-98
type Event struct {
    Seq           int     `json:"seq"`           // ← 每次 run 从 1 开始，跨 run 不唯一
    Iteration     int     `json:"iteration,omitempty"`
    Phase         string  `json:"phase,omitempty"`
    Model         string  `json:"model,omitempty"`
    DurationMs    int64   `json:"duration_ms,omitempty"`
    CostUsdMicros int64   `json:"cost_usd_micros,omitempty"`
    TaskType      string  `json:"task_type,omitempty"`
    Verdict       string  `json:"verdict,omitempty"`
    // ★ 注意：没有 "version"、"schema"、"schema_version" 字段
    // ★ 没有 "run_id"、"session_id" 字段
    // ★ 没有 "producer_version" 字段
}
```

当前格式决策在 Sprint 5（trace 包创建）时是合理的——一个内部格式不需要版本管理。
但以下是三个使其成为「定时炸弹」的发展：

1. **`forge scorecard rebuild` 依赖 trace 格式**（`scorecard_wind.go:236-330`）：如果 trace
   格式变化（例如字段重命名、新必填字段），旧的 scorecard 重建路径会静默失败或产生错误数据。

2. **trace 文件被设计为持久存储**（`trace.go` 用 `O_APPEND` 追加写入）：`trace.jsonl` 是
   跨 session 知识，不是临时日志。一个长期运行的项目的 trace 文件可能横跨多个 forge 版本。

3. **`scorecard-update.mjs` 读取 trace**（`harness/scorecard-update.mjs`）：Harness 层也读取
   同一格式。如果格式在 Go 侧改了但 JS 侧不同步更新（或反过来），会产生格式漂移。

### 边界情况

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| 新 forge 版本增加 `prompt_tokens` 字段 | 旧 trace 无此字段，新 reader 读到 0 | 版本标记让 reader 知道该字段不可用 |
| 旧 forge 读取了新版写的 trace | JSON 解析可能忽略未知字段，但语义可能错 | 版本标记让 reader 拒绝未知格式 |
| scorecard rebuild 从 v1 trace 恢复 | 格式未知，可能静默乱序 | 迁移脚本转换旧格式 |
| CI 从旧 trace 文件读学习数据 | 错误归因 | 版本标记触发 N/A 降级而非乱序数据 |

### 为什么现在做

对于 v2+ 的发展，有一个不可忽视的风险：今天 forge 自己所有的学习闭环（scorecard、history tiebreak、
memory）都建立在 trace 格式的稳定性上。一旦该格式需要演进（比如 Sprint X 增加 `prompt_tokens`、
`tool_use_count` 等字段），要么**永远不破坏向后兼容**（约束未来的修改），要么**有版本标记
和迁移路径**（当前两者都没有）。

一个 `version: 1` 字段加到 `Event` 上，加上类型级别的 `SchemaVersion` 常量，加上迁移工具
`forge trace migrate --from-v1 --to-v2`，就是完整的保护。只需要 ~0.5 sprint，包括 `trace.go`、
`scorecard_wind.go`、`scorecard-update.mjs` 的适配。

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `forgotten-five-foundations.md` 方向三 | trace 查询与分析 CLI | trace 格式版本化的缺失——格式的可演化性 |
| `structural-gaps-v41-genuinely-unexplored.md` 方向一 | trace 数据结构规范化 | schema 版本与迁移路径 |
| `five-verifiable-code-level-gaps.md` 方向二 | trace 语义 gap（run ID/seq） | schema 版本化与向后兼容 |

---

## 方向五 · `forge upgrade` 只升级 harness 不升级二进制：版本分裂风险

**优先级**: 🟡 P2 | **类别**: 版本治理 · 运维 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖验证**: 在 98+ 份文档中检索 `upgrade`/`self-upgrade`/`version skew`/`binary upgrade`，
`operational-product-five-gaps.md` 方向一（Binary Lifecycle Platform）讨论了**安装/分发**的缺失，
但未聚焦于 `harness/scaffold/forge-upgrade.mjs` 与 forge 二进制之间的**版本分裂**这个具体问题。

### 问题描述

`harness/scaffold/forge-upgrade.mjs` 是一个完整且测试良好的工具：

```javascript
// forge-upgrade.mjs:1-15
// ForgeOS forge-upgrade — stamp a TARGET project with the latest governance
// assets from a SOURCE ForgeOS installation. ...
// Usage: node harness/scaffold/forge-upgrade.mjs [target root dir]
//   ForgeOS 的治理资产（.agent/ + harness/ + .arch/ + CLAUDE.md + CI）不是由
//   项目的 `npm install` 或 `go get` 管理的——它们是从 forge-init 或者 forge-upgrade
//   直接复制到项目中的。这意味着：
//     1) 治理资产在 SOURCE forge 目录中
//     2) 项目通过 copy 获得它们的本地副本
//     3) forge-upgrade 是「将 SOURCE 的最新治理推送到项目的已有脚手架」的机制
//   ★ 但它只升级治理文件，不升级 forge 二进制本身 ★
```

但 `forge-upgrade` 有一个结构性缺口：它升级的是**声明层治理资产**（agent 卡、workflow 定义、
policy 表），但**不升级执行层**（forge-core Go 二进制）。

这意味着：

```
          forge-core v2.5                   项目 A（forge-upgrade 后）
┌─────────────────────────┐      ┌──────────────────────────┐
│ forge binary (v2.5)     │      │ .agent/workflows/build.yml  ← 新版本
│                         │      │ .agent/policies/modes.yml  ← 新版本
│ 某种特定行为             │      │ harness/gate.mjs           ← 新版本
│ — 解析 YAML 的方式      │      │ ──────────────────────────
│ — 处理收敛信号的逻辑    │      │ ★ binary 还是旧版           ★
│ — 路由算法的行为        │      │ ★ 新 workflow 语法可能无法 ★
│ — 等等                  │      │ ★ 被旧 binary 正确解析     ★
└─────────────────────────┘      └──────────────────────────┘
```

这个「版本分裂」的具体风险：

```go
// main.go:14-17
var forgeVersion = "dev"       // 默认"dev"——无法区分版本
var forgeCommit = ""           // 默认为空——无法追溯版本

// loadWorkflow 的 YAML 解析路径:
// yaml2json.Decode → json.Marshal → asset.LoadWorkflowJSON
// 如果新版 workflow.yml 使用了旧 yaml2json 不支持的语法 → 静默解析失败
```

### 边界情况

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| forge-upgrade 复制新版 workflow 定义 | 旧 binary 无法解析新语法 | 应有版本兼容性检查 |
| forge-upgrade 复制新版 policies.yml | 旧 binary 用 mode.Policy 的固定映射 | 应有 version gate |
| 用户升级了 binary 但没跑 forge-upgrade | 新 binary 可能不兼容旧 YAML 文件 | `forge version --check-upgrade` 应检测 |
| 二进制和治理资产都升了但版本不一致 | 静默运行，行为可能异常 | `forge preflight` 应检测版本不匹配 |

### 为什么现在做

ForgeOS 的核心价值主张中包含「治理统一性」——它要保证被管理的所有项目都使用一致的
治理规则。但如果 forge-core 的执行引擎和治理声明层之间存在版本分裂，这个承诺就无法兑现。

当前 `forge preflight` 已经能检查环境准备情况（`cmd/forge/preflight.go`），可以自然扩展为
版本兼容性检查。`harness/scaffold/forge-upgrade.mjs` 已经能复制治理资产——只需要加一行
检查或写一个版本标记文件（`.forge/version.json`）来记录当前的治理和二进制版本。

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `operational-product-five-gaps.md` 方向一 | 二进制安装/更新/回滚平台 | governance-harness 版本分裂的精确问题 |
| `forgotten-five-foundations.md` 方向二 | 治理热加载 | 二进制与治理资产的版本对齐 |
| `forge-core/cmd/forge/main.go:14-17` | `forgeVersion = "dev"` 默认值 | 版本分裂对系统行为的实际影响 |

---

## 总结：为什么是这五个方向

| # | 方向 | 类型 | 优先级 | 核心代码证据 | 已有覆盖 | 预估 |
|---|------|------|--------|------------|---------|------|
| 1 | `forge approve` 只读不写 | 产品·交互 | P0 | `approve.go:13` "Future" 注释 | 无人机交互分析 | 0.5 sprint |
| 2 | `project.yml overrides` 无声失效 | 配置·可靠 | P0 | `main.go:projectYAMLValue` 只读 mode/lifecycle | 无 | <0.5 sprint |
| 3 | 无 `forge init/demo` 快速通道 | 产品·采纳 | P1 | `main.go:69-82` 无 init demo tutorial | 无用户路径分析 | 1 sprint |
| 4 | trace 格式无版本标记 | 数据·可演化 | P2 | `trace.go:40-98` Event 无 version/schema | 无格式版本分析 | 0.5 sprint |
| 5 | upgrade 分裂治理与二进制 | 版本·运维 | P2 | `forge-upgrade.mjs` 只升级文件不升二进制 | 无版本分裂分析 | 1 sprint |

五个方向都不涉及新的引擎/功能/架构层。它们解决的是同一个根本问题：**ForgeOS 在技术
上已经极为先进，但在「被真实团队采纳」的路上，这些薄但关键的桥梁尚未建成**。
其中方向一和方向二是 P0 级别的产品 bug——不是"nice to have"，而是已交付功能的核心
交互路径有断裂。方向三是采纳杠杆最高的产品化投资。方向四和方向五是未来版本演进
的基础设施保护——今天不做不会出事，但等出事时再补的代价会大得多。
