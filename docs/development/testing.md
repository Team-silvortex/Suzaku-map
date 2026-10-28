# 构建与测试

在仓库根目录运行以下命令。Linux 是当前原生输入与安装包验收的重点；
默认开发检查不注册个人输入法、不切换桌面引擎，也不要求运行模型。

## 环境与构建

[CI 工作流](../../.github/workflows/ci.yml)使用 Rust 1.95.0、Ubuntu 24.04 和已提交的 `Cargo.lock`。
Linux 全功能构建需要 C 编译器、`pkg-config`、IBus/GLib、X11、xkbcommon、Wayland/EGL/Vulkan 开发依赖。
图形和应用回归还需要 D-Bus、Xvfb、CJK 字体、Python GI、GTK、Qt 及工作流列出的测试程序。
以该工作流的依赖清单为准，不用缺失依赖时的跳过结果冒充通过。

```sh
cargo build --locked --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
```

项目把开发与测试 profile 的调试信息设为 0，并关闭增量缓存，以控制磁盘占用。
资源有限时给 Cargo 增加 `-j 2`，不要同时运行多个完整构建。
已缓存全部依赖时可以额外使用 `--offline`；联网取依赖和运行模型是两回事。

## Rust 必须项

```sh
cargo fmt --all --check
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features -- --test-threads=1
```

完整测试保持单线程，因为部分 FFI 测试共享进程状态。
记录原进程退出码、失败项和明确忽略项；不要拼接多个局部重试当成一次完整通过。
默认忽略的 native/UI 项必须通过下列隔离入口另验，不能直接对全部测试加 `--ignored`。
本项目桌面验证使用全功能构建，不把精简 feature 组合的编译结果当作原生验收。

词库相关的较小回归集见 [词库维护](lexicons.md)。改动跨多个模块或准备版本时仍运行完整套件。

## 静态守卫与文档

这些入口检查 CI 图、功能证据、发包目录和测试脚本的结构，不启动个人输入法：

```sh
python3 -B scripts/check-documentation.py
python3 -B scripts/test-documentation.py
python3 -B scripts/test-package-docs.py
python3 -B scripts/test-ci-release-graph.py
python3 -B scripts/functional-tensor.py --check
python3 -B scripts/test-functional-tensor.py
python3 -B scripts/test-offline-pack-shipping.py
python3 -B scripts/test-local-artifacts.py
python3 -B scripts/check-package-audit-links.py .
python3 -B scripts/test-ast-guard.py
python3 -B scripts/run-ast-guard.py test-native-model-fixture.py
```

CI 还分别执行语言、数字键、快捷键、编辑器观察、IBus 借用生命周期等守卫，完整列表随工作流维护。
`run-ast-guard.py` 必须用于工作流指定的 AST 测试，不用导入测试模块代替正式入口。
Shell 检查包括 `shellcheck`；`bash -n` 只能检查语法，不能替代 ShellCheck。

修改功能实现或其测试后，按 [功能网络](../functional-network.md)的规则更新来源摘要和证据。
文档移动还要检查相对链接及安装包静态文档清单。

## 私有 IBus 和图形回归

```sh
bash scripts/test-linux-ci.sh ibus
bash scripts/test-linux-ci.sh ibus-custom-shortcuts
LIBGL_ALWAYS_SOFTWARE=1 bash scripts/test-linux-ci.sh ui
```

脚本创建独立 D-Bus、IBus 地址、XDG 目录及必要的虚拟显示，并设置临时配置与 socket。
它们使用本地合成服务测试模型失败、迟到结果及温度等行为，不下载或启动真实模型。
`ibus` 的完整入口依次运行各私有分区；专用 `ONLY` 环境变量只代表对应专项通过。
`ui` 逐个精确检查 opt-in 测试，并验证实际程序的单实例启动门禁，缺测试不会变成零项成功。

不要移除脚本的私有会话封装、超时、必需项清单或失败传播。
不要把桌面的 `DISPLAY`、`IBUS_ADDRESS`、个人配置或现用 host 目录传进这些测试。
权限不足时保留诊断并解决测试环境，不改用个人会话绕过隔离。

## 真实应用工作流

以下脚本同样在独立显示与会话里创建自己的应用窗口：

```sh
bash scripts/test-linux-apps.sh gtk3
bash scripts/test-linux-apps.sh candidates
bash scripts/test-linux-apps.sh bottom-layout
bash scripts/test-linux-apps.sh auto-layout
bash scripts/test-linux-apps.sh vocabulary
bash scripts/test-linux-apps.sh keyboard
bash scripts/test-linux-apps.sh lifecycle
bash scripts/test-linux-apps.sh qt5
bash scripts/test-linux-apps.sh qt6
```

另有编辑器、浏览器、恢复、设置和数据等专项；准确组合见工作流的 `linux-apps` job。
GTK3、候选定位和布局还分别验证默认及同步 IBus 传输，不用其中一种替代另一种。
默认 `vocabulary` 完整入口运行全部内置词库分区；`packs` 专项目前是历史 14 包子集，
不能据此宣称推荐目录 25 包全都经过原生应用验收。

脚本只向任务拥有的测试窗口注入输入。个人桌面的真实键盘、焦点切换和日常安装试用另行安排，
不能因为用户切回聊天，就把测试窗口失焦解释成输入法故障。

## 打包与干净安装

从本次源码构建到一个新的、任务拥有的输出目录：

```sh
suzaku_package_output=$(mktemp -d /tmp/suzaku-package-check.XXXXXX)
bash scripts/package-linux.sh --output "$suzaku_package_output"
bash scripts/test-linux-package.sh "$suzaku_package_output"
bash scripts/test-linux-install.sh "$suzaku_package_output"
```

打包不安装或启动用户服务。内容测试会运行本次构建的包内诊断程序；最后一步需要 Docker，
在受控 Ubuntu 容器中测试安装、升级和卸载，可能联网取得容器依赖，不挂载个人目录或桌面 socket。
第三方下载包应先只读验证，不能把以上执行型测试当作无风险的归档查看器。
正式发布仍需准确提交的远端 CI、accepted artifact 和独立资产验证，见 [发布要求](../linux-packaging-data.md)。

## 验证范围

合成 HTTP fixtures 能证明传输和交互契约，不能证明真实模型的语言质量或端到端延迟。
真实模型专项需要显式选择已安装模型与服务，并与默认回归结果分开记录。
Windows/macOS CI 的 desktop check 只证明对应目标的编译，不证明原生输入、窗口或安装链已运行。
历史成绩留在 [版本记录](../releases/)与[归档](../history/README.md)，不抄成新提交的通过结果。
