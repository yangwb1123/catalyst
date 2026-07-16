# ForgeOS — 五个真正新颖的系统级扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件深扫完整代码库:forge-core(19 Go 包 · ~35k LOC 生产代码 · 纯 stdlib 零依赖)、  
>    cmd/forge(17+ 子命令 · ~12k LOC)、harness(39+ 模块 · ~10.5k LOC 执法层)、  
>    `.agent/`(12 agent 卡 · 9 skill 卡 · 5 工作流 · 完整治理骨架)、  
>    `examples/`(url-shortener · go-taskd)、`pi-batch.py`(499 行无治理根脚本)、  
>    `docs/`(90+ 分析文档 + FUNCTIONAL_REQUIREMENTS_AUDIT 全审计 + 4 ADR + 31 轮 sprint)。  
> 2. **差异化验证**:对每个方向的**核心命题**,在 docs/requirements/(56 篇)+ docs/analysis/(-40 篇)+  
>    核心文档中做逐篇全文语义检索,确认该方向**作为独立扩展方向从未被展开**。下方  
>    「与已有覆盖的差异」部分引用最接近的已有分析并解释为什么不是同一个方向。  
> 3. **纪律**:不编写任何代码。每个方向附代码级证据、边界情况表、实际影响。  
> **日期**: 2026-07-10

---

## 全景定位

已有 ~90 份分析文档覆盖了 ForgeOS 几乎每个可触及的功能域:引擎补齐(-35 方向)、执行语义形式化  
(-15)、生产可靠性(-18)、二阶系统问题(-15)、多仓库/联邦/跨会话治理(-12)、产品视角(-10)、  
安全纵深(-10)、北极星桥梁(-8)、阶段间契约(-6)、结构缺口(-10)、以及其他散点(-20)。

但有一类方向落在所有已有分析的**类型间隙**中——它们不是「缺什么功能」或「边界情况修复」,  
而是**代码层已存在但从未被识别为独立系统性方向的设计特征**。这些方向有一个共同点:  
**修复它们不会增加用户可见的功能,但会根本性地改变 ForgeOS 作为一个长生命周期自治系统的  
可演化性、适应性和成本效率。**

| # | 方向 | 类型 | 优先级 | 核心问题 | 已有分析覆盖 |
|---|---|---|---|---|---|
| 1 | **跨会话可移植工作空间** | 架构 · 韧性 · 协作 | P2 | Forge 状态(.forge/ · memory · trace)困在本地文件系统,无法跨机器/克隆迁移 | 仅提及 phase 幂等性,非完整方向 |
| 2 | **自引用结构健康仪表盘** | 治理 · 可观测性 | P2 | ForgeOS 检测项目代码腐化但不追踪自己的结构趋势 | **零覆盖** |
| 3 | **环境多态运行时框架** | 韧性 · 可移植性 | P1 | 执行环境(OS/容器/工具链/网络)被假定而非被检测 | 仅提及 Windows Job Object 适配点 |
| 4 | **自校准阈值调优引擎** | 治理 · 机器学习 | P3 | 全部结构阈值固定硬编码,无法据项目自身统计分布自适应 | **零覆盖**(memory compaction 阈值无关) |
| 5 | **Agent 执行连接池** | 性能 · 成本优化 | P1 | 每 phase 独立 spawn claude 子进程,冷启动开销在顺序多 phase 工作流中重复支付 | **零覆盖** |

---

## 方向一 · 跨会话可移植工作空间

**优先级**: 🟡 P2 | **类别**: 架构 · 韧性 · 协作 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的累计状态完全困在本地文件系统中:

- **`.forge/` 目录**:各阶段审批标记(`.approved`/`.rejected`)、运行痕迹
- **`.agent/` 目录**:治理骨架(agent 卡 · skill 卡 · workflow 定义 · 策略文件)
- **memory 存储**:`<root>/.memory.jsonl`——跨迭代的学习积累(发现、决策、教训)
- **trace 存储**:`<root>/trace.jsonl`——24h 自治运行的完整结构化审计轨迹
- **checkpoint 存储**:`<root>/checkpoint.json`——可恢复的快照状态(iteration/phase)
- **scorecards**:`<root>/.agent/routing/scorecards.json`——路由历史择优数据

