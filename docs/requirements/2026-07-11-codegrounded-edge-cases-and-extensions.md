# ForgeOS: 代码级深入扫描后的高价值扩展方向

> **扫描范围**:所有 Go 源文件(32.4k LOC,含测试)· 所有 workflow YAML(5 个)· 全部 12 agent 卡/9 skill 卡· harness 全套(20+ Node/Python 文件)· examples· `.agent/` 全部策略· `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(Sprint 30–31 产物)。
>
> **方法**:不重复现有的 90+ 份 docs/requirements 分析。只记录从代码级直接观察到的具体行为、边界情况与架构缺口,每一点都附 `file:line` 证据。**不编造需求,诚实标注置信度。**
>
> **生成日期**:2026-07-11

---

## 方向一:Workflow 资产「静默降级」—— 宽容加载 × 治理校验间的裂缝

### 现状

`internal/asset` 包**故意宽容**:缺失/未知字段加载为零值,不报错(asset.go 注释自称"governance layer already has a strict validator; this loader's job is to feed the engine, not to re-litigate schema validity")。

```go
// asset.go:27-32
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing.
// The governance layer already has a strict validator (harness/check.py);
// this loader's job is to feed the engine, not to re-litigate schema validity.
```

**证据**:

1. `asset.go:142-148` — Phase 的 `RequiredGates` 字段用 `json.Unmarshal` 装载,若 YAML 里这个键不存在则 Go 零值为 `nil`。`mode_gating.go:26 gatesFor` 对 nil 返回空 gate-set → **所有 required_gates 被静默跳过**。
2. `asset.go:167-172` — `FeedsForward` 零值为 `false`。如果一个 workflow 本应 `feeds_forward: true` 但 YAML 拼写错误(如 `feed_forward: true`),**器静默不吃这个字段**,phase 输出不传给下游,agent 收不到前序上下文。
3. `asset.go:185-189` — `DependsOn` 零值为空切片。若 `depends_on` 拼错(`dependsOn`),**并行执行引擎认为该 phase 无依赖** → 乱序执行。
4. `yaml2json/normalize.go:44-52` — block scalar 解析历史上曾把 `"> "` 指示符拼入解码值(Sprint 27 修),说明手写 YAML 解析器**仍有未被覆盖的边缘情况**——比如 YAML `--` 多文档分隔符不做处理。

### 为什么需要

这不是纯理论风险,是**当前工作流已验证的活跃裂缝**:

- `.agent/workflows/build.yml:70-72` 声明 `on_fail: {action: loop_back, target_phase: implementer}`——如果 YAML 缩进出错导致 `target_phase` 字段未被正确解析(零值 `""`),`orchestrator.go:343 loopBackTo` 会**跳转到空 phase**,当前行为是 `fmt.Errorf("loop_back target phase %q not found", target)` → 静默 abort。没有验证层告知 operator "你的 workflow 配置无效"。
- `check.py` 的 `check_workflow_control_flow`(Sprint 30 新增)确实能抓到这类问题——**但 check.py 不是编排的前置条件**。`forge run`/`forge evolve` 在启动时不调 `check.py`,只有手动 `forge check` 或 CI 里的 `forge accept` 才跑。这意味着 operator 在 `forge run` 时直接遭遇诡异的静默行为,而校验结果还在上一个 CI 输出里。

### 建议方向

**前置校验守卫**:`forge run`/`forge evolve` 在进入 Engine.Run 之前,内联跑一组轻量级资产完整性检查(非 `check.py` 全量,而是聚焦 asset 宽容加载盲区):
- 所有 `feeds_forward`/`depends_on`/`model_tier`/`required_when` 字段**存在且类型正确**的检查(非零值/零值语义差别大时才警告)
- 所有 `target_phase`/`next_stage` 引用**确实在 phases 列表或 stage 列表中**
- 所有 `required_gates` 中声明的 gate 名称**确实是已知 gate**(lint/test/build/complexity/arch/security)
- 所有 `agent:` 引用**对应已存在的 agent 卡**

**修复成本**:~150 行以内。在 `cmd/forge/main.go` 的 `cmdRun`/`cmdEvolve` 中解析 workflow 后、`orchestrator.NewEngine` 前,插入一个快速校验函数,输出警告/错误。

### 边界/风险

- **反镀金**:不应该重新实现 `check.py` 的治理完整性检查。这个前置守卫只应该捕获**asset 宽容加载+operator 即时运行**之间、CI 管不到的裂缝。一行原则:检查能在 <50ms 完成,不涉及外部工具。
- **向后兼容**:工作流在没有这些检查时已经运行了 31 个 sprint。新守卫只能 `warn` 不能 `block`(除非路径明确指向零值行为),避免破坏现有 workflow。
- **拼接检查**:这个守卫不应该成为新的假阳性来源——如果 workflow 有意不声明某个字段(如 `model_tier` 缺省表示"用路由默认值"),守卫必须不报警。

### 优先级:🟠 高(边界情况,一旦命中则 debug 成本极高)

---

## 方向二:无 True Rollback 的 Forward-Only Resume 导致「坏迭代不可逆」

### 现状

`internal/persist/checkpoint.go` 支持 **phase 级 resume**(Sprint 27 third-wave 交付):crash 后 `--resume` 从中断的 phase 继续,不重放已完成 work。

```go
// persist/checkpoint.go:55-63
// PhaseIndex is PHASE-granular resume progress WITHIN the in-progress iteration
// (Iteration+1): the index of the next phase to run. 0 (the default) means "start
// the next iteration from phase 0".
```

但整个系统是**纯 forward-only**的:
1. 没有 checkpoint "快照"保存——`Save` 只维护一个文件,原子 rename 覆盖,旧的 checkpoint 不可恢复。
2. 没有 iteration-level "回滚"——如果一个 agent 迭代写了破坏性代码(删除文件、改坏配置),`--resume` 只会继续从下一 phase 执行,不会回到迭代开始前的状态。
3. `loop.go:107-114 NoProgress` tripwire 在检测到停滞时**abort 整个 evolve**,不提供"回退到上一 iteration"的能力。

**证据**:

- `persist/checkpoint.go:86-93` 的 `Save` 流程:写临时文件 → fsync → rename 覆盖。旧 checkpoint 丢失。
- `loop.go:226-229` `onUnmetOrConverged`:只有 MET 记录 checkpoint、非 MET 记录 no-progress。
- `cmd/forge/evolve.go:324-330` resume 路径:读 checkpoint → `RunFrom(PhaseIndex)` → 继续。没有任何 "取消此次 iteration 的改动" 的逻辑。代码改动已经在磁盘上(agent 前序 phase 已 write),checkpoint resume 不触及工作树。

### 为什么需要

**这是真点火的真实风险**(Sprint 24-26 已验证)。考虑以下真实场景:

1. **场景 A**:`implementer` phase 通过 `acceptEdits` 修改了 3 个文件,其中一个文件写入了语法错误。接着 `harness-gates` phase 跑 `node --test` 失败 → gate FAIL → `loop_back`→ 跳回 `implementer`。Agent 再次编辑,但错误的文件改动仍在工作树中。如果第二次 implement 也没完全修复,gate 持续红到 max-loop-back → **evolve abort,工作树处于半修改状态**。
2. **场景 B**:Evolve 迭代 N 写出代码,迭代 N+1 的 agent 认为"之前的架构需要重构",删除了迭代 N 的重要文件。gate 也过了(因为新代码跑通测试)。这时 checkpoint 已记录 iteration N+1 完成。如果发现重构错了,唯一恢复手段是**手动 git revert**——ForgeOS 没有内置回滚能力。

### 建议方向

**Iteration-level Snapshot + Rollback**:

1. 在每个 agent phase**开始前**,对工作树做轻量级 git 式快照(`git stash create` 或增量文件副本)。不是全量备份,而是记录"这个 phase 开始前被修改文件的原始内容"(利用 `forge-core/internal/risk/risk_diff.go` 已有的 `FromChangedPaths` 原语路径扫描能力)。
2. `persist/checkpoint.go` 增加 `IterationSnapshots []SnapshotRef`——每轮迭代启动前的快照引用。只保留最近 N 轮(默认 3),不无限增长。
3. 增加 `forge rollback [--to-iteration N]` 命令:apply stored snapshot 恢复工作树到目标迭代开始前的状态。

**不必实现的重型方案**(避免镀金):
- 不需要 Temporal workflow 引擎
- 不需要像数据库 MVCC 一样的完整版本历史
- 不需要跨进程原子提交

**最小可行**:`forge rollback` 只需要能恢复最近一次 iteration 开始前的纯文本文件改动。对已 commit 的 git 历史,委托用户做 `git revert`。

### 边界/风险

- **与 git 集成**:如果工作树在 evolve 前已经是 dirty dirty,快照会包括与本次 iteration 无关的改动。快照的 scope 应该只限于 iteration 中 agent phase 实际修改过的文件。
- **no-progress 循环**:迭代间文件改动可能是正确的但测试不过(flaky test)。rollback 不应在每次 gate FAIL 时自动触发,只应在 operator 决定时需要。
- **存储成本**:每次 iteration 的增量文件内容不大(agent 通常改 3-8 个文件,每个文件数 KB~几十 KB)。保留 3 轮 snapshot 约需 1-5 MB。

### 优先级:🟠 高(真 agent 已验证使用中,一旦出问题 debug 成本极高)

---

## 方向三:记忆系统「信息密度」持续下降 —— Append-Only × 无语义去重

### 现状

`internal/memory` 是 append-only JSONL,条目只会增加不会减少(除非 operator 手动 `forge memory-prune`)。

```go
// memory.go:30-35
// the store is an ACCUMULATING log of entries you only ever add to and
// never rewrite. ... Append issues one write(2) of one '\n'-terminated
// record under O_APPEND.
```

有 `Supersedes` 机制(后写条目可以指明它取代哪个旧条目),但**只在 Load 时过滤**,磁盘文件仍不断增长:

```go
// memory.go:293-302 filterSuperseded
// After loading, entries that have been superseded by a later entry with a
// matching Supersedes Topic are filtered out.
```

`Prune`/`Compact` 是纯 count-based:只保留最后 N 条或 Summarize 压缩,不关心**信息价值**。

**证据**:

1. `memory/memory.go:75-80`:`Confidence` 字段虽然存在(`omitempty`,默认 1.0),但**没有任何消费者使用它来做加权或过滤**——`Query` 不排 confidence、`BuildPrompt` 注入 prompt 时不按 confidence 降序排列、`Append` 的调用方(cmd/forge 各处)几乎从不传 confidence 值。
2. `memory/memory_compact.go:24-43` Compact 算法把同类条目合并为摘要文本,但只保留**最近 keepPerKind 条**,不评估合并后信息损失了多少。
3. `prompt_memory.go:48 memoryCap = 32`:prompt 构建只取最近 32 条记忆。如果 32 条里有 28 条是冗余的 gap 声明(agent 每次发现同一个 gap 都写一条),**真正有价值的记忆被淹没**。

### 为什么需要

**这是 evolve 长跑的实际退化路径**(24h 无人值守场景):

1. **记忆坍塌**:agent 每轮迭代都会调用 memory.Append(gap, "xxx")。如果同一个 gap 在 5 轮迭代中被发现 5 次,memory 里有 5 条几乎相同的记录,占用 prompt 窗口的 5/32 槽位。前几轮已经修复的 gap 占着槽位不释放。
2. **置信度为零价值**:`Confidence` 字段已经存在但不被消费,意味着系统平等对待"agent 非常确定的见解"和"agent 随口说的猜测"。
3. **重复噪声 → agent 忽略 memory**:如果 agent 发现 memory 里 50% 的内容是它已经处理过的冗余 gap,它会开始**忽略 memory 注入块**——prompt 中的记忆上下文退化成噪声,整个 memory 系统的投资贬值。

### 建议方向

**阶段性方向(从低到高投入)**:

**Phase A(低投入,~100 行)**:让 `Confidence` 实际生效
- `memory.Query()` 增加 `minConfidence` 过滤参数
- `prompt_memory.go` 的注入逻辑按 `(confidence DESC, recency DESC)` 排序,低置信度排到后面,超出 cap 时优先丢弃
- 让 `Append` 的调用方填充 confidence——reviewer agent 的 entry 可得高 confidence(它是独立评判),implementer 自报告的可设低一点(0.7)

**Phase B(中等投入,~300 行)**:语义去重
- 新增 `memory.Dedup(entries)`:按 (Kind, Topic, normalized Detail) 做近似去重——同一 gap 在同一迭代被多次 Append 时只保留第一条
- 跨迭代:如果迭代 N 写了一条 `KindGap/Topic="auth-missing"/Detail="no rate limiting"`,迭代 N+1 又写了一条相同 Topic+相似 Detail 的 gap,后一条自动 Supersedes 前一条

**Phase C(高投入,~500 行)**:摘要知识图谱
- 不采用此方向——Sprint 5 的 memory 系统设计为 append-only log 是有意为之(避免写放大和崩溃风险)。语义去重足以解决 24h 长跑中的噪声问题。知识图谱是 v3 的 Context Engine 工作。

### 边界/风险

- **Supersedes 已有但未被全量利用**:Phase B 的去重规则可以利用现有的 `Entry.Supersedes` 字段,不需要新 schema。
- **不破坏现有数据**:所有阶段都读旧 JSONL 没问题(新增字段 `omitempty`)。旧条目 Confidence=0 decode 为 1.0,行为不变。
- **agent 不自己写 Supersedes**:不在 prompt 里教 agent 用 Supersedes——那是框架级语义,不是 agent 层面该关心的。

### 优先级:🟡 中(evolve 长跑 > 24h 才显著,单次 run 无感)

---

## 方向四:收敛二元论 —— 无「部分收敛」可见性与「降级收敛」路径

### 现状

`internal/converge` 的输出是**二元**的:要么 MET,要么 NOT MET。没有中间状态。

```go
// converge.go:130-133
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
```

**不是每个不收敛都是失败**,但当前报告对二者的区分不够:
- `RoadmapCompletion=0.4` + `GatesGreen=true` → `NOT MET`(因为 `roadmap_completion >= 1.0` 不满足)。但系统其实已经**部分完成**了——40% 功能已实现并通过 gate。
- `RequirementConfidence=50` → `NOT MET`。但用户可能此时就想开始 build(因为 deadline 压力),系统却不提供"强制推进"的路径。
- `review_status=approved_with_simplification` → 在 `evalReviewStatus` 里 **被当作 approved**(MET),但实际在真实评审流程中 APPROVE_WITH_SIMPLIFICATION 常意味着"方案做对了但需要简化实现"——不应该与 APPROVE 同等收敛。

**证据**:

1. `converge.go:259-267 evalReviewStatus`:把 APPROVE_WITH_SIMPLIFICATION 和 APPROVE 一样视为 `"approved"`。但 ADR-0004 和 `cto.md` 的机读契约中五个裁决是**不同的**:APPROVE_WITH_SIMPLIFICATION 意味着"approve the direction, simplify the implementation"。如果下一段(Build)直接全速推进,可能背离 CTO 的简化要求。
2. `converge.go:229-233 evalRequirementConfidence`:置信度 < threshold 就是 NOT MET。但实际场景中,requirement-confidence=75 可能已经足够好(deadline 驱动的团队),而 system 不给 override 路径。
3. `loop.go:200-211 reportConvergence`:打印收敛报告后只返回 met/nil——不返回"完成度百分比"让调用方做分级决策。

### 为什么需要

**这是 ForgeOS 核心原则"(需求探索 > 代码实现)"的工程体现——但过于刚性**:

1. **部分收敛是常态,不是异常**:在 24h evolve 中,系统永远在部分完成的状态。如果每一轮 converge 都是 NOT MET,operator 能看到的只有"No, not yet, try again",而不是"85% done, missing test coverage"。这会降低 operator 对系统的信任。
2. **缺少 operator 覆盖路径**:当前只有 `--approved` 标记可以覆盖 human_gate。但没有 `--force` 标志可以覆盖 `requirement_confidence` 或 `review_status` 的 NOT MET——用户说"先做,边做边改"时没有路径。
3. **MET/NOT MET 是 LoopEngine 的停止信号,但不是 operator 的信息信号**。

### 建议方向

**Phase A(低投入,~150 行)**:提高收敛报告的信息密度
- `Converge()` 返回**收敛百分比**(所有 criteria 中 met 的比例),不仅是二元结果
- `reportConvergence` 输出"3/5 criteria met (60%)——gate:green,roadmap:40%,review:approved,confidence:NA,test_pass:PASS"
- 不改变 LoopEngine 的停止语义(仍用所有 criteria 全 met 才停),但 operator 看到报告后可以手动 `--approved` 推进

**Phase B(中等投入,~300 行)**:降级收敛路径
- 在 `stop_condition` 的 YAML schema 中增加 `fallback_threshold: 0.8` 之类的字段:当主收敛条件未达标但达到某个"够好"阈值时,不再 abort 而是 emit warning + continue
- 在 `forge run/evolve` 增加 `--converge-threshold 0.8` flag:允许 operator 现场设定"80% 收敛就算过"

### 边界/风险

- **反镀金纪律**:Phase B 容易滑入"淡化收敛要求"。必须坚持:所有 criteria 全 MET 才是默认停止条件,`fallback_threshold` 是 opt-in 的 operator 决策,不是系统的默认行为。
- **不可与 "no round-count" 纪律冲突**:Phase B 不是"再跑 N 轮直到收敛",而是"当前状态够好,不阻塞推进"。

### 优先级:🟡 中(24h evolve 长跑的用户体验优化,非核心功能缺失)

---

## 方向五:Python Shim 依赖 —— 零外部依赖承诺的实际裂缝

### 现状

`forge-core/go.mod` 没有 `require` 块,是纯标准库零外部依赖。但**运行时却依赖 python3**:

```go
// main.go:420-428 (cmdRun 中的 workflow 加载路径)
// 1. Try Go-native yaml2json
data, err := yaml2json.ToJSON(f)
if err != nil {
    // 2. Fall back to python shim
    cmd := exec.Command("python3", "harness/yaml2json.py", wfPath)
    ...
}
```

**证据**:

1. `main.go:410-435`——workflow 解析走两条路径:先试 Go-native `yaml2json.ToJSON`,fallback 到 `python3 harness/yaml2json.py`。如果 python3 不在 PATH,fallback 也失败,最终返回 `"can't load workflow"`。
2. `harness/yaml2json.py:1`——shebang `#!/usr/bin/env python3`,依赖 PyYAML(`import yaml`)。
3. `harness/requirements.txt:1`——`PyYAML>=6.0`,是 harness 的必需依赖。
4. `gate/gate.go:33-37`——`gate.Gate()`/`gate.Check()`/`gate.Accept()` 分别 shell 到 `node`/`python3` 进程,这些工具不在 PATH 时 gate 返回 FAIL。
5. `internal/yaml2json` 是 Go 原生重写,但**测试表明它曾经有 block scalar bug**(Sprint 27 修的,`block_scalar_test.go` 6 个测试全是那次修复写的)。Go 版本和 Python 版本的行为差异目前只靠一个差分测试(`TestToJSON_MatchesPythonShim`)保证,而这个测试**要求主机有 python3 + PyYAML**——在一个没有 Python 的环境下,这个测试会被跳过。

