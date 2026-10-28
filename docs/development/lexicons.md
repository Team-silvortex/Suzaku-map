# 词库维护

内置词库提供模型关闭、缺失、失败或超时时仍可使用的有限候选。
数据与语言转换、平台宿主和模型服务分开维护；补充词句不需要绑定某个 LLaMA 模型。
当前数量与资源入口集中在 [内置词库](../../data/lexicons/README.md)，可选主题见 [推荐词包](../../data/offline-packs/README.md)。

## 代码与数据入口

- [lexicon.rs](../../src/lexicon.rs)：资源格式、校验、嵌入资源与活动快照。
- [en.json](../../data/lexicons/en.json)、[zh-Hans.json](../../data/lexicons/zh-Hans.json)、[ja.json](../../data/lexicons/ja.json)：三语内置数据。
- [english.rs](../../src/languages/english.rs)、[chinese.rs](../../src/languages/chinese.rs)、[japanese.rs](../../src/languages/japanese.rs)：索引、匹配与转换规则。
- [packs](../../src/lexicon/packs.rs)：数据包格式、注册表、合并与启动快照。
- [制包 SDK](../../sdk/offline-packs/README.md)：Python 标准库作者工具，不是另一套转换器。

## 资源格式 v1

资源是 UTF-8 JSON，必填 `format_version: 1` 和 `language`，其余顶层数组默认空。
未知字段或版本、错误类型、重复层 ID、重复语境、空白污染和控制字符会报错，不静默丢弃。
以下是一个独立英文资源示例，不是给已满层数的内置文件追加第 65 层：

```json
{
  "format_version": 1,
  "language": "en",
  "word_layers": [{
    "id": "example",
    "words": ["hello", "world"],
    "next_words": [["hello", ["world"]]],
    "sentences": ["hello world."]
  }],
  "phrase_endings": [["hello", ["world"]]]
}
```

`word_layers` 按数组顺序提供词形、下一词搭配和完整句子，`id` 只标识数据层。
英文实现依次投影每层的 `words`、`next_words` 候选、`sentences` 单词，首次出现位置影响基础权重。
`phrase_endings` 填 `[语境, [补尾…]]`，其值不是完整句子。

中文和日文使用 `readings` 与 `continuations`：

```json
{
  "format_version": 1,
  "language": "zh-Hans",
  "readings": [{
    "reading": "xi'an",
    "text": "西安",
    "kind": "word",
    "require_separators": true
  }],
  "continuations": [["西安", ["西安我们下次一起去。"]]]
}
```

`kind` 必填，只能为 `word` 或 `sentence`，不能按某个特定文本硬编码类型。
中文使用小写拼音与 ASCII 撇号分音节，`v` 表示 ü；日文资源使用完整平假名读音，不填写罗马音。
`require_separators` 默认 `false`，由相应转换器解释；日文包不支持此标记。
`continuations` 的每条值是保留语境前缀的完整句子，不要只写句尾。

## 顺序与兼容性

数组顺序就是数据契约的一部分，不按字母排序，不自动去重历史重复词形。
历史重复项可能是权重锚点；同音项允许不同文本，但同一读音与文本组合不能重复。
同一语境不能跨层再次声明；扩展既有语境要明确保护原值顺序，不能用另一层暗中覆盖。

英文内置资源已有 64 层，达到单资源层数上限。新增内容复用适合的既有层，
同时验证句子投影没有引入未预期词形，也没有改变任何旧词的首次位置。
当前英文索引是 6,216 个规范化词形，不是旧开发记录里的 6,144；不得复制旧门槛当成当前统计。
中文与日文的新读音追加到表尾，保留所有旧同音顺序、类型和分隔标记。

正常扩充应保持旧条目的内容、次序和评分。历史指纹、冻结向量和严格逆变换不能被重录成新数据来“修绿”。
有意修正旧条目时应单独说明兼容影响，补充能在改动前失败、改动后通过的测试。
匹配窗口、解码束宽和候选页数属于实现预算，不能随词库扩充顺便放宽。

## 数据预算

| 对象 | 当前限制 |
| --- | --- |
| 单内置资源 | 2 MiB、最多 64 层、单文本最多 4,096 字符 |
| 单离线包 | 256 KiB、最多 8 层、1–2,048 索引单元 |
| 已安装包 | 最多 16 个，禁用包仍占名额 |
| 每语言启用扩展 | 合计最多 4,096 索引单元 |

索引单元是保守工作预算，包括句子投影；不是包内词形数、去重净增词数或可见候选数。
推荐目录现有 25 个包，不代表能全部同时安装，也不代表全部已做个人桌面或原生应用验收。
具体约束由 [包格式校验](../../src/lexicon/packs/format.rs)与 [catalog 合并](../../src/lexicon/packs/catalog.rs)执行。

## 新增内容流程

1. 明确场景、语言和词候选/句候选两种目标；先检查已有读音、语境、同义表达和冲突。
2. 使用项目有权分发的原创内容，避免个人输入历史、账号、实时信息或第三方受限语料。
3. 按既有顺序追加或扩展明确的语境；同时记录声明条目数、去重词形和索引预算，不能混用。
4. 写固定输入与预期候选，覆盖短前缀、半词/半读音、完整读音、长草稿、字面空白和标点。
5. 验证采用、撤回、Space 续写与精确提交，检查候选实际可见和可分页，不能只断言数组里有文本。
6. 用历史指纹或严格逆变换证明旧内容与顺序保持；再跑相关质量集及完整回归。
7. 更新当前资源统计或包目录与必要的版本说明，不把每批流水再次复制到多个入口文档。

## 回归入口

在仓库根目录运行：

```sh
cargo test --locked --all-features --test lexicon_resources -- --test-threads=1
cargo test --locked --all-features --test english_completion_quality --test chinese_completion_quality --test offline_vocabulary_quality -- --test-threads=1
cargo test --locked --all-features --test fallback_vocabulary --test long_draft_completion -- --test-threads=1
cargo test --locked --all-features --test offline_packs --test offline_pack_quality -- --test-threads=1
cargo test --locked --all-features --test work_clarification_dialogue --test day_coordination_followup -- --test-threads=1
python3 -B scripts/test-offline-pack-shipping.py
```

这些是常用入口，不是所有场景的测试清单。补充哪一批内容，就运行对应 integration suite，
并保留原有冻结基线；准备版本时用 [完整测试流程](testing.md)覆盖全部回归。
原生闭环可另运行 `bash scripts/test-linux-ci.sh ibus` 和 `bash scripts/test-linux-apps.sh vocabulary`，
它们在私有会话里验证，不操作个人输入法。合成提供器错误不等于真实网络或大模型质量测试。

## 启动快照与词包

内置资源修改后需要重编译。词包是独立的数据包，可用 CLI 显式安装、启停、替换和移除；
宿主及面板启动前加载并冻结快照，运行中不热更新，正常退出后重启两个进程才加载新组合。
解析 `Lexicon::from_json` 本身不会读个人配置、替换运行快照或连接模型。

内置层、同音顺序和词权重保留，扩展只追加；相同重复项跳过，冲突项拒绝。
启动时单包损坏会隔离跳过，注册表整体损坏则回退内置资源并记录错误。
独立库调用方通过 `packs::initialize` 显式初始化；首次查询后不能重新初始化。

使用与制作方法见 [包 SDK](../../sdk/offline-packs/README.md)及 [Linux 数据管理](../linux-packaging-data.md)。
各批次旧统计和调查过程完整保留在 [整理前词库记录](../history/2026-10/pre-reorganization/lexicons.md)，
其中旧包数、预算描述和验证成绩不作为当前状态。
