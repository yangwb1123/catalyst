# Skill: project-reorganization

> 根目录/结构失控 → 按领域归位。Stop sprawl, file by domain.

## 目标 (Goal)
把散落的文件按**领域 (domain)** 收纳进 `src/<domain>/`,使根目录文件 ≤ `max_root_files`(15),结构可导航。

## 触发条件 (Triggers)
- 根目录文件数 > `harness/policies.yml: max_root_files`(默认 15)。
- gate 报 `root has N files (max 15)`。
- 文件按类型而非领域堆放,或同领域代码散在多处。

## 步骤 (Steps)
1. **冻结**:停止在根目录新增文件;先治理再继续。
2. **盘点 (inventory)**:列出根目录每个文件 → 标注其所属领域 / 层 / 类型。
3. **划领域 (group by domain)**:按业务能力切分(如 `auth / billing / catalog`),非按 `controllers/ models/`。
4. **归位 (move)**:迁入 `src/<domain>/`;层内再按 clean-architecture 分 `presentation/application/domain/infrastructure`。
5. **更新引用 (rewire)**:修正所有 import 路径、路径别名、构建/入口配置;保留必要的 barrel re-export。
6. **保留根目录白名单**:仅留入口/配置/元文件(README、package.json、tsconfig、lockfile 等)。
7. **gate 复检**:`node harness/gate.mjs` + 构建 + 测试,必须全绿;循环依赖须为 0。

## 输出 (Output)
- 迁移映射表(`旧路径 → 新路径`)。
- 新目录树(`src/<domain>/...`)。
- 根目录剩余文件数(≤ 15)。
- gate + build + test verdict。
