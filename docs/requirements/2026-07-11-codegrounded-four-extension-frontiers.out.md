分析文档的四个方向我已经全部以当前代码库验证完毕。先说结论：

---

## 验证结果：四项方向真实有效，且未被当前 Sprint 覆盖

### 方向一：工作区隔离 ✅ 真缺口
你引用的代码证据准确。`forgeDir(root)` → `.forge/checkpoint.json`、`memory.jsonl`、`trace.jsonl` 全部使用单一 `root` 路径，无 workspace/branch UUID 分区。所有 I/O path 都是 `filepath.Join(root, ".forge/...")`，同一 project root 下多会话必然冲突。

值得注意的已有进展：`persist.Save` 已支持 `retain>0` 的历史轮转（`checkpoint.json.1/.2/...`），但这解决的是时序备份，不是并发分区。

### 方向二：跨进程锁 ✅ 真缺口
`persist.Save` 的 `temp+rename` 是单进程原子（同一内核可见），但跨进程竞态完全未处理。`memory.Append` 用 `O_APPEND` 保证单行原子性但两行顺序未定。`scorecard` 的 read-modify-write 是经典的 last-writer-wins。**这三个场景都是跨进程竞态，单进程 mutex 防不住。**

建议的新 `internal/lock` 包设计合理——`flock(2)` 优雅降级到无锁的 fallback 策略契合本仓既有的 honesty 惯例（见 Sprint 19 的 SCA 适配器模式）。

### 方向三：阶段契约校验 ⚠️ 部分覆盖，系统性方案缺失
**已做的部分**（文档未提及，值得补充）：
- Sprint 28-29 完成的 VERDICT 机读契约链（reviewer 二元 + CTO 五择一 + confidence 评分），走通 `parseXxx` → `observeFor` → `verdictLedger` → `converge.Signals` 全链路
- Sprint 31 的 `confidence_metric:` 字段驱动（从 agent 输出提取数值→ `Signals.RequirementConfidence`）
- Sprint 30 的 `secondary_template` 消费

**真缺口**：`asset.Phase` 确实没有 `OutputContract`/`InputContract` 字段。当前 feeds_forward 的数据流是「信任传递」——`phaseOutputLedger` 原样记录→`buildPrompt` 原样注入，无格式校验。当 Planner 输出结构漂移时，Implementer 静默消费错误格式，直到 gate FAIL 才发现。

你的轻量方案（agent 卡声明 `output_contract` + `observeFor` 时校验 + 不通过则低置信度标记）与现有 VERDICT 模式一脉相承，是合理的渐进式增强。

### 方向四：非 LLM 执行器 ✅ 真缺口
代码证据完全准确：
- `RunFrom` 循环只有两条分支：`runGates`（gate phase）和 `runAgentPhaseBudgeted`（LLM agent phase）
- `AgentExecutor` 接口是唯一的执行器抽象
- `SandboxConfig` 声明但永不消费（`command_executor.go:108-113`）

缺口不仅是「没有 plugin executor」，更深层的是 `asset.Phase` 没有 routing field——当前所有 phase 的 `Agent` 字段指向 LLM agent 卡，没有字段声明「这个 phase 跑 SQL 脚本/部署命令/通知」。

---

## 和本仓已有分析的关系

我查对了 CURRENT_SPRINT 的 31 个 Sprint + 下一前沿清单 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`，结论是：

| 方向 | CURRENT_SPRINT 覆盖 | FUNCTIONAL_REQUIREMENTS 覆盖 | 新颖性 |
|------|-----|-----|--------|
| 一：工作区隔离 | 未出现 | 未出现 | **高** |
| 二：跨进程锁 | 未出现 | 未出现 | **高** |
| 三：阶段契约 | 部分（VERDICT 链已闭环） | 未出现 | 中（系统性方案新） |
| 四：非 LLM 执行器 | 未出现 | 未出现 | **高** |

四项中有三项是真·未触及的新方向，方向三的系统级方案也是已有模式的有效推广而非重复。

---

## 想问你：倾向怎么推进？

`.out.md` 已生成但标注为「尚未生成过响应」——我是第一个读这份文档后做出响应的 agent。

几个可选路径（你可以选一个，或者有其他想法）：

1. **交叉验证 novelty** — 和 `docs/requirements/` 下 120+ 份已有分析做命中/未命中比对，确保这四项确实没有被其他分析覆盖过（方向一/二/四看起来新颖性很高，但数量太多需要系统确认）

2. **转化为 Sprint tasks** — 按本仓 Sprint 惯例，将方向一+二（工作区隔离 + 跨进程锁）拆解为具体的架构决策 + 实现清单 + 测试策略，定位到下个 Sprint

3. **深度验证代码证据** — 逐文件读取你引用的每处代码（`orchestrator.go` `RunFrom`、`command_executor.go` `SandboxConfig`、`scorecard.go` 的 read-modify-write 等），确认引用的精确性和上下文完整性，产出一份更严谨的验证报告

4. **直接原型方向一**（工作区隔离）— 这是最高优先级的快速 win：`internal/workspace` 包 + `forgeDir` 路径模式改造 + 调用点适配，~15 文件改动模式明确

你有偏好？