当开发者将仓库克隆到另一台机器、CI runner、或队友工作站时,以上**全部丢失**。  
新 clone 的 `forge evolve` 从零开始——没有 memory、没有 scorecard 历史、没有 checkpoint、  
没有轨迹审计。对于 24h+ 自治运行场景,这意味着:

1. **跨机器协作断裂**:开发者 A 跑了 12 小时 evolve,`forge accept` ACCEPTED,提交代码。  
   开发者 B 拉取后想查看 A 的收敛轨迹——trace 没进 git,看不到。
2. **CI 从零开始**:CI runner 每次 checkout 是干净的,memory 和 scorecard 不存在,  
   `forge evolve --resume` 找不到上次运行的 checkpoint,必须从头开始。
3. **灾难恢复盲区**:本地磁盘故障后,forge 状态完全丢失。代码在 git 中安全,  
   但「为什么做了这个架构决策」「上次收敛用了多少轮」「哪个路由历史数据有效」全无。

### 代码级证据

```
.forge/                          ← gitignored(在 .gitignore 中)
  .gitignore → ".forge/"
.memory.jsonl                    ← 不在 git 中(无 .gitignore 条目但从未被 add)
trace.jsonl                      ← 同上
checkpoint.json                  ← 同上
.agent/routing/scorecards.json   ← 在 git 中(仅因恰好被 add,非设计使然)
```

`internal/persist/checkpoint.go` 的 `Save/Load` 使用**硬编码的相对路径**  
`filepath.Join(root, "checkpoint.json")`——路径不可配置、不可重定向。

`internal/memory/memory.go` 的 `AppendTo/Load` 使用**同样的硬编码模式**。

`internal/trace/trace.go` 的 `NewTracer` 取一个 `io.WriteCloser` 参数——  
CLI 层(`cmd/forge/evolve.go`)绑定到 `<root>/trace.jsonl`。

`harness/acceptance.mjs` 的 `probeAppTests` 等硬编码 `<root>/test` 和 `<root>/app`  
路径布局,没有「备选搜索路径」或「显式清单」的概念。

### 边界情况

| 场景 | 当前行为 | 风险 |
|---|---|---|
| git clone 到新机器后 `forge evolve` | memory/trace/checkpoint 不存在→静默冷启动 | 丢失所有历史学习成果 |
| 跨 CI run 的 scorecard 持久化 | 每次 checkout 重建 scorecards.json | 路由历史择优退化为随机 |
| 多开发者协作:查看对方运行轨迹 | trace 不在 git 中,不可查看 | 审计断裂,无法事后复盘 |
| 从备份恢复:磁盘故障 | 只有 git 中的代码恢复,forge 状态全失 | 长时间运行的投资全丢 |
| 同一项目的两个不同工作目录 | 各自独立积累 memory,知识分裂 | 学习闭环被分割 |

### 与已有覆盖的差异

- `second-order-architectural-gaps.md` 方向三提到「Memory 快照/回放」,但那是 memory 的  
  命名快照用于 A/B 测试,不是跨机器可移植性。
- `five-uncovered-horizontal-frontiers.md` 提及 workspace snapshot 用于 phase 幂等性检测  
  (确定 phase 输出是否改变以跳过重跑),不是用于状态迁移。
- ADR-0003(`agent-os` submodule)讨论的是治理资产共享(agent 卡/workflow),不是运行时  
  状态(memory/trace/checkpoint)的可移植性。

### 实际影响

**为 forge 带来真正的「可移植持久性」**:一个 evolve session 的完整状态——memory 积累、  
收敛轨迹、路由历史——可以打包、传输、在另一台机器上恢复。这使 CI 能跨 run 持续学习、  
团队协作时不丢失上下文、灾难恢复时有完整的 forge 状态而非仅 git 代码。

---

## 方向二 · 自引用结构健康仪表盘

