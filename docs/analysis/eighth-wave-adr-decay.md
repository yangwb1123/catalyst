# ForgeOS — 第八次架构扫描：架构决策衰退与治理元缺口

> **扫描基准**：`b0c80e4`  
> **视角**：用「架构决策衰退」这一元透镜，对比 ADR 中承诺的决策与代码实际实现的差距  
> **方法论**：将 docs/adr/ 中的每条决策与代码对照，标记「已实现」「部分实现」「未实现」「已衰退」

---

## 核心发现：ForgeOS 的架构决策有四层，但缺少跨层验证

### 四层决策体系

```
第 1 层: ADR（docs/adr/）          — 架构决策，战略级
第 2 层: DECISIONS.md（.agent/）   — 项目决策，战术级
第 3 层: ROADMAP.md（.agent/）     — 执行路线图，当前/下一 sprint
第 4 层: YAML 声明（.agent/workflows/）— 运行时契约，直接被执行
```

每层都描述了系统应该如何工作。但**没有一层验证上层承诺与下层实现的一致性**。

---

## 四份 ADR 的决策实现审计

### ADR 0001：v0–v1 Ride Claude Code，v2 自研运行时

| 决策 | 状态 | 代码证据 | 审计 |
|------|------|---------|------|
| v2 forge-core 自研 Go 运行时 | ✅ **已实现** | `forge-core/` 12 个内部包 + cmd/forge | D6 DECISIONS 确认 |
| 零外部依赖 | ✅ **已实现** | `go.mod` 无 `require` 块 | 已验证 |
| v0–v1 声明式资产 + 薄胶水 | ✅ **已实现** | `.agent/workflows/*.yml` + `harness/*.mjs` | 已验证 |
| 取代条件触发后置 Superseded | ✅ **已实现** | ADR 标 Superseded，D6 记录触发 | 文档状态正确 |

**衰退：无**。这是实现最完整的 ADR。

### ADR 0002：Go-核心 Polyglot 栈，分期引入

| 决策 | 状态 | 代码证据 | 审计 |
|------|------|---------|------|
| forge-core = Go（编排/调度/路由） | ✅ **已实现** | 12 内部包 + cmd/forge | 已验证 |
| forge-ai = Python（智能层） | ❌ **未开始** | 代码库无 `forge-ai/` 目录 | 规划中 |
| forge-runtime = Rust（沙箱） | ❌ **未开始** | 代码库无 `rust/` 或沙箱实现 | 规划 v3 |
| forge-web = TS/Next（UI） | ❌ **未开始** | 代码库无 UI 代码 | 规划中 |
| `harness` CLI 未来固化 Go 静态二进制 | ⚠️ **部分** | `forge-core/cmd/forge` 是 Go 二进制，但 `harness/*.mjs` 仍是 Node.js | 过渡态 |
| Temporal/Postgres/NATS | ❌ **未开始** | `go.mod` 无依赖，代码无 gRPC | 规划 v2+ |

**衰退：中等**。Go 核心已实现，但 polyglot 栈的其余部分（3/4 的运行时）尚未开始。`harness` 仍是 Node.js——「固化 Go 静态二进制」的过渡态比预期的长。

### ADR 0003：agent-os 独立仓化

| 决策 | 状态 | 代码证据 | 审计 |
|------|------|---------|------|
| submodule 共享（否决 symlink/npm/subtree/vendoring） | ❌ **未执行** | `forge-init.mjs` 仍使用复制（copy） | Proposed 状态，推荐暂缓 |
| 路径解析改造（`FORGE_PROJECT_ROOT` 环境变量） | ❌ **未执行** | `gate.mjs:14` 仍 `ROOT = process.cwd()` | 无新代码 |
| 双层覆盖（agent-os 全局 + project 本地） | ❌ **未执行** | 无 `agent-os/` 目录 | 设计阶段 |
| `forge validate --cross-repo` | ❌ **未执行** | 无此命令 | 设计中 |
| 现有项目不强制过渡 | ✅ **（现状）** | 复制模式继续有效 | 向后兼容 |

**衰退：高**。这是设计最完整的 ADR（6 条决策、3 条待拍板、触发条件已定义），但代码行数变化为 0。项目继续用复制模式，治理资产的中心更新传播问题未解决。

