现在我已深入理解原始文档、审计报告和代码库现状。以下是我的架构分析。

---

# 架构分析：ForgeOS 五方向架构盲区

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在以下方面展现了极高的工程成熟度：

| 优势领域 | 具体表现 |
|---|---|
| **分层纯度** | `forge-core`（Go 运行时，零外部依赖）与 `harness`（Node/Python 治理层）之间边界清晰，`internal/` 包禁止被 `cmd/` 以外导入 |
| **可验证架构** | `internal/adr/adr_test.go` 已将 ADR 约束转化为 falsifiable 测试（`TestADR0001_ZeroExternalDeps` 等），验证了架构决策与代码的一致性 |
| **中央旋钮** | `mode × lifecycle` 双轴驱动 Router、Harness 严格度、Workflow 深度，是一个设计精巧的配置收敛点 |
| **持久化诚信** | `persist` 包采用原子写 + 显式损坏报告，不静默丢弃数据 |
| **测试基础设施** | 三层测试（Go 单元测试 + JS 治理测试 + 自举 acceptance 测试）覆盖面广 |
| **治理执法** | 8 项架构检查、10 项治理检查、secret 扫描、SCA 等形成结构化闸门 |

### 1.2 架构局限性与技术债务

| 债务类型 | 具体表现 | 严重程度 |
|---|---|---|
| **接口契约隐式化** | Agent 执行器接口（`BuildArgv`/`ParseCost`/`ParseVerdict`）散落在 `engine_build.go`/`cost.go`/`.agent/agents/reviewer.md` 中，100% 耦合于 Claude CLI | 🔴 架构性 |
| **治理元循环** | Harness 测试在自身仓库上运行，`gate.mjs` 的 bug 会阻断对 `gate.mjs` 自身的修复 | 🟠 可靠性 |
| **状态目录碎片化** | `.forge/` 下四种持久化数据（trace/memory/checkpoint/scorecards）各有不同的版本策略（checkpoint 有、trace 有、scorecards 无）和生命周期策略（无统一清理） | 🟠 可运维性 |
| **治理范围≠代码范围** | `pi-batch.py`（470 行独立 Python 执行器）无 arch-check、无 acceptance app-test、无独立测试 | 🟢 一致性 |
| **ADR 生命周期盲区** | ADR 测试已存在，但 ADR 状态机（Proposed→Accepted→Superseded→Deprecated）和自动漂移检测仍在缺失 | 🟠 治理完整性 |

### 1.3 关键设计决策评估

| 决策 | 评价 | 建议 |
|---|---|---|
| Go 核心 + 零外部依赖（ADR-0001） | ✅ 正确的平台决策——确保编译速度、静态分析可及性、部署简化 | 坚持不变 |
| Harness 用 Node.js（ADR-0002） | ✅ 合理的过渡选择——利用 Node 生态的原生 JSON/YAML 直觉 | 考虑 v3 将 Python 治理工具也纳入统一框架 |
| CommandExecutor 直接构造 Claude argv | ⚠️ 权宜之计已过时——项目愿景声明「站在所有 CLI 之上」，但只有一个适配器 | 必须引入适配器接口 |
| Checkpoint format version + retain=5 | ✅ 已经做对的——说明团队认识到了版本兼容的重要性 | 扩展至统一生命周期框架 |
| `mode × lifecycle` 双轴旋钮 | ✅ 优雅的设计——一个变量变化可以级联影响路由、严格度、深度 | 保持稳定 |

---

## 2. 扩展方向

### 方向 A：Agent 适配器契约化（对应原方向一）

**优先级：🔴 P0 | 杠杆：⭐⭐⭐⭐⭐**

**为什么需要：**
- 项目 ROADMAP 承诺「站在 Claude Code / Codex / Gemini CLI / OpenHands 之上」，但当前架构 100% 耦合于 Claude CLI 私有语法
- 新 CLI 适配器的开发成本 = 逆向工程 `engine_build.go` + `cost.go` + `prompt_memory.go` + `.agent/agents/reviewer.md` 的隐式契约
- 这是一个**产品问题**而非代码组织问题——多 CLI 愿景的架构前提不成立

