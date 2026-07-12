现在我已掌握足够信息。以下是我的架构分析。

---

# 架构师分析：ForgeOS 元治理与结构盲区

## 基本信息

- **项目**: ForgeOS — AI 原生软件工厂治理平台
- **分析依据**: `docs/requirements/forgotten-five-meta-governance-and-blindspots.md` + 实测验证报告 + 全仓代码/文档结构
- **总规模**: forge-core 63 个生产 Go 文件 (14,670 LOC) · harness 35 文件 (1,368 LOC) · **docs/ 739 个 .md 文件 (224,361 行)**
- **当前状态**: Sprint 31 已完成，v2 持续迭代中

---

## 1. 架构评估

### 优势

**1.1 治理优先的架构哲学 —— 行业领先**

ForgeOS 最独特的架构优势不是任何单一引擎，而是**治理优先**的整体方法：

- 所有闸门是 `host-independent` 的（不依赖宿主 CLI 能力），通过 `acceptance.mjs` 聚合 8 项检查
- 每项检查诚实标 N/A，绝不伪造通过（这是其他项目几乎从不做的）
- `fresh-context reviewer` 独立审查的纪律强制执行，没有"实现者自审"
- 多次"架构自纠"记录在案——当闸门发现自身违规时主动重构而非凑合

**1.2 分层降级的依赖管理**

从 Python -> Go 的 YAML 解析迁移展示了正确的模式：先用脚手架（Python shim）、再逐步用原生实现替换、始终保持向后兼容。同样的模式可复用到其他领域。

**1.3 可观察性闭环完整**

Sprint 24-26 用真 Claude 跑通了完整的 Learning loop 三维数据（quality + latency + cost），在基础设施层完全架通了 data pipeline。这是其他 AI 编排框架几乎没有做到的。

**1.4 工程纪律内化在代码中**

`gate.mjs` · `arch-check.mjs` · `check.py` · `secret-scan.mjs` 形成了一个从体积 → 架构 → 治理完整性 → 安全的层层递进的执法体系，且每层有独立验证。

### 局限性

**1.1 核心支柱的静默盲区（方向一）—— 最严重的架构债务**

`arch-check.mjs` 的 `checkLayering` 对所有 forge-core 文件返回 `null` 层，全部跳过。这不是一个 bug——**代码中有注释明确说明这是设计选择**：

```
// unmapped files get layer null (excluded from
// layering checks — e.g. forge-core's internal/<pkg> dirs are not layered).
```

后果是：
- ForgeOS 引以为豪的「依赖方向单向向内」红线对其自身代码**从未生效**
- `internal/routing` 可以依赖 `internal/orchestrator`（违背架构图）而不触发任何告警
- `[PASS] layering` 不等于架构干净——等于零检查被执行
- 这破坏了整个治理体系的**可信度基础**：如果执法器对自己都不执法，它对其他项目的执法声明永远带着星号

**严重性评估**: 这是**结构性债务，不是 bug**。修复本身成本极低（几行配置），但暴露的问题可能很大——打开 layering 后可能会发现大量现有违规。需要过渡期（先 warn 再 block）。

**1.2 零依赖承诺的运行时裂缝（方向四）**

`BOOTSTRAP.md` 和 `go.mod` 声明「纯 Go 标准库，零外部依赖」，但 `loadWorkflow` 在 Go 解析器失败时 fallback 到 `python3 harness/yaml2json.py`。如果 Python 不在 PATH 中，所有 `forge run`/`forge evolve` 失效。

这不是一个边缘场景——`yaml2json.py` 还被用作仓库识别标记（`main_agent_test.go:33`），可见它已被视为基础设施的一部分而非临时脚手架。

**严重性评估**: 中等。Go 解析器已实现（~700 行，7 个文件），对 7 个真实 workflow 文件的解析已与 PyYAML 逐位吻合。但**没有独立的 YAML 合规测试套件**——下次遇到不在 7 个文件中的 YAML 特性可能再次静默失败。这更像是一个**测试缺口**而非代码缺口。

**1.3 文档膨胀失控（方向三）—— 被严重低估的问题**

