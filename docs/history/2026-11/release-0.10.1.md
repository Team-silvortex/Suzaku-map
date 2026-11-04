# 0.10.1 发布准备与验收

2026-11-04 继续未完成的 0.10.1／build 51。变化见[版本说明](../../releases/0.10.1.md)，
当前下载在新版实际发布前仍指向已验收的 0.10.0，不恢复已取消的定时任务。
最新结果见末节：0.10.1 已发布；下文的待验收、权限阻断和旧下载入口均为当时阶段状态。

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

## 最终验收与权限阻断

2026-11-04，修正提交 `282e3cd5f272e71ab2624f89d801544479c53659` 已推送，
提交时间戳 `1793776334`。[CI 41790632168](https://github.com/Team-silvortex/Suzaku-map/actions/runs/41790632168)
的 Linux 核心、应用、安装包、secrets、format、Windows/macOS 编译与最终聚合八门均为 success。
旧 `2330da6` 失败保留，没有重跑旧运行或放宽守卫。

新完整本地 Rust 实际 exit 0：3,037 通过、0 失败、34 项显式忽略，323 个唯一结果组，零筛除并完成 Doc-tests。
该进程从 `2330da6` 开始；到修正提交期间 Rust、fixtures 和版本字段字节未改，
而准确修正提交另有独立完整远端验收。私有 UI 21/21 与严格 all-feature/all-target Clippy
也分别实际 exit 0，Clippy 43.07 秒。原 `Pending` 偶发失败的原因仍未证明，不称本次已修复它。

只下载 accepted artifact `13304544012`，
`suzaku-linux-X64-282e3cd5f272e71ab2624f89d801544479c53659`；
ZIP SHA-256 为 `45cf210d0fa71edf2a33d8871947559cb3200a55eb6625c93eb903b0aedaf355`。
恰好四资产的外部校验、788 条内部校验、安全路径／类型／权限、tar/deb 原始 manifest、
四程序和 1,178 条共享资源记录一致，25 JSON 包及 README、SDK 三文件与准确源提交字节一致。
manifest 绑定 0.10.1、上述 SHA／时间、干净源码、x86_64 GNU/Linux、glibc 2.39、
不附带模型或用户数据。独立检查仅流式解析，不执行包内程序；干净容器安装／升级／卸载由远端门禁通过。

注释标签 `v0.10.1` 已创建并推送；本地与远端对象同为
`ccc932c73ba964ae84a4ab79a51d05d42be0748c`，peeled SHA 为上述验收提交。
随后用准确提交的版本说明、固定提交链接和验收摘要创建 draft／prerelease／非 latest 时，
`POST https://api.github.com/repos/Team-silvortex/Suzaku-map/releases` 返回
`403 Resource not accessible by personal access token`，命令 exit 1；后续按 tag 读取为 404。
未上传资产，未发布 Release，不改用其他凭据、不重试创建、不移动标签。当前下载继续指向 0.10.0。

全部新记录保存在 `/tmp/suzaku-accept-0.10.1.wrftDo/`：
`release-lock.json`、`ci-evaluation.json`、`artifacts.json`、`unpack-receipt.json`、
`package-acceptance.json`、`local-validation.json`、`release-body.md` 和 `publication-blocked.json`。
四个已验收文件在其 `assets/`；原 Rust 日志仍在 `/tmp/suzaku-release-0.10.1.cer53x/rust-full-20261104.log`。
49 项包校验器和 42 项 ZIP 来源／安全的合成反例测试另行通过，不把它们算作真实安装或模型能力验收。

权限补齐后须先重新核对同一标签、准确 CI、artifact 未过期与资产字节，以及是否出现已有草稿／冲突资产；
不得把新 main 的文档提交替换为验收源。上传四资产后重新下载比较实际字节，才可发布。
未更换个人安装、服务／配置、Git／SSH 设置，也没有恢复自动化。

## 发布完成

2026-11-04，用户明确指定本机系统钥匙串中已有的组织凭据供本次发布使用。
只在发布子进程内读取该精确条目并注入 `GH_TOKEN`，不输出凭据、不落盘、不替换既有 gh 登录或修改条目。
重新核对原准确提交的八门 CI、未过期 accepted artifact／ZIP 摘要、四个包文件及未移动的注释标签后，
创建草稿 Release `433687763`。GitHub 的草稿按 tag 读取返回 404；通过列表定位准确 ID 后核对目标和说明，
没有因该 404 重复创建。四资产上传后全部重新下载、逐块字节比较并独立重算哈希和尺寸，通过独立复核。

随后明确指定 prerelease、`latest=false` 发布，最终 metadata 确认为 `draft=false`、`prerelease=true`，
发布时间 `2026-11-04T08:42:02Z`，目标仍为 `282e3cd5f272e71ab2624f89d801544479c53659`。
完整四资产 ID／尺寸／哈希和平台范围仅集中在[发布回执](../../releases/0.10.1.md#publication-receipt-2026-11-04)。
此前 CLI PAT 的两次真实 403 及原 `publication-blocked.json` 保留，不能改写为首次即成功；
新证据为同一任务目录的 `ci-before-publish.json`、`package-acceptance-keyring.json`、
`release-draft-by-id.json`、`release-uploaded.json`、`uploaded-receipt.json` 和 `release-published.json`。

后续文档提交只更新回执、下载入口与交付证据，不替换已验收源码或移动标签。
个人安装、输入法服务／配置、既有 gh／SSH／GitHub App 授权及取消的自动化均未改动。
