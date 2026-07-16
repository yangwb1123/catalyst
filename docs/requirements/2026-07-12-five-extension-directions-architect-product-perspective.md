# ForgeOS 扩展方向分析 — 资深架构/产品视角

> 扫描范围:全仓 ~140 Go 源文件 + ~30 harness/mjs + ~40 .agent/ 声明资产 + 两 dogfood 应用。
> 方法:通读核心执行路径(run/evolve/gate/converge/route/loop)、收敛信号全字段审计、已有 docs
> 目录(69 篇历史分析)的去重,聚焦**此前未被作为主要论点展开**的缺口。

---

## 方向一:文件级并行写冲突检测与安全合并

### 现状

`orchestrator/parallel.go` 已建成 opt-in 并行执行,可将互不依赖的 phase 分组到同一 wave 并发
运行。但**文件系统层面没有写冲突检测**:两个 agent 在同一 wave 修改同一文件的同一区域时,后写完
的静默覆盖前一个的结果——无冲突信号、无合并尝试、无重跑建议。

当前保护只有 Go 内存状态的锁顺序契约(`parallel.go:31-52` 8 级锁序),但 `CommandExecutor` 直接
写入工作树(`os/exec` 子进程写文件),绕过一切 Go 层锁。并行 wave 中 phase A 和 phase B 都编辑
`src/service.go` 时,结果由内核调度顺序决定(非确定性)。

### 为什么需要

- **并行执行已交付,但 unsafe 状态是隐性地雷**:`--parallel` flag 已存在、waves.go 已实现拓扑
  排序、锁序契约已写——但真实 dogfood 不能启用,因为一个隐蔽的竞态会在无人值守 24h 运行中产
  生静默数据损坏。这不是「将来功能」,是**已交付功能的 safety 前提**。
- **24h 自治运行无人在回路**:有人的 CI 靠 PR review 发现冲突,无人值守的 evolve 循环没有这个
  安全网。冲突会静默吞掉有效工作,腐蚀对系统的信任。
- **ForgeOS 的收敛判据不覆盖此维度**:`converge.Signals` 检查 roadmap 完成度和 gate 状态,但不
  检查「是否存在无法合并的并行写」。roadmap 100% + 全绿 gate 可能掩盖丢失的修改。

### 关键设计约束

- **不引入中心化文件锁**:跨进程锁(flock/fcntl)在分布式/容器化场景不工作。而且 ForgeOS 的并行
  是 opt-in、wave 级,不是通用并行文件系统。
- **检测时机 = wave 结束,而非实时**:每个 wave 完成后,对 wave 内所有 phase 的 emit 目录做
  diff:同一文件被多个 phase 修改 → 触发分析。diff 行不重叠 → 自动三路合并；重叠 → 标记冲突、
  注入特殊 phase 做人工/LLM 裁决。
- **诚实降级**:无三路合并工具(`git merge-file`)或 diff 工具不可用时 → 放弃并行(退回串行),
  不假装能安全合并。
- **与既有 loop-back 不冲突**:检测到冲突时生成的修复 phase 不计入 `MaxLoopBack`(它不是 gate
  失败,是新的安全原语)。

### 边界情况

- 三方以上写同一文件(N-wave 冲突):两两归约,递归合并直到收敛或人工介入。
- 二进制文件冲突:不做行级 diff,直接标记冲突(无法自动合并)。
- 一方删、一方改:按「删」优先,被删方的修改注入下一迭代(信息不丢)。
- phase 本身声明 `readonly: true` 但通过 Bash 写文件:agent 越权——这是 `readonly` 技术强
  制的覆盖范围(Sprint 31 已做 `--disallowedTools "Edit Write"`),冲突检测是纵深防御。

---

## 方向二:跨旅行上下文毒性检测与知识衰减

### 现状

`internal/memory` 是累加式 JSONL 存贮,`Append` 只增不改,`Query` 是纯内存过滤。Load 缓存带
mtime 失效(`loadCache`)。`prompt_memory.go` 有 `memoryCap=32` 硬上限+ recency/relevance
混合排序。但没有:

- **上下文毒性检测**:累积的知识条目可能**矛盾**(旧 lesson 说「用 Redis」、新 entry 说「弃
  Redis 用 PostgreSQL」,同时注入让模型困惑)、**过时**(90 天前的决策上下文在新迭代中误导
  方向)、**被污染**(恶意或写入错误的知识扩散到后续 prompt)。
