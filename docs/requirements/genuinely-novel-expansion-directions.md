# ForgeOS: 真正新颖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深度扫描（forge-core 18 Go 包 / 77 测试文件 / harness 39 模块 /
> 全部 `.agent/{WORKFLOWS,AGENTS,SKILLS,PROJECT,ARCHITECTURE}` 声明 / Sprint 1-31 完整演进记录），
> 交叉核对 **40+ 篇已有 `docs/analysis/*.md` + 8 篇已有 `docs/requirements/*.md`** 以确保真正的新颖性。
> **核心判断**: 已有分析已覆盖「引擎补齐」「生产可靠性」「架构演化」「二阶伴生问题」「第三地平线生态」等维度。
> 本文瞄准的是其中 **零覆盖或仅边缘触及** 的 5 个方向。
> **纪律**: 不编写任何代码。每条方向附带代码级证据 + 与已有分析的差异化证明。
> **日期**: 2026-07-09

---

## 前言：已有分析覆盖全景与本文定位

ForgeOS `docs/` 下已有 **40+ 篇分析文档 + 8 篇 requirements 合成分析**。它们覆盖了以下大维度：

| 维度 | 代表文档 | 覆盖方向数 |
|------|----------|-----------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断） | `high-value-expansion-directions.md` | 5 |
| 生产就绪可靠性（prompt QA / 信号硬化 / 环境验证） | `expansion-production-readiness.md` | 5 |
| 第三地平线生态（管线组合/多仓库联邦/事件驱动/资产升级/修正学习） | `expansion-horizon-three.md` | 5 |
| 执行语义形式化（原子性/幂等性/因果一致性/坐标系/版本演化） | `execution-semantic-gaps.md` | 5 |
| 系统二阶伴生问题（知识衰减/配置爆炸/自洽性/追踪/测试覆盖） | `second-order-architectural-gaps.md` | 5 |
| 架构盲区与多波分析 | `architectural-expansion-perspectives.md` + 40+ 篇 `docs/analysis/*.md` | 30+ |
| **总计已有覆盖** | | **~55 个方向** |

**本文不重复上述任何一个方向。** 下面 5 个方向在全部 55+ 已有方向中**零覆盖或仅边缘触及**，且均满足：
- 有可验证的代码级证据
- 在当前架构下可实现（无外部阻断依赖）
- 与已落地功能正交互补
- 解决的是「系统存在但未被识别」的缺口，而非「已知但推迟」的特性

---

## 方向一：Agent 凭据注入与秘密生命周期管理（Secure Credential Delivery）

### 现状

ForgeOS 有 `secret-scan`（`harness/secret-scan.mjs`），能**检测**代码中硬编码的秘密：

```javascript
// harness/secret-scan.mjs:68,75-76
// 按 basename 匹配 .env / .npmrc / id_rsa + provider-anchored 模式
var secretNeedles = ["secret", "credential", "vault", ".key", ".pem"]
```

`forge-core/internal/risk/risk_diff.go:47` 也有对称的启发式扫描。

**但这两个机制都只做「检测 - 报警」，不做「注入 - 使用」。**

当 agent（implementer / planner / reviewer）需要访问：

- 一个部署目标（Kubernetes API、AWS、Cloudflare）
- 一个外部服务（GitHub、数据库、消息队列）
- 一个包注册表（npm / PyPI / Docker registry）

……它没有任何安全的方式获取凭据。目前的选择只有：

1. **硬编码在代码中** → 被 `secret-scan` 拦截（正确），但然后呢？agent 卡住了
2. **通过环境变量传入** → `CommandExecutor` 有 `childEnv`，但当前只传播 `FORGE_AGENT_DEPTH` 系统变量，没有任何用户级凭据传入机制
3. **人工手动设置** → 破坏了自治循环。24h 无人值守的设计目标要求 agent 能自主完成从编码到部署的全流程

### 代码级证据

```go
// forge-core/internal/orchestrator/command_executor.go:36
// "needs that CLI plus credentials in the environment."
// ——注释承认需要凭据，但没有实现
```

