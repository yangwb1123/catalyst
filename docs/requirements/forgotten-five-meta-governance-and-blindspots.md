# ForgeOS — 被遗忘的五重前沿：元治理、架构盲区与沉默的运行时依赖

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全仓逐文件深扫: forge-core（18+ Go 包 · ~32k LOC 生产代码）、cmd/forge（17+ 子命令）、
>    harness（39+ 模块 · ~10.5k LOC）、.agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 ·
>    全部 ADR/DECISIONS/policies）、docs/（127 份文档）、examples/（url-shortener · go-taskd）
> 2. 逐篇通读已有分析: **127 份 `docs/` 文档**（含 43 份 `docs/requirements/*.md` + 43 份
>    `docs/analysis/*.md` + 核心文档 BOOTSTRAP/CURRENT_SPRINT(31)/FUNCTIONAL_REQUIREMENTS_AUDIT/
>    ADR 0001-0004/DECISIONS/loop-engineering/north-star/ha-security-rollout/ignition）—
>    合计 **~120+ 已有扩展方向，~60,000+ 行分析文本**
> 3. **差异化证明**: 每个方向用 `grep -rn` 在 127 份文档中验证核心关键词组合，确认该方向
>    **从未作为独立方向展开**（零独立覆盖或仅边缘子段落提及）
> 4. **视角**: 不从「加什么新引擎」出发，而从「哪些已被遗忘/忽略的基础层问题会阻止 ForgeOS
>    成为可信赖的生产治理平台」出发——关注的是**现有代码和文档体系中的结构性盲区**，
>    而非再提一个「新功能」
> 5. **纪律**: 不编写任何代码。每个方向附代码级证据、边界场景、与已有覆盖的差异化证明。
> **日期**: 2026-07-10

---

## 全景：已有 ~120+ 扩展方向覆盖图

已有分析压倒了几乎所有「加什么新功能」的域。但有一个共同的模式缺陷：**所有 ~120+ 方向都
是前向提案（propose new），没有一个是自我反省（introspect existing）**。