原文估算 docs/ 有 127 份文件、~60,000 行。实测：

| 指标 | 原文声称 | 实际 | 偏差 |
|------|---------|------|------|
| 总数 | 127 文件 | **739 文件** | 5.8x |
| 总行数 | ~60,000 | **224,361** | 3.7x |
| docs/requirements/ | 43 文件 | **403 文件** | 9.4x |

需要特别关注的新发现：
- `docs/results/` 有 **245 个文件**——原文完全没提到这个目录
- `*five*` 模式格式的文件有 **499 个**（占了 docs/ 的 67%）
- 带版本后缀的文件 43 个（`*v2*`/`*v3*`/`*v35*`）
- **零 archive 目录**——没有任何文档被归档过

**讽刺的是**：本文分析所依据的文档本身现在是 739 份文档之一——它对这个问题的贡献和它试图解决的问题是同一件事。这不是对文档质量的批评，而是对**缺少文档生命周期管理**的观察。

**严重性评估**: 高。文档膨胀直接影响新 agent 上手的认知负载。全仓最大的目录（docs/，224K 行）完全不受任何治理约束——`gate.mjs` 不检查 `.md`，`secret-scan.mjs` 跳过文档（但文档最可能有硬编码 URL/API key/端点示例），`arch-check.mjs` 只扫源文件。这违背了 ForgeOS「先治理自己」的核心承诺。

**1.4 人机协作接口的原始性（方向二）**

当前的 `Signals.HumanApproved bool` 是唯一的人机信号。在长达数小时的自治运行中，人类观察 agent 走向错误方向时**没有任何轻量级的干预工具**——只能 Ctrl-C 杀死进程。

这在 ForgeOS 的核心使用场景（24h 无人值守自治）中是一个真实痛点。真实的人机协作是**连续反馈**而非离散的 PASS/FAIL 事件。

**严重性评估**: 中等（P1 偏高）。对自治运行的质量控制影响大，但项目当前的用户群是 AI agent 自身（dogfooding），在用户扩展前可以接受当前状态。

**1.5 自我治理的完整性缺口（方向五）**

> 如果有任何领域 ForgeOS 的「先治理自己」承诺不成立，那这个承诺在任何地方都是打折扣的。

docs/ 是全仓最大的未治理领地。这本身就是一个治理缺口。更严重的是：方向一（arch-check 盲区）和方向五（dogfood 缺口）共同指向同一个根本问题——**ForgeOS 的治理体系对其自身的覆盖不完整**。

这与 ForgeOS 的核心价值主张（"治理系统先治理自己"）直接冲突。

---

## 2. 扩展方向

基于上述评估，我提出 5 个高价值的架构扩展方向。按优先级排序。

### 方向 A（P0）：自身治理覆盖完整化

**为什么需要**: 这是五个方向收敛的核心。如果 ForgeOS 的治理工具不对自己生效，那它对其他项目声称的「layering passed」永远带着星号。**这是可信度地基的修复**，不是新功能。

**核心挑战**:
- forge-core 的内包结构（`internal/<pkg>`）与现有 Clean Architecture 的四层模型（domain/application/interfaces/infrastructure）不完全匹配。强制映射可能扭曲架构，也可能暴露大量现有违规
- 需要先在 warn 模式下运行一个周期来收集违规基线
- 真正的依赖图可能与 `rules.yaml` 的 `forbidden` 规则冲突——冲突点可能揭示规则的合理性而非代码的问题

**预期的架构变更**:
1. `.arch/rules.yaml` 新增 forge-core 包层映射（如 `internal/asset` → `domain`, `internal/orchestrator` → `application`, `internal/risk` → `application`, `cmd/forge` → `interfaces`）
2. `forbidden` 规则扩展为覆盖 forge-core 实际依赖结构的规则
3. `checkLayering` 新增 `layering_coverage >= 80%` 阈值防止新增包再次滑出
4. 新增 `harness/arch/rules_forgecore.yaml`（可选）让 forge-core 自身使用与对外项目不同的层模型

