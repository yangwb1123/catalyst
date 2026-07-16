# ForgeOS — 产品运营视角的五方向系统性缺口

> **角色**: 资深架构师 / 产品经理
> **方法**:
> 1. 全局深扫 forge-core（18 Go 包 · ~33k LOC 运行时 + CLI）、
>    harness（39+ 模块 · ~10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 ·
>    5 工作流 · 全部 ADR+DECISIONS）、examples/、docs/（全部 105+ 已有分析文档）。
> 2. **差异化验证**: 对每个方向的**核心关键词**在全部已有文档中逐篇检查确认——
>    方向可能被**提及**（没有多少想法是完全没人想过的），但**不作为独立系统性方向展开**。
>    每个方向附「与已有覆盖的关系」说明。
> 3. **纪律**: 不编写任何代码。所有建议附 `file:line` 代码级证据、边界情况、
>    实际影响与杠杆评估。
> **日期**: 2026-07-10

---

## 全景定位：已有覆盖 vs 本文焦点

| 已深度覆盖（105+ 篇） | 本文焦点（产品运营视角） |
|---|---|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/resume） | **二进制生命周期与版本治理** |
| 生产可靠性（529/超时/退避/护栏/预算/进程组） | **人工可读的诊断表面** |
| 可观测性与学习闭环（trace/telemetry/scorecard/memory） | **运行时运营可观测性** |
| 安全纵深（secret-scan/递归/预算/CVE/护栏） | **优雅降级与部分恢复** |
| 治理/执法（arch-check/check.py/loop-back/circular-dep） | **跨运行身份与溯源** |
| 中枢旋钮（mode×lifecycle 全 7 维度） | — |
| 产品交付（部署流水线/决策解释/成本预测） | — |
| 扩展方向（记忆去重/热加载/plugin框架/trace查询） | — |

**本文的五方向不被任何已有分析作为独立系统性方向展开**——它们聚焦于 ForgeOS
作为一个**被团队采用、被运维、被长期使用的系统**时的五类缺失基础设施。

---

## 方向一 · 二进制生命周期与版本治理（缺失的升级路径）

**优先级**: 🟠 **P1** | **类别**: 运营 · 部署 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 的 `forge-core` 现在有一个 `forgeVersion = "dev"` 的硬编码字符串。没有版本兼容性矩阵、
没有数据迁移路径、没有升级回滚、没有一个地方能回答「这个 .forge/ 目录是哪个 forge 版本写的」
或「我升级到新 forge 后会破坏正在运行的 evolve 吗」。

这对于一个单用户工具是可以接受的，但 ForgeOS 的目标是 **Idea → Production 的自治工厂**——
工厂需要知道它的工具版本，需要安全的升级路径，需要停机窗口管理。当前状态是无版本管理状态。

### 代码级证据

1. **版本号硬编码，纯展示用** — `forgeVersion` 只出现在 `--version` 和 CI 构建的 `-ldflags`，
   没有任何运行时消费:
   ```go
   // forge-core/cmd/forge/main.go:30-33
   var forgeVersion = "dev"
   var forgeCommit = ""

   // 消费点 (main.go:282-286):
   if args[0] == "--version" || args[0] == "version" {
       ver := forgeVersion
       if forgeCommit != "" { ver += " (" + forgeCommit + ")" }
       fmt.Printf("forge %s\n", ver)
       return 0
   }
   ```
   forgeVersion 不写入 checkpoint、不写入 trace、不写入 memory。升级 forge 二进制后，
   没有任何办法知道一段 trace 是哪个版本产生的。

2. **checkpoint.json 无版本溯源** — `Checkpoint.FormatVersion` 存在但固定为
   `"forgeos.checkpoint.v1"`，不记录 forge-core 版本:
   ```go
   // forge-core/internal/persist/checkpoint.go:42-63
   type Checkpoint struct {
       FormatVersion  string `json:"_format,omitempty"` // 固定值 "forgeos.checkpoint.v1"
       Workflow       string
       Mode           string
       Iteration      int
       RoadmapCompletion float64
       // 无 OriginatingForgeVersion, 无 OriginatingBuildCommit
   }
   ```