**核心挑战：**
1. **契约边界划定**：哪些是 CLI 特有的（flag 语法、输出 JSON schema、成本字段名），哪些是 ForgeOS 通用的（prompt 构造、phase 拓扑、裁决语义）——二者当前深度纠缠
2. **裁决契约的机器可读化**：`VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` 现在定义在 `.agent/agents/reviewer.md` 的散文中，不是 schema。需要从散文→结构化契约，且不破坏现存 review 流程
3. **多语言适配器注册**：适配器在 Go 中定义（`internal/adapter/`），但 CLI 检测（`detect.go`）在 Go 中，而部分适配逻辑（如 fixture 测试）可能在 harness 中——注册点在哪里？

**预期架构变更：**
```
forge-core/internal/
  adapter/             ← 新增包
    adapter.go         ← AgentAdapter 接口定义
    claude.go          ← ClaudeAdapter 实现（从 engine_build.go/cost.go 提取）
    registry.go        ← 注册 + 按 agent-cmd 查找
  orchestrator/
    executor.go         ← AgentExecutor 接口保持，内部委派给 AgentAdapter
```

**对现有系统的影响：**
- 低风险——适配器是从现有代码提取出来的，不改变 orchestrator 的核心状态机
- 需要确保 `echo` 作为 agent-cmd 的 DryRunExecutor 仍能工作
- 向后兼容：`--agent-cmd=claude` 自动选 ClaudeAdapter，行为不变

**设计选项：**

| 选项 | 描述 | 权衡 |
|---|---|---|
| **A1：最小接口** | 只抽象 `BuildArgv` + `ParseCost` + `ParseVerdict`，适配器编译进 forge 二进制 | 简单；不支持热插拔，但 v1 够用 |
| **A2：插件化注册** | 引入 registry，适配器通过 `init()` 注册或外部 `.so` 加载 | 灵活但复杂，且 `.so` 与 ADR-0001（零外部依赖）精神矛盾 |
| **A3：DSL 声明** | 用 YAML schema 声明 CLI 的 flag 映射、JSON 路径、裁决模式，运行时反射执行 | 声明式优雅但有性能损失和表达能力上限 |

**建议：选择 A1（最小接口）**，在 `internal/adapter/` 定义纯 Go 接口。等出现第一个非 Claude 适配器需求时再考虑 A2/A3。

---

### 方向 B：ADR 治理闭环升级（对应原方向二）

**优先级：🟠 P1 | 杠杆：⭐⭐⭐⭐**

**为什么需要：**
- 项目中**已经存在** `internal/adr/adr_test.go`（`TestADR0001_ZeroExternalDeps` 等 9 个测试）——说明团队认识到了 ADR 可执行化的价值
- 但 ADR 缺失**生命周期管理**：Accepted 状态永不到期，Superseded 机制不存在，ADR 与代码的漂移（如 ADR-0004 的 P1+P2 vs 实际 P1+P2+P4）只能靠人审
- 作为一个自称「治理 OS」的项目，对自己的架构决策没有治理能力是一个结构性矛盾

**核心挑战：**
1. **状态机设计**：ADR 状态需要哪些转换？Proposal→Accepted→Superseded 是最小集合；Deprecated 是否需要？Rejected 是否需要保留？
2. **漂移检测的范围**：哪些 ADR 决策可以被自动化验证？100% 不可能（如「命名要好」这种主观决策），30-50% 可能（如「零外部依赖」「cmd/forge 不导入 internal/asset 直接」），需要务实的分级策略
3. **与现有 `adr_test.go` 的集成**：不破坏现有 9 个测试，在它们之上叠加状态机和生命周期管理

**预期架构变更：**
```
forge-core/internal/
  adr/
    adr.go             ← 新增：ADR Registry + 状态机
    adr_test.go        ← 已有，保持
cmd/forge/
  adr_cmd.go           ← 新增：forge adr list/status/supersede/validate
```

**设计选项：**