**对现有系统的影响**:
- 短期：CI 可能变红（现有违规被暴露）
- 中期：需要 1-2 次重构来修复暴露的依赖方向违规
- 长期：治理可信度大幅提升

**选项与权衡**:

| 选项 | 工作量 | 风险 | 优势 |
|------|-------|------|------|
| A1 直接在现有四层模型内映射 | ~1 sprint | 四层模型可能不匹配 | 保持统一，不引入特殊规则 |
| A2 为 forge-core 建立独立自描述层 | ~1.5 sprints | 两套规则增加认知负载 | 更准确反映实际架构 |
| A3 使用实际 import 依赖图自动推导层 | ~2-3 sprints | 可能产生不可读的自动生成规则 | 最精确 |

**建议**: 走 A1 路径，但将 `forbidden` 规则改为基于实际 import 图的**白名单许可模式**（只声明哪些跨层依赖是被允许的，其余自动禁止）而非当前的黑名单模式。这更接近微服务架构中的依赖审计。

### 方向 B（P0）：YAML 运行时依赖彻底消除

**为什么需要**: "零外部依赖"承诺与运行时 Python 需求之间的裂缝在架构层面不可接受。如果需要在干净容器中运行 forge-core，当前必须安装 Python 3——这与 `go.mod` 的空 `require` 块矛盾。

**核心挑战**:
- Go 标准库没有 YAML 解析器——必须自维护解析器
- 自解析器的 YAML 官方合规测试套件覆盖不足（当前只针对 7 个 forge-core 自己的 workflow 文件测试）
- YAML 1.2 规范庞大——需要严格界定 forge-core 需要的 YAML 子集
- `yaml2json.py` 被用作仓库识别标记——需要替换为显式标记文件

**预期的架构变更**:
1. `internal/yaml2json` 补全 YAML 官方合规测试套件（yaml-test-suite 的子集）
2. 移除 `loadWorkflow` 中的 Python fallback 路径
3. 将 `yaml2json.py` 从仓库识别标记降级并最终替换为 `.forgeos-root` 标记文件
4. 将 `internal/yaml2json` 包的文档锚定义为「正式解析器」而非「临时替代」

**对现有系统的影响**:
- 影响 `main.go:374-383` 的 `loadWorkflow` fallback 路径
- 影响 `main_agent_test.go:33` 的仓库检测
- 影响 `forge-init` 复制的模板（不再需要 `yaml2json.py`）

**建议的时间线**:
- Phase 1：Go 解析器 YAML 合规测试套件（~0.5 sprint）
- Phase 2：Python fallback 降级为 `--use-python-shim` flag，默认走 Go 路径（~0.5 sprint）
- Phase 3：移除 Python shim 代码和仓库检测依赖（~0.5 sprint）

### 方向 C（P1）：结构化人机反馈通道

**为什么需要**: 24h 自治运行中的人类监督能力是 ForgeOS 从「AI 实验室工具」进化为「生产治理平台」的关键能力。当前只有二元 Pause/Kill 模型，缺乏结构化反馈。

**核心挑战**:
- 在同步执行模型中植入异步反馈通道——Go 的 `exec.Command` 是阻塞的，需要额外的通信机制
- 反馈的安全性和溯源性——如何防止外部进程注入恶意反馈
- 反馈的幂等消费——如果进程崩溃后重启，未消费的反馈如何处理

**预期的架构变更**:
1. 新增 `.forge/feedback/` 目录作为反馈信号文件约定
2. 新增 `forge feedback` CLI 子命令（Unix socket 或文件写入）
3. `LoopEngine.Run` 在每个 phase 前检查外部反馈队列并注入 prompt
4. 反馈文件格式：`{ "action": "redo|inject|skip|pause", "target_phase": "...", "note": "..." }`

**设计选项**:

| 选项 | 复杂度 | 安全性 | 离线能力 |
|------|--------|--------|---------|
| C1 文件系统轮询（`.forge/feedback/*.json`） | 低 | 中（文件权限） | 强（持久化） |
| C2 Unix socket（`forge feedback` 直连） | 中 | 高（Unix 权限） | 弱（需进程存活） |
| C3 信号文件 + socket 双通道 | 高 | 高 | 强 |

