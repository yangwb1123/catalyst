现在我已对整个代码库有了透彻的了解。下面给出完整的安全审查。

---

# 安全审查：ForgeOS `forge-core` 运行时与 `harness` 网关基础设施

**审查范围：** `forge-core/cmd/forge/`、`forge-core/internal/`（编排器、资产、yaml2json、网关、风险、路由）、`harness/`（secret-scan、gate、gate-resolve）

**审查日期：** 2026-07-12 | **版本：** 提交 `905b65e` — 代码实现阶段

---

## 摘要统计

| 类别 | 严重 | 高 | 中 | 低 | 信息 |
|---|---|---|---|---|---|
| **鉴别与授权** | 0 | 0 | 1 | 0 | 0 |
| **输入验证** | 0 | 2 | 2 | 1 | 0 |
| **密码学** | 0 | 0 | 0 | 0 | 1 |
| **会话管理** | 0 | 0 | 0 | 1 | 0 |
| **数据保护** | 0 | 2 | 0 | 0 | 0 |
| **威胁模型** | 1 | 2 | 1 | 1 | 0 |
| **合规性** | 0 | 0 | 0 | 1 | 0 |
| **总计** | **1** | **6** | **4** | **4** | **1** |

---

## 发现

### FINDING-01（严重）：人类门禁审批标记可被伪造 — 最高杠杆率的安全闸门绕过

| 字段 | 值 |
|---|---|
| 类别 | **鉴别与授权** |
| 严重性 | **严重** |
| 标题 | 人类门禁审批仅依赖文件存在性，无加密签名 |
| 位置 | `forge-core/cmd/forge/gates.go:125-131` (`humanApproved`) — `forge-core/cmd/forge/approve.go:13-18` (`approvalPath`) |
| 描述 | 最高杠杆率的安全闸门（human_gate。即 `design.yml`'s stop，负责启动构建管线的审批）完全依赖文件存在性：`os.Stat(<root>/.forge/<stage>.approved)` 返回 `err == nil` 即视为已审批。没有签名、没有 HMAC，没有加密随机数 — 任何拥有仓库文件系统写入权限的进程（CI worker、并行运行的同一用户下的其他代理、可写共享卷上的攻击者）只需 `touch .forge/design.approved` 即可绕过由审查委员驱动的设计审批。审批标记存放在 `git` 已忽略的 `.forge/` 目录中（`gates.go:121` 文档如此说明），因此 Git 操作也不会提供保护。 |
| 攻击场景 | 场景 A（恶意内部人员）：一个以 `u1` 身份运行且对仓库有写权限的 `forge evolve` 代理同时运行 `touch /home/u1/catalyst/.forge/design.approved`，导致 `forge run design --executor=command` 在未经产品经理或架构师实际审查的情况下继续推进到 build 阶段。场景 B（CI 横向移动）：攻击者获得了对共享 CI 工作区的写权限，在另一个项目的 `.forge/` 下放置了一个预先计算好的标记文件，诱使信赖 `humanApproved()` 返回 `true` 的 Forge 运行跳过设计审批。场景 C（拒绝服务）：攻击者写入一个精心选择时间戳的标记，导致批准在窗口期外被错误确认。 |
| 影响 | 完全绕过 ForgeOS 中权限最高的安全控制。架构审查（`solution-architect` 阶段）和产品经理信心门禁被跳过，降低了整体输出质量 — 更重要的是，一旦已批准的设计进入 build，仍存在的架构缺陷就更难在成本已发生后被发现。OWASP Agentic Top-10 2025-12：*A1-权限不当*，*A6-访问控制失效*。 |
| 建议 | **短期（高影响/低工作量）**：在标记文件中写入一个加密安全的随机数（32 字节，来自 `crypto/rand`），由 `forge approve` CLI 创建并由 `humanApproved()` 验证。这是**零依赖变更**（Go 标准库 `crypto/rand`）。**长期**：将 `human_gate` 提升为需要离线带外签名的持久化跨进程门禁（`policy.yml` 中的 `durable_wait: v2` 方向）。代码结构已预留了该接口（`gates.go:125-131`），但当前未使用。 |
| 工作量 | **M** — 1-2 天（标记的随机数+签名；CLI 变更；`forge approve create/review`；测试） |

---

### FINDING-02（高）：凭证通过环境变量泄露给子进程