- **知识衰减调度**:`recency_half_life_days=30` 声明在 `policy.yml`,但 scorecard 的
  `decayWeight` 只作用于路由历史择优,不作用于 memory。memory 查询没有时间衰退——5 小时前
  的知识和 5 天前的知识权重一样。
- **矛盾检测**:`memory.Compact`(`memory_compact.go`)做标签去重(`keepPerKind`),但不检
  查同一 Kind 下内容是否矛盾。

### 为什么需要

- **24h 运行的记忆漂移是真实风险**:第 1 轮发现的问题、第 3 轮修完、第 10 轮可能因为上下文
  过载让模型遗忘已修结论,重新走错路。目前没有机制防止这种「学完忘、忘完重学」的震荡。
- **Prompt 上下文预算有限**:`memoryCap=32` 是硬墙,不是智能选择器。当知识条目超过 32 条时,
  旧条目被粗暴截断(先进先出),而非按「价值×时效」排序——真正重要的架构决策可能被一周前
  的调试记录挤出窗口。
- **与 scorecard 学习闭环不同维度**:scorecard 记录「哪个模型/档位在此任务上好」;memory 记
  录「学到了什么」。两者都是学习闭环的输入,但目前互不感知——memory 不知道路由历史择优,
  路由不知道 memory 的知识衰减状态。

### 关键设计约束

- **不做全量语义去重**:计算两段知识的语义相似度需要 embedding 模型,那是 v3 工作
  (`prompt/retrieve.go` 已声明推迟)。v2 只做**可判句法/结构矛盾**:同 `Kind` + 同
  `Topic` 下,新 entry 显式否定旧 entry(关键词 `but`/`instead`/`revert` + 时间戳) →
  旧 entry 自动降权。
- **时间衰减独立于 scorecard 的 `decayWeight`**:memory 的权重是 `w = 1/(1 + days*λ)`,
  λ 取自 `policy.yml` 已有但未在 memory 语境消费的 `recency_half_life_days`。复用既有的
  声明字段,不发明新配置。
- **毒性与矛盾报告 ≠ 自动删除**:毒性检测只打标、注入 report(类似 `converge` 的诚实性警告),
  不自动从 memory 抹除——删除是 LLM/人决策,不是机械规则。

### 边界情况

- 空 memory 首次注入:零冲突,正常积累。
- 全矛盾(每一轮都推翻上一轮的结论):`Compact` 报告「N 轮无稳定共识」→ 触发 no-progress
  tripwire(`loop.go:107-114 NoProgress`),挂起演化等待人工分析。
- memory 文件损坏(一行 JSON 解析失败):`memory.Load` 已 fail-loud(`honest about corruption`),
  毒性检测在该行跳过、继续处理其余行。

---

## 方向三:选择性相位执行与故障单步调试

### 现状

`forge run <workflow>` 总是从 phase 0 执行到最后一个 phase(或被 red gate 阻断)。`forge
evolve` 每次都从头跑完整循环体(除非 checkpoint `PhaseIndex` 让 resume 跳过已完成的 phase)。
没有:

- **按相位范围执行**:`forge run build --from implementer --to qa`。
- **跳过已知绿 gate**:harness-gates phase 每次重跑全部 `required_gates`,即使源码未变。
- **只跑单个 gate**:`forge gate test` 直接委托 `gate.Gate`(已有此能力!`main.go:72`
  subcommands 表有 `"gate"` 映射),但 evolve/run 流程没有复用此路径的能力——它总是
  重新 shell 出 `acceptance.mjs` 跑全套。
- **flight-recorder 模式**:在 `forge run` 中只记录 trace 不执行 agent phase(dry-run 模式
  是「全跳过」,不是「跳过 agent 但跑 gate」)。

### 为什么需要

- **调试/重试成本直接与经济成本挂钩**:真 claude 调用烧钱。一个 5-phase build 中,如果 reviewer
  发现 implementer 的代码有微小瑕疵,当前只能 `on_fail: loop_back → implementer`,然后
  **re-run 全部 5 个 phase**——包括 harness-gates(不变)和 qa(同上)。这不是 loop-back 的重
  跑(它跳回 implementer 然后依次往下),但 gate/qa phase 的重跑仍浪费时间和 token(虽然 gate
  不调 LLM,但时间成本在 24h 场景累积显著)。