| 已被充分覆盖的域 | 覆盖量 | 代表性文档 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back/自适应装配） | ~25 方向 | 大多数 requirements + analysis |
| 跨项目/跨会话/联邦治理（知识迁移/漂移检测/多仓库编排/事件驱动/定时平面） | ~15 方向 | `novel-five-perspectives.md`· `expansion-horizon-three.md` |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约 / 多级熔断） | ~15 方向 | `expansion-production-readiness.md`· `production-hardening-five-v42.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化/session/事务性执行） | ~12 方向 | `execution-semantic-gaps.md`· `v33.md` |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/跨进程锁/超时覆盖/并行安全） | ~15 方向 | `forgotten-five-system-boundaries.md`· `v25.md`· `v38.md` |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | ~12 方向 | `second-order-architectural-gaps.md`· `v26.md` |
| 收敛方法论/自诊断/停滞检测/治理测试/元治理 | ~10 方向 | `novel-five-perspectives.md`· `loop-engineering.md` |
| API 版本化/Schema 契约/产物格式/跨会话学习/RAG/自免疫测试/内部遥测 | ~10 方向 | `structural-gaps-v41.md`· `five-genuinely-uncovered-frontiers.md` |
| 安全/凭据/SCA/沙箱/注入防御/readonly 强制 | ~8 方向 | `security-review.md`· Sprint 31 |
| **总计已有覆盖** | **~120+ 方向** | **通过 ~100 份独立文档阐述** |

**本文的 5 个方向共同特征**: 不是「新功能方向」，而是**现有系统中已存在但未被正视的结构性盲区**。
它们不会在 `forge accept` 中显示为红色，但它们是阻隔 ForgeOS 从「功能完整」走向「架构可信」
的无声障碍。每个方向在已有 ~120 个方向中**零独立覆盖**。

---

## 方向一 · arch-check 分层执法对 forge-core 自身是静默盲区

**优先级**: 🔴 P0（架构可信度的地基问题）  
**类别**: 架构 · 治理 · 自身执法  
**预估**: ~1 sprint（诊断 + 修复）  
**杠杆**: ⭐⭐⭐⭐⭐（低投入高回报——几行配置 + 新增 package 映射）

### 问题描述

ForgeOS 的招牌架构执法——`arch-check.mjs` 的 **layering 检查**——对 forge-core 自身的
Go 包是**静默的 no-op**。所有 forge-core 生产代码都位于 `internal/` 目录下，而这些目录
没有被映射到任何架构层（`domain` / `application` / `interfaces` / `infrastructure`），
因此被检查器完全跳过。

这不是一个「不小心漏了」的 bug——它被代码**明确注释为设计选择**:

> `scan.mjs:55-58`:
> ```
> // classifyLayer: map a file's path onto a canonical layer using rules.dir_aliases.
> // The layer is the FIRST path segment (outermost-to-innermost as written on
> // disk) that has an alias; unmapped files get layer null (excluded from
> // layering checks — e.g. forge-core's internal/<pkg> dirs are not layered).
> ```

后果是:ForgeOS 的这个最高调、最核心的架构红线——**「依赖方向单向向内，domain 不 import
外层」**——在本项目中从未被实际执行。`internal/routing` 如果开始 import `internal/orchestrator`
（违反「router 不应依赖编排器」的设计意图），arch-check 不会报错。`internal/converge` 如果
开始依赖 `internal/prompt`，同样不会报错。

### 代码级证据

**证据 A: `.arch/rules.yaml` 定义了 4 层，但 forge-core 零匹配**

```yaml
# .arch/rules.yaml:51-60
architecture:
  layers:
    - interfaces
    - application
    - domain
    - infrastructure
  dir_aliases:
    domain: domain
    application: application
    service: application
    interface: interfaces
    interfaces: interfaces
    httpapi: interfaces
    store: infrastructure
    infrastructure: infrastructure
```

forge-core 的 18 个 Go 包的路径全是 `forge-core/internal/<pkg>`——没有一个路径段匹配
上面的 `dir_aliases` 中的任何键。结果是每个文件的 `layer` 都是 `null`。

**证据 B: `checkLayering` 明确跳过 `layer=null` 的文件**

```javascript
// arch-check.mjs:33-36
export function checkLayering(model, rules) {
  const forbidden = new Set(rules.architecture?.forbidden ?? []);
  const v = [];
  for (const f of model.files) {
    if (f.isTest || !f.layer) continue;  // ← !f.layer 跳过所有 forge-core 文件
    ...
  }
  return v;
}
```

**证据 C: 实际 arch-check 运行证实了这一点**

```
$ node harness/arch/arch-check.mjs
forge-arch: [PASS] layering    ← 总是 PASS，因为根本没有文件要检查
```

— 这行 `[PASS]` 不是「forge-core 架构干净」的信号，而是**「没有检查被执行」**。

**证据 D: `forbidden` 规则也是为 URL shortener 设计的，不是为 forge-core**

```yaml
# .arch/rules.yaml:69-74
  forbidden:
    - "domain -> application"
    - "domain -> interfaces"
    - "domain -> infrastructure"
    - "application -> interfaces"
