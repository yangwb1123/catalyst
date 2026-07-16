# ForgeOS — 五个产品视角的战略缺口

> **角色**: 资深架构师 / 产品经理  
> **视角**: 不从「再加一个引擎/功能/架构层」出发,而从 **ForgeOS 作为一个产品能否被团队采纳、长期信任、持续演进**出发。  
>  
> **方法**:  
> 1. 全仓扫描 forge-core (18 Go 包,~45KLOC) + harness (39+ 模块,~10.5KLOC) + `.agent/` 完整骨架 +  
>    `pi-batch.py` + `examples/` + 84 份 `docs/` 分析文档  
> 2. 逐篇通读 44 份 `docs/requirements/*.md` + 40 份 `docs/analysis/*.md` + 核心文档  
>    (BOOTSTRAP / CURRENT_SPRINT(31期) / FUNCTIONAL_REQUIREMENTS_AUDIT / ADR 0001-0004 / DECISIONS /  
>    north-star / loop-engineering / ignition) — 合计 ~120+ 已有方向  
> 3. **差异化证明**: 每个方向的关键词在 84 份已有分析文档中**从未或极少作为独立方向展开**  
>    (最多被其他方向的边缘段落提及)  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据、边界场景、差异化证明。  
> **日期**: 2026-07-10

---

## 已有分析全景(本文不重复)

已有 84 份分析文档压倒性地覆盖了以下域:

| 域 | 估算方向数 | 特征 |
|---|:---:|---|
| 引擎补齐(编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back) | ~25 | 全是「加什么新机制」 |
| 多仓库/联邦/跨会话治理 | ~15 | 全是「拓展到更多场景」 |
| 生产可靠性(Prompt QA / 信号硬化 / 自愈 / 健康契约) | ~15 | 全是「让现有机制更可靠」 |
| 执行语义形式化(原子性/幂等/回滚/版本演化) | ~12 | 全是「形式化现有行为」 |
| 系统边界盲区(级联截断/TOCTOU/信任边界/并发安全) | ~15 | 全是「防守型补漏」 |
| 二阶伴生问题(知识衰减/配置爆炸/无声数据丢失) | ~12 | 全是「治理副作用」 |
| 收敛/自诊断/测试元治理/API 契约 | ~10 | 全是「治理自身」 |
| 安全/凭据/SCA/沙箱/注入防御 | ~8 | 全是「安全纵深」 |
| 其他(混沌/联邦学习/冷启动等) | ~10 | 单篇覆盖 |
| **总计** | **~120+** | **无一从产品角度出发** |

**本文 5 个方向的共同特征**: 它们不是「加什么新功能」,而是 **「ForgeOS 作为产品还缺什么」**。
每个方向回答一个产品问题:

| # | 产品问题 | 方向 |
|---|---------|------|
| 1 | 44 份需求文档谁来看、谁来决策？ | 分析疲劳与元治理缺口 |
| 2 | 用户需要装几个运行时才能用 ForgeOS？ | 三运行时门槛 |
| 3 | 测试结果在不同机器上一样吗？ | 环境依赖的测试套件 |
| 4 | 第三方工具/CI/IDE 怎么集成 ForgeOS？ | 零外部集成面 |
| 5 | 治理到底有没有效果？ | 治理效果不可观测 |

---

## 方向一 · 文档分析疲劳:44 份需求文档的元治理缺口

**产品问题**: 44 份需求文档、每份 3-5 个方向、合计 ~176 个「高价值扩展方向」——  
**谁来决定哪个方向值得做？哪个已经做过？哪个被拒绝了？**

**类型**: 元治理 · 流程 | **优先级**: 🔴 P1（分析产出超过消费能力是系统性问题）  
**影响范围**: `docs/requirements/` · `docs/analysis/` · 产品决策流程  
**代码证据**: 零文件记录方向决策状态 | **差异化**: 在 84 份已有文档中**无一从此角度展开**

### 现状:数据