**建议**: 先走 C1（文件轮询）——与 Go 运行时零依赖冲突最小，机制直观，跨平台。C2 作为后续增强。C1 不需要新的 goroutine 模式——`LoopEngine` 的 phase 驱动的执行模型天然在每个 phase 边界有检查点。

### 方向 D（P1）：文档注册表与生命周期治理

**为什么需要**: docs/ 有 739 个文件、224K 行——比 forge-core 和 harness 的总和还要大一个数量级。没有注册表、没有过期机制、没有版本关系、没有去重。目前**任何新 agent 都无法合理阅读全部文档**，选择性阅读必然遗漏关键上下文。

**核心挑战**:
- 已有 739 个文档的回填工作量大——近程不可能全部加 metadata
- 文档之间的关系是图结构（supersedes/superseded-by/related-to），需要适合的存储格式
- 去重工具的精度需要校准——过松错过真实重复，过紧产生误报噪音

**建议的架构**:

```
docs/
├── INDEX.md                  # 活跃文档注册表（machine-readable YAML frontmatter）
├── archive/                  # 归档文档
├── adr/                      # 架构决策记录（已受 ADR 流程约束）
├── analysis/                 # 分析文档
├── requirements/             # 扩展方向提案
└── results/                  # 实验/运行结果
```

每篇文档的 YAML frontmatter：

```yaml
---
id: forg-031
title: ...
status: draft | active | superseded | implemented | retired  # 强制字段
supersedes: [forg-030]                                       # 可选
superseded_by: []                                            # 可选
created: 2026-07-12
expires: 2026-10-12   # 可选，过期自动告警
tags: [governance, introspection]
---
```

**对现有系统的影响**:
- `check.py` 新增 `check_doc_index` 检查（所有 docs/ 下的 .md 文件必须在 INDEX.md 中有注册或位于 archive/）
- `gate.mjs` 扩展为检查 .md 文件（至少行数检查）
- 新增 `forge doc-dedup` 工具（可选）
- 新增 `forge doc-status --stale` 列出过期文档

**不做什么**:
- 不去合并已有 739 个文档（那是独立的工程任务）
- 不强制文档行数上限（防止阻碍详细技术文档）
- 不要求所有文档立即加 frontmatter——设置 30 天过渡期

### 方向 E（P2）：docs/ 纳入安全扫描覆盖

**为什么需要**: 当前 `secret-scan.mjs` 只扫描被 `gate.mjs` 遍历的文件，而 `.md` 被完全排除。但文档是最可能含有硬编码 URL、API key、端点示例、临时凭据的目录——这是**安全盲区**。

**核心挑战**:
- 文档中的"凭据"通常是示例/教学用假值，误报率可能高
- 需要区分「教学用 `API_KEY=xxxx`」和真实泄露
- 扫描性能——739 个文档的扫描对 CI 的增量

**建议的架构变更**:
- `gate.mjs` 文件遍历扩展为可选包含 `.md`（通过 `--scan-docs` flag）
- `secret-scan.mjs` 的条件模式：对 `.md` 文件使用宽松规则集（允许特定模式的假值）
- 不在主 CI 路径上强制，作为 `forge security-scan` 的一部分

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

ForgeOS 的模块间接口应该遵循以下原则：

**P1 向后兼容优先**: ForgeOS 的治理体系本身就是 dogfood——任何 `arch-check` 行为变化都可能导致 CI 变红。接口变更必须提供过渡路径（例如新增配置项时，缺省值保持旧行为）。

**P2 显式契约优于隐式约定**: 当前 `product-manager.md` 的 `CONFIDENCE: <0-100>` 和 `cto.md` 的 `VERDICT: APPROVE` 机读契约是好的模式。所有 agent→engine 通信都应该最后落在一个可测试的机读契约上，而不是仅靠 prompt 文本。

