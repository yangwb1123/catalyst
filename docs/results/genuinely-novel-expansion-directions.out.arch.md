Now I have comprehensive context on both the project architecture and the validation report. Let me produce a thorough architectural analysis.

---

# 架构分析与扩展建议

## 1. 架构评估

### 1.1 核心优势

ForgeOS 的架构呈现几个罕见的优秀特质:

**声明式控制平面与运行时彻底分离。** 这是一个被反复证明正确的架构决策。`.agent/` 目录下的 YAML 声明（agent 卡 / workflow / 路由策略 / mode 矩阵）是唯一的真相源（SoT），而 `forge-core` Go 运行时只是这些声明的执行引擎。这意味着:
- 治理规则可以独立于运行时演进而变更
- 跨仓继承（`forge-init`）只需复制声明层 + harness 执法器，不需要 Go 运行时
- 工作流编排可以在不重编译的情况下通过 YAML 重新配置

**中枢旋钮（mode × lifecycle）是架构中最高杠杆的设计。** 单一二维矩阵同时驱动三处（Router 档位 · Harness 严格度 · Workflow 深度），并在 production lifecycle 上设有不可 bypass 的安全底线。这种「一个设置控制全局行为」的设计极大地降低了操作复杂度——用户只选 mode 和 lifecycle，不需要分别配置 gate-set、coverage 阈值、路由最低档、评审深度等 8+ 个独立参数。`production` 一票否决则确保了安全不依赖于用户正确设置所有这些参数。

**带外执法（Out-of-band enforcement）是对"站在所有 CLI 之上"这一约束的诚实回应。** `harness/gate.mjs` + `arch-check.mjs` + `secret-scan.mjs` + `check.py` 作为 host-independent 的真相之源运行，不依赖 Claude Code hooks 的可用性。CC PostToolUse 加速器只是带外执法的薄前端，降级后不影响本质安全。这一层分离使得 ForgeOS 可以对任意编码 CLI 实施统一治理，而不绑定到特定宿主的能力集。

**零外部依赖与极简主义是架构债务的主动预防。** Go 核心的 18 个包全部使用标准库、`go.mod` 零 `require` 行，这从根本上排除了依赖膨胀和传递性漏洞的风险。文件 ≤ 500 行、函数 ≤ 50 行、扇入上限（7 生产 importers）等阈值在 harness 中用机器自动执法——不是纸面规范。这意味着架构不会在无人干预时悄然腐化。

### 1.2 关键局限性

**Agent 输出协议是单薄的事实标准，而非架构级契约。** 验证报告方向三和方向五精准指出了这一点。目前 `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` 都只扫描末行机器可读 token，但:

- 没有结构化输出格式（JSON / YAML / Markdown 模板），每个 agent 卡只能通过散文描述+末行 token 约定来约束输出
- 没有输出的模式验证（schema validation），`forge check` 只校验资产引用完整性，不校验 agent 输出内容结构
- 末行 token 解析失败时没有降级路径——如果 agent 输出被截断或格式扭曲，当前实现直接返回空字符串而不是部分有效结果或可追溯的错误

这本质上是一个**紧耦合接口的变体**: agent 卡通过散文约定输出格式，cli 代码通过特定行模式解析，两者之间没有显式的版本化合约。当 agent 卡更新输出约定但 cli 解析器未同步时，静默失效（事实上 Sprint 30 的 `yaml2json block-scalar 损坏` 正是这种静默失效的体现——测试也在睡觉）。

**测试质量门禁停留在二进制层次，未见架构级深度。** `computeCodeTestRatio` 只算代码行数比（`testLines / (prodLines + testLines)`），`runCountedTest` 只检查 `# tests N > 0`。没有以下任何一项:

- 断言密度（assertions per test）
- 覆盖趋势（delta coverage）
- 变异测试（mutation testing）
- Flaky 检测（flaky test detection）
- 测试时间预算（time budget per test suite）

`assert(true)` 风格的无意义测试可以通过所有现有门禁。作为运行在 AI 写代码之上的治理层，这个缺口尤其关键——因为 AI 天然倾向于产生表面积大但实际零断言的测试（如只测实例化不测行为）。

