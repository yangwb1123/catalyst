现在我已掌握充分的上下文信息，可以撰写出深入的分析报告。

---

# 架构师分析：ForgeOS 五个闭包缺口扩展方向

> **分析范围**: `docs/requirements/2026-07-12-five-closure-gap-expansion-directions.md`  
> **分析基准**: forge-core v2 现状（18 Go 包，零外部依赖，中枢旋钮 mode×lifecycle 三轴完整）  
> **日期**: 2026-07-12

---

## 一、当前架构评估

### 1.1 架构优势

ForgeOS 的架构核心决策是**正确的**，且经过 31 个 sprint 的 dogfood 验证：

| 决策 | 评估 | 理由 |
|------|------|------|
| **带外执法为真相之源** | ✅ 正确 | host-independent gate 层不被任何宿主（CC/Codex/Gemini CLI）的能力边界绑架，是「站在所有 CLI 之上」承诺的唯一可行路径 |
| **中枢旋钮 mode×lifecycle** | ✅ 高度正确 | 三轴联动（Router + Harness + Workflow depth）是正交且完备的抽象，让一个设置同时驱动安全底线、严格度和深度，避免配置爆炸 |
| **零外部依赖 Go 核心** | ✅ 正确但双刃剑 | 纯 stdlib 确保了可审计性和部署单一性（18 包静态二进制），但 YAML 解析必须走 python shim 是功能性缺口 |
| **声明式 agent 卡 + workflow** | ✅ 正确 | 将 AI 行为编码为 .agent/ 中的 YAML/ Markdown 文件，而非嵌入 Go 代码，使治理资产可继承、可版本化、可跨项目复制 |
| **诚实标注（honest N/A）** | ✅ 卓越实践 | 在「有工具才查，无工具诚实 N/A」与「伪造通过」之间选择了更健康的文化，且经多轮 fresh-review 坐实 |

### 1.2 架构局限性

五个扩展方向揭示的局限性可归纳为**五个架构盲区**：

**盲区一：评估链条终止于 agent 自述。**
当前架构中，`converge` 系统的输入信号要么来自客观机械值（gate PASS/FAIL、ROADMAP 完成度），要么来自 agent 自报的 `CONFIDENCE: N`。后者是一条**单向信任链**——没有后验验证，没有偏差矫正。这不是一个 bug，这是一个**架构级别的设计取舍**：默认相信 agent，不信任但也不验证。对于生产级自治系统，这是不可持续的。

**盲区二：工作流模板是单一刚性结构。**
`workflows/*.yml` 的工作流定义只有一套模板，`asset.Workflow` 无 `Archetype` 或 `VariantOf` 字段。`forge detect` 的检测结果虽然已可推断项目类型（service / library / cli / monolith / config），但输出被**丢弃到 stdout**——不被 `forge run` 或 `forge evolve` 消费。这是架构中最明显的「半成品」：检测逻辑存在、消费侧不存在。

**盲区三：相位间契约是隐式的。**
`Phase.Emits []string` 仅记录文件名，没有任何 schema、格式、或者下游消费期望信息。跨相位数据流的兼容性完全依赖**命名约定 + 代码层硬解析**（如 `parseReviewerVerdict` 硬编码 `cto-review` 相位名）。如果重命名一个相位而不更新解析器，契约破坏是静默的——无任何架构层捕获。

**盲区四：prompt 构建对模型能力差异不敏感。**
`prompt.Build` 将 `tier` 只作为 banner 行注入，不影响 context 量、指令复杂度、或角色卡选择。这是在**经济上低效**的：Opus 收到和 Haiku 相同的任务描述，Haiku 收到和 Opus 相同的 context 量。更严重的是，全局常量 `adrTopK`、`taskCap`、`memoryCap` 不考虑 tier，限制了 Opus 的潜力、稀释了 Haiku 的信噪比。

**盲区五：阶段间交接无信息契约。**
`OnApproved.NextStage` 只有一个目标阶段名，没有 `RequiredArtifacts`、`RequiredGates`、`PassThrough` 等契约字段。从 discover 到 design、design 到 review、review 到 build、build 到 evolve 的**四道交接**，没有任何一步校验上一阶段的产出是否完整。当前的安全底线只是「人审阻断了设计→构建」，但自动化的 discover→design→review 链仍是无契约的。

### 1.3 架构债务与技术债

**已识别的技术债（来自 SPRINT 记录 + 本文分析）：**

