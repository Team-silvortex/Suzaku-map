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

## 首候选 CI 与守卫兼容修正

首候选 `2330da6d1241c551ce79e2291d2a5edf416318b5` 已推送。
[CI 41789261683](https://github.com/Team-silvortex/Suzaku-map/actions/runs/41789261683)
的 Linux job `126438563597` 在 `Test controlled native model replies and unchanged timing guards` 失败：
新增两个 UI 精确入口未被旧 `project_runner` 识别，历史摘要由预期 `521c5da0…` 变为 `c90aa61e…`。
本地正式 AST 入口复现同一失败；这与前述 Rust `Pending` 是两个独立问题。

修正只把两条精确追加的 UI 行在原相邻入口之间逆回；不改旧摘要、原九项核心守卫、
原 53 组回退数据、物理断言、90 秒期限或测试执行清单。
另增两项独立测试，强制新入口各存在一次并保持次序，拒绝单项删除、重复、更名、换序、
延长期限和去掉 `--exact`。不把历史投影用于运行时执行。
原核心分区守卫加新测试为十一项，通过；正式模型守卫二十项也通过，子进程结果不重复计数。
语言、数字键、按住切换与词库分区的正式 AST 入口也分别通过。

原失败日志、修正前后本地日志在 `/tmp/suzaku-publish-0.10.1.MFreug/`，
包括 `ci-linux-job.log`、`model-guard-local.log`、`model-guard-fixed.log` 和 `ui-guard-core-fixed.log`。
失败提交不重跑、不验包；修正提交必须独立通过新的完整 CI 后才可发包。
仓库迁移至组织后，已核对相同 repository ID `1160702978` 与既有 0.10.0 Release ID `426593249`；
未改 Git/SSH 配置。当前尚未创建版本标签或发布安装包，不操作个人安装、输入法服务或配置。