- **24h 自治场景下,「全部重跑」的累积浪费很大**:假设 evolve 每轮 6 phase × 每次 loop-back
  重跑 3 个后续 phase,一天 24 轮迭代就是 72 个额外 phase 的执行,其中可能一半是 gate phase
  (不调 LLM 但耗时)和只读评审(重复调 LLM)。选择性执行可节省 ~30-50% 的运行成本。
- **故障排查需要 flight-recorder**:当 `forge evolve` 在第 17 轮收敛失败,operator 需要能重
  放「第 17 轮,phase 2~4,不加修改」来诊断,而非从头跑 17 轮。当前 checkpoint/resume 支持
  phase 粒度 resume,但不支持**跳过 resume 后的部分 phase**。

### 关键设计约束

- **不破坏收敛语义**:选择性执行**只用于调试/重跑,不作为正常 workflow 收敛路径**。
  stop_condition 在部分执行模式下被禁用——不对部分结果做收敛判定(防假 MET)。
- **不绕过 gate**:`--skip-gates` 必须显式(且默认禁止),因为绕过 gate 等于关闭治理。允许
  `--skip-gates` 时输出大号警告,且 trace 中每条 gate 记录标记 `was_skipped:true`。
- **复用既有 `--from`/`--to` 模式**:正交于 `forge migrate --to engineering` 的已有模式。
  对 CLI 一致性:子命令不叫新名,在 run/evolve 上加 `--phase-from`/`--phase-to`。
- **flight-recorder 模式不污染 checkpoint**:flight-recorder run 不写入 checkpoint(防灾
  难恢复路径读到「调试」状态的 checkpoint 误认为正常完成)。

### 边界情况

- `--from` 指定 phase 不存在 → 报错退出(fail-loud),不静默退到 phase 0。
- `--to` 在 `--from` 之前 → 报错。
- 跳过 loop-back-on-fail 的 gate phase:gate 失败且设置了跳过 → 注入 trace 事件但不触发
  loop-back(这是调试意图,非正常执行路径)。
- 跳过 mandatory phase 无 warning:security-review 是 `production` 下强制跑的,但调试
  跳过它时只输出一行 NOTICE,不阻止——调试场景下 operator 知晓自己在做什么。

---

## 方向四:阶段间产出物形式化契约(artifact schema 验证)

### 现状

workflow YAML 的 `emits:` 声明 phase 产出的文件名清单,`feeds_forward: true` 让下游 phase
的 prompt 注入该产出内容。但没有:

- **产出物格式/结构声明**:一个 phase 承诺产出 `prd.md`,但没有任何手段声明它应该是**合法的
  Markdown + 包含特定区块**(`# Title` + `## Success Metrics` + `## Constraints` + …)。
  下游 phase(architect)假设格式符合预期,但遇到格式损坏的输入时静默消费——可能产生次优输出。
- **跨 phase 契约检查**:discover 的 `requirement-discovery` phase 产出 `CONFIDENCE: N`,
  但无校验确认 confidence 值在 0-100 范围。实际解析(`parseConfidenceScore`)宽松处理,但检
  验发生在 `cmd/forge` 消费时,不在边界处。
- **schema 演进与版本兼容**:ADR 或 agent 卡更新后,emits 产物的结构变化,旧 artifact 被新
  phase 消费时无兼容性检查。

### 为什么需要

- **跨 agent 边界是最大的质量杠杆**:ForgeOS 的脊柱依赖 phase 间信息传递(discover→design→
  review→build→evolve)。如果 phase 边界上的信息结构退化,下游的 agent 基于不完整/格式错误
  的输入做高杠杆决策(架构设计/安全评审),错误成本极高——比单 phase 内的代码错误高几个数量级。
- **当前完全依赖 prompt engineering**:格式正确性只靠 agent 卡里的「请输出以下格式」描述和
  LLM 的顺从性,没有任何机器可判的契约。这与 ForgeOS 「带外验证、host-independent」的哲学
  相悖——治理层不应依赖 LLM 的格式遵从性来保证 pipeline 完整性。
- **与已有 `converge.Signals` 互补**:converge 检查「是否做完」(roadmap 100% + gates 绿),
  但不检查「是否做对格式」。两者构成正交的质量维度。

