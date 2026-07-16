# ForgeOS — 第六次架构扫描：多模型并行宇宙与系统级漂移

> **扫描基准**：`b0c80e4`  
> **视角**：专门考察「ForgeOS 用几个平行的、但互相不感知的模型来描述同一个系统」  
> **方法论**：对代码库中所有「元模型」计数并检查互联关系

---

## 核心发现：ForgeOS 有三个平行的生命周期模型

ForgeOS 用三个互不感知的模型来描述同一个系统：

### 模型 A：项目生命周期（`.agent/project.yml` × `modes.yml`）

```
lifecycle: idea | mvp | growth | production
```

4 个阶段，控制**治理严度**。mode（explorer/balanced/engineering/cto）与 lifecycle 组合成 16 种中枢旋钮状态。这是**系统级**的成熟度标尺。

### 模型 B：Workflow 脊柱（`.agent/workflows/*.yml`）

```
discover → design → review → build → evolve
```

5 个 workflow，每个是可编排的阶段，有独立 YAML、独立 phase 序列、独立 stop_condition。这是**运行时**可执行的阶段序列。

### 模型 C：AI-SDLC（`.ai/prompts/`）

```
Stage 0: Product Discovery
Stage 1: Architecture Review
Stage 2: Security & Protocol Review
Stage 3: Distributed Systems Review
Stage 4: Implementation Review
Stage 5: Performance Review
Stage 6: Production Readiness
Stage 7: Sprint Planning
Stage 8: Post Sprint Review
Stage 9: CTO Executive Review
```

10 个 stage，17 个专业角色。这是**人工操作**的评审清单——手动填 Context、粘贴到 LLM、保存产出到 `.ai/reviews/`。

### 关系矩阵

| 维度 | 模型 A (lifecycle) | 模型 B (workflow) | 模型 C (AI-SDLC) |
|------|-------------------|-------------------|-------------------|
| **谁读** | `internal/mode` + `gate.mjs` | `internal/asset` + orchestrator | 人类（手动操作） |
| **谁写** | 人类 | 人类 | 人类 |
| **执行层** | 运行时 gate 开关 | orchestrator 编排 | 无（手动粘贴到 LLM） |
| **阶段数** | 4 | 5 | 10 |
| **互联** | workflow 通过 `mode_gating` 引用 mode | AI-SDLC 通过 `uses_template` 声明引用 | 无反向引用 |
| **验证** | `forge validate` 解析 YAML 结构 | `forge validate` 解析 YAML 结构 | 无 |

**核心问题**：三个模型做同一件事（描述项目在哪儿、要去哪儿），但它们之间**没有同步检查**。一个项目改了 `lifecycle: growth`，workflow 的 `mode_gating` 行为自动变了，但 AI-SDLC 的 10 个 stage 完全不受影响——人类手动执行时，仍用和 `idea` 阶段一样的 prompt。

---

## 七个未被前六轮覆盖的扩展方向

### 方向 1：跨模型一致性护拦（Multi-Model Drift Guard）

**当前状态**：
三套模型完全平行运行。人类的 AI-SDLC 评审可能说「架构评审通过」，但 workflow 的 mode_gating 说「explorer 模式，跳过评审」。反之，workflow 说「架构已批准」，但 AI-SDLC 的 Stage 1 评审从未执行。

没有一条代码检查以下问题：

