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

## 2026-11-13：快捷键持键、候选精确身份与小报告提示

本批从已推送的 `d97fd1cbc481f4649a4a3db77a54e6e0c8d29b8c` 继续抓虫，
不升版或自动提交推送。三项独立问题如下，原有报告快照、分页、按钮身份及设置保存语义不改。

- 快捷键录制原来只保存一个被消费的物理键。按住无修饰 A（绑定被拒绝），再按 Esc 取消
  或录入有效 Ctrl+K，会覆盖 A 的持键记录；之后 A 的连发可落入设置搜索。现在保留有限
  `KeyCode` 集合，逐键等待真实释放；未知键、修饰键不入集，IME 清理不解除持键，失焦和
  关闭仍清空全部。三项新增纯回归修前实际 exit 101（同一过滤运行 15 过、3 失败），
  修后实际 exit 0（18/18）；旧 tests 模块去掉三个新增块后逐字一致，原私有键盘项的六个
  按键事件与 20 秒期限不变。纯回归走实际 capture/hold，不冒充本批新的物理键盘验收。
- 句子卡片已经携带引擎源索引，却再次按忽略大小写、空白及末尾标点的标签查找首个匹配，
  点击第二项 `hello world!` 实际发送了第一项 `Hello world.`。修前私有已呈现按下/释放
  加合成 socket ACK 的真实发送回归 exit 101；现在源索引和对应精确标签必须一致，同名
  caption 的不同 payload 也不改发其它项。未知、越界、缺映射或陈旧标签不猜 selected/last。
  交叉审查另发现 Enter 会在上述拒绝后兜底发送默认项，追加独立拒绝回归后也实际 exit 101；
  仅独立面板撤掉该兜底，原生宿主动作和显式已选候选路径保留。
- English/Large 的合法短报告窗 640×310，正文仍有一行空间，但设置弹层式的双重留白令
  新输入法错误完全没有 overlay，随后静默到期；修前实际窗口回归 exit 101（0 对预期 1）。
  640×320 的相邻窄带实际把同字符高度从正文 21.84px 压成约 3.98px；新增真实正文/通知
  字形及栅格 ink 参照回归也实际 exit 101，不能仅以非空认定可读。报告通知改为
  正文区域专用布局并使用报告自身字号，仍裁剪在正文内，不遮警告、关闭或翻页控件；
  不修改通用 tooltip/设置提示的布局契约。小于完整一行的无效区域仍不承诺可读。

日志保留在 `/tmp/suzaku-bugfix-input-identity.l74xKd/`。首次候选 green 构建发现旧格式化
helper 测试仍引用被移除的方法，保留为 test-only 历史回归后不再让它参与发送身份判断。
首轮严格 Clippy 的新增测试迭代写法也真实失败；只改成 enumerate，不变场景或业务断言。
中间整合与后续字体边界回归分开保存，不能相加为最终通过数；最终独立验收结果见下段。

最终同一冻结源码的独立流水线原进程实际 **exit 0**：完整 panel 与 11 个相关集成目标，
**12 个唯一结果组，345 通过、0 失败、29 项原有显式忽略、零筛除**；完整私有 UI
**24 个唯一单项**与设置链 **4 个唯一单项**另行通过，各阶段真实 exit 0。
包含最终 Enter 拒绝路径、640×310/640×320 字号边界与 400×220 完整警告场景；
不将上述定向/TDD、中间整合或旧版本结果加进本轮数量。格式与严格 all-target/all-feature
locked/offline Clippy `-D warnings` 通过（6.05 秒），单构建任务、2 GiB MemoryHigh /
3 GiB MemoryMax / 无 swap。不是全仓 Rust/Doc-tests、个人 IBus、物理按键、实模或远端 CI 验收。

693 个源码/测试/脚本/Cargo/Android 文件运行前后逐字节一致，冻结清单 SHA256 为
`634e985cb89732f769feaeb42a53b5a155961974a99a9c8b6f118e5f487ea2b0`。
相对上版仅五个 panel 生产/测试文件改变；词库、25 包、SDK、模型协议、宿主、依赖与版本
保持。原 192 项 evidence 及 79 项评分/限制保留；本批只增加一个证据及相关引用，不提高成熟度。
本轮源码尚未提交推送，不重装、不改个人服务/配置、不恢复自动化。

## 继续抓虫：句卡手势身份与语音停止边界

继续以上未提交快照，不回退上一批修复。句卡的当前索引/精确标签已经受保护，但按住手势
和长句首击预览仍只记槽位：公开语言插件可在相同标签下替换 payload，刷新并实际重绘后，
松开旧手势仍发送新内容；长句 A 的预览确认也会被同槽位 B 继承。

新增私有鼠标/触摸回归各覆盖 payload、草稿、语言变化及完全不变，包含真正呈现、按下、
刷新、再呈现和释放，以及合成 socket ACK；新按下仍能正常发送当前内容，重复释放不能重发。
原实现实际 exit 101，首个鼠标 payload 向量错误发送；先只修按住身份后，八个持键向量
通过才到预览首个向量，再实际 exit 101，证实两个边界分开存在。随后同一冻结测试独立
exit 0（4.99 秒）。内部向量不重复加入 Cargo 总数，旧手势期限、断言和 positive 保留。
新增私有 `LocalSentenceFrame` 缓存显示/源草稿、语言和最多四个可见句卡的源索引、标签与
原文；身份变化清除按住和句卡预览许可，完全相同或仅分数变化不因此失效。经过刷新发生的
A→B→A 不会恢复旧许可。不改原生 revision 协议、词芯预览策略或手写。

语音普通 Stop 未读取后端已到达但尚未 poll 的 partial，插入还会先快照旧文本才停止。
因此 Stop/失焦可暂留旧稿，KeyI 的生产 callee 可采用旧稿甚至因可见稿为空而不停止；
不能笼统称后端永久丢失，因为未消费的 partial 还可能流入后续 capture。模式按钮转到
翻译、手写或键盘时，也没有停止 Listening；返回语音后旧计时可能自动采用旧 capture。
新增合成 C 桥 consume-once partial 的私有回归，覆盖 Stop/失焦/KeyI callee（非物理按键）、
可见空稿、Clear、三种模式离开/返回和旧原生 ACK 不清新 generation 源。旧实现实际
exit 101，24 项条件失败，只计一个失败测试，不当作 24 个独立产品问题。Digit1/3 的
额外结构守卫只证明调用点接线，不代替真实键盘事件验收。

普通停止现在仅在原先 Listening 时，在停止后读取一次已到达 partial，再重置捕获进度；
Idle 停止不 poll，避免旧队列内容复活。插入先停止、再快照最新文本，空稿也先结束监听。
模式按钮及 Digit1/3 的工具 fallback 在离开语音模式前执行同一停止入口；Clear、原生
ACK 的 generation/原文校验和目标切换保护保留，不新增后台启动、等待、重试或模型调用。
同一冻结语音回归修后独立实际 exit 0（1 项、0.70 秒），不承诺未来异步 final 或真实 ASR。