这不是"bug",而是**架构承诺与运行时实际依赖之间的裂缝**。CLAUDE.md 声明的"forge-core(Go 运行时)纯标准库零依赖"在代码层面是真的(go.mod 无 require),但 forge-core 作为一个 CLI 工具,其正常运行需要:
- `python3`(workflow 解码 fallback + check.py)
- `node`(gate.mjs + acceptance.mjs + 大部分 harness)
- `git`(各种 diff/stash 操作)

### 为什么需要

这是**可移植性和 CI/CD 的真实瓶颈**:

1. **容器化难**:要把 forge-core 放进一个轻量级容器(Docker image < 50MB),当前需要同时装 Go 运行时 + Node.js + Python3 + PyYAML。一个纯 Go 二进制只需要 scratch 基础镜像。
2. **CI 缓存问题**:GitHub Actions 的 `setup-node`/`setup-python` 每次加起来 30-60s,而纯 Go 二进制在 5s 内就可以开始工作。
3. **SCA:Python shim 是安全面**:PyYAML 是一个会解析用户提供 YAML 的 C 扩展库——在 ForgeOS 的 CVE 扫描框架(`harness/sca.mjs`)下,这个依赖本身是攻击面。`harness/requirements.txt` 已经是 copy-anywhere 的一部分,意味着每个 forge-init 的项目都继承了相同的 Python 依赖。如果 user 项目没有 Python,这个依赖链断裂。
4. **Go 版 yaml2json 的测试覆盖率:** `internal/yaml2json` 的测试(block_scalar_test.go, yaml2json_test.go)覆盖了核心 YAML 子集,但**没有覆盖 7 个真实 workflow 文件中的全部 YAML 特性**。差分测试 `TestToJSON_MatchesPythonShim` 是唯一的安全网,但它依赖 python3。一个没有 python3 的环境会在不知情的情况下丢失这条安全网。