| 债务项 | 性质 | 影响 | 建议处理时机 |
|--------|------|------|-------------|
| YAML python shim 转码 | 实现债 | 运行时多一层进程依赖，且 `yaml2json` 曾出过 block-scalar 损坏 bug | P1——方向三前置条件 |
| `workflows/*.yml` 的 `mode_gating` 注释与 Go `internal/mode` 双源 | 文档债 | 人类可读的 YAML 注释与 Go 代码镜像必须手工同步——Sprint 29 曾因不同步导致 reviewer 确认 | P2——将其形式化为唯一的声明源 |
| `Emits` 是字符串列表不是结构体 | 架构债 | 跨相位校验无法自动执行，所有契约隐式 | **方向三的核心变更对象** |
| `cmd/forge` 包文件数长期紧张 | 实现债 | 多次逼近 14~16 上限，`arch-check` 可能误报 | P0——持续关注，每个 sprint 检查 |

---

## 二、架构扩展方向

### 方向 1：评估体系的可信度增强（评估的评估）

> 对应文档方向一「置信度标定」

#### 为什么需要

`CONFIDENCE: N` 是 discover 阶段的收敛判据——N≥80 即视为需求已充分理解。但如果 agent 始终报 85% 而交付质量偏低，这个判据就是**虚假安慰**。在完全自治的运行场景（如无人值守的 `forge evolve`），这种虚假信号会导致系统在需求理解不充分的情况下过早进入设计阶段。

**业务价值**: 提高 discover 收敛的可靠性，减少因需求理解不足导致的设计反复。  
**技术价值**: 建立第一个「agent 诚实性度量」基线，为未来更广泛的 agent 行为审计创造条件。

#### 核心挑战

1. **后验配对的时序窗口**：一个 agent phase 的 confidence 需要在后续多个 gate/reviewer 结果后才能后验验证——这意味着 `memory.KindCalibration` 条目需要带延迟写入能力，而不是在 phase 结束时立即写入。
2. **偏差调整的收敛稳定性**：如果偏差调整因子在每次 `forge evolve` 迭代中都动态变化，可能导致收敛判据在迭代间抖动。需要指数滑动平均或退火策略。
3. **稀疏数据场景**：新 agent / 新 task_type 最初只有零到几个样本，校准因子不可靠。需要有「冷启动→稳态」的过渡方案。

#### 预期的架构变更

```
memory/memory.go
  + kindCalibrationEntry (new Kind)
  + CalibrationRecord struct {
      AgentRole     string
      TaskType      string
      ReportedConf  int
      ActualSuccess float64   // 0.0-1.0, 基于 gate PASS 率 + reviewer verdict
      Timestamp     time.Time
    }

converge.go
  - evalRequirementConfidence: 去掉直接读 sig.RequirementConfidence
  + evalRequirementConfidence: 查询历史校准 → 计算调整因子 → 调整当前 confidence
  + calibrationReport: 在 converge 报告中附加偏差信息
```

#### 对现有系统的影响

- **向后兼容**: 零。`evalRequirementConfidence` 的签名不变，缓存查找失败时回退到裸 confidence。
- **memory schema 变更**: 新增 `KindCalibration` 不影响已有条目的读写。
- **收敛行为变化**: 调整因子可能导致同一输入的收敛判定与当前不同——但这是**期望的改变**（更准确）。

---

### 方向 2：项目原型驱动的治理差异化

> 对应文档方向二「原型感知工作流」

#### 为什么需要

当前所有项目类型（CLI 工具 / 微服务 / 内部库 / 单体应用 / 配置仓库）都经过同一套 workflow 模板。这在经济上是无差异化的：一个内部库改一行 API 签名跑全 4 阶段三评审，一个支付微服务的 security gate 和只用作内部工具的项目强度相同。

**业务价值**: 减少低风险项目的治理开销（加速 2-3x），提高高风险项目的防护强度。  
**技术价值**: 让 `forge detect` 从「观赏性诊断」变为「驱动性配置」，完成其自然进化。

#### 核心挑战