日志目录 `/tmp/suzaku-bugfix-held-candidate.Zh7wbt/`。首个候选测试的枚举类型限定路径
写错导致构建 exit 101，修正测试引用后才取得行为 RED；首个语音命令漏掉嵌套模块前缀，
虽然 exit 0 但零测试，明确不接受并保留日志。临时 runner 补齐精确测试必须存在的守卫及
标准语音 QA 环境后，才取得真实失败。未以编译失败、零测试或定向结果替代终局整合验收。

最终独立流水线原进程实际 **exit 0**：完整 panel 与 11 个相关集成目标共 **12 个唯一结果
组，345 通过、0 失败、29 项原有显式忽略、零筛除**；私有 UI **24 个唯一单项**及设置链
**4 个唯一单项**另行通过，各阶段真实 exit 0。格式与严格 all-target/all-feature
locked/offline Clippy `-D warnings` 通过（6.01 秒）。单构建任务及 2 GiB MemoryHigh /
3 GiB MemoryMax / 无 swap，未将内部向量、定向运行、零测试或上轮结果相加。

693 个源码/测试/脚本/Cargo/Android 文件运行前后保真，相对本轮起点仅七个 panel 生产/
测试文件变化；冻结清单 SHA256 为
`85e5ed786bcfb3f702b7b511c4d9f2c3f265a6dd262aaeb819d20a0b074e3ab6`。
最终 Rust / UI / 设置日志 SHA256 分别为
`106d3f7cf47cde5de11c3cc86e649d344dc5b78a3b7416b9ef2af4bfa9a20d55`、
`f21696efbb86a0de976f692041e4b90166c104a44e1dbd496497e8f9affc0a1a`、
`b47d2d313a37624f5e3fd965df9943826b03937836d052cef29cd8ebb18e9411`。
原 193 项 evidence、79 项评分/限制保留，只加本轮证据与两个引用，不提高成熟度。
不是全仓 Rust/Doc-tests、真实麦克风/异步 final、物理键盘、实模、个人安装或远端 CI 验收；
词库、25 包、SDK、模型预算、宿主、依赖及版本不变。未提交推送、重装或更改个人服务/
配置，不恢复定时任务。

## 继续抓虫：词芯预览、手写撤销与跨输入框确认

继续上一批未提交快照，保留既有修复。本轮三条链使用私有面板状态和合成数据复现，
候选手势另经真实呈现、按下和释放；不调用模型或修改个人输入法。

- 长词芯 A 的首击滚动预览原来只绑定槽位；公开 English 插件刷新同槽 B 后，B 的首击
  错误直接采用。旧实现真实 exit 101；现在覆盖旧补全缓存前比较该槽完整 edit，以及
  显示草稿、源草稿和语言，只撤销变化项的词芯预览许可。完全相同或仅无关句卡变化
  不清许可；不改上批句卡身份策略。冻结回归覆盖鼠标/触摸各四种内容、草稿、语言、
  完全不变边界，含实际截断标志、首击预览、次击采用、重复释放和 Back 撤销。
  修后同一 exact 实际 exit 0（8.36 秒）；八个内部向量不重复计入 Cargo 总数。
- 按住手写候选 `t` 时调用 KeyZ 的生产 undo callee，再识别成 `T_T` 并重绘，释放旧手势
  实际采用了新源。旧实现真实 exit 101；现在重新识别或 Clear 时只取消手写候选按住/
  touch 待释放和该工具的预览，不取消画布笔画或其它工具手势。同标签但新笔画也换代；
  没变的源仍正常采用。私有本地/原生、鼠标/触摸及三种源边界通过，同一 exact 实际
  exit 0（7.45 秒），原生 ACK 和 generation 保护保持。首次测试遗漏草稿光标末尾设置，
  先在正常点击对照失败；该夹具错误单独保留，修正夹具后才取得行为 RED。
- A 输入框的翻译替换等待 ACK 时切到 B，B 的首个 Space/Backspace 会把 A 的 replacement
  挪作 B 队列基底，点击本地恢复实为 `translated Ax` 而不是 `new fieldx`，真实 exit 101。
  先限制 replacement 必须匹配当前 host/context/language 后，同一冻结回归又在旧 A
  ACK 拒绝时实际 exit 101：旧失败无条件阻塞 B。最终 PendingAction 单独保留 context/
  language（host 沿用 id），失败只阻塞同目标队列，不依赖可能已断开的当前 frame。
  修后同一 exact 实际 exit 0（3.73 秒）；覆盖 B 的两种语义键和恢复前 ACK、成功、拒绝、
  断开四分支，并保留 A 同目标两种 ACK/frame 顺序的正常对照。语义结果来自合成 B 宿主
  回包，不预测 Space/Backspace，也不把恢复文本重放到外部应用。

日志 `/tmp/suzaku-bugfix-tool-previews.iGiXgt/`。新增测试均接入三个既有 mandatory exact，
不增顶层测试、不改旧断言/期限；KeyZ 为生产 callee 而非物理按键验收，合成翻译非云模型，
原生队列夹具不等于个人 IBus 或应用兼容验收。终局整合结果另记，不将定向结果相加。

交叉复核补齐第三条链的普通文字首键：B 的 literal `x`、`y` 仍因 A pending 被永久标记
blocked；即使只缩小这个标记，旧 ACK 末尾也没有刷新 B literal，仍需再次输入才能继续。
因此主动终止首轮终局私有 scope，实际 exit 143；中间结果保留在
`interrupted-semantic-only/`，不是产品测试失败，也不算最终通过，不拼入新终局。
该中断目录的 trap 文件 `final.exit-code` 留下了 0，但父工具实际退出 143，UI 日志仍以
`Terminated` 结束且未完成；保留原始记录，不能单凭这个 trap 值判成功。
保留原有语义键断言后扩展 context、host、language 三种边界，追加 literal-first 矩阵；
同一 mandatory exact 共 38 个内部向量，新增 literal 在旧实现真实 exit 101（context、
accepted）。现在仅同目标 pending 标记该稿不确定，不同目标仍等待全局发送屏障；旧 ACK
退休后先刷新当前 literal，再刷新语义队列。当前目标/草稿/焦点/单在途检查及同目标失败
不重放的约束不变。上述 3.73 秒是此前语义键限定的定向通过，不冒充扩展后的终局。

扩展后的同一冻结 exact 修后实际 exit 0（4.86 秒）。随后从头启动的独立统一流水线原进程
实际 **exit 0**：完整 panel 与 11 个相关集成目标共 **12 个唯一结果组，345 通过、0 失败、
29 项原有显式忽略、零筛除**；完整私有 UI **24 个唯一单项**和设置链 **4 个唯一单项**
另行通过，六个 phase/终局退出码均为 0，不拼接首轮中断或内层向量。
格式与严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（5.82 秒），
单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap。

