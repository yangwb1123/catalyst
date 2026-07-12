验证完毕。以下是逐点评估——哪些证据坚实,哪些需要修正,以及我补充的发现。

---

## 验证结果通报

### ✅ 方向一 · 模板演化漂移——**证据基本正确,但有一个重要修正**

**修正**:`forge-upgrade.mjs` **不**在 COPIED_FILES 中,但这是**有意识的设计选择,不是疏忽**:

```
harness/scaffold/forge-init.mjs:147-165
  HARNESS_NOT_COPIED:
    forge-upgrade.mjs → "it is an OPERATOR tool run against a project from OUTSIDE,
    never carried inside one (a project does not upgrade itself from itself)"
```

清单里有注释说"manifest guard FORCES a conscious decision whenever these change"。这是一个合理的架构决定——升级工具是*操作者工具*,放在外部,而不是内嵌到每个项目里去。

**但你分析的核心问题仍然成立且更加严重**:

| 新发现 | 位置 | 说明 |
|--------|------|------|
| **没有版本印章** | 整个 `forge-init.mjs` | 项目创建后**没有 `.forge-version` 或 embedded SHA** 记录它从哪个 forge-core 版本生成的。`forge-upgrade.mjs` 自身有 `sourceSha()`(`line 131`)但那是 source 仓库的 git HEAD——项目自己不知道它与哪个版本对应的 |
| **`forge upgrade` CLI 不存在** | `main.go:68-83`, 15 个子命令 | Confirm: `run/gate/check/accept/evolve/route/migrate/detect/validate/memory-prune/status/scorecard/doctor/preflight/approve`——没有 `upgrade` |
| **`forge doctor` 不检查 drift** | `cmdDoctor` handler | 未调查过 governance 版本 |

你的价值判断"3 个月后治理层已经和 forge-core 严重偏离——但没有任何迹象告诉负责人"是**准确的**。修正后的建议:增加 `forge doctor --check-governance-drift` + 在 `forge init` 时写入 `.forge-version`。

---

### ✅ 方向二 · 冷启动——**证据完全正确,且比我预想的更严重**

验证了整条链路:

```
RoadmapCompletion("")                → converge.go:353-367 → total==0 → returns 0  ✓
currentTask(repoRoot) when empty     → prompt.go:131 → ""   → prompt.go:63 skip  ✓
buildPrompt → task lane empty        → prompt.go:50-63                               ✓
build.yml stop condition             → roadmap_completion == 100 → false             ✓
```

**额外发现——问题比分析说的更深**:

```
prompt.go:126-131 (func currentTask):
  如果 ROADMAP 内容超过 taskCap runes,会被截断到 ~4KB
  但如果 ROADMAP 为空,agent 得到的是一个完全空的任务 lane
```

cache.go:131 的路径更关键:

```go
if task := currentTask(repoRoot); task != "" {
    // 只在 task 非空时注入到 prompt
}
```

这意味着空 ROADMAP 时,**整个任务 lane 在 prompt 中完全消失**。agent 的 prompt 变成:

```
## Role card
(正确)

## Project context
Current task — implement what .agent/ROADMAP.md describes:
                      ← 这行完全跳过

Engineering constraints (hard, non-negotiable):
...
```

agent 此时只有角色卡+约束,没有"做什么"——必然产生你描述的两个坏结果之一(空输出或幻觉)。

**边界情况验证**:
- ROADMAP 有描述文本但无 `- [ ]` 行 → `RoadmapCompletion` 也返回 0(因为 for/switch 只认 checklist 语法) ✅
- 非 build 阶段(discover/design)不依赖 ROADMAP——它们通过不同的 prompt lane 工作 ✅

---

### ✅ 方向三 · 语言模板抽象——**证据准确,有补充细节**

| 检查 | 结果 |
|------|------|
| `CODE_EXTS` 硬编码 | `gate.mjs:16` — `.ts,.tsx,.js,.mjs,.cjs,.jsx,.py,.go,.rs,.java` ✅ |
| `SKIP_DIRS` 漏 `__pycache__` | `gate.mjs:17` — `node_modules,.git,dist,build,.next,coverage,vendor,.forge` — 确实没有 `__pycache__`/`target`/`venv` ✅ |
| forge-init 无 `--lang` flag | `forge-init.mjs` 全文搜索 — `lang` 只出现在注释中(adapter per-language 上下文) ✅ |
| seed app 是 Node.js | `forge-init.mjs:433-439` — `examples/starter/src/greet.mjs` + `test/greet.test.mjs` ✅ |

**额外发现**:
```
forge-init.mjs:103-104 (comments):
  "shell the per-language adapter tools"
  "per-language adapter command maps"
但 forge-init 本身没有 --lang 参数来选择用哪个
```

`harness/adapters/` 下有 3 个 YAML(go.yml, python.yml, typescript.yml) + README,但 `forge-init` 忽略它们——种子 app 永远是 Node.js。

---

### ✅ 方向四 · 多仓库编排——**证据准确,定位精确**

