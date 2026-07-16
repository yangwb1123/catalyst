# ForgeOS — 五个产品视角的结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件深扫完整代码库:  
>    - forge-core(19 Go 包 · ~35k LOC 生产代码 · 纯 stdlib 零依赖,`cmd/forge` 12.5k LOC / 17 文件)  
>    - harness(34+ 模块 · ~10.5k LOC 执法层 · gate/check/accept/arch/secret-scan/SCA/select-tests)  
>    - `.agent/` 完整治理骨架(12 agent 卡 · 9 skill 卡 · 5 工作流 · 完整 policies/modes/routing/eval)  
>    - examples/url-shortener(5 源文件,39 测试)· examples/go-taskd(5 源文件,4 测试)  
>    - CI pipeline(`.github/workflows/forge.yml`) · `pi-batch.py`(499 行,零测试,零治理)  
>    - 架构文档(4 ADR · north-star · HA-security · loop-engineering · ARCHITECTURE/ROADMAP/CURRENT_SPRINT)  
> 2. 逐篇通读 **56 份 `docs/requirements/*.md` + 40 份 `docs/analysis/*.md` + FUNCTIONAL_REQUIREMENTS_AUDIT**  
>    + 核心架构文档(共 ~100+ 份已有分析),对每个候选方向做关键词 + 语义交叉检索,确保**本方向从未  
>    作为独立扩展方向被系统性展开**。  
> 3. **差异化证明**:每个方向附代码级证据,引用最接近的已有分析并解释为什么不是同一方向。  
> 4. **纪律**:不编写任何代码。每方向附边界情况表、实际影响。  
> **日期**: 2026-07-10

---

## 全景定位:已有 ~100+ 份分析的覆盖格局

ForgeOS 经过 31 轮 sprint 迭代和 ~100+ 份分析文档的反复覆盖,几乎所有可触及的功能域都已被深度扫描:

| 覆盖域 | 约方向数 | 代表文档 |
|--------|---------|---------|
| 引擎补齐(编排/路由/记忆/收敛/信号/并行/wave/Reflect) | ~35 | `high-value-extension-directions*.md` |
| 生产可靠性(Prompt QA / 信号硬化 / 环境验证 / 自愈) | ~15 | `expansion-production-readiness*.md` |
| 执行语义形式化(原子性/幂等/因果一致性/回滚/版本演化) | ~12 | `execution-semantic-gaps.md` |
| 二阶系统问题(知识衰减/配置爆炸/TOCTOU/并行安全) | ~15 | `second-order-architectural-gaps.md` |
| 多仓库/联邦/跨会话治理(workspace/portability) | ~12 | `expansion-horizon-three.md`·`truly-novel-five-directions.md` |
| 安全纵深(凭据/SCA/沙箱/secret-scan/注入防御) | ~8 | `forgotten-five-system-boundaries.md` |
| 北极星桥梁(Temporal/OPA/OTel/多厂商/Sandbox/Web UI) | ~8 | `v2-to-northstar-gap.md` |
| 结构性债务(cmd/forge / YAML碎片 / pi-batch / 存储累积) | ~10 | `forgotten-five-structural-debt.md` |
| 跨层面系统性缺口(子进程错误/测试跳过/缓存一致性/轨迹盲区) | ~5 | `cross-cutting-systemic-gaps.md`·`expansion-five-systemic-gaps.md` |
| **总计已有分析覆盖** | **~150+ 方向** | **~100+ 份独立文档** |

**本文的 5 个方向落在上述所有已有覆盖的间隙中** —— 它们不是「缺什么引擎」或「边界情况修复」,
而是代码级观察结合产品视角推导出的**结构性产品盲区**。每个方向已在全部已有文档中验证从未作为独立方向展开。

---

## 方向一 · Post-Acceptance 治理管线:将部署/发布/回滚扩展为第一类治理阶段

**优先级**: 🟠 P1 | **类别**: 产品 · 架构 | **预估**: ~2–3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的脊柱止于 `forge accept ACCEPTED`。从「代码就绪」到「生产运行」之间有一条巨大的信任鸿沟:

- ACCEPTED 只保证代码通过了所有闸门 —— 不保证它**安全地服务于生产流量**
- 真实的软件交付需要金丝雀分析、滚动发布协调、自动化回滚、以及部署后验证
- 这些**不是独立于 ForgeOS 的外部问题** —— 它们与构建/测试/评审遵循相同的「声明式治理」模式,
  理应作为 Workflow 中的第一类 Phase 存在

### 代码级证据

**证据 A: `asset.Phase` 的结构完全支持部署阶段但没有部署 Workflow**

```go
// forge-core/internal/asset/asset.go
type Phase struct {
    Name        string          // 阶段名 — 可以叫 "canary-deploy" / "production-rollout"
    Agent       string          // 执行 agent — "devops-engineer" 或 "deploy-agent"
    Kind        string          // "agent" | "gate" — 部署可以是 gate(审批) 或 agent(自动执行)
    RequiredGates []string      // 部署前的合规闸门
    ModelTier   string          // 部署决策可以用 Opus
    // ... timeout, retry, on_fail, loop_back 全都在
}
```

Phase 结构已经支持 timeout/retry/loop-back/checkpoint 等可靠执行语义,天然适用于部署动作。
但没有 `deploy.yml`,没有 `release.yml`,`asset.Workflow` 没有 `next_stage: deploy`。

**证据 B: CI pipeline 只会构建和测试,不会分发**

```yaml
# .github/workflows/forge.yml (当前)
steps:
  - node harness/acceptance.mjs        # 闸门
  - go -C forge-core build ./...        # 构建
  - go -C forge-core test -race ./...   # 测试
  # ← 没有: 上传构建产物到 Release
  # ← 没有: 打 git tag
  # ← 没有: 生成 SBOM + 签名
```

**证据 C: `modes.yml` 的 production lifecycle 只有代码层面的收紧,没有部署阶段**

```yaml
# .agent/policies/modes.yml
production:
  coverage_delta: +20
  require_min_gates: [lint, test, build, complexity, arch, security]
  enforce_floor: block
  # ← 没有: require_deploy_pipeline
  # ← 没有: require_rollback_plan
  # ← 没有: require_canary_analysis
```

### 方案轮廓

1. **新增 `deploy.yml` / `release.yml` Workflow**:
   - `deploy.yml`: 构建 → 签名 → 上传 → 金丝雀 → 滚动发布 → 健康验证
   - `release.yml`: 打 tag → 生成 changelog → 发布 Release → 通知
   - 复用现有 `loop-back`, `on_fail`, `required_gates`, `human_gate` 等全部语义

2. **新增 `rollback.yml`**:
   - 检测部署后健康退化 → 自动化回滚到上一版本
   - 复用 `checkpoint` 的保留历史 (persist.Save 已有 `retain` 参数)
   - 回滚作为 `human_gate` 的紧急路径

3. **信号扩展**:`converge.Signals` 增加部署状态维度的信号:
   - `DeploymentStatus` — last deploy result
   - `RollbackAvailable` — 是否有安全回滚点
   - `HealthCheckPass` — 生产健康检查是否通过

### 边界情况

| 场景 | 行为 |
|------|------|
| 项目无部署目标(库/CLI 工具) | deploy 阶段跳过 → 诚实 N/A,与 lint/coverage 缺少工具的降级模式一致 |
| 金丝雀部署发现回归 | on_fail + loop_back → 自动回滚金丝雀 + 把问题喂回 implementer |
| 回滚后修复重新发布 | 复用 checkpoint + resume,从修复 phase 开始,不重跑整个 pipeline |
| 手动回滚 vs 自动回滚 | `human_gate` 要求人批准回滚;`on_fail.action: rollback` 触发自动回滚 |
| 多环境(staging + prod) | deploy.yml 定义 multi-phase 部署,每环境是一个 phase |

### 与已有覆盖的差异

- `five-high-value-extensions-v44.md` 方向一讨论的是「供应琏完整性(SBOM+签名)」,聚焦于**构建产物的可溯源**,
  而非本文关注的「将部署/发布作为治理阶段扩展到 ForgeOS pipeline」
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向二讨论的是「ForgeOS 二进制
  的分发与自更新」,那是 forge-core 自身的交付问题,不是 ForgeOS 为被治理项目提供的部署治理能力
