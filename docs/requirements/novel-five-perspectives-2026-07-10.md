# ForgeOS — 全局深扫后的五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 
> 1. 全局深扫 forge-core（18+ Go 包 · 32k LOC）、cmd/forge（16+ CLI 子命令）、
>    harness（39+ 模块 · 10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 
>    5 工作流 · 全部 ADR+DECISIONS+architecture）、examples/（url-shortener · go-taskd）
> 2. 逐篇通读 & 交叉验证: 36 份 `docs/requirements/*.md` + 40 份 `docs/analysis/*.md` + 
>    其他 docs 共 ~80 份已有分析文档（Sprint 1–31 完整演进 · FUNCTIONAL_REQUIREMENTS_AUDIT
>    · 根 ROADMAP · loop-engineering · north-star · ha-security-rollout）
> 3. **差异化证明**: 每个方向附「与已有 70+ 分析的核心区别」，并引用代码级证据，
>    说明为什么它是高价值、未被覆盖的真实缺口
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 全景映射

ForgeOS 已有分析覆盖了极其广阔的表面。下表列出**最密集覆盖域**与本文方向的相对位置:

| 已有分析高密度覆盖域 | 代表性文档 | 约覆盖方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行） | `high-value-extension-directions.md` · `v3` · `v33` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `v34` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md` · `v34` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` 方向一二 | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` · `v26` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/并行安全） | `strategic-extensions-v22~v33.md` · `v38` · `v25` | ~12 |
| 北极星桥梁（Temporal / OPA / OTel / 多厂商 / Sandbox） | `v2-to-northstar-gap.md` | ~6 |
| 被遗忘基础（跨进程守护/热加载/Trace CLI/可插拔扩展/状态自校验） | `forgotten-five-foundations.md` | ~5 |
| Loop 方法论 + 收敛控制 + Reflect 步 | `loop-engineering.md` | ~4 |
| 其他高频主题: 执行器多样性/测试质量/成本推演/ADR 执法/跨相位故障归因/自愈运行时 | 各单篇文档 | ~15 |

**本文的 5 个方向均落在上述密集覆盖域之外** —— 有些是已有分析**完全未触及**的领域,
有些是**浅提及但从未作为独立方向展开**的深缺口。每个方向附差异化证明。

---

## 方向一 · 跨项目治理策略漂移检测与调和

**优先级**: 🟠 P1 | **类别**: 治理 · 工程化 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 通过 `forge-init` 为新项目**复制完整治理模板**（`.agent/` · `harness/` · 
`CLAUDE.md` · CI runner）。这是一次性快照。但 forge-core 自身在持续演进 — `modes.yml`
增加新维度、`check.py` 增加新检查、`policies.yml` 调整 enforce 阈值。已创建的 child 
项目并不知道这些上游变化:它们的 `.agent/policies/modes.yml` 停留在初始化瞬间的状态。

当前没有任何机制来检测、比较或调和这种漂移。这不是「多仓库联邦」——那是关于在多个仓库
间编排工作流。这是更基础、更安静的**治理版本锁定**:项目随时间的治理「债务」无声积累,
而 ForgeOS 作为治理 OS 的核心承诺（一致、可演化的治理基线）在这里是不连续的。

### 代码级证据

1. **`harness/scaffold/forge-init.mjs`** 在初始化时从 `COPIED_FILES` 列表复制文件:
   - 遍历 COPIED_FILES → `cp(src, dest)` — **纯单向同步,无版本标记,无后续 diff**
   - 自测 (`test_forge-init.mjs:108`) 仅检查 **"每个源文件被复制或列入白名单"**,
     不检查 child 是否与当前上游一致
2. **`project.yml`** 无 `governance_version` 或 `origin_ref` 字段承载父项目身份:
   - `forge-core/cmd/forge/main.go:projectYAMLValue` 只读 `lifecycle` / `mode` / 
     `features` — 无用于辨识治理来源或版本的字段
3. **`harness/check.py`** 有 9 项治理完整性检查,但**全是对 child 自己的 `.agent/` 内部
   一致性检查** — 无任何跨项目比较（"本项目的 gate_set 是否偏离了 origin/forge-core
   的 canonical gate_set"）
4. **`forge migrate --to engineering`** (`internal/migrate/migrate.go`) 能迁移单个
   项目的 mode/lifecycle,但**不关联父项目** — 迁移是本地快照,不是拉取上游策略
5. **`.arch/rules.yaml`** (`harness/arch/arch-check.mjs:58-62`) 的阈值是项目自带的
   硬编码值,没有 `inherits: forgeos/canonical` 分层机制
6. **已有一个相关但不同的概念** — ADR-0003（agent-os repo extraction）讨论用 submodule
   共享.gate资产,但那是关于**资产复用**而非**策略漂移检测**。两者正交。

### 与已有 70+ 分析的核心区别

- `forgotten-five-foundations.md` 方向二（治理热加载）讨论的是**运行时更新**已加载的
  策略,不是**跨项目策略一致性**。
- `strategic-extensions.md` 方向三（配置一致性守卫）聚焦于单个项目的配置跨文件一致性,
  不是「origin vs child」的治理漂移。
- `five-genuinely-uncovered-frontiers.md` 方向五（治理测试框架）提到「声明 vs 实现」
  的一致性问题,但局限于**单个项目**范围内。
- 唯一提及「跨项目」的是 `expansion-core-five.md` 方向一（多项目拓扑编排）,但那是
  关于**工作流编排**（A 项目 build 完→B 项目 deploy）,不是治理基线漂移。

### 为什么需要它

没有治理漂移检测,ForgeOS 的「一次初始化,持续治理」承诺在半程断裂:当上游收紧 secret
扫描规则或新增安全检查,已存在的项目在暗中裸奔。该特性是治理即代码生态中自动合规的
最低要求 —— 与 Kubernetes 的 PodSecurityPolicy 迁移到 PodSecurity Admission 需要
版本迁移工具一样必要。

**高价值场景**:
- 安全团队更新了 `secret-scan.mjs` 的模式列表 → 所有旧项目需要知道自己落后了
- `modes.yml` 新增了 workflow_depth 维度 → 旧项目的 mode 行为与实际 forks 配置不一致
- `check.py` 新增检查 → 旧项目仍在用旧版,新 bug 模式不被捕获
- 审计需要证明「项目的治理基线在特定日期与上游版本 X 一致」

### 具体方向建议

- **治理版本标记** (`governance_version` / `origin_ref`): project.yml 记录初始化时
  的 forge-core commit/version + 模板快照 hash
- **漂移扫描 CLI** (`forge governance check --drift`): 比较 child 的治理文件与
  上游 canonical 模板,输出差异清单（新增检查/阈值变化/缺失的安全规则）
- **迁移/调和路线** (`forge governance sync`): 将上游策略安全地合并进 child,
  保留项目特定覆盖（类似 git merge 但作用于治理 YAML）
- **漂移预算**: 允许项目声明「本偏离已知并经批准」,像 CVE 例外列表一样管理

---

## 方向二 · 事件驱动 / 定时执行平面

**优先级**: 🟠 P1 | **类别**: 平台 · 自动化 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的全部执行是**人工显式触发**的 —— 用户在终端输入 `forge run` / `forge evolve`
并等待它完成。没有任何机制能让 forge 在以下场景自动激活:

- **定时**: 每晚自动跑一次 `forge evolve build` 让 AI 持续推进 backlog
- **事件**: 当 GitHub PR 被合并时自动触发 `forge evolve build` 验证主分支健康
- **信号**: 当 CI 失败或依赖漏洞被披露时自动启动修复 workflow
- **编排**: 当项目 A 完成 build 时自动触发项目 B 的依赖更新

这不是北极星架构中「API Gateway/BFF」或「Web UI」的替代 —— 这是一个更基础的能力:
**ForgeOS 作为自治 AI 软件工厂,应该能独立于人类主动开始工作,而不是永远等待人类输入
`forge` 命令**。整个「24h 无人值守自治」的 vision 当前在事件入口处就是断裂的 ——
你能连续跑 24h,但不能让它明天再跑一次。

### 代码级证据

1. **所有执行入口在 `cmd/forge/main.go:69-76` 的 `subcommands` map 中**:
   - 全部是 CLI 子命令（run/evolve/gate/check/accept/route/migrate/detect/…）
   - **零个是守护进程、零个注册了 HTTP/Unix socket listener、零个读取 cron 配置**
2. **`internal/orchestrator/loop.go:LoopEngine.Run`** 可以实现完整的自治循环,
   但它必须以 `forge evolve` CLI 调用启动 —— 包含 `flag.Parse` / 读取流 / 等待完成
3. **checkpoint + memory + trace 基础设施**（`internal/persist` · `internal/memory` ·
   `internal/trace`）**全部是文件 I/O,不是流式/消息驱动的** —— 它们写入 `.forge/`
   目录,没有发布事件的通道（无 NATS 生产者、无 webhook 回调）:
   - `trace.go:Emit` 写入 JSONL 文件后不通知任何消费者
   - `checkpoint.go:Save` 更新后不广播到任何协调者
   - `memory.go:Append` 追加后不产生任何信号
4. **`internal/converge` 仅用于单次执行的收敛判定**,没有「持续监视图腾柱,
   当一组信号满足时自动触发工作流」的原语
5. **`harness/acceptance.mjs` 与 `gate.mjs`** 是同步 CLI,不是可以被事件驱动的
   worker 进程

### 与已有 70+ 分析的核心区别

- `expansion-horizon-three.md` 全文提到「事件驱动」,但那是作为**架构目标**
  （north-star 的 Temporal + NATS + 事件溯源）,讨论的是编排层自身的内部架构,
  而不是 ForgeOS**作为系统对外提供事件驱动的执行能力**。
- `fresh-expansion-perspectives.md` 方向一（DevOps 管道集成）提到 CI webhook,
  但那聚焦于「外部 CI 调用 forge」的单向通知,不是 forge 自己持有一个执行平面。
- 所有已有分析讨论的「自动化」都是**执行阶段内**的自动化（workflow 内的 phase 
  编排、loop-back、收敛）—— 不是**跨执行**的自动触发调度。
- 北极星架构的「Orchestrator (Temporal)」解决的是持久化工作流引擎,但那是重量级
  微服务架构（v3 水平）。这里讨论的轻量级执行平面是 v2 当前即可增量实现的:
  一个 `forge daemon` 子命令 + 一个 cron 风格的定时器 + 一组 webhook 接收器。

### 为什么需要它

「24h 无人值守」不应是 24 小时前人工敲一次回车。ForgeOS 真正的价值在于:**当约束条件
满足时自动展开工作**。一个持续集成、持续治理、持续演化的系统不应该在人类睡觉时
静默 —— 它应该在人类睡觉时工作。

**高价值场景**:
- 每晚 03:00 `forge evolve scan` → gap-analysis → 早上 human 看到一份「昨日进展」
- PR 合并后 `forge run review --approved` 自动审阅变更对架构的影响
- SBOM 更新触发 `forge run security-review` 和 dependency-scan
- 当 Roadmap 完成度突破 80% 时自动升级 lifecycle→growth

---

## 方向三 · 非确定性收敛停滞检测与分化诊断

**优先级**: 🔴 P0 | **类别**: 可靠性 · 诊断 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的收敛判定建立在两个信号上: `roadmap_completion == 1.0` 和
`gates_green == true`。当这两个条件不满足时,系统继续迭代 —— 通过 planner →
implementer → gates → reviewer → qa 循环,期望 agent 在每次迭代中推进进度。

但存在一类重要的**非确定性停滞**:agent 产出的代码在行为上是正确的、通过测试的,
但恰好无法让 ROADMAP 的某个 `[x]` 被勾选（因为勾选依赖 agent 自述的完成,而
agent 的诚实自省机制在特定条件下失效）,或恰好让某一项 gate 因为与代码无关的
原因保持红色（如 yaml2json 的 block-scalar bug — 一个不属于业务逻辑的解析错误
堵死了整个收敛路径）。

当前系统没有能力**区分**「主动作失败」（代码写错、测试不绿）和「非确定性停滞」
（代码正确但信号传导路径断裂）。在第一种情况下,**继续迭代是正确策略**（agent 
修复代码）。在第二种情况下,**继续迭代是浪费预算**并可能形成死循环 —— 同一个
agent 在不自知的信号盲区中反复产出相同质量的代码,因为它**无法诊断自己为什么
还「没完成」**。

### 代码级证据

1. **`internal/converge/converge.go` `Evaluate` 方法是布尔合取**:
   - 它只回答「MET? YES/NO」,不产生产出 "为什么停滞" 的诊断元信息
   - `evalRoadmap` 返回 `met` + `detail`(`"90%"`),但不分析为什么最后 10% 
     无法被推进（是 agent 没写代码?还是写了但无法自勾?）
   - 没有「同一信号在 N 次迭代中无变化」的停滞模式检测
2. **`internal/orchestrator/loop.go:106-114` 的 `NoProgress` tripwire**:
   - 只在 `gates_status` 或 `roadmap_completion` 在 3 次迭代后无变化时触发
   - **但它不区分停滞的原因** — 把它视为 fail-safe 熔断,不是诊断信号
   - 触发后打印 `no-progress tripwire after 3 stale increments` 就 abort,不生成
     诊断报告
3. **`cmd/forge/gates.go:gatherSignals` 及相关的信号提取**:
   - `FileDelta`（已实现）可以检测到 agent 没写文件（停滞极端情况）
   - **但没有任何信号分析器**查看「FileDelta > 0 但 roadmap 没动」的组合模式
     —— 这正是「写了代码但自勾机制坏了」的指纹
4. **`forge doctor`** (`internal/doctor/`) 是诊断工具,但它是**事后静态分析**:
   - 不做运行时诊断
   - 不检测收敛停滞
   - 不分析迭代间的信号差分
5. **Sprint 29 的诚实日志**已经暴露了一个真实案例:FileDelta 为零时 route 告警
   「agent 自报可能夸大」—— 但这是被动告警,不是主动停滞诊断

### 与已有 70+ 分析的核心区别

- `edgecases-and-perf.md` §1.1 提过「收敛理论陷阱」,但那是关于「合取停止条件
  的理论充分性」,不是运行时诊断。
- 所有已有分析讨论的「停滞检测」都是**资源层面的**（no-progress tripwire 防
  doom-loop）或**机械层面的**（budget exhausted stop）,不是**认知层面的**
  （信号传导路径断裂的诊断）。
- `loop-engineering.md` 定义了收敛控制的完整方法论,但其「Reflect 步深化」
  蓝图讨论的是agent 层面的自反思,不是信号传导层的自诊断。
- `expansion-core-five.md` 方向四（自愈循环）讨论自动修正 ROADMAP 条目,
  但它假设的是 ROADMAP 不可达（条目设计错了）,不是信号本身坏了。
- `second-order-architectural-gaps.md` 方向二（无声数据丢失）是最接近的,
  但聚焦在数据持久化的无声损坏,不是收敛信号的无声断裂。

### 为什么需要它

ForgeOS 收敛控制的基石是**诚实信号**:如果信号是坏的,整个收敛算法退化为盲人骑瞎马。
Sprint 26-27 的真实教训是:一个 yaml2json 的解析 bug（block-scalar 损坏）让所有
workflow 的 `description` 字段被静默注入垃圾前缀,agent 一直在错误的 prompt 下工作
—— 这不是 agent 蠢,是信号坏了一直没人知道。

**高价值场景**:
- agent 连续写了 5 次迭代的代码,ROADMAP 停在 92% —— 需要诊断是缺测试?还是
  自勾机制失效?还是 ROADMAP 条目不可达?
- gate 连续 3 次 PASS 但 converge NOT MET,因为某个 `evalOne` 分支的源数据
  一直为空（像 Sprint 29 的 FileDelta 恒为 0）
- 需要迭代间信号差分:对比 iter N 和 iter N-1 的完整信号剖面,自动标注变化/不变
  的信号,缩小停滞的根因范围

---

## 方向四 · 治理系统自身免疫测试

**优先级**: 🟠 P1 | **类别**: 质量 · 基础设施 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的核心价值主张是带外治理 —— harness 是「源头真相」「载重墙」,`gate.mjs`/
`check.py`/`arch-check.mjs`/`secret-scan.mjs` 的裁决决定了 CI 是否通过、agent 是否
继续工作。但 **who guards the guardians?**

当前,没有系统性的机制来验证 harness 自身的正确性:
- 如果在 `gate.mjs` 中引入了一个 bug,让门永远 PASS,没有任何自动检测能发现
- 如果 `arch-check.mjs` 的 `checkFanin` 开始漏报（像 Sprint 26 之前误把测试
  文件记入扇入计算）,唯一能发现的是它在误报之后**人工**审核发现的
- 如果 `check.py` 的 `check_workflow_control_flow` 因为 PyYAML 解析行为变更
  而开始误报/漏报,当前没有回归测试捕获它
- 如果 `secret-scan.mjs` 的正则模式出现错误,且恰好没有使用该模式的代码
  提交,缺陷将被永久静默

这不是「forge accept 跑测试」的概念 — forge accept 测试的是**被治理项目**的
健康。这是**治理系统自身的健康**。一个工程成熟度跨越某个阈值后,元治理（对治理
代码的自动化防御）成为必需品。

### 代码级证据

1. **各 harness 工具的测试覆盖其自身逻辑**:
   - `test_gate.mjs` 8 测试 — 测试 gate.mjs 的逻辑
   - `test_check.py` ~24 测试 — 测试 check.py
   - `test_arch-check.mjs` — 测试 arch-check
   - `test_secret-scan.mjs` — 测试 secret-scan
2. **但是没有任何测试测试「如果 gate.mjs 自己坏了会怎样」**:
   - 没有**突变测试**:篡改 gate.mjs 的阈值或逻辑,验证其他测试能捕获破坏
   - 没有**属性测试**:随机生成不同的项目结构,验证 gate 的行为一致
   - 没有**自举测试**:用 forge 自己治理 forge 的治理代码
3. **`internal/doctor/` 可以诊断项目**,但它是外部诊断 —— 不诊断 harness
   自身:
   - `doctor.go` 检查 workflow/agent 引用
   - `anomaly.go` 检查异常模式
   - **没有 harness 自检子命令**
4. **`harness/acceptance.mjs`** 聚合全闸门但依赖**闸门自身的完整性**:
   - 如果 `gate.mjs` 输出 `PASS` 但错了,acceptance 也会 PASS
   - 没有独立验证者验证验证者
5. **`harness/arch/arch-check.mjs`** 单独有 8 检查,但它的 `scan.mjs`
   在 Sprint 26 被真实发现误把测试文件计入生产扇入 —— 这个 bug 存活了
   多久?不被发现的 bug 存活周期是未知的。

### 与已有 70+ 分析的核心区别

- `forgotten-five-foundations.md` 方向五（运行时状态自校验与恢复）讨论的是
  **forge-core 运行时**的状态校验（checksum + cross-file consistency）,
  不是**治理工具自身**的元测试。
- `expansion-production-readiness.md` 方向三（环境验证与预检查）讨论 forge
  运行前的环境检查（Node/Python/claude 是否可执行）,不是治理代码本身的正确性。
- `novel-five-highvalue-extensions.md` 方向一（治理策略测试框架）讨论的是给
  用户测试自己的治理策略的框架,不是 forge 自身治理代码的元测试。
- 已有 doc `self-testing-and-dogfooding.md` 讨论过**用 forge 治理 forge**,
  但那聚焦于「forge-init 复制自身治理」和「gate.mjs 对本仓库执行」,不是
  对治理代码进行 fuzz/mutation/property 测试。
- `expansion-production-blindspots-v36.md` 方向一（编排器自测框架）是最近的
  也是最接近的,但它讨论的是编排器接口（orchestrator.Engine）的契约测试,
  不是 harness 工具的元测试。

### 为什么需要它

治理 OS 的信任基础是治理工具本身的可靠性。Sprint 26 揭示的 arch-check 误把
测试文件算入扇入的 bug 是一个真实警示:治理工具的 bug 不会被「被治理项目」暴露,
只会在错误裁决被人工发现之前默默影响所有 run。一个在快速迭代中每月有 3+ 次
harness 改动的系统（ForgeOS 现况）,需要元测试来确保改动不破坏治理基线。

**高价值场景**:
- **突变测试**:自动篡改 gate.mjs 的阈值（如把 500 行改为 50 万行）,验证
  test_gate.mjs 能 FAIL —— 如果 test PASS,说明测试没有真正守卫红线
- **差分测试**:对每个 PR 涉及 harness 的改动,运行完整的治理测试矩阵,
  对比改动前后的 gate 输出,标记意外漂移
- **自举**:用 forge 自己的 gate 治理 forge 自身的治理代码（如 gate.mjs 
  自己是否超过 500 行? `check.py` 是否被 `check.py` 的 rule 覆盖?）
- **故障注入**:在 CI 中有意破坏 harness 工具（如删除 `secret-scan.mjs` 的
  关键正则）,验证 `forge accept` 是否 FAIL

---

## 方向五 · 跨会话学习和知识迁移

**优先级**: 🔴 P0 | **类别**: AI · 学习 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 当前的学习闭环（`memory.jsonl` · `trace.jsonl` · `scorecards.json`）是
**单项目、单会话、单模型族**的:

- `memory.jsonl` 依附于 `.forge/` 目录,只在一个项目的 `forge evolve` 会话内
  有效;一次 `forge run build` 产生的教训不会转移给下一次 `forge run build`
- `scorecards.json` 记录某个模型在某类任务上的表现,但这些数据不会被跨项目共享
  —— 项目 A 学到「opus on security-review costs $0.18/run」,项目 B 是冷启动
- `trace.jsonl` 是纯事件日志,无提炼机制 —— 100 次 run 留下 100 个文件,每个
  需要人工 jq 才能提取洞察
- **跨项目组织学习不存在**:如果 forge 治理了 10 个项目,每个项目的 agent 行为
  模式、常见失败路径、最佳 tier 选择都只能从零开始学习

这不是「知识引擎/RAG」（那是一个北极星服务）。这是一个更基础的问题:ForgeOS 
的循环（Eval→Scorecard→Router）在**单项目边界内**已经接通（Sprint 24-26 坐实
三维数据落盘 + HistoryTiebreak v1.5 多候选路由）,但它在**跨项目/跨时间/跨会话**
边界上是**完全空的**。每次 `forge run` 都是一次礼貌的遗忘。

### 代码级证据

1. **`internal/memory/memory.go` 的 `loadCache` 是进程内缓存** (`sync.Map`):
   - 它的设计注释明确写「两个 forge 进程在不同项目上不会互相碰撞」
     见 `// Use sync.Map so concurrent forge processes on different projects...
     方向3: global cache collision` — 已经承认跨项目隔离是**设计目标**,
     不是未来扩展
   - 没有共享 memory 的后端,没有与外部存储交换数据的通道
2. **`internal/trace/trace.go` 的 `Tracer` 写入单一文件**:
   - `traceFile := filepath.Join(forgeDir(root), "trace.jsonl")` 硬编码到项目
     目录
   - 无 trace 汇总/聚合器,无跨 run 的 trace 联合分析
3. **`internal/routing/scorecard.go` 的 `LoadScorecards` 读本地 JSON**:
   - `scorecardPath` 默认为项目内的 `.agent/routing/scorecards.json`
   - `HistoryTiebreak` 只在本项目的 scorecards 上运行
   - 没有一个 `ScorecardRegistry` 可以从共享存储加载跨项目数据
4. **`cmd/forge/scorecard_wind.go` 的 `runScorecardUpdate`** 写入后不做
   任何广播 —— 数据就像日记一样,写在只有自己能看到的笔记本里
5. **`internal/prompt/retrieve.go`** 实现了 TF-IDF 检索,但它的语料库是
   单项目的 `.agent/` 文档 —— 不是跨项目的知识库
6. **`internal/converge`** 的收敛判据是项目本地的 ROADMAP + gates —— 
   无法将「项目 A 遇到的常见收敛陷阱」应用到项目 B

### 与已有 70+ 分析的核心区别

- `strategic-extensions-v24.md` 方向二（跨项目记分卡共享）是最接近已有的
  分析,但它聚焦于**记分卡数据的共享机制**,不是完整的跨会话学习。
- `strategic-extensions-v23.md` 方向二（知识冻结与增长瓶颈）提到跨项目
  知识,但是作为「增长瓶颈」的一个子点,不是作为一个独立的高价值方向。
- 所有已有「学习闭环」相关分析（`high-value-extensions.md` 方向二、
  `loop-engineering.md` 的飞轮蓝图等）都讨论单向合学习（Eval → Scorecard → 
  Router）,且所有讨论都在**单一项目/单会话的假设**下进行。
- `expansion-horizon-three.md` 方向三（跨会话知识图谱）是最相关的已有
  方向,但它讨论的是**agent prompt 级别**的知识注入（什么是好的架构、
  什么测试模式适合这个项目）,不是**系统级别**的学习（什么 model tier 对
  哪种任务性价比最高、什么 gate 组合对某类项目最有效）。
- 核心独特性:这不是「agent 的知识」,这是「ForgeOS 对自身的了解」—— 
  系统对自身在不同项目上运行效果的积累知识。

### 为什么需要它

ForgeOS 的核心护城河是「越用越聪明」的数据飞轮。但如果这个飞轮在每次 `forge run`
后重启,那不叫飞轮,叫暖手。当 forge 治理 10+ 项目后,它应该能自动回答:

- 「security-review gate 在 Python 项目上通常 FAIL 两次才 PASS,所以默认设
   max-retry=3 比 max-retry=1 更划算」
- 「opus-on-reviewer 在项目 A 的成本是 $0.18/run,和项目 B 一样 —— 不必为
   项目 B 的第一次 review 用 sonnet 实验」
- 「80% 的收敛停滞由 test gate 引发,所以应该先优化 test suite 而不是加 reviewer」
- 「项目 A 从 explorer→engineering 迁移耗时 2 sprints,与项目 B 的模式一致」

**高价值场景**:
- 跨项目记分卡聚合:根据所有已治理项目的经验,为新项目推荐初始 model tier 
  和 gate 配置
- 跨会话 memory:上周 evolve 跑到的结论（"项目的 flaky test 集中在 auth 
  模块"）本周继续有效
- 模式发现:在多个项目中自动识别「gate X 的失败经常伴随 ROADMAP 勾选率低」
  的关联模式
- 冷启动加速:新项目的初始 `--max-agent-calls` 和 `--timeout` 等参数基于
  历史数据而非固定默认值
- 治理知识库:项目 A 在迁移 lifecycle 时遇到的具体问题,在项目 B 遭遇相同
  迁移时主动预警

---

## 汇总

| # | 方向 | 优先级 | 类别 | 预估工作量 | 核心差异化 |
|---|------|--------|------|-----------|-----------|
| 1 | 跨项目治理策略漂移检测与调和 | P1 | 治理 | ~2 sprints | 所有已有分析讨论「配置一致」在单项目内;本文讨论 origin-vs-child 的跨项目治理基线 |
| 2 | 事件驱动/定时执行平面 | P1 | 平台/自动化 | ~3 sprints | 已有分析讨论「自动化」都是在单个执行周期内;本文讨论跨执行周期的自动触发 |
| 3 | 非确定性收敛停滞检测与分化诊断 | P0 | 可靠性/诊断 | ~1.5 sprints | 已有分析的停滞检测全是资源层面/机械层面;本文讨论信号传导路径断裂的认知层诊断 |
| 4 | 治理系统自身免疫测试 | P1 | 质量/基础设施 | ~2 sprints | 已有讨论的是治理策略测试框架或状态自校验;本文讨论治理工具自身的元测试 |
| 5 | 跨会话学习和知识迁移 | P0 | AI/学习 | ~3 sprints | 已有「学习闭环」全在单项目/单会话假设下;本文讨论 ForgeOS 对自身的跨项目知识积累 |

---

## 收敛建议

**若只做一件**:方向三（非确定性收敛停滞检测）—— 成本最低（约 1.5 sprints,
主要在原已实现的信号框架上叠加诊断层）,解决的是真实 P0 bug 类别（Sprint 29 
的 FileDelta 假阳信号就是活体案例）。这个方向的缺失直接降低 ForgeOS 收敛控制的
可信度 —— 当系统可能卡死在不自知的信号断裂中时,「自治」的承诺受质疑。

**做前三件**:方向三 + 方向五 + 方向一 —— 分别闭合「收敛诊断自愈」「跨项目
学习」「治理基线一致性」,三者共同奠定了 ForgeOS 从「单项目工具」向「多项目
治理平台」演化的基础。其中方向五（跨会话学习）投入最大但杠杆最高 —— 它把
ForgeOS 从每 run 冷启动推进到「经验积累」的飞轮初态。

**全部五件**:方向二（事件驱动平面）是基础平台能力,方向四（自身免疫测试）
是质量保障。方向二是方向五的使能器（跨会话学习需要持续运行来积累数据,
手动触发不够）;方向四是所有方向的必要前提（治理工具的正确性是 ForgeOS 
可信度的根基）。