3. **trace.jsonl 无版本标签** — `Event.Format` 固定为 `"forgeos.trace.v1"`:
   ```go
   // forge-core/internal/trace/trace.go:63-84
   type Event struct {
       Format string `json:"_format,omitempty"` // 固定值
       // 无 OriginatingForgeVersion
   }
   ```

4. **memory.jsonl 无版本标签** — `Entry.Format` 固定为 `"forgeos.memory.v1"`:
   ```go
   // forge-core/internal/memory/memory.go:140-141
   type Entry struct {
       Format string `json:"format"` // 固定值
       // 无 OriginatingForgeVersion
   }
   ```

5. **harness 脚本与 forge-core 零版本联动** — `gate.Gate()`/`Check()`/`Accept()` shell 出
   node 脚本，但没有任何版本校验:
   ```go
   // forge-core/internal/gate/gate.go:68-79
   func Gate(root string) Result {
       return run("gate", RepoRoot(root), "node", gateScript, "--root", RepoRoot(root))
   }
   ```
   如果一个新版本的 forge-core 依赖一个新版本的 harness 脚本（例如新的 gate 协议），
   升级 forge 二进制而不同步更新 harness 会**静默产生错误结果**，没有任何警告。

6. **forge-init 创建的项目不锁定 forge 版本** — 生成的 CLAUDE.md / project.yml 没有
   `min_forge_version` 或 `forge_version_pinned` 字段。如果团队分布在不同的 forge 版本上，
   同一个项目可能会被不同版本的 forge 以不同方式治理。

### 边界情况

| 场景 | 当前行为 | 预期行为 |
|---|---|---|
| 升级 forge-core 后 `forge resume` | 读取旧 checkpoint，无版本感知 | 检测版本差，若兼容则升级 checkpoint，若破坏则拒绝并提示降级 |
| 两个 CI runner 不同 forge 版本 | 各自跑各自的，无冲突检测 | 写入 trace/checkpoint 时标注版本，运营 dashboard 可识别版本分布 |
| 回滚 forge-core 到旧版本 | 二进制替换，数据被新版本写过后旧版本无法理解 | checkpoint 格式版本化，旧版本拒绝不认识的新格式 |
| 团队 A 用 v2.3，团队 B 用 v2.5 | CI 结果因版本而异，无人察觉 | 项目声明 `min_forge_version`，低于该版本的 forge 拒绝执行 |
| forge-init 创建项目，6 个月后 forge 大版本升级 | 项目仍用旧配置结构 | forge migrate 包含 meta-migration：更新项目结构以匹配新版本 |

### 与已有覆盖的关系

- `expansion-directions-v14-operational-trust.md` 方向 3 讨论了 prompt 版本标签，
  但焦点是 prompt inject 的可观测性，不是 forge-core 二进制生命周期管理。
- 多篇分析提到 `forge-upgrade.mjs`（harness scaffold 中的升级脚本），但该脚本只复制
  harness 文件，不做版本兼容性检查或数据迁移。
- **没有已有分析把「forge-core 二进制版本管理」作为一个独立扩展方向提出。**

### 建议方向

1. **Checkpoint/trace/memory 嵌入 forge 版本** — 写入时注入 `forge_version` 字段。
2. **跨版本兼容性检查** — Load 时检查版本兼容性，不兼容时 fail-closed 而非静默误读。
3. **`forge migrate --version`** — 版本升级命令，做数据格式迁移 + 项目配置更新。
4. **项目级版本锁定** — `project.yml` 加 `forge_version: ">=2.5.0"`，低版本 forge 拒绝执行。
5. **版本兼容性矩阵** — 正式文档化 forge-core 版本与 harness 脚本版本的对应关系。

---

## 方向二 · 人工可读的诊断表面（缺失的「为什么」）

**优先级**: 🟠 **P1** | **类别**: 产品 · 体验 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 拥有「带外执法层」的正确架构设计——harness 闸门独立于任何执行宿主。但它的**错误消息
完全是写给引擎工程师看的，不是写给产品经理、运维人员、甚至熟悉 ForgeOS 但不熟悉 Go 实现的
开发者看的**。