693 个源码/测试/脚本/Cargo/Android 文件终局前后保真，相对本轮起点恰七个 panel 生产/
测试旧文件变化；冻结清单 SHA256 为
`dcc2d6517bd33a032b7657432c795ae87e716309d4ab7f2e282c9b747972ece1`。
Rust / UI / 设置日志 SHA256 分别为
`e43daf482b74b7a93af2d5f1327f4fe97b5cce0c89328ab35230190a48555c62`、
`f366b5f3d6b7be20ecaa8bf9b5d0d373868d4664902ec917f1eb7594767a610c`、
`52d8f0bf3370dd9fe34e573908c8c6aac4e1838bcc8b483499d6a63a869dd28c`。
原 194 项 evidence 与 79 项评分/限制保留，只新增本轮证据及 F30/F31/F32 引用，不提高成熟度。
不是全仓 Rust/Doc-tests、真实模型/麦克风、物理输入、个人 IBus 或远端 CI/发行验收；
词库、25 包、SDK、模型预算、宿主、依赖和版本不变，未提交推送、重装或恢复自动化。

## 继续抓虫：焦点先到的恢复稿与设置暂停

继续上一批未提交快照，仅收口两条输入生命周期边界，不改模型、词库或日常安装。

- `Focused(true)` 的生产处理先退出 native view，清空 typing/keyboard/confirmed action；
  随后的 SeedInput 恢复已取不到稿，`hello` 变回旧 `saved local draft`。有效私有窗口下
  实际回归 exit 101（pending literal、focus-first）。现在取得恢复文本后才退出原生视图，
  通过共享 helper 回填到本地；显式点击与焦点先到用同一恢复语义，不重复请求系统焦点。
  原 begin-text 的 request→成功/失败后恢复顺序不变；无恢复稿时不覆盖原本地稿。
  10 个内部向量覆盖无待恢复稿、pending literal、Space/Backspace 队列及 confirmed action
  两种事件顺序，恢复后继续编辑，晚 ACK/旧外部帧不得覆盖或重发。同一冻结 exact 修后
  实际 exit 0（5.13 秒）；只证明生产 callee 次序，不等同真实 WM 事件调度验收。
- 成功打开设置的 suspend 原来只检查 semantic queue；typing-only 的未发后缀没有被暂停。
  发送 `hell`、保留后缀 `o`，设置打开期间 ACK 先到、设置关闭后才收到 `hell` 新帧，
  会无新按键自动续发 `hello`，真实 exit 101。现在只要 typing 或 keyboard 之一存在，
  分别置 blocked 并提示恢复；不解除旧 blocked，不额外阻塞 confirmed-action-only。
  8 个内部向量覆盖纯文字/语义队列、两种 ACK/frame 顺序、suspend/正常续写对照，
  均可恢复完整 `hello` 且晚确认不重放。同一冻结 exact 修后实际 exit 0（3.71 秒）。
  新向量走成功 settings 创建后的实际 callee，非本轮真实设置窗口创建/失败注入验收。

日志 `/tmp/suzaku-bugfix-recovery-focus.gZidXD/`。初稿测试误读跨模块私有 ActionRequest
字段，构建实际 exit 101；改用不透明请求存在性与已知 fixture 元数据，不扩生产可见性。
随后 focus 对照在隐藏窗口上请求焦点失败，另行保留，不充产品 RED。最终测试仅为私有
X11 窗口 set-visible，并以 1 秒/2ms 有界检查确认 VIEWABLE；不改变生产 FocusLease、
旧断言或业务期限。两项新增 helper 复用既有 mandatory exact，不增顶层测试，不把
18 个内部向量或定向红绿相加为终局数量。恢复保留原有本地引擎/marked-text 同步，
不能称完全无 I/O；没有新增 OS 焦点请求、旧 native 操作重放或云模型调用。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 11 个相关集成目标共
**12 个唯一结果组，345 通过、0 失败、29 项原有显式忽略、零筛除**；私有 UI
**24 个唯一单项**及设置链 **4 个唯一单项**另行全过，六个 phase/终局退出码均为 0。
格式与严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（7.06 秒），
单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap；不拼接定向或旧轮数量。

693 个源码/测试/脚本/Cargo/Android 文件终局前后保真，相对本轮起点恰五个 panel 生产/
测试旧文件变化，冻结清单 SHA256 为
`f56afbbab68c81511e898bd1472831d386edffdf0243804180683be5b4af9153`。
Rust / UI / 设置日志 SHA256 分别为
`23d8c82fd10fc68adf9e30e43b52dac009690f0857974b78fb573789dab3af17`、
`68bad09e9899016555bf7ba01fbb70d9eb69f7fa2acd34db0def302aae2807a8`、
`ce3f459dbc6cd33b010b2fc333f1891006fdabd6212d366349aae07c083d5c54`。
旧 195 项 evidence 与 79 项评分/限制保留，只加本轮证据及 F30/F31 引用，不提升成熟度。
不是全仓 Rust/Doc-tests、真实 WM 调度、物理输入、个人 IBus、模型/麦克风或远端发行验收；
词库、25 包、SDK、模型预算、宿主、依赖及版本不变。未提交推送、重装、修改个人服务/
配置或恢复自动化。

## 继续抓虫：失焦手动恢复与候选退格

用户明确选择失焦时临时保稿、手动恢复；本轮继续未提交快照，不改变版本或个人安装。

- 编辑分支已经提前返回，候选操作区的 Backspace 却还要求 input_focused，实际不可达。
  私有 X11→winit 的真实按键先 Space 再 Backspace，原实现仍为 `hel `，实际 RED exit 101。
  移除这一矛盾条件，复用原退格路径；同一冻结 exact GREEN exit 0（4.27 秒）。
  原设置/preedit/修饰键优先级、step=11、至少 28 键事件和总计 20 秒期限不变。
  新阶段逐步检查空格、连续退格、删至空稿及空稿无效退格，未制造私有 KeyEvent。