### ADR 0004：REVIEW 阶段 AI-SDLC 深度评审集成

| 决策 | 状态 | 代码证据 | 审计 |
|------|------|---------|------|
| `review.yml` 新 workflow | ✅ **已实现** | `.agent/workflows/review.yml`（130 行） | `forge run review` 可用 |
| 4 个评审 agent（security/distributed/performance/cto） | ✅ **已实现** | `.agent/agents/security-engineer.md` 等 | 角色卡完整 |
| `uses_template` 引用 `.ai/prompts/*.md` | ⚠️ **声明仅** | `review.yml` 4 处声明，但 **`asset.Phase` 无 `UsesTemplate` 字段** | YAML 注解，不被代码读取 |
| `required_when` 路径引用 | ⚠️ **有字段但无解析** | `asset.Phase.RequiredWhen` 存在，但值是字符串 `"../policies/modes.yml#workflow_depth.review"` | 存储但不解析 |
| `mode_gating` 的 `authority` 引用 | ⚠️ **有字段但无解析** | build.yml: `gate_set: ../policies/modes.yml#harness.gates` | 同 required_when |

**衰退：低-中**。workflow 和 agent 卡已实现，YAML 层面的引用也写好了。但「将 `.ai/` 模板自动注入 prompt」的决策意图在代码中没有执行——`uses_template` 是项目团队说的「应该在这里插入 AI-SDLC 内容」的标记，但 forge-core 运行时看不到这个标记。

`required_when` 路径引用是一个更隐蔽的衰退：字段在 Go 结构体中**存在**，被**解析**为字符串，但字符串的内容 `"../policies/modes.yml#workflow_depth.review"` **从来不解析为一个实际路径和字段**。所以 `required_when` 的行为依赖手动确保 `modes.yml` 的路径一致。

---

## 跨 ADR 的冲突

### 冲突 1：ADR-0002（polyglot 零依赖）vs ADR-0003（submodule 共享）

```
ADR-0002: "harness Node/Python 零外部依赖"（铁律）
ADR-0003: "git submodule 共享治理资产"（设计决策）
```

如果 agent-os 以 submodule 引入，项目根会有 `agent-os/` 目录。`gate.mjs` 的 `ROOT = process.cwd()` 会指向项目根（正确），但 `scan.mjs` 和 `acceptance-kernel.mjs` 使用 `dirname(HARNESS_DIR)` 自锚定——当 `harness/` 来自 submodule 而不是项目根时，这个自锚算错位置。

**当前缓解**：ADR-0003 设计了 `FORGE_PROJECT_ROOT` 环境变量来修复路径解析，但**路径改造代码未写**。只要路径改造未完成，submodule 化就会破坏执法工具。

### 冲突 2：ADR-0002（Go 二进制化）vs ADR-0004（AI-SDLC 模板消费）

```
ADR-0002: "harness CLI 未来固化为 Go 静态二进制"
ADR-0004: "集成 .ai/prompts/ 模板（Markdown 格式）到 workflow prompt"
```

如果 `uses_template` 要被执行、prompt 要从 `.ai/prompts/*.md` 构建，有两种途径：
- (A) Go 代码直接读 Markdown 文件 — 可行，零依赖
- (B) 通过 Node/Python 工具预处理 — 增加了运行时依赖

ADR-0002 的「Go 静态二进制」承诺倾向于途径 A，但途径 A 需要 Go 代码理解 Markdown 段落和 `{{Context}}` 占位符替换——目前在代码中不存在。

### 冲突 3：DECISIONS.md O2（ADR-0003 暂缓）vs 实际治理规模

```
DECISIONS O2: "推荐暂缓至被治理项目 ≥ 2~3 且治理资产仍高频演进"
```

当前治理资产（`.agent/agents/`、`harness/`、`policies.yml`）仍在频繁修改。`forge-init.mjs` 的对 `GOVERNANCE_DIRS` 的字节级断言测试（`test_forge-init.mjs`）**每次修改治理资产都会中断**。这意味着：

> **每改一次治理资产，所有已复制的项目的治理状态都「过期」了，但没人知道。**