1. **archetype 推断的准确性**：从 `go.mod` / `package.json` 元数据推断项目类型是启发式的——一个同时暴露 HTTP API 和 CLI 入口的项目是 `service` 还是 `cli`？需要有 fallback 和手动 override 机制。
2. **workflow overlay 的组合性**：如果一个 `service` 项目同时是 `mvp` 生命周期，它的 gate set 应该是 `base(service) + mvp-overlay` 还是 `base(service) × lifecycle(mvp)`？正交性设计决定了组合复杂度。
3. **现有项目迁移**：已存在的项目在 `project.yml` 中没有 `archetype` 字段，缺省值需要向后兼容（默认 `balanced` 行为，不破坏现有 workflow）。

#### 预期的架构变更

```
asset/asset.go
  type Workflow struct {
    ID          string
    Stage       string
    Archetype   string          // NEW: "service" | "library" | "cli" | "monolith" | "config" | ""
    VariantOf   string          // NEW: base workflow ID this one extends
    // ...
  }

project.yml
  + archetype: service         // 可选字段，由 forge detect 写入或用户手动设

forge detect → 扩展输出
  + 写入 .forge/project-archetype.json
  + 或在 forge run/evolve 时实时推断

workflow selection logic (新模块)
  - archetypeWorkflowLoader(base + archetype + lifecycle)
  - overlay merging (gate-set union, phase filtering, reviewer count)
```

#### 对现有系统的影响

- **向后兼容**: `archetype=""` 或缺失 → 使用当前的全量 base workflow，零行为变化。
- **`forge detect` 成为 load-bearing 命令**: 需从纯 stdout 报告改为写状态文件。
- **治理资产膨胀**: 5 个 archetype × 5 个 workflow = 25 个变体定义，需要 overlay 模式避免 5 倍复制。

---

### 方向 3：跨相位产物契约校验

> 对应文档方向三「跨相位产物契约」

#### 为什么需要

当前 `emits: [task-plan.md]` 只是一个文件名标签——没有格式声明、没有 schema、没有消费者验证。如果一个 planner phase 改变了 `task-plan.md` 的 Markdown 结构，implementer phase 会静默地错误解析。唯一的结构化输出契约是代码层硬解析 `VERDICT: APPROVE` 和 `CONFIDENCE: 85`——这些是机读契约，但它们是**写死在 Go 代码里的，而不是声明在 workflow YAML 中的**。

**业务价值**: 相位间契约破坏从「运行时静默异常」转变为「部署前可发现」。  
**技术价值**: 为方向五（阶段间交接协议）提供前置条件。

#### 核心挑战

1. **schema 定义的标准**：Markdown 文件的 schema 是什么？JSON Schema 适合 JSON/YAML，但对 Markdown 需要轻量级结构约束（必须包含 `## 标题`、必须包含 `- [ ]` 列表等）。
2. **离线 vs 在线校验**：`forge validate --emits` 是离线校验（不运行工作流），只能检查声明和文件存在性。运行时校验（feed-forward 前检查格式）需要更多注入点。
3. **已有 emits 的后向兼容**：所有 5 个 workflow 文件的 emits 都需要从 `[]string` 迁移到 `[]EmitDeclaration`。需要 codemod 工具或兼容性层。

#### 预期的架构变更

```
asset/asset.go
  type EmitDeclaration struct {
    Path     string   `json:"path"`
    Format   string   `json:"format"`     // "markdown" | "json" | "yaml"
    Schema   string   `json:"schema"`     // 可选，指向 .agent/schemas/ 文件
    Required bool     `json:"required"`   // 是否必须被下游消费
  }

  type Phase struct {
    // ...
    Emits    []EmitDeclaration   // 替换 Emits []string
  }

新命令 forge validate --emits
  遍历所有 phase → 收集 emit 声明
  逐文件检查格式是否符合声明
  检查未消费的 emit（孤岛产物）
  检查消费 missing emit（缺失依赖）

prompt_context.go
  + 注入 [context:emit-schema:task-plan.md] 到 agent prompt
```

#### 对现有系统的影响

- **YAML 变更**: 所有 `emits:` 条目需要从 `- requirement-draft.md` 扩展到：
  ```yaml
  emits:
    - path: requirement-draft.md
      format: markdown
      required: true
  ```
- **向后兼容**: 旧格式的 `- requirement-draft.md`（字符串）需要继续被解析。建议在 YAML 解析层做兼容：如果 emits 元素是字符串，自动转为 `{path: <string>, format: markdown, required: false}`。
- **schema 文件管理**: 引入 `.agent/schemas/` 目录和文件管理——这是新的治理资产类型，需要纳入 `forge-init` 和 `check.py` 校验。

---

### 方向 4：模型档位感知的 Prompt 适配

