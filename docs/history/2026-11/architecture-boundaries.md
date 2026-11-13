# 架构边界整理：模型本地 HTTP 与面板设置同步

2026-11-11，在已发布 0.10.2 之后整理模型适配层，不改变输入行为、模型配置或公开接口。
工作区基线为 `f4e154ca176872c26dd68410d4833eb4256b1428`；这是新的未提交源码，
不属于已发布 `v0.10.2` 的安装包。

## 本轮边界

原 `src/model.rs` 同时承接配置、provider 编排、JSON 协议和本机 socket 请求。
`runtime` 与 `transport` 从根模块导入底层 HTTP，职责方向不清楚。现将本地 URL、
loopback 地址、HTTP 请求及解码下沉到私有 `src/model/local_http.rs`，两个调用者直接
依赖它。根模块保留配置、模型适配与公共重导出，共享取消检查仍供本地和云端复用。

八个迁移函数体与基线逐字节一致，只调整可见性；结构体字段的可见性限于 `model` 内部。
根文件从 2188 行变为 1962 行，原有测试保留在原模块，不能把行数减少等同于性能提升。
`model::is_local_llm_endpoint` 与 `languages::model`、`languages::llama` 的旧路径保持，
没有新增公共低层 HTTP API。原始测试正文、断言和期限不变，仅修正测试专用导入。

本机仍只接受 loopback HTTP：不做 DNS、代理或重定向，不读取云凭据；128 KiB 响应、
16 KiB header、单一总 deadline 和 50 ms 取消轮询保持。模型发现、完整草稿替换协议、
候选排序及补全/翻译各自的预算与成功条件没有并入传输层。

## 守卫与检查

新增一项结构守卫，先在原架构实际失败，再检查私有模块归属、公共重导出名单、内部 helper
可见性以及 runtime/transport 的直接依赖。它是针对当前源码布局的显式检查，不是 Rust AST
或一般 cfg 解释器；精确排除测试专用 imports，避免把测试 IO 当作生产层依赖。
原 29 项结构测试保留。新文件进入模型及发现实现登记，参与来源摘要。

独立临时 fixture 验证 22 个正确/错误布局向量，实际 22/22 通过：公开模块、额外公开
重导出、helper 扩权、反向根导入、leaf 混入云端或上层类型和根生产 IO 均被拒绝；
正确布局及测试专用 IO 导入放行。日志为
`/tmp/suzaku-local-http-guard-fixtures.vO7zWO/guard-fixtures.log`；不修改仓库或增加
Rust 测试数量，也不证明该源码守卫对任意 Rust 写法完备。

新增一项纯内存公共兼容测试，验证三个导出路径对 14 个 loopback、远端地址、注入和非法
URL 的判断一致，不发网络请求。独立机械审查确认八函数、共享取消函数及邻接测试保真。
审查中发现遗漏的 `http_request_with_cancel` 测试专用导入，已在编译前补齐，测试正文不动。

首轮全仓格式检查因新兼容测试的长字面量失败；补齐测试 import 后，第二轮又因导入排序
失败。两次命令都在格式阶段停止，没有开始 Clippy，不记作 Clippy 失败或通过。
修正后格式检查实际退出 0，严格 locked/offline、all-target/all-feature Clippy
加 `-D warnings` 实际退出 0（37.12 秒）。日志目录为
`/tmp/suzaku-architecture-local-http.lC3Cfg/`，保留 `architecture-before.log`、
`fmt.log`、`fmt-final.log`、`fmt-accepted.log` 和 `clippy.log`。

## 完整回归中断

冻结源码后执行完整 `cargo test --all-features --locked --offline -- --test-threads=1`，
两构建并行。原进程实际退出 **143**，没有完成整套回归；已完成的 274 个结果组累计
2746 通过、0 失败、33 项显式忽略、零筛除，只是中断前的部分结果。
当时进入 `japanese_terminal_n_progress`，第三项
`builtin_pending_n_sentence_survives_vowel_completion_and_backtracking` 没有结束。

日志 `rust-full.log` SHA256 为
`73076b28ffaeb931c3f6a1d0d5b98507281a19abd3f9a0c0fb813d300e271208`。
终止来源尚未确定，没有证据将其归因于本轮重构、OOM 或该用例断言；旧版本验收中出现过
同一集成目标中的另一项中断，但不能把相似现象当作共同原因。没有改动或放宽测试断言、期限与 CI 门禁。
后续独立范围检查不与这些部分结果相加，也不代替一次完整通过。