**P3 检查器对自身代码可见（Self-Introspection）**: 每个治理检查器（arch-check、gate.mjs、check.py）应该能报告「我检查了多少文件」「我被跳过了多少文件」。这防止「连自己都不检查自己」的盲区。例如 `checkLayering` 当前返回 `[PASS]` 但零文件被检查——更好的接口是返回 `layering: 0 files checked, 25 files skipped (no layer)`。

**P4 诚实边界分离**: 以下三类逻辑应保持在不同层：
- **机制就绪层**：数据通路、接口定义、测试覆盖——即使无真 agent/真数据也验证通过
- **数据消费层**：有数据时计算、无数据时诚实省略/降级
- **执行层**：默认 dry-run，需要显式授权才能执行真操作（如花真钱调 API）

### 3.2 是否需要新的抽象层

**是——需要「治理元接口」**

当前没有统一的接口让治理自身报告其覆盖范围：

```go
// 现有代码中不存在——被治理检查器跳过时完全无声
type GovernanceCheck interface {
    Name() string
    Check(model *Model, rules *Rules) (Result, error)
    // 新增：覆盖率信息
    FilesChecked() int
    FilesSkipped() int
    SkipReason() string  // "unmapped layer", "test file", etc.
}
```

这不要求现有的 Node 检查器用 Go 重写——只需要每个检查器在输出中增加一行类似于 `layering: PASS (3 files checked, 25 skipped - no layer mapping)` 的信息，被 `acceptance.mjs` 聚合时一并显示。

**设计决策**: 选择嵌入式的行内格式（每检查器自己报告覆盖率） vs 集中式的覆盖率注册表

| 方案 | 复杂性 | 诚实度 | 推荐 |
|------|--------|--------|------|
| 每检查器行内自报告 | 低，每检查器加一行 | 中，可能遗漏 | ✅ **首选** |
| 集中式 GovernanceCoverage 注册表 | 高，需所有检查器注册 | 高 | v3 目标 |
| 不报告 | 零 | 低 | ← 当前状态 |

### 3.3 向后兼容性策略

**层映射变更**（方向 A——最可能破坏现有 CI）：
- Transition 模式：新增 `arch-check --warn-layers` flag，将层违规报告为 warning 而非 error，exit 0
- 持续时间：至少 1 个完整 sprint 周期（~2 周）
- 升级为 error：收集 baseline 后，在新版本 release notes 中声明

**文档注册表**（方向 D）：
- Old: 文档不在 INDEX.md 中 -> 无影响
- New: 文档不在 INDEX.md 中 -> `check.py` 报 warning
- 30 天后：升为 error
- 已有 739 个文件不需要全部回填——`status: uncataloged` 作为默认状态即可让它们被注册而不被强制分类

**反馈通道**（方向 C）：
- 完全向后兼容——当前命令无反馈时行为不变
- `.forge/feedback/` 目录不存在 -> 反馈消费跳过
- `forge feedback` 命令不改变任何已有执行路径

---

## 4. 技术选型

### 4.1 需要引入的新技术栈

| 技术 | 方向 | 评估 |
|------|------|------|
| YAML 合规测试套件（yaml-test-suite） | B | **立即需要**。不是框架，是测试数据。占用 ~200KB 的 YAML 测试文件子集，没有 Go 依赖冲突 |
| Go 原生 YAML 实现（自维护） | B | 已经存在（`internal/yaml2json`），需要的是测试覆盖率提升而非新实现 |
| Unix socket（`forge feedback`） | C | Go 标准库 `net` 包支持，零外部依赖 |
| TF-IDF 去重（文档相似度检测） | D | 可选。可以用 Python 脚本（已有 Python 依赖）或纯 Node 实现（`harness/` 的 Node 栈已有）。**不推荐引入新的嵌入模型/RAG 系统**——对于文件名级别和章节标题级别的相似度检测，TF-IDF 足够，不需要 embedding |

**总体结论**: 不需要引入新的运行时依赖。5 个方向都可以在当前技术栈（Go + Node + Python）内完成。

### 4.2 第三方依赖评估标准

ForgeOS 的「零外部依赖」原则是架构红线，不应妥协。评估任何第三方依赖的标准：