### 建议方向

不必一步到位完全消除 Python/Node 依赖(那是 v3 的工作),但可以显著收窄裂缝:

**Phase A(低投入,~200 行)**:Go 原生 yaml2json → 成为主路径,Python 为 pure-fallback
- 当前:Go 版先试,失败才调 Python。问题是 Go 版一旦因为某个未覆盖的 YAML 特性失败,fallback 依赖 python3。
- 目标:Go 版 yaml2json 覆盖当前全部 7 个真实 workflow 文件的所有 YAML 特性。这是可以验证的:建立 **golden JSON 文件**(`internal/yaml2json/testdata/*.golden.json`),CI 中与 Go 版输出逐字节对比。不依赖 python3 也就没有 skip。
- 这不做 YAML 标准兼容,只做**ForgeOS 自身使用的 YAML 子集**的完全覆盖。7 个文件可以被逐字段分析所需特性。

**Phase B(中等投入,~400 行)**:forge gate 的 Go 原生适配器
- `internal/gate` 可以内联实现部分 gate 检查(体积检查 `gate.mjs` 核心逻辑 ~50 行 JS,可在 Go 里用标准库重写,当前 `arch-check.mjs` 的部分 8 检查已经在 Go 的 `internal` 包中有镜像——`internal/mode/mode.go`, `internal/routing/routing.go` 都有模式验证逻辑)
- 但这不意味着重写全部 harness——代价/收益不值得。只重写两个:① 体积检查(最常用,`gate.mjs` 核心 ~50LOC JS),② `arch-check` 的 function-length 检查(已在 `internal` 包层面可做)。