**Scorecard 的质量度量与 prompt 效能之间存在语义鸿沟。** 验证报告指出的方向四 quality_score 事实错误暴露了一个深层问题: 当前的 `QualityScore = accepted_tasks / total_tasks` 度量的是**任务级二进制裁定**，而非**提示词级行为效能**。由于 trace 中没有 `PromptHash` / `PromptVersion` / `PromptTokens`，当一个 prompt 变更导致 agent 行为恶化时:

- 无法将质量下降归因到特定的 prompt 版本
- 无法建立 prompt 版本的 A/B 比较
- 无法在部署新 prompt 前做回归验证

这是一个**可观测性缺口**，而不是一个功能缺口——功能（scorecard 存 quality_score）已经有了，但可观测性的维度不足以支持 prompt 级别的优化和调试。

**凭据管理完全缺失，没有任何架构级注入机制。** 验证报告方向一准确识别了这个 gap。当前 `childEnv` 只传播 `FORGE_AGENT_DEPTH`，没有 `SecretProvider` 或 `SecretStore` 字段，`secret-scan.mjs` 只做检测不做注入。这意味着任何需要密钥的工作流（API key 访问、数据库连接、云服务认证）都必须由用户预先设定环境变量——这与 ForgeOS 宣称的"24h 无人值守自治开发"存在矛盾。无人值守的关键前提是凭据可以安全地按需注入，而不是依赖用户在长期运行的 agent 会话开始前预配所有可能的秘密。

### 1.3 架构债务

**`cmd/forge` 包文件数预算的反复接近和突破**（Sprint 27 抬到 18，Sprint 29 回调 16，Sprint 30 实测 16 + headroom 1 = 17）是一个清晰的信号: `forge run/evolve/gate/check/accept/migrate/route` 的 CLI 命令面在不断膨胀，但 `cmd/forge` 仍是一个单包。按照单体 CLI 的增长曲线，除非持续向 `internal/` 剥离逻辑，这个边界会反复被触达。这不是严重的技术债（架构自纠文化已经证明了能自动恢复），但它说明 CLI 入口的拆分机制不是自动化的——每次都是人工意识到"快到了"才开始拆。

**`forge run` 的单趟架构与 Review 段的多相位需求之间存在根本性张力。** `forge run` 是一个从 phase 0 到 N 的线性执行，不循环。但 review.yml 有 4 个评审相位（security / distributed / performance / CTO executive），当 CTO 返回 `REDESIGN` 时，机制需要回到设计阶段的某个 phase 重新执行——当前用 `on_rejected` + 标记文件实现，但这是一个人工造的 post-hoc 机制，不是循环执行的天然属性。当评审迭代超过一轮时，这种"串行 + 标记回跳"模式会变得难以追踪状态。

**YAML 转码的 Python shim 是一个诚实的临时方案，但未纳入架构演进路径。** `forge-core` 的零依赖策略排除了 `go-yaml`，当前通过 `python3 harness/yaml2json.py` 在运行时 shell 出 JSON。ROADMAP 将其标记为"脚手架，未来可换 Go YAML 库——属 architect/cto 的依赖决策"。这意味着:
- 每次 `forge run` / `forge evolve` 都增加了一个进程 fork + Python 解释器的启动开销
- YAML 解析器与运行时之间没有类型安全边界——yaml2json 的输出是 `map[string]any`
- 如果 Python 在目标环境缺失，整套编排会崩溃（不是所有生产容器都有 Python）

这是一个**理性债务**——在明确知道未来方向的情况下选择权宜之计，但需要清晰地绑定在路线图上（如 v3 或依赖决策里程碑）。

---

## 2. 高价值架构扩展方向

### 方向 P0: Agent 输出结构化契约系统

**为什么需要:** 当前 agent 输出是"散文 + 末行 token"的非正式协议。这导致三个问题:(1) 解析器与 agent 卡之间的依赖是隐式的，一方变更但另一方不更新时静默失效；(2) 无法对输出做自动的模式校验——任何意外格式都被静默吞掉；(3) 每个新 agent 角色（product-manager / security-engineer / performance-engineer 等）都需要手写新的 `parse*` 函数。