```go
// forge-core/cmd/forge/route.go:208
fs.BoolVar(&sig.TouchesSecrets, "touches-secrets", false,
    "change reaches secrets/credentials")
// ——路由层能识别「涉及秘密」的改动，但没有安全处理流程
```

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    // 没有 SecretStore / Vault / CredentialProvider 字段
    // 只有 Dir, Command, Args, Env, Timeout, MaxDepth, MaxOutputBytes
}
```

安全扫描覆盖了 `before`（写码前检测泄漏），但 `after`（使用凭据完成部署）完全是空白。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 需要 `DEPLOY_TOKEN` 推送 Docker 镜像 | Agent 无法完成部署阶段 | 无机制——agent 卡在「无凭据」状态 |
| Reviewer 需要读取 GitHub 私有仓库的代码 | Reviewer 无法审查依赖的代码 | 无机制——reviewer 只能看到自己仓库的代码 |
| Planner 需要查询 issue tracker 的优先级 | Planner 无法获取外部输入 | 无机制——planner 局限于本地文件 |
| 多项目共享同一凭据 | 凭据复制 N 份，撤销时需要 N 处操作 | 无集中管理 |
| 凭据轮换（credential rotation） | 系统在轮换后继续使用过期凭据 | 无生命周期感知 |
| 审计：谁用了哪个凭据做了什么 | 凭据使用完全不可审计 | 无日志 |

### 与已有分析的差异化证明

- `secret-scan` 在所有分析中只作为「检测泄漏」的执法器被讨论（`expansion-core-five.md` 方向五）。
- **没有任何一篇分析提出「agent 如何安全获取凭据来完成部署」这一互补问题。**
- 这也不是 `forge detect`（检测项目类型）或 `forge doctor`（诊断配置健康）能覆盖的——它是一个尚不存在的子系统。

### 建议的工程边界

1. **不做 Vault 集成（那是部署级基础设施）。** 做的是 ForgeOS 级别的凭据声明与代理：
   - 在 `project.yml` 或新 `.agent/secrets.yml` 中声明需要的凭据（声明式，不存值）
   - 通过环境变量 + `.env`（git-ignored）注入实际值，或通过外部命令获取（`aws secretsmanager get-secret-value`）
   - 声明 → 注入路径经 `secret-scan` 豁免（避免扫描工具误报系统级注入）
   - 凭据使用有 trace 事件记录（便于审计），但日志中**屏蔽值内容**

2. **在 CommandExecutor 层增加 SecretProvider 接口**：
   ```
   type SecretProvider interface {
       // Env returns a map of env-var-name → secret-value for the given phase/agent.
       Env(ctx context.Context, phase asset.Phase, agent string) (map[string]string, error)
   }
   ```
   默认实现：读 `.agent/secrets.yml` 的声明 + `.forge/secrets.env`（自动 git-ignored）的取值。
   AWS/GCP/Vault 实现是外部扩展（v3）。

3. **Agent 卡在凭据缺失时应产生可诊断的错误**：
   当系统检测到 `forge run / forge deploy` 需要凭据但未配置时，输出清晰的诊断信息而非静默失败：
   ```
   forge run: phase "deploy" requires credential "DOCKER_TOKEN"
     (declared in .agent/secrets.yml:10)
     → set DOCKER_TOKEN in .forge/secrets.env or run `forge secrets set DOCKER_TOKEN`
   ```

---

## 方向二：测试套件质量门禁（Test Quality Gatekeeping）

### 现状

ForgeOS 的门禁体系能可靠地回答「测试是否通过了」（`test_pass` / `app_test_pass`），但**不能回答测试是否足够好**。

```go
// forge-core/cmd/forge/gates.go
// gate_test.go: 测试 pass/fail 判定
// acceptance.mjs: test / app-test 全绿 → PASS
```

当 agent 写测试时，最常见的反模式是：

- **Tautological tests**：测试与实现使用同一逻辑，永远不会 fail（例如测试一个 `add(a,b)` 函数，测试里自己算了 `a+b` 再断言结果等于 `a+b`）
- **Low-coverage tests**：只测 happy path，不测 error path、边界条件、并发安全
- **Flaky tests**：测试偶尔 fail 或 pass，引入 gate 信号的随机噪声
- **Implementation-coupled tests**：测试深度绑定内部实现细节，重构时频繁假阳性 fail
- **No assertion tests**：测试文件存在，但所有 test case 里没有 `assert` / `expect` 调用

### 代码级证据

```go
// forge-core/cmd/forge/gates.go
// gatherSignals / computeCodeTestRatio:
//   codeTestRatio = changed_test_lines / (changed_prod_lines + changed_test_lines)
//   ——只测「测试行数比」，不测「测试质量」
```

```go
// internal/converge/converge.go:77
// CodeTestRatio: 0 = no test changes, 0.5 = equal test and prod, 1 = all tests
// ——同样是数量比率，不是质量指标
```

```javascript
// harness/acceptance.mjs
// test / app-test 检查: 跑测试框架，检查 exit code
// 没有 "did any test actually run?" + "do tests have assertions?" + "is coverage adequate?"
```

当前系统对「测试质量」的全部认知就是一个**代码行数比率**——它完全不知道测试里是否有有意义的行为验证。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 写了 100 行「assert(true)」的测试 | 测试全绿，但全无意义 | 无检测。`test_pass` → PASS |
| Agent 只测了 happy path，跳过所有错误处理 | bug 进入生产但测试一直绿 | 无覆盖度量。`coverage` 适配器已存在但 N/A（缺工具） |
| 测试 3 次 pass 1 次 fail（flaky） | 单次 fail 阻断收敛，导致无意义的 loop-back | 无 flaky 检测。`forge accept` 判 REJECTED 但原因不透明 |
| Agent 删了有意义的测试并替换为 stub | 测试套件退化但行数保持 | `CodeTestRatio` 不变，无退化告警 |
| 100 个测试全部 pass 但只覆盖了 5% 的代码 | 大量未测试代码引入回归风险 | `coverage` 适配器 N/A（因为覆盖率工具缺），系统对此完全不可见 |

### 与已有分析的差异化证明

- `novel-extensions-v12-architect-perspective.md` 提到 flaky tests 一次（在变异测试上下文中），但未将其作为系统级门禁提出来。
- `strategic-extensions-v15-deep-boundary.md` 讨论了测试结果对调试的价值（「哪个测试失败？」「哪次 iteration 引入？」），但用的是**已有测试结果**做诊断，不是**测试质量度量**。
- `execution-semantic-gaps.md` 和 `expansion-production-readiness.md` 大量讨论 gate 可靠性，但聚焦于 gate **流程**（并发安全、原子性、组合测试），而非 gate **输入**（测试本身的质量）。
- **没有任何分析提出「测试质量下降是 LLM 编码助手特有的风险」这一命题。**

### 建议的工程边界

1. **不做复杂变异测试（那是长时间计算任务）。** 做的是四个轻量级启发式检查，嵌入门禁体系：

   - **Assertion presence check**：扫描测试文件中 `assert` / `expect` / `should` / `t.Error` / `t.Fatal` 的出现频率。如果测试文件数 > 测试断言数，给出 WARN（不是 FAIL——诚实降级）
   - **Coverage adapter 硬化**：当前 `coverage` 适配器已在框架层面就绪（Sprint 12），`forge-init` 复制了适配器文件，但**新项目开箱仍 N/A**。优先完成 Go/Node/Python 三种覆盖率工具的零配置探测，使 `coverage_threshold` 成为真正的 `pass/fail` 门
   - **Flaky test 检测器**：在 `forge evolve` 的多次 iteration 中，自动重跑上次 FAIL 后又 PASS 的测试。同一测试在相邻 iteration 中交替 pass/fail → 标记为 `potential_flaky` 并降级其 gate 权重（不阻断收敛但记入质量趋势）
   - **Test-to-code coverage trend**：单次 coverage 绝对值不准，但跨 iteration 的**趋势**（coverage 在持续下降）是强信号，应触发 `forge doctor` 告警

2. **CodeTestRatio 增强为 CodeTestHealth**：
   不再只是行数比，而是组合指标：
   ```
   CodeTestHealth = w1 × ratio + w2 × assertion_density + w3 × coverage_trend
   ```
   各维度的权重在 `modes.yml` 中按 mode 可配（engineering 模式要求全部三个维度，explorer 只要求 ratio）。

---

## 方向三：Agent 输出结构验证（Output Schema Compliance）

### 现状

当前系统对 agent 输出的解析仅限于**行级机读契约**：

```
VERDICT: APPROVE           → parseReviewerVerdict (cost.go)
VERDICT: REDESIGN          → parseExecutiveVerdict (cost.go)
CONFIDENCE: 85             → parseConfidenceScore (cost.go)
```

这些是 **last-line magic tokens**——它们告诉系统 agent 的**裁决**，但完全不验证 agent 输出内容的**结构**。

实际内容（PRD、架构文档、审查报告）是自由文本，没有任何结构保证：

```markdown
# PRD（product-manager 的输出）
## 市场分析
（可能为空——当前无校验）
## 用户故事
（可能只有一条——当前无校验）
## 非功能需求
（可能完全缺失——当前无校验）
## 风险分析
（可能不存在——当前无校验）
```

同样的，架构文档可能缺失数据流图、部署策略；安全审查可能遗漏 STRIDE 的某些维度；性能审查可能没有负载测试数据。**系统无法区分「好的输出」和「格式正确的空输出」。**

### 代码级证据

```go
// forge-core/cmd/forge/cost.go
// parseReviewerVerdict / parseExecutiveVerdict / parseConfidenceScore
// 全部只扫末行机读 token，不接触输出正文
```

```go
// forge-core/cmd/forge/prompt_artifacts.go
// buildPromptWithEmits 注入的约束只有：
// - 引用 agent card 的职责段
// - 注入 ADR 上下文
// - 指令 agent "输出 PRD 到 docs/discovery/prd.md"
// ——没有定义 PRD 应该包含哪些章节
```

```
// .agent/agents/product-manager.md
// 职责描述是散文格式：
// "Write a PRD with user stories, acceptance criteria, and risk assessment"
// ——但没有任何结构定义（JSON schema / YAML schema / Markdown template）
//    来验证 agent 实际产出的 PRD 是否包含这些章节
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Product manager 输出 PRD 但遗漏了「非功能需求」 | 下游 architect 基于不完整 PRD 做设计 | 无检测——所有下游 agent 不知道 PRD 不完整 |
| Security engineer 遗漏了「数据存储安全」维度的分析 | STRIDE 矩阵缺一行，但 agent 输出了 VERDICT: APPROVE | 无检测——系统认为审查通过了 |
| Architect 的提案没有「成本估算」和「风险分析」 | Human approver 没有足够信息做决策 | 人会在 Design→Build 闸门处发现，但**此前已经浪费了 agent 的 token 和墙钟** |
| Implementer 的输出缺少 README / API 文档 | 产出物不完整但 gate 全绿 | `emits:` 只检查文件是否存在，不检查内容完整性 |
| Performance engineer 审查报告没有性能测试数据 | 结论缺少实证支撑，但 VERDICT token 正常解析 | 无检测 |