> 对应文档方向四「Tier 感知 Prompt」

#### 为什么需要

当前架构最大的低效来源之一：Haiku（低成本、低能力）和 Opus（高成本、高能力）收到相同的 prompt。这既是经济浪费（Opus 做得更多但只做 Haiku 级别的工作），也是质量风险（Haiku 被要求做只有 Opus 能可靠完成的事）。

**业务价值**: 在保证质量的前提下降低 30-50% 的 LLM 调用成本（通过让 Haiku 处理更多简单任务、Opus 聚焦高价值任务）。  
**技术价值**: 对齐「模型即资源」的思想——不同档位的模型应该有不同的能力契约和期望输出。

#### 核心挑战

1. **角色卡的 tier 分区表达能力**：当前 `.agent/agents/*.md` 是单一文本。引入 `## Haiku` / `## Opus` 分区是自然的扩展，但需要定义优先级规则（通用指令被 tier 块覆盖？还是叠加？）。
2. **tier 推断与动态切换**：`TierFor()` 在 `forge run` 启动时决定模型档位。如果运行中因为 budget 限制降级，角色卡需要即时切换——这与当前 pipeline 的静态构建矛盾。
3. **`adrTopK` 等解常量化**：从全局常量变为 tier 函数后，需要决定配置层：这些值应该在 YAML 中可配？还是硬编码在 Go 代码层？

#### 预期的架构变更

```
prompt/prompt.go
  - const adrTopK = 6
  - const taskCap = 4000
  - const memoryCap = 32
  + func adrTopKForTier(tier string) int  → {haiku:3, sonnet:6, opus:10}
  + func taskCapForTier(tier string) int   → {haiku:2000, sonnet:4000, opus:8000}
  + func memoryCapForTier(tier string) int → {haiku:16, sonnet:32, opus:48}

prompt_context.go
  + tier-aware context filtering: 根据 tier 选择注入哪些 context 块
  + tier-aware instruction style: 在 prompt 中描述当前 tier 的能力边界

agent role cards (YAML/MD)
  + 可选 tier 分区: ## Haiku / ## Opus 块
  + 或 +haiku.md / +opus.md 片段覆盖机制
```

#### 对现有系统的影响

- **向后兼容**: 角色卡无 tier 分区 → 所有 tier 收到同一通用指令，与当前行为一致。
- **行为变化**: `adrTopK` 等不再是全局常数——当 tier 变化时，注入的 context 量变化。这可能导致同一任务在不同 tier 下观察到不同上下文——这是预期的。
- **成本降低**: 假设 60% 的角色使用 Haiku、30% Sonnet、10% Opus，Tier 适应的 prompt 能让 Haiku 的失败率降低（因为任务更匹配其能力），从而减少重试次数。

---

### 方向 5：阶段间交接协议与数据护照

> 对应文档方向五「阶段交接协议」

#### 为什么需要

ForgeOS 的四道阶段边界（discover→design→review→build→evolve）是**全系统最自然的架构接缝**——但当前没有任何机器可读的契约跨越这些接缝。当一个 `forge evolve` 从 build 自动跳转到 evolve 时，evolve 阶段面对的是一个**空白的 slate**，它不知道 build 阶段完成了什么、留下了什么、gate 状态如何。

**业务价值**: 减少阶段过渡时下游 agent「从零推断」的信息损失，提高自治连续性。  
**技术价值**: 完成工作流编排的最后一个抽象缺口——执行流 + 信息流两者全覆盖。

#### 核心挑战

1. **护照格式与持久化**：`stage-passport.json` 需要在 `.forge/` 目录下持久化，且在 `forge run build` 被中断后恢复时仍然可用。这意味着 passport 的写入必须是原子操作。
2. **前向兼容性**：如果 evolve 阶段依赖 build 阶段的某个数据字段，但 build 阶段是不久前新版本的 forge-core 才加入的——旧 build 的 passport 缺失该字段，evolve 需要优雅降级。
3. **Evolve baseline 锚定**：build→evolve 的护照传递，核心是为 evolve 的第一次 scan 提供 delta baseline。但如果 build 阶段跑完后又有人手工修改了 ROADMAP，baseline 就过期了。需要「巡检」式验证而非盲目信任。

#### 预期的架构变更

