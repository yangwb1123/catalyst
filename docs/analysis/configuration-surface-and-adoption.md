# ForgeOS — 配置表面积、跨文件一致性与采纳体验

> **第六次扫描**，这次关注**声明式配置生态的健康度**
> —— 多达 14 个 YAML/YML 文件、47 个 Markdown 文件、4 个 JSON 文件之间的
> 交叉引用一致性、配置复杂度、新用户采纳障碍。
>
> 不写代码，只做审计。

---

## 目录

1. [配置表面积：27 个声明式文件的复杂度](#1-配置表面积27-个声明式文件的复杂度)
2. [跨文件引用：声明链的断裂点](#2-跨文件引用声明链的断裂点)
3. [代理卡模式：9 张自由格式卡片的隐式契约](#3-代理卡模式9-张自由格式卡片的隐式契约)
4. [中枢旋钮的组合爆炸](#4-中枢旋钮的组合爆炸)
5. [新用户的采纳障碍](#5-新用户的采纳障碍)

---

## 1. 配置表面积：27 个声明式文件的复杂度

### 1.1 全清单

ForgeOS 当前有 27 个声明式配置文件（不含代码、不含文档）：

| 类别 | 文件 | 行数（约） | 运行时消费 | 治理验证 |
|------|------|-----------|-----------|---------|
| **项目标识** | `.agent/project.yml` | 18 | ❌ 仅 info | ✅ check.py |
| **中枢旋钮** | `.agent/policies/modes.yml` | 140 | ✅ mode.Effective | ✅ check.py |
| **路由策略** | `.agent/routing/policy.yml` | 115 | ❌ v1 仅人工读 | ❌ |
| **路由记分卡** | `.agent/routing/scorecards.json` | 动态 | ⚠️ `forge route` 读 | ❌ |
| **记分卡 schema** | `.agent/routing/scorecard.schema.yml` | 28 | ❌ 声明式 | ❌ |
| **验收 schema** | `.agent/eval/acceptance.schema.yml` | 33 | ❌ 声明式 | ✅ check.py 部分 |
| **工作流 ×4** | `.agent/workflows/{build,design,discover,evolve}.yml` | ~300 总计 | ✅ asset.Load | ✅ check.py |
| **架构规则** | `.arch/rules.yaml` | 90 | ✅ arch-check.mjs | ✅ arch-check.mjs |
| **Harness 策略** | `harness/policies.yml` | ~30 | ⚠️ arch-check 读 | ✅ arch-check 自证 |
| **语言适配器 ×3** | `harness/adapters/{go,python,typescript}.yml` | ~70 总计 | ⚠️ adapters.mjs | ❌ |
| **代理卡 ×9** | `.agent/agents/{architect,cto,explorer,implementer,planner,product-manager,qa,researcher,reviewer}.md` | ~200 总计 | ⚠️ prompt.Build 读 `card` 参数 | ❌ |
| **架构文档** | `.agent/ARCHITECTURE.md` | ~60 | ❌ 仅文档 | ❌ |
| **工程红线** | `.agent/AGENTS.md` | ~40 | ⚠️ Gather 读 `constraints()` | ✅ check.py |
| **项目说明** | `.agent/PROJECT.md` | ~15 | ❌ 仅文档 | ❌ |
| **路线图** | `.agent/ROADMAP.md` | ~50 | ✅ converge.RoadmapCompletion | ❌ |
| **当前迭代** | `.agent/CURRENT_SPRINT.md` | ~15 | ❌ 仅文档 | ❌ |
| **决策记录** | `.agent/DECISIONS.md` | ~30 | ❌ 仅文档 | ❌ |
| **ADR ×3** | `docs/adr/000{1,2,3}-*.md` | ~100 总计 | ⚠️ prompt.Gather 读标题 | ❌ |
| **Eval README** | `.agent/eval/README.md` | ~50 | ❌ 仅文档 | ❌ |
| **技能 ×7** | `.agent/skills/*.md` | ~200 总计 | ❌ 仅文档 | ❌ |

**共 27 个文件，约 1500 行声明式配置和文档。**

### 1.2 运行时覆盖率只有 30%

从这些文件中，**运行时实际消费的只有 8 个**（约 30%）：
- `workflows/*.yml` → `asset.LoadWorkflowJSON`（经过 yaml2json.py 转换）
- `policies/modes.yml` → `mode.Effective`
- `.arch/rules.yaml` → `arch-check.mjs` 读取
- `harness/policies.yml` → `arch-check.mjs` 读取
- `harness/adapters/*.yml` → `adapters.mjs` 读取
- `.agent/routing/scorecards.json` → `forge route` 读取
- `.agent/ROADMAP.md` → `converge.RoadmapCompletion` 解析 checklist
- `.agent/AGENTS.md` → `prompt.Gather` 读取约束
- `.agent/agents/*.md` → 通过 `Build(card)` 注入 prompt

其余 19 个文件（70%）是**纯人类的声明性文档**【或】供未来使用的 schema——运行时从不读取它们。

### 1.3 配置文件的成熟度差异

| 文件 | 版本标记 | Schema | 跨文件引用 | 错误处理 |
|------|---------|--------|-----------|---------|
| modes.yml | ✅ `version: 1` | ✅ 隐式结构 | ✅ 多处引用 | ⚠️ 容错 |
| policy.yml | ✅ `version: 1` | ⚠️ 注释即 schema | ✅ 引用 scorecard.schema.yml | ❌ 未写入 |
| scorecard.schema.yml | ✅ `version` 在内容中 | ✅ 自身就是 schema | ✅ 被 policy.yml 引用 | N/A |
| acceptance.schema.yml | ❌ 无 | ✅ 自身就是 schema | ✅ 被 eval/README.md 引用 | N/A |
| build.yml | ❌ 无 | ⚠️ 隐式结构 | ✅ modes.yml#fragment | ⚠️ 容错 |
| .arch/rules.yaml | ❌ 无 | ⚠️ 隐式结构 | ✅ harness/policies.yml | ✅ 硬执法 |
| agent cards | ❌ 无 | ❌ 自由格式 | ❌ 无 | N/A |

**不一致性**：有的文件有 version 标记，有的没有。有的有 schema 声明，有的没有。
跨文件引用引用的是**文件路径 + fragment 语法**（`modes.yml#workflow_depth.reviewer`），
但没有解析器验证目标是否存在。

---

## 2. 跨文件引用：声明链的断裂点

### 2.1 引用网络图

```
modes.yml  ──┬── workflow_depth.reviewer ── 被 build.yml/evolve.yml 的 required_when 引用
             ├── gates[...] ── 被 harness/policies.yml 引用
             ├── migrations.explorer_to_engineering ── 被 migrate.go 引用
             └── workflow_depth.describe ── 被 discover.yml mode_gating 引用

policy.yml  ──┬── history.source ── 引用 scorecard.schema.yml
              ├── dimensions ── 被 route.go 的 dimWeights 硬编码
              └── tiers.by_task_type ── 被 route.go 的 taskTypeFloor 硬编码

.arch/rules.yaml  ──┬── 引用 harness/policies.yml 的 max_file_lines/max_root_files
                     └── 引用 architecture.dir_aliases（但 forge-core 不使用任何别名目录）

build.yml ── required_when: ../policies/modes.yml#workflow_depth.reviewer ── 但 runtime 不做解析
discover.yml ── authority: ../policies/modes.yml#workflow_depth.discover ── 同上
```

### 2.2 断裂分析

**断裂 1：路由策略文件与路由实现不一致**

`policy.yml` 声明了 6 个评分维度、权重、信号列表；但 `route.go` 的 `dimWeights` 是**硬编码**的。

```go
// route.go
var dimWeights = map[string]float64{
    "complexity":       0.25,
    "risk":             0.25,
    "dependency_change": 0.12,
    "security":         0.18,
    "context_size":     0.10,
    "business_impact":  0.10,
}
```

这些权重与 `policy.yml` 中的完全一致（现在）。但如果有人修改了 `policy.yml` 而没有更新 `route.go`，
两者会漂移。**没有运行时验证说"路由策略文件中的权重与代码权重必须一致"。**

`taskTypeFloor` 同理：

```go
var taskTypeFloor = map[string]string{
    "docs":             routing.Haiku,
    "crud":             routing.Haiku,
    "test":             routing.Haiku,
    "implementation":   routing.Sonnet,
    "refactor_medium":  routing.Sonnet,
    "bugfix":           routing.Sonnet,
    "architecture":     routing.Opus,
    "security":         routing.Opus,
    "payment":          routing.Opus,
    "authorization":    routing.Opus,
    "requirements":     routing.Opus,
    "reviewer":         routing.Opus,
}
```

与 `policy.yml` 的 `by_task_type` 一致——现在。但**双源真理必有漂移**。

**断裂 2：工位与工作流不一致**

`discover.yml` 声明 `mode_gating`：

```yaml
mode_gating:
  explorer:    skip
  balanced:    light
  engineering: full
  cto:         full
  authority: ../policies/modes.yml#workflow_depth.discover
```

但 `modes.yml` 的 `workflow_depth.discover` 值也是一样的。问题是：
- **谁是真的？** YAML 注释说 modes.yml 是"单一事实源"，但 discover.yml 复制了这些值
- 运行时（`mode.Effective`）只读取 `modes.yml`，从**不**读取 discover.yml 的 mode_gating
- discover.yml 的 mode_gating 是纯文档——**运行时不使用它**
- 如果某天 discover.yml 的 mode_gating 被单独修改，它会无声地与 modes.yml 漂移

**断裂 3：project.yml 与 AGENTS.md 不一致（隐式）**

`project.yml` 声明：
```yaml
mode: engineering
lifecycle: mvp
```

AGENTS.md 中的硬闸门假设 `mode: engineering` 启用了全闸门。但如果项目切换到 `balanced`
模式，某些锁门（arch、security）会从锁闭集合中消失。没有文档说明"如果你切换模式，
AGENTS.md 的前 6 条闸门中的几条会停止强制执行"。

**断裂 4：eval/README 描述的闭环未被实现**

eval/README.md 描述了一个漂亮的闭环：

```
gate/Reviewer 结果──▶ Eval聚合(model × task_type)──▶ scorecard更新──▶ Router.history择优──▶下次更准↺
```

但实际上：
- `acceptance.mjs` 的输出格式（`collect()` 返回的数组）与 scorecard 模式之间**没有正式映射**
- `scorecard-update.mjs` 需要手动从 CLI 调用（`--model`, `--task-type` 等），不是自动的
- wind-down 只在 `forge evolve` 结束时执行，不在 `forge run` 后执行
- **`forge route` 读取 scorecards.json，但只有 `--scorecard` 标志——不是默认行为**

闭环存在，但需要大量的手动胶水来连接各个环节。

### 2.3 建议：跨文件一致性快照

```
forge validate --cross-refs

检查：
  ✅ route.go 的 dimWeights == policy.yml 的 dimensions[].weight
  ✅ route.go 的 taskTypeFloor == policy.yml 的 by_task_type
  ✅ modes.yml 的 workflow_depth == discover.yml 的 mode_gating（派生自 modes.yml）
  ✅ .agent/workflows/ 中引用的 required_when 目标在 modes.yml 中存在
  ✅ .agent/workflows/ 中引用的 agent 名称等于 .agent/agents/*.md 的文件名
  ✅ eval/README.md 描述的闭环所必需的 scorecard-update 调用链完整
```

---

## 3. 代理卡模式：9 张自由格式卡片的隐式契约

### 3.1 结构

9 张代理卡（`.agent/agents/*.md`）都遵循相同的隐式格式：

```
# Agent: <name>

**Role** — ...
**Phase** — ...
**Default model** — ...
**Mode 行为** — ...

## 输入 (consumes)
## 输出 (produces)
## 硬边界 (Boundaries)
## 交接 / 停止 (handoff / stop)
```

这种一致性是人工维护的——没有 schema、没有验证、没有结构化的 front-matter。

### 3.2 问题

**a) 无结构化前端**

没有 front-matter（YAML 前端数据块）意味着：
- 无法自动提取 agent 的名称、阶段、模式行为到机器可读的结构中
- `workflow.yml` 中引用的 `agent: reviewer` 在运行时被映射到 `Build(agent, ...)` 中的
  agent 名称——但**没有验证** agent "reviewer" 是否存在对应的 `agents/reviewer.md`
- 如果代理卡被删除或重命名，没有运行时错误——只是注入空的角色卡

**b) 角色卡内容未被运行时结构化使用**

`Build(agent, phase, mode, tier, card string)` 将完整的卡文本作为字符串注入。
运行时从不解构卡的内容来提取特定的规则或边界。

这意味着：如果工程师将"不写代码/不改 bug"规则改为"可写修复性 bug"，没有运行时知道这个变化——
因为运行时从不解析卡的内容。

**c) 跨代理卡的一致性**

代理卡定义了 handoff 链：

```
planner → implementer → reviewer → qa
architect → proposal-generator → HUMAN_APPROVAL
product-manager→ (researcher) →product-designer
```

但这些 handoff 链是：
- 在卡片中记录
- 在工作流 YAML 中编码为 phase 顺序
- 在 ARCHITECTURE.md 中表达

**三重记录，没有单一事实源。** 如果某个工作流的 phase 顺序改变了（例如 reviewer 移到 qa 之后），
代理卡和 ARCHITECTURE.md 不会自动更新。

### 3.3 建议

最小的改动：在每张代理卡中加入 front-matter：

```yaml
---
id: reviewer
phase: build
default_model: opus
mode_behavior:
  engineering: strict
  balanced: focused
  explorer: light
handoff_from: [implementer]
handoff_to: [qa]
readonly: true
---
```

这可以使 check.py 验证：
- 所有被 workflow YAML 引用的代理名称都有对应的卡文件
- 所有卡文件都声明了 `readonly`，与工作流 phase 的 `readonly` 一致
- handoff 链与工作流 phase 顺序一致

---

## 4. 中枢旋钮的组合爆炸

### 4.1 组合矩阵

`modes.yml` 定义了 4 个 mode × 4 个 lifecycle = **16 种组合**。
每种组合导致：

```
mode_effective = mode (基线)
    ▸ lifecycle_modifier 覆盖（覆盖率增量、最小闸门集、执行方式下限）

结果同时驱动：
  1. Router 默认档（mode.router_default_tier）
  2. Harness 严格度（gates、coverage_threshold、enforce）
  3. Workflow 深度（discover、design、adr、reviewer、evolve）
```

### 4.2 隐式优先级规则

**规则 1**：production lifecycle 压过所有 mode：

```yaml
production:
    coverage_delta: +20
    require_min_gates: [lint, test, build, complexity, arch, security]
    enforce_floor: block
    max_file_lines: 500  # 压过 explorer 的 800
```

所以 `mode: explorer, lifecycle: production` 的结果是：
- Router 默认：`haiku`（来自 explorer）但 `coverage_delta: +20` 和 `enforce_floor: block`
  会使 harness 接近 engineering 水平
- 工作流深度：`skip discover`（来自 explorer）但 `require_min_gates: [all gates]`
  会强制执行探索阶段被跳过的所有闸门

这是个合理的组合，但不容易推导——用户需要阅读整个 modes.yml 并理解叠加规则。

**规则 2**：`enforce_floor: block` 压过 mode 的 `enforce: warn`

```yaml
# explorer mode
enforce: warn
# production lifecycle
enforce_floor: block
# 结果：block（更严格的）
```

**规则 3**：`require_min_gates` 与 `mode.gates` 做交集，取更严格的值

```yaml
# explorer mode
gates: [lint, build]
# growth lifecycle
require_min_gates: [lint, test, build, complexity]
# 结果：[lint, test, build, complexity]（growth 要求 + explorer 已包含的）
```

### 4.3 16 种组合很少被测试

当前本仓库配置为 `mode: engineering, lifecycle: mvp`——1/16 种组合。
代码中的 `mode_test.go` 测试了有限的几种组合。**所有 16 种组合的完整效果从未被验证。**

如果有人在 production lifecycle + explorer mode 下运行—一个用户可能在探索阶段已过、
但不想为全 production 配置付出 full discover 成本的情况下这样做——没有测试验证
这种组合的行为是否符合预期。

### 4.4 建议

**最小改动**：在 mode.go 的 `Effective` 函数中增加显式的"叠加后"报告：

```
forge run build --mode explorer --lifecycle production --executor dry
...
mode: explorer+production → effective gates=[lint,test,build,complexity,arch,security] enforce=block
```

这样用户（和开发者在测试中）可以亲眼看到叠加规则的作用效果。

---

## 5. 新用户的采纳障碍

### 5.1 文档森林：从哪里开始？

一个新用户克隆 ForgeOS 后，看到了约 **47 个 Markdown 文件**，分布在 5 个目录中：

```
/README.md              ← 项目介绍
/BOOTSTRAP.md           ← 强制首先阅读
/CLAUDE.md              ← 本仓库适配层
/ROADMAP.md             ← 项目路线图（与 .agent/ROADMAP.md 重复？同文件通过符号链接）

.agent/
├── PROJECT.md          ← 项目元数据 + 架构文档索引
├── ARCHITECTURE.md     ← 架构描述 + 引擎清单
├── AGENTS.md           ← 工程红线（核心规则）
├── ROADMAP.md          ← 功能路线图
├── CURRENT_SPRINT.md   ← 当前 sprint 任务
├── DECISIONS.md        ← 技术决策记录
├── agents/*.md         ← 9 张代理卡
├── skills/*.md         ← 7 个技能文档
├── workflows/*.yml     ← 4 个工作流定义
├── policies/modes.yml  ← 中枢旋钮策略
├── routing/policy.yml  ← 路由策略
├── eval/README.md      ← 评估系统描述
├── architecture/*.md   ← 未来架构文档

docs/
├── adr/*.md            ← ADR 记录
├── analysis/*.md       ← （我们刚写的分析文档）
├── ignition.md         ← onboarding 文档

forge-core/README.md   ← Go 运行时构建说明
harness/README.md      ← Harness 系统说明
examples/url-shortener/README.md + SPEC.md ← 示例项目
```

**用户需要大约阅读 15-20 个文档才能全面理解系统。** 这些文档有推荐的阅读顺序（BOOTSTRAP.md → .agent/* → 代码），但即使按照这个顺序，用户也需要吸收大量的信息。

### 5.2 没有"快速开始"教程

虽然有一个 `forge-init` 脚手架（`harness/scaffold/forge-init.mjs`），但：
- 它创建一个新的被 ForgeOS 治理的项目，不教用户如何使用 ForgeOS 本身
- 运行 `forge run build` 需要先了解 workflow 的概念
- 没有 `forge tutorial` 或互动式引导

### 5.3 技能系统的未被运行时使用

7 个技能文档（`.agent/skills/{clean-architecture,code-review,cognitive-architecture,modularization,project-reorganization,refactor-large-file,security-review,testing}.md`）
描述了代理在特定情况下应该遵循的步骤和原则。

但**运行时从不检索或注入技能文档到 prompt 中**。`Gather()` 只注入 ADR 标题、ROADMAP 体、
和 AGENTS.md 的约束。技能只是人类阅读的参考文档。

这意味着：
- 如果 agent 应该在重构大文件前查看 `refactor-large-file` 技能，运行时不会帮助它
- 技能中描述的步骤（如"先拆函数，再拆文件，gate 复绿再继续"）不会自动成为 prompt 的一部分
- 技能文档与代码行为之间存在**裂缝**：文档说应该做 X，但运行时不知道 X

### 5.4 调试体验：没有"为什么"

当一个工作流失败时，用户看到：

```
convergence: NOT MET (conjunction)
  [ ] roadmap_completion >= 100% — roadmap_completion=50%
  [ ] gates_green == true — a required gate is not green
forge run: workflow completed
exit code 1
```

这给出了什么失败，但没有**为什么**。特别是：
- RoadmapCompletion 为什么只有 50%？哪些项没有完成？
- 哪个 gate 是红色的？为什么是红色的？
- 如果我加了 gate 豁免标记，哪个 gate 被豁免了？
- 我应该怎么做？

### 5.5 建议

```
# 对采纳障碍的三个最小修复

1. forge tutorial 命令
   交互式引导用户跑完第一个 forge run build --executor dry
   解释输出并演示调试模式

2. forge debug 命令
   在当前工作流的上下文中运行，输出详细的状态分解：
   - 哪些 phase 已执行/待执行/被跳过
   - 当前 gate 状态（哪个通过了、哪个失败了、为什么）
   - 当前 memory 状态（有多少条目、最新的 5 条是什么）
   - 当前 checkpoint 状态（如果有中断的运行）

3. 技能注入
   将相关的技能文档注入 agent prompt（通过 topic 匹配，
   就像 ADR 标题的 Retrieve 一样）
```

---

## 总结：配置健康度评分

| 维度 | 评分 | 关键问题 |
|------|------|---------|
| 跨文件一致性 | ⚠️ 6/10 | policy.yml ↔ route.go 双源真理；agent 卡无 front-matter；eval 闭环未完全实现 |
| 配置复杂度 | ⚠️ 5/10 | 27 个文件、16 种 mode×lifecycle 组合、隐式叠加规则 |
| 引用可追溯性 | ❌ 4/10 | YAML `#fragment` 引用无解析器；没有 `forge validate` |
| 采纳体验 | ❌ 3/10 | 47 个文档需要阅读；没有快速开始教程；调试输出没有"为什么" |
| 技能集成 | ❌ 2/10 | 7 个技能文档完全不被运行时使用；文档与行为分离 |
| schema 一致性 | ⚠️ 6/10 | 部分有版本标记、部分有 schema、部分容错加载 |

*分析日期：2026-06-29 | 基于第六次全量扫描（配置生态 + 采纳体验视角）*