## 独立受影响链回归

同一冻结源码另跑一套明确范围：完整 library、panel、suzaku_tool，以及
`model_module_compat`、`model_provider_cli`、`model_setup_cli`、`prediction_protocol`、
`prediction_preservation`、`prediction_edit_conflicts`、`prediction_language_boundaries`、
`ime_prediction`、`model_japanese_classification` 和 `japanese_terminal_n_progress`。
原进程实际退出 **0**，13 个结果组共 **1108 通过、0 失败、29 项显式忽略、零筛除**。

新增三路径 endpoint 兼容用例和全部七项 terminal-n 测试均实际通过；后者包含全量中断时
正在执行的那项，但这不确定先前进程被终止的原因。日志 `rust-bounded.log` SHA256 为
`681d7a5b73eac363080cb19a9672b20c1d1b84dc0bea6fbd8ebd58e5af249067`。
这套结果不包含全仓所有集成目标或 Doc-tests，也不与中断的 274 个结果组拼接。

独立 `scripts/test-linux-ci.sh ui` 在私有 Xvfb / D-Bus / XDG 会话实际退出 **0**，
24 个唯一单项全部通过；不使用个人 IBus 会话或真实模型，不与上述 Rust 数量相加。
结构测试 30/30、文档测试 25/25、包文档测试 15/15、导航和 264 个包审计链接检查通过。
模型及发现的新路径参与来源摘要
`f6d361f12ff378af1287b1f3ef1464eaac97c72bf9effe494bd114fb8c33c154`；
所有旧 evidence 对象和成熟度原样保留，仅新增本轮证据及两处引用。

源码、测试、脚本和 Cargo 文件的冻结清单在各轮验证后再次逐文件核对；文档更新不改变
被测试源码。以上初轮检查没有完整全仓通过记录，不以 Clippy、相关回归或 UI 通过替代它。
后续独立完整回归见下一节；没有新的远端 CI、安装或发布验收。

## 独立完整回归补录（面板整理前）

信号诊断期间，使用同一冻结源码重新执行完整全功能、locked/offline、单线程 Rust 回归。
原进程实际退出 **0**，**330 个唯一结果组、3157 通过、0 失败、38 项显式忽略、零筛除**，
包含 Doc-tests；634 个源码、测试、脚本和 Cargo 文件的前后冻结校验全部通过。
日志 `/tmp/suzaku-sigterm-investigation.Y6wVf0/rust-full-traced.log` SHA256 为
`87bfd1b80a4bdfb43e5481bdaf6c1d829e66d613e3746eaf9a183c91eb0402f1`。
这是一轮独立完整通过，不与旧 274 组部分结果或 13 组专项拼接。

该轮加入信号跟踪，会改变时序，不作为性能测量；跟踪日志含部分子进程 detach，
没有 SIGTERM 行不证明整个进程树从未收到信号。独立诊断观察器也返回过 143；用户补充
当时出现模型 capacity 报错，但检查的客户端日志未找到可精确对齐的记录，因果仍未确认。
后续探针仅在主动清理时收到已知发送方的 SIGTERM，不是故障复现。原失败证据全部保留。

这份完整通过只验收本节所述、**面板后续整理前**的源码。它不是后续工作区的全仓通过、
个人安装、真实模型或远端交付证明；原 evidence 对象与成熟度均不覆盖改写。

## 面板设置同步边界

在上述完整回归之后，将控制器中的 `persist_display_settings` 和 `adopt_settings_from`
移入私有 `src/bin/panel/settings_sync.rs`，仍是同一 `PanelState` 的两个原方法，原签名和
`pub(super)` 可见性不变。控制器不再直接导入显示设置快照、应用和保存实现；指针参数归一化、
交互分派、候选提交及 GPU 呈现留在原处，`app_state` 不反向依赖 `PanelState`。

私有 `SettingsUpdatePlan` 只比较当前和传入值，保留原显示、字体、模型及快捷键四项判断。
快捷键比较提前到预测编辑代数复制之前，两者无数据依赖；其余写入、调用、条件与返回次序
机械核对保持原样。保存方法体逐字保持：设置窗口不直接写盘，保存使用实时 window scale，
失败反馈和场景失效保持；采用设置仍先应用、更新布局/装饰并保存，再复制导航状态，最后按条件
重建字体和重配模型。宿主设置 ACK 及在途补丁继续由 `prediction_settings` 独立持有。