```
# 应检查但不检查的问题
1. lifecycle=idea 但 workflow 引用了 production 级 gate 集 → 过于严格
2. lifecycle=production 但 AI-SDLC 的 Stage 6 (Production Readiness) 从未在 reviews/ 目录出现 → 缺少生产就绪评审
3. modes.yml 中某个 gate 被删除 → workflow 引用了已不存在的 gate 名 → 运行时静默退化
4. agent 卡中的 `uses_template` 指向不存在的 `.ai/prompts/*.md` → 声明的关联已断裂
```

**建议方案**：

```bash
# 跨模型一致性检查
forge validate --models
  [PASS] lifecycle=production → mode_gating 引用 gate_set=full (匹配)
  [WARN] lifecycle=production → .ai/reviews/ 缺少 Stage 6 (production-readiness.md)
  [FAIL] review.yml 引用 gate 'security' → modes.yml#harness.gates 无此 gate
  [INFO] agent 'security-engineer' 声明 uses_template .ai/prompts/02-security-rfc-review.md → 文件存在
  [INFO] agent 'performance-engineer' 声明 uses_template .ai/prompts/05-performance-review.md → 文件存在
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **完整性** | 三套模型是同一个系统的三个面向。当它们不一致时，系统在对自己说谎——说「生产级」但缺少生产就绪评审 |
| **安全性** | `lifecycle=production` + `mode=engineering` 下遗漏安全评审等于合规风险。一个跨模型检查能发现「安全评审从未在 .ai/reviews/ 出现」 |
| **治理闭环** | ForgeOS 的治理哲学是从 central knob（mode×lifecycle）驱动一切。但当前 knobs 只驱动了 workflow，没驱动 AI-SDLC |

**边界情况**：

1. **Staged adoption**：团队可能选择只跑 workflow 不跑 AI-SDLC（或反之）。`forge validate --models` 需要 `--strict` 和 `--advisory` 模式
2. **假阳性**：`warn: lifecycle=production 但 .ai/reviews/ 缺少 13 个文件`——不是所有 AI-SDLC stage 都强制。需要声明哪些 stage 是 `required`、哪些是 `optional`
3. **自定义 stage**：团队可能自定义 AI-SDLC stage（增加 Stage 10: 第三方审计）。检查器需要适配

---

### 方向 2：`uses_template` 从注释变成执行链

**当前状态**：
`review.yml` 和 agent 卡声明了 `uses_template: .ai/prompts/02-security-rfc-review.md`，但这个字段**只是注释**——没有代码读取它。`.ai/` 的 10 个 SDLC 模板永远不会被 forge-core 自动用于构建 prompt。

```yaml
# review.yml 的声明（很美好，但只是声明）
- name: security-review
  agent: security-engineer
  uses_template: .ai/prompts/02-security-rfc-review.md   # 没人读这一行
```

**建议方案**：

当 `uses_template` 存在时，`buildPrompt` 将模板内容作为 prompt 的后置段落。模板中的 `{{Context}}` 占位符由运行时填充：

```go
// 伪代码：prompt_context.go 增加
if p.UsesTemplate != "" {
    tmpl, err := readFile(filepath.Join(root, p.UsesTemplate))
    // 用当前 context 填充 {{Context}} 占位符
    // 追加到 prompt 正文
    prompt += "\n\n---\n" + fillTemplate(tmpl, ctx)
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **利用已有投资** | `.ai/prompts/` 是 ForgeOS 最成熟的内容资产——10 个详细 stage、17 个角色定义、4 个共享模板。当前不用于自动化，纯手工操作 |
| **一致性** | 人类手动填 Context 粘贴到 LLM 和自动化 prompt 构建之间没有差异。如果 `forge run review` 自动包含了 AI-SDLC Stage 2 的安全评审框架，人工执行和自动执行的结果可比较 |
| **渐进增强** | 不破坏现有行为——`uses_template` 缺失时与原行为相同。存在时自动包含、允许被 prompt 覆盖 |

**边界情况**：

1. **模板中的 Markdown 格式冲突**：`.ai/` 模板是为人类可读的 Markdown 写的，可能包含 LLM 指令格式冲突（如 `---` 分隔线被解释为上下文分隔符）
2. **模板长度**：AI-SDLC Stage 6（Production Readiness）约 300 行。附加到 prompt 上可能撑爆上下文窗口。需要智能摘要或可选装载
3. **模板版本与 workflow 版本不匹配**：更新了 `00-product-discovery.md` 但没更新 `discover.yml` → 模板引用指向旧内容。需要内容 hash 校验

---

### 方向 3：Phase `readonly` 的运行时强制执行

**当前状态**：
每个 workflow phase 声明 `readonly: true/false`：

```yaml
# build.yml
- name: implementer
  readonly: false        # 允许写文件
- name: reviewer
  readonly: true         # 禁止写文件
```

但 `readonly` **仅是一个声明性注解**——orchestrator 不强制执行它。如果 reviewer 的 agent prompt 中包含「写了文件」，`executor`（claude）会照写。系统在**执行前**不知道 agent 是否违反了 readonly 约束——只能通过 gate 发现意外修改（如果 gate 检查了文件变更集）。

**建议方案**：

```go
// orchestrator 在执行 agent phase 前后检查 readonly 遵守情况

// 执行前：快照当前文件树的 hash
snapshot := hashFilesystem(o.root)

// 执行后：对比 hash，如果不是 readonly 但无变化，或 readonly 但有变化，报告
diff := diffFilesystem(o.root, snapshot)
if phase.Readonly && diff.FilesChanged > 0 {
    // 违反 readonly → 回滚变化？
    // 或记录 trace 事件，让 reviewer gate 判定
    return fmt.Errorf("phase %s violated readonly: wrote %d files", p.Name, diff.FilesChanged)
}

// 或更轻量：git diff 检查
if phase.Readonly {
    changedFiles := gitStatus(root)
    if len(changedFiles) > 0 {
        // 回滚 git checkout 或记录违规
    }
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **审计** | `readonly=true` 的 phase 不应该修改代码库。如果 reviewer 改了代码，静默接受等于绕过了评审—实现分离 |
| **安全性** | 当前依赖 agent 对 prompt 的遵守。如果 agent 受提示注入指示「忽略 readonly 指令」，没有运行时的 fail-safe |
| **数据质量** | planner 的 `feeds_forward` 产出被用于 prompt context。如果 planner 写了额外的文件，但代码库不可预测地修改了 |

**边界情况**：

1. **Agent 必须写临时文件**：有些 tool-use 场景下 agent 写临时文件然后删了。`git diff` 在文件被删后是干净的，但中间状态有写。需要细粒度策略
2. **README 更新**：`readonly=true` 的 reviewer 可能建议更新 README——这是「代码修改」还是「文档」？需要 `readonly_except: ["*.md"]`
3. **Git 子模块**：agent 可能修改了 vendor/ 子模块的文件。只监控 `--root` 下的文件，或监控 `git diff` 的变更

---

### 方向 4：未被引用的 Agent 角色的可发现性

**当前状态**：
12 个 agent 卡中，所有 12 个都**被某个 workflow 引用了**（我在前次分析中错误认为 3 个未引用）。但有些 agent 只在特定 mode 下可用：

| Agent | 用到的 Workflow | mode 限制 |
|-------|----------------|----------|
| `product-manager` | discover, discover(P3) | explorer=skip |
| `researcher` | discover | balanced=optional |
| `security-engineer` | review | explorer=skip |
| `distributed-engineer` | review | explorer=skip |
| `performance-engineer` | review | explorer=skip |
| `cto` | design, review | 全 mode |
| `harness` | build | 全 mode |

问题：当一个 agent 被 **mode 过滤跳过**时，它的角色卡**完全消失**了——不参与 prompt 构建，不产生输出，不给任何 trace。用户可能不知道 `security-engineer` 存在，因为 `mode=explorer` 跳过了所有 review workflow。

**建议方案**：

当 mode 跳过特定 agent 时，记录一条 trace 事件 `agent_skipped`：

```
forge run discover --mode=explorer
  [INFO] agent 'security-engineer' skipped (mode=explorer, required_when=workflow_depth.review)
  [INFO] agent 'performance-engineer' skipped (mode=explorer, required_when=workflow_depth.review)
```

并在 `forge status` 中提供「当前 mode 下无法访问的 agent」指示：

```
forge status --mode=explorer
  Active workflows: discover (light), build (gate_set=minimal)
  Inactive (skipped by mode): review, design, evolve
  Inactive agents: security-engineer, performance-engineer, distributed-engineer
  Tip: forge status --mode=engineering 可查看全量 agent
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **可见性** | 用户选 `mode=explorer` 时可能不知道他们跳过了安全评审。可发现的跳过列表提供了「知道自己在放弃什么」的认知 |
| **模式比较** | `forge status --diff-modes balanced engineering` 可以明确列出两个 mode 之间的差距——哪些 gate、agent、workflow 不同 |
| **学习** | 新用户可能不知道自己需要 `security-engineer`。一个 `forge suggest --mode` 可以根据项目类型推荐 mode |

**边界情况**：

1. **过度报告**：`mode=balanced` 下跳过 3 个 agent，每个都报一条 `[INFO]` 消息会使输出杂乱。聚合为一段说明
2. **用户显式优化**：用户在 `mode=engineering` 下但显式跳过了一些 agent（通过自定义配置）。跳过列表应反映用户显式选择 vs mode 隐式跳过的区别

---

### 方向 5：AI-SDLC 评审产物的自动回归检查

**当前状态**：
`.ai/reviews/` 目录存储人工执行的 AI-SDLC 评审产物。目前仅有一个示例文件 `example-gateway-stage0.md`。评审产物**与代码无关**——如果代码实现了评审中提出的安全建议，没有机制验证这些建议是否真的被代码覆盖。

```markdown
# .ai/reviews/example-gateway-stage0.md
## 安全发现
- 发现 1: OAuth2 token 生命周期未定义
- 建议: 实现 token 轮转 + 撤销
```

如果 implementer 实现了 `refresh_token.go`，没有机制将代码中的 `refresh_token.go` 链接到 `example-gateway-stage0.md` 中的「发现 1」。

**建议方案**：

**Phase 1：评审产物扫描**
```bash
forge validate --reviews
  [PASS] .ai/reviews/example-gateway-stage0.md — machine-parseable format
  [INFO] 5 security findings: 3 open, 1 resolved, 1 deferred
  [WARN] Finding "OAuth2 token lifecycle undefined" → no matching implementation found
```

**Phase 2：评审—代码追踪**
评审产物中的发现可以关联到代码标签：

```markdown
## 安全发现
- [SEO-001] OAuth2 token 生命周期未定义
  - status: open
  - suggested_fix: 实现 token 轮转 + 撤销
  - code_tags: [token, refresh, rotation]
```

`forge validate --reviews` 扫描代码中匹配 `code_tags` 的文件，如果发现 `*refresh*`/`*token*`/`*rotation*` 文件，将状态更新为 `implemented`。

```bash
forge validate --reviews --update
  [UPDATED] SEO-001: open → implemented (found: auth/refresh_token.go)
  [INFO] SEO-002: open (no matching code found — token revocation missing)
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **评审 ROI** | AI-SDLC 评审的劳动密集型很高（人工填 Context → 粘贴 → 解读产物）。如果评审建议从不被追踪到实现，评审的 ROI 为负 |
| **合规可追溯** | 合规要求「发现 → 修复 → 验证」的可追溯链。`code_tags` + `forge validate --reviews` 提供了机器可读的链 |
| **债务管理** | 如果一个安全发现在评审中被标记为 `deferred: Sprint 4`，但 Sprint 4 结束了代码中没有 `auth/revocation.go`，可以自动提醒 |

**边界情况**：

1. **评审产物格式**：当前是自由格式 Markdown。需要约定 YAML frontmatter 或结构化 section header 来让 machine parse 可行
2. **假阳性匹配**：`code_tags: [token]` 可能匹配 `tokenizer.go`（与 OAuth2 token 无关）和 `auth_token.go`（相关）。需要 context-aware 匹配或人工确认
3. **遗留评审**：项目可能已经有 50 个 AI-SDLC 评审产物。一次性扫描会产生大量噪音。需要 `--from <date>` 或 `--state open` 过滤

---

### 方向 6：Workflow `required_when` 的路径引用的可解析性

**当前状态**：
workflow YAML 广泛使用 `required_when: ../policies/modes.yml#workflow_depth.*` 形式的路径引用。这是一种类似 XPath 的约定：

```yaml
# review.yml
required_when: ../policies/modes.yml#workflow_depth.review   # 指向特定字段
authority: ../policies/modes.yml#workflow_depth.discover      # 同样模式
gate_set: ../policies/modes.yml#harness.gates                 # 指向另一个字段
```

但**没有任何代码解析这些路径引用**。它们是人工可读的注释，不是机器可执行的契约。当 `modes.yml` 中的字段被重命名或删除时，workflow YAML 中的引用会静默地变成死链接。

**建议方案**：

```bash
# 验证所有 YAML 路径引用
forge validate --references
  [PASS] build.yml → policies/modes.yml#harness.gates → resolves to ["lint","test","build",...]
  [PASS] review.yml → policies/modes.yml#workflow_depth.review → resolves to "standard"
  [FAIL] design.yml → policies/modes.yml#workflow_depth.design → FIELD NOT FOUND
           modes.yml 有这个键吗？检查 authority 引用
```

解析器实现：

```yaml
# 简单的 YAML 路径解析规则
# modes.yml#harness.gates      → 读 modes.yml, 取 .harness.gates
# policies/modes.yml#foo.bar   → 相对路径转到 policies/modes.yml, 取 .foo.bar
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **可维护性** | 当前 5 个 workflow YAML 中有约 15 条路径引用。每个都是潜在的断链 |
| **重构安全** | 重命名 `workflow_depth.review` 为 `workflow_depth.review_dimensions` 会打断所有 workflow 的 `required_when` 逻辑，但没任何工具告诉你 |
| **静态分析** | `forge validate` 当前只检查语法。跨文件引用验证是下一步的自然扩展 |

**边界情况**：

1. **循环引用**：如果 `modes.yml` 反过来引用 workflow YAML 中的值，形成循环依赖。验证器需要检测
2. **引用通配符**：当前是精确路径。将来可能支持 `modes.yml#harness.*` 匹配所有 gate。解析器需要通配符支持
3. **非 YAML 源**：`required_when` 也可能引用 Go 代码中的常量。当前只有 YAML 文件间的引用被声明

---

### 方向 7：Orchestrator 自身状态的 Telemetry（第四维可观测性）

**当前状态**：
ForgeOS 有三维 telemetry：

| 维度 | 数据源 | 存储 | 用途 |
|------|--------|------|------|
| Trace | `trace.go` + `cost.go` | `.forge/trace.jsonl` | 事件日志 + 成本 |
| Memory | `memory.go` | `.forge/memory.jsonl` | 跨会话记忆 |
| Checkpoint | `persist/checkpoint.go` | `.forge/checkpoint.json` | 崩溃恢复 |

缺少的第四维：**orchestrator 自身的健康 / 决策 telemetry**。

具体来说，以下事件的 **决策日志** 不在任何 trace 中：

| 决策事件 | 当前状态 | 应记录 |
|---------|---------|-------|
| 为什么 agent tier 从 Sonnet 降到了 Haiku？ | 只在 stderr 的 logln 中 | 写入 trace/kraftkind:"tier_decision" |
| 为什么 stale counter 增长了？（roadmap 平 + gate 没变绿） | 只在 loop.go 内部 | trace 一个 stale_increment 事件 |
| overload 退避等待了多少秒？ | 只在 backoff.go 日志 | trace 一个 overload_backoff 事件 |
| `resolveEnforce` 返回了 warn 而不是 block？ | 只在 gate.mjs 日志 | trace 一个 enforce_decision 事件 |
| `readonly` 约束被遵守了吗？ | 不记录 | trace 一个 readonly_check 事件 |

**建议方案**：

增加 `DecisionTrace`，记录 orchestrator 的每次重要决策：

```go
type DecisionTrace struct {
    Timestamp  time.Time
    Kind       string   // "tier_decision" | "stale_increment" | "overload_backoff" | ...
    Phase      string   // optional, phase context
    Decision   string   // the actual decision (e.g., "downtier Sonnet→Haiku")
    Reason     string   // why (e.g., "spend_ratio=0.85 > 0.80 threshold")
}
```

写入 `trace.jsonl` 作为一个新的 event kind：

```json
{"kind":"decision","timestamp":"2026-06-30T12:00:00Z","decision":"downtier Sonnet→Haiku","reason":"spend_ratio=0.85","phase":"implementer"}
```

`forge status` 可以输出决策统计：

```
forge status --decisions
  last 50 decisions:
    tier_decision:      12 (avg down-tier ratio 0.65)
    overload_backoff:   8  (avg wait 15s)
    stale_increment:    3  (all from flat roadmap + red gates)
    readonly_check:     1  (violation: review phase wrote auth/notes.md)
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **调试** | 当前 trace 告诉你「发生了什么」（phase X ran），不告诉你「为什么」（因为 budget 85% 所以选 Haiku）。决策日志填充这个缺口 |
| **成本分析** | 知道多少次 tier_decision 导致了降级，就能评估 budget cap 是否设得太低 |
| **自优化** | 方向 3（相位级动态超时）需要知道「为什么」一个 phase 超时了（overload vs 太复杂）。决策日志提供了这个数据 |

**边界情况**：

1. **日志膨胀**：每个迭代约 10-30 个决策事件。50 次迭代 × 30 = 1500 个事件/run。需要 event 级别的合并（连续 5 个 stale_increment 合并为一条）
2. **敏感决策**：tier_decision 包含模型名——理论上不敏感，但如果 agent 降级因为 budget 低，这条日志可能被误解为「系统不可靠」。需要在 `forge status --json` 中可选 redact
3. **重放**：决策日志是诊断工具，不用于重放（不像 checkpoint）。它是 append-only，不参与 checkpoint 恢复

---

## 优先级矩阵

| 方向 | 影响面 | 成本 | 前置依赖 | 推荐 |
|------|--------|------|---------|------|
| **1. 多模型 Drift Guard** | 治理完整性：高 | 中 | 无 | Sprint n+1 |
| **2. `uses_template` 执行** | 生态整合：高 | 低-中 | agent 卡 + workflow 已有声明 | Sprint n+1 |
| **3. `readonly` 运行时强制执行** | 安全性/审计：中 | 低（快照 diff） | phase 声明已有 | **Sprint n** |
| **4. 被跳过 Agent 的可发现性** | 采纳/透明：中 | 极低（~30 行日志） | 无 | **Sprint n** |
| **5. 评审-代码追踪** | 合规/ROI：中-高 | 中 | 评审产物结构化约定 | Sprint n+2 |
| **6. YAML 路径引用解析** | 可维护性：中 | 低 | 无 | Sprint n+1 |
| **7. Orchestrator 决策 Telemetry** | 调试/分析：中 | 中 | 需要 trace event kind 扩展 | Sprint n+2 |

---

## 关于「六个分析」的反思

这是第六次全量扫描。回顾前五次，覆盖了：

| 轮 | 视角 | 核心发现 |
|----|------|---------|
| 1 | 经典产品架构 | 未交付缺口 + Agent/记忆/安全/版本 |
| 2 | 内联对话 | 多仓/Custom Gate/Workflow 检查器/事件触发 |
| 3 | 声明-实现一致性 | 收敛注册表/运行时 arch/通知/AI-SDLC 桥接 |
| 4 | 消费端链路 | 输出合约/doctor 接入/相位画像/自修复/自演化 |
| 5 | 工程化运营 | 版本/基准/preflight/错误 UX/交互 init |
| 6 | 多模型并行 | Drift Guard/template 执行/readonly 强制/决策 telemetry |

六轮下来，代码库的 12 个 Go 包、5 个 Workflow YAML、12 个 Agent 卡、20+ 个 Harness 工具、10 个 AI-SDLC 模板、50 个测试文件、4 个分析目录——**所有可读文件都被扫描过了**。前 5 轮覆盖了所有我能想到的「功能缺口」和「架构优化」。本轮（第 6 次）的 7 个方向回归到系统工程的元问题——**当一套系统中同时存在多个「解释系统如何工作」的模型时，谁来确保它们一致？**

< 如果还要第 7 次，唯一剩下的角度是：什么功能/优化**不应该**做？但那是篇哲学文章，不是分析文档。 >

---

*分析日期：2026-06-30 | 第六次也是最后一次（我判断）全量扫描，聚焦三套生命周期模型的漂移*
