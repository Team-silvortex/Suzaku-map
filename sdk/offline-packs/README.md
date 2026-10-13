# 离线词库包制作小型 SDK

Python **3.10+**、标准库即可，不需要 pip、模型或网络。Python 不随 SDK 打包，
只在制作包时需要；消费生成的 JSON 不需要 Python。SDK 不实现另一套输入转换算法，
权威格式、语言与预算校验仍由你明确指定的 Suzaku 工具完成。

- [suzaku_pack.py](suzaku_pack.py)：`PackBuilder` API，以及已有 JSON 的校验/重新序列化入口。
- [example.py](example.py)：项目编写的中文园艺示例，不是园艺专业词典。

输出只有数据，不附带脚本或动态库，也不要加入个人输入记录或无权分发的第三方语料。

## 选择当前工具和 SDK 路径

以下命令分别在源码根目录、解压的二进制 tar 包根目录和已安装 `.deb` 的环境执行。
`./garden.json` 必须不存在，父目录必须已经存在；不要在只读系统目录中生成文件。

```sh
# 源码：先按项目构建说明取得工具；此命令本身不构建程序。
python3 sdk/offline-packs/example.py ./garden.json ./target/debug/suzaku_tool

# 二进制 tar 包：从解压后的包根目录运行。
python3 share/suzaku/offline-pack-sdk/example.py ./garden.json ./bin/suzaku_tool

# .deb：在你有写权限的工作目录运行，不要 sudo。
python3 /usr/share/suzaku/offline-pack-sdk/example.py ./garden.json /usr/bin/suzaku-tool
```

这是三选一的入口示例，不要对同一个输出文件连续执行三条命令。
API/命令行默认校验器名为 `suzaku_tool`；`.deb` 的可执行名是 `suzaku-tool`，
因此建议始终像上面一样传入确切路径，避免误用 PATH 中的旧工具。

将自己编写的 UTF-8 JSON 校验并序列化成新文件：

```sh
python3 /usr/share/suzaku/offline-pack-sdk/suzaku_pack.py ./authored.json ./distribution.json --tool /usr/bin/suzaku-tool
/usr/bin/suzaku-tool pack inspect ./distribution.json
```

当前开发版 `pack inspect` 只读显示 manifest、原始声明记录统计及格式校验结果；
旧版工具可使用 `pack validate`。统计不代表去重后的新词数，`indexing_units` 仅对应
既有资源预算。校验通过不代表来源可信、许可或签名已验证，也不检查与已启用包的兼容性。

## Python API

从同目录导入 `PackBuilder`；若在自己的脚本目录编写，可将 SDK 模块复制到该目录。
方法按调用顺序追加数组，**顺序影响候选优先级，不要排序历史条目**。

| 方法 | 内容 |
| --- | --- |
| `PackBuilder(...)` | 指定 ID、语言、名称、说明、主题、作者、许可；版本默认 `1.0.0` |
| `words(layer, words, next_words=..., sentences=...)` | 英文显式词形、下一词搭配及完整句子 |
| `reading(reading, text, kind="word", require_separators=False)` | 中文分音节拼音或日文平假名读音，以及目标文字 |
| `continuation(context, *sentences)` | 中日语境续句，每条完整句子必须以语境开头 |
| `phrase_ending(context, *endings)` | 英文句尾；填写补尾而非完整句子 |
| `build(destination, tool=...)` | 校验并写到不存在的新文件；不会安装或启用 |

`topics`、`authors`、`words`、`sentences` 要传列表、元组或生成器，即使只有一项也不能
直接传单个字符串。`next_words` 是 `(context, [choices])` 的集合，例如
`[("good", ["morning", "evening"])]`；每组的候选也必须是集合。相对地，
`continuation(context, "完整句一", "完整句二")` 和
`phrase_ending(context, "补尾一", "补尾二")` 接受逐个文本参数；已有列表可用 `*列表` 展开。

一个包只对应 `en`、`zh-Hans` 或 `ja` 中一种语言。语言支持由既有转换器决定，
增加别的语言标签不会自动得到新的输入法。主题按相关内容组织，可分别发布不同语言版本。

中文读音示例为 `yu'miao'pan`（用 ASCII 撇号分音节，`v` 表示 ü）；日文读音使用
平假名，如 `しおり`，不直接填写罗马音。`require_separators` 只适用于中文。
英文词形显式填写，句子使用完整文本；中日 `continuation` 不是只填写句尾。
API 不代替语言规则校验，调用不适用于所选语言的方法会在 `build` 校验时被拒绝。

## 发布与安装是两件事

SDK 先在输出同目录创建私有临时文件，调用指定工具的 `pack validate`；成功后原子
无覆盖发布。已有输出、校验器缺失/失败或超时都不会覆盖目的文件，不发布半成品。
目录须由你预先准备；不承诺跨文件事务，也不会自动签名、联网分发或注册个人词包。

需要试用生成的 JSON 时，另行明确执行 `pack install FILE`。新包默认启用，更新同 ID
需 `--replace`；安装/启用才检查与当前活动包的冲突。`pack list`/`pack preview` 表示
下次进程启动状态，已运行宿主与面板要正常退出后重新启动，不是热更新。
单包上限为 256 KiB、8 层和 2,048 索引单元；每语言启用扩展最多 4,096 单元、
最多安装 16 包，包括已禁用的包。可选目录数量可以更多，不能把整个目录当成可同时
安装的组合；制包或独立格式校验成功也不会预留安装名额。以当前工具的实际校验结果
为准，不以文件大小推断候选覆盖或质量。