当 `forge evolve` 失败时，当前输出是:
```
forge: run: orchestrator: gate "test" FAIL — exit status 1
```
这不是一个「你的测试套件有失败用例」的消息。这是 Go 错误链的序列化。用户不知道:
- 这是临时失败还是永久失败？
- 是他们该修代码，还是 ForgeOS 配置错了？
- 他们可以重试吗？还是必须改配置？还是联系管理员？
- 上次成功运行是什么时候？这次和上次的差异是什么？

对于一个目标是「24h 无人值守自治运行」的系统，操作员必须能快速理解发生了什么。

### 代码级证据

1. **`forge doctor` 输出是技术检查清单，不是诊断工具** — 每条 `Check` 输出为
   `[PASS]/[FAIL] name`，失败时带 Detail。但 Detail 是面向引擎的:
   ```go
   // forge-core/internal/doctor/doctor.go:66-70
   func (c Check) Line() string {
       if c.OK { return "[PASS] " + c.Name }
       return "[FAIL] " + c.Name + " — " + c.Detail
   }
   ```
   示例实际输出（从代码推断）: `[FAIL] workflow-agent-refs — phase "implement" references
   agent "implementer" but card file not found at .agent/agents/implementer.md`
   ——正确的信息，但缺少**下一步动作**（"Create .agent/agents/implementer.md or fix the
   phase agent field in build.yml"）。

2. **所有 Go 错误链冒泡到 CLI，无上下文转换** — orchestrator 错误通过 `fmt.Errorf`
   逐层包装，最终由 `main.go` 的 run() 打印:
   ```go
   // forge-core/cmd/forge/main.go:525-529
   if runErr != nil {
       fmt.Fprintf(os.Stderr, "forge: run: %v\n", runErr)
       return 1
   }
   ```
   没有错误分类、没有严重级别、没有如何修复的建议。`KindTimeout` / `KindOverloaded` /
   `KindConfig` / `KindFailed` 分类只在 `ExecError` 内部使用，从不向用户暴露不同的处理路径。

3. **`rejectHumanGate` 的改进在 Sprint 26 被标记为"待做"但从未真正完成** — Sprint 26 的
   CURRENT_SPRINT 记录说 rejectHumanGate 应该输出有帮助的指示（如当前状态、如何继续），
   但代码中的实现仍然是简单的:
   ```go
   // forge-core/cmd/forge/evolve.go:65-67
   func rejectHumanGate(stage string) int {
       fmt.Fprintf(os.Stderr, "forge evolve: %q is a human_gate workflow…", stage)
       return 1
   }
   ```
   没有报告当前 checkpoint 状态、没有建议 `forge run --approved`、没有列出 `forge approve list`。

4. **配置验证零用户友好的错误输出** — `forge validate --models` 通过 doctor 包做校验，
   出错时输出 Go 结构体序列化，而非"你的 build.yml 第 42 行引用了不存在的 agent
   'implementer_typo'，你是指 'implementer' 吗？"。

5. **无交互式诊断 CLI** — 没有 `forge why` 命令来分析上次失败、解释原因、给出修复建议。
   用户必须自己读 trace.jsonl + checkpoint.json + 痛苦地拼凑发生了什么。

### 边界情况

| 场景 | 当前行为 | 预期行为 |
|---|---|---|
| `forge evolve` 因 budget 耗尽停止 | `forge: evolve: budget exhausted` | `forge: Budget exhausted after $12.50 (limit $10.00). Phase "implementer" (iteration 3) overspent. Use --run-budget-usd to adjust.` |
| checkpoint 损坏 | `forge: run: persist: load checkpoint: unexpected end of JSON input` | `forge: Checkpoint at .forge/checkpoint.json is corrupted. Last good backup: .forge/checkpoint.json.bak (duration 2026-07-09 14:32). Run with --repair to attempt recovery.` |
| human gate 待审批 | `forge evolve design: "design" is a human_gate workflow` | 显示 pending approval 详情 + 如何批准 |
| 配置漂移（agent 卡被删除但 workflow 还在引用） | `[FAIL] workflow-agent-refs` | 加一行: `Fix: restore the card at .agent/agents/implementer.md or update build.yml phase 3 agent field` |
| 并行模式下 resume | 静默退化到 iteration 边界 | `forge evolve --parallel --resume: Parallel resume only supports iteration granularity. Phase 3 checkpoint ignored, resuming from iteration 4.` |

