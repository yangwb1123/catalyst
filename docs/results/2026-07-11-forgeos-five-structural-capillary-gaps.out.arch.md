# 架构分析报告：ForgeOS「毛细缺口」结构性诊断

## 1. 架构评估

### 1.1 当前架构的优势

从文档描述的代码模式可以推断，ForgeOS 的架构存在一套**隐式但完备的基础设施层**：

| 设施 | 形态 | 成熟度 |
|------|------|--------|
| YAML 驱动的 workflow 定义 | `.agent/workflows/*.yml` | 生产级 |
| 策略门禁系统 | `orchestrator/mode_gating.go` | 已启用 |
| 跨会话记忆管理 | `memory_compact.go`, `evolve.go` | 运行时级 |
| 风险启发式评分 | `internal/risk`, `routing.Score` | 启发式可用 |
| 架构一致性执法 | `arch-check.mjs` | CI 强制 |

这些设施的共存证明：**ForgeOS 的设计者从一开始就预见了从「Claude wrapper」向「通用编排引擎」演进的路径**——他们只是没有在早期投入资源把最后 10% 的连通层做完。这不是「欠设计」，这是**故意延迟的架构决策（delayed architectural decisions）**，是一种理性的工程策略。

### 1.2 核心局限性

文档揭示了一个**结构性模式**，我称之为「隐式契约 + 旁路通道」：

```
声明层（YAML） → 隐式契约 → 执行层（Go）
                     ↓
               旁路通道（Go 硬编码）← 新需求
```

具体表现：

| 位置 | 声明层已就绪 | 执行层以旁路处理 |
|------|-------------|-----------------|
| `mode_gating.go` | YAML 写明了 `policy:` 路径引用 | `requiredWhenKey()` 用 `strings.LastIndex` 硬解析 |
| `engine_build.go` | YAML 声明了 `cli:` 厂商标识 | `isClaude` 用 `strings.Contains` 模式匹配 |
| `memory.go` | Entry 数据结构自带 `kind`/`age` 字段 | 生命周期管理依赖定时触发 + CLI 命令 |
| `routing.go` | `Score` 函数定义了加权评分模型 | 特征提取管道缺失，评分依赖启发式子串匹配 |

**架构债务的精确诊断**：这不是「200 行死代码」级别的债务，而是**每一处连通缺失都表现为一个「假阴性缺口」**——当前因为只支持一个厂商（Claude）、一种策略格式（纯字符串）、一种解析模式（末行匹配），所以所有旁路都能工作。但当第二个厂商或第二种策略格式加入时，每个旁路通道都会变成一个静默退化点。

### 1.3 关键设计决策评估

**决策一：Workflow YAML 作为声明式输入（✓ 正确）**

将 `mode_gating:`、`cli:` 等策略声明放在 YAML 中而非 Go 代码中，是正确决策。它为策略声明提供了版本控制、代码审查、非开发者可编辑三个关键属性。

**决策二：`isClaude` 的 `strings.Contains` 匹配（✗ 脆弱）**

```go
isClaude := strings.Contains(o.agentCmd, "claude")  // 引擎盖下的「隐式兼容性契约」
```

这个决策在只有 Claude 时看起来很聪明（自动兼容 wrapper），但它建立了一个**不可显式声明的契约**。任何名字含 "claude" 的二进制都会无差别获得所有 Claude-specific 行为。这本质上是**用实现细节做接口设计**。

**决策三：fallback 链的隐式信号降级（✗ 危险）**

```
parseReviewerVerdict (fail) → parseExecutiveVerdict (fail) → parseConfidenceScore (maybe match)
```

当一个解析器失败后静默 fallback 到下一个，且 fallback 路径上的匹配不验证「输入类型」——这是一个**类型不安全的状态机**。调用方无法区分「A 成功匹配」和「C 错误匹配」。

**决策四：Compact 的定时触发而非事件驱动（△ 可接受）**

`compactMemoryIfDue` 每 10 次迭代触发一次，而 `forge run` 完全不触发。这在当前规模下是可接受的，但它意味着记忆压缩对非 evolve 工作流是静默缺失的。这不是架构错误，是**触发策略过于保守**。

### 1.4 架构债务的量化评估