**核心挑战:**
- Agent 卡背的自然语言多样性：不同的 agent（product-manager vs security-engineer）需要不同的输出结构，一个统一模式框架需要兼顾表达力和简洁性
- 对 LLM 的非侵入性：要求 LLM 输出严格 JSON 可能会降低响应质量或增加成本；`claude -p` 输出自然语言后提取 token 是更可靠的模式
- 向后兼容：现有 12 个 agent 卡的散文输出不能突然失效

**预期架构变更:**
1. 引入 `OutputSchema` 作为 agent 卡的新字段（YAML 定义），声明该 agent 应产生的结构化输出
2. 新增 `internal/output` 包，提供从自然语言到结构化数据的提取层（支持末行 token、JSON block、Markdown 模板三种模式，按 agent 卡声明选择）
3. 将现有的 `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` 迁移为此框架的实例
4. 在 `forge check` 中增加 schema 漂移检测——当 agent 卡的 OutputSchema 变更但解析器未更新时产生警告

**对现有系统的影响:**
- 中间：需要迁移 12 个 agent 卡，但可以逐步进行，每迁移一个淘汰一个 `parse*` 函数变体
- 正面：消除解析器 - agent 卡之间的隐式耦合，通过显式 schema 让接口成为一等公民
- 正面：新 agent 角色的添加成本从"写手写 parse 函数"降为"在 agent 卡 YAML 中声明 schema"

**多种实现路径的权衡:**

| 方案 | 优点 | 缺点 |
|------|------|------|
| **A. 声明式末行 token**（延续当前模式，但用 YAML schema 声明代替散文约定） | 最轻量，LLM 兼容最好，向后兼容 | token 表达能力有限，复杂结构不支持 |
| **B. JSON block 模式**（agent 输出自然语言后再出一个 JSON block） | 表达能力最强，可做 schema 验证 | LLM 可能输出无效 JSON，需要优雅降级，成本略高 |
| **C. 混合**（末行 token 做快速裁决，JSON block 做完整输出，声明式选择） | 灵活，各取所长 | 实现复杂，需要为每个 agent 卡指定模式选择逻辑 |

**建议:** 方案 C，但以方案 A 作为默认起步，JSON block 作为可选增强（由 agent 卡 `output_format: json` 开启）。

---

### 方向 P0: Trace 可观测性升级（Prompt 版本指纹注入）

**为什么需要:** 这是验证报告方向四暴露的核心缺口。当前 trace 有 `DurationMs` / `CostUsdMicros` / `Verdict`，但没有 `PromptHash` / `PromptVersion` / `PromptTokens`。没有 prompt 版本指纹，scorecard 的 quality/latency/cost 三维数据就无法归因到 prompt 变更——你只能知道"quality 低了"，但不知道是哪个 prompt 版本导致的。

**核心挑战:**
- Prompt 的内容可能很大（上下文、注入约束、workflow 定义），计算完整 hash 有性能开销
- Prompt 的版本化本身就是问题——prompt 是由 buildPrompt 在运行时构造的（gather constraints + ADRs + task description + gate results），没有单独的"prompt 仓库"
- 需要区分 prompt 逻辑变更（如 `buildPrompt` 代码变更）和 prompt 内容变更（如 ADR 更新导致上下文注入变化）

**预期架构变更:**
1. `prompt/cache.go` 升级：每次 `Gather` 后产出一个 `PromptFingerprint`（对拼接后的完整 prompt 做固定长度 hash，如 FNV-1a 64-bit 以控制开销）
2. `Event` 结构体新增 `PromptHash`（指纹）和 `PromptTokenCount`（token 数），由 `observeFor` 在 phase 执行起点写入
3. 新增 `prompt/registry.go`：将指纹映射到 prompt 元数据（buildPrompt 版本、context 中根文件 hash 列表），以满足"某 fingerprint 下的 prompt 是由哪版代码、哪些上下文构造的"查询需求
4. Scorecard 质量聚合加上 `by_prompt_hash` 维度——按 prompt 指纹分层的 quality/latency/cost 统计

