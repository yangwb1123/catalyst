# ADR 0003 — agent-os 独立仓化:治理资产经 submodule + .agent 覆盖层全局共享

- 状态:Proposed
- 日期:2026-06-20

> **设计记录,非执行授权。** 拆分边界 / 机制 / 覆盖语义 / 迁移计划在本 ADR 内已定;
> **远程仓库位置**与**启动不可逆迁移的批准**留给用户(见「## 待拍板」)。用户定位置 +
> 批准前,不创建任何仓、不改任何代码。对应 `../../.agent/DECISIONS.md` O2 与
> `../../.agent/ROADMAP.md` v2 待项「独立 agent-os 仓库(submodule)」。

## 背景

ForgeOS 的价值是**治理资产**(红线 + harness 执法器 + 声明式 agent/skill/workflow/eval/
routing/policies + 架构规则)。`harness/forge-init.mjs` 已把它**逐项目复制**(Catalyst Vision
70%/30% 拆分):70% UNIVERSAL、对每个项目相同、COPIED verbatim;30% PROJECT identity、
GENERATED。这交付了「可继承、可运行、完整」的治理 —— 一命令 stamp 出 day-1 即 ACCEPTED 的项目。

但「复制」是**快照**:中心改了红线 / 加了 arch-check / 调了路由,**已复制出去的项目收不到**。
ROADMAP v2 要把治理抽到独立 `agent-os` 仓,各项目经 **submodule + `.agent` 覆盖层共享**:
**中心一改,所有项目继承**。本 ADR 决定**是否、如何**做,并诚实权衡「复制已够」的反方。

**两个载重事实(已核实):**

1. **拆分边界已由 forge-init 定义且被测试锁定。** `GOVERNANCE_DIRS`(`.agent/{agents,skills,
   workflows,eval,routing,policies}/`)+ `COPIED_FILES`(`.agent/AGENTS.md` + `.arch/rules.yaml` +
   全套 `harness/*.{mjs,py}` + `arch/*` + `policies.yml` + `adapters/*.yml` + self-test)= 「全局」
   清单;`.agent/{PROJECT,ROADMAP,CURRENT_SPRINT}.md` + `project.yml` + `CLAUDE.md` + README +
   gitignore + CI + `examples/starter/` = 「项目生成」。`test_forge-init.mjs` 对每个 COPIED 条目
   断言 byte-identical —— 机制变更须同步此测试。
2. **harness 路径解析是「混合锚定」,这是 submodule 化的核心机械障碍(已核实)。**
   - **自身位置锚定**(submodule 化会算错项目根):`acceptance-kernel.mjs:19`/`sca.mjs:48`/
     `secret-scan.mjs:42` 的 `ROOT = dirname(HARNESS_DIR)`、`arch-check.mjs:16` 的
     `dirname(dirname(ARCH_DIR))`(并以此自锚 ROOT 调参数化、本身根无关的 `scan.mjs`)、
     `acceptance.mjs` 扫 `join(ROOT,'examples')`。harness 落入
     submodule 子目录 → 这些算出 ROOT = submodule 根、扫错树(扫不到项目 `src/`/`.agent/PROJECT.md`)。
   - **cwd 锚定(已对)**:`gate.mjs:14 ROOT=process.cwd()`、`check.py` 默认 cwd。
   - forge-init「复制」零接线就能跑,正因 harness 复制到项目根 `harness/`、两种锚定**意外一致**。
     搬进 submodule 这巧合破裂 —— 这是机制选择与「是否值得现在做」的关键输入。

## 决策

**git submodule** 共享,**`agent-os/` submodule(全局宪法)+ 项目根 `.agent/` 覆盖层(本地实例)**
双层;**渐进**:本地原型 → 远程;**dogfood 优先**:ForgeOS 本仓先消费 agent-os。

**决策 1 — 拆分边界:沿用 forge-init 70%/30%,裁定四个边界 case:**
- **`CLAUDE.md` → 项目层。** 它已是 forge-init GENERATED(注入项目名)、是 CC 这一宿主的薄适配器,
  随「harness 住哪」而变 —— 天然项目层。
- **`.arch/rules.yaml` + `policies.yml` → 全局基线(agent-os)+ 项目旋钮(已存在)。** 入 agent-os 作
  全局默认;项目覆盖走 `project.yml` 的 `overrides` + `mode×lifecycle`(已实现)。**约束**:二者被
  `arch-check` drift-guard 要求相等,必须**同住 agent-os**(否则跨仓护栏失效),项目不直接改这两份。
- **`harness/adapters/*.yml` → 全局。** 语言工具映射普适,正是「中心一改全体继承」最受益处。
- **`examples/starter/` → 项目层。** 种子注定被真实 feature 替换,非宪法。
- 一句话:**对所有项目一样且会被持续改进的入 agent-os;项目身份 / 本地实例 / 会被替换 / 随宿主变的留项目层。**

**决策 2 — 机制 submodule(否决 subtree / vendoring / npm / symlink):** 第一性目标是「真·中心更新」(中心
一改、所有项目继承),subtree「无中心上游」、vendoring(= forge-init 现状)「无版本指针」结构上
做不到。submodule 的 detached-HEAD / init 复杂度是真实代价,但**只在 clone/update 边界**、可被
forge-init 生成的 `git submodule add` + README 吸收。**否决 npm 包**:引入包管理器 / registry
违反 forge-core/harness **零外部依赖**铁律(ADR 0002),且 `.agent/*.md` 声明式资产装进 npm 是错配。
**否决 symlink**(DECISIONS O2 第三选):无版本指针、跨机不可移植,实质同 vendoring 类。

**决策 3 — 覆盖语义 + 路径解析改造(硬核):** agent-os 与项目 `.agent/` **文件名集合不相交**
(`PROJECT/ROADMAP/CURRENT_SPRINT/project.yml` 从不在 COPIED 清单),「覆盖」退化为「并置」——
工具同读 agent-os 全局规则 + 项目本地状态,无「同名谁赢」冲突;唯一可调旋钮走 `project.yml`(值级,
非文件级)。**但路径解析必须改**:引入单一权威「项目根」—— `FORGE_PROJECT_ROOT` 环境变量(默认
`process.cwd()`),由 forge 入口设为项目仓根;自身位置锚定的工具(`acceptance.mjs`/`arch-check.mjs`/
`scan.mjs`/`secret-scan.mjs`/`sca.mjs`)把 `ROOT` 改为 `process.env.FORGE_PROJECT_ROOT ?? cwd`、自身
位置仅用于定位 sibling 工具 / self-test;`acceptance.mjs` 的 cwd 相对 test-glob 改基于 `HARNESS_DIR`
绝对 glob。**向后兼容**:未设 `FORGE_PROJECT_ROOT` 回落 cwd == 项目根、单仓行为不变。**此项是迁移
最大单点工作量、也是「是否值得」权衡重心**(触碰执法热路径,改坏即假绿)。

**决策 4 — 仓库位置:本地原型 → 远程(唯一需用户拍板):**
- **阶段 A(本地,可逆)**:本地裸仓 / `file://` 作 submodule 源,验证路径改造 + forge-init 改造 +
  ForgeOS dogfood 消费后仍 ACCEPTED。**本 ADR 批准后即可做,不碰远程。**
- **阶段 B(远程生产)**:推到用户指定远程。**远程 URL / 托管是唯一真正需用户拍板的输入。**

**决策 5 — 迁移 + 风险 + dogfood:**
- **序列**:① `git filter-repo`/`subtree split` 带历史抽 agent-os(**不可逆,需批准**)② 路径解析改造
  (保 self-test 全绿)③ **ForgeOS 先 dogfood**(本仓换 submodule,`acceptance.mjs` 仍 ACCEPTED ——
  机制成立的验收闸门)④ forge-init 从「复制」改「`submodule add` + 仅生成项目覆盖」+ 同步 test。
- **不可逆点**:历史抽取 + 批准切换 → Status=Proposed 等批准。**submodule 税**:detached-HEAD / CI
  漏 `submodules:recursive` → forge-init 生成 README + CI 模板缓解。**路径回归**:决策 3 回落 +
  dogfood 活体回归(本仓 forge accept 必须仍绿)缓解。**drift-guard**:rules+policies 同住 agent-os
  (决策 1),仍一仓内校验、不跨仓,无新增风险。
- **向后兼容**:已复制的现有项目**不强制过渡**(继续用快照),提供可选迁移配方,新项目默认 submodule。

**决策 6 — 推荐 + 诚实反方:** 推荐**批准进入阶段 A(本地原型,可逆),远程化 + 不可逆切换作 dogfood
成功后的独立 go/no-go**。诚实反方:① forge-init 复制已满足 **~80% 价值**,submodule 多换的是那 20%
(中心更新传播)—— 若被治理项目少 / 治理资产趋稳,传播收益有限、而 submodule 税每个消费者天天付;
② 路径解析改造是真实非平凡工作量、触碰最关键执法热路径(改坏即假绿);③ ROADMAP 收敛建议是「只做
方向一(韧性运行时)/ 至多 ①+②」,全局化不在当前最高杠杆路径。**净表述:机制方向已定(submodule +
双层 + 路径改造),设计在本 ADR 完结;但建议默认 (B) 暂缓,作为「触发条件达成即可执行」的就绪方案,
不现在投工 —— 与「不 day-1 镀金、按 lifecycle 演进」纪律一致。**

## 后果

- **优点**:治理真·中心化(兑现 ROADMAP v2 全局化);宪法 vs 实例关注点彻底分离;路径改造顺带消除
  「混合锚定」技术债、让 harness 真正可移植。
- **代价**:submodule init/CI/detached-HEAD 税(长期);路径改造触碰执法热路径(高风险);历史抽取不可逆。
- **反对**:复制已覆盖 80% 价值、不在当前最高杠杆;倾向暂缓至触发条件。
- **触发条件(达成即从 Proposed 推进执行)**:被治理项目 ≥ 2~3 **且** 治理资产仍高频演进。

## 待拍板(交用户)

1. **(必须)远程仓库位置** —— agent-os 远程 URL / 托管(GitHub / GitLab / 自托管)。ADR 不替选。
2. **(必须)批准启动不可逆迁移** —— 是否批准阶段 A(本地、可逆)及后续删本仓被抽目录的不可逆切换。
3. **(建议先答)now vs 暂缓** —— 决策 6 的 (A) 现在做 还是 (B) 暂缓(本 ADR 倾向 B)。

参见 `../../.agent/DECISIONS.md` O2 · `../../.agent/ROADMAP.md` v2 · ADR 0002(零依赖哲学,约束否决
npm 包)· `../../harness/forge-init.mjs`(70%/30% 拆分的事实源)。