新增 4 个纯内存测试，分别验证无变化、18 个持久字段、2 个快捷键值字段及 15 项瞬态状态。
18 字段通过完整解构要求未来扩展时显式补测；当前 `LlmModelPreset` 只有 `Configured`，
兼容名称是同值别名，因此该列验证不触发更新，不伪造不可构造的“仅模型枚举改变”。
新结构守卫及临时文本 fixture 验证 34 个布局向量（3 接受、31 拒绝）；原 30 项保留。
首次新守卫因叶子文件尚未创建而失败，随后扩展公开 helper 检查时出现一次正则分组拼写错误；
真实失败日志保留，修正只补齐分组，未放宽断言。守卫仅约束当前显式布局，不是通用 Rust AST。

本轮独立受影响链验证包含完整 panel target 及 8 个相关集成目标：`panel_layout_settings`、
`ime_panel_theme`、`ime_panel_input_bounds`、`ime_panel_content_fit`、`ime_engine_ui_alignment`、
`custom_shortcut_settings`、`native_settings_focus` 和 `settings_chain_audit`。
原进程实际退出 **0**，9 个结果组 **334 通过、0 失败、28 项既有忽略、零筛除**。
4 个新增纯内存测试全部通过；这不是修改后全仓 Rust 回归，也不与上一节的 3157 相加。

首轮私有 UI 实际退出 **101**：前 9 项通过，第 10 项
`settings_native_test::native_input_errors_are_visible_in_settings_and_compact_panel`
在 `PanelState::new` 创建图形设备时返回 `RequestDeviceError / Device(Lost)`，
尚未执行该项设置逻辑。保留 `ui.log`，不把它算作通过，也没有据此宣称已修复驱动问题。
相同源码和相同配置另启一次完整私有 UI 检查，**24 个唯一单项全部通过**；随后原有设置链
专项的 3 个唯一单项全部通过，整个命令原进程实际退出 **0**。不拼接首轮 9 项与重试片段。
两个入口仍使用私有 Xvfb / D-Bus / XDG；原测试正文、断言、时限和渲染后端选项未改。

格式检查、严格 all-target/all-feature locked/offline Clippy `-D warnings` 均实际退出 **0**，
后者 4.03 秒；结构测试 32/32、文档 25/25、包文档 15/15、338 个本地导航链接及
264 个打包审计链接检查通过。635 个源码、测试、脚本和 Cargo 文件冻结，清单 SHA256 为
`109563cf377376782e07d79e455f9e5f0078360ffca3dbd4bc0fe0775fc70330`。
日志目录 `/tmp/suzaku-architecture-settings.y42Qj5/`；受影响 Rust 日志 SHA256 为
`66866f6c839b78bcb22a097712679e5ec2fdcd8a118c0daf2c1a71d388dcab52`，
完整新 UI 日志为 `a4660814f8e9c5e4698ccbd0d19a707e9a77ac48761dd2fae7f41b64a9a85354`，
设置链日志为 `c3cddbd247401c47479039b8958b6708a1305b6879cfaefb10b496cc092efa4d`。
本轮不证明 Windows/macOS 原生运行、真实模型质量、个人安装或发布验收。

## 后续切口

面板的 `controller` 仍混合候选提交、GPU 渲染和外观交互，后续可继续分离交互与呈现，
保持宿主 ACK、窗口缩放与持久化次序。宿主可再将 C ABI facade 与会话所有权分开，
但必须保留同一个共享会话、原 Rust 导出路径和 C 符号。上述架构整理阶段没有改动渲染链和宿主 ABI；
后续独立故障修复见下一节。