### 与已有覆盖的关系

- `docs/analysis/edgecases-and-perf.md` §5（治理盲区）提及了未检测的测试退化等信号缺失，
  但焦点在**缺失的度量**，而不是**错误消息的可读性**。
- `product-deployment-transparency-five-gaps.md` 方向二（决策可解释性与 AI 透明度）讨论的是
  **系统决策的透明度**，不是**系统自身错误的诊断可读性**。
- `forgotten-five-foundations.md` 方向三（结构化 Trace 查询与分析 CLI）讨论的是 trace 数据的
  可查询性，不是面向操作员的诊断界面。
- **没有已有分析把「ForgeOS 自身错误诊断的人工可读性」作为独立方向展开。**

### 建议方向

1. **错误分类与增强** — 引入 `OpError` 结构，携带错误分类（configuration / infrastructure /
   agent / budget / unknown）+ 严重级别 + 修复提示 + 相关文档链接。所有 CLI 出口点使用。
2. **`forge why` 命令** — 分析上次 run/evolve 执行的 trace + checkpoint，生成结构化诊断报告:
   - 是什么？(错误类型、发生位置)
   - 为什么？(根本原因分析，基于 trace events)
   - 怎么办？(具体修复步骤)
   - 状态？(当前项目健康度)
3. **交互式 preflight 增强** — `forge preflight` 不仅检查配置完整性，还生成「你的配置有 X 个
   问题，Y 个警告，推荐 Z」的人类可读摘要。
4. **自动化建议引擎** — 常见错误模式检测（budget 耗尽 → 建议提高上限或降低 tier；gate 失败 →
   建议检查测试或配置；agent 卡缺失 → 建议创建或修改引用）。

---

## 方向三 · 运行时运营可观测性（缺失的仪表盘）

**优先级**: 🟠 **P1** | **类别**: 运营 · 可观测性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 是一个设计为在 24h 内无人值守运行自治 evolve 循环的系统。但当前，**没有任何办法知道
系统正在做什么**——除非 SSH 到机器上、读 `.forge/trace.jsonl`、手动解析 JSON。

一个产品经理或运维人员应该能回答:
- 当前有 evolve 循环在运行吗？
- 跑了多久？花了多少钱？
- 当前收敛状态是什么？
- 哪些 gate 通过了？哪些卡住了？
- 上次成功构建是什么时候？

当前对所有这些问题的回答都是「运行 forge 子命令然后读输出」，没有任何持久化或聚合。

### 代码级证据

1. **Tracer 只写文件，不支持实时查询** — trace 写入 JSONL，但没有任何方式从运行中的
   forge 进程查询当前状态:
   ```go
   // forge-core/internal/trace/trace.go:89-95
   type Tracer struct {
       mu     sync.Mutex
       seq    int
       writer io.Writer // 只写不回读
       Now    func() time.Time
   }
   ```
   没有 `Status() ([]Event, error)` 方法。没有实时流式访问。

2. **`forge status` 存在但输出是静态快照** — `cmdStatus` 调用 `doctor.Status` 读取当前
   .forge/ 目录下的文件，输出一次后退出。没有 `--watch` 模式，没有持续刷新:
   ```go
   // forge-core/internal/doctor/status.go 主要逻辑
   func Status(root string) (*StatusReport, error) {
       // 一次性读取 checkpoint + trace + memory → 输出 → 退出
   }
   ```

3. **无健康端点** — forge-core 没有 HTTP 端点、没有 Unix socket、没有命名管道来暴露
   运行时状态。嵌入 forge-core 的外部系统（如未来 Web UI）无法查询:
   ```go
   // forge-core/cmd/forge/main.go — 全部入口是 CLI 子命令
   func main() {
       os.Exit(run(os.Args[1:])) // 执行 → 退出
   }
   ```

4. **并行运行的 evolve 彼此不可见** — 如果两个 evolve 在不同终端同时运行（例如用户手动
   跑一个，CI 触发另一个），没有任何机制能检测或报告这种冲突。`forgotten-five-foundations.md`
   方向一讨论了文件锁，但那是数据完整性保护，不是可观测性。

