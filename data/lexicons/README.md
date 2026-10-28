# Suzaku 内置词库

内置词库提供不依赖模型的英文补全、中文拼音与日文罗马音候选，是模型不可用时的本地回退。
这里只保存有序词句数据；转换、排序和匹配由语言实现负责，不绑定 LLaMA 或任何具体服务。

## 当前资源

以下为 0.10.0 源码内置数据，不含可选词包或个人偏好：

| 语言 | 文件 | 词形或读音 | 语境 | 完整候选句 |
| --- | --- | ---: | ---: | ---: |
| 英文 | [en.json](en.json) | 6,216 个规范化索引词形 | 968 | 1,900 |
| 简体中文 | [zh-Hans.json](zh-Hans.json) | 3,442 条读音记录 | 959 | 1,916 |
| 日文 | [ja.json](ja.json) | 638 条读音记录 | 447 | 893 |

英文词形数包含各层词、下一词和句子投影后的去重结果，不等于 `words` 数组条目和。
中文与日文的读音记录不是去重汉字数；同音不同文本按顺序保留。
这些是有限原创预设，不代表完整词典、自动纠错或任意句子的覆盖率。

## 格式与顺序

资源为 UTF-8 JSON，必填 `format_version: 1` 和 `language`，未填写的顶层数组默认空。
独立英文资源示例：

```json
{
  "format_version": 1,
  "language": "en",
  "word_layers": [{
    "id": "example",
    "words": ["hello", "world"],
    "next_words": [["hello", ["world"]]],
    "sentences": ["hello world."]
  }]
}
```

- 英文 `word_layers` 保存显式词形、下一词语境和完整句子；`phrase_endings` 单独保存句尾。
- 中文/日文 `readings` 包含 `reading`、`text`、`kind` 和可选 `require_separators`；`kind` 为 `word` 或 `sentence`。
- 中文读音用 ASCII 撇号分音节，如 `xi'an`；日文使用平假名，如 `にほんご`，不在资源里写罗马音。
- `continuations` 是 `[语境, [完整句子…]]`，句子必须保留语境前缀，不是只填补尾。
- 层、读音、语境和值的数组顺序影响优先级，不字母排序、不去重历史权重锚点，不重录历史指纹。

英文已有 64 层，达到单资源上限；新内容复用合适的现有层，不能照示例再追加第 65 层。
扩充时必须保护旧词的首次索引位置，中文/日文保留原同音顺序、类型与分隔标记。
格式校验由 [lexicon.rs](../../src/lexicon.rs)负责；完整约束与流程见 [词库维护](../../docs/development/lexicons.md)。

## 加载与扩充

内置 JSON 经 `include_str!` 编入程序，首次使用时校验并缓存；修改后需要重编译。
输入路径不读词库文件或联网，增加已有语言的数据不需要修改模型协议。
增加一种语言标签也不会自动提供转换器，仍需实现和注册语言规则。

可选内容使用独立的 [离线词包](../offline-packs/README.md)，当前推荐目录有 25 包。
包格式、安装预算和冲突处理见 [开发规范](../../docs/development/lexicons.md)，
作者工具见 [小型 SDK](../../sdk/offline-packs/README.md)。安装或启停后需正常重启宿主与面板，
不热替换正在输入的草稿，也不自动安装整个推荐目录。

## 验证与历史

[数据契约回归](../../tests/lexicon_resources.rs)保护格式和历史顺序；
[词库维护](../../docs/development/lexicons.md)提供质量测试与原生闭环入口。
新增候选要同时验证实际可见、采用、撤回、续写和精确提交，不能只更新总数断言。

各批次原始说明和旧数量完整保存在 [整理前词库记录](../../docs/history/2026-10/pre-reorganization/lexicons.md)。
当前说明不再累加历史流水，版本变化见 [发布记录](../../docs/releases/README.md)。