| 债务类别 | 严重程度 | 修复成本 | 退化风险 |
|---------|---------|---------|---------|
| 厂商耦合（6 个 Claude-specific 点） | 高 | 3-5 sprint | 高（静默退化） |
| 策略解析旁路 | 中 | 0.5-1 sprint | 低（无新策略维度时不影响） |
| 记忆管理触发策略 | 中 | 1 sprint | 低 |
| 特征提取管道缺失 | 高 | 2-3 sprint（v1） | 中 |
| 输出解析器耦合 | 中 | 附入厂商抽象 | 中 |

---

## 2. 高价值扩展方向

### 方向 A：厂商无关的 CLI 抽象层（P0）

**业务价值**：
- 打开多厂商支持（Gemini、OpenAI、自研模型）的商业路径
- 消除 6 个 Claude-specific 分支的累积耦合
- 使厂商切换可测试、可逆、可版本化

**技术价值**：
- 将 `isClaude: bool` 升级为 `vendorID: string` 即可获得可观测性
- `AgentCLI` 接口定义后，每个厂商的实现验证会自然暴露契约中的模糊点
- 为结构化输出契约（方向五）提供宿主

**核心挑战**：

```
            ┌──────────────────────┐
            │   AgentCLI Interface │
            ├──────────────────────┤
            │ + ArgvBuilder        │
            │ + CostParser         │
            │ + OutputParser       │  ← 版本化输出信封
            │ + TierMapper         │
            └──────┬───────┬───────┘
                   │       │
        ┌──────────┘       └──────────┐
        ▼                             ▼
┌─────────────────┐         ┌─────────────────┐
│  ClaudeAdapter  │         │  EchoAdapter     │
├─────────────────┤         ├─────────────────┤
│ isClaude=true   │         │ isClaude=false   │
│ vendor="claude" │         │ vendor="echo"    │
│ tier: opus=1    │         │ tier: echo=1     │
└─────────────────┘         └─────────────────┘
```

**真正的技术难点不是接口设计**（加一个 interface 不复杂），而是**确定契约的完备性边界**——即「够一个商业 CLI 用的最小方法集」。过度设计：接口膨胀到 10+ 个方法但只有 Claude 一个实现者。欠设计：接口够小但第二个厂商发现缺方法。

**决策选项**：

| 选项 | 策略 | 风险 |
|------|------|------|
| A1. 先提取后验证 | 从当前代码中提取 6 个方法，然后实现 echo/noop 验证 | 方法集可能不够完备 |
| A2. 先建桩后提取 | 先设计完整接口（含输出信封、版本协商），再适配现有代码 | 可能过度抽象 |
| **推荐 A3. 渐进提取+桩验证** | 第 1 步：`isClaude` → `vendorID`；第 2 步：提取 `ArgvBuilder` + `CostParser`；第 3 步：用 echo 验证 | 每一步可逆、可验收 |

**架构变更范围**：
- 新增：`internal/vendor/` 包，含 `AgentCLI` 接口及厂商标识注册表
- 修改：`engine_build.go`、`cost.go`、`cash_register.go` 中的分支代码移至适配器
- 删除：`isClaude` bool 及其 `strings.Contains` 隐式契约

**对现有系统的影响**：低（行为不改，代码位置变）

---

### 方向 B：声明式策略解析引擎（P1）

**业务价值**：
- 使 workflow YAML 中的 policy 引用成为「一等公民」，而非 Go 代码的注释
- 降低新增策略维度的门槛（从「改 Go 代码」到「写 YAML 引用」）
- 消除 `requiredWhenKey()` 的 `strings.LastIndex` 模式

**技术价值**：
- 关闭 yamlpath 与 asset 之间「各自解析 YAML」的重复
- 为策略继承、策略组合、策略版本化提供基础

**核心挑战**：

当前 yamlpath 的 `Resolve()` 走 `python3 yaml2json.py` shim，而 `internal/asset` 的 YAML→JSON 在 Python 侧已完成。两个系统用不同方式解析同一份 YAML——**这是同一种资源的两条不一致的解析路径**。

```
          ┌─────────── YAML 文件 ───────────┐
          │                                  │
          ▼                                  ▼
   internal/asset                    yamlpath.Resolve
   (Python yaml2json.py)            (Python yaml2json.py)
          │                                  │
          │  JSON (结构化)                    │  多次 fork python3
          ▼                                  ▼
    Go 代码使用                        Go 代码使用
    (O(1) 解析)                         (每次调用 fork)
```

**架构决策**：yamlpath 需要一个 Go native YAML 1.1 解析器。但当前的 Go YAML 生态（`gopkg.in/yaml.v3`）不支持 1.1 的所有特性。为此引入一个新的 YAML 库（如 `github.com/goccy/go-yaml`）会增加外部依赖——这与「forge-core 零外部依赖」的红线冲突。