**对现有系统的影响:**
- 低：不需要修改现有 trace 结构，只需扩展 `Event` 并确保写入点覆盖所有 phase 执行路径
- 正向：打开 prompt A/B 比较、prompt 回归检测、prompt 效能追踪等新能力
- 正向：与方向一（结构化输出契约）协同——prompt 版本 + 输出 schema 版本构成完整的「prompt → 输出」可追溯链

---

### 方向 P1: 凭据编排层

**为什么需要:** 24h 无人值守自治开发的关键前提是凭据可以按需注入。当前需要用户在启动前预设环境变量——这在单次交互中可以接受，但在长期运行的 agent 集群中不可行。凭据管理不是"功能特性"，而是**自治运行的架构前提**。

**核心挑战:**
- 安全性：凭据注入点是最高的攻击面之一，必须在注入点做强隔离（每次使用时才解密，用完立即清除）
- 宿主独立性：不能绑定到特定密钥管理服务（AWS Secrets Manager / HashiCorp Vault / 1Password），必须定义抽象接口
- 最小权限：每个 agent 只应看到它需要的凭据，而不是全部凭据
- 审计：所有凭据访问应可追溯

**预期架构变更:**
1. 定义 `SecretProvider` 接口（`Resolve(name string) (string, error)` / `ResolveAll(names []string) (map[string]string, error)` / `Close()`）
2. 内置实现：环境变量读取（当前语义的保留）、`.env` 文件读取、socket 代理读取（用于外部密钥服务）
3. 在 `CommandExecutor` 中新增 `SecretProvider` 字段，`childEnv` 扩展为从 provider 按需解析并注入到 agent 环境
4. Workflow phase 声明 `secrets: [DB_PASSWORD, API_KEY]`，runtime 在 phase 执行前解析并将这些 key 注入 agent 环境
5. 新增 `forge secret set <key> --value-literal/--from-env/--from-file` CLI 子命令

**对现有系统的影响:**
- 中低：`childEnv` 函数需要重构以接受 `SecretProvider`，但接口设计可以向后兼容（nil = 当前行为）
- 安全：SecretProvider 实现需要明确的安全审查——不能将凭据写入 trace 或日志
- 文化：这是 ForgeOS 从"编排平台"走向"自治平台"的本质性提升

**两种设计选择的权衡:**

| 决策 | 选项 A: 懒加载 | 选项 B: 预解析 |
|------|--------------|--------------|
| 做法 | phase 声明 `secrets:`，运行时在执行该 phase 前才解析 | `forge run` / `forge evolve` 启动时解析全部需要的凭据 |
| 优点 | 最小权限（只解析当前 phase 需要的），启动快 | 失败提前（启动时就能知道缺少哪些凭据） |
| 缺点 | 失败延迟（运行到第 5 个 phase 时才报缺凭据） | 可能解析不需要的凭据，启动稍慢 |
| 建议 | 默认选项 A，`forge run --validate-secrets` 触发选项 B 的预解析检查 |

---

### 方向 P1: 测试质量深度门禁（Assertion-aware 测试治理）

**为什么需要:** 验证报告方向二指出 `acceptance.mjs` 已有零测试防护（`# tests N > 0`），但 `assert(true)` 风格的测试可以全绿通过。在 AI 驱动的开发中，空转测试（tautological tests）是真实风险——AI 天然倾向于产生"不会失败"的测试来满足 gate 通过率要求。需要从架构层级确保测试的**实际验证力**，而不仅仅是"有没有跑"。

**核心挑战:**
- 语言无关性：ForgeOS 是 polyglot 平台，Go / Python / TypeScript / Rust 测试框架各有不同的断言 API 和输出格式
- 性能开销：全量解析测试文件做 AST 分析在大型仓中可能有不可忽视的开销
- 阈值合理性：断言密度的合理阈值因领域而异（API 测试 vs 解析器测试 vs UI 测试），单一阈值会产生误报

**预期架构变更:**
1. 扩展 `adapters/*.yml` 的 `assertion_detection` 配置（按语言指定断言函数/宏的模式匹配）
2. `acceptance-quality.mjs` 新增 `probeAssertions` 步骤:
   - 对测试文件做轻量模式匹配（从 adapter 配置中读取断言模式，如 Go 的 `assert.*Equal`/`require.*Nil`，TypeScript 的 `expect.*toEqual`/`assert.strictEqual`）  
   - 产出 `total_assertions` / `files_without_assertions` / `assertion_per_test` 统计