### 与已有分析的差异化证明

- `expansion-production-readiness.md` 方向二（信号硬化）讨论的是「如何保证收敛信号的可靠性」——即 `ReviewStatus`、`RequirementConfidence` 等信号是否能被正确解析和赋值。它关注的是机读 token 的**提取正确性**，不是输出正文的**结构完整性**。
- `execution-semantic-gaps.md` 方向五（资产 Schema 治理）讨论了 asset 结构体的版本一致性，关注的是 **Go 代码中 asset struct 与 YAML 声明的对齐**，不是 agent 输出（markdown/自由文本）的结构对齐。
- **本文方向提出的是「agent 产出的文档应该像 API 响应一样有 schema 可验证」——这在所有已有分析中为零覆盖。**

### 建议的工程边界

1. **不做 LLM-as-judge 自动评分（rater drift + 成本不可控）。** 做的是**结构完整性检查（completeness check）**：
   - 为每类产出（PRD、架构文档、审查报告、测试计划）定义轻量级 **Markdown section header schema**
   - 实现 `harness/output-check.mjs`：扫描产出的 markdown 文件，检查 `##`/`###` 标题是否匹配预期结构
   - 缺失的章节 → WARN（不是 FAIL——结构检查是质量增强，不是阻断门）
   - 预期结构定义在 `.agent/agents/*.md` 的模板段中（这是 agent card 本该有但从未用过的字段）