```
asset/asset.go
  type StagePassport struct {
    FromStage            string
    ToStage              string
    Artifacts            []string       // 已产出的文件列表
    GateResults          map[string]string // gate 名→PASS/FAIL/N/A
    RoadmapCompleteness  float64
    RequiredForNextStage []string       // 必须在 ToStage 前存在的文件
    Timestamp            time.Time
  }

  type OnApproved struct {
    NextStage          string
    RequiredArtifacts  []StageArtifactRef  // NEW
    // ...
  }

新加载点 forge validate --stage-readiness <stage>
  - 读取 passport
  - 检查 required artifacts 是否存在
  - 检查 required gates 是否已 PASS

forge run <stage>
  - 启动时检查 stage-readiness（如果失败则 FAIL 并报告缺项）
  - 写入 passport on completion / on failure

forge evolve
  - build→evolve 时自动写入 passport
  - evolve 第一次 scan 基于 passport baseline 做 delta
```

#### 对现有系统的影响

- **向后兼容**: 无 passport → stage-readiness check skip，零行为变化。
- **passport 文件管理**: 新增 `.forge/stage-passport.json` 写入点，需要 AT 写入或临时文件交换确保原子性。
- **`forge run` 的启动流程变化**: 新增的 readiness preflight 可能被过去从不经过此检查的老工作流视为额外约束。建议在第一版中只做 WARN 而非 BLOCK，让用户适应后再切换为 BLOCK。

---

## 三、接口设计建议

### 3.1 关键模块接口设计原则

| 原则 | 说明 | 被影响的方向 |
|------|------|-------------|
| **声明式优先，命中后消费** | 所有契约（emits schema、stage passport、tier 参数）优先在 YAML 中声明，Go 代码只是机械执行声明。绝不把契约规则编码在 Go 层。 | 方向三、五 |
| **向后兼容是第一破坏线** | 任何新字段/参数缺失时，行为必须等价于当前行为。新功能的生效不能以现有项目报错为代价。 | 全部方向 |
| **fail-open to safe** | 配置缺失/解析失败时，取最保守的值（全 gate、全 reviewer、最低 tier），绝不静默降级。 | 方向二、四 |
| **诚实可见** | 新引入的校验/契约不是「暗箱生效」的——`forge validate --verbose` 应报告每项检查的通过/跳过/失败，引用具体文件和行号。 | 方向三、五 |

### 3.2 是否需要新的抽象层

是的，两个关键抽象层是必需的：

**抽象层 1：工作流变体解析器（Workflow Variant Resolver）**

```
当前: asset.LoadWorkflow("design") → 直接读 design.yml
目标: variant.LoadWorkflow("design", archetype="service") →
       1. 加载 base design.yml
       2. 加载 archetype-override design-service.yml
       3. 按合并策略融合（gate-set 取并集，phase 列表取扩展）
       4. 返回融合后的 Workflow 对象
```

这个抽象层替代了当前硬编码的 YAML 文件路径，为方向二（原型感知）和方向五（阶段间契约）提供引擎。

**抽象层 2：Tier 感知 Context 装配器（Tier-Aware Context Assembler）**

```
当前: prompt.Build(agent, phase, mode, tier, card, ctx)
目标: prompt.BuildTierAware(agent, phase, mode, tier, card, ctx, opts)
       其中 opts.TierConfig = {
         adrTopK: tier == "haiku" ? 3 : tier == "opus" ? 10 : 6,
         taskCap: tier == "haiku" ? 2000 : ...,
         memoryCap: tier == "haiku" ? 16 : ...,
         cardVariant: card + ".haiku.md" 等
       }
```

这个抽象层将当前 `prompt.Build` 的单个 flat 函数拆为可分 tier 定制的装配流水线。

### 3.3 向后兼容性策略

| 变更类型 | 策略 | 示例 |
|----------|------|------|
| 新增字段（有默认值） | 无影响 | `archetype: ""` → 等效于 base workflow |
| 新增字段（可选） | 缺失 = 跳过该功能 | 无 `schema_ref` → 跳过格式校验 |
| 类型扩展（string→struct） | 解析层兼容 | `- file.md` → 自动转为 `{path: file.md, format: markdown}` |
| 新增命令/flag | 无影响 | `forge validate --emits` 是新命令，不影响已有命令 |