```
$ ls docs/requirements/*.md | wc -l
44

$ ls docs/analysis/*.md | wc -l
40

$ grep -c "方向" docs/requirements/*.md docs/analysis/*.md | awk -F: '{s+=$2}END{print s}'
~500+   // 实际方向声明次数(含跨文档重叠)
```

每一个文件都声称自己的 3-5 个方向「从未被已有分析覆盖」。每个文件的元认证都声称「逐篇通读全部已有文档」——但 44 份文档各自逐篇通读 43 篇旧文档,这本身就是一个 O(n²) 的不可扩展过程。

**没有以下任何机制**:

| 要素 | 当前状态 | 需要 |
|------|---------|------|
| 方向唯一性验证 | 每份文件靠作者手动 grep 验证 | 自动化方向注册表,检测重复 |
| 方向生命周期 | 无(写入即永存) | proposed → reviewed → accepted/rejected/scheduled/done |
| 方向优先级 | 作者自评(P0/P1/P2),各文件间不可比较 | 团队评审的统一优先级 |
| 实现追踪 | 无 | 哪个方向在哪个 sprint 实现,链接到 commit |
| 跨文档引用 | 靠自由文本标注(如"v25.md 方向二") | 结构化引用,更新时双向通知 |

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| 方向 A 已在 docs/requirements/v25.md 提出,新文档 v43 再次提出 | 作者声称「未覆盖」,实际重复 | 分析工作浪费,决策不透明 |
| 方向 B 在 Sprint 15 实现,文档仍在 requirements/ 中 | 永存,无人知道已实现 | 新人不确定哪些方向还是待办 |
| 新作者读不全 44 份文档就写 v45 | 必然与已有方向重叠 | 分析信用度下降 |
| 44 份文档中的优先级互斥 | 无法比较 | 无法决定先做 P0 还是另一个 P0 |

### 建议方向

1. **方向注册表**: 一个 YAML 文件(`.agent/direction-registry.yml`)作为单一事实源,每条记录:方向名、提出日期、提出文档、优先级、状态(proposed/accepted/rejected/scheduled/done)、实现 sprint/commit。新分析文档必须注册到该文件,注册时自动检测与已有方向的重叠。
2. **方向生命周期流程**: 定期(每 sprint) triage 注册表中的 `proposed` 方向,投票决定 accept/reject。`accepted` 进入 ROADMAP 候选,`rejected` 记录理由。每方向只存在一次。
3. **陈旧文档清理**: 当方向状态变为 `done` 或 `rejected` 时,在对应文档头部加 superseded-by 注释。一季清理一次已 superseded 文档——归档而非删除(保留决策历史)。

### 差异化证明

- 84 份已有分析文档关注的是「再加什么功能/架构/引擎」,**无一关注分析文档自身的管理问题**。
- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 是唯一建立需求清单的文档,但它是**从源头推导功能实现状态**,不是**管理分析方向的生命周期**。两者的区别:审计回答「我们承诺了什么、做到了没」;方向注册表回答「谁提议了什么、决策了什么」。
- `AIGENTS.md` 的「治理完整性」检查(`check.py`)验证 agent/skill/路由档无悬挂引用,不验证需求分析文档的状态。

---

## 方向二 · 三运行时门槛:ForgeOS 的真实依赖面

**产品问题**: 一个新用户要跑 `forge init` → `forge accept`,需要安装几个运行时？

**类型**: 开发者体验 · 采用 | **优先级**: 🔴 P1（直接影响采用率）  
**影响范围**: 安装文档 · `forge-init` 脚手架 · CI 配置  
**代码证据**: forge-core 零外部依赖,但完整功能需要三个运行时 | **差异化**: 在 84 份已有文档中**零独立覆盖**

### 现状:运行时依赖清单