5. **无 run ID → 无跨文件关联** — Checkpoint / Trace / Memory 的写入都是独立的时间点写入，
   没有任何字段把同一次 run 的三个数据流关联起来:
   - checkpoint.json 有 `Workflow`, `Mode`, `Iteration`
   - trace.jsonl 有 `Seq`, `Kind`, `Name`, `Status`
   - memory.jsonl 有 `Kind`, `Topic`, `Content`
   - 没有任何 `RunID` 字段
   - 没有任何 `SessionID` 字段
   因此无法回答「属于第三次 evolve iteration 的 trace events 有哪些？」或「这个 memory
   条目是在哪个 run 里写入的？」

### 边界情况

| 场景 | 当前行为 | 预期行为 |
|---|---|---|
| CI 上 `forge evolve` 跑 6 小时，想看看进度 | 等它跑完或 SSH + cat trace.jsonl | `forge status --watch` 实时显示收敛趋势、budget 消耗、gate 状态 |
| 发现 `forge evolve` 似乎卡住了 | 无法确认——等待 timeout 或 kill | 自动超时告警 + status 显示 "stalled" 信号 |
| 平台团队想监控所有项目的 forge 运行 | 手动 SSH 到每个项目目录 | 中心化的 forge 运营 API + 可集成到 Datadog/Grafana |
| 用户想找到三天前的 run 的 trace | 在 `.forge/trace.jsonl` 里 grep 日期 | `forge trace list --before 2026-07-07` 列出所有 run |
| 查看总 spend 趋势 | 无聚合，需手动查每个 trace | `forge status --spend --days 30` 显示每日成本趋势 |

### 与已有覆盖的关系

- `trace.go` 的设计目标是记录可审计的事件流，不是实时可观测性。
- `scorecard.mjs` 和 `telemetry` 是**运行后**的评估数据，不是运行时仪表盘。
- `forgotten-five-foundations.md` 方向三的「结构化 Trace 查询与分析 CLI」
  讨论的是 `forge trace` 子命令来查询历史数据——不是运行时实时观测。
- **没有已有分析把 ForgeOS 自身的运行时运营可观测性作为一个独立方向提出。**

### 建议方向

1. **Run ID + Session ID** — 每次 `forge run`/`forge evolve` 生成强唯一 run ID，
   写入 checkpoint/trace/memory。三步数据从此可关联。
2. **`forge status --watch`** — 持续刷新模式，显示:
   - 当前 iterate（N/M）
   - 完成度趋势图（文本 ASCII）
   - gate 状态矩阵
   - 累计成本 + 耗时
   - 异常事件滚动窗口
3. **运行时健康端点** — forge-core 可选监听 Unix socket / TCP 端口，暴露:
   - 活动 run 列表
   - 每个 run 的详细状态
   - 预算消耗
   - gate 缓存
4. **事件总线钩子** — 在关键生命周期点（phase start/end, gate pass/fail, converge check,
   error, budget warning）触发可配置的 HTTP webhook / 文件事件 / stdout JSON。
   外部系统（CI 通知、Slack bot、运营 dashboard）通过钩子集成。
5. **`forge trace list`** — 列出所有已知的 run（从 trace.jsonl + checkpoint.json 聚合），
   带时间戳、持续时间、状态、成本。`forge trace show <run-id>` 显示完整详情。

---

## 方向四 · 运行时自愈优雅降级与部分恢复（缺失的韧性模式）

**优先级**: 🟠 **P1** | **类别**: 可靠性 · 韧性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 对**代理失败**有极其健壮的处理——超时、退避、重试、预算护栏、递归守卫、输出上限。
但对**自身失败**（forge-core 自身的数据损坏、文件丢失、配置错误）的处理是「报错，退出」。

对于一个目标是 24h 无人值守自治的系统，这个策略是不可接受的。如果 checkpoint 在迭代 47 损坏了，
正确的响应不是「报错退出，让操作员去修」，而是:
- 尝试从备份恢复
- 尝试从其他文件重建
- 退化到上一次已知完整的 checkpoint
- 如果一切都失败，至少以可恢复的方式暂停而非猝死

### 代码级证据