**解决方案选项**：

| 选项 | 方案 | 权衡 |
|------|------|------|
| B1. Go native 解析器 | 在 `internal/yamlpath` 中自建 YAML 1.1 subset 解析器 | 无外部依赖，但实现成本 1-2 sprint |
| B2. 复用 Python shim + cache | 为 `Resolve()` 加内存 cache + 惰性加载 | 保留 Python 依赖，性能改善但不消除 |
| **推荐 B3. 混合策略** | asset 加载时预解析 policy refs 写入 cache；yamlpath 先查 cache，miss 才走 python | 零性能退化，渐进迁移 |

**对现有系统的影响**：中（需要修改 `asset.go` 的 `LoadWorkflowJSON` 加入 post-processing）

---

### 方向 C：记忆生命周期的声明式管理（P1）

**业务价值**：
- 支持长时间运行的 agent 任务（>24h）而不丢失上下文
- 提供用户可控制的内存保留策略（TTL、归档、优先级）
- 使非 evolve 模式的工作流也受益于记忆压缩

**技术价值**：
- `Entry.TTL` 字段使生命周期声明式化
- 惰性过期策略使 Compact 从定时触发变为 on-read 触发
- 归档路径使 Compact 从「不可逆删除」变为「可追溯移动」

**核心挑战**：

当前 `Compact()` 是 age-aware + kind-aware 的，但它缺少一个关键属性：**可逆性**。`Compact()` 的 summarization 本质上是不可逆的有损压缩。对于非关键 entry 这没问题，但对于有取证需求（audit trail）的 entry，需要归档机制。

```
当前：Entry → Compact (有损) → 丢弃
期望：Entry → Compact (有损) → 衰减 TTL → Archive (无损) → 可选恢复
```

**架构决策**：

引入归档（`.forge/archive/`）需要解决：
1. **所有权**：归档文件的命名空间与当前会话 ID 的映射关系
2. **查询穿透**：`Query()` 是否需要查归档？如果需要，要定义分层查询策略
3. **GC**：归档文件的 TTL 策略（保留 30 天后自动清理？）

**决策选项**：

| 选项 | 归档策略 | 查询策略 | 成本 |
|------|---------|---------|------|
| C1. 无归档（仅 TTL） | 不可逆 compact 加 TTL 前 eviction | 不查归档 | 0.5 sprint |
| **推荐 C2. 惰性归档** | Compact 时移动 age>72h 的 entry 到归档 | 只在显式 `--include-archive` 时穿透 | 1 sprint |
| C3. 全归档 | 所有 compact 的 entry 都归档 | 归档支持带衰减权重的查询 | 2 sprint |

**推荐 C2**，因为 C1 仍然有「Compact 丢失追溯能力」的问题，C3 在当前 32-entry cap 下过于重量级。

**对现有系统的影响**：低（`Entry` 结构增加字段，`Load()` 增加惰性过期路径）

---

### 方向 D：自动路由的特征管道架构（P2）

**业务价值**：
- 使 `phaseTierResolver` 从「启发式子串匹配」进化到「多维特征评分」
- 支持根据变更复杂度、影响范围、上下文大小自动选择模型
- 降低「选错模型导致成本飙升」的用户错误

**技术价值**：
- 定义特征提取管道（pipeline）架构，使特征可组合、可替换
- 为 routing 引入置信度仲裁策略，而非简单加权求和

**核心挑战**：

当前 `routing.Score` 的加权求和模型有一个隐式假设：**所有特征维度是独立的**。但实际上它们高度相关——一个包含大量文件修改的 diff 通常也意味着高复杂度。

```
Score = w1*complexity + w2*dependency_change + w3*context_size + w4*business_impact
```

当维度相关时，加权求和会导致**多重共线性**——某些维度被重复计算，某些被低估。更好的模型是**分层仲裁**：

```
                 ┌─ complexity ≥ threshold ──→ 升级
                 │
输入特征 ─→ 并行管道 ── dependency_change ≥ threshold ──→ 升级
                 │
                 └─ 均低于阈值 ──→ 保持或降级
```

**架构设计建议**：

