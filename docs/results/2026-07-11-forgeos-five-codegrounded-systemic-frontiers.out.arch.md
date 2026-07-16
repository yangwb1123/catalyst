# 架构师分析：ForgeOS 需求分析系统的元架构问题

## 1. 架构评估

### 1.1 当前架构的显式结构 vs 隐式行为

文件的反馈揭示了一个**元架构问题**——它不是关于 ForgeOS 运行时系统的架构，而是关于**知识生产系统本身**的架构。这个系统目前是隐式的、自发的、无收敛约束的。

| 维度 | 当前状态 | 架构评估 |
|------|---------|---------|
| **结构** | 227 个独立 `.md` 文件，扁平命名空间 | ❌ 无层次、无关联、无生命周期 |
| **生成协议** | LLM prompt → 单次生成 → 保存 | ❌ 开环，无反馈/收敛机制 |
| **唯一性约束** | 关键词 grep（语义浅层） | ❌ 无法检测重复方向 |
| **消费侧** | 无人消费 → 无实际产出 | ❌ 信息熵持续衰减 |
| **生命周期** | 无过期/归档/版本管理 | ❌ 文档永远 "存活" 但永远不成熟 |

### 1.2 关键设计缺陷

**缺陷 A：缺乏收敛算子（convergence operator）**

整个管线缺少一个函数式编程意义上的 `reduce`。生成是 `map`（N 篇文档），但没有 `reduce`（N → M，M << N）。这直接导致了 227 个文件的熵增。

**缺陷 B：新颖性检测为 O(n) 关键词搜索而非 O(1) 语义索引**

当前方案用 `grep` 做差异化声明验证。grep 是**语法级精确匹配**，而架构方向之间的重叠是**语义级近似匹配**。这是工具与问题的不匹配（square peg, round hole）。

**缺陷 C：无状态聚合层**

没有哪份文档是 "方向 X 的权威聚合"——所有 227 份都是平权的。当一个工程师想了解 ForgeOS 的架构空白时，他需要读 5.7MB 文本然后自己做聚类。这本身就是架构债务。

### 1.3 架构债务量化

| 债务类型 | 描述 | 严重等级 |
|---------|------|---------|
| 重复文档 | 方向一在 3+ 文件中以几乎相同措辞出现 | P0 |
| 虚假唯一性 | 差异化声明系统性地低估重复度 | P0 |
| 无消费契约 | 生成分析文档无对应 issue/ADR/PR | P1 |
| 无生命周期 | 无过期策略，无法区分 "活跃分析" 与 "历史痕迹" | P1 |
| 模板僵化 | "差异化声明表格"成为必须遵守的模板，无论是否适用 | P2 |

---

## 2. 扩展方向

以下五个方向是针对**当前系统（知识生产管线）** 的架构改进，而非 ForgeOS 运行时。如果需要后者，请告知。

### 方向 A：引入语义唯一性门禁（Semantic Uniqueness Gate）

**为什么需要**：当前 grep 门禁的召回率接近零。方向一能以完全相同的语义通过差异化声明，这是系统性的误报。

**核心挑战**：
- 嵌入相似度的阈值选择：太严则扼杀合理演进（迭代完善已有方向），太松则重复
- 需要区分 "同一方向的新版本"（允许）与 "同一方向的重复声明"（拒绝）

**架构变更**：
```
当前: prompt → writer → grep-check → save
改为: prompt → writer → embed → cosine-sim(embed, index) → threshold-decision → save/merge/reject
```

**对现有系统的影响**：
- 需要引入嵌入索引存储（已有文档的向量索引）
- 写入流程增加一个网关步骤
- 现有 227 篇文档需要一次性嵌入建立基线

**设计选项**：

| 选项 | 方案 | 权衡 |
|------|------|------|
| A1 | 生成时实时计算嵌入，与已有索引比较 | 延迟增加 200-500ms，但无需离线任务 |
| A2 | 定期批量重索引，门禁用缓存索引 | 索引可能滞后于最新写入 |
| A3 | 增量索引 + 实时查询 | 最复杂但准确度最高 |

**建议**：走 A1 起步，达到 90% 召回率后考虑 A3。嵌入模型用 `text-embedding-3-small` 或 `all-MiniLM-L6-v2`（本地可跑，无外部依赖）。

---

### 方向 B：文档聚合收敛管线（Convergence Pipeline）

**为什么需要**：227 个文件需要坍缩为 10-15 个真正独立的方向簇，每个簇有一份权威文档。

**核心挑战**：
- 聚类粒度控制：什么样的相似度阈值算 "同一簇"？（0.75？0.85？）
- 合并时的信息损失：簇内文档可能有互补的子场景，合并不能丢细节
- 需要人类确认：自动聚类可以建议，但最终合并需要架构师判断