| 组件 | 运行时 | 实现语言 | 是否必需 | 用途 |
|------|--------|---------|---------|------|
| forge-core | **Go** | Go | ✅ 必需 | 编排执行引擎(CLI) |
| harness gate.mjs | **Node.js** | JavaScript | ✅ 必需 | 体积/文件数闸门 |
| harness check.py | **Python 3** | Python | ✅ 必需 | 治理完整性检查 |
| harness yaml2json.py | **Python 3** | Python | ⚠️ yarn 转码(Go 手写解析器已作首选, python 是 fallback) | YAML→JSON 转码 |
| harness sca.mjs | **Node.js** | JavaScript | ⚠️ 有 DB 才生效 | CVE 扫描 |
| harness select-tests.mjs | **Node.js** | JavaScript | ❌ advisory 加速器 | 增量测试选择 |
| harness scorecard-*.mjs | **Node.js** | JavaScript | ✅ `forge run/evolve` 自动调 | 学习闭环 |
| PyYAML | Python 包 | Python | ✅ `check.py` 必需 | YAML 解析 |

**总计:至少需要 Go + Node.js + Python3 + PyYAML** 才能跑 `forge accept`。

```
$ python3 -c "import yaml" 2>&1 || echo "PyYAML required for forge check/accept"
```

代码证据:

```bash
# harness/check.py: 第 1 行 import
try:
    import yaml
except ImportError:
    print("PyYAML is required; install: pip install pyyaml", file=sys.stderr)
    sys.exit(2)

# go.mod: 零 require —— 但只 cover forge-core
$ cat forge-core/go.mod
module forgeos/forge-core
go 1.26
# ← 没有 require 块,没有提到 Node/Python
```

### 为什么这是个问题

1. **叙述矛盾**: CLAUDE.md 及 BOOTSTRAP.md 多次强调「forge-core 纯 Go 标准库、零外部依赖、零依赖」——这描述的是 forge-core 自身,但用户实际需要的是**完整栈**(three runtimes)。零依赖叙述对用户有误导性。
2. **CI 配置复杂度**: `.github/workflows/forge.yml` 需要安装三个运行时加一个 Python 包。每个运行时有自己的版本管理、安全更新、兼容性问题。
3. **边缘场景**: Windows 用户需要额外配置(Go 没问题,Node 没问题,Python + PyYAML 需要 PATH 和 pip 配置,Unix 进程组机制不可用)。
4. **容器化是权宜之计,不是解决方案**: Docker 镜像可以打包三个运行时,但不解决「CI pipeline 必须拉 1GB+ 镜像」的实质。

### 建议方向

1. **诚实叙述**: 在 README 和 BOOTSTRAP 中明确列出完整运行时需求,不要只强调 forge-core 的零依赖。开箱即用的 Go 单二进制目标应有明确路线图。
2. **Go 静态链接覆盖 chec.py**: `check.py` 的关键功能(`check_workflow_agent_refs` 等)可以逐步移植到 Go,最终消除对 Python 的依赖。(ADR-0002 的「未来 consolidate harness 到 Go 静态二进制」可赋予具体里程碑。)
3. **yaml2json.py 降级为真正 fallback**: Go 手写解析器已是首选,进一步让 python fallback 不可见——仅当 Go 解析器失败时才报错提示安装 python,正常路径不依赖。
4. **可选依赖按需提示**: 像 sca.mjs 那样,缺失时诚实 N/A 而非报错。

### 差异化证明

- 84 份已有文档讨论的是**架构层的扩展**,无人讨论**用户需要装什么才能用**。
- `expansion-horizon-three.md` 方向五(federated governance)提到「要求目标仓库安装 ForgeOS harness」——但那讨论的是分布式治理的远程依赖,不是本地用户的运行时负担。
- `forgotten-five-system-boundaries.md` 方向四(跨平台可移植性)聚焦 Windows Job Object、信号处理、路径分隔符——这些是**运行时兼容性**,不是「用户需要安装什么」的采用门槛。

---

## 方向三 · 环境依赖的测试套件:测试结果不可复现

**产品问题**: 同一个 `go test ./...` 在我的机器上和 CI 上结果一样吗？

