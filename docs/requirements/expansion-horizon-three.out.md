现在我已完成了全面交叉验证。以下是我的结构化反馈:

---

## 验证结果

### 确认准确(证据站得住)

| 方向 | 核心主张 | 验证结果 |
|------|----------|----------|
| **方向一** | `next_stage` 声明存在,运行时**零消费做自动管线切换** | ✅ 精确。`main.go:433-436` 仅在 human_gate 报告中**打印** `next_stage`,没有"加载下一个工作流"的路径 |
| **方向二** | 跨仓库概念在代码/架构中完全为零 | ✅ 精确。搜索 `federation/multi-repo/cross.repo/workspace` 仅在 `.agent/skills/cognitive-architecture.md` 出现一次"federation"(作为业务域),与多仓库治理无关 |
| **方向三** | 无 webhook/server/listener,不 import `net/http` | ✅ 精确。唯一的"事件"是 SIGINT/SIGTERM 优雅关闭 |
| **方向五** | Supersedes 字段零消费,拒绝信号不进入 memory | ✅ 精确。拒绝机制(`.forge/<stage>.rejected` 文件)存在但**仅驱动 loop-back**,从不写 memory。`gates.go` 对 `memory` 包的调用量为零 |

### 需要修正(一个关键错误 + 两个可深挖的点)

#### ❌ 方向四:forge-upgrade.mjs 已存在——但文档声称"零实现、零机制"

这是文档中最显著的硬伤。**`harness/scaffold/forge-upgrade.mjs`**(1015 行)是一个完整的升级工具:

```
node harness/scaffold/forge-upgrade.mjs --from <forgeos-repo> --target <project> [--apply]
```

功能完整度:
- ✅ 漂移分类(added/changed/unchanged)——`classifyDrift()`
- ✅ DRY 模式(默认只报告,不写入)
- ✅ `--apply` 模式 + 自动备份(`.forge/upgrade-backup/<timestamp>/`)
- ✅ 幂等性(二次 --apply 不重复写)
- ✅ 共享真相源——`GOVERNANCE_DIRS + COPIED_FILES` 同 `forge-init` 统一导入
- ✅ 红线保障——不碰 identity 文件
- ✅ scope 诚实声明——不声称能升级 forge-core 二进制
- ✅ 单元测试(`test_forge-upgrade.mjs`)
- ✅ `node:test` 零外部依赖(符合工程红线)

**文档的辩护空间**:forge-upgrade.mjs 确实**不完整**:
1. ❌ 未集成进 forge CLI——无 `forge upgrade` 子命令,需 `node harness/scaffold/forge-upgrade.mjs`
2. ❌ 无版本追踪——`.agent/project.yml` 确实无 `forge_version` 字段(已确认)
3. ❌ 无合并策略——只有覆盖(overwrite),无 merge/skip/three-way
4. ❌ 无批量操作——一次一个项目
5. ❌ 无 `--prune` 实际删除——仅显示,不执行
6. ❌ 无文档——`.agent/*.md` 均未提及

**建议修正**:把"零实现"改为"存在原型但未集成、无版本追踪、无合并策略——距生产级治理升级管线差 6 项关键能力"。

#### ⚠️ 方向一:证据可再精确一层

文档说 `next_stage` "零消费"。更精确地说:

- **消费存在,但仅用于报告**:`main.go:395-436` 解析 `next_stage` 并在 human_gate 报告中打印 `"approved → unlocks <next_stage>"`。这是一个**告知性**消费,不是**操作性**消费。
- **真正的缺口**:没有任何代码读 `next_stage` 后调用 `forge run <next_workflow>` 或等价操作。缺口不在解析,在**执行链的下一步**。

#### ⚠️ 方向五:loom 的"已实现拒绝"可被误读为证伪

当前拒绝机制(`.forge/<stage>.rejected` marker → `rejectionPhaseIndex()` → loop-back)是一个**单会话、单工作流**的纠正。文档准确指出它不进 memory。但读者可能会说"拒绝已经实现了,你说错了"。建议加上明确区分:

> 拒绝已实现(单会话 loop-back),但**跨会话修正学习**需要:① 拒绝信号写入 memory ② Supersedes 字段被填充 ③ 路由系统读取修正信号做负反馈。这三步全缺。

---

### 边界情况的评估

我逐一核对了五个方向的边界情况表,发现一个遗漏:

**方向一(管线组合)遗漏的关键边界**:
- **部分批准(partial approval)**:设计阶段有 3 个方案,human 批准了 A 但 reject B/C。管线进入 build 时,应该只 build A 还是全部?无此语义。
- **管线暂停/恢复**:`forge pipeline pause` + `forge pipeline resume`——如果 pipeline 运行 6 小时,中间可能需要暂停。

**方向二(多仓库联邦)遗漏的关键边界**:
- **循环依赖检测**:如果 frontend/ 依赖 backend/ 的 API,同时 backend/ 依赖 frontend/ 的类型定义——跨仓库循环依赖的检测需要在联邦层做,当前代码无此概念。
- **部分联邦(partial federation)**:有些仓库想加入联邦治理,有些想保持独立——这个"选择加入"的机制没有考虑。

**方向三(事件驱动)遗漏的关键边界**:
- **事件过滤(webhook 风暴)**:GitHub 可能同时推送 10 个 PR——每个都触发 `forge run`?需要 dedup + 合并策略(例如同分支的多个 commit 合并为一个触发)。
- **事件源抽象**:不只是 GitHub,还有 GitLab/Gitea/Docker Hub/Slack command——事件源的可插拔性没有讨论。

---

### 优先级建议的微调

文档推荐: **H1:管线 → H2:资产升级 → H3:修正学习 → H4:事件驱动 → H5:多仓库**

基于代码库实际状态,我的调整建议:

| 方向 | 当前实现成熟度 | 推荐优先级 | 理由 |
|------|---------------|-----------|------|
| **方向四**(资产升级) | 🔶 原型已存在(forge-upgrade.mjs),缺集成+版本+合并 | **H1** ↑ | ROI 最高——代码已写 70%,接 CLI + 版本追踪即可交付价值 |
| **方向一**(管线组合) | 🔶 声明就绪,运行时缺 Pipeline 引擎 | **H2** | 代码零但资产 schema 已定义,工作量中等 |
| **方向五**(修正学习) | 🔴 拒绝机制有但不进 memory | **H3** | 涉及 memory 层改动和路由负反馈架构设计 |
| **方向三**(事件驱动) | 🔴 完全缺失 | **H4** | 对外依赖重,安全要求高 |
| **方向二**(多仓库联邦) | 🔴 完全缺失 | **H5** | 规模最大,需前四个方向打基础 |

核心逻辑:forge-upgrade.mjs 已存在,把它补完(CLI 集成 + 版本追踪)的成本远低于从零开始建 Pipeline 引擎。

---

### 三类高价值补充

#### 1. `forgeVersion` 应写入 `project.yml`

```
# 当前: 无版本字段
# 需要的:
forge_version: v2.5.0
forge_upgraded_at: 2026-07-09T12:00:00Z
forge_upgraded_from: a3b2c1d
```

`forge-core/cmd/forge/main.go:26` 已有 `forgeVersion` 构建时注入——init/upgrade 时把它 stamp 进 project.yml 即可。这是 forge-upgrade.mjs 能否从"原型"进化到"生产级"的**最小关键路径**。

#### 2. 方向三可以和方向一联动

事件驱动触发不应止于触发单个工作流。最自然的入口:PR 合并 → 自动触发 `discover → design → build → review → evolve` **管线**(方向一)。没有方向一,方向三的价值被削弱 50%——因为事件触发的最终目标是跑一个完整的自治循环,而不是跑单个 `forge run build`。

#### 3. 方向五的"最小可行实现"路径

当前最缺的不是 Supersedes 写入(代码路径清晰但无人调用),而是:
- `forge reject` CLI 命令(当前只有 `forge approve` + 手工写 `.rejected` 文件)
- 拒绝时的 `reason` 文本捕获——当前 reject marker 是空文件,不携带原因
- 将 reason + context 写进 memory 作为 `Supersedes` + `Confidence < 0.3` 的决策条目

三条中,CLI 命令是门槛:有 `forge approve` 就有 `forge reject` 的对称预期。

---

### 整体评估

这是一篇**质量极高**的 H3 视野分析。五个方向选择精准,对应的是 ForgeOS 从"单项目自治"到"多项目生态"的必然结构性缺口。证据引用方法(代码搜索→边界情况→与已有分析的区别)规范且可复现。

唯一需要修正的是方向四的"零实现"论断——`forge-upgrade.mjs` 的存在是一个需要纳入分析的重要事实。但有趣的是,它反而**强化**了方向四的论点:这个工具已经存在却无人知晓、无文档、无 CLI、无版本追踪——这恰恰说明了"治理资产生命周期管理"这个系统性问题有多么根深蒂固。