**预期架构变更**：
```
             ┌─────────────┐
 227 文档 ──→│ 语义聚类器   │──→ 簇 1 (n=23)
             │             │──→ 簇 2 (n=15)
             │             │──→ 簇 3 (n=8)
             └─────────────┘       ...
                    │
                    ↓
             ┌─────────────┐
             │ 融合撰写器   │──→ vision.md (方向簇 1 的权威文档)
             │ (Human + LLM)│──→ resilience.md
             └─────────────┘──→ ...
                    │
                    ↓
             ┌─────────────┐
             │ 原始文档 →   │──→ docs/archive/v1/
             │ 归档         │
             └─────────────┘
```

**对现有系统的影响**：
- 一次性操作（对 227 篇文档做聚类 + 融合 + 归档）
- 之后增量维护，新文档先走方向 A 门禁再写入
- 需要新增 `docs/consolidated/` 目录作为权威源
- 需要新增生命周期字段：`status: draft | active | superseded | archived`

---

### 方向 C：方向文档的 ADR/Issue 双向链接契约

**为什么需要**：当前分析文档与实际行动之间的断开是最大的价值损失。没有链接意味着没有闭环。

**核心挑战**：
- 需要识别哪些分析方向已具备足够的成熟度进入工程化
- ADR 和 Issue 有不同的守卫条件（ADR 需要架构决策，Issue 需要可行性评估）
- 链接需要双向可追溯（ADR → 分析文档，分析文档 → ADR）

**架构变更**：
```
在每位分析文档的 frontmatter 中新增:
---
id: req-042
title: 门禁输出结构化
status: draft           # draft | ready-for-adr | adr-linked | implemented
cluster: computational-gate
adr: adr/014-gate-output-format.md
issues:
  - forgeos/gate#117
supersedes:
  - req-015
  - req-023
---

然后在 ADR 中反向引用:
---
title: ADR 014: Gate Output Format Standardization
related-requirements:
  - req-042
  - req-038
---
```

**对现有系统的影响**：
- 需要对 227 篇文档添加 frontmatter（可通过批处理脚本完成）
- 新增 CI 检查：文档设为 `ready-for-adr` 后 7 天内若无对应 ADR 则自动降级为 `stale`
- 新增 Dashboard：可视化各方向从 "分析" 到 "实施" 的管线状态

---

### 方向 D：生成时的上下文量约束（Context Budget Enforcement）

**为什么需要**：反馈中提到的 "每次 prompt 要求完全新颖" 是一个**约束鸿沟**——prompt 说 "从未被覆盖"，但系统没有给 LLM 提供足够的信息来验证这个约束。

**核心挑战**：
- 需要将已有文档的语义摘要注入 prompt，但不超上下文窗口
- 摘要粒度的选择：每篇文档一句话？一段话？嵌入向量列表？
- 需要区分 ForgeOS 核心代码上下文 vs 需求文档上下文（前者代码变化会过时后者不变）

**架构变更**：
```
当前: "请识别 5 个从未被覆盖的架构方向"
改为: "已有方向集合如下（嵌入摘要）：
      簇 A (23篇): 门禁性能与安全
      簇 B (15篇): 跨进程状态管理
      ...
      请识别与以上所有簇的余弦相似度 < 0.7 的新方向"
```

**对现有系统的影响**：
- 需要维护方向簇的嵌入摘要缓存量（每次更新后刷新）
- prompt template 结构变更 —— 不再是独立生成，而是基于上下文的增量生成
- 副作用：生成质量从 "广度" 转向 "深度"——因为 LLM 知道哪些已被覆盖，会更深入挖掘剩余空白

---

### 方向 E：分析文档的自动化衰退测试（Automated Staleness Detection）

**为什么需要**：许多分析文档引用了具体代码行号（如 `readFileSync('utf-8') at line 42`）。当代码演进后，这些引用会过时。目前无机制检测。

**核心挑战**：
- 需要解析文档中的代码引用模式（文件名 + 行号 + 代码片段）
- 需要持续对比引用 vs 实际代码
- 假阳性处理：行号偏移（插入/删除导致行号变化）不一定是内容变化
- 需要区分 "引用已修复" vs "引用已过时代码但问题依然存在"

**架构变更**：
```
新增工具: stale-ref-check.mjs

输入: 一篇分析文档
输出: 引用的代码位置状态 (valid | offset | stale)

CI 集成: 每天运行一次，标记过时文档。
超过 30 天 stale 的文档自动降级为 deprecated。
```

**对现有系统的影响**：
- 新增一个独立的扫描工具（借鉴 `secret-scan.mjs` 的架构模式）
- 文档 frontmatter 新增 `last-code-verify: 2026-07-12` 字段
- CI 新增一个检查步骤，不阻断 PR 但生成警告报告