1. **最低权限**: 能否用 Go 标准库 / Node 内建模块实现？→ 能则自建
2. **安全面**: 依赖是否引入新的攻击面（网络、进程生成、文件系统写权限）？
3. **替换成本**: 如果依赖在未来被放弃或出现安全漏洞，替换成本多大？
4. **版本兼容**: 是否与 Go 1.x（for core）和 Node 22+（for harness）兼容？

**当前唯一合理的第三方依赖引入场景**: YAML 合规测试数据（yaml-test-suite），它只是测试时的输入数据，不是运行时依赖。

### 4.3 自建 vs 采购

本分析涉及的所有方向都是自建路径。原因是：

- 项目定位是**治理平台本身**——引入外部治理工具（如文档管理系统、CI 依赖扫描平台）来治理自己，违背 dogfood 哲学
- 外部工具不在 forge-core 的「零依赖」承诺范围内
- 这些方向的特异性高——为通用场景设计的工具通常无法理解 forge-core 的项目特定层模型

---

## 5. 实施路线图

### 优先级排序

| 方向 | 优先级 | 工作量 | 杠杆系数 | 前置依赖 |
|------|--------|--------|---------|---------|
| A - 自身治理完整化 | **P0** | ~1 sprint | ⭐⭐⭐⭐⭐ | 无 |
| B - YAML 依赖消除 | **P0** | ~1.5 sprints | ⭐⭐⭐⭐ | 无 |
| C - 人机反馈通道 | **P1** | ~2 sprints | ⭐⭐⭐⭐ | 无 |
| D - 文档注册表 | **P1** | ~1 sprint (初始化) + 持续 | ⭐⭐⭐⭐⭐ | 是 E 的先决条件 |
| E - 文档安全扫描 | **P2** | ~0.5 sprint | ⭐⭐⭐ | D 完成后 |

### 阶段划分

#### 阶段 1 — 地基修复（P0，Sprint 32-33）

**目标**: 消除「治理自己不治理」的核心可信度裂缝

**工作项**:
1. **方向 A 第一步**: forge-core 包到层的映射定义 → warn 模式运行 → 收集基线违规
   - 定义 `internal/*` 包到 domain/application/interfaces 的映射
   - `arch-check` 在 warn 模式下输出违规清单但不强制阻断
   - 输出「forge-core 层映射合规报告」
2. **方向 B 第一步**: YAML 合规测试套件引入 → 验证 Go 解析器覆盖
   - 从 yaml-test-suite 提取 forge-core 需要的子集（无 anchor/alias/tag 等）
   - 补充 `internal/yaml2json` 的测试，覆盖所有边缘 case
3. **方向 D 第一步**: 初始化 INDEX.md → 设置 `check_doc_index`
   - 创建 `docs/INDEX.md` 骨架（含 YAML frontmatter 模板）
   - 实现 `check.py` 的 `check_doc_index`（验证新文档已注册）
   - 对已有 739 文档设置 30 天过渡期

**里程碑**: forge-core layering 在 warn 模式下产生首份合规报告；所有 YAML 解析路径的 Go 实现通过 yaml-test-suite 子集；新文档必须注册的规则生效。

#### 阶段 2 — 过渡与并行修复（Sprint 34-35）

**目标**: 方向 A/B 从 warn 升级为 block；方向 C 开始

**工作项**:
1. **方向 A 第二步**: 修复暴露的层违规 → 配置硬阻断
   - 逐个修复方向暴露的依赖方向违规
   - 将 arch-check 从 `--warn-layers` 切换为硬阻断模式
   - 新增 `layering_coverage >= 80%` 阈值
2. **方向 B 第二步**: 移除 Python fallback → 替换仓库检测标记
   - `loadWorkflow` 移除外层 fallback，只走 Go 解析器
   - `main_agent_test.go` 改用 `.forgeos-root` 标记文件
   - 添加 `--use-python-shim` 降级 flag 作为逃生舱（默认关闭）