| 选项 | 描述 | 权衡 |
|---|---|---|
| **B1：纯文件状态机** | ADR 状态记录在 YAML 文件头部（`status: superseded`, `superseded_by: ADR-0005`），`forge adr validate` 读取文件系统 | 简单无状态；但无法支持版本历史追踪 |
| **B2：Registry + JSONL** | 用 JSONL 文件（类似 trace/memory）记录 ADR 状态变更历史，`internal/adr/` 包维护运行时 Registry | 可追踪审计；引入新的持久化模式 |
| **B3：嵌入 gate 决策** | ADR 约束编译为 `gate.mjs` 可检查的规则，漂移检测成为 gate 的一部分 | 治理闭环最完整；复杂度最高 |

**建议：B1（纯文件状态机）先落地**，在 ADR 文件头部加 metadata 字段。B2 可在 v3 引入以支持审计追踪。B3 过度工程化，不适合当前阶段。

---

### 方向 C：状态目录统一生命周期（对应原方向四）

**优先级：🔴 P0 | 杠杆：⭐⭐⭐⭐⭐**

**为什么需要：**
- 审计报告确认：checkpoint 已有 `_format` 版本（`forgeos.checkpoint.v1`）+ `retain=5`——团队已经做对了一部分
- 但 **真正缺失的缺口** 确实存在：trace 二次旋转（仅启动时旋转一次，持续积累可达数 10MB）、memory 无 TTL、无统一 `forge cleanup` 命令、无配额告警
- 在 24h+ 无人值守的 evolve 运行中，`.forge/` 无限制增长是一个可靠性风险——磁盘满导致循环异常终止

**核心挑战：**
1. **统一框架 vs 各管各**：trace 是按大小旋转的、memory 是按条目数 compact 的、checkpoint 是按 retain 数保留的——生命周期策略不统一。强行统一可能丢失各状态类型的语义特异性
2. **迁移路径**：已存在的 `.forge/` 目录（包含旧格式 checkpoint、无版本 trace、积累的 scorecards）如何在升级时平滑迁移？
3. **与已有策略的衔接**：`project.yml` 中 `mode` 是否应该影响生命周期策略（engineering 模式下保留更长？production 模式下 TTL 更短？）

**预期架构变更：**
```
forge-core/internal/
  state/               ← 新增包
    state.go           ← StateDir 接口
    policy.go          ← 策略解析（从 project.yml 或新 .forge/policy.yml）
    cleanup.go         ← 清理引擎
cmd/forge/
  cleanup.go           ← 新增：forge cleanup 命令
  doctor/              ← 已有，增加状态目录健康检查
```

**设计选项：**

| 选项 | 描述 | 权衡 |
|---|---|---|
| **C1：最小收敛** | 只为 trace 加二次旋转 + memory 加 TTL + `forge cleanup` 命令，不引入统一框架 | 快速见效；但碎片化问题未根除 |
| **C2：统一 StateDir 接口** | 新增 `internal/state/` 包定义 `StateDir` 接口，trace/memory/checkpoint 都实现，共享生命周期策略 | 干净抽象；需要现有代码重构 |
| **C3：策略文件驱动** | 在 `.forge/policy.yml` 中声明所有生命周期参数，`forge evolve --cleanup` 按策略自动执行 | 声明式运维友好；引入新的配置依赖 |

**建议：C1 先落地（2 次 sprint）**，解决最迫切的 trace 二次旋转和 memory TTL。C2 可作为方向在 v3 实现。C3 过度设计——生命周期策略应保持在 `project.yml` 中，不再新增配置文件。

---

### 方向 D：自举测试隔离（对应原方向三）

**优先级：🟠 P1 | 杠杆：⭐⭐⭐⭐**

**为什么需要：**
- 审计报告确认核心论点：`forge accept` 的 test gate 执行 `node --test harness/`，其中包含测试 `forge accept` 自身行为的测试——循环依赖真实存在
- 虽然 `mkdtempSync` 已被部分使用，但 `test_acceptance.mjs` 的「acceptance gate ACCEPTS the repo it runs in」测试确实操作真实仓库
- 这种风险目前未触发，因为团队工程纪律较好。但系统**没有任何设计来优雅处理它**——一旦一个 gate bug 导致假阳性，修复路径只能是 `--bypass-gate`，损害治理信誉

