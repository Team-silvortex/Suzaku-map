# 开始使用 Suzaku

Suzaku 的普通输入流程是“输入草稿 → 采用词句并继续修改 → 明确提交”。
本页以 Ubuntu 24.04 amd64 / glibc 2.39 / IBus、X11 或 GNOME/XWayland 为基线。
先保留一个可用的系统输入法作为回退；完整原生 Wayland 和其他平台尚未完成同等验收。

## 安装与升级

从 [0.10.0 Linux Alpha](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0)
下载同一版本的 `.deb` 和 `.sha256` 文件，在下载目录执行：

```sh
sha256sum -c suzaku_0.10.0_amd64.deb.sha256
sudo apt install ./suzaku_0.10.0_amd64.deb
```

升级前完成或取消正在输入的草稿，并从托盘完全退出 Suzaku；更新后重新打开面板。
安装本身不注册输入源、不启动个人服务，也不切换当前输入法。
压缩包、卸载、备份和恢复步骤见 [Linux 打包与数据管理](../linux-packaging-data.md)。

## 首次注册与激活

需要作为输入法使用时，在当前桌面用户会话执行以下命令，**不要 sudo**：

```sh
/usr/bin/suzaku-tool linux-register install
/usr/bin/suzaku-tool linux-register verify
/usr/bin/suzaku-panel
```

注册会添加 Suzaku 输入源和用户宿主服务，并保留原活动输入法。
从托盘激活 Suzaku 后，在目标应用的普通文本框开始输入。
GNOME 需要桌面托盘支持；看不到图标时先检查托盘支持，不要重复注册。
释放或正常退出时恢复先前输入法；如果你已手动切换，Suzaku 不应覆盖你的新选择。
遇到恢复失败时按托盘提示重试，不要反复注册或更改系统键盘布局。

## 写第一段文字

输入英文前缀，采用词候选后继续按空格、补词或选择句候选。
采用的内容仍是草稿，目标应用的正文在明确提交前不变。

| 操作 | 普通非空草稿中的行为 |
| --- | --- |
| `1`–`6` | 采用当前页对应候选，仍可继续修改 |
| `Alt+0`–`9` 或数字小键盘 | 输入字面数字 |
| `Space` 或标点 | 继续草稿，不自动提交 |
| `Tab` / `Shift+Tab`、方向键 | 移动候选选择 |
| `Shift+Enter` | 采用当前候选，不提交 |
| 补全后立即按 `Backspace` | 撤回最近一次采用，恢复之前拼写 |
| 英文 `Ctrl+Backspace` | 删除最近的空白分隔草稿词 |
| `Enter` 或候选左键点击 | 明确提交；点击不带应用快捷键修饰 |
| `Esc` | 取消草稿；未完成 Compose 时先只取消组合 |

没有草稿时，普通数字直接进入应用。对应用明确报告的密码/PIN、数字、十进制和电话字段，不接管这些候选操作。
仅在视觉上掩码不等于应用已报告隐私用途，详见下方隐私说明。
候选可分页；完整按键、点击和隐私字段规则见 [IBus 输入与候选](../ibus-candidates.md)。

## 切换语言与面板

默认 `Ctrl+Shift+Space` 在已激活 Suzaku 的普通 IBus 输入框中切换英文与中文，并保留草稿。
日语模式按此组合进入英文；日语也可从托盘选择。自定义语言组合会替换默认组合，清除自定义后恢复。
这不是系统全局激活快捷键；桌面或应用若先截获它，可使用托盘。

面板有焦点且无 IME 预编辑时，`F1` 打开“设置 → 快捷键”，`Ctrl+,` 打开设置。
“设置 → 外观 → 面板布局”提供自动、跟随光标和底部布局。
自动布局优先考虑平台信息，显式保存的布局会覆盖自动选择；底部布局不移动其他应用。
详见 [快捷键](../shortcuts.md)和[界面语言](../interface-languages.md)。

## 使用模型与本地偏好

不用模型也能使用有限的本地作者词库。模型候选需要显式开启；发现功能只检查已运行的本机服务，
不会替你安装或启动 Ollama、llama.cpp 或其他模型服务器：

```sh
/usr/bin/suzaku-tool model discover
/usr/bin/suzaku-tool model status
```

本地服务也可能转发数据，请自行信任和配置服务端。云端使用需要明确同意，并可能产生费用。
Linux 候选偏好默认保存在本机，可从托盘清除；偏好摘要不等于加密或匿名化。
查看 [模型配置](../model-providers.md)、[隐私边界](../privacy.md)和[已知限制](../known-limitations.md)。