3. **方向 C 第一步**: `.forge/feedback/` 文件轮询 → `LoopEngine` 接入
   - 实现 `.forge/feedback/*.json` 的读取和消费
   - `LoopEngine.Run` 增加 phase 前反馈检查点
   - 实现 `forge feedback --inject/--skip-phase/--redo-phase`（文件写入模式）

**里程碑**: forge-core 自身通过 layering 检查（硬阻断）；Python 不再是 forge-core 的运行时依赖；人类可以在自治运行中注入结构化反馈。

#### 阶段 3 — 完整化（Sprint 36+）

**目标**: 方向 D/E 完整化；方向 C 扩展

**工作项**:
1. **方向 D 第二步**: 去重标记 → 文档过期机制
   - 实现 `forge doc-dedup`（可选工具）
   - 设置文档过期自动告警（60 天未更新的 `draft` 或 `active` 文档告警）
   - 移动过时文档到 `archive/`
2. **方向 E**: docs/ 纳入 secret-scan
   - 扩展 `secret-scan.mjs` 条件覆盖 `.md` 文件
   - 定义文档安全扫描的宽松规则集
3. **方向 C 第二步**: Unix socket 模式（可选增强）
   - `forge run` 过程中监听 Unix socket
   - 支持瞬时反馈（无需写文件再轮询）
4. **方向 D 第三步（可选）**: 文档引用完整性检查
   - `harness/doc-ref-check.mjs` 验证 docs/ 内的相对链接指向存在文件
   - 不是强制阻断，作为 `forge check --check-doc-refs` 可选参数

**里程碑**: ForgeOS 对其自身的治理完整覆盖——docs/ 被纳入治理体系、安全扫描覆盖文档、人类有连续反馈能力。

### 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| **层映射暴露大量违规** | 中 | CI 长时间变红 | 先 warn 模式运行 1 sprint，逐步修复 |
| **YAML 合规测试发现 Go 解析器有未覆盖特性** | 中低 | Phase 2 延迟 | 逐 case 评估，对 forge-core 未使用的 YAML 特性诚实声明确认不覆盖 |
| **方向 C 的文件轮询与执行模型竞争** | 低 | 反馈应用于错误 phase | 反馈文件带 `iteration_phase` 标签，不匹配则排队到下次；单元测试覆盖竞争条件 |
| **docs/ INDEX.md 本身变成另一个需要治理的文件** | 高 | 元治理递归 | `check_doc_index` 自身检查 INDEX.md 的有效性；这是可接受的二阶治理 |
| **过渡期内团队绕过新规则** | 中 | 治理退化 | 过渡期的警告模式用于收集基线而非强制；硬阻断时间表在 Sprint 开始时宣布 |
| **文档去重产生误报** | 中 | 不必要的合并请求 | 去重工具只标记不自动执行；建议清单需要人工确认 |
| **Python shim 移除后用户自定义 YAML 扩展不可解析** | 低 | 兼容性断裂 | 先在 `--use-python-shim` 后运行一个版本周期，收集失败案例 |

---

## 总结：五个方向的收敛关系

```
方向 A（自身治理）─────→  方向 E（文档安全）
    │                           ↑
    │                           │
    ↓                           │
方向 B（YAML 依赖）         方向 D（文档注册表）
    │                           ↑
    │                           │
    ↓                           │
方向 C（人机反馈）───────┘
```

**关键依赖链**:
- **方向 D 是方向 E 的先决条件**——在建立文档注册表之前让安全扫描覆盖 docs/ 会产生大量无法分类的告警
- **方向 A 是最短路径**——方向 A + 方向 B 可以在 2.5 sprints 内解决两个最核心的架构盲区
- **方向 C 是独立路径**——不依赖其他方向，可以随时启动
- **方向 D 需要 30 天过渡期**——因为涉及回填 739 个已有文档的 metadata，不能一蹴而就

**诚实标注**: 以上路线图假设 Sprint 持续时间为标准的 1 周。如果项目有更高优先级的外部任务，方向 C（人机反馈）和方向 E（文档安全扫描）是最容易被推迟的——它们解决的是体验和完整性而非地基。方向 A 和方向 B 不应推迟。