- 真实宿主 FocusOut 会 context+1 并发出 redacted 帧，若它先于面板焦点到达，旧 typing/
  keyboard 就被清除，手动恢复只余 saved-local。合法帧 parse 后的定向 RED exit 101。
  新增独立 terminal 内存槽，只保留最近一份退休公开稿，不保存到磁盘、不进入 flush/ACK。
  退休前取最后公开基底与 literal 后缀，不猜测未确认的 Space/Backspace 语义结果。
  Focused 不消费新槽；只在显式进入输入区、退出 native 后校验原本地基底再恢复。
  新 active 稿优先并消费旧缓存；新本地编辑/清空/提交及改变草稿的采用/撤销会淘汰旧缓存。
  原 typing/queue 的断连 blocked、单在途及跨目标禁止重放断言保持；confirmed-only 在 None
  清除前也可脱离暂存。Some 空串不视为 None。没有新稿的重复失焦/重连不擦除缓存。
  最初 8 向量同一冻结 exact 修后 exit 0（4.44 秒），再加 confirmed-only→None 与空稿
  控制，10 向量独立 exit 0（4.45 秒）；不将内部向量或定向红绿计入终局测试总数。
  保留原 5×2 恢复顺序和所有旧断言，并检查新宿主/隐私/语言/字段、晚回复、本地改稿/
  清稿、新 B 稿优先、手动恢复只消费一次且零额外发送。这里只证明生产 callee 顺序，
  不等同真实桌面 WM/个人 IBus 调度；本槽不是多稿历史或最新权威稿承诺。

日志 `/tmp/suzaku-bugfix-backspace-flow.xQD1qI/`，本轮源码相对起点仅六个 panel 生产/
测试旧文件变化；原 native_sync_test 全字节保持，旧边界断言不因保稿偏好被放宽。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 11 个相关集成目标共
**12 个唯一结果组，345 通过、0 失败、29 项原有显式忽略、零筛除**；私有 UI
**24 个唯一单项**及设置链 **4 个唯一单项**另行全过，六个 phase/终局退出码均为 0。
格式和严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（5.79 秒）；
单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap，不拼接定向或旧轮数量。
693 个源码/测试/脚本/Cargo/Android 文件终局前后逐项保真，冻结清单 SHA256 为
`93bf01c1cd2ef194671ffe761bf8e1b50d3f2cd0d2a12cf71a9d313fc0faee57`。
Rust / UI / 设置日志 SHA256 分别为
`6f21fe6cb1892af7f95408c73f0af217a9da43ccfceb2c0b3a2c2349b24ce065`、
`da77b7e7a5a8d8b3e2435e664a518d158989607db64d659a02b818728f53a5ea`、
`8b137d8a89fde555b5e1de154a8b88129b799e99826fec3ab40d6e99ff38e38c`。
原 196 项 evidence 与 79 项评分/限制保留，只新增本轮证据及 F30/F31 引用，不提高成熟度。
不是全仓 Rust/Doc-tests、真实 WM、物理输入、个人 IBus、模型/麦克风或远端发行验收；
词库、25 包、SDK、模型预算、宿主、依赖和版本不变。未提交推送、重装、改个人服务/
配置或恢复自动化。

## 继续抓虫：工具旧回包、命令持键与悬浮球草稿

基线为已推送的 `8cf5b4248f48f14e18a193af9f32d4fd9aebbb52`，本轮不升版。
用户明确排除误贴的另一项目 operator/OIDC 任务；这里只处理 Suzaku 面板。

- 旧语音 A 的成功 ACK 已不清理新 B 的源，却无条件把 B 的“忙碌、已保留”提示改成
  插入成功。让工具收尾返回是否真正退休当前源，仅成功退休时更新提示；宿主 ACK/frame
  屏障仍独立完成，不重发 B。冻结三向量含正常 A、不同文字 B、同文新代次 B，原 exact
  实际 RED 101（4 个提示/计时发现），修后 GREEN 0 / 0.67 秒。
- 候选操作区的 I 插入、1 切键盘会进入编辑，未松开的命令重复却先走编辑分支。
  私有 X11→winit 验证第二 press 的 `repeat=true`，原码得到 `helloi` 而非 `hello`，
  实际 RED 101 / 4.95 秒。复用有限已占用键集合，只在这两个命令真正执行前登记，
  在各自真实 release 后解除；不改数字选候选，不全局禁止文字重复。
- 普通可聚焦后端收成悬浮球后仍可能保有焦点，Backspace/Space 改动隐藏草稿，
  同一冻结 exact 在仅修持键后独立 RED 101 / 4.48 秒（`hell ` != `hello`）。
  compact 不自动聚焦编辑区，也不接受普通编辑；设置、缩放、退出快捷键保留。
  第一版 compact 拦截误挡已打开设置的 Esc，冻结对照真实 RED 101 / 4.45 秒；
  补上设置分流条件后同一测试 GREEN 0 / 4.13 秒，未改断言或时限。

键盘新增阶段验证 I/1 释放后的新按键及普通重复仍写入 `helloii` / `abc11`，
悬浮球保留完整引擎快照与非空撤销历史，F1/Esc 可用。真实 X11 事件仅送私有测试窗；
额外 Focused(true) 为明确 callee 模拟，不是个人 WM 调度。旧 step 11、至少 28 键、
总计 20 秒与全部旧代码/断言保留。语音源为合成采集桥，不是实时麦克风或 ASR 验收。

日志 `/tmp/suzaku-bugfix-tool-ack.qWMgWn/`；工具/键盘冻结测试 SHA256 分别为
`a174c9877276be762c1fcc93bfe6df614fc943c6ab45f20a5c377af257825dc9`、
`7ca54a0bafb85178d773c4bc4fc67b1ae3c5f3f29ce1375d776b8cccb3b6b08b`。
只改五个 panel 生产/测试旧文件，198 项旧 evidence 和 79 项评分/限制保留，不提高成熟度。
词库、包、SDK、模型协议/预算、宿主、依赖与 0.10.4/build54 不变；未提交推送、安装、
改变个人服务/配置或恢复定时任务。相关统一验收结果补在下文，不以定向红绿拼接总数。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 11 个相关集成目标
共 **12 个唯一结果组，345 通过、0 失败、29 项原有显式忽略、零筛除**；私有 UI
**24 个唯一单项**及设置链 **4 个唯一单项**另行全过，六个 phase/终局退出码均为 0。
格式与严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（7.30 秒）；
单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap，不拼接定向或历史结果。
693 个源码/测试/脚本/Cargo/Android 文件终局前后逐项保真，冻结清单 SHA256 为
`83ce8c782e405b960a15913e5bfde8fb9c5016d8162d57e71cbd3485a8a2134d`。
Rust / UI / 设置日志 SHA256 分别为
`5e9a310302a10274426bf08324b03ca3990296cc92ae435b255981b22309e27e`、
`e4157cd423264dc5e3387b649b933f23a85e7dd057cb615b5ccd40dd6dd12b94`、
`c749a11fd12ba7d6221ddfe34733b68850b46757200459edf849019baece35af`。
不是全仓 Rust/Doc-tests、物理输入、个人桌面或远端 CI/发行验收；不据此提高成熟度。

## 继续测试：手写触点、失焦识别与采样边界

本轮接续上节未提交快照，仍基于 `8cf5b4248f48f14e18a193af9f32d4fd9aebbb52`。