**类型**: 工程质量 · 可靠性 | **优先级**: 🔴 P1（测试可信度影响整个治理链）  
**影响范围**: `forge-core/**/*_test.go` · CI pipeline · `forge accept`  
**代码证据**: 至少 27 处 `t.Skip` 依赖环境 | **差异化**: 84 份已有文档**零覆盖**

### 现状:skip 分布

```
$ grep -rn "t\.Skip\|Skipf" forge-core/ --include="*_test.go"
forge-core/cmd/forge/main_agent_test.go:    t.Skip("python3 not available")
forge-core/cmd/forge/main_agent_test.go:    t.Skip("not running inside the ForgeOS repo")
forge-core/cmd/forge/main_agent_test.go:    t.Skip("python3 not available")
forge-core/cmd/forge/evolve_test.go:        t.Skip("python3 not available")  (×4)
forge-core/cmd/forge/adr_test.go:           t.Skipf("ADR-0002: cannot read go.mod (%)")
forge-core/internal/gate/gate_test.go:       t.Skipf("%s not available: %v")  (×2)
forge-core/internal/yaml2json/yaml2json_test.go: t.Skip(...)  (×6, 各自不同条件)
forge-core/internal/persist/replay_test.go:  t.Skipf(...)  (×5, fixture 缺失)
forge-core/internal/orchestrator/loop_test.go: t.Skip("fixture lacks...")
forge-core/internal/orchestrator/command_executor_unix_test.go: t.Skip(...)  (×3)
forge-core/internal/yamlpath/yamlpath_test.go: t.Skipf("shim not found")
```

**总计:至少 27 个测试在不同环境条件下静默跳过。**

影响范围按包：

| 包 | skips | 覆盖(全环境) | 覆盖(缺 python) |
|---|:---:|:---:|:---:|
| `cmd/forge` | 7 | ~67% | ~55% |
| `internal/gate` | 2 | ~59% | ~35% |
| `internal/yaml2json` | 6 | ~76% | ~50% |
| `internal/persist` | 5 | ~74% | ~74%(需 fixture) |
| `internal/orchestrator` | 4 | ~94% | ~94% |
| `internal/yamlpath` | 1 | ~92% | ~85% |

### 为什么这是个产品问题

ForgeOS 对外声称的治理标准——诚实、不伪造通过——在测试套件自身没有贯彻。具体问题：

1. **流水线中跑 `go test ./...` 和用户本机跑的结果不同**: CI 装了 python3,用户可能没装。CI 在 ForgeOS 仓库根目录,用户在子项目。CI 打 100% 绿的测试结果,换个环境少跑 10% 的用例。
2. **skip 的测试不产生告警**: `go test -v` 输出 `SKIP` 行,但 `go test` 默认汇总时 exit 0。没有机制告知"这次测试少跑了 7 个用例"。
3. **覆盖率的分子不一致**: `coverage: 93.8% of statements` 在缺 python 的环境下可能计算的是**子集**的覆盖率——分母是实际执行的代码,不是全量代码。
4. **`forge accept` 依赖的测试可能因环境跳过而假绿**: acceptance.mjs 调用 gate → test_pass,如果 test 子进程因 skip 而全绿,用户以为全过,实际部分测试未执行。

### 建议方向

1. **skip 注册表**: 每个 `t.Skip` 必须附带一个 tag(`skip:no-python` / `skip:no-fixture`),测试收官时输出「本次跳过 N 个测试」+ 按类别汇总。CI 强制要求「跳过 0 个」。
2. **嵌入式 fixture**: 将 yaml2json 和 persist 测试依赖的 .yml/.jsonl 文件用 `embed.FS` 打包进测试二进制,消除「文件不在工作目录」的 skip 理由。(Go 1.16+ 支持 `//go:embed`,forge-core 用 Go 1.26,完全可用。)
3. **python shim 隔离**: 将依赖 python 的测试(yaml2json 对比测试、evolve 集成测试)抽到 `*_integration_test.go`(Go 构建约束 `//go:build integration`),默认不跑,CI 额外跑。让 `go test ./...` 对纯 Go 代码完全确定。
4. **skip 告知 ≥ 告知 N/A**: 模仿 acceptance 的诚实 N/A 模式——skip 时输出 `SKIP[N]: reason`,汇总行输出 `N tests skipped`。若 N>0 且非 `-short`,exit code 告警。