**优先级**: 🟡 P2 | **类别**: 治理 · 可观测性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 为被治理的项目提供强大的结构健康检查:gate.mjs 检查文件行数、arch-check 检查  
8 个架构维度、check.py 检查治理完整性、secret-scan 检查硬编码秘密。

**但它从不检查自己。**

具体来说:

1. ForgeOS 的 `harness/arch/arch-check.mjs` 每次运行输出**当前快照**的度量值  
   (`max fan-in: 15`, `cmd/forge files: 16`),但这些值**从不被记录为时间序列**。  
   没有「上周 fan-in 是 12,这周变成 15」的趋势数据。

2. 跨 sprint 的架构漂移全靠人工记忆:从 `.arch/rules.yaml` 的注释中可以读到一连串  
   的 `RECALIBRATED` 记录(2026-06-27 · 2026-07-02 · 多次),但这些校准事件完全手工记录,  
   没有任何自动化的「在这个 PR 中 fan-in 从 12 增长到 15」告警。

3. ForgeOS 自己的 `.agent/CURRENT_SPRINT.md` 记录了 31 轮 sprint 中几乎每一次架构自纠  
   (cmd/forge 文件数反复突破、gate_resolve.go 的包归属迁移、prompt_artifacts.go 的拆分),  
   但**没有机器可读的结构健康度量**与之对应。每次架构自纠都是被动反应式的——  
   等到违反红线才修,而非趋势恶化到阈值 80% 时主动干预。

