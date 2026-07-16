# AI-SDLC 快速上手指南

## 三步走

```
① 选子系统 → ② 填 Context → ③ 喂给 AI Agent
```

就这三步。下面展开。

---

## ① 选子系统

你需要评审的对象，可以是：

| 类型 | 例子 |
|------|------|
| 一个新模块 | "Gateway 引擎"、"通知系统"、"审计日志" |
| 一个现存模块 | "Orchestrator 状态机"、"Context Engine 注入" |
| 一个跨模块改动 | "接入多厂商模型池"、"迁移到事件驱动" |
| 一个非技术决策 | "是否自研 Sandbox"、"是否引入 Kafka" |

**原则：一次只评审一个子系统。** 不要贪多。

---

## ② 填 Context

每个模板都有一个 `## CONTEXT` 块。你只需要替换 `{{...}}` 部分。

### 最小填写示例（Stage 0）

假设你要评审：**是否需要在 forge-core 中实现 Gateway 引擎**。

打开 `00-product-discovery.md`，找到：

```markdown
## CONTEXT

Project:              {{Project}}
Subsystem:            {{Subsystem}}
Current Sprint Goal:   {{Goal}}
Proposed Feature:     {{Feature Description}}
User Scenarios:       {{User Scenarios}}
Product Goals:        {{Product Goals}}
Relevant Documents:   {{RFCs / Specs / Customer Requests}}
```

替换为：

```markdown
## CONTEXT

Project:              ForgeOS — AI-native software engineering platform
Subsystem:            Gateway Engine（forge-core 新增模块）
Current Sprint Goal:   评估 Gateway 是否为当前阶段必做项
Proposed Feature:     在 forge-core 中实现统一 Gateway，承接所有外部模型调用
                      （Claude/OpenAI/Gemini），提供负载均衡、fallback、rate limiting
User Scenarios:       1. forge-core 需要调用多个模型厂商 API，当前硬编码 Claude
                      2. 某厂商 API 故障时需要自动 fallback 到其他厂商
                      3. 企业客户希望控制每个厂商的调用预算
Product Goals:        G3 自动模型调度（多厂商池）= v3 目标
Relevant Documents:   ROADMAP.md v3 条目、ADR-0003、ARCHITECTURE.md 引擎列表
```

就这么简单。关键是 **Project** 和 **Subsystem** 要准确，其他字段尽量给具体信息。

---

## ③ 喂给 AI Agent

### 方法一：直接粘贴（最常用）

1. 打开填好的模板文件
2. 全选复制
3. 粘贴到 Claude Code / Codex / Gemini CLI / ChatGPT 的对话框
4. 等待输出
5. 保存输出到 `.ai/reviews/{subsystem}/{stage}-output.md`

### 方法二：通过 system prompt（适合 CLI agent）

```bash
# Claude Code 示例
claude -p "$(cat .ai/prompts/00-product-discovery.md)"

# Codex 示例
codex --system-prompt "$(cat .ai/prompts/00-product-discovery.md)"

# Gemini CLI 示例
gemini --system-instruction "$(cat .ai/prompts/00-product-discovery.md)"
```

### 方法三：作为文件附件

大多数 Agent 支持文件输入，直接指定路径即可。

---

## 完整工作流（以 Gateway 为例）

```
Week 1 前半:
  Stage 0 → 填 Context → 粘贴到 Agent → 产出 Product Discovery Report
         → 结论: "Critical — 当前硬编码 Claude 限制多厂商支持"
         → 保存到 .ai/reviews/gateway/00-product-discovery.md

  Stage 1 → 把 Stage 0 输出填入 Stage 1 的 Context
         → 产出 ADR-0004: Gateway Architecture
         → 保存到 .ai/reviews/gateway/01-architecture-review.md

Week 1 后半:
  Stage 2 → 把 ADR 填入 Stage 2 的 Context
         → 产出 Security Findings（API key 管理、rate limit 绕过等）
         → 保存到 .ai/reviews/gateway/02-security-rfc-review.md

  Stage 3 → 产出 Distributed Review（fallback 一致性、circuit breaker 等）
         → 保存到 .ai/reviews/gateway/03-distributed-review.md

Week 2 前半:
  Stage 4 → 如果已有代码草稿，放入 Stage 4 Review
         → 产出 Implementation Review（命名、接口、错误处理等）
         → 保存到 .ai/reviews/gateway/04-implementation-review.md

  Stage 5 → 产出 Performance Review（延迟预算、连接池等）
         → 保存到 .ai/reviews/gateway/05-performance-review.md

Week 2 后半:
  Stage 6 → 产出 Production Readiness Checklist
         → 保存到 .ai/reviews/gateway/06-production-readiness.md

  Stage 7 → 产出 Sprint Backlog（可直接导入 Jira/Linear）
         → 保存到 .ai/sprint/gateway-backlog.md

  Stage 9 → 最终 CTO Decision（Approve / Simplify / Delay / Reject）
         → 保存到 .ai/reviews/gateway/09-cto-decision.md
```