```
┌───────────────────────────────────────────────┐
│                Routing Pipeline                │
├───────────────────────────────────────────────┤
│   ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│   │Complexity│  │  DepChg  │  │ BizImpct │   │  ← 特征提取器接口
│   │  Pipe    │  │  Pipe    │  │  Pipe    │   │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘   │
│        │             │             │          │
│        ▼             ▼             ▼          │
│   ┌─────────────────────────────────────┐    │
│   │        Arbitration Strategy         │    │  ← 仲裁层（非加权求和）
│   │  (e.g., priority-ordered gates)     │    │
│   └─────────────────┬───────────────────┘    │
│                     │                        │
│                     ▼                        │
│            ┌────────────────┐                │
│            │  Tier Decision │                │
│            └────────────────┘                │
└───────────────────────────────────────────────┘
```

**关键设计问题**：

> 当特征提取器置信度低时（如 `business_impact` 管道没有足够数据），系统应该升级（保守安全）还是降级（最大化性能）？

当前 `phaseTierResolver` 的策略是「不降级只升级」——保守安全。但这意味着当某个特征提取器故障时，所有请求都被升级到最昂贵模型。需要引入**置信度加权**：

```
final_score = Σ(wi * si * ci) / Σ(wi * ci)
```
其中 `ci` 是特征 `i` 的置信度。当 `ci < threshold` 时，该特征被排除或降权。

**对现有系统的影响**：中-高（需要新增 pipeline 框架、特征提取器接口、仲裁策略）

---

### 方向 E：输出契约的版本化信封（附入方向 A）

**业务价值**：
- 消除 `lastNonEmptyLine` 的格式漂移风险
- 使 agent 输出契约可版本化、可迁移
- 解决 fallback 链的信号类别混淆

**技术价值**：
- `AgentCLI.OutputParser` 接口自然要求一个版本化的输出格式
- 版本化信封使格式变更变成显式迁移（而非静默退化）

**核心挑战**：

当前的三步 fallback 链（reviewer → executive → confidence）的问题不是「解析逻辑不够强壮」，而是**没有对输入进行分类**。一个包含 `CONFIDENCE: 85` 的讨论文本不应该被误认为一个有效的 confidence 信号。

```
当前：输入文本 → 正则匹配 → 值（无输入分类）
期望：输入文本 → 信封解析 → 类型标签 + 值 → 类型检查 → 消费
```

**架构设计建议**：

```
{
  "version": 1,
  "type": "verdict",           // or "confidence", "analysis"
  "timestamp": "...",
  "payload": {
    "verdict": "APPROVED",
    "reason": "..."
  }
}
```

或者在纯文本格式下，使用显式分隔符和类型前缀：

```
---VENDOR-OUTPUT-V1---
Type: VERDICT
Verdict: APPROVED
---END-VENDOR-OUTPUT---
```

**对现有系统的影响**：低（如附入方向 A，作为 `AgentCLI.OutputParser` 的一部分）

---

## 3. 接口设计建议

### 3.1 核心接口原则

**原则一：显式 > 隐式**

当前架构的最大问题是**隐式契约太多**：
- `isClaude` 是隐式的（`strings.Contains`）
- fallback 链是隐式的（函数调用顺序）
- policy 引用是隐式的（`strings.LastIndex`）

每个隐式契约都应该升级为**一个显式的类型或接口**。

**原则二：验证位置 > 消费位置**

当前在消费位置（`mode_gating.go`）做 policy 解析，正确的位置是在加载位置（`asset.LoadWorkflowJSON`）做一次解析并持久化结果。

**原则三：悲观解析 > 乐观解析**

当前对 agent 输出是乐观的——假设格式正确，匹配失败则 fallback。应该改为：假设格式可能是错误的，匹配失败则报错。只在显式标记的「模糊匹配」场景才 fallback。

### 3.2 引入的新抽象层

**抽象一：`VendorRegistry`**

```go
// 厂商标识注册表——替代 isClaude bool
type VendorRegistry struct {
    vendors map[string]AgentCLI
}

func (r *VendorRegistry) Resolve(agentCmd string) AgentCLI
// 先精确匹配 os.Args[0]，再 fallback 到 Contains 匹配
// 显式注册的 vendor 优先于隐式匹配
```

**设计理由**：保留向后兼容性（`strings.Contains` 作为 fallback），但为精确匹配提供显式路径。

**抽象二：`PolicyResolver`**

```go
// 策略解析器——在 workflow 加载时运行一次
type PolicyResolver struct {
    cache map[string]PolicyRef  // 加载时预填充
}

func (r *PolicyResolver) Resolve(wf *WorkflowJSON) error
// post-processing：找到 yamlpath 引用，预解析并写入 wf
```