2. **模式定义示例**（在 `product-manager.md` 的职责段声明）：
   ```yaml
   output_schema:
     prd.md:
       required_sections:
         - Market Analysis
         - User Stories
         - Acceptance Criteria
         - Non-Functional Requirements
         - Risk Assessment
         - Success Metrics
       recommended_sections:
         - Technical Spikes
         - Rollout Plan
   ```

3. **消费端注入**：
   当 `buildPrompt` 构建 agent 指令时，如果目标 phase 有 `output_schema` 声明，在 prompt 末尾追加：
   ```
   ## Output structure requirements
   Your PRD MUST include all of these sections:
   • Market Analysis
   • User Stories
   • Acceptance Criteria
   • Non-Functional Requirements
   • Risk Assessment
   • Success Metrics
   ```
   这是**双向治理**：既验证输出，也（更重要的是）指导 agent 产出更好结构的输出。

---

## 方向四：Prompt 效能度量与系统级 Prompt 优化（Instruction Effectiveness Loop）

### 现状

ForgeOS 的 `buildPrompt`（`prompt_context.go`）是所有 agent 行为的最终指令源。它组合了：

- Agent role card（角色定义 + 职责 + 输出契约）
- Workflow phase 指令
- 上下文记忆（memory）
- ADR / AGENTS / ROADMAP 注入
- 前序 phase 输出（gate裁决 / phase output ledger）

