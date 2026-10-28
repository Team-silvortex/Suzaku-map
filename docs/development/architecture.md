# 架构与模块边界

Suzaku 用共享 Rust 引擎处理可编辑草稿、语言转换和候选，再由平台宿主完成真实提交。
面板是输入与展示端，不是第二个系统输入法；模型提供器只返回预测结果，不拥有键盘事件或提交权限。

## 主链路

```text
应用键盘事件 → Linux IBus 桥 → HostImeSession → 共享引擎 → 语言转换与离线候选
                                               │
                                               ├→ 有界预测 worker → PredictionProvider → 服务适配器
                                               │
                                               └→ 候选快照 → 原生面板或 IBus 展示
用户明确提交 → 宿主复核当前输入上下文 → IBus CommitText → 目标应用
```

离线候选先可用，模型结果异步合并。编辑、语言变化、焦点或隐私边界变化会取消或淘汰旧预测；
一次展示的候选不能在异步刷新后被旧手势误提交。选择进入可编辑草稿，与最终提交是不同动作。

## 模块所有权

| 边界 | 代码入口 | 负责什么 |
| --- | --- | --- |
| 输入引擎 | [core_engine.rs](../../src/ime/core_engine.rs) | 草稿、语言注册、候选、选择、提交和会话上下文 |
| 语言实现 | [languages](../../src/languages/mod.rs) | 英文补全、中文拼音与日文罗马音的归一化、解码和语言策略 |
| 数据契约 | [lexicon.rs](../../src/lexicon.rs)、[packs](../../src/lexicon/packs.rs) | 有序词句、格式校验、可选包与启动快照 |
| 预测契约 | [prediction](../../src/prediction/mod.rs) | 版本、请求编号、任务输入输出、预算、取消和类型化错误 |
| 模型适配 | [model.rs](../../src/model.rs)、[model/](../../src/model/) | 部署配置、发现、HTTP 协议和响应解析 |
| 宿主会话 | [ime_host.rs](../../src/ime_host.rs) | 共享引擎与平台事件的桥接、会话状态和 FFI 接口 |
| Linux 原生宿主 | [linux_ibus_host.rs](../../src/platform/linux_ibus_host.rs)、[IBus C 桥](../../src/linux/ibus_engine_bridge.c) | IBus 生命周期、按键与应用上下文、原生提交 |
| 面板 | [panel.rs](../../src/bin/panel.rs)、[panel/](../../src/bin/panel/) | GPU 绘制、窗口、设置、候选手势、屏幕键盘和工具交互 |
| 平台能力 | [platform](../../src/platform/mod.rs) | 适配器契约、运行时能力、窗口与输出桥 |
| 偏好与持久化 | [preferences.rs](../../src/preferences.rs)、[worker](../../src/preferences/worker.rs) | 候选偏好、衰减、有界后台存储与清除 |
| 数据管理 | [data](../../src/data.rs) | 受管路径、配置读写、备份恢复和并发租约 |

## 草稿和候选

`LanguageRegistry` 注册 `LanguagePlugin`。内置语言 ID 是 `en`、`zh-Hans` 和 `ja`；
新增语言要实现转换规则，添加同名 JSON 不能自动提供新的输入法。
候选包含正文、展示标签、类型、来源和本地评分；展示的序号、角标和注解不属于提交正文。

词库只提供有序数据。匹配窗口、拼音解码、罗马音处理、大小写和前缀保留属于语言实现；
词句混排属于 [candidate_mix.rs](../../src/ime/candidate_mix.rs)。
扩充语料不能通过改写历史首选或增加 UI 特例来绕过这些契约。
实际按键语义见 [快捷键](../shortcuts.md)与 [IBus 候选体验](../ibus-candidates.md)。

## 预测与模型

引擎的 [PredictionWorker](../../src/ime/prediction.rs) 只有一个可替换的待处理请求和结果槽。
模型运行期间不持有引擎会话锁；本地 revision 屏障独立于响应里的请求编号。
`PredictionProvider` 的 v1 输出是完整草稿替换候选，不是追加 token、任意范围编辑或工具调用。

`HttpModelProvider` 将该任务适配为 Ollama 或 OpenAI-compatible 请求。
模型名称、部署在本地还是云端、HTTP 格式是独立选项；本地发现偏好已安装的 LLaMA，
并不绑定某个家族或版本。词库查询不发起模型请求。
旧 `languages::model`、`languages::llama` 和旧联想接口是兼容入口，不是另一套服务。

当前协议仍共享语言前缀与提示策略，并非完全没有语言依赖的独立 crate。
显式翻译使用独立的 `TranslationProvider`，共享服务配置和传输，不复用候选任务。
预算、配置和接入方法统一见 [模型服务](../model-providers.md)。

## Linux 宿主与面板

[linux_ime_host](../../src/bin/linux_ime_host.rs) 在输入循环启动前初始化词包和偏好，
再进入 IBus 宿主。C 桥负责 IBus 事件与应用上下文；Rust 会话负责输入状态。
候选默认由 Suzaku 原生面板呈现，IBus 展示保留为兼容路径，不能同时建立两个互不相干的草稿。

[linux_ime_sync](../../src/platform/linux_ime_sync.rs) 与 [linux_ipc](../../src/platform/linux_ipc.rs)
连接宿主和面板。面板采用宿主快照；动作必须绑定当前目标和展示状态。
独立面板的输出同样需要送达确认，失败或未知结果不能当作成功清空草稿。
窗口定位、缩放、设置交互属于面板；不能通过扩大系统按键钩子来实现普通 UI 功能。

平台抽象保留 macOS、Windows、Android 等适配入口，但当前发行与重点实测在 Linux。
代码里的能力矩阵、接口存在或跨平台编译成功，都不是平台原生运行已经验收的证明。

## 数据与隐私

内置 JSON 经 `include_str!` 嵌入并在首次使用时校验缓存。
可选词包在进程启动时合成不可变快照；输入路径不读磁盘或网络，运行中安装包不会热替换索引。
包是数据，不加载其脚本或动态库；详见 [词库维护](lexicons.md)。

候选偏好与模型配置无关。文件读写由后台 worker 承担，文件存储使用私有路径和单写者租约；
存储不可用时保留内存候选，不阻塞打字。偏好摘要仍属于敏感数据，不因未保存原文而公开分发。
焦点和隐私边界也要清理提交上下文；密码/PIN 不进入普通候选与模型链。
具体数据位置、清除方法和云端授权见 [隐私说明](../privacy.md)及[数据管理](../linux-packaging-data.md)。

## 继续调查

[功能网络](../functional-network.md)将功能、实现和证据连接起来；
[测试手册](testing.md)说明如何独立验证这些边界。
早期架构描述保留在 [整理前开发记录](../history/2026-10/pre-reorganization/development.md)，
不作为当前平台成熟度或性能承诺。