---

## 不同场景的填写技巧

### 场景 A：评审一个已存在的模块

把代码直接贴进 `Relevant Code` 字段：

```markdown
Relevant Code:
  forge-core/internal/orchestrator/engine.go（核心状态机）
  forge-core/internal/orchestrator/build.go（Build 流程）
  forge-core/internal/orchestrator/evolve.go（Evolve 流程）
```

Agent 会自动读代码并评审。如果你用的是支持文件读取的 Agent（如 Claude Code），
直接给路径即可：

```markdown
Relevant Code:
  请阅读以下文件:
  - forge-core/internal/orchestrator/
  - forge-core/cmd/forge/
```

### 场景 B：评审一个还没开始做的新功能

不需要代码，重点写清楚：

```markdown
Proposed Feature:     实现 Agent Sandbox（Firecracker VM 隔离执行）
User Scenarios:       1. forge 生成的代码需要在隔离环境执行
                      2. 防止恶意代码影响宿主系统
                      3. 资源限制（CPU/Memory/Network）
Product Goals:        v3 目标：带外 Sandbox runner
Relevant Documents:   ADR-0003（agent-os 仓库提取）、ROADMAP v3
```

### 场景 C：跨项目复用模板

模板不绑定 ForgeOS。换一个项目，只需要改 Context：

```markdown
## CONTEXT

Project:              MyApp — 电商后台系统
Subsystem:            订单超时自动取消模块
Current Sprint Goal:   替代人工巡检，实现自动超时取消
Proposed Feature:     定时扫描未支付订单，超过 30 分钟自动取消并释放库存
User Scenarios:       1. 用户下单后不支付，库存被锁定
                      2. 运营需要手动巡检取消超时订单
                      3. 高峰期库存锁定影响其他用户购买
Product Goals:        降低库存锁定率，减少运营人力
Relevant Documents:   订单系统 API 文档、库存服务设计文档
```

### 场景 D：只做 L1 快速决策

不需要走完全部 Stage。很多时候你只需要：

```
Stage 0（30 分钟）→ 这个需求值不值得做？
Stage 1（1 小时）  → 架构怎么搞？
Stage 9（15 分钟） → Go / No-Go？
```

这就够了。**Stage 2-8 在确认要做之后再逐步推进。**

---

## 常见错误

| 错误 | 后果 | 修正 |
|------|------|------|
| Context 填太笼统 | Agent 产出泛泛而谈 | 给具体场景、数据、代码路径 |
| 一次评审多个子系统 | Agent 注意力分散 | 一次一个 |
| 跳过 Stage 0 直接做 Stage 1 | 可能做了一个不该做的东西 | 至少先过 Stage 0 |
| 不做 Stage 9 | 没有最终决策记录 | Stage 9 是强制的 |
| 产出物不保存 | 下次评审没有基线 | 每次输出都存到 `.ai/reviews/` |
| 让同一个 Agent 既实现又评审 | 自己审自己 | Stage 4 的 reviewer 必须是 fresh context |

---

## 产出物存档结构

```
.ai/
├── reviews/
│   ├── gateway/                      # 按子系统归档
│   │   ├── 00-product-discovery.md
│   │   ├── 01-architecture-review.md
│   │   ├── 02-security-rfc-review.md
│   │   ├── ...
│   │   └── 09-cto-decision.md
│   ├── notification/
│   │   └── ...
│   └── audit-log/
│       └── ...
├── adrs/
│   ├── 0004-gateway-architecture.md  # Stage 1 产出的 ADR
│   └── ...
└── sprint/
    ├── gateway-backlog.md            # Stage 7 产出的 Sprint Backlog
    └── ...
```

---

## 一句话总结

> **选一个子系统 → 填 Context 里的 `{{...}}` → 粘贴到任意 AI Agent → 保存产出物 → 下一个 Stage。**
>
> 不需要全部 10 个 Stage 都走。最小路径：**Stage 0 → Stage 1 → Stage 9 = 30 分钟快速决策。**