**但系统完全不知道这个 prompt 是否有效：**

- Agent 是否理解了指令？
- Agent 是否按格式输出了？
- Prompt 是否太长导致 agent 忽略了关键部分？
- 是否需要调整 memory 注入的顺序 / 长度 / 相关性阈值？
- 是否需要优化 task formulation（任务描述方式）以减少 loop-back？

当前没有任何指标来衡量 prompt 的有效性：

```go
// forge-core/cmd/forge/prompt_context.go
// buildPrompt 的返回是 string（prompt 文本），
// 没有返回 prompt 相关的元数据（token 数、注入源列表、指令版本）
```

```go
// forge-core/cmd/forge/prompt_artifacts.go
// 模板注入逻辑：读文件 → 注入字符串
// 没有「这个模板是否产生了预期 agent 行为」的反馈回路
```

### 代码级证据

```go
// forge-core/internal/trace/trace.go
type Event struct {
    Kind       string // "agent" | "gate" | "converge" | "iteration"
    Phase      string
    Agent      string
    Model      string
    DurationMs int
    CostUsd    int
    // 没有 PromptHash / PromptTokens / PromptVersion
    // 没有 effectiveness 相关字段
}
```

```go
// forge-core/cmd/forge/scorecard_wind.go
// Scorecard 维度：quality, latency, cost
// quality 当前恒 N/A——因为没有任何质量评估机制
// 但如果有了质量评估，也无法关联到 prompt 版本
// ——trace 中没有 prompt 版本指纹
```

```go
// forge-core/internal/prompt/cache.go
// ContextCache: run-scoped 不变上下文构建一次、各 phase 复用
// 缓存的是 ADR + AGENTS + ROADMAP 的渲染结果，
// 但从不缓存 prompt 本身，也不对 prompt 版本做管理
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| `memory` 注入太长导致 agent 忽略了核心 instruction | Agent 产出偏离预期，增加 loop-back | 无检测——`boundMemory` 只做硬截断，不做重要性排序反馈 |
| Reviewer 指令格式变更后，agent 停止输出 VERDICT token | 评审结果无法解析，无意义浪费 opus 成本 | 无检测——`parseReviewerVerdict` 返回空，但空 verdict 不代表「评审失败」 |
| 某个 AGENTS.md 段的写法导致 agent 行为退化 | 所有 agent 角色同时受影响，发散多个方向 | 无检测——每次 `forge run` 都用同一组 AGENTS.md，从来无人读效果回馈 |
| ADR 注入数量增加后 agent context window 压力增大 | Agent 遗忘更早的指令或上下文 | `ContextCache` 存在但只缓存不变内容，不预警 prompt 大小 |
| 同一 task 用不同措辞产生完全不同的产出质量 | 好的措辞无法被保留和复用 | 无从知道——没有 prompt 版本追踪 |

### 与已有分析的差异化证明

- `expansion-production-readiness.md` 方向一（Prompt QA）讨论的是「buildPrompt 逻辑是否需要测试」——即如何通过单元测试验证 prompt 构建代码的正确性。这是**开发时质量**（testing-time quality）。
- `second-order-architectural-gaps.md` 方向四的边栏提到「prompt 版本指纹」和「rollback 到上周 prompt」——但这只是作为 trace 增强的一个观察点，不是作为独立的效能度量体系提出的。
- `expansion-next-frontier.md` 提到了 evaluator version tracking（评估者版本追踪），但聚焦的是**评分模型的版本**，不是**被评估的 prompt 的版本**。
- **本文方向提出的是「prompt 本身应该是可度量、可优化、可版本管理的资产」——这在所有已有分析中尚未被作为一个完整方向提出。**

### 建议的工程边界

1. **不做自动化 prompt 优化（A/B 测试需要真实 agent 调用 + 统计显著样本量，成本极高）。** 做的是**效能度量与可观测性**：
   - 在 trace event 中加入 `prompt_hash`（prompt 模板内容的 SHA256）和 `prompt_token_count`
   - Scorecard 的 `quality_score` 按 `(agent_role, model, prompt_hash)` 分桶，使路由能识别「同样 agent+model，prompt 版本 X 的表现比版本 Y 好」
   - `forge doctor` 新增 `--prompt-stats`：按 prompt 版本汇总 agent 成功率、gate 通过率、loop-back 频率

2. **Prompt 模板版本目录**：
   ```
   .agent/prompts/
     v1/
       implementer.md    # 当前版本
       reviewer.md
       …
     v2/                  # 试验版本，不影响当前运行
       implementer.md
   ```
   `project.yml` 增加 `prompt_version: v1` 选择活跃版本。`forge migrate --prompt-version v2` 切换版本。

3. **Effectiveness 代理指标（不直接测量「prompt 是否被理解」，但测量强相关信号）**：
   - **Verdict parse rate**：agent 输出中机读 token 被成功解析的比例。`parseReviewerVerdict` 成功 / 总 review phase 数。下降 = prompt 格式指令可能有问题
   - **First-pass gate pass rate**：agent phase 在首次执行（无 loop-back）即通过 gate 的比例。下降 = task formulation 可能需要调整
   - **Retry rate per phase**：同一 phase 因 agent 错误（非 gate 失败）需要 retry 的频率。上升 = prompt 可能导致 agent 行为不稳定
   - **Average output size**：agent 输出 token 数趋势。异常增加 = agent 可能在「绕圈子」，减少 = agent 可能在偷懒

---

## 方向五：非交互式输出模式与 CI/CD 集成协议（Structured Output Protocol for Automation）

### 现状

ForgeOS 的 CLI 输出完全是**人类可读的自由文本**：

```text
forge run: --parallel ignored (workflow declares no depends_on) — running serially
convergence: MET (conjunction)
  [x] gates_status=green — All gates pass
  [x] roadmap>=100% — 100/100 items done