| 字段 | 值 |
|---|---|
| 类别 | **数据保护** |
| 严重性 | **高** |
| 标题 | `childEnv` 将整个父进程环境传播给所有子进程 |
| 位置 | `forge-core/internal/orchestrator/command_executor.go:191-199`（`childEnv` 函数） |
| 描述 | `childEnv` 创建一个新环境切片，其中包含 `os.Environ()` 中除 `FORGE_AGENT_DEPTH` 外的所有键值对。这意味着 Claude API 密钥（预计存储在 `ANTHROPIC_API_KEY` 或类似的环境变量中，供 `claude -p` 使用）、云提供商凭据（`AWS_ACCESS_KEY_ID`、`GITHUB_TOKEN` 等）以及 Forge 运行时进程自己的任何其他环境变量都会被传递给**每个子进程** — 包括 `node harness/gate.mjs`、`python3 harness/check.py`、`git diff` 以及赋能在 `--executor=command` 模式下运行的完整 Agent CLI。虽然这是类 Unix 系统的标准行为（`os/exec.Cmd` 在 `Env` 为 nil 时默认继承），但 `childEnv` 在**显式显式过滤**时未移除敏感键值。 |
| 攻击场景 | 场景 A（命令执行）：攻击者在 YAML 工作流中注入一个恶意 `agent_allowed_tools` 条目（例如 `"Bash(git*)"`，当前是不允许的），或者通过工作流 YAML 上游的某个输入诱导 Agent 运行 `env`，输出该 Agent 可访问的所有环境变量 — 包括 `ANTHROPIC_API_KEY`。场景 B（共享 CI）：在多租户 CI 上，一个阶段运行 `forge run --executor=command`；同一 worker 上并行运行的其他作业可访问同一 `ps` 表，该表包含带环境变量的进程。场景 C（日志泄漏）：Agent 输出被记录（`finish` -> `c.logf`），若 Agent 在标准错误中转储环境变量，密文便会进入日志。 |
| 影响 | 明文暴露 Claude/LLM API 密钥、云提供商凭据以及任何其他敏感环境变量。OWASP Top-10: A05:2021-安全配置错误；OWASP Agentic Top-10: A6-敏感信息披露。NIST SP 800-53: AC-6(9)、SC-8。 |
| 建议 | 在 `childEnv` 中添加去重过滤器，移除常见的凭据键：`ANTHROPIC_API_KEY`、`AWS_*`、`GITHUB_*`、`SLACK_*`、`TOKEN`、`SECRET`、`PASSWORD`、`FORGE_*`（保留 `FORGE_AGENT_DEPTH`）。使用前缀匹配（`strings.HasPrefix(kv, "ANTHROPIC_")` 等）以捕获变体。或者更好：将过滤逻辑移到外部，使其可配置（`AllowedEnv`/`BlockedEnv` 模式在 `Config` 中）。Agent CLI 通过 `--agent-allowed-tools` 令牌化访问 Bash 后，这种过滤变得尤为重要，因为现在 Agent 可以显式运行 `env`。 |
| 工作量 | **S** — < 1 天（标准库字符串操作的三行代码；编写针对 `childEnv` 的测试） |

---

### FINDING-03（高）：Python YAML 回退 shim 允许任意 Python 执行