3. `forge accept` 的判定增加 assertion 维度（阈值从 `policies.yml` 读取，按 mode×lifecycle 可配置，`production` 强制 > 0）
4. `computeCodeTestRatio` 升级为非行数维度的**多重度量**: 行数比 + 测试比 + 断言密度 + 覆盖趋势（delta）

**对现有系统的影响:**
- 低：现有 gate 结构完全不变，新增的 assertion 检查是额外维度
- 中：adapter 配置需要扩展，但对已有 4 种语言（Go/TS/PY/RS）维护者是合理的负担
- 正向：可以与方向一（输出结构化契约）协同——agent 卡可以声明"我产生的测试应达到 X 断言密度"，然后被自动验证

---

### 方向 P2: 评审工作流的迭代循环引擎（Review Loop 架构原生支持）

**为什么需要:** review.yml 的 4 相位评审在当前架构中是一个串行执行，结束时一次性判断 convergence。当 CTO 返回 `REDESIGN` 时，需要通过 `on_rejected` + 标记文件机制回到设计阶段。这个机制虽然经过了彻底的实现和验证（Sprint 31 已端到端坐实），但其本质是一个"单趟执行 + 后门跳转"的变通方案，不是循环引擎的一等支持。随着评审深度的增加（security review → performance review → CTO redesign loop），这种模式变得难以追踪迭代次数和状态。

**核心挑战:**
- 反向依赖：评审结果（`VERDICT: REDESIGN`）需要驱动非评审阶段（design）的执行，跨阶段状态管理是单趟架构的自然盲区
- 迭代预算：评审循环不应该无限迭代（每次 round-trip 涉及 LLM 调用），需要明确的 max-review-loop 上限
- 状态持久化：循环状态（当前在第几轮、前一轮的评审意见摘要）需要在 checkpoint/resume 场景中正确恢复

**预期架构变更:**
1. 在 `LoopEngine` 中增加 `type PhaseLoop struct { Phase string; MaxIterations int; StopCondition *StopCondition }`——workflow 可以声明某个相位为"可循环"的，而不是只有"单次执行"
2. review.yml 的 executive-review 相位标记 `loop_on: [REDESIGN, REJECT]`——当 CTO 返回这些裁决时自动回到 review.yml 的 phase-0（security-review），而不是用外部标记文件跳转
3. `forge run review --max-review-loop 3`——设置评审循环的上限，超过上限时 `REJECTED`（而不是无限循环）
4. 现有的 `on_rejected` 标记文件机制降级为"显式 CLI 替代模式"，LoopEngine 原生循环作为默认

**对现有系统的影响:**
- 中：需要修改 `RunFrom` 以支持 phase 级的循环语义，当前线性执行假定每个 phase 只会跑一次
- 中：checkpoint/resume 需要扩展以保存循环索引
- 正向：消除人工标记文件的脆弱性
- 正向：为未来更复杂的工作流（如 iterative design → review → redesign loop）提供原生支持

**与现有 `on_rejected` 的关系:** `on_rejected` 是真实实现且已验证过了，不能简单抛弃。建议的做法是将 `on_rejected` 重构为 LoopEngine 循环的一个特例配置——所有现有标记文件仍然有效，额外提供一个更简洁的 YAML 声明方式，让新 workflow 不需要接触标记文件即可获得循环能力。

---

## 3. 接口设计建议

### 3.1 关键接口原则

**Agent 卡与 CLI 解析器之间的接口应该显式契约化，而非隐式耦合。** 当前每个 `parse*` 函数都是一份隐式协议——agent 卡散文约定输出格式，CLI 代码用行模式解析。当两边不同步时，静默失效（Sprint 27 的 `yaml2json block-scalar 损坏` 就是例子，差分测试存在不断言也是同源问题）。

**建议:** 引入 `OutputSpec` 作为 agent 卡的一等字段:
```yaml
# .agent/agents/reviewer.md 末尾（或同名的 .spec.yaml）
output_spec:
  mode: last_line_token  # 或 json_block / markdown_section
  tokens:
    - name: verdict
      pattern: "VERDICT: (APPROVE|REQUEST_CHANGES)"
      required: true
    - name: confidence
      pattern: "CONFIDENCE: (\\d+)"
      required: false
      default: 100
```