```

这在终端交互场景下很好。但在以下场景中完全不适用：

1. **CI 流水线**（`.github/workflows/forge.yml` 已经定义了 CI 调用 `forge accept`）
2. **自动化脚本**（cron 定时 `forge evolve` → 结果汇总到监控系统）
3. **Webhook 回调**（GitHub webhook 触发 `forge run` → 结果返回 PR comment）
4. **事件驱动的聚合**（多个项目的 forge 结果汇总到中央 dashboard）

当前的问题是：

- `forge accept` 输出人类文本，CI 只能通过 exit code 知道 PASS/FAIL，**无法知道具体哪个门 FAIL 了**
- `forge run` / `forge evolve` 的进度信息与输出信息混合打印在 stdout，无法程序化解析
- 没有机器可读的「运行结果摘要」——trace.jsonl 有事件细节但没有汇总
- `--json` 标志仅 `forge detect` 和 `forge doctor` 支持，核心编排命令不支持

### 代码级证据

```go
// forge-core/cmd/forge/main.go
// 所有 cmd* 函数的返回值都是 int（exit code）
// 所有输出都是 fmt.Print / fmt.Printf 到 stdout/stderr
// 没有返回 structured result 的接口
```

```go
// forge-core/cmd/forge/gates.go
// reportConvergence 直接 fmt.Printf 结果
// 调用方（execEngine / buildLoop）无法拿到 structured convergence result
```

```yaml
# .github/workflows/forge.yml
# CI 已经定义但只能做:
#   node harness/acceptance.mjs → exit code 0/1
# 无法做: 
#   - check if specific gate failed
#   - report which test failed
#   - attach convergence report to PR
```

```go
// forge-core/cmd/forge/preflight.go
// preflightReport 结构体可以累加 PASS/FAIL 结果，
// 但最终输出也是通过 fmt.Printf 渲染成人类文本，
// 没有 `--json` 输出模式。
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| CI 中 `forge accept` 返回 1（REJECTED） | CI 只知道「失败了」，不知道为什么 | 只能人读日志找原因 |
| 多个并行 CI job 的结果需要聚合 | 无法程序化聚合 | 无结构化输出可供聚合 |
| Slack/Teams 通知每次 evolve 结果 | 需要自由文本解析 | 不可靠——文本格式变更即 break |
| 跨项目 forge 状态 dashboard | 无法汇总多项目状态 | 不可能实现 |
| PR comment 自动贴 `forge run` 结果 | 需要结构化数据来格式化 | 只能贴整段 stdout |

### 与已有分析的差异化证明