- Touch 未区分 device/id，第二指 Started 会拆出新笔画，Moved/Ended/Cancelled 也可干扰
  原手势。私有真实呈现画布上的受控 winit Touch 回归实际 RED 101 / 7.49 秒；现在首指
  独占手势至匹配的终止事件，外来触点和期间的鼠标坐标/按钮不改变它。失焦、显式取消、
  隐藏、收缩及两个关闭请求入口清理持有，避免无焦点后端丢失 release 后永久挡住新触点。
  几何刷新仅取消旧操作而不移交仍按着的触点，防止尾事件误采用新目标。
- 手写中失焦保留了新笔迹，却直接清 dragging，留下旧候选和旧模型摘要。仅修触点后，
  同一冻结 exact 独立 RED 101 / 7.69 秒，候选实际不等于当前笔迹的识别结果。
  失焦复用既有取消触点的完成识别路径，不自动插入；晚释放不重复识别，新触点仍可绘制。
  同一冻结 exact 修后 GREEN 0 / 7.65 秒，正常完成、外来触点、新手势与无额外发送对照保留。
- 默认 padding=4 在小于 8 的画布轴上、或有限但过大配置下，会给 f32::clamp 反向边界；
  即使先限为半轴，小数舍入仍可反转。每轴限定 padding 至半轴，并让上界不低于已算下界，
  仅塌缩不足轴，普通/零 padding 坐标精确不变。平滑上下限先分别限制到 [0,1] 再排序，
  修复负范围得到 [0,-0.5] 的 panic 和高范围实际 alpha=1.4142135。
  两个普通 CPU 测试各以 current_exe exact 子进程隔离 OnceLock，清空环境并核实完成标记；
  六种几何与四种平滑内部 case 在旧实现 RED 101 / 0.05 秒，冻结测试修后 2/2、exit 0 / 0.04 秒。
  默认鼠标/触摸三档速度计算逐值不变，不把子进程或内部 case 计为新增顶层结果组。

日志 `/tmp/suzaku-bugfix-handwriting.2Ktxb6/`。首次触点测试因私有窗口尚未展开而找不到
canvas（exit 101 / 7.54 秒），属于测试夹具失败，不算产品 RED。仅修新增准备 helper，
由生产 fit 请求并限 1 秒确认真实窗口尺寸稳定，再呈现；旧失败与旧冻结均保留，
目标断言和原测试期限未变。修夹具后冻结 SHA256 为
`df0da175c55631ce013ab03baeb88e5c79459d92ddc82c53b5a550a90672ef95`；
参数测试冻结文件 SHA256 为
`6262f3f65f25b4ccd0993eb831e3d9abeb53198de8aefa430406055cf25dd3d4`（只比较新增测试块）。
触点与焦点为显式 callee 事件，不是物理触摸板/屏幕或真实 WM 调度验收；参数测试也不
等同真实窗口缩放、识别质量或任意非法/非有限几何保证。隐藏清理的生产调用仅静态复核，
不冒称个人桌面实际隐藏/重开。词库/包/SDK/模型预算/宿主/依赖/版本不变，未提交推送、
安装、修改个人服务/配置或恢复自动化。统一相关验收另列，不拼接定向或上一轮总数。

保留上述 df0 全部代码/断言后，仅追加 owner Cancelled、CloseRequested、收缩展开及
触摸期间鼠标事件对照，最终工具测试冻结 SHA256 为
`4b04c9ccda3d2b2a04f6a73c7f0856469fc2a79cc68790f4e6ac32d862f96700`，
增量 exact 实际 GREEN 0 / 7.88 秒；旧尾事件不改变笔迹/候选/摘要/代次，新触点可继续。
CloseRequested 只核对 PanelState 取消和 flag，后续重置 flag 是夹具隔离，不冒称 App 隐藏。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成目标
共 **14 个唯一结果组，360 通过、0 失败、29 项原有显式忽略、零筛除**，包含额外的
手写布局/绘制场景；私有 UI **24 个唯一单项**及设置链 **4 个唯一单项**另行全过。
六个 phase/终局退出码均为 0，格式与严格 all-target/all-feature locked/offline Clippy
`-D warnings` 通过（5.88 秒），单任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap。
693 个源码/测试/脚本/Cargo/Android 文件终局前后逐项保真，本轮相对上节起点仅六个旧
生产/测试文件改变；冻结清单 SHA256 为
`a6bf986fa2c778cbe595051b37114033361ca7d2e96e89ccd03df8d78a3cd92a`。
Rust / UI / 设置日志 SHA256 分别为
`4dcafc7130a4429babec6c818d41e91616f3c6a2bbf5f5fd2743df40f70d7d9c`、
`3cd20eb47570abb27f6e142ef164aeb5e452f7f37884b814a965542aaaa7c73c`、
`b1d6a870123480945da8d95c36d556cd4667fd8a16ab5dddf609ca46fda8d952`。
199 项旧 evidence 与 79 项评分/限制保持，只加本轮证据及 F32 引用；不提升成熟度。
本轮不是全仓 Rust/Doc-tests 或当前未提交修正的远端验收，不拼接之前版本/定向测试数量。

## 继续测试：报告持键与完整触点身份

接续上节未提交源码，仍为 `8cf5b4248f48f14e18a193af9f32d4fd9aebbb52` 的后续修正。

- 报告吞下按键却不保存归属，关闭后同设置窗保留搜索焦点，未松开的重复键误改查询。
  自有 Xvfb 窗口真实 X11 press→已呈现按钮鼠标关闭→第二 press，硬验 winit.repeat=true，
  旧码实际把 `retained query` 退格为 `retained quer`：RED 101 / 5.86 秒。
  抽中性 hold_owned_key/consume_owned_hold 并保留旧 wrapper 与所有旧 CPU 测试块；
  报告 press 仅登记、不拦自身导航重复，真 release 清理后仍检查 Escape 匹配松键关闭。
  进入、替换和退出报告不清 holds，真正失焦才直接清理，不调用宿主/IME 焦点处理。
- 报告触点只保存 id，漏掉事件的 device。仅修持键后同一冻结 exact 独立到受控 Touch
  异设备同 id 误完成 Next（page 0→1）：RED 101 / 5.76 秒。
  PointerSource::Touch 增加 DeviceId，路由原样带入事件的双字段；原 press/release、
  异源取消旧手势、呈现/generation/矩形检验、分页及 Mouse 行为保持，五处旧 unit
  仅机械补同一个 dummy 设备。异源 release 已取走 press，旧 A 尾事件也不能复活它。