这提供了一个版本化、可校验的接口描述，CLI 的解析器可以自动生成（而非手写）。当 agent 卡更新了 token 模式但 CLI 解析器未更新时，`forge check` 可以检测到不匹配。

**评估者（evaluation/converge）的回调接口应该保持单向数据流。** 当前 `OnGateResult` / `Observe` 等回调已经遵循了良好的模式——引擎发出事件，观察者在外部接收、转换、存储。这种"引擎不关心谁在听"的设计应该保持并推广。任何新的评估维度（assertion 密度、prompt 指纹、凭据注入审计）都应作为新的 `Observer` 实现，而不是修改引擎核心。

### 3.2 需要引入的新抽象层

**`internal/output` 包**。如前所述，将目前散落在 `cost.go`（3 个 parse 函数）和 `prompt_context.go`（verdictLedger）中的输出解析逻辑提取为统一框架，支持三种模式:
- `LastLineToken`（当前方式，迁移过来）
- `JSONBlock`（新能力，agent 卡声明 `output_mode: json` 时启用）
- `Hybrid`（末行 token 做快速裁决 + 可选 JSON 做完整结构）

**`internal/secret` 包（暂定名）**。与 `SecretProvider` 接口相关的抽象:
- `Provider` 接口（`Resolve` / `ResolveAll` / `Close`）
- `EnvProvider` 实现（从环境变量读）
- `FileProvider` 实现（从 `.env` 文件读）
- `NullProvider` 实现（返回 `ErrSecretNotFound`，用于无凭据需要的安全场景）

**`internal/metrics` 包**。当前 scorecard 的质量/延迟/成本统计逻辑散落于 `scorecard_*.mjs`（多个文件）和 `scorecard.go`/`scorecard_wind.go` 之间。提取一个框架层:
- 统一的指标类型定义
- 归因维度（按 prompt 指纹 / 按 agent 角色 / 按 phase）
- 持久化抽象（当前是 JSON 文件，可扩展为 SQLite 或 Prometheus endpoint）

### 3.3 向后兼容策略

**对所有新接口采用"opt-in + falback"模式:** 新的 agent 输出 schema 以 agent 卡新增字段的形式加入，现有 agent 卡不声明则沿用旧有的末行 token 散文解析。`SecretProvider` 默认为 nil（等于当前行为）。测试质量深度门禁在 `engineering` mode 以上才启用阈值，`explorer` 和 `balanced` 走 advisory 模式。

**对所有新维度采用"先可观测后强制"的模式:** 新度量（如 assertion 密度）先加入 `forge accept` 的 `--json` 输出（可观测但不影响通过与否），积累足够数据和体验后再加入判定逻辑。这沿用了当前系统对 lint/coverage 适配器的模式（探测到→真跑→N/A 不 FAIL）。

---

## 4. 技术选型

### 4.1 当前不需要引入的

- **Go YAML 库**: 当前 Python shim 虽然不优雅，但工作可靠。在决定将 YAML 解析纳入核心契约之前引入 go-yaml 会提前构建依赖。建议推迟到"有真实性能开销数据"或"Python 缺失导致现场问题"时再做决策。
- **嵌入式数据库（SQLite/bbolt）**: 当前 JSON 文件持久化（trace.jsonl + scorecards.json）在小到中等规模下足够。如果需要大规模查询和历史趋势分析，应考虑 SQLite（纯 Go，无 CGO 亦可），但这不是当前架构瓶颈。
- **消息队列 / 事件总线**: 单机单进程架构中不需要。

### 4.2 可能需要引入的

- **`hash/fnv`**: Go 标准库已有。Prompt 指纹不需要加密货币级哈希，FNV-1a 64-bit 足够快且冲突率在可接受范围内。
- **`encoding/csv`**: 如果要从 trace 生成可导入分析工具的数据集，CSV 比 JSONL 更方便。但这是工具层需求，不是核心运行时需求。
- **`net/http`/`net/http/httptest`**: 如果未来需要 SecretProvider 的远程实现（如对接 HashiCorp Vault API），HTTP 客户端是必需的。`httptest` 用于模拟测试。但 SecretProvider 接口本身不要求 Go net/http——它可以抽象为 io.ReadWriter。

