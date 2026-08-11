# 备份与恢复

## 备份(自动)

不可逆 schema 迁移前,`SqliteHubStore::open` 自动快照现有 hub 到
`state/backups/hub-v<N>-before-upgrade-<ts>.sqlite3`(N = 迁移前版本)。
快照由同一 SQLite 连接执行 `VACUUM INTO`，是包含迁移前已提交 WAL pages 的
一致单文件数据库，而不是对 main file 的裸复制。新建库(version 0)不备份。这是 Stage-06 生产就绪条件
(backup-before-upgrade)的实现;降级→迁移→备份验证测试在
`sqlite_hub_backup.rs`。

手动备份必须使用 SQLite Backup API、`VACUUM INTO`，或先停止全部 writer 后做离线复制。例如在确认没有并发 writer 时：

```sh
sqlite3 .forge/state/hub.sqlite3 \
  "VACUUM INTO 'backups/manual.sqlite3'"
```

禁止在 writer 活跃时用 `cp` 单独复制 main database，或依次复制 main/WAL/SHM；这些文件并非一个原子快照，顺序复制会产生竞态。

## 恢复

1. 停止所有 forge 进程。
2. 保存故障现场后，按受控恢复流程清理目标位置遗留的 `hub.sqlite3-wal`、`hub.sqlite3-shm` 和 rollback journal，再用一致的单文件快照替换
   `hub.sqlite3`。不要把来源环境的 sidecar 与快照拼接。
3. 先以受控、允许迁移的 mutation-capable Forge 启动路径打开恢复库；旧版本库会自动迁移到当前 schema（迁移本身自动备份，
   恢复现场可再次回退）。普通 journal read 和 semantic read 都不会迁移旧库，不能把只读检查误当成恢复升级步骤。

## 验证

`forge accept` 的 backup 测试覆盖"降级→迁移→备份存在且版本正确"，并验证 hot-WAL 中已提交的 page 会进入一致的 pre-upgrade snapshot。
生产恢复演练(完整 restore drill)记录为后续(部署层)。