需特别注意的破坏风险：
- **方向三**的 `Emits` 从 `[]string` 到 `[]EmitDeclaration` 的类型变更——如果任何外部代码依赖 `Phase.Emits` 的类型，这是破坏性变更。在 Go 中，通过保持一个兼容性 getter 或使用 `json:"emits,omitempty"` 双读可以缓解。
- **方向四**的 `adrTopK` 从全局常量变为 tier 函数——任何直接引用 `adrTopK` 的测试将改变行为。需要 monorepo 内同步修改。

---

## 四、技术选型

### 4.1 是否需要引入新技术栈

**否定——五个方向均可在当前技术栈内实现。**

| 方向 | 可能需要的新工具 | 评估 |
|------|-----------------|------|
| ① 置信度标定 | 无（纯 memory + converge 修改） | 所有变更在现有 Go 包内 |
| ② 原型感知 | 无（workflow overlay 解析器，纯 Go） | 可复用现有的 `internal/yaml2json` 基础设施 |
| ③ 产物契约 | JSON Schema 库 | 可选择自研轻量级校验（50 行）而非引入 3rd party schema 库，保持零外部依赖 |
| ④ Tier prompt | 无（纯 prompt 字符串构造变更） | 所有变更在 `prompt/` 包内 |
| ⑤ 阶段交接 | 无（纯 asset + converge + 文件 I/O） | passport 可以是 JSON，Go stdlib 足够 |

### 4.2 第三方依赖评估

当前 forge-core 的**零外部依赖**约束是战略性的，不应轻易打破。唯一可能触发引入依赖的场景是：

| 场景 | 建议 |
|------|------|
| JSON Schema 校验（方向三） | **自研**——需要的不是完整 JSON Schema 实现，只是对少数声明格式的校验。自研 50-100 行模式匹配 + 递归校验器即可，远轻于引入 `sanitize` 或 `gojsonschema` |
| YAML 解析（方向三/五持续使用 YAML） | **继续 python shim**，或在 forge-core 外部化时与 architect 讨论引入 Go YAML 库——这是已知的开放决策（DECISIONS.md D6 诚实标注） |
| 统计工具（方向一校准计算） | **Go stdlib `math` 够用**——指数滑动平均、皮尔逊相关系数、简单线性回归均可手写 |

**评估标准**：第三方依赖仅在以下全部条件满足时引入：
1. 功能无法在 ≤200 行自研代码内实现
2. 该依赖是纯 Go、零 CGO、MIT/Apache 许可证
3. 不增加 forge-core 的构建复杂度（go.sum 仅增加有限几行）
4. 有明确的 API 稳定性和维护承诺

当前五个方向均不满足条件。保持零外部依赖。

### 4.3 自建 vs 采购

ForgeOS 的哲学是**控制面自研、生态面采购/适配**：

| 层次 | 当前策略 | 本分析建议 |
|------|----------|-----------|
| 执行引擎（workflow orchestration） | 自研 forge-core | ✅ 维持不变，五个方向都是自研增量 |
| 模型路由 | 自研决策逻辑 + 采购 LiteLLM（v3） | ✅ 维持 |
| 策略执法 | 自研 PDP/PEP 模式 | ✅ 方向五的 passport + readiness check 是自研增量 |
| schema 校验 | 自研轻量 | ✅ 方向三建议自研 |
| sandbox 隔离 | 采购 Firecracker（v3） | ✅ 维持 |

---

## 五、实施路线图

### 5.1 优先级排序（修订版）

文档给出的优先级推荐基本合理，我做以下微调：

| 方向 | 文档推荐 | 我的评估 | 理由 |
|------|---------|---------|------|
| ① 置信度标定 | P1 | **P1** | 影响收敛正确性，低复杂度（~300 行），0 依赖→尽快修复信任链 |
| ③ 跨相位产物契约 | P1 | **P1** | 方向五的前置条件，`Emits` 扩展是基础设施变更 |
| ② 原型感知工作流 | P1 | **P1** | 高业务价值，中等复杂度（~600 行），但需要先与团队确认 archetype 定义 |
| ⑤ 阶段交接协议 | P2(依赖方向三) | **P1.5** | 依赖方向三，但可并行开始设计 passport 格式和 `forge validate --stage-readiness`——不依赖方向三的完整实现 |
| ④ Tier 感知 prompt | P2 | **P2** | 纯成本优化，无功能影响，复杂度中等 |

**修订后的执行顺序**：

