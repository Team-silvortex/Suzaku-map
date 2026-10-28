# 历史与审计

这里保存开发过程和当时的验证证据，不作为当前安装或使用步骤。
当前说明见 [文档目录](../README.md)，版本变化与实际发行见 [版本目录](../releases/README.md)。

[2026-10-28 文档整理记录](2026-10/documentation-reorganization.md)说明本轮迁移与验证范围。

## 文档整理前的完整记录

2026-10-28 从提交 `e3ce76e1620fd52ece28d2fe88dedd2bee0e1938` 归档了以下入口，
移除了它们与现行指南之间的重复正文。记录中的日期、失败、测试数量和限制原样保留；
只机械重定位 Markdown 链接，重定位清单可逆恢复原始 Git 字节。

- [原项目 README](2026-10/pre-reorganization/project-readme.md)：发布回执和重复批次摘要。
- [原开发流水](2026-10/pre-reorganization/development.md)：历次开发、测试、失败和平台探索。
- [原词库扩充记录](2026-10/pre-reorganization/lexicons.md)：各批数据、排序指纹及回归范围。
- [原功能网络记录](2026-10/pre-reorganization/functional-network.md)：历次网络审计与功能扩充。
- [原链路图](2026-10/pre-reorganization/functional-network.mmd)与[原张量](2026-10/pre-reorganization/functional-tensor.json)。
- [归档来源与校验清单](2026-10/pre-reorganization/manifest.json)：原路径、来源提交、原始/归档哈希与链接重定位记录。

当前张量中的历史 evidence 路径指向这些归档，而不是已瘦身的入口文件。
历史文字中的“当前”“未提交”“待验收”仅指记录当时；不要把不同运行、局部重试或子进程结果相加。
临时目录可能随系统清理消失，保留的失败记录不因后续通过而删除。

## 问题调查

审计文件暂保留原地址，避免既有提交、发布说明和安装包引用失效。

| 主题 | 代表记录 |
| --- | --- |
| 安装、注册和数据 | [打包与注册](../bug-audit-packaging-2026-09-13.md) · [0.7.0 安装](../install-audit-0.7.0-2026-09-27.md) · [0.8.6 安装](../install-audit-0.8.6-2026-10-11.md) |
| 连续输入、草稿和候选 | [采用后的续写](../bug-audit-adopted-continuation-2026-09-27.md) · [句子进度](../bug-audit-sentence-progress-2026-09-27.md) · [中英链路](../bug-audit-bilingual-core-2026-09-24.md) |
| 模型请求和异步取消 | [模型延迟](../bug-audit-model-latency-2026-09-24.md) · [取消](../bug-audit-model-cancellation-2026-09-24.md) |
| 应用、焦点和恢复 | [跨应用](../bug-audit-cross-apps-2026-09-23.md) · [生命周期](../bug-audit-app-lifecycle-2026-09-23.md) · [服务恢复](../bug-audit-service-recovery-2026-09-24.md) |
| 人机交互与设置 | [人机工程](../ergonomics-audit-2026-09-25.md) · [快捷键焦点](../bug-audit-shortcut-focus-2026-09-24.md) · [设置 I/O](../bug-audit-settings-io-2026-09-27.md) |

其他 `bug-audit-*`、`install-audit-*` 和 `ci-*-followup-*` 文件也属于历史证据。
新增调查按年月放入本目录，并从相关功能 evidence 链接；不要再复制到 README、词库说明和功能网络。