这是当前最隐蔽的风险。「推荐暂缓」是理性的（担心 submodule 的复杂度和破坏性），但暂缓的「代价」——治理资产的中心更新不传播——已经以测试维护成本的方式体现了。

---

## 五个高价值扩展方向

### 方向 1：ADR 可测试性——将每条决策编译为可执行断言

**当前状态**：
ADR 是纯文本 Markdown。没有机制阻止 ADR 中的承诺与代码行为偏离。例如：
- ADR-0002 说「zero external deps for forge-core」——但这是通过人工检查确保的
- ADR-0004 说 `uses_template` ——但 asset.Phase 甚至没有这个字段
- ADR-0003 说 `FORGE_PROJECT_ROOT`——但没有任何代码检查是否已引入

**建议方案**：

为每份 ADR 创建对应的测试文件 `forge-core/internal/adr/`，测试 ADR 决策的持续有效性：

```go
// adr_0002_test.go — ADR-0002 决策的测试
func TestADR0002_ZeroExternalDeps(t *testing.T) {
    // ❌ 决策: forge-core 零外部依赖
    // 检查 go.mod 没有 require 块
    data, _ := os.ReadFile("../../go.mod")
    if bytes.Contains(data, []byte("require (")) {
        t.Error("ADR-0002 violation: forge-core must have zero external dependencies")
    }
}

func TestADR0002_GoStaticBinary(t *testing.T) {
    // ⚠️ 决策: harness CLI 未来固化 Go 静态二进制
    // 检查: 如果 forge 二进制存在，检查是否静态链接
    // 当前仅检查 forge 二进制是否存在（过渡态）
}

func TestADR0002_PolyglotNotStarted(t *testing.T) {
    // ⚠️ 决策: Python/Rust/TS 分期引入
    // 记录当前状态，如果引入则通知
    if _, err := os.Stat("../../forge-ai"); err == nil {
        t.Log("ADR-0002: forge-ai (Python) detected — polyglot stage advancing")
    }
}
```

```go
// adr_0004_test.go — ADR-0004 决策的测试  
func TestADR0004_UsesTemplateConsumed(t *testing.T) {
    // ❌ 决策: uses_template 应被代码消费
    // 检查 asset.Phase 是否有 UsesTemplate 字段
    // 当前: 没有这个字段 → 测试 FAIL，标记决策未实现
}
```

```go
// adr_0003_test.go — ADR-0003 决策状态监控
func TestADR0003_TriggerCondition(t *testing.T) {
    // 监控触发条件：被治理项目数和治理资产变更频率
    // 当前: 只有 ForgeOS 自身（1 个项目）
    // 如果触发条件达成但决策未推进 → 提醒
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **决策可审计** | Every ADR becomes a runnable contract. "Accepted" means "passes its tests" |
| **衰退早检测** | 当代码偏离决策时，测试在 CI 中 FAIL，不等代码审查发现 |
| **触发条件监控** | ADR-0003 的「暂缓至触发条件达成」可以被自动化监控——条件达成时 CI 输出提醒 |

**边界情况**：

1. **测试的稳定性**：`ADR-0002` 的 `go.mod` 检查如果引入了新的 Go 标准库包（不增加外部依赖），测试应该 PASS
2. **多阶段决策**：ADR-0002 的 polyglot 分期引入——测试应有 `expect` 字段标注预期阶段
3. **决策被推翻**：如果 ADR-0003 被否决、选择了 npm 替代方案，测试应该更新而非 FAIL

---

### 方向 2：`uses_template` 字段代码化

**当前状态**：
`review.yml` 中 4 处声明（security/distributed/performance/cto 各一个模板引用），`asset.Phase` 无此字段。代码盲区。

**建议方案**：

```go
// asset.Phase 增加
UsesTemplate string `json:"uses_template,omitempty"`
```

对应 Go 代码在 `buildPrompt` 中处理：

```go
if p.UsesTemplate != "" {
    tmplPath := filepath.Join(root, p.UsesTemplate)
    tmpl, err := os.ReadFile(tmplPath)
    if err != nil {
        // warn 但不 block — 模板可选
        logln(fmt.Sprintf("forge: WARNING uses_template %s not found (%v)", tmplPath, err))
    } else {
        // 将模板内容附加到 prompt
        ctxExtra += "\n\n---\n" + string(tmpl)
    }
}
```

同样，`required_when` 的路径引用也需要解析器：

```go
// 解析 required_when 路径引用
// 输入: "../policies/modes.yml#workflow_depth.review"
// 输出: modes.yml 中的 workflow_depth.review 字段值