**核心挑战：**
1. **边界定义**：哪些测试是「自举安全的」（可以在自身仓库上运行）？哪些是「循环依赖风险」的（必须去隔离环境运行）？
2. **fixture 仓库的维护成本**：为 `test_gate.mjs` / `test_arch-check.mjs` / `test_acceptance.mjs` 创建和维护合成 fixture 项目有持续成本
3. **CI 集成**：自举测试跑在 CI 的 `forge accept` 阶段，而隔离测试是否应该在不同阶段运行？

**预期架构变更：**
```
harness/
  test_fixtures/       ← 新增：存 fixture 项目模板（最小化的 .agent/ + 代码桩）
  test_acceptance.mjs  ← 修改：IS_INNER 保护 + fixture 优先
  test_gate.mjs        ← 修改：所有集成测试在 mkdtemp 中运行
```

**设计选项：**

| 选项 | 描述 | 权衡 |
|---|---|---|
| **D1：INNER guard 升级** | 将环境变量 `FORGE_ACCEPT_INNER` 升级为结构化配置，声明哪些测试需要隔离 | 增量改进，工作量小；但 fixture 问题未解决 |
| **D2：全 fixture 化** | 所有集成测试改为在 mkdtemp 目录 + fixture 项目上运行，禁止操作真实仓库 | 信度最高；fixture 维护成本高 |
| **D3：双模式运行** | CI 中先跑全 fixture 模式（确定系统无 bug），再跑 self-test 模式验证自举信度 | 覆盖最全；CI 时间加倍 |

**建议：D1 + D3 组合**。D1（结构化的 INNER 声明）在 1 sprint 内可落地。D3（双模式运行）作为长期目标在 v3 引入。

---

### 方向 E：治理收编与示例治理化（对应原方向五）

**优先级：🟢 P2 | 杠杆：⭐⭐⭐**

**为什么需要：**
- 审计报告确认：pi-batch.py 未违反 `max_root_files=15`（根目录 7 个文件），但确实**没有任何治理覆盖**——无 arch-check、无 acceptance app-test、无独立测试
- `examples/` 目录下的两个完整应用没有 `.agent/` 目录、没有独立 CI、无法演示治理循环
- 这对于一个以「治理」为核心价值的项目是一个一致性问题——你的范例应该体现你的产品能力

**核心挑战：**
1. **pi-batch.py 的归宿**：整合到 `forge-core/cmd/forge` 作为 `forge batch` 子命令（需要 Go 重写）vs 迁移到 `harness/` 作为治理工具
2. **Examples 的治理化成本**：为 `go-taskd` 和 `url-shortener` 生成 `.agent/` + CI 配置需要额外维护工作
3. **治理范围声明**：`project.yml` 是否需要声明哪些路径在治理范围内、哪些不在？

**建议：**
- pi-batch.py：移至 `harness/pi-batch.shim.py` 作为治理工具（非根目录独立脚本），补测试
- Examples：仅 `go-taskd` 添加 `.agent/` 目录，作为 `forge init` 对齐的示范；`url-shortener` 作为轻量级展示保持原样
- 1 sprint 可落地，P2 优先级合理

---

## 3. 接口设计建议

### 3.1 关键接口设计原则

| 原则 | 说明 |
|---|---|
| **显式契约优于隐式约定** | 当前 `VERDICT: APPROVE` 在 reviewer.md 散文中、`total_cost_usd` 在 claude JSON schema 中——都应转为结构化接口声明 |
| **诚信优先（Honesty-first）** | ForgeOS 已有这个文化（`persist` 包显式报告损坏而非静默丢弃）。适配器接口和 ADR 状态机应继承这一原则：不静默降级、不猜测、不出错返回 0 值 |
| **fail-closed** | 版本不兼容 → 拒绝运行，不静默加载；适配器找不到 → 报错，不降级到空操作 |
| **单一事实源** | 每个架构契约只有一个定义点（ADR 元数据在 `docs/adr/`，适配器接口在 `internal/adapter/`），不分散 |