**设计理由**：将 policy 解析从「按需 Go 硬编码」改为「加载时预计算」——这是经典的**查询与命令分离**。

**抽象三：`ArbitrationStrategy`**

```go
// 仲裁策略——替代加权求和
type ArbitrationStrategy interface {
    Resolve(features []Feature) TierDecision
}

// 门控策略：高置信度特征优先
type GateStrategy struct {
    gates []Gate  // 按优先级排列
}
```

**设计理由**：加权求和不适用于高相关度特征；门控策略更接近人类的决策模型。

### 3.3 向后兼容性策略

| 变更类型 | 兼容策略 | 
|---------|---------|
| `isClaude bool` → `vendorID string` | 保留 `IsClaude() bool` 兼容方法，但标记为废弃 |
| `requiredWhenKey` → `PolicyResolver` | 先共存两个路径，通过 feature flag 切换 |
| `Compact` 增加 TTL | TTL=0 表示「使用默认策略」，完全向后兼容 |
| `parseReviewerVerdict` fallback 链 | 加一个 `StrictMode()` 选项，默认 false 保持当前行为 |

---

## 4. 技术选型

### 4.1 需要引入的技术

| 需求 | 推荐方案 | 替代方案 | 决策依据 |
|------|---------|---------|---------|
| Go native YAML 1.1 | 自建 subset（`internal/yamlpath/parser.go`） | `gopkg.in/yaml.v3`（不支持 1.1），`github.com/goccy/go-yaml`（外部依赖） | forge-core 零外部依赖红线 |
| 特征提取管道框架 | 自建 pipeline 模式（Go interface 组合） | 引入 workflow 引擎（如 Temporal） | 当前场景不需要分布式编排 |
| Tokenization | `github.com/pkoukk/tiktoken-go`（外部依赖但轻量） | 自建 token 计数器 | 依赖小（~200KB），性能好，非核心运行时路径 |
| 持久化归档 | 文件系统（`.forge/archive/{session-id}/`） | SQLite（K/V 存储）或 BoltDB | 当前规模下文件系统足够 |

### 4.2 第三方依赖评估标准

基于 forge-core 零外部依赖红线，我建议按三级评估：

| 等级 | 允许 | 例子 |
|------|------|------|
| **L0（禁止）** | forge-core 运行时（Go 进程） | `gopkg.in/yaml.v3`, `github.com/goccy/go-yaml` |
| **L1（有条件允许）** | harness 工具（Node/Python）、构建时、CI | `tiktoken-go`（仅用于 token 计数，非核心路径） |
| **L2（允许）** | 测试、开发工具 | `github.com/stretchr/testify`, `github.com/golangci/golangci-lint` |

**自建 vs 采购的决策树**：

```
需要功能
  │
  ├─ 核心运行时（L0） → 自建（无外部依赖红线）
  │
  ├─ 非核心运行时（L1） → 
  │     ├─ 依赖 < 500KB + 无间接依赖 → 可引入
  │     ├─ 依赖 > 500KB + 可自建 < 2 sprint → 自建
  │     └─ 依赖 > 500KB + 自建 > 4 sprint → 引入（但需讨论红线豁免）
  │
  └─ 开发/测试（L2） → 优先引入成熟库
```

### 4.3 关键自建决策

**自建 YAML 1.1 subset 解析器**（1-2 sprint）：

理由：
1. 红线豁免不值得申请——yamlpath 只需要 YAML 1.1 的一个子集（标量 + 映射 + 序列 + anchor/alias）
2. 自建解析器在 500 行以内可以完成（参考 `gopkg.in/yaml.v3` 的核心 parser 约 1500 行，但做 subset 可以大幅简化）
3. 自建解析器可以精确适配 yamlpath 的查询路径语法，复用 asset 的 JSON 输出做集成测试

**不自建 tokenizer**：

理由：
1. tokenization 不是 forge-core 的核心能力
2. `tiktoken-go` 的 API 稳定，200KB 的依赖风险可接受
3. 自建 tokenizer 需要维护 o200k_base/p50k_base 等编码表的更新，成本高

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 量级 | 理由 |
|--------|------|------|------|
| **P0** | 方向 A：厂商抽象 | 3-5 sprint | 关闭最后一道架构屏障；方向 E 附入；方向 D 的前置依赖 |
| **P1** | 方向 B：声明式策略 | 1-2 sprint | 消除 policy 解析旁路；高可见度（dead code 告警归零） |
| **P1** | 方向 C：记忆生命周期 | 1 sprint | 长期运行刚需；技术债务 vs 功能收益平衡 |
| **P2** | 方向 D：自动路由 | 2-4 sprint | 需要方向 A 完成；特征基础设施投资大 |
| **P3** | 方向 E：结构化输出 | 0（附入 A） | 作为 A 的实现细节，不独立排期 |

