# ForgeOS — Gap Analysis (phase=gap-analysis, architect)

> Producer: `architect` (tier=opus, mode=engineering, lifecycle=mvp) · 2026-08-09
> Upstream input: `EVOLVE_SCAN_V1` (thorough, 6 dimensions) + memory 中 10+ 轮
> 「roadmap=100%, gates_green=false」的重复教训。
> Target: `.agent/ROADMAP.md`(v2 完成态 + v3 规划) · `.agent/ARCHITECTURE.md` 脊柱 ·
> `.agent/AGENTS.md` 硬闸门 · `.agent/PROJECT.md` G1–G5 · north-star。

## 0. 结论先行 (ranked)

| # | Gap | 维度 | 价值 (why) | 风险若不修 | 成本 | 顺序 |
|---|-----|------|-----------|------------|------|------|
| G1 | `forge-runtime` 源回归:4 acceptance FAIL 的**真实根因**(E0432×2/E0282,恰在 pinned 1.93.0 复现) | code | 主仓 CI 自 HEAD 起红;`forge accept` 不可能 ACCEPTED;是记忆里多迭代「100% roadmap 但 gates red」死循环的死结 | 高:evolve 迭代继续空转,记录继续失真 | S | **第 1** |
| G2 | Sprint 86/87 记录失真(「Cargo 1.83 on PATH」归因已被事实证伪) | architecture_drift | 中高:记录是 fresh-context 信任与后续审计的来源;错误归因会再造错误修复 | 中:不改正则持续污染下游判断 | S | ②(须等 G1 fix commit 落盘) |
| G3 | SCA 盲区:`Cargo.lock` 216 个锁定依赖不与 OSV 比对(全仓最大依赖面) | dependencies | 高:tokio/reqwest/rusqlite/schemars/sha2 供应链风险零可见性 | 中:新解析器+fixtures,须守零依赖纪律 | M | ③(与 G1 无依赖,可并行) |
| G4 | a) `.coverage` 被 Git 跟踪,每次 pytest 弄脏树;b) 阈值已声明(0/60/80)但 coverage 判据从不执法 | test_coverage | 中:树卫生 + 「有工具则真查」的诚实缺口 | 低 | S→M | ④(a 无条件做, b 须经 ADR 0040 裁决) |

每项都有**真仓证据**(见 §1),且**全部是单体范围内修复**:不换架构边界、不触发任何
拆服务/事件驱动演进触发器(lifecycle 保持 mvp,见 §4 防镀金审计)。

## 1. 差距基线:architect 独立复验的当前事实 (HEAD `f70f841`, 2026-08-09)

不只引用 scan 证据,我直接复核:

```
$ cargo --version → 1.93.0 (083ac5135) ; rustc 1.93.0      # = forge.yml pinned 工具链
$ cd forge-runtime && cargo check --all-features --offline
error[E0432]: unresolved import `super::unavailable`
  → crates/infrastructure/src/sqlite_hub/schema_location.rs:8:28
error[E0432]: unresolved import `super::unavailable`
  → crates/infrastructure/src/sqlite_hub/schema_contract/reentry.rs:15:21
error[E0282]: type annotations needed
  → crates/infrastructure/src/sqlite_hub/schema_location.rs:153:19
EXIT=101
```

- **机制**:`schema.rs` 以 `#[path]` 声明 `mod location;` → `schema_location.rs` 的父模块是
  `schema`;`unavailable` 只定义为 `sqlite_hub/mod.rs:478` 的**私有** fn,`schema` 不再
  re-export。`git show --stat f70f841` 确认该 commit 移除了 re-export → **回归由 f70f841 引入**。
- **连带失效**:
  - `harness/adapters/rust.yml` 配置 `cargo clippy -- -D warnings` + test/check/build →
    编译失败即 FAIL/exit 101 → acceptance 的 lint 判据诚实报 FAIL;
  - `harness/test_adapters.mjs:170` 断言仍写死「repo 无 linter 配置 ⇒ lint 必须 N/A」,
    与 `rust.yml` 已配置的事实矛盾 → 该 harness 测试自身也 FAIL;
  - `.github/workflows/forge.yml` 每次 push 跑 accept → **main 自 f70f841 起红**。
- **G4a 证据**:`.gitignore:7` 只忽略 `coverage/`;`git ls-files | grep '\.coverage$'` = 1;
  `git status --short` 显示 `M .coverage`(每次 pytest 改写跟踪文件)。
- **G3 证据**:`harness/sca.mjs` `MANIFESTS`(L≈59)只覆盖 `go.mod / package.json /
  requirements.txt`;`forge-runtime/Cargo.lock` 216 个锁定包不进 OSV 比对循环。
- **G4b 证据**:`.agent/policies/modes.yml:48/69/87` 声明 `coverage_threshold 0/60/80`;
  `.agent/eval/acceptance.schema.yml:45` 的 coverage 判据 `required: false`,从未接入执法。

## 2. Gap 契约与验收判据(可机器验证;architect 只定规格,不写修复代码)

### G1 — Repair forge-runtime import regression (价值/风险双高,第 1 优先)