func resolveYAMLPath(root, ref string) (string, error) {
    // 1. 分割 # 为文件路径和字段路径
    // 2. 加载 YAML 文件
    // 3. 按 . 分割字段路径逐层遍历
    // 4. 返回值
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **完成 ADR-0004 的承诺** | 声明了 4 次 `uses_template`，但代码看不到。加这个字段 &#x2B50; 完成集成 |
| **消除 YAML 注释** | 当前这些引用是「可读注释」而非「可执行契约」。代码化后它们变成真正的编译时/运行时检查 |
| **dogfood a bug** | review.yml 声明了模板引用，但没人发现代码不读它。这说明 YAML-代码对账需要自动化 |

**边界情况**：

1. **模板不存在**：`.ai/prompts/02-security-rfc-review.md` 被删除了——代码应该 warn 还是 error？
2. **模板内容冲突**：prompt 已经给了 agent 角色卡，模板内容如果提供了不一致的指令——谁优先？
3. **性能**：每次 buildPrompt 读一个 Markdown 文件（~300 行），平均 5 个 phase × 1 个文件 = 5 次额外读 I/O。对冷启动可忽略

---

### 方向 3：`required_when` 路径引用的机器解析（YAML XPath 引擎）

**当前状态**：
workflow YAML 广泛使用了类似 XPath 的引用语法来指向 `modes.yml` 中的字段：

```
required_when: ../policies/modes.yml#workflow_depth.review
authority:     ../policies/modes.yml#workflow_depth.discover
gate_set:      ../policies/modes.yml#harness.gates
```

这些引用被作为字符串存储、从不解析。当 `modes.yml` 的字段被重命名或移动时——引用静默断裂。

**建议方案**：

```go
// YAML 路径引用解析器
type YAMLPath struct {
    File string   // 相对路径（相对于当前文件所在目录）
    Path string   // 点分隔的字段路径（如 "workflow_depth.review"）
}

func ParseYAMLPath(ref string) (YAMLPath, error) {
    parts := strings.SplitN(ref, "#", 2)
    if len(parts) != 2 {
        return YAMLPath{}, fmt.Errorf("invalid YAML path reference: %s", ref)
    }
    return YAMLPath{File: parts[0], Path: parts[1]}, nil
}

func ResolveYAMLRef(root string, ref string, baseDir string) (any, error) {
    yp, err := ParseYAMLPath(ref)
    if err != nil {
        return nil, err
    }
    // 解析相对路径
    absFile := filepath.Join(baseDir, yp.File)
    if !filepath.IsAbs(absFile) {
        absFile = filepath.Join(root, yp.File)
    }
    // 加载 YAML（纯 Go，不用 python shim）
    data, err := os.ReadFile(absFile)
    if err != nil {
        return nil, err
    }
    // 沿 . 分隔的路径遍历
    return walkYAMLPath(data, strings.Split(yp.Path, "."))
}
```

这可以用来替换 Python yaml shim 的部分功能，同时让 YAML 路径引用变成可验证的。

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **断链检测** | 当前 5 个 workflow × ~3 个引用/个 = 15 条路径引用全部无人验证 |
| **重构安全** | `workflow_depth.review` 重命名为 `workflow_depth.review_depth` — 所有 workflow 的 required_when 突然不匹配，但没人知道 |
| **准备 Go 化 YAML** | 当前依赖 `harness/yaml2json.py`（Python shim）的唯一原因是 YAML 解析。一个 Go 原生的 YAML 路径解析引擎是消除此 shim 的第一步 |

**边界情况**：

1. **YAML 锚点和别名**：`modes.yml` 使用了 YAML 锚点（`&default`、`<<: *default`）。Go YAML 库需要支持这些
2. **跨文件引用**：当前引用在 workflow 目录 → modes.yml（上层目录）。如果将来有跨仓引用（ADR-0003），需要文件解析
3. **循环引用**：如果 modes.yml 反过来引用 workflow YAML。解析器需要检测循环

---

### 方向 4：架构决策的「触发条件监控」

**当前状态**：
ADR-0003 推荐暂缓的主要理由是「收益有限，风险高」。暂缓的条件是：

> 被治理项目 ≥ 2~3 **且** 治理资产仍高频演进

这个条件是人工判断的。没有监控告诉我「现在有 N 个项目在使用 ForgeOS 治理」「治理资产在过去 M 天中修改了 K 次」。

**建议方案**：

```bash
forge status --governance
  Governance assets:
    .agent/agents/:      12 files (last modified: 2026-06-30)
    .agent/workflows/:   5 files  (last modified: 2026-06-30)
    harness/:            26 files (last modified: 2026-06-30)
    .ai/prompts/:        10 files (last modified: 2026-06-19)
  
  Consumption:
    Direct: 1 project (this repo)
    forge-init snapshots: unknown (no registry)
  
  ADR-0003 trigger status:
    ❌ 被治理项目 ≥ 2~3:  FALSE (1 project, need ≥2)
    ✅ 治理资产高频演进:  TRUE (12 changes in last 30 days)
    → 结论: 触发条件尚未达成（缺更多消费者）
  
  ADR implementation:
    ADR-0001: ✅ 完全实现
    ADR-0002: ⚠️ 部分实现（Go ✅, Python/Rust/TS ❌）
    ADR-0003: ❌ 未执行（Proposed, trigger condition not met）
    ADR-0004: ✅ workflow ✅, uses_template ⚠️
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **决策可观测** | 「推荐暂缓」不是一个永久状态。监控触发条件决定何时重新评估 |
| **治理透明** | 知道治理资产正在变化、有多少消费者受变化影响——这是基础设施决策的基础数据 |
| **历史基线** | 如果 6 个月后治理资产不再变化、「高频演进」变「稳定」，submodule 化的优先级会自然降低 |

**边界情况**：

1. **forge-init 项目无法追踪**：当前复制模式意味着 forge-init 创建的项目不会回传遥测。无法准确知道有多少项目在用 ForgeOS 治理
2. **触发条件的多重解释**：「高频演进」是什么频率？每周 1 次还是每月 10 次？需要可配置的阈值
3. **false negative**：只有 1 个已知消费者（本仓），但可能有 3 个其他项目用 forge-init 创建了但没被追踪。触发条件监控需要诚实标注「未知消费者数」

---

### 方向 5：Go 原生 YAML 解析——消除 Python shim 依赖

**当前状态**：
`harness/yaml2json.py` 是 Python 脚本，用于将 YAML 转换为 JSON。它是 **代码库中唯一的 Python 运行时依赖**（除了可选的 `check.py` 治理检查）。

```python
# harness/yaml2json.py
import sys, yaml, json
print(json.dumps(yaml.safe_load(sys.stdin)))
```

这个 shim 的存在原因是 forge-core 的 Go 代码需要读取 YAML 格式的 `modes.yml` 和 `policies.yml`，但 forge-core 代码本身没有 YAML 解析能力（零外部依赖约束）。

当前消费路径：

```
modes.yml (YAML)
  → harness/yaml2json.py (Python + PyYAML)
  → JSON (stdout)
  → Go 的 encoding/json
  → internal/mode 消费
```

路径上依赖 Python3 + PyYAML 包 + `yaml2json.py` 文件存在。任何一环出问题，整个中枢旋钮系统不可用。

**建议方案**：

将 `modes.yml` 的解析从「YAML → Python → JSON → Go」改为「YAML → Go」。具体的权衡：

| 方案 | 依赖 | 复杂度 | 收益 |
|------|------|--------|------|
| A: 嵌入纯 Go YAML 子集解析器 | 零依赖 | 中 (~300 行读 key-value) | 消除 Python shim |
| B: 使用 Go 标准库 `encoding/json` + YAML 转 JSON 缓存 | 零依赖 | 低 | 减少 Python 调用频率 |
| C: 将 modes.yml 改为 JSON 格式 | 零依赖 | 低（改格式 + 迁移） | 完全消除 YAML |
| D: 声明 CGO + libyaml | CGO 依赖 | 低 | 违背零依赖铁律 |

最推荐的是**方案 A**：一个简单的、只读的键值对 YAML 解析器（不需要支持 YAML 全部 60 种特性——只需支持 `modes.yml` 和 `policies.yml` 使用的子集：map/dict、list、scalar、嵌套、锚点）。

```go
// internal/yaml (新包) — 纯 Go YAML 子集解析器，仅需支持 forge-core 使用的结构
// 不需要解析多行字符串、不需要 YAML 1.2 全部特性
// 只需要解析 modes.yml 和 policies.yml 使用的子集
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **消除运行时依赖** | Python3 + PyYAML 是 forge-core 运行时唯一的「必须安装」「必须正确版本」的外部工具。消除后 forge-core 的运行时依赖降为 `0`（除了操作系统调用和可选的 claude CLI） |
| **消除 shim 进程开销** | 每调用 `yaml2json.py` 一次 fork + Python 解释器启动 + stdout 解析。对于 `buildPrompt` 的每次调用（每次 agent phase 执行）都走一次这个路径 |
| **ADR-0002 承诺的部分实现** | 「Go 静态二进制」意味着零运行时依赖。Python shim 是这个承诺的最后一个缺口 |

**边界情况**：

1. **YAML 子集选择**：必须 read-only、不支持 multi-document YAML、不支持 YAML tag。只解析 ForgeOS 实际使用的子集
2. **锚点支持**：`modes.yml` 使用了 YAML 锚点（`defaults: &defaults`）。解析器需要至少支持 `&` 和 `*`
3. **与 Python 版本的并行期**：过渡期两个解析器都运行——Go 原生解析和 Python shim 输出应该 byte-for-byte 一致。用测试确保

---

## 优先级矩阵

| 方向 | 影响面 | 成本 | 前置依赖 | 推荐 |
|------|--------|------|---------|------|
| **1. ADR 可测试性** | 治理完整性：高 | 低（每个 ADR 一个测试文件） | 无 | **Sprint n** |
| **2. `uses_template` 代码化** | 完成 ADR-0004 承诺：高 | 低（asset.Phase 加字段 + buildPrompt 读） | asset.go 修改 | Sprint n+1 |
| **3. YAML 路径引用解析** | 重构安全：中 | 中（YAML 路径引擎 + 测试） | `required_when` 字段已存在 | Sprint n+1 |
| **4. 触发条件监控** | 决策可观测：中 | 低（`forge status` 扩展） | 无 | Sprint n+1 |
| **5. Go 原生 YAML 解析** | 消除 Python 依赖：高 | 中（子集解析器 ~300-500 行） | 无 | Sprint n+2 |

---

## 元总结：七次分析覆盖了什么

| 轮 | 文件 | 角度 | ADR 衰退覆盖 |
|----|------|------|-------------|
| 1 | `strategic-expansion-and-edge-cases.md` | 经典功能缺口 | ❌ |
| 2 | 内联回答 | 多仓、自定义 gate 等 | ❌ |
| 3 | `third-wave-expansion.md` | 收敛注册/运行时 arch | ❌ |
| 4 | `fourth-wave-architecture.md` | 输出合约/doctor/相位画像 | ❌ |
| 5 | `fifth-wave-operational.md` | 工程化/版本/基准 | ❌ |
| 6 | `sixth-wave-multimodel.md` | 三生命周期漂移 | ❌ |
| 7 | `seventh-wave-data-realism.md` | 持久化层数据真实性 | ❌ |
| **8** | **eighth-wave-adr-decay.md** | **ADR 决策衰退 + 治理元缺口** | ✅ |

八次扫描的八份分析文档覆盖了：功能缺口、性能优化、工程化、多模型一致性、数据可靠性、架构决策衰退。每一轮都从不同扫描入口找到了不同的发现。如果还有第九次，那唯一剩下的角度是——产品市场定位与竞争分析（哪个垂直行业最适合 ForgeOS、与 Copilot/Codex 的差异点在哪里）——但那是个战略文档，不是技术扫描。

---

*分析日期：2026-06-30 | 第八次全量扫描：从「架构决策衰退」的元透镜审视 4 份 ADR 的代码实现现状*