```
Sprint N:     方向① + 方向③ 并行
                │
Sprint N+1:   方向① CI 观察 + 方向③ schema 落地 + 方向② 开始
                │
Sprint N+2:   方向② 迭代 + 方向⑤ 开始（利用方向③的基础设施）
                │
Sprint N+3:   方向⑤ 接 passport + 方向④ 开始
                │
Sprint N+4:   方向④ 迭代 + 全方向集成测试
```

### 5.2 阶段划分和里程碑

**Phase 1 — 信任基础（1-2 sprints）**

- 方向① `KindCalibration` 内存与收敛修改 → 后验验证统计 → `forge status --calibration`
- 方向③ `Emits` 类型扩展 → yaml2json 兼容解析 → `forge validate --emits` 命令
- **里程碑**: `forge validate --emits` 能发现阶段间 emits/consumes 不匹配

**Phase 2 — 差异化治理（2-3 sprints）**

- 方向② archetype 定义 → workflow overlay 解析器 → project.yml 接入 → `forge detect` 扩展
- 方向⑤ passport 格式 → `StagePassport` 结构体 → `forge validate --stage-readiness` 命令
- **里程碑**: 一个 `archetype=library` 项目跑 `forge run build` 自动使用轻量 workflow

**Phase 3 — 效率优化（2-3 sprints）**

- 方向④ Tier 感知 context 预算 → 角色卡 tier 分区 → `adrTopK`/`taskCap`/`memoryCap` 解常量化
- 全方向集成测试 + fresh-context review
- **里程碑**: Haiku/Sonnet/Opus 收到差异化 prompt，成本计费显示优化效果

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 方向①的校准因子导致 discover 收敛行为反常（eg. 本应 MET 的变为 NOT MET） | 中 | 高 | 1. 校准因子初版只报告不调整（observability-only mode） 2. 调整因子有 95% 置信区间下限，低于下限则不调整 3. 用户可通过 `--no-calibration` 退出 |
| 方向②的 archetype 推断准确性不够，误分类导致治理过度/不足 | 高 | 中 | 1. 所有 archetype 都有 fallback 到 full base 的安全路径 2. `forge detect` 的 archetype 建议可被 project.yml 的手动覆盖 3. 初版只做 gate-set 变异，不做 phase skip（降风险） |
| 方向③的 YAML emits 从 string→struct 的迁移损坏已有解析 | 低 | 高 | 1. yaml2json 的双读兼容（见「向后兼容性策略」） 2. 在 forge-core 的 Go 解析层增加 emits 类型自动降级 3. 先在一个 sprint 内跑全 CI 确认无回归 |
| 方向④的 tier 角色卡分区导致 prompt 膨胀（base + haiku + opus = 3×） | 低 | 低 | 1. 角色卡分区使用 `+haiku.md` 文件片段机制而非同一文件膨胀 2. prompt 构建时只加载当前 tier 的片段，与当前 token 预算同等 3. 实测验证 |
| 方向⑤的 passport 在中断后的一致性问题 | 中 | 高 | 1. 使用临时文件 + 原子重命名写入 2. passport 包含 `started_at` 和 `completed_at` 两个时间戳，下游可判断是否中断 3. `--resume` 路径走 passport 的 `completed_at` 做分支 |

---

## 六、总结架构决策树

五个方向中最关键的架构决策是**Emits 类型的扩展**（方向三）。它既是其他方向的前置条件（方向五的 required_artifacts 依赖 emits schema），也是最窄的接口决策——一旦类型从 `[]string` 变为 `[]EmitDeclaration`，所有 workflow YAML 文件和 Go 代码中的引用都会随之改变。**这个决策应该在 Phase 1 的第一周完成并锁定。**

第二关键的决策是**方向二的 archetype 合并策略**——gate-set 是 union（双方都要）还是 max（取更严）？我的建议是 **union**（叠加不取代），因为 lifecycle 的 `production` 覆盖应该始终比 archetype 的默认更严格，取 union 自然实现这一点。

第三关键的决策是**方向四的角色卡分区形式**——是在单文件内用 `## Haiku` 分区，还是使用外部片段文件 `+haiku.md`？我建议**外部片段文件**，原因：(1) 保持 base 角色卡干净；(2) 允许 tier 片段独立于基础卡进行版本演进；(3) 未来引入新 tier（如 4o-mini/claude-3.5-haiku）只需加新片段，不动 base。

---

以上分析覆盖了全部五个方向的架构评估、扩展建议、接口设计、技术选型和实施路线图。所有建议均保持 forge-core 零外部依赖承诺，且不引入新的框架或运行时依赖。