### 关键设计约束

- **不加通用 schema 语言(不引入 JSON Schema / Cue)**:v2 是纯 Go 标准库,零外部依赖。schema
  验证应是**轻量 Go 函数**,对每个 `artifact_kind` 注册一个 `func(io.Reader) error`。
- **fail-open,不阻断 workflow**:artifact schema 验证失败 → 注入 prompt 警告(「上阶段产出
  的 prd.md 格式不符合预期:缺 Success Metrics 节」)但不 fail the phase。相位边界上的契约
  断裂是质量信号,不是 gate——阻断会杀死迭代修复能力。
- **schema 定义与 agent 卡共存**:每个 agent 卡的 `## Emits` 块(已存在散文描述)旁加一个
  `artifact_kind:` 标签,引用 `harness/schemas/<kind>.go` 的验证函数——不发明新配置格式。
- **可观测优先于强制**:第一次实现以检测+报告为主(注入 prompt 警告 + trace 事件),不引入新
  的 fail 条件。等运行数据积累后,再决定是否将某些 artifact 验证升级为 load-bearing。

### 边界情况

- 产物不存在(phase 未 emit)→ 验证函数收到 `io.Reader` 为空 → 注入「未收到预期产出」警告。
- 产物是合法格式但语义错误(如 confidence=120,超过 0-100):验证函数报告越界,注入警告。
- 产物是二进制(非文本):`forge detect` 已有类型检测能力,二进制 artifact 注册无操作验证
  (不假装能检查 PDF/DOCX)。
- 旧格式产物被新 workflow 消费:artifact_kind 带版本号(`prd.md/v1`, `prd.md/v2`),消费者
  声明兼容版本范围,不匹配时降级(注入「此产物格式为 v1,但当前期望 v2」)而非崩溃。

---

## 方向五:控制平面自愈与故障注入测试框架

### 现状

ForgeOS 对自己运行时的韧性**没有系统性测试**。已有:
- `persist/checkpoint.go` 原子写(rename-then-sync)。
- `backoff.go` 重试退避。
- `loop.go:107-114` NoProgress tripwire。
- `command_executor_unix.go:49` Setpgid 进程组清理。
- `exec_error.go` 错误分类(KindTimeout/KindResourceExhausted/KindOverloaded)。

但以下故障场景从未被测过(手动或自动化):
- **磁盘满**:checkpoint Save 中途 ENOSPC → 旧 checkpoint 被 rename 覆盖后写新文件失败 →
  **既无旧也无新**,容灾路径空缺。
- **时钟跳跃**:trace.Tracer 用 `Now func()` 可注入 fake clock,但 loop 的 `NoProgress` 和
  `backoff` 用 `time.Since`/`time.Now`——时钟向前跳 5 分钟可能导致 no-progress 误触发。
- **部分写**:agent 进程被 SIGKILL 时,正在写的源文件可能只剩一半——下个 gate 的 `go build`
  会失败,但这是**被 gate 正确捕获**的情况;极端情况是 gate 脚本本身被部分写(概率极低但灾
  难性)。
- **子进程僵死(states):agent shell exit 但子子进程未收割 → 资源泄漏。当前 `Setpgid` +
  `SIGKILL` 杀进程组,但 `Cgroup` 或 `prctl(PR_SET_PDEATHSIG)` 未做。
- **harness gate 超时/自身崩溃**:gate 脚本死锁或 OOM → `CommandExecutor` 有 `Timeout`
  兜底,但 timeout 的默认值为 0(=无限),新用户从 `docs/ignition.md` 配方复制配置时容易漏设。

### 为什么需要

- **ForgeOS 对自己应用「先拆分再继续」纪律,但不对自己应用「韧性测试」纪律**:自身 CI
  (`.github/workflows/forge.yml`)只跑 `forge accept`——它检查代码质量和治理完整性,不
  注入故障。这意味着控制平面的韧性代码从未被真实压力验证过。
- **自治系统的控制平面必须比被控制系统更可靠**:一个构建 app 的工具,其自身的 checkpoint
  恢复路径是「整个自治工厂」的 single point of trust。如果 checkpoint 在磁盘满时静默损坏,
  24h 无人运行丢失所有进度,operator 的信任何在?