冻结测试 SHA256 `3a772bc1bf9b4e6e23be15a21e3617534a6e5aab7365fa810d7bcc9ac7c7145f`，
两修复后同一 exact 完整 GREEN 0 / 5.60 秒。新增四个键向量含 Backspace、报告中新 A、
真实录制拒绝 A 后进入并替换报告、报告内松 A；恢复后的新按键及普通文字重复均可用。
PageDown 报告重复分页、Escape 按下/重复不关闭及匹配松键关闭均有正对照。
硬核对 29 个实收键转移和 9 个真实 winit repeat；旧报告同步主体 359 行逐字逆投影保持，
新增阶段 20 秒截止、原外层 90 秒/exact 不变，不把内部向量计为新顶层测试。
受控 Touch 对照覆盖 Next/Close 的异源拒绝、旧尾事件及新同源手势；两个合法 opaque
设备值取自自有 X11 键盘事件和 dummy 并先 assert_ne，不是双物理触屏验收，也不保证
后端向应用暴露物理唯一设备 ID。未知物理键和纯 modifier 仍不存，销毁的窗口不跨窗移交键。

日志 `/tmp/suzaku-bugfix-report-holds.2JmFQW/`。本轮仅四个旧生产/测试源文件改变，
200 项旧 evidence 与 79 项评分/限制保持；只引用 F40，不提高成熟度。
不改模型、词库/包/SDK、宿主/依赖/版本、个人安装或服务/配置，不提交推送或恢复自动化。
上述为私有事件路由与 callee 验收，不是物理键盘、真实 WM、实模或本轮远端/发行验收；
统一相关结果另列，不累加定向红绿、内部向量或旧轮数量。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成目标
共 **14 个唯一结果组，360 通过、0 失败、29 项原有显式忽略、零筛除**；私有 UI
**24 个唯一单项**及设置链 **4 个唯一单项**另行全过，六个 phase/终局退出码均为 0。
格式与严格 all-target/all-feature locked/offline Clippy `-D warnings` 通过（6.75 秒），
单构建任务、2 GiB MemoryHigh / 3 GiB MemoryMax / 无 swap。693 个源码/测试/脚本/
Cargo/Android 文件终局前后逐项保真，冻结清单 SHA256 为
`853c56be843785e7b2cac6f7f9821689109332238396b06f86d409d8fd60e1ee`。
Rust / UI / 设置日志 SHA256 分别为
`3d8b9cb4b3eb5ba1927ec7ff7cc2c81a1cee93049ef9bc522dd9a8262aa17382`、
`a7c5a71a47507977d9aa226d20e117b9d859f0f0274daf16c93785c7b1a8b900`、
`2f02002bfa2daa8f45fd730732c9918d1d7268db22a9b96b6decaf1df68f552b`。
本轮不重跑全仓 Rust/Doc-tests，不以旧提交 CI 通过代替这些未提交修正的远端验收。

## 继续测试：设置窗口不接管模型配置

辅助 Settings 窗口没有已确认的模型配置，却在点击 On 或 Tone 时重配自己的引擎。
普通配置错误在设置页本来不可见，真实症状是它覆盖了仍可见的输入法服务错误，导致
提示提前消失；同时不必要地重置辅助窗的 engine/seed。修正在重配入口只允许 Main
继续，设置值与编辑代次仍通过原镜像、补丁和确认链传递，不向 Settings 复制配置。

原设置链 mandatory first_status exact 仅追加 helper，旧主体和断言不变。私有窗口
先呈现服务错误，再点击模型 On，旧代码实际把它覆盖成普通配置错误：目标 exact
RED 101 / 0.96 秒、整设置脚本实际 exit 1。修复后同一冻结测试 GREEN 0 / 1.09 秒，
整设置链 4 项实际 exit 0。测试冻结 SHA256：
`38ab7ca9c28a193b99a03413a6a66e6b10d8cff84969937072305cc84a49196b`。

有/无服务错误两组各点击 On、Focused、Expressive、Off，核对原 feedback/ticks、
实际 overlay 有无、辅助 engine/候选/seed 保持，以及两窗值和编辑代次同步。Main
使用已确认配置、空草稿仍为 Idle，关闭仍为 Disabled；Main 缺少配置仍报真实错误，
恢复配置后清除错误。空草稿不能调度预测，不使用模型或密钥，也不把这项配置测试
称为真实 HTTP、模型质量或个人桌面输入验收。日志
`/tmp/suzaku-bugfix-settings-model.MHQbr5/`。本轮只变更两个生产/测试文件，保留前轮
全部修正、201 项旧证据与 79 项评分/限制；不提交推送、安装或改个人服务/配置。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成
目标共 **14 个唯一结果组，360 通过、0 失败、29 项原有显式忽略、零筛除**；
私有 UI **24 个唯一单项**及设置链 **4 个唯一单项**另行全过，格式和严格
all-target/all-feature locked/offline Clippy `-D warnings` 通过（5.87 秒）。六阶段及
终局退出码均为 0，693 个源码/测试/脚本/Cargo/Android 文件前后冻结校验通过；
冻结清单 SHA256 为 `b348bc6627e928e1f1b378676045b562f743c9cb8fec04d4db83e00096c53120`。
Rust / UI / 设置日志 SHA256 分别为
`9296950d8b8f67ca7ccab70e0014a68749e0ef44f3fb4d4254e469eecd419d41`、
`6a320a196536c9da8fc06a1d359479368d461819c9c704dd24c30623b7fa2ad4`、
`0ae8782fa20e6cc84bcc919a2415d757068b9f9edb2a67ff22dbd9e1c8de011c`。
本轮不是全仓 Rust/Doc-tests 或当前未提交修正的远端验收，内部 8 个动作不另计顶层测试。

## 继续测试：独立候选选择与实际可见目标

引擎有六项候选、独立面板只显示前四项时，方向选择仍可到第 5/6 项；旧 Enter
绕过句卡可见身份，发送用户未见的候选。保留候选完整可达性，不截断共享引擎：仅在
独立面板把选中项投影到第四句卡槽，保留前三项锚点及真实 source/label/payload。
返回前四项即恢复默认映射，英文下一词仍从完整池派生，native 分页路径不改。

投影先于 LocalSentenceFrame 建立；投影变化沿原刷新退役旧手势和呈现，重新绘制
后才能 Enter。可见区内移动、零位移与边界不额外作废呈现或激活 bridge。选择刷新
的轻量比较只供同步 move 使用，不能替代异步候选、语言或草稿变更时的完整刷新。
Enter 的本地显式选择和原默认主卡还须通过 resolve 及实际已绘制 Candidate 目标
检查，覆盖翻译工具隐藏全部卡片的情况；拒绝时不另发其它候选。

原 mandatory Enter exact 只追加两个 helper 和一个调用，旧主体/断言/期限逆投影
逐字保持。六项公开合成插件和私有 ACK fixture 无模型请求；测试冻结 SHA256：
`7439384fd145c63710523a84ea47455e7cfa082d4b794bb750598f92992fcb7d`。
旧码第 5 项未呈现 Enter 已发一条请求：实际 RED 101 / 4.52 秒；只修投影后，同一
冻结测试独立复现 Translation 无卡时发送默认内容：RED 101 / 4.67 秒。加可见目标
门禁后同 exact 完整 GREEN 0 / 5.00 秒。