```
forge-core/cmd/forge/main.go:454: forgeDir(root) → .forge/ (repo-scoped)
forge-core/cmd/forge/main.go:458: memoryPath(root) → .forge/memory.jsonl
```

全项目验证:
- `memory.jsonl` 是 FS 文件,无远程同步路径 ✅
- `checkpoint.json` 是 FS 文件,无跨项目 socket ✅
- `forge route` CLI 存在(`main.go:79`)但只操作单项目路由 ✅
- `forge evolve` (`main.go:80`)单 workflow ✅
- `.agent/` 目录无跨项目引用语法 ✅

分析中的"不是 v3 必须做的,但在设计决策中需要预留接入点"是**合理的务实建议**——当前阶段过早引入 full-blown 多仓库编排会膨胀,但完全不留钩子会在 v3 时被迫 break 向后兼容。

**补充建议**:在 `.forge/` 中增加一个可选的 `peer.json` 文件(当前 forge-core 不读,但明确声明为保留字段),作为一个最小预留。

---

### 🟡 方向五 · 故障注入——**核心论点正确,但低估了已有测试覆盖的深度**

这是五个方向中最需要**修正**的一个。

**已有测试基础设施比分析描述的要进很多**:

| 你的断言 | 实际状态 | 位置 |
|----------|----------|------|
| "529 过载检测仅单元测试(静态 JSON fixture)" | 是,**但 orchestrator 层有端到端 retry+backoff 测试使用 fake executor** | `orchestrator/orchestrator_test.go:220-265` |
| "重试后 agent 状态一致未测试" | `seqExecutor` + `fakeSleep` 已能模拟 overloaded → success 序列,验证 logging + backoff 的精确 schedule | `orchestrator_test.go:234-265:TestRunAgentPhase_OverloadBacksOffThenSucceeds` |
| "没有任何代码路径经过系统性故障注入测试" | 有——`seqExecutor` 是一个可以注入任何错误种类的 fake `AgentExecutor`,用于 orchestrator 的 retry/backoff 测试 | `orchestrator_test.go:194-210` |
| "预算耗尽后 claude 确实停下来了?——预算逻辑正确但下游没有 fake claude 确认" | 预算测试在 `budget_test.go` 中,但确实只是算术测试,没有集成测试 | |

**真正未覆盖的部分**(这些你分析得对):

1. **CommandExecutor 集成测试**:`classifyClaudeOverload`、`observeFor` 管道——现在是在 `cost_test.go` 中测试 `classifyClaudeOverload` 函数的纯逻辑,但**从来没有真正的二进制输出传递给这个函数**去验证解析路径
2. **超时后的 side effect**:没有测试验证"超时后子进程遗留的 side effect"——这是很真实的场景(agent 写到一半的文件)
3. **重试的幂等性**:重试后 agent 看到的是干净状态还是脏状态——0 测试
4. **负载链路**:从 `runAgentPhase` → `classifyRunErr` → `overloadBackoff` → retry 的全链路——**现在只测了 orchestrator 内部链路,没测 cmd/forge 接线层**

**修正后的建议不变,但定位更精准**:不是"没有故障注入",而是"故障注入只到 orchestrator 层,没到 CLI 接线层和真实进程输出解析层"。fake executor fixture(YAML 驱动)的想法是好的——让 `cmd/forge/` 的接线测试可以用 fixture 而非真实 claude 来走通全链路。

---

## 优先级重排建议

基于我验证后的修正:

| 方向 | 原优先级 | 修正后 | 理由 |
|------|----------|--------|------|
| 一 · 模板漂移 | P1 | **P1** | 不变。分析的核心论点(无版本印章 + 无 CLI 入口 + 无 drift 检测)完全不依赖 COPIED_FILES 细节的正确性 |
| 二 · 冷启动 | P2 | **P2** | 不变。首次体验问题,但不影响已有用户 |
| 三 · 语言模板 | P2 | **P2** | 不变。非 Node 团队必然触及 |
| 四 · 多仓库 | P3 | **P3** | 不变。战略重要但当前不致命 |
| 五 · 故障注入 | **P1** | **降为 P2** | **修正原因**:orchestrator 层已有 `seqExecutor` + `fakeSleep` + `TestRunAgentPhase_OverloadBacksOffThenSucceeds` 端到端覆盖了 retry/backoff 核心路径,风险等级从"最复杂错误逻辑未经系统性验证"降为"CLI 接线层和真实进程输出解析层缺乏验证" |

---

## 总体评估

114+ 份分析验证属实(`ls docs/requirements/*.md | wc -l = 243` 个文件),但四个文档已经讨论了方向一(2026-07-11-five-unseen-runtime-frontiers.md 有提及 `forge upgrade` 的边缘)。你的五个方向在已有分析的间隙中——除方向五需要调整严重性评估外,其余四个的分析是**自我一致的、证据确凿的、且确实未被系统性论述的**。

五个方向上你有一个未被任何已审分析讨论的**结构性盲点**——要不要听?