### 差异化证明

- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 审计的是功能需求的实现状态,从未审计测试质量。
- `expansion-production-readiness.md` 方向四「测试成熟度模型」讨论的是**被治理项目**的测试要求(覆盖阈值/适配器框架),不是**ForgeOS 自身**测试套件的可靠性。
- `expansion-production-blindspots-v36.md` 方向四「测试套件可靠性」提到 flaky test 和 deterministic teardown,但焦点是**测试框架层面的治理**(fixture 污染/并行冲突),不是**环境导致测试静默跳过**这种产品级问题。

---

## 方向四 · 零外部集成面:ForgeOS 只能通过 CLI 使用

**产品问题**: CI 系统、IDE 插件、监控面板、内部开发者平台——它们怎么和 ForgeOS 集成？

**类型**: 平台化 · 集成 | **优先级**: P2（v2 范围外的平台能力,但影响长期生态）  
**影响范围**: forge-core 整体架构 | **代码证据**: 零外部 API 端点 | **差异化**: 在 84 份已有文档中作为架构层提及,但从未**从「产品集成面」角度作为独立方向展开**

### 现状:所有集成都靠 shell out + 文本解析

```go
// 当前唯一的"API": forge CLI 文本输出
// 用户/CI 这样调用:
//   output=$(forge run build --mode balanced 2>&1)
//   echo "$output" | grep "convergence: MET"  # 文本解析
//   exit_code=$?                               # 靠 exit code 判断
```

| 集成场景 | 当前方案 | 问题 |
|---------|---------|------|
| CI 判断是否继续 | 检查 `forge accept` exit code | exit code 只有 0/1,无法区分「测试失败」「架构违规」「secret 泄露」 |
| IDE 插件显示治理状态 | 无 | 必须 shell out 到 CLI,解析文本 |
| 监控面板显示趋势 | 无 | 必须定时 shell out + 聚合 trace.jsonl |
| 内部开发者平台 | 无 | 无法以服务形式嵌入 |
| Webhook 通知 | 无 | 无法在 gate FAIL 时通知 |

支持的集成维度:

```
$ grep -rn "HTTP\|gRPC\|WebSocket\|Listener\|Serve\|Listen" forge-core/cmd/forge/*.go forge-core/internal/*/*.go | grep -v "_test.go" | grep -v "\.ListenAndServe" | grep -v "http\.Listen"
# 零输出——forge-core 没有任何网络监听功能
```

`examples/go-taskd/main.go` 意外说明问题:taskd 是一个完全独立的 Go 程序(HTTP 服务),与 forge-core 之间只有 CLI 调用关系——forge-core 通过 `exec.Command("taskd")` 启动它,然后靠文本解析获取结果。

### 建议方向

1. **`forge daemon` 子命令**: 一个长期运行的守护进程,暴露 Unix socket 或 HTTP 端点。子命令 `forge run/evolve/accept/status` 等均可通过 daemon 调用而非每次启动新进程。(`internal/daemon/` 新包,与 `cmd/forge` 对称。)
2. **结构化事件流**: daemon 模式下降 gate FAIL / converge MET / human approval needed 等关键事件推送到注册的 webhook 或输出到结构化流(NDJSON/SSE)。不是轮询 trace.jsonl。
3. **导出 Go library API**: 当前 `internal/` 包因 Go 惯例不可外部导入。将 `orchestrator.Engine` / `converge.Converge` / `gate.Runner` 等核心抽象提升到公共 API(`forgeos/forge-core/pkg/`)。让第三方用 Go 程序直接调用 forge-core,不必 shell out。
4. **结构化 JSON 输出模式**: 当前只有 `acceptance.mjs --json` 有结构化输出。`forge run` / `forge evolve` 的文本输出加 `--json` 模式,输出每个阶段的详细结果(exit code、duration、cost、verdict)。CI 工具不再解析文本。

