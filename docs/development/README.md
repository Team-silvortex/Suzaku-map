# Suzaku 开发手册

这里维护当前代码的职责、开发流程和验证入口。主线是 Linux 上的连续输入体验；
英文补全、中文拼音和日文罗马音共用引擎，离线词库与模型适配器分别演进。

## 按任务阅读

| 任务 | 入口 |
| --- | --- |
| 第一次构建和检查 | [构建与测试](testing.md) |
| 找到输入或候选链的所有者 | [架构与模块边界](architecture.md) |
| 扩充内置词句、维护旧排序 | [词库维护](lexicons.md) |
| 制作可选主题包 | [离线包 SDK](../../sdk/offline-packs/README.md)、[推荐目录](../../data/offline-packs/README.md) |
| 适配模型服务或预测后端 | [模型服务与预测协议](../model-providers.md) |
| 检查功能路由和已有证据 | [功能链条与成熟度](../functional-network.md) |
| 准备 Linux 分发包或发布 | [打包与数据管理](../linux-packaging-data.md) |

## 先明确边界

普通构建和静态测试不需要注册输入法、修改个人设置或运行模型。
原生输入回归使用脚本创建的 Xvfb、D-Bus 和 XDG 目录；安装回归使用受控容器。
个人桌面试用、实际模型请求和正式发布是单独的操作，不混入默认测试步骤。

当前分发定位为 Linux Alpha。Windows 和 macOS 的 CI 编译检查不等于原生输入验收；
Android、XR 和平板相关代码也不表示已有同等成熟度的发行版。
具体支持范围见 [已知限制](../known-limitations.md)。

## 保持文档可维护

操作指南只保留当前方法，版本变化写入 `docs/releases/`。
旧调查和每批扩充证据通过 [历史记录](../history/2026-10/pre-reorganization/development.md)引用，
不要再把同一批日志复制到 README、开发手册、词库说明和功能网络四处。
新增事实应落在一个权威页面，其他入口只链接过去。

返回 [开发入口](../../DEVELOPMENT.md)或[项目首页](../../README.md)。