---

## 3. 接口设计建议

### 3.1 文档生命周期接口

当前系统无显式生命周期。建议引入四个状态：

```
                          ┌──────────┐
    新生成 ──────────────→│  draft   │
                          └──────────┘
                              │
                     ┌────────┴────────┐
                     ↓                 ↓
                ┌──────────┐    ┌──────────┐
                │  active  │    │ superse- │ (被更新版本替代)
                │          │    │ ded      │
                └──────────┘    └──────────┘
                     │
                     ↓
                ┌──────────┐
                │ archi-   │ (已实现为代码/ADR)
                │ ved      │
                └──────────┘

    任何状态 → stale (30天无更新 / 代码引用过期)
     stale → archived (60天无恢复)
```

### 3.2 文档内容接口

当前每篇文档的模板是扁平的。建议引入结构化 frontmatter：

```yaml
---
id: req-042
type: architectural-frontier   # | resilience-gap | implementation-plan
status: draft
author: claude-code-agent
created: 2026-07-12T10:00:00Z
updated: 2026-07-12T10:00:00Z
cluster: computational-gate    # 语义簇归属
novelty-check:
  method: embedding-cosine     # 唯一性检测方法
  max-similarity: 0.31         # 与已有簇的最高相似度
  above-threshold: false       # 高于 0.70 则拒绝
code-refs:
  - file: packages/gate/src/main.mjs
    lines: [42, 55]
    hash: a3f2c9d              # 引用时的 git commit
    status: valid
---
```

### 3.3 聚合层接口

```typescript
// 伪接口定义——不是代码实现，只是契约结构

interface DocumentCluster {
  id: string;                    // cluster-001
  name: string;                  // "门禁计算与安全"
  documents: string[];           // [req-001, req-015, ...]
  centroid: number[];            // 嵌入向量的中心点
  consolidatedDoc?: string;      // docs/consolidated/computational-gate.md
  status: 'unconsolidated' | 'in-progress' | 'consolidated';
}

interface NoveltyCheckRequest {
  candidateDoc: string;          // 新文档路径
  embedding: number[];           // 预计算嵌入
}

interface NoveltyCheckResponse {
  isNovel: boolean;              // true = 不重复
  similarClusters: Array<{
    clusterId: string;
    similarity: number;
    name: string;
  }>;
  decision: 'accept' | 'merge-into' | 'reject';
  mergeTarget?: string;          // 当 decision=merge-into 时的目标簇 ID
}
```

### 3.4 保持向后兼容

所有变更必须保证：

1. **旧文档不被破坏**：不修改已有 227 篇文档的内容正文，只通过批处理脚本添加 frontmatter
2. **旧工具体系不中断**：`forge accept`（gate.mjs/arch-check.mjs）不依赖新接口
3. **读写分离**：新架构先只影响写入侧（新生成走门禁），不影响读取侧（`ls docs/requirements/` 依然可用）
4. **可选参与**：旧生成的 prompt 流程无需修改——新流程是并行路径，不是替代

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

| 需要的功能 | 推荐方案 | 理由 |
|-----------|---------|------|
| 文本嵌入 | `@xenova/transformers`（Node.js 本地推理） | 零外部依赖，符合 "harness 零外部依赖" 红线 |
| 聚类 | 简单的 K-means 或层次聚类（自实现，~200 行） | 数据量 227 篇，不需要 Spark 或分布式 |
| 向量相似度搜索 | 暴力搜索（线性扫描 227 个向量） | 数据量极小，不需要 Pinecone/Chroma |
| 索引持久化 | JSON 文件 + mmap | 符合已有工具链模式（`gate.mjs` 就是单文件） |
| 生命周期变化检测 | `chokidar`（文件系统 watch）或 定时 CI cron | 推荐 CI cron + diff 检查 |

**核心决策**：**不需要引入任何外部运行时依赖**。

`@xenova/transformers` 是唯一的 NPM 包依赖，但它在 Node.js 内直接跑 ONNX，不需要 Python，不需要 GPU，不需要外部 API 调用。这完全符合 ForgeOS 当前 "Node.js 自包含工具链" 的架构模式。

### 4.2 自建 vs 采购的决策依据

| 功能 | 自建 | 采购 | 决策 | 理由 |
|------|------|------|------|------|
| 语义嵌入 | ONNX 本地模型 | OpenAI API | **自建** | 数据不出本地，API 费用随使用增长，而 227 篇文档太小不值得走 API |
| 聚类 | K-means 自实现 | 调用 sklearn | **自建** | Node.js 环境，不需要 Python 依赖。K-means 在 227 个 384维向量上 < 100ms |
| 文档存储 | 已有文件系统 | 数据库 | **保持文件** | 文档是 Markdown，文件系统是最自然的存储。加 index.json 做元数据索引即可 |
| 搜索 | grep + 嵌入 | Elasticsearch | **嵌入搜索为主** | 对 227 篇文档搭建 ES 是过度工程 |