| 字段 | 值 |
|---|---|
| 类别 | **输入验证** |
| 严重性 | **高** |
| 标题 | `loadWorkflow` 回退到通过命令调用执行外部 Python |
| 位置 | `forge-core/cmd/forge/main.go:168-171`（`exec.Command("python3", shim, ymlPath)`） |
| 描述 | 当 Go 原生解析器（`yaml2json.Decode`）失败时，`loadWorkflow` 会回退到 `exec.Command("python3", shim, ymlPath)`，其中 `ymlPath` 是**攻击者控制的用户输入的拼接结果**（用户提供的 workflow `name` + 固定路径模板）。虽然路径被限制在 `.agent/workflows/<name>.yml`，但 `shim` 路径指向仓库根目录下的 `harness/yaml2json.py` — 如果攻击者可以修改该文件（例如通过 PR 合并），那么每次 Go 原生解析器在**任何**工作流上失败时，`loadWorkflow` 都会执行攻击者的代码。此外，`name` 参数仅通过一个简单的 `args[0][0] == '-'` 检查进行验证（`splitPositional` 函数） — 未执行路径遍历过滤。 |
| 攻击场景 | 场景 A（shim 替换）：攻击者提交一个修改 `harness/yaml2json.py` 的 PR。该 PR 通过审查（因为 yaml2json.py 很少被审查，通常被认为是一个过时的 shim），然后每个后续的 `forge run/evolve` 在 Go 解析器遇到任何解析错误时都会执行攻击者的有效载荷。场景 B（路径遍历）：`name` 包含 `../../tmp/evil`；虽然 `.yml` 后缀会附加，但 `os.Stat` 处的错误消息 (`workflow not found: <expanded>)` 会泄露目录布局。 |
| 影响 | 在主机上执行任意 Python 代码，以运行 Forge 的用户权限。OWASP Top-10: A03:2021-注入。OWASP Agentic Top-10: A6-敏感信息披露，A1-权限不当。 |
| 建议 | **移除 Python 回退**。Go 原生 `yaml2json` 解析器（`internal/yaml2json`）已经成熟（支持映射、序列、块标量、内联、注释），并具有 100% 的 Go 测试覆盖率。回退到 Python 会引入一个不需要的攻击面，而不会带来额外价值。如果保留回退的目的是为了在过渡期间覆盖边缘情况，那么请将其更改为对 `yaml2json.py` 使用静态 `embed.FS` 路径，而不是允许通过仓库路径进行操纵，并添加严格的输入验证。 |
| 工作量 | **S** — < 1 天（移除回退；将 `exec.Command("python3"...)` 行替换为向用户报告原生解析器错误的错误返回） |

---

### FINDING-04（高）：`computeCodeTestRatio` 和 `computeFileDelta` 中的 Git 命令注入

| 字段 | 值 |
|---|---|
| 类别 | **输入验证** |
| 严重性 | **高** |
| 标题 | 用户控制的仓库根路径传递给 `git -C` 无 sanitize |
| 位置 | `forge-core/cmd/forge/gates.go:175`（`computeCodeTestRatio`）、`gates.go:213`（`computeFileDelta`、`gitChangedPaths`）；`route.go:206`（`gitChangedPaths`） |
| 描述 | `computeCodeTestRatio`、`computeFileDelta` 和 `gitChangedPaths` 都执行 `exec.Command("git", "-C", root, ...)`，其中 `root` 源自用户输入（`--root` 标志或 `$FORGE_REPO_ROOT` 环境变量，通过 `gate.RepoRoot` 解析）。`RepoRoot`（在 `gate.go:33-40` 中）仅检查非空字符串；它不验证 `root` 是目录路径、不包含 shell 元字符，也不以破折号开头（这将使 git 将其解释为另一个标志）。虽然 `exec.Command` 不调用 shell（因此类似 `; rm -rf /` 的注入不起作用），但如果 `root` 是一个 git 标志（例如 `--paginate` 或 `-c`），它将被 `git` 解释为标志而不是路径，从而改变 git 的行为。 |
| 攻击场景 | 场景 A（git 标志注入）：`root = "-c core.gitProxy=evil.com"` → git 将 `-c` 解释为一个标志 → `core.gitProxy` 被设置为攻击者的代理 → 后续 git 操作（拉取、克隆）可能被重定向。场景 B（拒绝服务）：`root = "--help"` → git 打印帮助并退出，导致管道损坏和空输出 → `computeCodeTestRatio` 默默返回 0，但 `computeFileDelta` 可能记录错误。场景 C（信息泄露）：`--paginate` 或 `-c core.pager=cat` 改变 git 输出，可能绕过日志过滤。 |
| 影响 | git 行为改变（中等严重性），潜在的信息泄露或拒绝服务。这是 ForgeOS 的内部组件，不是面向用户的 Web 服务，因此完全利用需要本地访问，但它为已经具有某些访问权限的攻击者增加了横向移动的载体。 |
| 建议 | 在 `RepoRoot` 中或刚通过它解析后验证 `root`：确认它是一个可访问的目录（`os.Stat(root)`），并且不是以 `-` 开头（将其视为路径而非标志）。在传递给 `git -C` 之前，使用 `filepath.Abs(root)` 将路径规范化为绝对路径。在所有三个使用点对 `exec.Command("git", "-C", safeRoot, ...)` 进行单元测试，路径类似 `-c`。 |
| 工作量 | **S** — < 1 天（向 `RepoRoot` 添加三行验证；更新所有调用点以使用已验证的根路径；编写单元测试） |

---

### FINDING-05（高）：`humanApproved` 中的竞争条件 — TOCTOU 窗口

| 字段 | 值 |
|---|---|
| 类别 | **威胁模型** |
| 严重性 | **高** |
| 标题 | 审批标记文件检查和消费之间的 TOCTOU（Time-of-Check Time-of-Use） |
| 位置 | `forge-core/cmd/forge/gates.go:125-131`（`humanApproved`）；`gates.go:189-204`（`resolveRejectionStartPhase`） |
| 描述 | `humanApproved` 执行 `os.Stat(approvalPath)` 并假设在整个运行期间结果保持稳定。`resolveRejectionStartPhase` 检查拒绝标记 (`os.Stat`)，然后在假设没有并发修改的情况下将其删除 (`os.Remove`)。如果在检查和消费之间，另一个进程写入/删除该标记，则可能出现：竞态条件 — 两个并发的 `forge run` 实例可能都读取到已审批的标记，或者标记被消费后无人拾取。虽然 v1 串行编排器防止了来自单个编排器的并发运行，但**外部操作**（CI 中的并行分支、用户同时运行 `forge approve`）可能引入竞态条件。 |
| 攻击场景 | 场景 A（重复审批）：两个并发的 `forge run design --approved` 实例在 `humanApproved` 的检查点都返回 `true` → 两个实例都进入构建阶段 → 重复执行相同的构建工作流，浪费预算。场景 B（否决）：攻击者在 `resolveRejectionStartPhase` 进行 `os.Stat` 和 `os.Remove` 之间放回一个审批标记 → 运行使用已消费的标记重新启动。场景 C（拒绝审批）：一个快速循环在 `humanApproved` 读取到被阻止后但在可以执行删除之前删除一个审批标记。 |
| 影响 | 非原子操作导致逻辑错误。HW 门禁在合并后可能被绕过，或者预算被重复的构建执行浪费。OWASP Top-10: A01:2021-Broken Access Control。 |
| 建议 | 使用原子重命名模式：不检查标记然后使用，而是使用带有 `O_EXCL` 标志的 `os.OpenFile` 来“消费”它（尝试在确认其存在后立即以独占模式打开它）。或者，在标记中存放一个一次性随机数，并在单个原子 `rename(2)` 操作中验证它。对于 `rejectionPath`，将 `os.Stat` + `os.Remove` 改为只进行一次原子重命名（重命名到已知的已消费位置），这样并发进程就会丢失竞态条件。 |
| 工作量 | **M** — 1-2 天（为标记消费实现原子文件操作模式；重构 `humanApproved` 以使用独占创建；在并行测试下验证竞态条件） |

---

### FINDING-06（高）：机密扫描器缺乏熵检测和对 Base64/混淆的处理

| 字段 | 值 |
|---|---|
| 类别 | **数据保护** |
| 严重性 | **高** |
| 标题 | 机密扫描器绕过：无熵启发式、无 base64/连接检测 |
| 位置 | `harness/secret-scan.mjs:1-215`（全部）；具体匹配器在 `PATTERNS`（第 56-71 行） |
| 描述 | 机密扫描器仅通过正则表达式匹配已知模式（AWS `AKIA...`、GitHub `ghp_...`、PEM 头部），并且仅有一条通用规则，它需要**同时**存在类似机密的键名**和**一个 >=20 字符的 base64 字符串值。这种设计刻意偏向于低误报率，但引入了严重的假阴性：任何 base64 编码的机密、连接在不同行上的字符串、ROT13 编码的凭据、键名不匹配 `api_key|secret|token|password` 的机密，或长度低于 20 个字符的机密，都将完全不被检测到。同文件第 19-23 行的文档明确承认了这些局限性，但合规性要求（SOC2、PCI-DSS）规定必须检测混淆的机密。 |
| 攻击场景 | 场景 A（base64）：ATTACKER 提交 `const x = "QUtJQUlPU0ZPR05ESkZP..."`（base64 编码的 AWS 密钥）。扫描器未匹配 → 机密已提交。场景 B（字符串连接）：`const key = "AKIA" + "IOSFOGNDJ" + "FOPRIVATE"` — 扫描器未匹配，因为单一部分不是有效模式。场景 C（短键名）：`const myKey = "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"` — 如果`myKey` 不匹配 `api_key|secret|token|password|passwd|pwd`，则通用规则不触发。 |
| 影响 | 机密在提交到仓库时未被检测到，可能被 CI 日志、Slack 集成等处泄露。OWASP Agentic Top-10: A6-敏感信息披露。NIST SP 800-53: AC-6(9), SC-8, SC-13。 |
| 建议 | 添加一个第二阶段的熵分析器（参考：Shannon 熵 > 4.5 比特/字节用于 base64 编码数据）。在通用规则中将最小长度从 20 降低到 16（典型 API 密钥长度的下限）。添加对检测跨多行连接（`"prefix" + "suffix"`）的简单 AST 模式的支持。作为 v0 的补充，而不是替换。 |
| 工作量 | **M** — 2-3 天（熵计算实现；更新通用正则表达式；添加连接检测；在已知的真实正例上验证以确保不增加误报率） |

---

### FINDING-07（中）：非 Unix 平台上进程组泄漏（Windows/no-pgid）

| 字段 | 值 |
|---|---|
| 类别 | **威胁模型** |
| 严重性 | **中** |
| 标题 | Windows 上的 `setupProcessGroup` 是无操作 — Agent 子进程超时后存活 |
| 位置 | `forge-core/internal/orchestrator/command_executor_other.go:18-21`（`setupProcessGroup` 无操作） |
| 描述 | 在非 Unix 平台（主要是 Windows）上，`setupProcessGroup` 是一个空函数。这意味着 `exec.CommandContext` 的默认 `Cancel` 函数（仅杀死直接子进程）保持生效，而 `WaitDelay` 保留默认值 0。当在 Windows 上以 `--executor=command` 运行 `claude -p` 时，Claude 可能会派生自己的工具子进程（bash、git、test runner），这些进程继承管道的写入端。当超时触发时，直接子进程（`claude`）被杀死，但孙进程继续运行。`cmd.Run()` 会阻塞，直到所有管道写入端都关闭，导致超时**永不返回** — 一个挂起的编排器。 |
| 攻击场景 | 攻击者在 Windows CI runner 上提交一个导致 Agent 派生一个长时间运行的孙进程的工作负载。编排器超时启动，但无法返回。整个 `forge run` 挂起，消耗预算和 CI 分钟数，直到外部看门狗终止它。进程泄漏也可能消耗系统资源（文件描述符、内存）。 |
| 影响 | 拒绝服务 — Windows 上挂起的编排器。进程泄漏累积，影响同一主机上的其他作业。OWASP Top-10: A04:2021-不安全设计（平台差异上的欺诈性简化假设）。 |
| 建议 | **短期**：添加一个平台检查，当在非 Unix 操作系统上使用 `--executor=command` 与 `--timeout` 时，记录一条可见的警告，说明超时保证在 Windows 上较弱（进程组清理缺失）。**中期**：在 Windows 上为作业对象实现 `setupProcessGroup`。Go 标准库不提供该功能，但可以通过 `golang.org/x/sys/windows` 包中的 `syscall.CreateJobObject` + `AssignProcessToJobObject` 实现 — 这构成了对外部包的单点依赖，其范围仅限于一个文件。或者，将 macOS/Linux/Windows 上的行为记录为“已知差异”。 |
| 工作量 | **M** — 2-3 天用于 Windows 作业对象实现 + 测试；**S**（< 1 天）仅用于警告 + 文档 |

---

### FINDING-08（中）：YAML 解析器接受任意数字、布尔值和 `null` YAML 标签，可能导致意外语义

| 字段 | 值 |
|---|---|
| 类别 | **输入验证** |
| 严重性 | **中** |
| 标题 | YAML 解析器在无模式上下文中执行类型强制转换 |
| 位置 | `forge-core/internal/yaml2json/scalar.go:1-75`（`parseScalar` → 将所有 YAML 类型强制转换为 JSON 类型而无架构验证） |
| 描述 | `yaml2json` 包中的 `parseScalar` 函数将 YAML 标量转换为 Go 值（string、float64、bool、nil），而不考虑预期的工作流架构。诸如 `yes`/`no`/`on`/`off` 之类的 YAML 真值被转换为 `true`/`false`；数字字符串被转换为 `float64`；`null`、`~`、`Null` 被转换为 `nil`。这意味着如果某人为 `description: no`（意图是字符串“no”）编写一个工作流 YAML，它将在 JSON 中变为 `false`，并且在 Go 结构体中 `Description` 字段被反序列化时变为空字符串（因为 `phase.Description` 是 `string` 类型，而 JSON `false` 在字符串字段中变为 `""`） — 描述被静默丢弃。更有问题的是，像 `model_tier: "null"` 这样的字符串被转换为 `nil` 而不是保留为字符串 `"null"`，这可能绕过故障安全默认值。 |
| 攻击场景 | 场景 A（配置绕过）：攻击者在工作流中设置 `model_tier: NO`（意图为 `"NO"` 字符串）。YAML 将其解析为布尔值 `false`，JSON 将其序列化为 `false`，Go 的 `json.Unmarshal` 将布尔值 `false` 转换为字符串 `""`，然后故障安全默认值被使用（`Sonnet`），即使攻击者意图是“不覆盖”（`Haiku`）——他们意外地获得了一个更高的层级。虽然这很难被恶意利用，但它是一个可能意外导致未按设计路由路径的静默行为。场景 B（绕过）：`required_when: NO` → 解析为布尔值 `false` → 序列化为 `false` → Go 字符串 `""` → `requiredWhenKey("")` 返回 `""` → 门控逻辑将其视为“始终运行”，即使作者意图是不运行。 |
| 影响 | 工作流 YAML 中静默的类型强制转换导致意外行为。低利用概率，但由于工作流驱动执行策略，中等的潜在影响。 |
| 建议 | 当已知某个字段应为字符串时（例如 `description`、`agent`、`model_tier`、`required_when`），在 `asset.Phase` 中向 `LoadWorkflowJSON` 添加架构验证。来自 `yaml2json` 的基本标量应该在 JSON 阶段之后但被解析为 Go 结构体之前，通过一个架构检查器进行验证。或者，在调用 `parseScalar` 时，将可疑字符串（像 `yes`、`no`、`null` 这样的单词）用引号括起来以保留其字符串性质，而不是强制转换为 `bool`/`nil`。 |
| 工作量 | **S** — < 1 天（向 `LoadWorkflowJSON` 添加一个架构验证函数，用于已知为字符串的字段） |

---

### FINDING-09（中）：恶意工作流导致门控检查中出现空 `argv` 或故障行为

| 字段 | 值 |
|---|---|
| 类别 | **威胁模型** |
| 严重性 | **中** |
| 标题 | 空 `Build` 或空 `argv` 导致静默的“无操作”阶段，而不是失败关闭 |
| 位置 | `forge-core/internal/orchestrator/command_executor.go:58-59`（`Execute` → `len(argv) == 0` → `configErr`） |
| 描述 | 当 `Build` 函数返回空 `argv` 时，`CommandExecutor.Execute` 返回一个 `KindConfig` 错误，该错误被认为是**不可重试的**并中止运行。然而，如果 `Build` 本身是 `nil`，则返回的 `configErr` 将 `Err` 设置为 `nil`（`configErr(p.Name, nil)`）。此外，即使在非 nil 的 `Build` 上返回空 `argv` 也算作一个配置错误。考虑到恶意的或精心设计的 YAML 工作流可能产生一个空阶段（一个具有空 `Agent` 空 `Phase`），路径将是：未设置 `Exec`（因为路由表中没有该 Agent），或者 `Exec.Execute` 被调用，然后 `Execute` 本身用 nil Build 短路。|
| 攻击场景 | 攻击者提交一个具有一个空 Agent 阶段（`agent: ""`）的工作流 YAML。`Build(p, mode)` 返回 `[""]` 或 `[]`。对于 `[""]`，`exec.Command("")` 立即失败并返回 `exec.ErrNotFound`，或针对空字符串返回 `os.ErrNotExist`。运行在 `configErr` 处中止 — 这是一种拒绝服务形式，但不是任意代码执行。在极少数情况下：如果 `Build` 返回 `[""]` 并且操作系统恰好有一个名为 `""` 的可执行文件在 PATH 上（几乎不可能，但不可否认在系统受损的情况下），它将被运行。 |
| 影响 | 低 — 该发现更像是防御性编码问题。具有空阶段的工作流将触发日志行并中止运行。不存在利用 `argv[0]` 为空的代码执行场景。严重性降低为“信息”。 |
| 建议 | 对 `configErr` 添加 `argv[0]` 验证：如果命令名包含路径遍历字符（`../`、`./`）或首字符不是字母数字，则在尝试运行之前拒绝。对于 `Build` 为 nil 的情况：记录一个不同的日志行（`"phase %s: no Build function configured — this executor cannot run any phase"`），而不是一个无差别的空 argv 错误。 |
| 工作量 | **S** — < 1 天 |

---

### FINDING-10（中）：`yaml2json` 中的行内序列解析器对括号深度没有硬限制

| 字段 | 值 |
|---|---|
| 类别 | **威胁模型** |
| 严重性 | **中** |
| 标题 | 深度嵌套的 YAML 导致行内序列解析器中的堆栈溢出 |
| 位置 | `forge-core/internal/yaml2json/inline.go`（行内序列/映射解析） |
| 描述 | 行内序列（`[a, [b, [c, ...]]]`）是通过递归解析的。没有最大嵌套深度的强制执行。一份精心设计的工作流 YAML，其嵌套深度为 ~10000 （通过 `[` 标记），将导致 Go 运行时中的堆栈溢出 — 导致编排器进程崩溃（`SIGSEGV`，因为 Go 使用有界 goroutine 堆栈）。虽然在这种有限部署的本地工具中这不是远程漏洞，但在 YAML 解析过程中的崩溃会在没有干净关闭的情况下终止编排器，从而可能损坏检查点或留下部分状态。 |
| 攻击场景 | 针对本地可执行文件的基于 YAML 的拒绝服务：攻击者提交一个具有深度嵌套行内结构的工作流 YAML 文件。`forge run <workflow>` 加载 YAML，解析器递归崩溃 → 编排器终止 → 如果检查点文件部分写入，则预算跟踪可能丢失 → 团队可能不得不手动清理检查点状态。 |
| 影响 | 来自 YAML 解析的本地拒绝服务。工作流状态可能损坏。 |
| 建议 | 向 `parseInlineValue` 添加一个最大嵌套深度参数（例如 64 层）。当深度超过限制时，返回一个带有堆栈跟踪信息的错误。或者，将解析器重写为迭代式（对于当前的使用场景，这可能过于复杂；64 层的限制是更简单且更合理的修复方式）。 |
| 工作量 | **S** — < 1 天（向所有解析器函数添加 `depth int` 参数；在 64 或 128 层深度时返回错误） |

---

### FINDING-11（低）：检查点临时文件后缀可能会留下陈旧的 `.tmp` 文件

| 字段 | 值 |
|---|---|
| 类别 | **数据保护** |
| 严重性 | **低** |
| 标题 | 检查点 `Save` 在失败的重命名后不会清理 `.tmp` 文件 |
| 位置 | `forge-core/internal/persist/checkpoint.go:99-108`（`Save` — `writeSynced` 后跟 `os.Rename`） |
| 描述 | `Save` 将检查点写入一个 `.tmp` 文件，执行 `f.Sync()`，然后将其重命名为最终文件。如果 `os.Rename` 在写入成功后失败（例如在跨文件系统边界运行时出现 `EXDEV` 错误），当前代码会通过 `os.Remove(tmp)` 清理临时文件。然而，如果进程在 `writeSynced` 返回之后但在 `os.Rename` 之前崩溃，或者如果操作系统的 `rename(2)` 系统调用本身被中断，则 `.tmp` 文件将作为陈旧的暂存文件留在文件系统上。这些陈旧的 `.tmp` 文件可能包含敏感数据（阶段路线图完成百分比、模式、支出金额 — 但机密 API 密钥不会出现在检查点中）。虽然文件权限设置为 `0644`，但在共享文件系统上，这些文件可能对同一台机器上的其他用户可读。 |
| 攻击场景 | 共享开发环境中的信息泄露：攻击者在 `.forge/` 中检查陈旧的 `.tmp` 文件，发现其中包含带有路线图完成百分比的检查点数据，该数据可能揭示正在进行的工作。 |
| 影响 | 极低 — 检查点数据是运行时状态（模式、迭代、路线图百分比），不是秘密。权限为 `0644`。这主要是一个整洁性问题。 |
| 建议 | 添加一个后台清理机制（关闭 + 移除），用于在运行开始时或在 `Save` 被调用之前的加载过程中发现陈旧的 `.tmp` 文件。在 `Load` 函数中，如果 `.tmp` 文件比对应的最终文件更新，则发出警告。 |
| 工作量 | **S** — < 1 天（在 `Save` 开始时添加一个 `os.Remove(path+".tmp")` 前置步骤） |

---

### FINDING-12（低）：FIPS/合规背景下的弱密码学随机性（审批标记）

| 字段 | 值 |
|---|---|
| 类别 | **密码学** |
| 严重性 | **低** |
| 标题 | 审批标记依赖文件存在性而不依赖密码学签名 |
| 位置 | `forge-core/cmd/forge/gates.go:125-131`（`humanApproved` — 使用 `os.Stat`，无密码学） |
| 描述 | 审批标记机制不使用任何密码学原语。见 FINDING-01 中关于绕过标记的详细内容。从密码学角度，该标记需要在内容上至少有一个 `crypto/rand` 随机数或一个 HMAC 签名，才能被认为是一个抗篡改的审批信号。当前的文件存在性检查在 FIPS 140-3 或 SOC 2 CC6.1（需要加密保护的逻辑访问）等合规框架下不满足要求。 |
| 攻击场景 | 见 FINDING-01。没有密码学绑定，标记就只是任意的文件。 |
| 影响 | 不计入密码学控制点的合规信用。 |
| 建议 | 将 `crypto/rand` 整合到标记中（生成 256 位随机值，将其 base64 编码，并将其与审批元数据一起存储）。在验证时，对照已存储的已知良好目标值检查整个文件。即使已经实施 FINDING-01 中建议的随机解决方案，也增加了密码学绑定。 |
| 工作量 | **S** — < 1 天（`crypto/rand` 用于标记生成；将标记布局从纯存在性更改为 `{"approval_id": "<random>", "stage": "design", "timestamp": ...}`） |

---

### FINDING-13（低）：验证器没有 `computeCodeTestRatio` 和 `computeFileDelta` 的错误返回

| 字段 | 值 |
|---|---|
| 类别 | **合规性** |
| 严重性 | **低** |
| 标题 | 聚合、诚信相关的计算错误被静默忽略（始终返回 0） |
| 位置 | `forge-core/cmd/forge/gates.go:170-183`（`computeCodeTestRatio`）、`gates.go:196-221`（`computeFileDelta`） |
| 描述 | `computeCodeTestRatio` 和 `computeFileDelta` 在 Git 操作或文件读取出现任何错误时静默返回 0。虽然这些被记录为“仅用于信息丰富，不是聚合输入”，但一个始终为 0 的系统性故障（例如缺少 `git` 二进制文件、非 Git 目录、损坏的仓库）会产生一个持续的错误画面：LoopEngine 可能会在每次迭代时记录“test-gap warning”（第 80-82 行）和“honesty gap”（第 63-66 行），提醒团队注意代码覆盖率的缺失 — 但这些警告是基于 0 值的，这意味着在非 Git 目录中，每次运行都会错误地触发它们。 |
| 攻击场景 | 没有直接的安全利用点，但持续的误报可能导致团队忽略真正的警告（“狼来了”效应），从而遗漏真正的回归问题。 |
| 影响 | 分析降级 — 诚信警告在非 Git 目录中会误触发，导致团队可能屏蔽它们。 |
| 建议 | 添加一个环境检测（`os.Stat(".git")`）在运行这些函数之前，以便在非 Git 目录中，它们完全跳过并返回一个显式的 sentinel 值（例如 `-1`），而不是 `0`。调用者使用 `-1` 来抑制警告。 |
| 工作量 | **S** — < 1 天 |

---

### FINDING-14（低）：`configErr` 中 nil Build 的处理

| 字段 | 值 |
|---|---|
| 类别 | **输入验证** |
| 严重性 | **低** |
| 标题 | 当 `Build` 为 nil 时，`configErr` 返回一个包含 `nil` 原因的 `*ExecError` |
| 位置 | `forge-core/internal/orchestrator/command_executor.go:55-56`（`configErr(p.Name, nil)`） |
| 描述 | 当 `Build` 为 `nil` 时，`configErr` 创建一个 `*ExecError`，其 `Err` 设置为 `nil`。这意味着 `errors.Is(err, someError)` 通过该 `ExecError` 之后将始终返回 `false`，因为它没有要检查的基础错误。这阻止了上游的 `errors.As` 或 `errors.Is` 检查正常工作。 |
| 攻击场景 | 非安全 — 代码质量问题。 |
| 影响 | 调试困难；如果调用方期望 `errors.Is(exec.ErrNotFound)` 返回 `true`，则在测试中会出现意外行为。 |
| 建议 | 为 nil Build 错误提供一个明确的 sentinel 错误：`var ErrNilBuild = errors.New("executor: no Build function configured")`，并在 `configErr` 中使用它而不是 `nil`。 |
| 工作量 | **S** — < 1 天 |

---

### FINDING-15（信息）：`yaml2json` 解析器以静默方式重写未知的 YAML 标签

| 字段 | 值 |
|---|---|
| 类别 | **威胁模型** |
| 严重性 | **信息** |
| 标题 | YAML 标签（`!!str`、`!binary`）被忽略 — 潜在的隐式类型强制转换 |
| 位置 | `forge-core/internal/yaml2json/yaml2json.go:18-30`（文档中说明不支持标签）和 `scalar.go` |
| 描述 | 文档说明 YAML 标签（`!!str`、`!binary`、`!custom`）不被支持，但解析器不会在遇到它们时报错 — 它会静默地将值视为未标记的 YAML。一个像 `key: !!binary dGhpc0lzQmFzZTY0Cg==` 这样的条目将被解析为字符串 `dGhpc0lzQmFzZTY0Cg==`（base64 编码的原始字面量），而不是被解码为二进制数据。类似地，`!!str 12345` 会产生字符串 `12345`，但 `12345`（不带标签）会产生数字 `12345`（float64）。一个工作流作者在键上使用标签不会触发错误，但可能会产生意外的类型。 |
| 攻击场景 | 不适用于利用，但需要注意：如果 YAML 是外部来源提供的，那么意外地意外地以意想不到的类型使用不当标签的值可能会影响逻辑。 |
| 影响 | 知识/文档目的。这不会导致注入问题。 |
| 建议 | 添加关于不支持标签的文档注释，并考虑在遇到标签时记录一个调试级别的警告（`fmt.Fprintf(os.Stderr, "yaml2json: line %d: ignoring YAML tag\n", l.number)`）。 |
| 工作量 | **S** — < 1 天 |

---

## STRIDE 分析摘要

| 类别 | 评估 | 主要发现 |
|---|---|---|
| **欺骗** | 🟡中等 | 审批标记可以伪造（FINDING-01），Agent 身份派生自可修改的工作流 YAML |
| **篡改** | 🟢良好 | 检查点写入是原子的（重命名）；检查点 JSON 包含了 `format_version` 字段 |
| **抵赖** | 🟢良好 | 所有操作都会被记录；Agent 门控结果标记（APPROVE/REQUEST_CHANGES）是从 Agent 输出中解析出来的 |
| **信息泄露** | 🟡中等 | 环境变量泄露给子进程（FINDING-02）；YAML 错误暴露路径（FINDING-03）；`.tmp` 文件可能暴露运行时状态（FINDING-11） |
| **拒绝服务** | 🟡中等 | 深度嵌套的 YAML 导致堆栈溢出（FINDING-10）；Windows 上挂起的进程组（FINDING-07）；git 标志注入（FINDING-04） |
| **权限提升** | 🔴需要关注 | 审批标记绕过允许跳过人类审查门禁（FINDING-01）；无沙盒限制 Agent 命令（SandboxConfig 是一个占位符） |

---

## 整体安全态势

| 维度 | 评级 |
|---|---|
| **整体状况** | 🟡需要改进 |
| **认证与授权** | 🔴需要关注 — 审批标记是纯文本文件 |
| **输入验证** | 🟡中等 — Python shim 回退和 git 命令注入 |
| **密码学** | 🟢可接受 — 不适用于本地运行时（无需 TLS；无网络表面） |
| **会话管理** | 🟢不适用 — 无状态的 CLI 工具 |
| **数据保护** | 🟡中等 — 环境变量泄露；标记可伪造 |
| **合规性** | 🟢对于当前状态可接受 — 支持 N/A 的诚实门控；缺乏 FIPS 绑定在 v2 之前是可以接受的 |

---

## 最重要的 3 个问题

1. **FINDING-01（严重）：人类门禁审批标记可被伪造** — 整个安全模型中最关键的改变。修复应该优先于所有其他安全工作和功能开发。

2. **FINDING-02（高）：环境变量泄露给子进程** — API 密钥通过 `os.Environ()` 被批量传播到所有子进程。由于──executor=command 为代理进程提供了对 Bash的访问权限，任何 `env` 或 `/proc/self/environ` 都会泄露凭据。

3. **FINDING-03（高）：Python YAML shim 回退** — 一个在解析错误时产生远程代码执行意味的废弃组件。将其移除会消除不需要的攻击面，并加强零依赖策略。

---

## 最重要的 3 个快速胜利

| # | 查找 | 工作量 | 影响 |
|---|---|---|---|
| 1 | **FINDING-03**：移除 Python YAML shim 回退 | **S**（< 1 天） | 消除了一个根深蒂固的代码执行载体，强化了零依赖策略，简化了构建过程 |
| 2 | **FINDING-02**：在 `childEnv` 中过滤凭据环境变量 | **S**（< 1 天） | 保护 Claude API 密钥不被泄露给随意的 `git diff` 和 `node` 子进程 |
| 3 | **FINDING-04**：在 `RepoRoot` 中验证仓库根路径 | **S**（< 1 天） | 阻止 `git -C` 标志注入，并确保路径是一致性的目录 |

---

## 安全债务

| 项目 | 类型 | 当前状态 | 计划 |
|---|---|---|---|
| SandboxConfig 未实现，占位符 | 架构性 | 代码中的占位符结构体；`CommandExecutor.Sandbox` 是一个 nil 字段 | v3 路线图（Firecracker 微虚拟机） |
| 对 Agent 回调中成本/延迟的信任（`Observe`、`ClassifyOverload`） | 信任边界 | Agent CLI 向编排器报告 `total_cost_usd` 和延迟；这可能被伪造 | 计划中：与端点签署遥测数据 |
| Windows 进程组隔离 | 平台 | `setupProcessGroup` 在非 unix 系统上是空操作 | 识别为 v2 项目 |
| FIPS 合规的随机数用于审批标记 | 合规 | 尚未实现 | 与 FINDING-01 修复捆绑 |
| 通过 `forge approve list` 审计审批记录 | 可审计性 | 当前仅列出待审批项；没有审批历史记录 | 逻辑上的下一个项目 |
| 机密扫描器熵检测 | 检测 | 仅基于模式；无熵 | FINDING-06 的 v0+1 增强 |

---

## 参考文献

- **OWASP Top-10 2021**: A01（权限控制失效）、A03（注入）、A04（不安全设计）、A05（安全配置错误）
- **OWASP Agentic Top-10 2025-12**: A1（权限不当）、A6（敏感信息披露）
- **NIST SP 800-53**: AC-6(9)（最小权限）、SC-8（传输机密性和完整性）、SC-13（密码学保护）
- **PCI-DSS v4.0**: 要求 3（存储的保护数据）、要求 7（限制业务知悉需要基础上的访问）
- **SOC 2**: CC6.1（逻辑访问安全）、CC6.7（数据的不可否认性/完整性）