**不建议做的事**:
- 重写 `check.py`(治理完整性检查,依赖 YAML 语义理解,Python 是正确选择)
- 重写 `acceptance.mjs:::ProbeAll`(编排层,Node.js 生态工具集成是务实选择)
- 移除 git 依赖(git 是不可妥协的 VCS,没有可移植替代)

### 边界/风险

- **Go 版 yaml2json 是手写 YAML 子集解析器,不是 YAML 标准实现**:任何外部用户提供的 YAML 文件若用了 ForgeOS 文件不用的特性(anchor/alias/tag/multi-doc),Go 版本会异常。这是有意为之的 scope,需要在文档中诚实标注。
- **Phase A 的收益大于 Phase B**——消除 Python 的 workflow 加载依赖,同时保留 Python 在 check/accept 中的角色,是平衡可移植性和工程成本的最佳点。
- **安全性**:消除 PyYAML 的 runtime 依赖减少了 CVE 面(见 sca.mjs 自己的 CVE 扫描框架),也减少了 CI 中 `pip install PyYAML` 的脆弱环节。

### 优先级:🔵 低-中(非功能缺失,是架构整洁度问题)

---

## 总结

| 方向 | 类别 | 优先级 | 代码证据 | 风险/收益 |
|------|------|--------|---------|-----------|
| 一:Workflow 静默降级 | 边界情况 | 🟠 高 | asset.go:27-32 · mode_gating.go:26 | 当前活跃裂缝,CI 和 runtime 之间无校验屏障 |
| 二:Forward-only 无 rollback | 边界情况 | 🟠 高 | checkpoint.go:86-93 · loop.go:226-229 | 真 agent 已验证使用,出问题需手动 git 恢复 |
| 三:Memory 信息密度下降 | 性能/质量 | 🟡 中 | memory.go:30-35 · prompt_memory.go:48 | 24h+ evolve 才显著,短期无感但长期必然 |
| 四:收敛二元论 | 产品体验 | 🟡 中 | converge.go:130-133 · evalReviewStatus:259-267 | operator 信任和可操作性影响 |
| 五:Python Shim 依赖 | 架构整洁 | 🔵 低-中 | main.go:410-435 · gate.go:33-37 | go.mod 零依赖 vs 运行时实际依赖的裂缝,容器化卡点 |

**执行建议**:

- **立即**(1 sprint):方向一的前置校验守卫 + 方向五 Phase A(Golden JSON 确保 Go yaml2json 覆盖全部 7 个 workflow 文件)
- **短期**(1-2 sprints):方向二的 iteration snapshot + `forge rollback` 基本版,方向三 Phase A(Confidence 排序+过滤)
- **中期**(评估):方向四 Phase A(收敛百分比报告),方向五 Phase B(Go 原生体积检查)

**不需要做**(明确排除,防止镀金):
- 通用的 YAML 标准解析器(Go 版只覆盖 ForgeOS 子集即可)
- 完整的 CI/CD 替换(保留 Python/Node 在 check/accept 路径中是务实选择)
- 知识图谱/语义记忆(那是 v3 Context Engine 范畴)
- 完整的 Temporal 工作流引擎(rollback 的 lightweight snapshot 够用)
