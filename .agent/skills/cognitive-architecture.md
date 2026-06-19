# Skill: Cognitive Architecture(认知架构)

> 目标不是「目录少」,而是**认知负担低 · 边界清晰 · 职责单一 · 演进可持续**。
> 北极星(litmus):新人不看 README/文档,只看目录树 30 秒,就能说出「这系统干什么、
> 核心模块有哪些、依赖大概怎么走」——达标。

## 触发条件
- Architect/Planner 设计或重组目录结构时;新项目 `forge init` 后定第一版骨架时。
- `arch-check` 的 cognitive/layering 报警,或 reviewer 觉得「打开仓库看不懂」时。

## 八条原则(设计时用)
1. **目录表达架构,不是技术实现**:`domains/ protocols/ platform/ interfaces/ infrastructure/`
   一眼成模型;`service/ manager/ handler/ utils/ common/` 看完不知道项目干嘛——后者是反模式。
2. **按「变化原因」聚合(一起变的放一起)**:改一个 OAuth 功能只进 `protocols/oauth/`,
   而非在 controller/service/repo/model 四处跳(技术分层的代价)。
3. **按业务域聚合**:`domains/{identity,tenant,permission,audit,federation}` 是业务能力,非技术层。
4. **协议单独成层**:oauth/oidc/saml/scim → `protocols/`,不散在一级。
5. **平台能力单独成层**:cache/cluster/events/jobs/metrics/registry → `platform/`(非业务)。
6. **外部世界放边缘**:http/grpc/cli/admin → `interfaces/`(只是进入系统的方式,非业务)。
7. **基础设施沉底**:storage/crypto/messaging/observability → `infrastructure/`(可替换实现)。
8. **顶层求「广度小、深度够」**:50 个细碎一级目录 < 10 个有层次的目录;10 个垃圾目录 < 30 个清晰目录。
   **目录数是代理指标,不是目标**——控制的是*入口处的认知广度*,靠分层加深度。

## 方法论:混用,别照抄一种
DDD(划业务域)· Hexagonal(隔外部依赖)· Clean Architecture(控依赖方向向内)·
Cognitive Load Theory(控目录广度)· Stable Dependencies(控依赖稳定性)· Package by Feature(按功能组织)。
> **混用是给「设计/判断」层的;「执法」层必须落成一张具体可检的层图**
> (`.arch/rules.yaml` 的 `layers` + `dir_aliases` + `forbidden`)。判断靠人/architect,机械靠 arch-check。

## 机械可查 vs 需要判断(ForgeOS 分工)
- **arch-check 机械执法**(可证伪、~0.1s,违规挡 `forge accept`):依赖方向(layering)、
  顶层广度(cognitive)、包大小(package)、扇入耦合(fanin)。
- **architect/reviewer 判断**(不可机械化):目录名是否真「表达能力」(`domains` 好 / `utils` 坏 是语义判断);
  30 秒 litmus;方法论取舍。reviewer 用 litmus 作启发式。
- **反模式名检测**(`utils/common/misc/manager/handler/service` 作为*唯一*组织方式)是个**廉价代理**,
  可加进 arch-check;但它只能抓「明显的技术分层」,证不了「结构好」。

## 输出
- 一版能过 30 秒 litmus 的目录骨架 + 对应的 `.arch/rules.yaml`(layers/dir_aliases/forbidden 落具体)。
- 结构性决策写 ADR(目录布局 / 分层 / 域边界);反模式(技术分层命名、顶层过宽、依赖方向反了)→ 重构,不放过。