- `expansion-horizon-three.md` 方向三（外部事件驱动）讨论了 webhook listener 触发 `forge run`，但聚焦的是**触发方式**（push→run），而不是**运行结果的传输格式**。
- `strategic-extensions-v22.md` 方向三（实时可观测性）讨论了流式遥测（trace/metric/log），聚焦的是**运行时数据流**，不是**特定 run 的结构化结果报告**。
- `high-value-expansion-directions.md` 方向五（运行诊断/`forge investigate`）讨论的是**人类阅读的运行分析**（事后调试工具），不是**机器读取的结果协议**（自动化集成接口）。
- **本文方向提出的是「ForgeOS 需要一个结构化的输出协议，使自身成为一个可编程的治理平台」——这在所有已有分析中为零覆盖。**

### 建议的工程边界

1. **不做 Web UI（那是 v3 的独立大项目）。** 做的是 CLI 的 `--json` 输出协议扩展：

   - `forge run --json`、`forge evolve --json`、`forge accept --json` 输出机器可读的 JSON
   - 输出包含：
     ```json
     {
       "command": "forge run build",
       "exit_code": 1,
       "run_id": "evolve-20260709-143022",
       "workflow": "build",
       "mode": "balanced",
       "lifecycle": "mvp",
       "duration_ms": 34500,
       "phases": [
         {"name": "planner", "status": "passed", "duration_ms": 8000, "model": "sonnet", "cost_usd": 0.08},
         {"name": "implementer", "status": "passed", "duration_ms": 15000, "model": "sonnet", "cost_usd": 0.25},
         {"name": "reviewer", "status": "failed", "duration_ms": 7000, "model": "opus", "cost_usd": 0.40},
         {"name": "qa", "status": "skipped", "reason": "reviewer did not approve"}
       ],
       "gates": {
         "gate_test": {"status": "passed", "detail": "12/12 tests passed"},
         "gate_lint": {"status": "passed", "detail": "eslint: no issues"},
         "gate_secret-scan": {"status": "passed", "detail": "0 secrets found"},
         "gate_arch-check": {"status": "failed", "detail": "fan-in violation in internal/gate (14 > 12)"}
       },
       "convergence": {"met": false, "stop_type": "conjunction", "criteria": [
         {"name": "gates_status", "met": false, "detail": "arch-check failed"},
         {"name": "roadmap", "met": true, "detail": "8/8 items done"}
       ]},
       "cost": {"total_usd": 0.73, "phase_breakdown": {"sonnet": 0.33, "opus": 0.40}},
       "recommendation": "Retry with --mode engineering to tighten harness; reviewer has identified architectural concerns"
     }
     ```

2. **输出协议的 CI 消费示例**（`forge run build --json | jq`）：
   ```bash
   # CI script
   result=$(forge run build --json)
   echo "cost: $(echo $result | jq -r .cost.total_usd)"
   if [ "$(echo $result | jq -r .convergence.met)" = "false" ]; then
     failing_gates=$(echo $result | jq -r '.gates | to_entries[] | select(.value.status=="failed") | .key')
     echo "Failing gates: $failing_gates"
     # 自动 comment on PR
   fi
   ```

3. **输出协议向后兼容**：
   - `--json` 标志新增（缺省 false），不改变任何现有行为
   - JSON 输出到 stdout，人类阅读文本到 stderr（Unix 惯例：stdout=数据，stderr=日志）
   - 所有现有测试不动——`--json` 是新增路径

---

## 汇总：五方向的价值矩阵与区分度

| # | 方向 | 类别 | 与已有分析的重叠度 | 核心价值 | 实现量级 |
|---|------|------|--------------------|----------|----------|
| 1 | Agent 凭据注入与秘密生命周期管理 | **核心功能** | **零覆盖**（已有分析只讨论检测，不讨论注入） | Agent 能从「写代码」进阶到「部署代码」，解锁完整自治循环 | 中（SecretProvider 接口 + `.agent/secrets.yml` 声明 + 注入路径） |
| 2 | 测试套件质量门禁 | **边界情况** | **边缘触及**（flaky 仅在变异测试上下文提到一次） | 防止「全绿但无用」的测试反模式，提高收敛信号可信度 | 中（assertion check + coverage 适配器硬化 + flaky 检测器） |
| 3 | Agent 输出结构验证 | **核心功能** | **零覆盖**（所有已有分析只验证机读 token，不验证输出正文结构） | 让 PRD、架构文档、审查报告像 API 响应一样有 schema 可验证 | 小（section header scanner + agent card schema 声明） |
| 4 | Prompt 效能度量与优化 | **性能优化** | **边缘触及**（prompt 版本指纹在二阶分析中被提及但未作为方向展开） | 关闭「指令质量」这个最大的效能盲区；让 prompt 成为可优化的资产 | 小～中（prompt_hash 入 trace + 效度代理指标 + prompt 版本目录） |
| 5 | 非交互式输出协议 | **边界情况** | **零覆盖**（已有分析讨论触发方式、可观测数据流、人类可读诊断，但不讨论机器可读的结果协议） | 让 ForgeOS 从交互式 CLI 变成可编程的治理平台；CI 集成的必要前提 | 小（`--json` 输出模式 + 结构化 result 类型） |

