# Suzaku 开发入口

Suzaku 当前以 Linux 和 IBus 输入体验为主线，共享 Rust 引擎供原生宿主和面板使用。
开发说明只保留当前架构、约束和可复现命令；版本变化与验收过程分别保存，不再在这里追加流水。

## 开发手册

- [开发导航](docs/development/README.md)：按任务找到代码与验证入口。
- [架构与模块边界](docs/development/architecture.md)：输入、模型、面板、平台和数据的职责。
- [构建与测试](docs/development/testing.md)：Linux 依赖、静态检查、隔离 IBus 和 UI 回归。
- [词库维护](docs/development/lexicons.md)：格式、排序、预算与新增内容的回归要求。
- [功能链条与成熟度](docs/functional-network.md)：功能路由、实现和证据索引。
- [发布与打包](docs/linux-packaging-data.md)：安装包、数据管理和发布门禁。
- [贡献约定](CONTRIBUTING.md)、[安全报告](SECURITY.md)。

使用方法见 [项目入口](README.md)、[快捷键](docs/shortcuts.md)、
[模型服务](docs/model-providers.md)和[隐私边界](docs/privacy.md)。

## 本机开发起步

在仓库根目录执行。Linux 原生构建需要 Rust 1.95.0 和 IBus、图形库等开发依赖；
完整依赖清单以 [CI 工作流](.github/workflows/ci.yml)为准。

```sh
cargo build --locked --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
cargo fmt --all --check
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features -- --test-threads=1
```

依赖已经缓存时可以额外加 `--offline`；缓存缺失不应通过去掉 `--locked` 解决。
完整测试使用单线程，避免共享进程状态的 FFI 测试互相干扰。
以上命令不注册或启用个人输入法。原生、图形和应用工作流请使用
[测试手册](docs/development/testing.md)中的私有会话入口，不向日常窗口模拟输入。

## 文档维护原则

- README 只放简介、下载、快速开始和导航；日常说明维护在对应指南，不复制整段更新记录。
- 架构和开发手册描述当前契约；功能变化要同时更新实际受影响的说明与测试入口。
- 版本说明写入 `docs/releases/<版本>.md`，区分源码变化、本地验证和准确提交的远端验收。
- 调查过程、失败证据和旧说明保留在历史记录，通过链接引用，不当作当前操作指南。
- 不以本地测试、合成模型或编译检查推导真实模型质量、个人桌面验收或其他平台的运行支持。
- 移动或压缩文档时同步检查相对链接、打包清单和功能证据索引；已发布标签与安装包不改写。

## 历史记录

[整理前开发记录](docs/history/2026-10/pre-reorganization/development.md)保留原始开发流水，
[整理前词库说明](docs/history/2026-10/pre-reorganization/lexicons.md)保留各批次的原始说明。
其中日期、数量、命令和验证范围属于当时快照，不覆盖当前指南。
历史版本变化见 [发布记录目录](docs/releases/)。