### 5.2 阶段划分

```
季度 1（Sprint N ~ N+5）
┌─────────────────────────────────────────────────┐
│ 阶段 1：厂商抽象（P0）                         │
│ Sprint N:   vendorID + 可观测性                │
│ Sprint N+1~N+2: AgentCLI 接口 + ClaudeAdapter  │
│ Sprint N+3: EchoAdapter 验证 + 结构化输出附入  │
│ Sprint N+4~N+5: 回退测试 + 边界修复             │
│ 里程碑：Echo CLI 在 6 个阶段全部走通             │
└─────────────────────────────────────────────────┘

季度 2（Sprint N+6 ~ N+11）
┌─────────────────────────────────────────────────┐
│ 阶段 2：策略引擎 + 记忆管理                    │
│ Sprint N+6~N+7: 方向 B（yamlpath Go native）    │
│ Sprint N+8: 方向 C（TTL + 惰性过期）            │
│ Sprint N+9~N+10: 方向 D v1（complexity + dep） │
│ Sprint N+11: 集成测试 + 性能回归                 │
│ 里程碑：workflow YAML 支持声明式策略引用         │
└─────────────────────────────────────────────────┘
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| **AgentCLI 接口定义循环** | 中 | 高 | 强制 3 个 sprint 的接口稳定期，不做 precooking |
| **自建 YAML parser 遇到 YAML 1.1 边界** | 中 | 中 | 资产加载路径已有 JSON 输出作为 fallback；parser 可渐进替换 |
| **归档导致磁盘增长失控** | 低 | 中 | 显式限制：归档保留 30 天 + 总大小 100MB ceiling |
| **自动路由仲裁策略导致模型选择退化** | 中 | 高 | 默认行为不变（保守升级），仲裁策略通过 feature flag 上线 |
| **方向 A 完成后 Claude 行为偏移** | 中 | 中 | 全量 E2E 测试套件 + diff 比较（新旧路径输出一致） |

### 5.4 关键决策点的时间线

```
决策点 1（Sprint N 截止）：AgentCLI 接口包含哪些方法？
- 候选：ArgvBuilder, CostParser, OutputParser, TierMapper
- 决策依据：echo adapter 是否能全部实现？

决策点 2（Sprint N+2 截止）：OutputParser 信封格式
- 选项：JSON vs 纯文本分隔符 vs 两者都支持
- 决策依据：Claude 的 JSON 信封能否无损映射到新格式？

决策点 3（Sprint N+6 截止）：自建 YAML parser vs 引入外部库
- 选项：自建 subset vs goccy/go-yaml vs 保持 Python shim
- 决策依据：自建 1.5 sprint 能否完成？goccy/go-yaml 的间接依赖树多大？

决策点 4（Sprint N+9 截止）：自动路由 v1 范围
- 选项：只做 complexity vs complexity+dependency
- 决策依据：dependency 管道的 Git diff 数据源是否稳定？
```

---

## 总结

这份分析文档揭示的最有价值的发现不是某个具体的技术债务，而是 **「存在但未连通」**（present-but-disconnected）这个可复用的诊断 lens。ForgeOS 的基础设施层大量就位，但连通它们的「最后一公里」表现出四个系统性特征：

| 特征 | 表现 | 修复模式 |
|------|------|---------|
| 声明层旁路 | YAML 声明有值，但 Go 代码不消费它 | 消费位置移到加载位置 |
| 隐式契约 | `Contains`/`LastIndex` 替代显式注册 | 升级为显式类型注册 |
| 静态 fallback 链 | 硬编码的函数顺序替代仲裁策略 | 引入可配置的仲裁器 |
| 触发时机缺失 | 定时触发但事件驱动场景漏接 | 加上惰性/on-read 触发 |

如果方向 A（厂商抽象）在下一个 quarter 完成，它消除的不只是 6 个 Claude-specific 分支——它关闭了 ForgeOS 从「Claude wrapper」到「通用编排引擎」的**最后一道架构屏障**。