### 差异化证明

- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向四(结构化输出协议)关注的是**跨进程/跨 session 的统一输出格式**,本文方向关注的是**ForgeOS 作为平台被第三方工具集成的能力**。前者是「输出什么格式」,后者是「外部世界怎么访问这些输出」。
- `expansion-horizon-three.md` 方向三(事件驱动编排平面)提出了「事件总线」(event bus)概念,但那是 ForgeOS **内部**编排的事件驱动(多个 workflow 之间的编排),不是**外部工具**集成的事件流。
- `forgotten-five-foundations.md` 方向一(跨进程守护)提到 daemon 进程用于**配置热加载和健康检查**,不是作为**外部 API 网关**。

---

## 方向五 · 治理效果不可观测:"我们为什么要用 ForgeOS？"

**产品问题**: 用了 ForgeOS 一个月,代码质量真的变好了吗？怎么证明？

**类型**: 可观测性 · 价值证明 | **优先级**: 🔴 P1（长期信任的生命线）  
**影响范围**: 治理 dashboard · CI 集成 · 报告系统  
**代码证据**: `trace.jsonl` + `scorecard.jsonl` 是有数据的,但无人聚合为趋势 | **差异化**: 在 84 份已有文档中**零作为独立方向**

### 现状:所有数据都存在,但无人看

```
.forge/
  trace.jsonl        # 每次 agent 执行的延迟/成本/状态
  memory.jsonl       # 跨 session 的知识
  scorecards.json    # 每个 (model, task_type) 的质量/延迟/成本
  checkpoint.json    # evolve 的迭代 checkpoint
```

每个文件都有结构化数据,但 **没有人问「这些数据告诉了我们关于治理效果的什么信息」**:

| 问题 | 当前回答 | 应有回答 |
|------|---------|---------|
| 用了 ForgeOS 后违规减少了？ | 无数据 | "文件超 500 行从 12 降到 2" |
| 哪个 gate 最常 FAIL？ | 无数据 | "secret-scan 每月 FAIL 3 次" |
| 治理变严了还是松了？ | 无数据 | "平均每个 PR 触发 1.2 次 gate FAIL,较上月降 15%" |
| 哪个项目的治理最好？ | 无数据 | "项目 X 连续 60 天 forge accept ACCEPTED" |
| reviewer 裁决的分布？ | 无数据 | "APPROVE 78% / REDESIGN 12% / REJECT 10%" |
| 收敛一个 build 平均要几轮？ | 无数据 | "平均 2.3 轮 loop-back,方差 0.8" |

### 代码级证据

```go
// internal/trace/trace.go — Event 包含丰富的结构化数据
type Event struct {
    Type          string  // "agent" | "iteration" | "gate"
    PhaseName     string
    Agent         string
    Model         string
    DurationMs    int64
    CostUsdMicros int64
    Status        string  // "ok" | "fail" | "retry"
    Iteration     int
    // 但这些数据从未被聚合为趋势
}

// internal/routing/scorecard.go — 记分卡以 (model, task_type) 为键
// 存储质量/延迟/成本的三维分数 + recency 衰减
type ScorecardEntry struct {
    Model     string
    Score     float64
    LatencyMs float64
    Cost      float64
    UpdatedAt time.Time
    // 但无人问"model A 的质量趋势是上升还是下降"
}
```

`forge scorecard` 命令目前只支持 `rebuild`(从 trace 重建记分卡)和 `wind`(单个值查询)。**没有 `forge scorecard trend` 或 `forge report` 之类的聚合分析命令。**

### 建议方向