回归核对实际 CandidatePrimary 标题绘制和非零目标，不用 selected_label 元数据
冒充显示；第 5/6 项重显后精确发送、拒绝 ACK 后保稿与选择、回前区恢复默认映射、
数字 4 采用第 6 项为可撤销编辑、翻译隐藏区的显式/default 两类零发送及完整
seed/history/snapshot 保留、重显后可发送、空稿不注入卡片均通过。旧英文 hello/help
及 native 用例原样保留；这是生产 callee/GPU 呈现与合成回执测试，不是物理按键、
个人 IBus、实模质量或新性能验收。日志 `/tmp/suzaku-bugfix-visible-selection.ryBrt8/`。
本轮只改动三个旧生产/测试文件，保留 202 项旧证据与 79 项评分/限制、前轮全部
修正和失败；词库/包/SDK/模型协议预算/宿主/依赖/版本不变，未提交推送、重装或操作
个人服务/配置，不恢复自动化。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成
目标共 **14 个唯一结果组，360 通过、0 失败、29 项原有显式忽略、零筛除**；
私有 UI **24 个唯一单项**及设置链 **4 个唯一单项**另行全过，格式和严格
all-target/all-feature locked/offline Clippy `-D warnings` 通过（6.22 秒）。六阶段与
终局退出码全为 0，693 个源码/测试/脚本/Cargo/Android 文件前后冻结校验通过；
冻结清单 SHA256 为 `060c9e905df3383f29cbb6f7959b21616db725be91af40ce4e62754e92cc3f13`。
Rust / UI / 设置日志 SHA256 分别为
`385355be3369ea3e8613b0f63a4e9dcaa340b5b8845984cf5086e8077b7e2035`、
`55032a436caae6f418d036f521f0e0cbbc8275a4a2d291b3ad4ebbaa070564db`、
`612096ac5c1eff23c2f2cf702a1a610ec26f7a4195297d729976fb4b3a9d93ef`。
本轮不是全仓 Rust/Doc-tests 或准确新提交的远端验收；内部向量不计为新增顶层测试，
也不累加定向红绿或前轮数量。

## 继续测试：完全遮挡不授权指针操作

Main 的 Occluded 原先只撤销 native lease，没有取消按压和指针呈现。冻结主窗测试
`573b9c5e200fb6962cfc616e98d838ec67616d83c48314506125011435ea67aa`：
已呈现候选按下→完全遮挡→松开，旧码仍发送 hello（RED 101 / 5.44 秒）；仅取消旧
手势后，同一测试继续复现遮挡中重绘重新授权新按压（RED 101 / 5.40 秒）。
现在 Main 转 true 先取消按压/触摸归属/拖拽并清 hover，bool 改变才失效呈现；重复
false 不伤正常可见手势，因为 X11 Partial/Unobscured 通知都映射为 false。

Main 与报告共用指针事件门，Main 在 touch owner 分配前拒绝遮挡期输入，重绘不会
重新授权鼠标、触摸、滚轮或已有缩放手势。报告旧 invalidate 不足以挡新按压：冻结
`1ec9578d216bbeb98464e1879a6d5e8bcb362b8825968b6f1e350d823d9c419a` 在遮挡重绘
后仍翻页 0→1（RED 101 / 5.88 秒）。入口复用指针门后，两份原 exact 分别完整
GREEN 0 / 5.42 秒和 6.35 秒，旧报告真实 X11 持键及双 device 触点测试一并保留。

主窗 Mouse/Touch 覆盖四类旧尾释放时序、遮挡重绘后的新手势与 owner 拒绝、复显
未呈现拒绝、重新呈现后新点击精确交付及 ACK0 保稿、重复 false 正例。报告覆盖
旧 Next/Close 按压跨遮挡不复活、遮挡重绘后的新 Next/Close 与 wheel 拒绝，复显
前后授权及正常 Mouse/Touch 翻页/关闭，冻结源、编辑状态、隐藏 compact 主窗和配置
字节保持。旧测试主体/断言/期限逆投影逐字保留，新 CPU 分类父测试只证明分流。
生命周期、重绘、键盘与 IME 原策略不变，不把这轮说成“遮挡时全部输入禁用”。

日志 `/tmp/suzaku-bugfix-occluded-input.gaLk6x/`。vendor X11 确实从 VisibilityNotify
发出该事件，但上述是受控生命周期/指针 callee 与真实私有 GPU 呈现，并非真实 WM
盖窗、物理触屏、Wayland 或跨平台验收；也不承诺 partial 遮挡的逐区域可见性。
未来新增有效指针手势须同步分类。仅四个旧生产/测试源文件改变，保留 203 项旧证据、
79 项评分/限制与前轮全部修正；不改词库/包/SDK/模型协议预算/宿主/依赖/版本，
未提交推送、安装或修改个人服务/配置，不恢复自动化。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成
目标共 **14 个唯一结果组，361 通过、0 失败、29 项原有显式忽略、零筛除**；
其中仅新加一个普通 CPU 分类父测试，内部向量不另计。私有 UI **24 个唯一单项**
及设置链 **4 个唯一单项**另行全过，格式和严格 all-target/all-feature locked/offline
Clippy `-D warnings` 通过（7.13 秒）；六阶段及终局退出码均为 0。
693 个源码/测试/脚本/Cargo/Android 文件前后冻结校验通过，清单 SHA256 为
`37795ad1ba12a2677fcb81e19d0e8ecbb96f96219505ee1667cf48d28fa73ad8`。
Rust / UI / 设置日志 SHA256 分别为
`2b8944af838ad4cd1ac8eccf514cf15655e576fa73177ee931e8cd703af10be5`、
`f180d52cbfdf9769a9193ce3c850a4e8823797072919ef6f49caac07df265aab`、
`54aab865b4181561969181918b7d377aa3739a14ffe7843b9e826c2dc0d851ec`。
不拼接定向红绿或前轮数量，不称全仓 Rust/Doc-tests 或新提交远端验收。

## 继续测试：设置滚动拖动在失焦时收尾

辅助 Settings 的 release 会在失焦时被输入门禁过滤，旧 focus 清理却遗漏滚动条拖动。
回焦后仅移动鼠标即可继续旧拖动。原 mandatory first-status exact 追加 helper 与调用，
旧主体/断言/期限逐字保留；冻结测试 SHA256 为
`5ae350a177fabc07be2bc98497a0f12ca1e31cbea2c5ac69d3b39024267fa32d`。
先用实际 520×420 视口呈现滚动条并证明正常拖动，失焦时发送具体 release，再回焦
只移动鼠标：旧码 offset 从 41.106834 变为 50.2417，首 exact 实际 RED 101 / 3.72 秒，
四项设置脚本实际 exit 1。仅在真实 focus true→false 分支调用现有
end_settings_scroll_drag，清理 flag/start_y/start_offset，不重置实际滚动位置；
相同 focus 的早返回和其余输入清理保持。同一冻结测试 GREEN 0 / 3.82 秒，四项脚本
实际 exit 0，没有改断言、期限或 runner 清单。