契约(→ implementer):
- 让 `schema` 对 `super::unavailable` 重新可见:在 `schema.rs` 加 `pub(super) use super::unavailable`
  (最小方案),或等价模块路径调整;**禁止**把 `unavailable` 改为全局名的方案。
- 修复后用 **pinned 工具链**复验 forge.yml 中的全部后缀:offline 下
  `cargo fmt --check` · `cargo test --all-targets --all-features` ·
  `cargo clippy --all-targets --all-features -- -D warnings` ·
  `cargo check` + `cargo build` —— 0 error。
- `harness/test_adapters.mjs:170` 断言同步修正:真实仓库配置了 linter 时,修复后应得
  **PASS**(而非硬编码 N/A);该断言契约 =「无工具 ⇒ N/A;有工具 ⇒ 如实 PASS/FAIL」。
- **验收(机器)**:`node harness/acceptance.mjs` 输出 `QA_VERDICT`/接受 ACCEPTED(允许的
  诚实 N/A 除外),且 `.github/workflows/forge.yml` 同 HEAD 全绿。**不 ACCEPTED 不许收敛。**

### G2 — 修正 sprint 记录 (drift;依赖 G1 fix commit)

- `.agent/CURRENT_SPRINT.md` Sprint 86/87 尾部改写:
  「4 FAIL 归因 Cargo 1.83 / 与本次 round 无因果」→ 如实记录:「HEAD(f70f841)引入的源级
  回归,在 pinned 1.93.0 上可复现,由 commit <G1-fix> 修复;失败与修复均出真仓证据」。
- **验收**:记录中不再出现「Cargo 1.83 / PATH 工具链」归因;含 G1 fix commit;fresh-context
  Reviewer 复核通过。

### G3 — 把 SCA 扩展到 Cargo.lock

契约(保持 harness 零外部依赖、offline 确定性):
- `harness/sca.mjs` 增 manifest kind `Cargo.lock`(ecosystem `crates.io`):解析
  `[[package]] {name, version}`,**跳过** `source = "path"` 的 workspace 依赖;与其他 manifest
  同一 OSV 比对与输出格式。
- `harness/test_sca.mjs` 增 fixtures(go.mod 既有形状 + 纯 Cargo.lock + 混合场景)。
- `sca_fetch.mjs` 重生成 `.agent/security/advisories.json` 快照(含 crates.io 生态,保持
  确定性 offline 可用;不能联网时在快照内如实注明范围)。
- **验收**:acceptance 的 SCA 判据对 forge-runtime 显示≥1 个真实包的比对覆盖(不再只比
  PyYAML);harness 套件全绿。

### G4 — coverage:先止血,再决定执法

- **G4a(无条件,立即)**:`.gitignore` 增 `.coverage` / `.coverage.*`;`git rm --cached .coverage`;
  pytest 后 `git status` 干净。验收 = git 树干净。
- **G4b(决策项)**:把覆盖率探针接入 acceptance 的 coverage 判据(阈值从
  modes.yml `mode×lifecycle` 读;工具缺失仍诚实 N/A,绝不伪 PASS)。此改动变更 Stop 闸门语义,
  属复杂决策 → 已出 ADR 草案(`docs/agent/adr/../adr/0040-honest-coverage-enforcement.md` 见
  `docs/adr/0040-honest-coverage-enforcement.md` 草案)。**未批准不实施**,实施后验收 =
  带阈值实跑 + 无工具时 N/A 可见且不算已满足。

## 3. 交付序列与依赖图 (dependency-aware)

```
iter 1: G1 + G4a(零风险同轮)
        → 实跑 `node harness/acceptance.mjs` —— 必须 ACCEPTED,否则不声明收敛
iter 2: G3(独立可并行)+ G2(G1 fix 落地后才可写)
        → 全量 accept 复跑
iter 3: G4b 依 ADR 0040 批准与否决定(未批准 = 维持诚实 N/A,不再列为缺陷)
```

依赖:`G1 ⟵ G2`;`G1 ∥ G3 ∥ G4a`;`G4b`(ADR 批准)。无其他耦合。

## 4. 防镀金审计 (day-1 gilding / 范围纪律)

- 四项全部是**既有工具/记录的缺陷修复**:不建新服务、不引中间件/事件驱动/CQRS/Kafka/
  k8s;对外架构(north-star 目标态)零变化;lifecycle 保持 mvp。
- 不把「lint 报了 FAIL」写成 N/A(诚实红线);不把记录偏差就地圆场。
- 本分析自身不写实现代码、不开 sprint 任务、不做厂商/SKU/预算决策(→ cto)。

## 5. 停止条件与本阶段的交接

- 本 gap-analysis 的交付 = 上表 4 项契约 + 依赖图 + 验收标准;**G1 必须先落地**
  (它是「100% roadmap 但 gates red」重复死循环的物理死结)。
- 下家:planner(按 §3 序列拆任务)→ implementer → fresh Reviewer → accept 复验;
  G4b 交 cto 综合裁决后再决定是否实施。
- 需求源头无缺口:AGENTS 硬闸门 + `forge accept` 契约本身就是本轮的 NFR;不需退回
  product-manager。