1. **checkpoint 损坏 = 死路** — Load 遇到损坏的 JSON 返回错误，调用者（cmd/forge）直接退出:
   ```go
   // forge-core/internal/persist/checkpoint.go:96-103
   func Load(path string) (Checkpoint, bool, error) {
       data, err := os.ReadFile(path)
       if err != nil {
           if errors.Is(err, fs.ErrNotExist) { /* 冷启动的正常情况 */ }
       }
       cp, err := decode(data)
       if err != nil {
           return Checkpoint{}, false, err // 损坏 = 硬错误，无备份恢复尝试
       }
   }
   ```
   调用方（`resumeStart`）面对错误直接 abort，没有任何尝试从 `.bak.1` / `.bak.2` 等备份恢复。

2. **memory 损坏 = 全部丢失** — Load 遇到损坏条目返回错误，且没有索引重建或跳过损坏条目的选项:
   ```go
   // forge-core/internal/memory/memory.go:180-200
   func decode(data []byte) ([]Entry, error) {
       scanner := bufio.NewScanner(bytes.NewReader(data))
       for sc.Scan() {
           var e Entry
           if err := json.Unmarshal([]byte(sc.Text()), &e); err != nil {
               return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
               // 一个坏行 → 全文件废弃
           }
       }
   }
   ```
   对比设计意图（文件头诚实声称「坏行是显式错误，永不静默跳过」），这个策略在哲学上是对的，
   但在生产中是残酷的——一个字节的磁盘错误（比如磁翻转）会让整个 memory store 无法使用。

3. **trace 损坏 = scorecard 不可用** — `scorecard_wind.go` 在整个文件上做 `sc.Scan()`，
   一个坏行导致 `Decode` 失败，scorecard 更新失败:
   ```go
   // forge-core/cmd/forge/scorecard_wind.go:71-88
   func runScorecardUpdate(root string, events []trace.Event) {
       // 消费者解析每个 event，坏行导致整个解析中止
   }
   ```

4. **无退化模式 flag** — forge-core 没有 `--repair`、`--force`、`--recover-from-backup` 等
   标志位来处理部分损坏的情况。损坏 = 唯一出路。

### 边界情况

| 场景 | 当前行为 | 预期行为 |
|---|---|---|
| checkpoint.json 在迭代 47 时损坏（磁盘扇区错误） | `forge resume` → 错误退出。丢失迭代 1-47 所有进度和成本 | 从最近备份恢复。无备份时使用 memory+ 残留 trace 重建近似状态 |
| memory.jsonl 的第 300 行因编码损坏 | 一个字节的错误导致全文件不可用，丢失 300 条记忆 | 跳过损坏行+日志警告，返回其余条目。附带 `--repair` 命令重建干净版本 |
| trace.jsonl 被意外截断（日志轮换误操作） | trace 文件结尾残缺 → scorecard 从完整事件中读到 EOF 前出错 | 在残缺处容错截断（最后一行不完整则忽略而非报错） |
| 两个文件不一致（checkpoint 说 iteration=5，trace 只有 iteration=1-3 的事件） | 无检测，各自独立使用 | 启动时交叉验证 + 日志警告建议修复 |
| .forge/ 目录被部分删除（用户误 rm -rf） | checkpoint+memory+trace 全部丢失。冷启动似全新项目 | 各文件独立缺失可被其他文件部分补偿 |
| 磁盘满导致 checkpoint 写入失败 | write fail → 错误退出，当前 iteration 进度丢失 | 紧急模式：清除旧 trace/memory 以释放空间，重试写入 |

### 与已有覆盖的关系

- `edgecases-and-perf.md` §2.1（trace 无限增长）提到 trace 轮换，但未讨论数据损坏恢复。
- `forgotten-five-foundations.md` 方向五（运行时状态自校验与恢复）聚焦于**checksum 提前检测
  损坏以防静默误读**，不是**已损坏状态下的优雅降级与修复**——两者形成互补:
  自校验防止进入坏状态，部分恢复处理已进入坏状态的情况。
- `genuinely-uncovered-five-frontiers.md` 方向一关于 checkpoint 备份的讨论偏向
  pre-crash 预防，而非 post-crash 恢复。
- **没有已有分析以「运行时自身故障的优雅降级与部分恢复」为独立方向展开。**

### 建议方向