### 4.3 自建 vs 采购的决策框架

ForgeOS 的架构约束（零外部依赖、宿主独立性、可审计性）强烈倾向于**自建薄层**而非引入外部依赖。但这不意味着一切自建:

| 决策场景 | 自建理由 | 采购（引入依赖）的理由 |
|----------|---------|---------------------|
| Secret 解析 | 暴露面最小，可以精确控制审计边界 | - |
| Prompt 版本追踪 | 深度集成在 trace 事件模型中，外部工具无法理解自定义 schema | - |
| 测试断言检测 | 需要跨语言适配器框架，需要理解 ForgeOS adapter 配置格式 | 特定语言的 lint 工具（如 eslint-plugin-jest）可作为 adapter 的后端 |
| 威胁建模 | 工程红线中已经自建了轻量 STRIDE 模型，不适合引入独立工具 | - |
| 成本估算 | 使用 schema 定义的 token 单价 + PID 估算，外部估算工具不了解自定义 prompt 结构 | - |

**一个重要的例外:** 变异测试（mutation testing）的引擎可以复用已有的工具（Go 的 `go-mutesting`、TypeScript 的 `stryker`），通过 adapter 框架包装。自建变异测试引擎的价值较低——这是已经成熟的市场，而 ForgeOS 的核心价值在编排和治理层。

---

## 5. 实施路线图

### 优先级总览

| 优先级 | 方向 | 投入 | 依赖 | 独立价值 |
|--------|------|------|------|---------|
| **P0** | Agent 输出结构化契约系统 | 中（5-7 天） | 无 | 高 — 消除解析器-卡隐式耦合，新 agent 角色添加成本降低 |
| **P0** | Trace 可观测性升级（Prompt 指纹） | 小（2-3 天） | 无 | 高 — 打开 prompt 级质量归因，验证报告指出的是真实缺口 |
| **P1** | 凭据编排层 | 中（5-7 天） | 无 | 高 — 24h 自治的前提条件 |
| **P1** | 测试质量深度门禁 | 中（3-5 天） | adapter 配置扩展 | 中高 — AI 空转测试是真实风险 |
| **P2** | 评审工作流迭代循环引擎 | 大（7-14 天） | 上述 P0 的部分成果 | 中 — 已有 `on_rejected` 标记文件运行中，紧急度低但设计合理性高 |

### 阶段划分

**阶段一（P0 交付，2 周）**

核心目标:修复验证报告识别的两个最大缺口——输出协议脆弱性和 prompt 可观测性缺失。

```
第 1 周: 
  - internal/output 包: 末行 token + JSON block 框架
  - 迁移现存的 parseReviewerVerdict / parseExecutiveVerdict / parseConfidenceScore
  - 新增 OutputSpec 作为 agent 卡可选字段
  - forge check 增加 output_spec 漂移检测

第 2 周:
  - Event 扩展: PromptHash / PromptTokenCount
  - prompt/cache.go 指纹计算
  - prompt/registry.go: 指纹 → 元数据映射
  - Scorecard 增加 by_prompt_hash 归因维度
  - 向后兼容验证: 零新字段时行为不变

里程碑: "agent 卡的输出契约版本化，trace 中的每个 phase 可追溯其 prompt 版本"
```

**阶段二（P1 交付，2-3 周）**

核心目标:解决自治运行的前提条件（凭据管理）和测试可信度问题（断言密度）。

```
第 3-4 周:
  - SecretProvider 接口 + EnvProvider + FileProvider 实现
  - childEnv 重构以支持 SecretProvider
  - Workflow phase 的 secrets 声明
  - forge secret set CLI 子命令
  - 安全审计: trace / log / error 中不泄露凭据

第 4-5 周:
  - adapter 配置扩展: assertion_detection 模式
  - probesAssertions 实现（Go + TS 优先，PY + RS 延续）
  - forge accept 新增 assertion 检查维度（advisory 模式）
  - computeCodeTestRatio 多度量升级

里程碑: "agent 可以安全获取运行所需凭据，空转测试自动标记"
```