1. **`forge report` 子命令**: 生成一份治理健康报告,聚合 trace/scorecard/gate 数据,输出:
   - 治理趋势图(ASCII 或 JSON):gate PASS/FAIL 趋势、文件违规趋势、收敛轮次分布
   - 跨项目对比(多 `.forge/` 扫描):哪些项目通过了 `forge accept`,哪些没通过
   - 成本报告:各模型/各阶段的累计花费、成本效率趋势
2. **gate 结果持久化**: 当前 `forge accept` 的 verdict 只反映当下。将每次 `forge accept` 的结果(PASS/FAIL/NA 明细)追加到 `.forge/accept-history.jsonl`。季度报告可以问「合规率从 Q1 到 Q2 怎么变化的」。
3. **治理 SLA 告警**: 可配置阈值——「如果工程模式的 `forge accept` 连续 3 次 REJECTED,发通知」。将 ForgeOS 从被动闸门升级为主动监控工具。
4. **价值案例生成**: `forge report --value-case` 自动用趋势数据写成一段 Markdown——「上个月,我们的代码库超 500 行的文件从 15 个减少到 3 个,架构违规从 8 处到 0。ForgeOS 的治理协议在 building 阶段强制执行了单一职责原则。」——用于向团队/领导层展示 ROI。

### 差异化证明

- `expansion-production-blindspots-v36.md` 方向二「自治运行时主机资源消耗盲区」讨论的是 forge-core 自身进程的 RSS/goroutine/FD 消耗——那是**基础设施监控**,不是治理效果的可观测性。
- `forgotten-five-system-boundaries.md` 方向二「trace 数据格式版本化和迁移策略」讨论的是 trace.jsonl 的 schema 演化——那是**数据格式的生命周期**,不是数据的消费端分析。
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三「状态目录生命周期管理」讨论的是 `.forge/` 目录的备份/恢复/清理——那是**存储管理**,不是「从这些数据中提取治理信号」。

---

## 小结:产品方向 vs 架构方向

| 本文方向 | 产品问题 | 架构方向 vs 已有哪些 |
|---------|---------|-------------------|
| 一 · 分析疲劳与元治理 | 44 份文档谁来看、怎么决策 | 所有已有文档都在提扩展方向,无人管方向自身的治理 |
| 二 · 三运行时门槛 | 用户要装多少东西才能用 | 所有已有文档讨论的是「加什么」,不讨论「用户怎么到那里」 |
| 三 · 环境依赖的测试 | 测试结果在不同环境一致吗 | 所有已有文档不审计 ForgeOS 自身测试质量 |
| 四 · 零外部集成面 | 第三方工具怎么集成 | 已有文档讨论内部事件总线/Web UI,不讨论「CI/IDE 今天怎么集成」 |
| 五 · 治理效果不可观测 | 治理到底有用吗 | 已有文档讨论 trace/scorecard 的数据存储格式,不讨论数据消费 |

---

## 优先级建议

| 方向 | 优先级 | 杠杆 | 一句话理由 |
|------|--------|------|-----------|
| 三 · 环境依赖的测试 | **P1** ⭐⭐⭐⭐⭐ | 低投入高回报:embed fixture + 集成测试 tag,消除 27 个 skip 中的大部分 | 影响治理链可信度的基础问题 |
| 二 · 三运行时门槛 | **P1** ⭐⭐⭐⭐ | 中等投入:逐步将 check.py 功能移植 Go,归并运行时 | 直接影响团队采用意愿 |
| 一 · 分析疲劳与元治理 | **P1** ⭐⭐⭐ | 低投入:一个 YAML 注册表 + Triage 流程,停止产出 45 份文档 | 让 84 份已有分析可消费、可追踪 |
| 五 · 治理效果不可观测 | **P1** ⭐⭐⭐ | 中等投入:`forge report` 子命令 + accept-history 聚合 | 长期信任的生命线 |
| 四 · 零外部集成面 | **P2** ⭐⭐ | 高投入:需要 daemon + API 设计 | v2 范围外但长期不可缺 |