### 3.2 是否需要新的抽象层

| 抽象层 | 需要 | 理由 |
|---|---|---|
| `AgentAdapter` 接口 | ✅ 必须 | 当前架构的致命弱点，限制多 CLI 愿景 |
| `StateDir` 接口 | ⚠️ 可选 | C1 方案不引入统一接口，C2/C3 才需要 |
| `GenericPolicy` 策略引擎 | ❌ 不需要 | `mode × lifecycle` 中央旋钮已足够，不需要另一个策略层 |
| 插件注册机制 | ⚠️ v3 考虑 | v1 编译时绑定即可 |

### 3.3 向后兼容性策略

1. **适配器接口**：默认 `ClaudeAdapter`，行为不变；未显式选择适配器时自动检测 agent-cmd
2. **ADR 状态机**：现有 ADR 文件自动获得 `status: accepted`（向后兼容），新增状态字段可选
3. **`.forge/` 生命周期**：`forge cleanup --dry-run` 先预览不删除；`forge migrate --state` 处理旧格式
4. **自举测试**：`FORGE_ACCEPT_INNER` 现有 guard 保持兼容，结构化配置作为可选升级路径

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

| 技术栈 | 评估 | 决策 |
|---|---|---|
| Go 插件（`plugin` 包） | 提供动态加载能力，但 Linux-only、对版本敏感 | ❌ 不符合 ADR-0001（零外部依赖）+ 过于复杂 |
| WebAssembly 适配器 | 理论上可跨语言调用，但引入 `wasm` 运行时依赖 | ❌ 过度工程化 |
| JSON Schema / CUE | 用于声明化适配器契约和 ADR 约束 | ⚠️ 可以考虑 CUE（Go 原生），但 v1 先最小化 |
| YAML-only（已有解析器） | 用 YAML schema 描述 CLI 映射，复用 `parseRules` | ✅ 零新增依赖，适合 v1 |

### 4.2 第三方依赖评估标准

ForgeOS 目前严格执行 ADR-0001（零外部依赖），这是正确的：

| 场景 | 判断标准 |
|---|---|
| 新增 Go 模块依赖 | ❌ 几乎总是拒绝——ADR-0001 是架构宪法 |
| 新增工具链依赖（如 CUE 二进制） | ⚠️ 仅当收益明显且可独立安装（不进入 go.mod） |
| 新增 Harness Node 依赖 | ⚠️ 审慎接受——但有 `package.json` 约束，当前零 dep |

### 4.3 自建 vs 采购

所有五个方向都适合**自建**：

| 方向 | 自建理由 |
|---|---|
| Agent 适配器契约 | 这是 ForgeOS 的核心差异化能力。开源社区没有现成的「多 CLI 编排适配器」 |
| ADR 治理闭环 | 项目自身已有 `adr_test.go` 基础，是在现有基础上扩展而非从零构建 |
| 状态目录生命周期 | 高度定制于 ForgeOS 的 `.forge/` 格式，无通用方案 |
| 自举测试隔离 | 特定于 ForgeOS 的 self-testing 架构，无法外部采购 |
| 治理收编 | 项目自身的代码库治理问题 |

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0 (当前 sprint 1-2)           P1 (sprint 3-4)              P2 (sprint 5+)
─────────────────────        ─────────────────────        ────────────────────
A. Agent Adapter 接口         B. ADR 状态机 + CLI          E. 治理孤儿收编
C1. trace 二次旋转            D1. INNER 结构化             ─ examples .agent/
   memory TTL                   ─ 测试隔离声明               ─ pi-batch 迁移
   forge cleanup              ─ fixture 优先模式
                               C2. 统一生命周期框架(可选)
