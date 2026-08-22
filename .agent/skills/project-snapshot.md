# Project Snapshot — bounded local source observation adapter

## 职责与触发

在 Run 开始、resume，或 source/configuration/deployment 文件可能变化后，需要一个显式、
content-addressed 的本地 source reference 时使用本 adapter。ADR-0070 只交付
`project-snapshot` package 的窄切片：Linux Catalyst producer 对 Git worktree 做两次完整端点
观察，portable Skill 对输出做 strict validation。它不是 atomic/current/complete ProjectSnapshot，
也不是 GraphSnapshot、secret scan、configuration/deployment classifier 或 authority source。

portable package 位于 `skills/project-snapshot/`。本 `.agent` 文件只是 ForgeOS 路由入口；文件
存在、scaffold 复制或 package 校验成功都不表示宿主已安装 Skill、存在兼容 `forge` runtime，
或已获 worktree/process 权限。

## 输入契约

- caller 为产品 CLI 与 portable adapter 显式提供 clean canonical absolute Git worktree root、
  `project_id`、`run_id` 与 compatible `forge` executable；不得从目录名、remote、branch、
  环境或 actor 推断。底层 library 可独立 canonicalize，但 public CLI 必须拒绝相对或不 clean root。
- 产品 surface 只有
  `forge project-snapshot capture --project-id ID --run-id ID --root DIR`；三个 option 各一次、
  可重排、无 stdin。
- portable adapter 必须显式接收 `--forge /absolute/path/to/forge`，并先通过 closed package
  manifest 校验。unsupported host 或 runtime 路径不存在时固定 exit 3/`not_executed`；路径存在但
  malformed、non-executable、wrong-architecture、CLI-incompatible 或执行/验证失败时固定 exit 1。
  禁止替换为 `find`、`git archive`、`git status` 或自制 hash loop。
- host permission 位于本 Skill 之外。本 adapter 不授予 filesystem、process、Git、network、
  secret、Registry、Grant 或 persistence 权限。

## 执行 SOP

1. 运行 `python3 -I -B skills/project-snapshot/scripts/check_package.py`，拒绝 extra/missing、
   symlink/hardlink/special file、mode/size/digest 或 closed-manifest drift。checker 要求
   descriptor-relative no-follow primitives；缺失时 fail closed 为 exit 1，绝不把 unchecked package
   记为 `not_executed` success。`-I` 必须位于 script path 前；它排除 script/current directory、
   `PYTHONPATH` 和 user site 作为 import source，entrypoint 源码在自身 non-built-in imports 前检查
   `sys.flags.isolated`。它不禁用、认证或隔离 system site、stdlib、interpreter startup、host 或
   publisher。package check 与后续 capture 是两个非原子操作：PASS 不锁定整个使用期，也不认证
   publisher。capture 只从自身锚定的 exact vendored package location 加载，不向 `sys.path` 添加
   `scripts/` 或 `_vendor/`；这阻止 package/PYTHONPATH import shadow，不证明 check 后未突变。
2. 阅读 `skills/project-snapshot/references/contract.md`，确认固定 sensitive/control policy、
   Linux-only capture 和 coverage/authority caveat。
3. 在 shell invocation 前选择 captured root 外的 output target；adapter 无法感知 shell redirection。
   root 内目标会在首轮 observation 前被创建或截断，使 capture 不再观察 invocation 前的 exact root；
   final write 又会使目标偏离 capture 期间观察到的 bytes，后续 recapture 还可能纳入旧输出。然后通过唯一
   adapter 执行：

   ```sh
   python3 -I -B skills/project-snapshot/scripts/capture.py \
     --forge /absolute/path/to/forge \
     --root /absolute/path/to/worktree \
     --project-id PROJECT_ID \
     --run-id RUN_ID > /absolute/path/outside/worktree/project-source-snapshot.json
   ```

4. adapter 必须等待 child 成功并以 vendored strict checker 验证完整 canonical production，
   才能写任何 success stdout。参数错误为 2，capture/validation 为 1，runtime unavailable 为
   3/`not_executed`；这些失败不得留下可消费 snapshot。
5. 下游同时绑定 `request_sha256`、`source_manifest_sha256`、`coverage_sha256`、
   `snapshot_identity_sha256`/`snapshot_id` 和 `snapshot_sha256`。`source_revision` 只是
   unauthenticated HEAD hint，不能替代五项 identity。
6. resume 或 relevant change 后重新 capture。相同结果只表示两个 bounded observation 相等，
   不证明 writer quiescence、currentness 或 atomicity。

## 输出契约

只接受 `forgeos.governance.local-project-source-snapshot-production/v1`。唯一正结果是
`CAPTURED_BOUNDED_LOCAL_PROJECT_SOURCE_OBSERVATION` 的完整 Schema 常量：固定
`atomic=false`，`freshness/currentness/system_completeness=unknown`，authority/permission/
truth/persistence/effect attestations 全为 false。

manifest 只包含 allowed regular worktree entry、tracked-absent fact、hashed metadata-only
exclusion、ignored count 与 unauthenticated Git observation。它不包含 raw content、raw excluded
path、symlink target、ignored locator、configuration/deployment semantics 或 graph topology。
Tracked、nonignored-untracked 与 ignored-count coverage 保持 PARTIAL；其余 surface 保持 exact
UNKNOWN/NOT_OBSERVED/NOT_PERFORMED partition。

## 规则、禁止与权限

- built-in policy 匹配必须先于 collector leaf lstat/open/read/readlink；不得打开或披露 matched
  sensitive/control leaf，也不得读取 symlink target。
- path policy 不是 content DLP。allowed 名称仍可能含 secret；Git/config/control-metadata 读取不在
  collector leaf guarantee 内，ignored bytes 也未检查。
- 禁止把两次 endpoint equality、HEAD、零 exclusion、checker PASS 或 package VALID 改写为
  atomic/current/complete/clean/secret-free/authenticated/authorized/persistent/effect-attesting。
- 禁止由本结果生成 GraphSnapshot、Evidence/Claim/Knowledge truth、CapabilityInvocation、Grant/
  PDP/Approval/Transition，修改 ADR-0068 Registry，选择 runtime effect 或写 project memory。
- v1 live producer 与 capture adapter 只支持 Linux。unsupported host 由 adapter 在 runtime access 前
  exit 3/`not_executed`；存在但不兼容或执行失败的 runtime 为 exit 1。不能弱化 filesystem primitive
  或静默 fallback。pure decoder 只是 source-portable，不执行 live capture。

## 自动化与验收

- Schema：`docs/contracts/project-source-snapshot-v1.schema.json`
- Golden：`docs/contracts/fixtures/project-source-snapshot-v1.json`
- Strict checker：
  `python3 -I -B harness/project_source_snapshot_contract/check.py --golden .`
- Portable package：`python3 -I -B skills/project-snapshot/scripts/check_package.py`
- Go producer：`go test ./internal/projectsnapshot`（仅 Catalyst repository runtime）
- Forward eval：`skills/project-snapshot/references/evals.json` 的 normal 与 dangerous case

Scaffold/upgrade 复制 adapter、portable package、ADR、Schema、golden、Python checker/tests，但
不复制 Catalyst `forge-core` runtime，不安装宿主 Skill，也不提供权限。最终完成权只属于
`forge accept`；shadow detector、snapshot validation 与 package validation 都不能替代它。