### 推荐优先级

1. **方向五（结构化输出协议）**——ROI 最高、实现量级最小。它解锁所有自动化集成场景（CI、Webhook、Dashboard），且与任何其他方向正交。没有它，方向一的凭据注入和方向二的测试质量门禁都无法被 CI 流水线消费。

2. **方向三（Agent 输出结构验证）**——安全和质量的高杠杆改进。在当前「verdict token 全链路」的基础上加一层结构检查，是防御「格式正确但内容空洞」的最佳防线。验证逻辑（Markdown section scanner）纯本地、零外部依赖、零 LLM 成本。

3. **方向一（Agent 凭据注入）**——完整性最高的改进。当前自治循环在「代码写好但部署不了」处断裂。凭据注入是修补这一断裂的最关键链路。但需要安全审查和设计讨论，建议先做设计 ADR。

4. **方向二（测试质量门禁）**——防御性最強的改进。LLM 写「passing but useless」测试是已知问题，且随系统使用时间增长而恶化。但覆盖率适配器硬化需要验证多种语言工具链，周期可能较长。

5. **方向四（Prompt 效能度量）**——影响力最大但投产周期最长的方向。涉及 trace schema 扩展、效度代理指标建立、prompt 版本目录约定。建议先只做 `prompt_hash` 入 trace（1-2 天工作），让数据开始积累，优化决策推迟到有数据支撑时。

### 被排除的方向（与已有分析确认的重叠）

| 方向 | 已有分析覆盖 | 原因 |
|------|-------------|------|
| 多维模型路由自动化 | `high-value-expansion-directions.md` §1 | 已完整覆盖 |
| 并行调度引擎激活 | `high-value-expansion-directions.md` §2 | 已完整覆盖（含 depends_on 声明建议） |
| 管线工作流组合 | `expansion-horizon-three.md` §1 | 已完整覆盖 |
| 多仓库联邦治理 | `expansion-horizon-three.md` §2 | 已完整覆盖 |
| 跨会话修正学习 | `expansion-horizon-three.md` §5 | 已完整覆盖 |
| Prompt 构建管道 QA | `expansion-production-readiness.md` §1 | 已完整覆盖 |
| 收敛信号硬化 | `expansion-production-readiness.md` §2 | 已完整覆盖 |
| 知识质量衰减 | `second-order-architectural-gaps.md` §1 | 已完整覆盖 |
| 多进程并发安全 | `high-value-expansion-directions.md` §4 | 已完整覆盖 |
| 运行诊断/根因分析 | `high-value-expansion-directions.md` §5 | 已完整覆盖 |

---

## 附录：验证策略建议（非实现代码）

每方向的验证建议：

1. **凭据注入**：
   - 单测：Mock `SecretProvider` 返回 map → 验证 `CommandExecutor.childEnv` 包含预期键值
   - 端到端：声明凭据 → `forge run deploy` → 验证子进程 env 中有对应变量

2. **测试质量门禁**：
   - 单测：构建不同质量的测试文件 → 验证 assertion density / flaky detection 正确分类
   - 集成：对 `examples/url-shortener` 注入「tautological 测试」→ 验证 `forge accept` WARN 但不阻断

3. **输出结构验证**：
   - 单测：定义 schema → 对不同内容的 PRD markdown 运行 scanner → 验证缺失章节正确检出
   - 端到端：`product-manager.md` 声明 output_schema → `forge run discover` → 验证 PRD 产出后自动运行结构检查

4. **Prompt 效能度量**：
   - 单测：`buildPrompt` 返回结构体含 `PromptHash`、`TokenCount` → 验证两次相同输入产出相同 hash
   - 集成：trace 写入后 → 按 prompt_hash 分组查询 → 验证分组正确

5. **结构化输出协议**：
   - 单测：mock gate 结果 → `reportConvergence` 在 `--json` 模式下输出合法 JSON → 内容与 exit code 一致
   - 集成：`forge run --json build` → 验证 JSON 结构包含 phases/gates/convergence/cost 四段
