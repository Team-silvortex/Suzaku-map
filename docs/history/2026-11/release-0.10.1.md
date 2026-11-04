# 0.10.1 发布准备与验收

2026-11-04 继续未完成的 0.10.1／build 51。变化见[版本说明](../../releases/0.10.1.md)，
当前下载在新版实际发布前仍指向已验收的 0.10.0，不恢复已取消的定时任务。

## 已保留的失败

先前完整 Rust 进程实际退出码为 101，失败为
`daily_household_english::household_local_fallback_survives_missing_failed_timed_out_and_empty_models`，
位置 `tests/daily_household_english.rs:335`：状态仍为 `Pending`，预期为 `Unavailable`。
完整日志 `/tmp/suzaku-release-0.10.1.cer53x/rust-full.log` 保留，不把之前通过的目标算作整套通过。

该测试、预测 worker、核心引擎及合成提供器相对 HEAD 未改变。
单项精确复现一次通过，耗时 5.00 秒，日志为同目录的 `household-repro-20261104.log`；
它只证明没有稳定复现，不证明原失败根因或修复。等待循环可能在末次观察后跨过期限，
但原日志没有提供器完成时间，不能据此认定原因，也不能把它说成网络故障。
原断言、两秒期限及预测代码保持不变，不用延时、终点补读或重复尝试放宽要求。

## 发布门槛

五权威文件的八个版本字段同步为 0.10.1／51；其他冻结源码和资源保留。
需重新完成独立完整 Rust、严格 Clippy 和私有 UI 验收；原四轮界面记录不冒充新版完整验收。
随后仍需准确提交的八项远端 CI 全部成功，只验该运行的 accepted artifact，
核对四资产校验和、安全归档、相同 manifest、准确源提交和时间、25 包／SDK字节及干净容器门禁。
草稿上传后重新下载比较实际字节，才可发布为 Linux Alpha prerelease、非 latest stable。

当前尚未创建版本标签或发布安装包，不操作个人安装、输入法服务或配置。