```

### 5.2 阶段划分

**阶段一（Sprint 1-2）：核心架构改进**

| 任务 | 涉及 | 预估 |
|---|---|---|
| 定义 `AgentAdapter` 接口 | `internal/adapter/` | 0.5 sprint |
| 从 `engine_build.go`/`cost.go` 提取 `ClaudeAdapter` | `internal/adapter/claude.go` | 0.5 sprint |
| trace 二次旋转（定时而非仅启动时） | `evolve.go:openTracer` | 0.3 sprint |
| memory TTL 机制 | `memory/memory_compact.go` | 0.3 sprint |
| `forge cleanup` 命令 | `cmd/forge/cleanup.go` | 0.3 sprint |
| 配额告警（80% 阈值） | `doctor/quick.go` | 0.1 sprint |

**阶段二（Sprint 3-4）：治理深度**

| 任务 | 涉及 | 预估 |
|---|---|---|
| ADR 文件 metadata（status/supersedes/superseded_by） | `docs/adr/` 模板 + 现有 ADR 更新 | 0.3 sprint |
| `forge adr validate` 命令 | `cmd/forge/adr_cmd.go` | 0.5 sprint |
| 自举测试 INNER 结构化声明 | `harness/test_acceptance.mjs` + 配置 | 0.3 sprint |
| 关键集成测试 fixture 化 | `harness/test_gate.mjs` 等 | 0.3 sprint |
| 版本兼容性启动检查 | `cmd/forge` main 入口 + `internal/state/` | 0.3 sprint |

**阶段三（Sprint 5+）：一致性收尾**

| 任务 | 涉及 | 预估 |
|---|---|---|
| pi-batch.py 迁移至 `harness/` | 移动 + 补测试 | 0.3 sprint |
| `examples/go-taskd` 添加 `.agent/` 目录 | 生成 `forge-init` 产物 | 0.3 sprint |
| ADR 状态机完整实现（含 Superseded 历史追踪） | `internal/adr/registry.go` | v3 规划 |

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 适配器接口提取破坏现有 claude 流程 | 低 | 高 | 100% 保持 `ClaudeAdapter` 行为不变，extract 不 refactor |
| ADR 状态机增加认知负担 | 中 | 中 | 保持向后兼容，新 ADR 用新格式，旧 ADR 自动标记 `status: accepted` |
| `forge cleanup` 误删有用数据 | 中 | 高 | 先 `--dry-run` 灰度，默认保留最近 N 份 |
| 自举测试 fixture 与实际仓库不同步 | 中 | 低 | fixture 仓库用代码生成（从真实仓库去除可变部分） |
| 团队对 v3 方向的分歧（统一框架 vs 最小收敛） | 低 | 中 | C1 落地后获得实际数据再决策 C2 是否值得 |

---

## 总结

审计报告揭示了原始文档的 ~40% 事实性错误（checkpoint 版本已存在、retain=5、`adr_test.go` 已实现、根目录文件合规），但这些错误**不削弱五个方向的根本价值**——它们只是要求修正定位基准：

| 方向 | 修正后定位 | 核心价值 |
|---|---|---|
| 1. Agent 适配器契约 | 从零构建 → 100% 不变 | 架构最致命弱点——多 CLI 愿景的前提不存在 |
| 2. ADR 治理闭环 | 从零构建 → 在 `adr_test.go` 基础上扩展 | 治理 OS 治理自己的架构决策 |
| 3. 自举测试隔离 | 放宽对 `mkdtempSync` 的指责 → 聚焦真实仓库集成测试 | 自举循环依赖是真实可靠性风险 |
| 4. `.forge/` 生命周期 | 删除 checkpoint 版本错误 → 聚焦 trace 旋转、memory TTL、统一清理 | 仍为 P0——24h+ 运行可运维性前提 |
| 5. 治理收编 | 删除根目录数违规声明 → 聚焦 pi-batch 无治理、examples 无 .agent/ | P2 合理，一致性收尾 |

**内核已经非常坚固**（编排引擎、路由调度、收敛循环、持久化、安全纵深），但**外围尚未达到同等成熟度**——这正是五个方向的共同主题。它们是 ForgeOS 从「功能性原型」走向「生产级平台」的最后几个结构性障碍。