### 4.3 具体实现建议

```javascript
// 嵌入工具：~50 行，基于 @xenova/transformers
import { pipeline } from '@xenova/transformers';

const embed = await pipeline('feature-extraction', 'Xenova/all-MiniLM-L6-v2');

export async function getEmbedding(text) {
  const result = await embed(text, { pooling: 'mean', normalize: true });
  return Array.from(result.data);
}
```

```javascript
// 相似度搜索：~30 行
export function cosineSimilarity(a, b) {
  const dot = a.reduce((s, v, i) => s + v * b[i], 0);
  const normA = Math.sqrt(a.reduce((s, v) => s + v * v, 0));
  const normB = Math.sqrt(b.reduce((s, v) => s + v * v, 0));
  return dot / (normA * normB);
}
```

（这不是生产代码，只是架构示意图——展示技术复杂度是可接受的。）

---

## 5. 实施路线图

### P0（本周）——阻止熵增继续

| 任务 | 描述 | 产出 |
|------|------|------|
| P0.1 | 嵌入所有 227 篇文档，建索引 | `docs/requirements/.index.json`，包含每篇文档的嵌入向量和元数据 |
| P0.2 | 实现语义相似度门禁 | `harness/novelty-gate.mjs`，在文档保存前检查余弦相似度 |
| P0.3 | 对所有 >0.75 相似度的文档簇做人工标注 | 确定哪些是真正的重复，哪些是互补 |
| P0.4 | 修改 prompt template 注入上下文 | 新生成分析时提供方向簇摘要，要求相似度 <0.7 |

**风险**：P0.3 需要人工参与（架构师或 Reviewer）。缓解方案：先跑自动聚类，然后一次评审会（1 小时）确认。

### P1（两周内）——萎缩已有文档集

| 任务 | 描述 | 产出 |
|------|------|------|
| P1.1 | 对 227 篇文档做自动聚类（K=15） | 将 227 篇映射到 15 个簇，每簇含相似度矩阵 |
| P1.2 | 对每簇写融合文档 | `docs/consolidated/cluster-{01..15}.md` |
| P1.3 | 将原始文档移入 `docs/requirements/archive/` | 扁平目录变为活性/归档分离 |
| P1.4 | 将融合文档链接到 GitHub issues | 每个簇至少一个 issue，标注优先级 |

**风险**：融合过程可能丢失子场景。缓解方案：融合文档保留指向原始文档的反向链接，并在附录中列出未合并的边界场景。

### P2（一个月内）——闭环与自动化

| 任务 | 描述 | 产出 |
|------|------|------|
| P2.1 | 实现文档生命周期自动化 | 30天无更新自动 stale |
| P2.2 | 实现代码引用过时检测 | `harness/stale-ref-check.mjs` |
| P2.3 | 新增文档 → ADR → Issue 链接的 CI 检查 | PR 检查：如果文档状态为 `ready-for-adr` 但无对应 ADR，发出警告 |
| P2.4 | Dashboard（可选） | 文档管线可视化 |

---

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| 自动聚类结果质量低（簇内方向混杂） | 中 | 高 | 聚类输出仅作为建议，人工 reviewer 确认后才归档 |
| 嵌入模型的 API 依赖导致离线不可用 | 低 | 中 | 选用 `@xenova/transformers` 本地模型，零网络调用 |
| 工程师抵制 "写分析文档变复杂了" | 中 | 中 | 门禁是透明的——用户只管写，门禁自动检查；只有在重复时才交互式提示 |
| 227 篇文档的嵌入成本（计算，非金钱） | 低 | 低 | 一次性的 227 次推理，`all-MiniLM-L6-v2` 在 CPU 上 < 30 秒 |

---

## 总结

我完全同意反馈的第二层判断：**这不是一个内容问题，这是一个元架构问题**。227 篇文档不是分析的成果，它是管线设计缺陷的可视化症状。

如果把 ForgeOS 的架构设计看作一个系统，那么知识生产管线是它的一部分。一个产生 228 次重复输出的设计决策系统，和 ForgeOS 本身 "门禁自防" 的方向四——**信息熵持续衰减**——是同一问题的不同实例化。

**建议**：在实施上述任何方向前，先做 P0.1-P0.4（阻止熵增继续），然后做一次一次性的收敛操作（P1），最后将收敛结果作为持续运作的基线（P2）。不做收敛直接启动新方向，会以 228+1 结束而非 15。
