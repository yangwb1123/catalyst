# 备份与恢复

## 备份(自动)

不可逆 schema 迁移前,`SqliteHubStore::open` 自动快照现有 hub 到
`state/backups/hub-v<N>-before-upgrade-<ts>.sqlite3`(N = 迁移前版本)。
新建库(version 0)不备份。这是 Stage-06 生产就绪条件
(backup-before-upgrade)的实现;降级→迁移→备份验证测试在
`sqlite_hub_backup.rs`。

手动备份:

```sh
cp .forge/state/hub.sqlite3 backups/manual-$(date +%s).sqlite3
```

注意 WAL:备份前应先 checkpoint(`PRAGMA wal_checkpoint(TRUNCATE)`)或在
无写入窗口内复制 `hub.sqlite3` + `hub.sqlite3-wal`。

## 恢复

1. 停止所有 forge 进程。
2. 用备份替换 `hub.sqlite3`(及 `-wal`/`-shm`,若存在)。
3. 启动 forge —— 旧版本库会自动迁移到当前 schema(迁移本身自动备份,
   恢复现场可再次回退)。

## 验证

`forge accept` 的 backup 测试覆盖"降级→迁移→备份存在且版本正确"。
生产恢复演练(完整 restore drill)记录为后续(部署层)。