4. `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 doc-drift-only 项诚实承认  
   「`.agent/ROADMAP.md` 的一些计数已过时(9 agent 卡→实际 12)」——这些漂移本可以被  
   自动的结构趋势仪表盘捕获。

### 代码级证据

`harness/arch/arch-check.mjs` 的输出格式是供人阅读的,不是供机器消费的:

```js
// arch-check.mjs: 最后输出示例
console.log(`PASS root: ${dirs} dirs (budget ${budget}), ${files} files (budget ${rootFiles})`);
console.log(`PASS package ${pkg}: ${count} files (budget ${maxFiles}), ${exports} exports (budget ${maxExports})`);
console.log(`PASS fanin ${target}: ${importers} importers (budget ${maxImporters})`);
```

这些值**没有被写到任何结构化存储**(JSON/JSONL/CSV)中,不能被后续运行查询趋势。

`harness/policies.yml` 和 `.arch/rules.yaml` 中的阈值是**绝对硬常数**。  
没有「阈值使用率」的概念——当前值距离阈值还有多远——更不用说趋势记录。

### 边界情况

| 场景 | 当前行为 | 风险 |
|---|---|---|
| fan-in 连续 5 个 sprint 从 12 增长到 19 | 直到 >=20(阈值)才被发现→告警 | 架构耦合潜移默化恶化 |
| 函数长度分布 50 分位数从 12 涨到 30 | 只要单个不超 50 就绿 | 整体代码复杂度悄悄上升 |
| pkg 文件数在阈值内反复波动 | 每次 PASS,无人注意模式 | 无法区分「稳定」vs「不稳定的边缘」 |
| 架构自纠 SPRINT 注释已 5 次提及同一包 | 人工追踪 | 需要机器可读的结构健康记录 |

### 与已有覆盖的差异

- `agent-orchestration-five-novel-perspectives.md` 的 `quality_trend` 检测的是**学习循环  
  健康**(agent 产出质量改进/退化),而不是代码库的结构健康。
- `docs/expansion-deep-analysis.out.md` 的 `6e("No mechanism detects whether agent  
  actually obeys AGENTS.md constraints")` 是关于 agent 行为合规性,不是关于  
  ForgeOS 自己的代码结构。
- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的系统审计是**一次性人工演练**,不是持续运行的  
  趋势仪表盘。

### 实际影响

**让 ForgeOS 践行自己的原则**。如果 ForgeOS 声称「先拆分,再继续」和「单一职责」是好的,  
它应该能够证明自己随着时间的推移也在变好(或至少不恶化)。一个自动结构健康仪表盘,  
每次 `forge accept` 或 CI 运行时记录自己的度量,使架构漂移**在变成违规之前就可视化**——  
从被动反应到主动管理。

---

## 方向三 · 环境多态运行时框架

**优先级**: 🔴 P1 | **类别**: 韧性 · 可移植性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 对其运行环境做了一组**隐含的、缺乏文档的、毫不容错的假设**:

- **操作系统**:POSIX (Linux/macOS)——`command_executor_unix.go` 使用 `Setpgid` 和  
  `SIGKILL` 进程组清理。`command_executor_other.go` 的 `setupProcessGroup` 是空函数——  
  在 Windows 上,子进程变成孤儿,`forge run` 结束后 agent 进程继续运行。

- **工具链**:`node` 在 PATH 中、`python3` 在 PATH 中、`go` 在 PATH 中、`claude` 在 PATH 中  
  ——`cmd/forge/engine_build.go` 硬编码 `exec.Command("claude", ...)`。  
  如果 `claude` 不在 PATH 中但 `claude-code` 在,运行静默失败。

- **文件系统**:POSIX 路径分隔符(`/`),`/tmp` 可写,`$HOME` 存在。  
  `harness/secret-scan.mjs` 使用 `path.relative` 处理 POSIX 路径。  
  `internal/persist/checkpoint.go` 使用 `os.Rename`(跨文件系统 rename 在 POSIX 上可能失败,  
  在 Windows 上需要 `MoveFileEx`)。

- **网络**:永远在线——`claude` API 调用假设网络连通性。  
  没有离线模式:即使所有 phase 都只是本地文件操作(重构、测试),如果 LLM 不可达,  
  整个 `forge run` 会失败,而不是降级到预缓存或本地回退。

- **终端**:`claude -p` 假设交互式终端或 `--output-format json` 可用——  
  某些 CI 环境或非交互式 shell 可能表现不同。

- **沙箱**:无——`command_executor.go` 的 `SandboxConfig` 是"v1 placeholder skeleton",  
  没有容器/VM 隔离。Firecracker 是 v3。

### 代码级证据

`forge-core/internal/orchestrator/command_executor_unix.go`:
```go
//go:build unix
func setupProcessGroup(cmd *exec.Cmd) {
    cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}
```

`forge-core/internal/orchestrator/command_executor_other.go`:
```go
//go:build !unix
func setupProcessGroup(cmd *exec.Cmd) { /* no-op */ }
```

`cmd/forge/engine_build.go`:
```go
exec.Command("claude", ...) // 硬编码二进制名,无 PATH 搜索回退
```

`harness/gate.mjs`:
```js
const ROOT = dirname(fileURLToPath(import.meta.url));
// 假设 Node.js 可用,无 fallback
```

整个代码库中没有**环境检测**代码——没有检查操作系统类型、工具可用性、  
网络连通性、文件系统特征的功能。环境差异是静默容错的(空函数)或静默失败的  
(二进制未找到→exit 1)。

### 边界情况

| 场景 | 当前行为 | 理想行为 |
|---|---|---|
| 在 Windows 上运行 `forge run` | `setupProcessGroup` 是 no-op,子进程孤儿;路径分隔符可能不兼容 | 检测到 Windows,使用 Job Object |
| `claude` 以 `claude-code` 名安装 | `exec.Command("claude")` 失败→exit 1 | 探测常见二进制名变体 |
| CI runner 无网络(离线) | `forge run`→claude API 调用失败→abort | 检测到离线,降级到dry-run或本地模式 |
| 容器内无 `python3` | `forge run/evolve` 的 YAML→JSON shim 失败 | 探测可用解释器,PyPy/python/python3 回退链 |
| `/tmp` 不可写 | checkpoint 写失败→潜在数据丢失 | 先测试 `/tmp` 可写,不可写则用 `$TMPDIR`/`.forge/tmp` |
| 受限 shell(无 `setpgid`/job control) | fork-bomb 无保护 | 检测受限环境,启用替代进程管理策略 |

### 与已有覆盖的差异

- `strategic-extensions-v24-uncovered-frontiers.md` 方向二在讨论孤儿进程残留时提及  
  `cross_platform.go`(Windows Job Object 适配)作为子方向之一——但它聚焦于进程清理,  
  而非系统的环境检测与自适应运行时框架。
- 没有一个已有分析将「跨平台环境检测与自适应」本身作为一个独立的架构方向。
- `acceptance.mjs` 的 `N/A` 诚实降级模式(工具缺失→诚实报告)是**横向适配器模式**  
  (按语言/工具),不是**纵向环境适应**(按 OS/网络/沙箱级别)。

### 实际影响

**为 forge 带来真正的环境韧性**。当前,forge 在 POSIX 开发机上工作良好,但在  
Windows/macOS/Linux 的各种变体、容器化 CI、受限 shell、离线环境下的行为是脆弱或  
未定义的。一个系统的环境多态框架将:

1. 启动时做一次环境探测,生成 `EnvironmentProfile`(OS/工具/网络/沙箱级别)  
2. 按 profile 选择执行策略(Unix→`Setpgid`,Windows→Job Object,容器→`prctl`)  
3. 按 profile 优雅降级(无→dry-run,网络不可用→本地缓存模式)  
4. 将 profile 写入 trace 事件,使事后审计可追溯「此 run 在什么环境下执行」

---

## 方向四 · 自校准阈值调优引擎

**优先级**: 🟢 P3 | **类别**: 治理 · 机器学习 | **预估**: ~4 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 全部结构阈值是**固定硬常数**,服务于「触发审查」信号而非机械砍刀:

- 单文件 ≤ 500 行
- 单函数 ≤ 50 行  
- 根目录文件 ≤ 15  
- 扇入(生产 import)≤ 20  
- 包文件 ≤ 17  
- 认知模块 ≤ 8  

这些阈值对所有项目**一视同仁**——无论项目是微服务(10 个包,每个 2 个文件)还是  
单体仓库(200 个包,每个 20 个文件)。但问题的结构在于:

1. **不同项目有不同的统计分布**。一个嵌入式 C 项目和一个 Go 微服务项目的  
   函数长度分布完全不同。500 行文件阈值对 JS 配置文件目录(典型 30 行)太宽,  
   对一个 Go 编排器(典型 200 行)可能合理。

2. **同一项目在不同成熟度阶段有不同的分布**。ForgeOS 自身在 sprint 1 时有  
   8 个 Go 包;在 sprint 31 时有 19 个包。早期阶段的阈值在后期阶段可能太宽松。

3. **阈值校准事件完全手工**。从 `.arch/rules.yaml` 的注释可以看到一串校准事件:  
   ```
   RECALIBRATED 2026-06-27: pkg max_exports 20→30
   RECALIBRATED 2026-07-02: pkg max_files 14→18→16→17
   ```
   这些校准是**人工响应式**的——等到红线被触发,然后人工调整。  
   从未有过「当前中位数是 40 行,P95 是 110 行,建议将函数长度阈值设为 120」的分析。

4. **无趋势数据做校准依据**。校准决策基于直觉而非统计证据。  
   「30 个 export 是否太多?」的回答应该是「各包 export 数的 P90 是 22,  
   30 在 P95 以下是合理的」——而非「感觉差不多」。

### 代码级证据

`.arch/rules.yaml` 中的阈值是纯数字字面量:
```yaml
package:
  max_files: 17
  max_exports: 30
file:
  max_lines: 500
```

`harness/arch/arch-check.mjs` 读取这些阈值,但从未测量或报告实际统计分布:
```js
// arch-check.mjs 检查逻辑
if (count > budget) fail; // 二元 PASS/FAIL,不记录实际值
```

`harness/policies.yml` 中的 `max_function_lines: 50` 和 `circular_dependency_count: 0`  
同样是硬常数。

整个代码库中:  
- 没有统计收集层(收集每次运行的度量值到一个时间序列存储)  
- 没有分布报告(中位数/P50/P90/P99/标准差)  
- 没有趋势分析(与过去 N 次运行比较)  
- 没有校准建议(基于收集的统计提出新阈值)  
- 没有`forge suggest --thresholds` 命令

### 边界情况

| 场景 | 当前行为 | 自校准后的行为 |
|---|---|---|
| 新项目使用 forge-init | 继承与 ForgeOS 自身相同的 500/50/15 阈值 | 首次运行采集统计,建议项目特定阈值 |
| 经过 20 次重构后函数长度分布整体右移 | 只有超 50 的单函数被标记 | 仪表盘显示中位数从 12→24,建议审查 |
| monorepo 中包数很多但每个包很小 | 包数量超过 max_modules → 告警 | 按包大小/文件的真实分布校准 |
| 某包 fan-in 从 5 增长到 18 但从未超过 20 | 始终 PASS,无人注意趋势 | 趋势仪表盘显示 +260%,触发审查 |

### 与已有覆盖的差异

- 所有已有分析的方向都是关于**如何执行阈值**(gate 执法、adapters 框架、lifecycle mode-gating),  
  没有一个是关于**如何设置阈值**。
- `harness/policies.yml` 和 `.arch/rules.yaml` 的注释确实记录了校准事件,但那是  
  人工变更日志,不是自适应系统。
- `memory_compact.go` 的 `DefaultCompactThreshold` 是 ForgeOS 中唯一不是结构健康  
  阈值的自适应参数,但它的触发条件是条目计数而非统计分布,且用途完全不同  
  (memory 压缩 vs 结构健康)。

### 实际影响

**将阈值管理从「仲裁」变为「统计」。** 目前,阈值是规则——越了就是错。  
自校准引擎让阈值成为**统计基线**——偏离基线会触发审查,但在有充分统计证据  
支持的情况下,调整阈值本身也是有效的治理操作。这消除了一类最烦人的治理摩擦:  
「我的代码合规但阈值太紧了」vs 「阈值是纪律」的争论。

---

## 方向五 · Agent 执行连接池

**优先级**: 🔴 P1 | **类别**: 性能 · 成本优化 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 当前的 agent 执行模型是**每 phase 一个独立子进程**:

```
build.yml (5 phases):
  planner      ──→ claude subprocess (cold start) → exit
  implementer  ──→ claude subprocess (cold start) → exit
  harness      ──→ (非 LLM,shell out)  
  reviewer     ──→ claude subprocess (cold start) → exit
  qa           ──→ claude subprocess (cold start) → exit
```

每个 `claude` 子进程的冷启动包括:

1. **OS 进程创建**:fork+exec,加载运行时(约 50-200ms)
2. **模型上下文初始化**:加载 system prompt、历史、工具定义(约 500-2000ms)  
3. **LLM 推理预热**:首次 token 生成前的模型前向传播(约 1-5s,取决于模型)

对于 Sonnet 档位的连续 phase(planner→implementer),更具体来说:

- planner 完成 → process exits → OS 回收资源
- implementer → 完全新的 OS 进程、新的上下文传输、新的 model warmup

对于 evolve 循环(多迭代 × 多 phase),这一开销被重复支付:  
N 迭代 × M phase/迭代 = N×M 次冷启动。

`internal/orchestrator/command_executor.go` 的 `Command` 方法每次创建全新的  
`exec.Cmd` 实例:

```go
func (e *CommandExecutor) Command(ctx context.Context, root, name string, argv ...string) *exec.Cmd {
    cmd := exec.CommandContext(ctx, name, argv...)
    cmd.Dir = e.root
    // ... 每次新建,无复用
    return cmd
}
```

`cmd/forge/engine_build.go` 中的 `runAgentPhase` 调用 `agentExecutor` 每次都构建  
完整命令行和 prompt,然后 spawn:

```go
func runAgentPhase(ctx context.Context, eng *orchestrator.Engine, ...) error {
    prompt := buildPrompt(...) // 每次完整构建
    cmd := eng.Command(ctx, root, agentCmd, prompt...) // 新进程
    ...
}
```

### 连接池设计思路(概念)

一个 `AgentPool` 维护一组已 Warm 的 LLM 进程:

```
Pool (maxSize=2, tier=sonnet)
  ├── Process #1 ── 状态: idle ── 已加载 sonnet 模型
  └── Process #2 ── 状态: busy ── 正在执行 implementer phase
         ↑ planner 完成后归还
         
新 phase 需要 sonnet → 从 pool 获取空闲进程 → 注入新 prompt → 执行 → 归还 → 复用
```

关键设计约束:

1. **模型档位隔离**:不同 tier(haiku/sonnet/opus)有各自的池,haiku 进程不与 opus 混用
2. **安全边界**:每个执行业务前必须重置 prompt 上下文——无跨 phase prompt 泄露
3. **最大空闲超时**:池中空闲进程超过 `MaxIdleTime` 后被清理,不浪费资源
4. **故障隔离**:一个进程中的 LLM 异常不污染池中其他进程
5. **回退**:池空/满时退化为当前的一次性 spawn 行为
6. **厂商感知**:`--agent-cmd claude` 和 `--agent-cmd codex` 不共用池

### 收益估算

典型 build.yml(5 phase,其中 4 个 LLM):

| 场景 | 当前(每次冷启动) | 有连接池(前 1 次冷启动+3 次重用) | 节省 |
|---|---|---|---|
| 冷启动延迟 | 4 × ~3s = 12s | 1 × ~3s + 3 × ~0.3s = 4s | ~66% 延迟 |
| cold token 预热 | 4 × ~500 tokens = 2000 tokens | 1 × ~500 + 3 × ~50 = 650 tokens | ~67% token |
| 总相位启动时间 | ~12s 非 LLM 开销 | ~4s 非 LLM 开销 | 8s/run |

Evolve 5 迭代 × 上述场景:节省 ~40s 非推理延迟 + ~6750 token 预热浪费

### 边界情况

| 场景 | 处理策略 |
|---|---|
| 池中进程异常退出 | 从池中移除,按需启动新的替换;不影响其他 phase |
| 新 phase 模型档位高于池中所有进程 | 退化为一次性 spawn,不做「降档运行」 |
| 池满但有新 phase 到达 | FIFO 驱逐最久空闲进程,或退化 spawn |
| `claude` 升级重启(进程退出) | 池检测到退出→标记无效→下次获取时更换 |
| 并行模式 + 连接池 | 池大小需按并发数展:maxSize ≥ parallel.Wave.MaxPhases |
| 进程注入安全 | 每次获取时清空标准输入/输出缓冲区,防止跨 phase 信息泄露 |

### 与已有覆盖的差异

- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 提及  
  `forge daemon mode`(守护进程模式)用于配置热重载——这不是连接池,而是  
  一个持续运行的管理进程。连接池是执行优化,不是管理模式。
- 所有已有分析覆盖了并行执行(`parallel.go`)、执行预算(`budget.go`)、  
  超时/取消(`command_executor.go`),但没有一个分析 LLM 子进程的复用。
- `edgecases-and-perf.md`(第二次扫描)覆盖了并行编排竞态、资源泄漏、收敛陷阱、  
  锁争用——但没有关于进程复用/连接池的内容。

### 实际影响

**减少多 phase 工作流中约 50-70% 的 LLM 进程非推理开销**。这不是减少 LLM 推理本身  
(那是模型级别的问题),而是减少围绕每次 LLM 调用的操作系统和初始化开销。对于  
`forge evolve` 这样的多迭代循环,收益随迭代次数线性增长。

---

## 综合优先级建议

由项目自身治理纪律(«先拆分,再继续»/单一职责)和实际影响评估:

| 方向 | 优先级 | 理由 |
|---|---|---|
| **环境多态运行时框架** | P1 | 当前在 Windows/容器/离线环境下的行为是脆弱或未定义的;修复影响所有用户 |
| **Agent 执行连接池** | P1 | 直接改善延迟和成本,对多 phase 工作流(所有 build/evolve)都有即时收益 |
| **跨会话可移植工作空间** | P2 | 高杠杆但依赖场景:单用户本地用不上,协作/CI/灾难恢复场景方凸显价值 |
| **自引用结构健康仪表盘** | P2 | 高杠杆但非紧急:当前 gate 足以在违规时捕获,但趋势发现需要时间积累 |
| **自校准阈值调优引擎** | P3 | 治理成熟度项:在已有阈值体系稳定运作后才值得做精细化校准 |

### 实施顺序建议

```
Environment Polymorphism (P1)
        ↓
Agent Connection Pool (P1)
        ↓
Portable Workspace (P2)
        ↓
Structural Health Dashboard (P2)
        ↓
Self-Calibrating Thresholds (P3)
```

前三项有自然依赖:连接池的实现需要知道它运行在什么环境下  
(环境检测→按检测结果选择池实现),可移植工作空间需要与环境检测  
配合(导出时检测可用工具链,选择合适的归档格式)。

---

## 附录:与 90+ 已有分析的差异化证明

下文对每个方向的**最接近已有分析**逐一证明差异:

### 方向一:跨会话可移植工作空间

| 已有分析 | 覆盖内容 | 本文差异 |
|---|---|---|
| `second-order-architectural-gaps.md` §3 | memory 命名快照用于 A/B 测试 | 本文:打包全部 forge 状态跨机器迁移 |
| `five-uncovered-horizontal-frontiers.md` §1 | workspace snapshot 用于 phase 幂等性检测 | 本文:跨会话/机器/CI 的完整状态可移植性 |
| ADR-0003 | `agent-os` submodule 治理共享 | 本文:运行时状态(memory/trace/checkpoint)而非治理资产 |
| `expansion-directions-v20.md` §3 | hash(workspace snapshot)跳过重跑 | 本文:export/import 格式+传输+恢复机制 |

### 方向二:自引用结构健康仪表盘

| 已有分析 | 覆盖内容 | 本文差异 |
|---|---|---|
| `agent-orchestration-five-novel-perspectives.md` | learning loop quality_trend(agent 产出质量) | 本文:代码库结构健康趋势(fan-in/函数长度/耦合) |
| `expansion-deep-analysis.out.md` 6e | agent 对 AGENTS.md 的合规性 | 本文:ForgeOS 自身代码的架构漂移追踪 |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` | 一次性人工审计 | 本文:持续自动化趋势仪表盘 |

### 方向三:环境多态运行时框架

| 已有分析 | 覆盖内容 | 本文差异 |
|---|---|---|
| `strategic-extensions-v24-uncovered-frontiers.md` §2 | 孤儿进程清理提及 Windows Job Object 适配点 | 本文:完整环境检测+自适应运行时框架(OS/工具/网络/沙箱) |
| `edgecases-and-perf.md` | 竞态/资源泄漏/收敛/锁(全是运行时行为) | 本文:宿主环境差异的优雅处理 |
| `adr/0002` | 目标栈:Go/Rust/Python/TS 跨语言 | 本文:OS 级别跨平台、工具链、网络 |

### 方向四:自校准阈值调优引擎

| 已有分析 | 覆盖内容 | 本文差异 |
|---|---|---|
| `memory_compact.go` DefaultCompactThreshold | memory 压缩的自适应阈值 | 本文:结构健康阈值的统计自校准(完全不同的域) |
| `harness/policies.yml` | 阈值定义 | 本文:如何根据项目自身统计设置和调整这些阈值 |
| `expansion-core-five.md` §4 | 执法补完中的「人类可读策略」 | 本文:机器可读的统计基线管理 |

### 方向五:Agent 执行连接池

| 已有分析 | 覆盖内容 | 本文差异 |
|---|---|---|
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | forge daemon mode 配置热重载 | 本文:LLM 子进程连接池复用(执行优化,非管理进程) |
| `edgecases-and-perf.md` | 并行编排竞态、资源泄漏 | 本文:进程级连接复用(完全不重叠的关注点) |
| `expansion-core-five.md` §1 | 韧性运行时(超时/重试/checkpoint) | 本文:消除冷启动开销(正交的优化维度) |
| `parallel.go`/`RunParallel` | 同迭代内并发 phase | 本文:跨 phase 的进程复用(串行和并行都受益) |