1. **完整的三层恢复策略**:
   - **L1**: 从备份恢复（保留最近 3 个 checkpoint 版本，`.bak.0` / `.bak.1` / `.bak.2`）
   - **L2**: 从其余数据重建（用 trace + memory 推断最后一次已知好的状态）
   - **L3**: 干净冷启动但保留 trace + memory（不丢审计线索和历史知识，接受进度丢失）
2. **`forge repair` 子命令** — 分析 .forge/ 目录完整性，提供交互式修复向导。
3. **读取损坏条目的容错模式** — `memory.Load()` 增加 `Tolerant bool` 选项，设置后
   跳过解析失败的行、返回剩余条目 + 损坏行索引列表。
4. **跨文件一致性检查** — `forge status --health` 验证 checkpoint/trace/memory
   三者之间的交叉引用一致性（例如 checkpoint 的 iteration 必须 ≤ trace 的 max iteration）。
5. **紧急空间回收** — 当 checkpoint/memory 写入因 ENOSPC 失败时，自动清理最旧的
   trace 段和 memory 条目，写入紧急事件到 syslog，然后重试。

---

## 方向五 · 跨运行身份与溯源（缺失的运行护照）

**优先级**: 🟠 **P1** | **类别**: 运营 · 数据管理 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 的所有运行时数据——trace、checkpoint、memory——都缺乏**运行身份**。一次 `forge evolve`
的 50 次迭代产生的 300+ trace events、50+ memory 条目、50 次 checkpoint 写入**都没有携带
「这次运行是谁、何时、为何启动」的元数据**。

当一个项目被多人使用（开发者 A、CI 系统、夜间批量任务）、或者一个机器上运行多个 `forge`
进程时，.forge/ 目录中的数据变成混杂物——无法区分哪些数据属于哪个 run、哪个用户、哪个目的。

当前状态就像机场行李传送带上所有行李箱都用同一种黑色，没有标签。你可以说「行李都到了」，
但说不清哪件是谁的。

### 代码级证据

1. **无 RunID 字段** — checkpoint 没有 RunID:
   ```go
   // forge-core/internal/persist/checkpoint.go:42-63
   type Checkpoint struct {
       Workflow    string
       Mode        string
       Iteration   int
       PhaseIndex  int
       // 无 RunID, 无 CreatedBy, 无 Origin, 无 SessionID
   }
   ```

2. **trace 没有来源标识** — Event 没有 user/host/session:
   ```go
   // forge-core/internal/trace/trace.go:63-84
   type Event struct {
       Seq      int    // 进程内序号，从 1 开始
       Kind     string
       Name     string
       Status   string
       Duration int64
       // 无 RunID, 无 User, 无 Host, 无 PID
   }
   ```
   注意 `Seq` 是进程内自增。两个进程同时写同一个 trace 文件时，两个事件的 `Seq` 可能
   都是 1（各自 Tracer 独立计数），造成 trace 流中的歧义。

3. **memory 无上下文** — Entry 没有关联到产生它的 run:
   ```go
   // forge-core/internal/memory/memory.go:140-155
   type Entry struct {
       CreatedAtUnix int64
       Kind          string
       Topic         string
       Content       string
       // 无 RunID, 无 Workflow, 无 SourcePhase
   }
   ```