模型 prompt/policy 仍间接依赖语言规则，不宣称已成为完全独立 crate。本轮不修改词库、
25 个可选包、SDK、默认预算、版本号或成熟度，不调用真实模型，不替换个人安装、操作
输入法服务或恢复定时任务。此前源码快照及完整 note 保留在
[基线张量](https://github.com/Team-silvortex/Suzaku-map/blob/f4e154ca176872c26dd68410d4833eb4256b1428/docs/functional-tensor.json)。

## 后续故障修复：表面恢复与设置重开

同日转入 bug 修复后，确认两个独立问题，不将它们当成前述设备初始化 `Device(Lost)` 的根因。

- `SurfaceError::Lost/Outdated` 原来调用同尺寸 `resize`，被正常的去重分支提前返回，既不重配
  表面也不请求新帧。新增独立 `recover_surface`，清除已呈现候选与指针场景，再重配并请求重绘；
  保留正常 resize 去重、窗口布局/焦点及其它错误的退出策略。主面板、设置和只读报告复用此分派。
- 新 Settings 构造虽然接收实时 chrome，随后仍加载旧磁盘偏好，可能丢掉保存失败后的预览，
  首次事件又将旧值倒灌主面板。现在只有 Main 启动读取磁盘显示偏好；辅助窗口继承实时快照。
  不改显示设置格式、宿主 ACK、模型预算或自动重试策略。

新增持久 CPU 回归直接提取当前生产 `resize`、恢复方法与错误分派，使用假表面/窗口编译执行。
同一回归在修前实际退出 101：Lost、Outdated 两项均没有配置表面；正常 resize 和其它错误策略
两项通过。修后实际退出 0，内部四项全过。它是针对当前源码布局的有限行为回归，不是通用 Rust
解析器或真实 GPU 故障注入；旧片段及日志保留于 `/tmp/suzaku-surface-recovery-Y43kuS/`。

设置新增独立私有 UI 必跑项：先保存 Suzaku，令私有配置目录不可写，再真实点击 Baihu、关闭并
重开设置、发送首次重绘，恢复权限后点击 Retry。修前实际退出 101，立即重开、首次事件和重试落盘
都错误回到 Suzaku；修后实际退出 0，三阶段全部保持 Baihu。保存失败提示仍可见，未自动写盘或发送
宿主补丁。证据 `/tmp/suzaku-settings-reopen.0caBzJ/{before.log,after.log}`；模型关闭、无个人服务。
首次构建缺少新增测试局部 import 的真实编译失败也保留，没有修改旧断言或期限。

此修复不重建丢失的图形设备，不宣称解决设备创建失败、驱动重置或所有 `SurfaceError::Other`。

修后冻结源码，独立运行完整 panel、前节 8 个相关集成目标，另加 `ime_panel_drag_regions` 和
`panel_surface_recovery`：11 个唯一结果组 **338 通过、0 失败、29 项显式忽略、零筛除**。
之后完整私有 UI **24 个唯一单项通过**，设置链 **4 个唯一单项通过**，原进程实际退出 **0**。
新增 opt-in 设置项已包含在后者中，不与普通 Rust 的忽略数或单独 TDD 结果叠加。
格式及严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（6.40 秒）。
检查以单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap 的专属 scope 运行；
不改旧入口、断言、超时或后端选择，不是新全仓、真实 GPU 故障、个人安装或远端 CI 验收。

636 个源码、测试、脚本与 Cargo 文件前后冻结一致，清单 SHA256 为
`5fc1e585f06b00f4b23853eb4c13621b8ca20a6e5a5d52f3698746e573e8fc41`。
相对前节只有 5 个预期旧文件改变，新增一个 CPU 集成测试；模型及词库等其余文件保持。
整合日志目录 `/tmp/suzaku-bugfix-recovery.jzKRVB/`，Rust / UI / 设置链日志 SHA256 分别为
`5f40ee55ed7169f90a6b6e3fb88efdfb36d26d08d202b5d0d2b92a6a0878ed2b`、
`acbe81fe6d071d1f8be0be07714b65cbc74cd3da5fbc580b5e966468318d2853`、
`672b5ac0d316df50f9a3cd889ff3f3cc32c489806a71630f7073752e8017e7a3`。
旧历史运行记录及成熟度不改，未提交、推送、升版、调用模型或更改个人输入法服务/配置。

## 辅助窗口失败恢复与报告内的新错误

同日继续抓虫，确认辅助窗口三处构造都用 `expect` 处理 OS 创建或 GPU 初始化返回的错误。
设置入口还会在成功前暂停原生键盘队列；其 `blocked` 状态不会随着关闭设置自动恢复。
现在统一返回带创建/初始化上下文的 `Result`，设置只在成功创建或复用后暂停原生队列并同步
输入权限。失败清除打开请求，提示错误，普通重绘不再重复创建；明确再次打开仍可重试。
报告构造失败只记录反馈及诊断，不展开、显示或聚焦主面板，也不改变原生输入所有权。

新增 CPU 集成回归执行实际生产入口、主窗事件协调及 gear/快捷键请求体，替代窗口/GPU 操作。
持续创建/初始化失败、三次失败后的普通重绘和明确重试、原有 blocked 状态、隐藏折叠报告及
已有窗口复用均覆盖。修前外层实际退出 101、内部 7 项全失败；修后外层退出 0、内部 7 项全过。
它证明有限控制流而非真实 OS/GPU 故障；普通 Cargo 只计外层 1 项，不将子进程结果再加总。
日志及冻结旧片段位于 `/tmp/suzaku-bugfix-auxiliary.msxQUy/`，包括
`auxiliary-before-formatted.log`、`auxiliary-after-formatted.log` 和 `before/`。

聚焦报告收到 `InputMethodError` 时，旧代码将错误写进报告禁止绘制的普通设置反馈；临时报告
关闭后提示直接消失。现在报告独立保存新错误，原始及转义后提示各不超过 4096 字节，仅在当前
正文区域显示；不读取、覆盖或推进原设置反馈。报告期间只推进独立 180 帧提示寿命，仍禁止
语音、翻译等编辑轮询，报告快照、页码、原始场景和已按下按钮的身份不变。
新增 3 项纯测试覆盖 Unicode 安全截断、控制符可见化、多语言不误译 opaque 参数、寿命与
无效正文区域；旧报告私有 UI 必跑项追加两条路径，保留所有旧断言并实际呈现新提示。
同一 exact 项修前实际退出 101（overlay 0 而非 1）；修后完整项和 3 项纯测试均实际退出 0。
前后冻结通过，原始日志在 `/tmp/suzaku-report-feedback.ZoJYJB/`。

整合时发现明确 `OpenSettings` 也会被 180 ms 悬浮球折叠防抖吞掉。保留旧设置链断言，追加
刚折叠便复用/新建设置的私有 UI 场景；修前完整设置链实际退出 1，新断言在复用场景失败。
现在只有明确打开设置时清除该防抖，普通 bubble 点击策略不变；CPU fixture 仅同步新增的
字段占位，原 7 项断言不变。修前日志 `settings-debounce-before.log` 保留。

本轮只承诺原生排队编辑不被失败请求提前锁住、已提交草稿不被修改，不宣称本地尚未提交的
IME preedit 跨失败保留：gear 触发设置时的旧权限清理仍会结束这类预编辑。主窗口初次创建的
错误退出、底层 `DeviceLost` 重建和前述偶发初始化失败根因也不在本轮验收范围。

以上修正冻结后完整运行 panel 及 11 个相关集成目标，原进程实际退出 **0**：12 个唯一结果组
**342 通过、0 失败、29 项显式忽略、零筛除**；新增 CPU fixture 只计外层一次。
同一冻结源码随后完整私有 UI **24 个唯一单项通过**，强化后的设置链 **4 个唯一单项通过**。
格式及严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（5.42 秒），
保持单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap；不拼接前述 TDD 或旧轮结果。
这不是全仓、实际设备故障注入、物理键盘、个人安装、真实模型或远端 CI 验收。

637 个源码、测试、脚本与 Cargo 文件前后冻结一致，清单 SHA256 为
`1819f67578c59294deee6705426b3f7fcb70a4c747fc76f0dc80fb2121a1802b`。
较上一轮仅 7 个预期旧文件与 1 个新集成测试变化，其余模型、词库及包/SDK 等字节保持。
整合日志在 `/tmp/suzaku-bugfix-auxiliary.msxQUy/`，Rust / UI / 设置链 SHA256 分别为
`d3b58a9c91c58aee88d6904267134d91ba2064a17707f2abd0d6930ea98bec4f`、
`087528505d372eff4dcc3934ba6587553b5337e640c84cf5ef7b0e44e18db09b`、
`9c65044d379c47b9802fcf64a67dbe75fc8453f86b4f846ed7b7605ab2156a3f`。
189 个既有 evidence 与全部 79 项评分/限制保持；未提交、推送、升版或恢复自动化。

## 输入交接：候选呈现、报告转设置与预编辑

继续抓虫时，发现原生候选滚轮会主动测量新场景，在新 revision、context 或窗口尺寸尚未
呈现时仍能发送翻页。现在只使用最近已呈现且身份/几何仍有效的候选区域，复用键盘候选的
呈现门槛。零值、非有限值、滚动方向及事件阶段的原断言保留；旧正向分页 fixture 也必须
先真实呈现，不能仅测量布局便把它当作可交互帧。

原 mandatory 私有 UI 项新增三个边界的呈现前/后六个向量；修前整项实际退出 101，
三个未呈现场景都错误发送 N12。失败及冻结清单保留于
`/tmp/suzaku-candidate-wheel.emPq8E/`，不将正确呈现后的翻页取消或禁用。

主面板 gear、F1 和 Ctrl+, 另用一次性显式请求协调设置窗口。报告期间普通重绘仍保留报告，
明确打开才结束只读报告并复用设置，明确关闭则关闭报告或设置；复用时请求系统焦点。
请求只在成功进入输入分派后重置，拒绝重入不会消费外层请求或清理外层输入状态。

打开设置的本地 IME 权限同步延后至辅助构造结果：失败保留未提交 preedit，成功才结束原
输入上下文，不靠复制/恢复预编辑文本。上一节关于 gear 仍清 preedit 的限制是当时源码的
历史结论，不改写旧证据；本节新增独立修正。正在组字时 F1/Ctrl+, 本来就不发设置请求，
继续保持该保护，不能伪造“组字快捷键创建失败”作为验收路径。

CPU fixture 提取实际入口/协调，新增报告切换、gear 的创建/初始化失败与成功、拒绝重入
三项；保留原七项。冻结旧生产源码实际退出 101，内层 8 过/2 失败；冻结修后源码实际退出 0，
内层 10 过。假平台及权限状态只证明有限控制流，普通 Cargo 仅计外层一次。日志及旧片段在
`/tmp/suzaku-settings-transition.YOoRhw/`。真实私有窗口项另补已呈现 gear 的 MouseInput
经过生产 handler 的延后检查，构造结果明确为合成；F1/Ctrl+, 使用私有 X11 按键事件验证
组字保护。沿用原步骤、断言和 20 秒内限，不将两层夹具等同于真实 OS/GPU 故障注入。

系统焦点只调用标准窗口请求，未使用特殊抓焦代替设置窗口机制；无窗口管理器的 Xvfb 不
证明真实桌面会接受焦点。设置失败也不回滚原来明确显示/展开主面板的语义，不承诺底层
DeviceLost、个人 IBus、物理输入或实际模型验收。

修后整合命令原进程实际退出 **0**：完整 panel 与 11 个相关集成目标，12 个唯一结果组
**342 通过、0 失败、29 项显式忽略、零筛除**；另行完整私有 UI **24 个唯一单项**和
设置链 **4 个唯一单项**全部通过，不拼接 TDD/CPU 内层或旧轮数量。格式与严格
all-target/all-feature locked/offline Clippy `-D warnings` 通过（5.39 秒）；单构建任务、
2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap。独立键盘 exact 也实际退出 0；其首次
启动因 systemd 工作目录回到 home 而未找到 Cargo.toml，单独留档，不算测试失败或通过。

637 个源码/测试/脚本/Cargo 文件前后冻结一致，较前轮仅 8 个预期旧文件变化，无新增源码。
冻结清单 SHA256 为 `9b2c3d4c7ac64c35edff76d8bec3b2407b4970cf0f80eeda25483f98cdec61b0`。
整合日志 `/tmp/suzaku-bugfix-input-handoff.T7US1s/`；Rust / UI / 设置链 SHA256 分别为
`4db02d6a051a2e129227b662d38eb4f04f8bf735dd36b7d5073c01679285ca7b`、
`699ce039faa0ba52616f251668329f7c71c9fa70604740091bb9cb46fcb6c6b5`、
`97556abb7b065038f1e53499906e8869a1cdd559792ef4ebeeceb9ad6536b25f`。
190 个旧 evidence 与 79 项评分/限制原样保留，不把本轮当作全仓回归或新的性能承诺。
未提交、推送、升版、重装、调用模型、更改个人服务/配置或恢复定时任务。