**阶段三（P2 交付，1-2 周）**

核心目标:评审工作流从"串行 + 标记跳转"升级为原生循环。

```
第 6-7 周:
  - PhaseLoop 类型 + LoopEngine 扩展
  - review.yml 的 loop_on 声明
  - forge run --max-review-loop 上限
  - 现有 on_rejected 标记文件重构为 PhaseLoop 的特例
  - checkpoint/resume 扩展以保存循环索引
  - 向后兼容验证: 未声明 loop_on 的 workflow 行为不变

里程碑: "评审循环是一等架构特性，无需标记文件变通"
```

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| P0 输出契约框架导致 agent 卡变更摩擦 | 中 | 中 | 保持 OutputSpec 可选字段，不声明则沿用旧解析；提供迁移指南 |
| Prompt 指纹的 hash 计算引入性能开销 | 低 | 低 | FNV-1a 64-bit，实测 benchmark，如 > 1μs 则改为惰性计算（只在 trace 写入时才 hash） |
| SecretProvider 接口被恶意 agent 利用泄露凭据 | 低 | 高 | 安全设计原则: SecretProvider 只暴露 `Resolve`（单值）而非 `ResolveAll`，且只在 phase 执行期间活动，结束后清空；审计日志持久化；无凭据写入 OutputSchema |
| 断言检测在跨语言环境中产生误报 | 中 | 中 | 采用 adapter 模式分语言配置断言模式，误报率可单独调整；advisory 模式下只报告不阻断 |
| 评审循环引擎与现有 `on_rejected` 实现不兼容 | 中 | 中 | 保证标记文件路径按原样运行，新 LoopEngine 只在新 workflow 或显式声明的场景生效；并行运行两个机制直到旧路径完全无人使用 |

### 诚实边界

与项目既有文化一致，我在此明确标注以下边界:

- 以上路线图假设**无需真 claude 付费运行即可验证**每个阶段的成果。所有新机制都应按既有模式设计为通过 fake-agent 端到端验证 + 单测坐实，真 claude 验证留到独立的"点火"阶段（需用户显式授权预算）。
- **不保证**这些方向在现有 `cmd/forge` 包文件数预算内可以全部实现。按照历史模式，每个方向都可能触发包拆分或 `internal/` 下沉，届时需按"先拆分再继续"纪律处理。
- P2 评审循环引擎的 checkpoint/resume 兼容性**已在分析中标注为中风险**，具体影响取决于当前 checkpoint 实现（尚未详细审查 `persist` 包）——真实影响可能在实现阶段浮现。
- **没有为自动化技术债务检测（如测试无断言）设定数值阈值。** 阈值应在每个方向的实现阶段由当前 `policies.yml` 的 mode×lifecycle 矩阵派生，而不是在架构分析中预设数值。

---

## 总结

ForgeOS 的架构在核心约束（零依赖、声明式第一、带外执法、中枢旋钮）上有着出色的纪律性和一致性。验证报告识别的 5 个方向中，有 3 个（凭据注入、输出结构验证、输出协议）证据链完整、零偏差——这是架构层面真实存在的、有文档和代码双重支持的缺口。

我给出的 5 个扩展方向中，**P0 级方向一（输出结构化契约）和方向二（Trace Prompt 指纹）** 直接回应的就是验证报告中证据最干净的 3 个方向。这两个方向解决的不是"新增一个功能"，而是**修复基础设施层的可演化性问题**——让 agent 卡和 CLI 之间的接口变得显式且版本可控，让 trace 数据可以被归因分析。

**P1 级方向三（凭据编排）** 是 24h 自治运行的架构前提——没有它，无人值守只是一个"在已配置好的环境中运行"的子集。方向四（测试深度门禁）回应的验证报告方向二的偏差——acceptance.mjs 已有零测试防护，但 AI 时代的测试质量需要在更深的维度被治理。

**P2 评审循环引擎**是这些方向中架构影响最大的（需要修改 RunFrom 的核心执行模型），但也是紧急度最低的——现有的 `on_rejected` 标记文件虽然不优雅，但已经过了真实端到端验证，并且在用户明确"就此打住"后处于可接受的终态。