4. **CommandExecutor 不注入 run 上下文** — 子进程环境变量不包含 run ID:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:72-88
   type CommandExecutor struct {
       Build func(p asset.Phase, mode string) []string
       // 没有设置 FORGE_RUN_ID 环境变量
       // 没有设置 FORGE_USER 环境变量
   }
   ```

5. **`.forge/` 是单目录，不支持命名空间** — 所有 run 共享 checkpoint.json、trace.jsonl、
   memory.jsonl。没有 `.forge/runs/<run-id>/` 子目录结构。

### 边界情况

| 场景 | 当前行为 | 预期行为 |
|---|---|---|
| 开发者 A 和 CI 同时在同一个 repo 上运行 `forge evolve` | 两个进程写入同一 trace/memory 文件，数据交错且无法区分 | 每个 run 有唯一 ID，数据写入带 run 标签的子目录或事件字段 |
| 想查看「CI 今天跑了多少次 evolve」 | 手动 grep trace，靠猜测 | `forge run list --user ci --after 2026-07-10T00:00:00Z` |
| 发现 trace 中有异常，想确认是哪个 run、谁运行的 | trace 只有 seq/kind/status | trace 事件携带 runID，可追溯到启动命令行和用户 |
| 三个月前的 memory 条目不再相关，想清除某个用户产生的 | 无法区分是谁写入的 | 按 runID/用户过滤 memory 做有针对性的 `memory-prune` |
| CI 流程中 `forge evolve` 产生 trace 后传给分析系统 | trace 没有 run 来源 | trace 携带 build ID / CI job ID / commit SHA |
| 想对某次特定 run 做 post-mortem 分析 | 从 trace 时间戳猜测，但 checkpoint 不记 run 边界 | checkpoint 记录 run 起止时间、runID、触发原因 |

### 与已有覆盖的关系

- `forgotten-five-foundations.md` 方向一（跨进程运行时状态守护）讨论的是**并发写入竞争条件**，
  通过文件锁解决。本文方向五讨论的是**数据归属与溯源**，通过 run 身份标签解决。
  两者互补：锁防止同时写入破坏数据；run 身份标签让同文件中的数据可区分。
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三讨论了
  session/data lifecycle，但聚焦于数据生命周期的管理（何时归档/删除），而非运行身份与溯源。
- `trace.go` 的 Seq 字段是进程内序号，设计目标是为 trace 提供定序，不是跨 run 身份。
- **没有已有分析把「跨运行身份与溯源」作为一个独立的系统性方向提出。**

### 建议方向

1. **RunID 基础设施** — 每次 `forge run`/`forge evolve` 生成 UUIDv7（时间有序 UUID）:
   - 注入为 `$FORGE_RUN_ID` 环境变量，传递给子进程
   - checkpoint.json 增加 `run_id` 字段
   - trace.jsonl 每个事件增加 `run_id` 字段
   - memory.jsonl 每个条目增加 `run_id` 字段
2. **`.forge/runs/` 子目录** — 每个 run 的事件写入 `runs/<run-id>/trace.jsonl`，
   共享 `checkpoint.json` 仍保留为整体进度记录但加 run_id 关联。
3. **用户与上下文溯源** — 记录启动用户（`$USER` / `$BUILD_USER`）、主机名、触发命令
   （完整 argv）、git commit SHA（如果 repo 是干净的）。
4. **`forge run list` / `forge trace list`** — 列出所有已知的 run，按时间/用户/状态过滤。
5. **跨文件关联 API** — `forge trace show <run-id>` 聚合 checkpoint、trace、memory
   中与该 run 关联的所有数据，生成完整运行报告。

---

## 优先级总结

| 方向 | 优先级 | 预估 | 杠杆 | 核心价值 |
|------|--------|------|------|---------|
| 方向一: 二进制生命周期与版本治理 | P1 | ~2 sprints | ⭐⭐⭐⭐⭐ | 多团队采用的基础——没有版本管理就无法规模化 |
| 方向二: 人工可读的诊断表面 | P1 | ~2 sprints | ⭐⭐⭐⭐ | 降低采用门槛——好的错误消息是产品成熟的标志 |
| 方向三: 运行时运营可观测性 | P1 | ~3 sprints | ⭐⭐⭐⭐⭐ | 24h 自治系统的眼睛——没有仪表盘就无法运营 |
| 方向四: 优雅降级与部分恢复 | P1 | ~3 sprints | ⭐⭐⭐⭐ | 24h 自治系统的保险——没有恢复策略就无法信任 |
| 方向五: 跨运行身份与溯源 | P1 | ~2 sprints | ⭐⭐⭐⭐ | 多人/多系统共用的基础——没有身份就无法协作 |

**共同模式**: 这五个方向不是 forge-core 的新功能——它们是让 ForgeOS 从**一个人能用的强大工具**
进化为**一个团队能依赖的运营平台**所缺失的基础设施。已有 105+ 篇分析文档覆盖了
「AI 控制面」的完整性和可靠性，但这五个方向覆盖的是「产品运营面」的初始层——
版本、诊断、观测、恢复、溯源——没有它们，ForgeOS 的无人值守自治工厂愿景
将永远是技术演示，而非生产就绪的产品。