回归覆盖 mouse/touch 两种起始拖动、失焦 release 被过滤、回焦无 press 的鼠标移动
以及旧 touch tail 不滚、重新完整拖动正常、重复 Focused(true) 保留合法拖动。
两窗草稿/翻译/完整引擎快照/PersistedDisplaySettings 与私有配置原字节保持；末尾
恢复窗口大小、焦点、光标和导航状态。这是私有 Xvfb 实际辅助 Settings 呈现及生产
PanelApp 受控事件路由，不是物理键鼠/触屏、真实 WM 切窗或 Main 内嵌设置滚动验收。
日志 `/tmp/suzaku-bugfix-scroll-focus.T21SKt/`；本轮仅两个旧生产/测试源文件变化，
204 项旧证据与 79 项评分/限制不变，前轮修正及真实失败均保留。词库/包/SDK/模型
协议预算/宿主/依赖/版本不变，未提交推送、安装或修改个人服务/配置，不恢复自动化。

最终同一冻结源码独立流水线原进程实际 **exit 0**：完整 panel 与 13 个相关集成
目标共 **14 个唯一结果组，361 通过、0 失败、29 项原有显式忽略、零筛除**；私有
UI **24 个唯一单项**及设置链 **4 个唯一单项**另行全过，不把 exact 的正常筛选冒充
普通回归零筛除，也不累加定向红绿或前轮数量。格式与严格 all-target/all-feature
locked/offline Clippy `-D warnings` 通过（6.23 秒），六阶段及终局退出码均为 0。
693 个源码/测试/脚本/Cargo/Android 文件前后冻结校验通过，清单 SHA256 为
`d6e2b6f5fa39862692a740dcdda8b0d6537c7eeba4546e4df0b08b68c35ef074`。
Rust / UI / 设置日志 SHA256 分别为
`0fc6a4e73b5a219bec520bf18bd58e2d52ffe0a5b3b2e33348a8028b997be620`、
`d67f60f0025baf12642e3d526d8df2a0ed48f93cb42ffc7384d1655fd65d8790`、
`f140977a04442662060ac2f29d3ec7c0d4294f183674db7e6b2d7c8721159cb3`。
本轮没有新增顶层 test；helper 内向量不另计，不称全仓 Rust/Doc-tests、个人安装
或准确新提交的远端验收。

## 继续测试：设置命令的持键归属

同一已聚焦 Settings 中按 Ctrl+, 或 Ctrl+F，先松 Ctrl 而继续按住命令键，旧分派会
把后续无修饰键 repeat 当成搜索文字。已接受的设置命令现在登记实际 physical key，
由原真实 release 退休归属；Ctrl+F 的 logical f/F 与精确 Ctrl/macOS Super 判断
抽成共享纯函数，供分派持键与设置动作共同使用，不猜测布局上的物理 KeyF。
录制/preedit 前置门禁、未知物理键不保留、焦点清理与普通文字 repeat 均保持；
本轮不实现 Main→Settings 跨窗口持键转移。

原 mandatory recording exact 七处纯新增 350 行，旧主体/六 pressed 计数和整个
exact 的原 20 秒期限逐字保留。冻结 SHA256：
`4753940f577be6fc62f3cfa0b11ddbb141757b6cc668011b0d815c261371bdd0`。
同一实际私有 Settings 重新聚焦，每命令八项 X11→winit 转换均核 physical/repeat/
modifier。旧 Ctrl+, 未释放尾 repeat 把 Text 改为 Text,，实际 RED 101 / 0.77 秒；
仅补第一处分派持键后，同冻结测试继续复现 Ctrl+F 的 Text→Textf，RED 101 /
0.82 秒。两处修正后完整 GREEN 0 / 0.90 秒，初始搜索未聚焦、实际命令动作、
释 Ctrl 后无输入、真实释键后 fresh + plain repeat 得到 Text,,/Textff 均通过。
草稿、翻译、完整引擎、显示偏好、快捷键代次和两私有配置的存在/缺失及原字节保持。
这是私有 Xvfb 注入 X11 事件经实际 winit/生产 dispatcher，不是物理键盘、个人
桌面输入、跨窗口转移、Wayland 或模型请求验收。

初轮扩展回归实际 exit 101：辅助窗口 CPU 夹具抽取生产快捷键分支，却缺少新增的
event/hold 依赖，rustc 报 E0425/E0599。保存原失败后，补显式 F1/Comma 身份、
事件上下文与去重 hold/clear stub；原生产分支和旧所有恢复断言均保留，只追加一个
内层测试。它直接调用请求分支，仅证明已接受请求不抹 preedit，不意味着 composing
门禁允许 F1/Ctrl+,。定向父测试实际 exit 0，内部 11 项均过；不把这些 stub 当成
真实按键重复验收，也不计为新增顶层测试。日志
`/tmp/suzaku-bugfix-settings-holds.MmHkRn/`。

仅四个生产/测试源文件变化，保留 205 项旧证据、79 项评分/限制及前轮全部修正与
真实失败；词库/包/SDK/模型协议预算/宿主/依赖/版本不变，未提交推送、安装或操作
个人服务/配置，不恢复自动化。

补齐夹具后，重新冻结四处源改动并独立执行整轮，原进程实际 **exit 0**：完整 panel
与 13 个相关集成目标共 **14 个唯一结果组，361 通过、0 失败、29 项原有显式忽略、
零筛除**；私有 UI **24 个唯一单项**、设置链 **4 个唯一单项**另行全过。格式及严格
all-target/all-feature locked/offline Clippy `-D warnings` 通过（0.51 秒，增量检查，
不是性能验收）；六阶段及终局退出码全为 0。不拼接初轮局部通过、定向重试或前轮
数量，也不称全仓 Rust/Doc-tests 或本轮准确提交远端验收。
693 个源码/测试/脚本/Cargo/Android 文件前后冻结校验通过，清单 SHA256 为
`416f4696d85e24d8bbd082779dc4d9149a03d8c9c2e897617a786ef051e4f3a0`。
Rust / UI / 设置日志 SHA256 分别为
`ed8b7236f948e2afcdec5449af441b3c9f46b5d038936d1736c2c6eb41f95473`、
`530b2f43a131bef98f9ae49ce51037637b8e6bc82f2e96b1333bdddfe91041eb`、
`4cab35081d7ee15d0b3ea5c5e99ec86a60c2a75f7b268b348dfc133d0f574fc1`。