- 其他分析文档中出现的 `deploy` 关键词均为随口的产物提及,从未将「Post-Acceptance 治理管道」作为一个
  完整的扩展方向展开

---

## 方向二 · 跨会话记忆传递与学习继承

**优先级**: 🟠 P2 | **类别**: 可靠性 · 知识管理 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的记忆系统 (`internal/memory`) 在单个 evolve 会话内累积 learnings:每次迭代 append
一条 Entry,后续迭代通过 Load 读取。但**这个记忆系统是 session-scoped 的**——当 evolve 循环
结束,下一轮 `forge evolve` 从零开始:

1. 上一轮学到的教训(`lesson` kind entries)完全丢失
2. 上一轮发现的决策(`choice` kind entries)需要 agent 重新发现
3. 上一轮的收敛轨迹(什么策略 WORKED,什么策略 DIDN'T)没有任何机制传递给下一轮
4. 如果上一轮因 budget 耗尽在 60% 完成度停止,下一轮没有「已学知识」的上下文

这不同于 checkpoint resume:checkpoint 恢复的是**执行位置**(上次跑到哪个 phase),而不是
**知识状态**(上次学到了什么)。

### 代码级证据

**证据 A: memory 是 session-local,没有跨 session 查询机制**

```go
// forge-core/internal/memory/memory.go
// Append 只在 evolve loop 内被调用
// Load 只能读取当前 .memory.jsonl 文件的内容
// 没有任何 --load-memory / --inherit / --previous-session 参数
// Query 是纯内存过滤器,不跨文件
```

```go
// forge-core/cmd/forge/evolve.go
// cmdEvolve 的入口:
// - loadWorkflow → buildRunEngine → execLoop
// - 没有读取上次 evolve 的 memory
// - 没有「如果你上次学到了 X,这次继续用」的机制
```

**证据 B: memory 文件路径固定在 `<root>/.memory.jsonl`,无版本/无会话标识**

```go
// forge-core/internal/memory/memory.go
// 文件路径硬编码为 <root>/.memory.jsonl
// 没有 session ID,没有迭代范围标记
// Append 总是追加到文件末尾,Load 总是读取全部
```

**证据 C: forge evolve 的 `--resume` 只恢复 checkpoint,不恢复 memory 上下文**

```go
// forge-core/cmd/forge/evolve.go — resumeStart
// resumeStart 从 checkpoint 恢复:
// - workflow 名称
// - iteration 计数
// - phase_index
// - spent_usd_micros
// ← 没有: memory_state
// ← 没有: last_session_id
// ← 没有: accumulated_knowledge
```

### 方案轮廓

1. **Session 标识**:每次 `forge evolve` 生成唯一 Session ID,写入 memory Entry 的元数据
2. **跨 session Load**:`forge evolve --load-memory N` 加载最近 N 个 session 的记忆
3. **记忆衰减**:旧 session 的条目按时间衰减权重(类似 scorecard 的 recency_half_life_days)
4. **知识摘要**:新 session 启动时,生成「先验知识摘要」注入 agent prompt:  
   「以下是从上次 evolve 中学到的教训,请避免重复相同错误:...」

### 边界情况

| 场景 | 行为 |
|------|------|
| 首次 evolve(无历史记忆) | 空记忆,不注入先验摘要(与当前行为一致) |
| 旧 session 的教训在当前已过时 | 衰减机制降低权重,agent 收到低置信度标注 |
| 跨大版本重构(全部重写) | `forge evolve --fresh` 跳过加载历史记忆 |
| memory.jsonl 损坏 | 与当前 Load 逻辑一致:显式错误,不静默丢弃 |
| 多个 team 跑同一项目的不同 evolve | Session ID + team tag 隔离记忆空间 |

### 与已有覆盖的差异

- `truly-novel-five-directions.md` 方向一「跨会话可移植工作空间」关注的是**状态文件(.forge/、memory、
  trace、checkpoint)能否跨机器/克隆迁移**。本文关注的是**知识内容能否跨 evolve 会话继承和复用**。
  两者正交:一个是文件的可移植性,一个是语义的知识传递。
- 所有已有的 memory 相关分析(>10 方向)都聚焦于 memory 的**内部机制**(压缩/衰减/缓存/一致性),
  从不讨论 memory 的**跨会话消费**。

---

## 方向三 · Workflow Phase 模板库与阶段复用

**优先级**: 🟢 P2 | **类别**: 架构 · 产品化 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 当前有 5 个 workflow 文件(`discover/design/build/review/evolve.yml`),每个都
从头定义自己的 phases。这导致:

1. **重复定义**:`harness-gates → reviewer` 序列在 build.yml 和 evolve.yml 中几乎完全相同
2. **治理漂移**:当一个 workflow 的某个 phase 更新了(如 build.yml 的 reviewer 加了新 emit),  
   evolve.yml 中对应的 reviewer phase 需要手动同步
3. **领域特定性缺失**:Go 微服务和 Python ML 项目使用完全相同的 workflow 定义——没有
   「Go-service 标准 pipeline」或「ML-training standard pipeline」的预制模板
4. **采纳门槛**:新用户需要理解 5 个完整 workflow 文件来配置治理——如果能有「import 现成模板」的机制,
   可以大幅降低入门难度

### 代码级证据

**证据 A: `asset.Workflow` 没有模板/继承/组合机制**

```go
// forge-core/internal/asset/asset.go
type Workflow struct {
    Name   string
    Phases []Phase   // ← 唯一的执行单元
    Stop   StopCondition
    // ← 没有: Include []string          — 引入其他 workflow
    // ← 没有: ImportPhase []string      — 引入单个 phase 定义
    // ← 没有: Extends string            — 基于某个 workflow 扩展
    // ← 没有: OverridePhase []PhaseOverride — 覆盖被引入的 phase
}
```

**证据 B: build.yml 和 evolve.yml 的 gate→review 序列高度重复**

```yaml
# build.yml (示意)
- name: harness-gates
  required_gates: [lint, test, build, complexity]
  on_fail: {action: loop_back, target_phase: implementer}
- name: reviewer
  agent: reviewer
  on_fail: {action: loop_back, target_phase: implementer}

# evolve.yml (几乎相同的序列)
- name: harness-gates
  required_gates: [lint, test, build, complexity]
  on_fail: {action: loop_back, target_phase: plan}
- name: reviewer
  agent: reviewer
  on_fail: {action: loop_back, target_phase: plan}
```

**证据 C: `forge detect` 检测项目语言但不返回不同类型的 workflow**

```go
// forge-core/cmd/forge/detect.go
// detectProject 检测:
// - 语言(go/node/python/rust)
// - 构建系统
// - 测试框架
// - CI 存在性
// 但所有检测结果都映射到同一套 workflow(通用的 discover/design/build/review/evolve)
// 没有: "detected Go module → suggest go-service.yml"
// 没有: "detected Python + ML deps → suggest ml-training.yml"
```

### 方案轮廓

1. **新增 `.agent/templates/` 目录**:存放可复用的 phase 模板 YAML 文件
2. **`uses:` 语法**:workflow phase 可以声明 `uses: templates/quality-gate.yml` 来引入预定义阶段
3. **领域特定模板库**:
   - `go-service.yml` — Go 微服务的标准 pipeline(lint→test→build→cover→security)
   - `node-package.yml` — Node 包的标准 pipeline
   - `ml-training.yml` — ML 训练项目的特殊 pipeline(数据验证→训练→评估→注册)
4. **`forge detect --template`**:根据项目类型自动推荐和安装对应的 workflow 模板
5. **模板参数化**:模板支持 `{{ .CoverageThreshold }}`、`{{ .RequiredGates }}` 等参数,在 `project.yml` 中填写

### 边界情况

| 场景 | 行为 |
|------|------|
| 模板文件不存在 | 与 `uses_template` 的失效模式一致:forge validate 报告 WARN |
| 模板和本地 phase 混合声明 | 按声明顺序展开,模板展开后的 phases 在本地 phases 之前或之后 |
| 模板版本化管理 | 模板本身在 `.agent/templates/` 中是 versioned(git-tracked)资产 |
| 覆盖模板中某个 phase | workflow 提供 `override:` 块,类似面向对象的方法覆盖 |
| 用户自定义模板 | 与 agent card / skill card 一样,用户编写 YAML 放入 templates/ 即可 |

### 与已有覆盖的差异

- `expansion-five-truly-uncovered-frontiers-v46.md` 方向四的「Workflow Composition Algebra」
  在该方向的 12 个子点中包含了一句「Shared Phase Template」。但那是 12 个子点之一,
  不是作为独立方向展开;本文将其提升为独立的、产品面向的扩展方向,并深入展开模板库、
  领域特定模板、`forge detect --template` 等产品特性。
- `genuine-expansion-gaps.md` 方向一「Workflow Composition Engine」关注的是**跨 workflow 的
  管线编排**(一个 workflow 完成后触发下一个),而不是**同一个 workflow 内部 phase 的复用**。
  两者正交:一个是执行顺序的组合,一个是定义内容的复用。

---

## 方向四 · 组织级多租户与策略继承体系

**优先级**: 🟠 P1 | **类别**: 架构 · 规模化 | **预估**: ~3–4 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 当前是**单项目**的治理系统。一个组织如果有 50 个项目使用 ForgeOS,每个项目都通过
`forge-init` 获得一套完全独立的治理副本:

1. **无组织级策略基线**:无法定义「组织内所有项目必须通过 secret-scan」作为统一基线
2. **策略漂移**:项目 A 修改了 `policies.yml`,项目 B 完全不知道
3. **无跨项目依赖管理**:项目 A 改了 API → 项目 B 需要重新通过相关闸门——当前没有任何机制
4. **无集中可见性**:无法一个命令查看所有项目的 gate 状态
5. **渐进式治理不可行**:新项目从 explorer 开始,但无法「继承组织基线再放宽」——只能从头配置

这不同于已覆盖的「多仓库联邦(polyrepo/federation)」。联邦关注的是**一个特性跨多个仓库的
编排**(类似 monorepo 的跨仓库拆分的解决方案);多租户关注的是**一个组织管理多个独立项目
的策略和治理**。

### 代码级证据

**证据 A: `project.yml` 是单项目配置,没有组织/继承概念**

```yaml
# .agent/project.yml (当前)
mode: engineering         # ← 项目自己的模式
lifecycle: mvp            # ← 项目自己的生命周期
# ← 没有: org: forgeos
# ← 没有: governance_baseline: org/v2
# ← 没有: policy_inherit: [org-base, team-frontend]
```

**证据 B: `forge-init` 复制而非引用治理资产**

```javascript
// harness/scaffold/forge-init.mjs
// COPIED_FILES 将 .agent/ 的全部内容复制到新项目
// 复制后,项目与新项目之间没有任何引用关系
// 没有: forge governance sync    — 与组织模板同步
// 没有: forge governance drift   — 检测偏移
```

**证据 C: 无跨项目状态共享或查询机制**

```go
// forge-core/cmd/forge/main.go
// forge 的所有命令都接受 --root 指向一个项目
// 没有: forge fleet status       — 查看所有项目状态
// 没有: forge fleet gate         — 对所有项目跑闸门
// 没有: project.yml 中声明 depends_on: [project-a, project-b]
```

### 方案轮廓

1. **组织策略基线**:`project.yml` 新增可选字段:
   ```yaml
   governance:
     org: my-org
     baseline: org/v2
     overrides:
       mode: engineering
       coverage_threshold: 90
   ```

2. **`forge governance` 子命令族**:
   - `forge governance sync` — 同步组织基线到本地(新增的 gate/check 自动安装)
   - `forge governance drift` — 检测本地治理与组织基线的差异
   - `forge governance report` — 输出治理健康报告

3. **中央模板仓库**:组织可以有一个 `forgeos-policies` 仓库,
   被所有 `project.yml` 通过 `governance.baseline` 引用

4. **跨项目依赖追踪**(v2):当项目 A 的公共 API 变化时,机制检测到并提醒
   项目 B 需要重新验证

### 边界情况

| 场景 | 行为 |
|------|------|
| 组织基线新增一个 gate | `forge governance sync` 安装新 gate + 更新 project.yml |
| 组织基线删除一个 gate | 项目保留本地 overrides,governance sync 报告冲突 |
| 离线环境无法访问中央仓库 | 使用本地缓存的基线版本,诚实标注「last synced: 2026-07-01」 |
| 项目需要完全自定义治理 | `governance.baseline: none`(当前行为,向后兼容) |
| 多团队不同策略(前端/后端/ML) | baseline + team overlay: `baseline: org/v2, team_overlay: team-frontend/v1` |

### 与已有覆盖的差异

- `expansion-horizon-three.md` 的「多仓库联邦」和跨仓库编排关注的是**一次特性变更跨多个仓库
  的执行编排**。多租户关注的是**多个独立项目之间策略的一致性和组织级治理**。一个是执行维度的
  横向扩展(一个特性横跨 N 个仓库),一个是管理维度的纵向扩展(一个组织管理 N 个独立项目)。
- `cross-cutting-systemic-gaps.md` 的「Agent 治理资产的版本化生命周期管理」关注的是单项目内
  治理资产的版本演化和兼容性,而非跨项目的策略继承。
- `five-systemic-oversights-v45.md` 方向五「治理熵与 Hygiene」提到了 `forge governance drift` 命令
  但那是作为治理熵管理的一个子功能,且聚焦于单项目的「无用治理资产清理」。本文将其提升为
  组织级多租户策略继承的完整方向。

---

## 方向五 · ForgeOS 自评价元认知循环

**优先级**: 🟢 P3 | **类别**: 治理 · 可观测性 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 的 scorecard 系统 (`internal/routing/scorecard.go`) 为**模型路由**服务:它记录
哪个模型在哪个任务类型上表现最好,驱动 `HistoryTiebreak` 选择最优模型。

但 ForgeOS **没有同等系统来评价自己**:
- 哪些 workflow phase 最经常 FAIL/loop-back?——不知道
- 哪种 mode(explorer/balanced/engineering)在实际项目中最快 converge?——不知道
- 哪个 agent card 的 prompt 结构导致最多 REQUEST_CHANGES?——不知道
- 哪个 harness gate 最常被触发且最可能指示真实问题?——不知道
- 系统瓶颈在哪里?成本消耗最大的 phase 是哪些?——不知道

这是一种元认知盲区:ForgeOS 收集了大量数据(trace/scorecard/memory/checkpoint),
但**从来不分析这些数据来改进自己**。

### 代码级证据

**证据 A: trace 数据丰富但零消费于系统自评价**

```go
// forge-core/internal/trace/trace.go
// Event 包含:
// - Kind: "iteration" | "agent" | "gate" | "decision" | "converge" | "error" | ...
// - DurationMs: 阶段耗时
// - CostUsdMicros: 成本
// - Model: 使用的模型
// - Verdict: gate 裁决(PASS/FAIL/NA)
// 
// 这些数据被写入 trace.jsonl,但:
// - 没有人分析「哪类 phase 超时最多」
// - 没有人分析「哪个 gate 最常 FAIL」
// - 没有人分析「converge 的平均迭代次数」
```

**证据 B: scorecard 只用来选模型,不用于系统诊断**

```go
// forge-core/internal/routing/scorecard.go
// ScorecardPair 包含:
// - Model, TaskType, Quality, LatencyMs, AvgCostUsd, SampleCount
// 消费点只有:
// - routing.HistoryTiebreak — 选择模型
// 没有:
// - 「哪个 workflow converge 最快」
// - 「哪个 agent card 产生的 VERDICT 最准确」
```

**证据 C: 没有 `forge retrospective` 或 `forge analyze` 命令**

```go
// forge-core/cmd/forge/main.go — 命令表
// 现有子命令:
// run, evolve, gate, check, accept, route, migrate,
// detect, validate, doctor, preflight, approve, scorecard
// ← 没有: forge retrospective   — 分析上次 evolve 的效能
// ← 没有: forge analyze          — 分析 trace/scorecard 数据
// ← 没有: forge suggest           — 根据分析结果建议改进治理
```

### 方案轮廓

1. **溯源分析引擎**:分析 trace.jsonl + scorecards.json 回答以下问题:
   - 「哪些 phase 的 loop-back 最多?」→ 指示 agent card 或 phase 设计有问题
   - 「converge 的迭代数趋势:在改进还是恶化?」→ 指示项目健康度
   - 「哪个 gate 的 FAIL 率最高且与真实 bug 相关性最强?」→ gate 质量指标

2. **`forge retrospective` 命令**:每次 evolve 结束后,运行后评价:
   ```bash
   $ forge retrospective --session latest
   forge retrospective:
     session: evolve build #12 (2026-07-10)
     duration: 47m 32s
     iterations: 6 (converged at iteration 4, 2 extra due to loop-back)
     total cost: $2.84
     phase breakdown:
       planner:      2 calls, $0.31, 0 loop-backs
       implementer:  8 calls, $1.82, 3 loop-backs (most: reviewer gate)
       reviewer:     4 calls, $0.71, 0 loop-backs
     bottleneck: "implementer phase — gate feedback integration is the chokepoint"
     suggestion: "consider adding a pre-review lint gate to catch formatting before reviewer"
   ```

3. **治理改进建议**:基于跨 session 的模式,推荐治理优化:
   - 「你的项目 80% 的 gate FAIL 是 lint 问题 → 建议在 implementer phase 加 pre-commit hook」
   - 「每次 evolve 迭代 1 和 2 都在修同一类问题 → 建议添加对应的 lint rule」

### 边界情况

| 场景 | 行为 |
|------|------|
| 首次 evolve(无历史数据) | retrospective 报告「no data」,不产生建议 |
| 数据样本不足(<3 session) | 标注「low confidence」,不产生建议(与 historyMinSamples=20 模式一致) |
| 建议可能不准确 | 诚实标注为「suggestion」而非「diagnosis」,agent 自主判断 |
| 用户不需要自评价 | retrospective 是可选命令,不影响现有 evolve/run 行为 |
| 多团队共享 ForgeOS 实例 | session 按 team 和 project 聚合分析 |

### 与已有覆盖的差异

- `forgotten-five-structural-debt.md` 方向三「长运行时存储累积」关注的是 trace/memory/checkpoint
  文件的磁盘增长问题。本文关注的是**这些已有数据的分析利用**,两者是完全正交的问题——一个是存储
  管理(减负),一个是数据利用(增值)。
- 所有已有的 scorecard 相关分析(>8 方向)都将 scorecard 视为**模型路由的数据源**,从不讨论将同样的
  分析框架用于 ForgeOS 自身的效能评价。
- `truly-novel-five-directions.md` 方向二「自引用结构健康仪表盘」与之最接近,但方向二关注的是
  **代码结构指标的趋势**(文件数增长/扇入变化/包大小),而非**运行时行为模式的分析**(哪些 phase
  最容易 FAIL/loop-back/超时/烧预算)。两者一个是静态架构的追踪,一个是运行时行为的回顾性分析。

---

## 总结

| # | 方向 | 类型 | 优先级 | 预估工作量 | 杠杆 | 已有最近覆盖 | 差异化 |
|---|------|------|--------|-----------|------|------------|--------|
| 1 | Post-Acceptance 治理管线 | 产品/架构 | P1 | ~2–3 sprints | ⭐⭐⭐⭐⭐ | SBOM/供应琏(v44) | 部署≠签名,聚焦于扩展 workflow 模型 |
| 2 | 跨会话记忆传递 | 可靠性/知识 | P2 | ~1.5 sprints | ⭐⭐⭐⭐ | 可移植工作空间(方向1) | 知识继承≠文件可移植 |
| 3 | Phase 模板库与复用 | 架构/产品 | P2 | ~2 sprints | ⭐⭐⭐⭐ | Workflow 组合代数(v46) | 模板复用≠管线组合 |
| 4 | 组织多租户策略继承 | 架构/规模化 | P1 | ~3–4 sprints | ⭐⭐⭐⭐⭐ | 治理熵(v45)/联邦(federation) | 策略层级≠跨仓编排 |
| 5 | 自评价元认知循环 | 治理/可观测 | P3 | ~1.5 sprints | ⭐⭐⭐ | 结构健康仪表盘(方向2) | 行为分析≠结构追踪 |

---

*本分析基于 2026-07-10 工作树 (commit HEAD, Sprint 31+ after state)。  
已在 ~100+ 份已有 `docs/requirements/` + `docs/analysis/` 文档中逐篇交叉验证每个方向的新颖性。  
免责声明:ForgeOS 的分析文档数量庞大(~100+),尽管做了系统的关键词和语义检索,仍有个别方向可能
在某个未被遍历到的子文档中有类似提及。如有遗漏,欢迎补充定位。*