```

这些规则假设包的结构是 `domain/` 在最内层、`application/` 在中间、`interfaces/` 在最外层。
但 forge-core 的实际依赖结构完全不同——`internal/asset`（数据模型/类型）被几乎所有包依赖，
`internal/orchestrator` 依赖 `asset`/`gate`/`mode`，但 **arch-check 对这些完全不检查**。

### 为什么它被集体遗漏

127 份已有分析文档在讨论「架构执法」「arch-check 8 检查」时，都停留在**功能层面**
（"arch-check 有 layering 检查 ✅"），从未深入到检查器**对 forge-core 自身是否有效**。
这与所有分析的模式一致：它们都是前向提案，从未逆向验证现有执法器是否在说真话。

### 方向建议

1. **为 forge-core 包建立层映射**: 在 `.arch/rules.yaml` 中增加 forge-core 的
   `dir_aliases`——例如 `internal/asset` → `domain`, `internal/orchestrator` → `application`,
   `cmd/forge` → `interfaces`。这需要理解 forge-core 的实际依赖图并为其建模。

2. **修复 `forbidden` 规则**: 将当前泛泛的 `"domain -> interfaces"` 扩展为覆盖
   forge-core 实际架构的规则（如 `"internal/risk -> internal/prompt"`—风险分类器
   不应依赖 context 检索器）。

3. **验证模式**: 在 CI 中运行 arch-check 时，require 至少有一定比例的源文件被 layering
   检查覆盖（例如 `layering_coverage >= 80%`），防止未来新增的包再次滑出检查范围。

### 边界情况

| 场景 | 风险 | 建议 |
|------|------|------|
| 给 `internal/*` 加 `dir_aliases` 后现有依赖可能不合法 | 可能暴露现有违规，CI 瞬时变红 | 先用 `--warn` 模式运行一周期，收集违规列表后再迭代修复 |
| `internal/` 包之间的依赖关系比经典 Clean Architecture 更扁平 | 强制四层模型可能不合适 | 考虑使用 forge-core 实际依赖图自描述的层（如 core/service/adapter）而非经典分层 |
| 测试文件是包内依赖的主力 | 扇入、layering 排除测试后覆盖更少 | 保持排除测试，但增加一个 `layering_coverage` 计数器 |

---

## 方向二 · 自治运行中的人机结构化反馈通道

**优先级**: 🔴 P0（24h 无人值守 ≠ 人类不能干涉）  
**类别**: 工作流 · 人机协作  
**预估**: ~2 sprints  
**杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 当前的「人机交互」只有三个原子操作: `forge approve`（批准人类闸门）、
`forge run --approved`（批准后继续）、Ctrl-C（杀死进程）。在长达数小时的
自治运行中，人类如果观察到 agent 走向错误方向，**没有轻量级的结构化工具有
干预路径**。

典型场景:
- 运行的第二个小时，人类看到 agent 在实现 `POST /api/users` 但决定改用
  GraphQL——当前只能 Ctrl-C 杀死整个 run，修改 ROADMAP，然后 `--resume`
- agent 的某个 phase 写得很好，但测试覆盖率不够——人类想「这个 phase 通过，
  但下个 phase 请补充测试」
- agent 在 reviewer phase 输出 `REDESIGN`——人类觉得这个裁决过于激进，
  想「接受部分、驳回部分」

当前的二元批准/拒绝模型（`human_gate` + `on_rejected` loop-back）假设人类
干涉是一个**离散事件**（"设计阶段结束时的审批"），但真实的人机协作是**连续
反馈**的。

### 代码级证据

**证据 A: `human_gate` 只有 PASS/FAIL 两个状态**

```go
// internal/converge/converge.go
type Signals struct {
    HumanApproved bool  // true/false — 唯一的人机信号
    ...
}
```

`on_rejected`（Sprint 31 实现）允许定向跳回某个 phase，但**不携带任何反馈内容**
——agent 被跳回后不知道「为什么」被拒绝。

**证据 B: `Observe` sink 只捕获 agent 输出，不捕获人类输入**

```go
// cmd/forge/cost.go — Observer 只从 agent stdout 解析信号
// 没有等价的人类反馈解析器
```

**证据 C: 所有执行入口是同步阻塞的**

```go
// cmd/forge/main.go — subcommands map
// 所有命令都是 "run and wait for completion"
// 没有 "run and listen for external input" 的模式
```

如果用户想在 `forge evolve` 运行到第 3 轮迭代时输入反馈，没有任何机制能
让正在执行的进程接收这个输入——标准输入要么被传到子进程（claude），要么
被忽略。

### 方向建议

1. **反馈信号文件（`.forge/feedback/`）**: forge 运行中定期扫描
   `.forge/feedback/` 目录下的结构化反馈文件（如 `01-redo-phase-3.json`
   内容 `{"action":"redo","target_phase":"implementer","note":"add tests"}`），
   将其注入收敛循环作为外部信号。人类可以在不中断进程的情况下写入文件。

2. **`forge feedback` CLI 子命令**: 向正在运行的 `forge evolve` 进程发送
   信号——通过 Unix socket 或信号文件。支持指令:
   - `forge feedback --pause` — 完当前 phase 后暂停
   - `forge feedback --inject "add tests for auth module"` — 向下一阶段
     注入文本指令
   - `forge feedback --skip-phase qa` — 跳过即将到来的 qa phase
   - `forge feedback --redo-phase implementer --note "use Option type"` —
     跳回指定 phase 并携带说明

3. **收敛循环中的反馈插入点**: `LoopEngine.Run` 在每个 phase 前检查外部
   反馈队列，如果存在未消费的反馈，将其注入 phase prompt（类似 memory 的
   注入方式，但优先级更高）。

### 边界情况

| 场景 | 风险 | 建议 |
|------|------|------|
| 用户输入反馈的同时 phase 刚好完成 | 反馈被应用到错误的 phase | 反馈标记 `iteration_phase` 目标，不匹配则排队到下一次 |
| 多个用户同时写反馈文件 | 冲突/覆盖 | 反馈文件用递增命名（`001-redo.json`），按序消费 |
| agent 忽略人类反馈 | 反馈纳入而不强制执行 | 反馈注入到 prompt 中标记 `[HUMAN_FEEDBACK: must]`，非可选 |
| 进程崩溃后重启时反馈丢失 | 用户输入浪费 | `.forge/feedback/` 中的未消费反馈跨 run 持久化 |

---

## 方向三 · 分析文档膨胀与元治理——治理的治理

**优先级**: 🟠 P1（长期可持续性的门槛问题）  
**类别**: 治理 · 自身工程化  
**预估**: ~1 sprint（初始化治理）+ 持续维护  
**杠杆**: ⭐⭐⭐⭐⭐（影响整个项目发展方向的质量）

### 问题描述

`docs/` 目录目前有 **127 个 markdown 文件，合计 ~60,000+ 行**，其中:
- `docs/requirements/` — 43 份文件，~25,000 行（全部是扩展方向提案）
- `docs/analysis/` — 43 份文件，~25,000 行（分析文档）
- 其他 — ~41 份文档

这些文档中:
- 大量文件有几乎相同的标题（`high-value-extension-directions.md` ·
  `high-value-extension-directions-v2.md` · `high-value-extension-directions-v3.md` ·
  `high-value-extension-v35.md`）
- 许多提案涵盖相同的方向（至少 15 份文档讨论「跨项目治理」,10+ 份讨论「执行语义」）
- 没有文件标记自己是否**仍然活跃**、**已被 supersede**、**已实现**或**已放弃**
- 没有文件记录自己与其他文件的关联关系（supersedes / superseded-by / related-to）
- 没有版本/日期在文件名之外（只有目录列表的 mtime）

结果是:任何新 agent 进入项目时，阅读全部 127 份文档是不现实的，但选择性阅读
又有极高的可能性遗漏关键 context。**分析文档本身已经成为了认知负担和噪声源。**

### 代码级证据

**证据 A: 文件名冲突模式**

```
docs/requirements/
├── high-value-extension-directions.md
├── high-value-extension-directions-v2.md
├── high-value-extension-directions-v3.md
├── high-value-extension-v35.md
├── high-value-expansion-directions.md  ← 注意 expansion vs extension
├── strategic-extensions.md
├── strategic-extensions-v32.md
├── strategic-extensions-v33.md
...
```

一个未来的 agent 需要读几篇才知道哪个是最新的？

**证据 B: 相同的方向在多个文档中重复**

`five-genuinely-uncovered-frontiers.md` 方向三 = forge-core 内部遥测；
`governance-prod-five-frontiers.md` 方向一 = 治理执行性能画像——这两个
方向有 80% 内容重叠。两篇文章在同一天（2026-07-10）由不同的 agent 写入。

**证据 C: `docs/` 不受任何治理规则约束**

```bash
# .arch/rules.yaml 的 cognitive 检查只算顶层模块:
#   max_root_modules: 8
#   current: 4 (docs, examples, forge-core, harness)
# 但 docs/ 内部有 127 个文件，没有任何结构/命名/体积约束。

# gate.mjs 只检查根目录文件 ≤ 15 和源文件 ≤ 500 行
# docs/ 中的 markdown 完全不被检查
```

ForgeOS 自己的治理执法 (`gate.mjs` / `arch-check.mjs` / `check.py`) 对 `docs/`
目录零覆盖——这是全仓最大的不受治理的目录。

### 方向建议

1. **文档清单 + 元数据**: 在 `docs/INDEX.md` 中维护一个活跃文档清单，每篇文档
   包含: `id`, `title`, `status`（active | superseded | implemented | retired）,
   `supersedes`/`superseded_by` 引用。新增文档必须注册。

2. **文档生命周期门控**: `check.py` 增加 `check_doc_index` 检查——每个 `docs/`
   中的 `.md` 文件必须在 INDEX.md 中有注册；状态为 `superseded` 或 `implemented`
   超过 60 天的文件建议归档到 `docs/archive/`。

3. **去重扫描**: 新增 `forge doc-dedup --scan` 工具，计算文档间的 TF-IDF 相似度，
   标记内容重叠 >60% 的文档对，供人类判断是否需要合并。

4. **文档 header 模板**: 所有新分析文档必须在开头包含 `status: draft` 和
   `supersedes: []` 字段。这不需要复杂工具——一个简单的 YAML front-matter
   约定 + `check.py` 验证即可。

### 边界情况

| 场景 | 风险 | 建议 |
|------|------|------|
| INDEX.md 本身过时 | 文档清单变成另一个需要治理的文件 | 用 `check_doc_index` 自动验证——所有文档必须被索引或位于 `archive/` |
| 归档后有人需要旧文档 | 信息丢失 | 归档 = 移动到 `docs/archive/`，不是删除；INDEX.md 保留 `retired` 条目 |
| 去重工具产生误报 | 错误建议合并 | 去重工具只标记、不自动执行；建议列表需人工确认 |
| 强制文档 header 阻碍快速草稿 | 写文档的门槛提高 | 允许 `status: draft`；草稿可以快速写，只是不会出现在正式索引中 |

---

## 方向四 · Python YAML shim 运行时依赖——零依赖声明的裂缝

**优先级**: 🔴 P0（架构承诺 vs 运行时事实的漂移）  
**类别**: 架构 · 工程化 · 运行时可靠性  
**预估**: ~1.5 sprints（验证 Go 解析器完备性 + 迁移）  
**杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 在 `BOOTSTRAP.md` 和 `go.mod` 中宣称**「纯 Go 标准库，零外部依赖」**
（`go.mod` 确实无 `require` 语句）。但 forge-core 的 YAML 解析路径存在一个
**运行时必须的 Python 依赖**:

```
forge run/evolve → loadWorkflow → yaml2json.Decode (Go) → fail → python3 harness/yaml2json.py (fallback)
```

Python shim 不是「可选的加速器」——它是**主解析路径的 fallback**。当 Go 原生
解析器对一个 YAML 文件返回 error 时，整个 workflow 加载 fallback 到 Python shim。
如果 Python 不可用，workflow 完全不能加载。

### 代码级证据

**证据 A: `loadWorkflow` 的 fallback 路径要求 Python**

```go
// cmd/forge/main.go
func loadWorkflow(root, name string) (*asset.Workflow, error) {
    path := filepath.Join(root, ".agent", "workflows", name+".yml")
    wfJSON, err := yaml2json.Decode(path)  // Go native parser
    if err != nil {
        // fallback to Python shim
        shim := filepath.Join(root, "harness", "yaml2json.py")
        out, err := exec.Command("python3", shim, path).Output()
        ...
    }
    ...
}
```

如果 `yaml2json.Decode` 失败且 `python3` 不在 PATH 中，workflow 无法加载，
所有 `forge run`/`forge evolve` 命令失效。

**证据 B: Go 原生解析器未被验证为完整替代品**

```go
// internal/yaml2json/ — 10 个文件，~700 行
// 在 Sprint 27 中发现了 block-scalar 损坏的 bug
// 该 bug 通过了所有已有测试（测试未断言正确值）
// 修复后与 PyYAML 的 7/7 真文件比对通过
```

Go 解析器的正确性验证仅限于 7 个 forge-core 自己的 workflow 文件。它没有
针对边缘情况（多文档 YAML、锚点/别名、标签、非 UTF-8 编码）的测试套件。
下次遇到不在 7 个测试文件中的 YAML 特性时，可能会再次通过测试但输出错误。

**证据 C: yaml2json_test.go 的 `TestToJSON_MatchesPythonShim` 是集成测试**

```go
// main_agent_test.go:190-243
// 这个测试在 CI 中需要 python3，跳过如果 python3 不可用
// 所以在一个没有 Python 的环境中，Go 解析器的正确性完全不可验证
```

**证据 D: python shim 还是 forge-core 自检测的锚点**

```go
// main_agent_test.go:33
if _, err := os.Stat(filepath.Join(dir, "harness", "yaml2json.py")); err == nil {
    // 用这个判断是否在 ForgeOS repo 内
}
```

`yaml2json.py` 的存在被用作「是否在 ForgeOS 仓库中」的检测标记——这个 shim
已经从工具进化成了身份标记。

### 历史背景

2026-06-19: Go 标准库无 YAML 解析器；用 Python shim 作为「临时脚手架」，
未来可换 Go YAML 库（ROADMAP.md 诚实标注）。

Sprint 27（2026-07-02）: yaml2json block-scalar bug 被 real agent 发现。
Go 原生解析器修复 + 验证与 PyYAML 输出一致。但**仍无单独的 YAML 解析测试套件**。

### 为什么它与已有方向不同

- `ROADMAP.md` 提到「YAML 经 python shim 转码—临时脚手架」，但那是作为
  观察陈述，没有提出完整的迁移策略和验证框架。
- `expansion-core-five-2026-07-01.md` 方向二子段落提到「从 Python shim
  迁移到 Go YAML」作为一个架构任务，但那是作为「文档-代码漂移检测」的
  一个子示例，不是独立的迁移计划。
- 没有一个已有方向把「Go YAML 解析器完备性验证」和「移除 Python 运行时依赖」
  作为独立目标。

### 方向建议

1. **Go 解析器 YAML 合规测试套件**: 建立基于 YAML 官方测试套件
   （`yaml-test-suite`）的 pytest 级别的测试矩阵，覆盖:锚点/别名/标签、
   多文档流、非 ASCII 编码、block 和 flow 样式、各种标量类型。

2. **移除 Python fallback**: 当 Go 解析器通过 YAML 合规套件后，将 Python shim
   从 fallback 路径降级为 `--use-python-shim` 可选 flag，默认关闭。最终在
   v3 之前完全移除。

3. **移除 `main_agent_test.go` 对 `yaml2json.py` 的仓库检测依赖**: 用一个
   显式的 `.forgeos-root` 标记文件代替，彻底切断对 Python 的存在依赖。

4. **Go YAML 性能基准**: 对比 Go 解析器 vs Python shim 在真实 workflow
   文件上的解析延迟（预期 Go 解析器更快，需要数据佐证迁移决策）。

### 边界情况

| 场景 | 风险 | 建议 |
|------|------|------|
| Python shim 移除后，用户自定义的 YAML 扩展无法解析 | 兼容性断裂 | 先在 `--use-python-shim` 后运行一个周期，收集失败案例 |
| Go 解析器遇到 PyYAML 接受的语法但 Go 拒绝 | Go 可能更严格 | 报告具体差异，逐个判断是 Go bug 还是 PyYAML 过于宽松 |
| `forge-init` 复制的项目需要 Python | 新项目继承旧依赖 | `forge-init` 在 Python shim 移除后不再复制 `yaml2json.py` |
| 用户依赖 Python shim 的特定行为 | 无声的行为变化 | 迁移周期 = 1 个完整版本（先 flag 控制 → 默认关闭 → 代码移除） |

---

## 方向五 · ForgeOS 自身的 dogfood 鸿沟——docs/ 是最大的未治理领地

**优先级**: 🟠 P1（违背「先治理自己」的核心承诺）  
**类别**: 治理 · dogfood · 自身一致性  
**预估**: ~1 sprint  
**杠杆**: ⭐⭐⭐⭐⭐（直接验证 ForgeOS 的治理承诺）

### 问题描述

`CLAUDE.md` 和 `AGENTS.md` 声明 ForgeOS **自己遵守自己的治理规则**。
但 `docs/` 目录——全仓最大的文件集合（127 文件，~60,000 行）——完全不受
任何治理约束:

- `gate.mjs` 行数检查:不检查 `docs/` 下的 `.md` 文件
- `arch-check.mjs` 架构检查:只检查 Go/JS/Python 源文件
- `check.py` 治理完整性:不检查文档间引用或文档结构
- `secret-scan.mjs`:只扫描源文件，不扫描 markdown

这违背了 ForgeOS 的核心承诺:**治理系统先治理自己**。如果 `docs/` 可以不受
治理地膨胀，那 ForgeOS 的治理承诺在哪里是真实的？

更重要的是:directions 1-4 中建议的任何修复（文档索引、YAML 迁移、架构层
映射、人类反馈）都应在 ForgeOS 自己的代码库中先通过 `forge accept`——但
当前的文档治理鸿沟让任何关于「治理完善」的声明都带着星号。

### 代码级证据

**证据 A: `gate.mjs` 仅检查特定扩展名**

```javascript
// gate.mjs — 默认文件遍历
// 只检查 .go, .js, .mjs, .ts, .py, .rs, .java, .kt, .swift, .c, .h, .cpp
// .md 完全不在扫描范围内
```

**证据 B: `secret-scan.mjs` 跳过文档**

```
secret-scan.mjs 的 provider 匹配器运行在所有被 gate.mjs 遍历的文件上
→ .md 文件不被 gate.mjs 遍历 → secret-scan 对 doc 零覆盖
```

但文档中**最可能含有硬编码 URL/API key/端点示例**。

**证据 C: ForgeOS 的功能需求审计自身就是一份文档**

`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（Sprint 30 产物）列出了所有已知
功能需求及其状态。但这份文档本身:
- 没有被任何自动化工具验证是否反映当前代码（它引用的代码可能已改变）
- 没有过期机制
- 没有交叉引用验证（它声称的状态是否与 `CURRENT_SPRINT.md` 一致?）

### 方向建议

1. **`docs/` 纳入 `gate.mjs` 的范围**: 至少对 `docs/` 下的 `.md` 文件执行
   行数检查（单文件 ≤ 1000 行可能更合理——文档需要比代码多一点的篇幅），
   并纳入 `secret-scan.mjs` 的扫描范围。

2. **文档反模式命名检查**: `arch-check` 的 `checkAntiPatterns` 可以扩展
   到检查文档目录中的反模式——如名称只有版本号差异的文件
   （`*v2.md` / `*v3.md` / `*v35.md`），提示可能需要合并。

3. **文档引用完整性检查**: 新增 `harness/doc-ref-check.mjs`，扫描
   `docs/` 中的 markdown 文件，验证其中的相对链接（`[text](./file.md)`）
   指向的文件存在。这是一个已知的常见问题——文档间链接在文件被移动或
   重命名后腐烂。

4. **自我 dogfood 门**: 在 CI 中增加一个 gate，要求 ForgeOS 自身能在
   engineering/mvp 模式下通过 `forge accept`——这不仅验证治理规则是否
   被遵守，也验证 `docs/` 治理后的结果。

### 边界情况

| 场景 | 风险 | 建议 |
|------|------|------|
| 文档行数限制阻碍详细技术文档 | 长文档被迫拆分，可能降低可读性 | 文档阈值设为 1000 行（代码的 2 倍）；超过才触发 |
| 文档引用检查在 CI 中减慢速度 | markdown 解析比代码扫描慢 | 引用检查可选（`--check-doc-refs`），不在主 `gate.mjs` 路径上 |
| 自我 dogfood 门在文档治理未完成时永远 FAIL | 无法合并任何代码 | 先建文档治理基线，再启用自我 dogfood 门（有一个过渡期） |
| 旧文档没有 front-matter | 无法自动分类 | 设置过渡期（30 天），之后未分类文档触发告警 |

---

## 汇总

| # | 方向 | 优先级 | 类别 | 预估工作量 | 核心差异化 |
|---|------|--------|------|-----------|-----------|
| 1 | arch-check 分层执法对 forge-core 自身是静默盲区 | **P0** | 架构/自身执法 | ~1 sprint | 所有已有分析认为 arch-check layering 是「有效检查」;本文证明它对 forge-core 自身是静默 no-op |
| 2 | 自治运行中的人机结构化反馈通道 | **P0** | 工作流/人机协作 | ~2 sprints | 已有方向聚焦「自动化」;本文关注人在自动化运行时如何有结构地干预 |
| 3 | 分析文档膨胀与元治理 | P1 | 治理/自身工程化 | ~1 sprint | 127 份文档 ~60,000 行从未被作为「需要治理的系统」审视;已有方向全是前向提案,从未逆向反思 |
| 4 | Python YAML shim 运行时依赖 | **P0** | 架构/运行时可靠性 | ~1.5 sprints | 「零外部依赖」的核心承诺被 Python shim 静默打破;本文是第一个完整迁移计划 |
| 5 | ForgeOS 自身的 dogfood 鸿沟 | P1 | 治理/dogfood | ~1 sprint | `docs/` 是全仓最大的未治理领地;违背「先治理自己」的核心纪律 |

### 收敛建议

**若只做一件**:方向一（arch-check 分层盲区）——成本最低（几行 config + 包映射），
修复的是整个架构执法体系的可信度地基。如果 ForgeOS 自己的架构都不被 arch-check
实际验证，那 arch-check 对其他项目的「layering passed」声明就永远带着星号。

**做前三件**:方向一 + 方向四 + 方向二 —— 分别解决**自身架构可信度**、
**运行时依赖诚实性**和**人机协作可操作性**。三者共同奠定了 ForgeOS 从
「功能完整」到「架构可信」的基础。

**方向三（文档元治理）是方向五（dogfood 鸿沟）的先决条件**——在 docs/ 被
纳入治理之前，ForgeOS 的「先治理自己」承诺有一个显眼的例外。没有方向三，
方向五无法落地。

> **诚实边界**:方向三不试图解决「已有 127 份文档太多」的问题——它只建立
> 一个防止未来继续膨胀的框架。已有文档的去重/归档是一个独立的工程任务，
> 建议作为后续 sprint 专项处理。