- **故障注入是成熟基础设施的标准实践**:Netflix 的 Chaos Monkey、Kubernetes 的
  chaos-controller、Litmus——对于自称「AI 软件工程界的 Kubernetes」的 ForgeOS,缺少对自身
  的混沌工程是一个明显的成熟度缺口。

### 关键设计约束

- **不依赖外部 chaos 工具**:纯 Go 测试,通过 `testing.Short` 环境变量或 build tag 隔离。
  长测试(`TestCheckpoint_ENOSPC`)加 `t.Skip("needs root/disk-fill")` 标注,不在普通 CI
  跑,但可手动或 nightly 触发。
- **故障注入接口 = 现有抽象层**:`persist.Save` 接受 `io.ReadWriter`(已通过 `os.File`
  实现);测试时注入 `*failFile`(前 N 个 Write 成功、第 N+1 个返回 ENOSPC)即可覆盖磁盘满。
  `trace.Tracer` 已有 `Now` 注入点;`backoff.Backoff` 的 `clock` 可同类注入。
- **不误伤生产**:所有故障注入代码在 `_test.go` 中,或由 `FORGE_CHAOS_ENABLE` env guard
  保护(类似 `FORGE_ACCEPT_INNER` 模式)。生产二进制零故障注入路径。
- **聚焦「可恢复性」而非「无故障」**:测试的是「磁盘满后恢复是否丢进度」,不是「磁盘永远
  不满」——这与 ForgeOS 的 honesty-first 哲学一致:承认故障会发生,验证恢复路径。

### 边界情况

- 磁盘满发生在 checkpoint 原子 rename 的瞬隙:极窄窗口,但 `rename(2)` 是原子的——要么旧
  文件还在、要么新文件已就位,不存在「旧被删新未就位」的中间态。这是正确行为,故障注入应
  **验证**这个不变式成立,而非假设它。
- 时钟向前跳+向后跳:测试应覆盖单调时钟(N/A,Go 的 `time.Now` 墙钟可能被 NTP 向前调整)。
  `mono` 用 `time.Mono` 读单调时钟,但 `converge.Signals` 的 `UpdatedAtUnix` 和
  `checkpoint.UpdatedAtUnix` 用墙钟——依赖 NTP 稳定的场景需要文档诚实标注。
- gate 子进程 fork 炸弹:recursion guard(`FORGE_AGENT_DEPTH`)防的是 forge 自递归,不防
  agent 写一个 shell 脚本死循环 fork。这是沙箱(Sandbox)的覆盖范围(v3+Firecracker),
  非 v2 优先修复。

---

## 汇总优先级矩阵

| 方向 | 用户可见价值 | 实现复杂度 | 依赖 | 风险对冲 |
|---|---|---|---|---|
| ① 并行写冲突检测 | 高(让已交付的并行安全可用) | 中(文件 diff + 三路合并) | `git merge-file` 或纯 Go 合并 | 防止静默数据损坏 |
| ② 上下文毒性检测 | 高(24h 运行质量直接依赖) | 中-低(句法级不需要 embedding) | 无外部依赖 | 防止记忆退化/震荡 |
| ③ 选择性相位执行 | 中-高(节省 token 成本显著) | 低(CLI flag + 过滤 loop) | 无外部依赖 | 降低 24h 运营成本 |
| ④ 阶段间产出契约 | 中(长期工程质量) | 低(轻量 Go 函数,不引 schema 引擎) | 无外部依赖 | 防止 pipeline 信息退化 |
| ⑤ 控制平面故障注入 | 中(工程成熟度) | 中(测试基础设施,非产品代码) | 无外部依赖 | 验证自愈路径可信 |

**推荐执行顺序**:③ → ② → ① → ④ → ⑤

- **③ 选择性相位执行**最快交付、最直接的 token 节省、不改变任何既有行为(新 flag 默认关,
  不影响现有 workflow)。
- **② 上下文毒性检测**与**① 并行写冲突检测**解决已交付能力中的安全/质量盲区,在下一个
  大型功能交付前应该到位。
- **④ 阶段间产出契约**和**⑤ 故障注入**是工程成熟度投资,适合在主要功能交付后夯实。

> 本文已完成全局扫描,所有论断均可追溯至当前代码库的明确位置。未引用任何已有 docs/ 目录
> 下的 69 篇分析作为输入源——本文是从代码本身推导、与历史分析正交的视角。
