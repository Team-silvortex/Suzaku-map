# 可选离线词库包

这些包是项目编写、MIT 许可的有限日常词句集合，用于模型不可用时的本地兜底。
一个 JSON 就是一个独立的数据包；不会执行脚本、加载动态库、下载模型或读取个人输入历史。
**随程序提供不等于已经安装或启用**。内置词库始终保留，下面的包按需选择。

## 按语言和主题选择

当前开发目录共 **25 包：英文 8、简体中文 8、日文 9**，包版本均为 `1.0.0`。
观星、学习写作、烹饪备餐、旅行住宿、工作协作、家庭生活、购物和互联网日用八个主题已有中英日集合，
日文另有铁路包；主题齐备不代表词汇量或输入能力完全相同。
`outdoors` 目前只是观星相关的小集合，不是通用户外词典。
`travel` 标签可筛出中英住宿出行包，以及日文铁路与住宿两包；
只找住宿请用 `lodging`，具体范围以每行说明为准。
`home` 包括烹饪和家庭生活；只找烹饪请用 `cooking`。

<!-- pack-catalog:start -->
| 语言 | 包文件 | ID | 版本 | 主题标签 | 范围 |
| --- | --- | --- | --- | --- | --- |
| `en` | [en-outdoors.json](en-outdoors.json) | `org.suzaku.en.outdoors` | `1.0.0` | `outdoors`, `astronomy` | 英文观星与观测用语 |
| `zh-Hans` | [zh-Hans-outdoors.json](zh-Hans-outdoors.json) | `org.suzaku.zh-hans.outdoors` | `1.0.0` | `outdoors`, `astronomy` | 中文观星与观测用语 |
| `ja` | [ja-rail.json](ja-rail.json) | `org.suzaku.ja.rail` | `1.0.0` | `travel`, `rail` | 日文铁路出行 |
| `en` | [en-study.json](en-study.json) | `org.suzaku.en.study` | `1.0.0` | `study`, `writing` | 英文学习与写作 |
| `zh-Hans` | [zh-Hans-study.json](zh-Hans-study.json) | `org.suzaku.zh-hans.study` | `1.0.0` | `study`, `writing` | 中文学习与写作 |
| `en` | [en-cooking.json](en-cooking.json) | `org.suzaku.en.cooking` | `1.0.0` | `cooking`, `home` | 英文烹饪与备餐 |
| `zh-Hans` | [zh-Hans-cooking.json](zh-Hans-cooking.json) | `org.suzaku.zh-hans.cooking` | `1.0.0` | `cooking`, `home` | 中文烹饪与备餐 |
| `en` | [en-travel.json](en-travel.json) | `org.suzaku.en.travel` | `1.0.0` | `travel`, `lodging` | 英文旅行与住宿 |
| `zh-Hans` | [zh-Hans-travel.json](zh-Hans-travel.json) | `org.suzaku.zh-hans.travel` | `1.0.0` | `travel`, `lodging` | 中文旅行与住宿 |
| `en` | [en-work.json](en-work.json) | `org.suzaku.en.work` | `1.0.0` | `work`, `collaboration` | 英文工作与协作 |
| `zh-Hans` | [zh-Hans-work.json](zh-Hans-work.json) | `org.suzaku.zh-hans.work` | `1.0.0` | `work`, `collaboration` | 中文工作与协作 |
| `en` | [en-family.json](en-family.json) | `org.suzaku.en.family` | `1.0.0` | `family`, `home` | 英文家庭生活与家务分工 |
| `zh-Hans` | [zh-Hans-family.json](zh-Hans-family.json) | `org.suzaku.zh-hans.family` | `1.0.0` | `family`, `home` | 中文家庭生活与家务分工 |
| `ja` | [ja-family.json](ja-family.json) | `org.suzaku.ja.family` | `1.0.0` | `family`, `home` | 日文家庭生活与家务分工 |
| `en` | [en-shopping.json](en-shopping.json) | `org.suzaku.en.shopping` | `1.0.0` | `shopping`, `errands` | 英文购物、试穿、收据与换货询问 |
| `zh-Hans` | [zh-Hans-shopping.json](zh-Hans-shopping.json) | `org.suzaku.zh-hans.shopping` | `1.0.0` | `shopping`, `errands` | 中文购物、试穿、收据与换货询问 |
| `en` | [en-internet.json](en-internet.json) | `org.suzaku.en.internet` | `1.0.0` | `internet`, `digital` | 英文浏览搜索、收藏传输、登录状态与社交分享 |
| `zh-Hans` | [zh-Hans-internet.json](zh-Hans-internet.json) | `org.suzaku.zh-hans.internet` | `1.0.0` | `internet`, `digital` | 中文浏览搜索、收藏传输、登录状态与社交分享 |
| `ja` | [ja-internet.json](ja-internet.json) | `org.suzaku.ja.internet` | `1.0.0` | `internet`, `digital` | 日文浏览搜索、收藏传输、登录状态与社交分享 |
| `ja` | [ja-shopping.json](ja-shopping.json) | `org.suzaku.ja.shopping` | `1.0.0` | `shopping`, `errands` | 日文购物、试衣、尺寸颜色、包装与订货询问 |
| `ja` | [ja-travel.json](ja-travel.json) | `org.suzaku.ja.travel` | `1.0.0` | `travel`, `lodging` | 日文酒店、入住退房、行李与房间设施 |
| `ja` | [ja-work.json](ja-work.json) | `org.suzaku.ja.work` | `1.0.0` | `work`, `collaboration` | 日文会议文档、进度、审批与工作交接 |
| `ja` | [ja-study.json](ja-study.json) | `org.suzaku.ja.study` | `1.0.0` | `study`, `writing` | 日文课堂、学习与写作交流 |
| `ja` | [ja-cooking.json](ja-cooking.json) | `org.suzaku.ja.cooking` | `1.0.0` | `cooking`, `home` | 日文厨具、食材与备餐交流 |
| `ja` | [ja-outdoors.json](ja-outdoors.json) | `org.suzaku.ja.outdoors` | `1.0.0` | `outdoors`, `astronomy` | 日文夜空、观星与观测器材交流 |
<!-- pack-catalog:end -->

这张表对应文件中的 manifest；条目顺序不代表自动安装顺序，也不是推荐强度排名。
包中的词句是输入示例，不提供实时行程、账户查询或专业指导。
购物包只帮助表达需求，不执行购买/退换操作，也不承诺库存、价格或商家退换政策。
互联网包仅提供浏览、搜索、收藏、下载/上传、登录状态和社交分享的有限日常表达，
不会执行网络请求或账号操作，也不会访问浏览历史、文件、消息或账号内容。
日文包沿用现有罗马音与假名输入能力；有限词句补全并不等于完整的日语形态分析或任意长句转换。
当前源码支持 `thi → てぃ`：住宿包的 `アメニティ` 可输入 `amenithi` 或半截 `amenith`，
原来的 `amenitexi`/`ameniteli` 仍可用，`ti → ち` 没有改变。
已有预设句可以接住末尾的未完成音节，例如 `引用sh`、`引用shit`；
单个 `引用n` 也能提示「引用の出典をもう一度確かめます。」，而主候选仍保留「引用ん」。
`nn`、`xn`、`n'` 等明确鼻音写法不会被这样重解释；分隔开的 `s h` 不会再被拼成 `sh`。
这不是任意整句罗马音预测，也不保证所有句子都排在第一页。
**25 个可选包不等于可以同时安装 25 个**：最多安装 16 包的现有上限不变，
禁用仍占安装名额。请按需选择；满额时先保留原 JSON，再明确移除不需要的包，才能安装新 ID。
这里的开发目录不表示旧版工具或已经发布的安装包包含新增内容。

## 查找、查看和安装

下列命令以 `.deb` 安装的 `/usr/bin/suzaku-tool` 为例，不要使用 `sudo` 管理个人词包。
源码构建时替换为 `./target/debug/suzaku_tool`；在解压后的二进制 tar 包根目录，
替换为 `./bin/suzaku_tool`。三种方式应使用同一个桌面用户和词包数据目录。

```sh
/usr/bin/suzaku-tool pack catalog --language ja --topic family
/usr/bin/suzaku-tool pack catalog --details --language en --topic study
/usr/bin/suzaku-tool pack catalog --details --language zh-Hans --topic internet
/usr/bin/suzaku-tool pack catalog --details --language ja --topic internet
/usr/bin/suzaku-tool pack catalog --details --language ja --topic shopping
/usr/bin/suzaku-tool pack catalog --details --language ja --topic lodging
/usr/bin/suzaku-tool pack catalog --details --language ja --topic work
/usr/bin/suzaku-tool pack catalog --details --language ja --topic study
/usr/bin/suzaku-tool pack catalog --details --language ja --topic cooking
/usr/bin/suzaku-tool pack catalog --details --language ja --topic astronomy
/usr/bin/suzaku-tool pack export org.suzaku.en.study ./en-study.json
/usr/bin/suzaku-tool pack inspect ./en-study.json
/usr/bin/suzaku-tool pack validate ./en-study.json
/usr/bin/suzaku-tool pack install ./en-study.json
/usr/bin/suzaku-tool pack list
/usr/bin/suzaku-tool pack preview en "please annotate this para"
```

`catalog` 是此工具内置的可选包目录，不是已安装列表；旧版二进制的目录可能较小。
当前开发版新增 `--details`，返回 `manifest` 与 `stats`；默认输出仍是 manifest 数组。
`inspect FILE` 查看尚未安装的本地 JSON，返回 `format_version`、`manifest`、`stats`
及 `validation`，沿用现有格式和读音规则校验，不读取注册表、不安装、不联网上传。
`stats` 是包内声明记录数，不去重；`indexing_units` 是现有预算的索引单元计数，
不是唯一词数或输入质量分数。旧版工具不支持新查看命令时仍可用 `pack validate`。

**inspect/validate 成功不代表签名、来源、许可已审核，也不证明与当前启用包兼容。**
组合冲突在实际安装/启用时检查；新包默认启用，同 ID 更新必须明确 `--replace`。
`export` 不覆盖现有文件。安装包附带的同目录 JSON 也可直接交给 `inspect`/`install`，
无需先导出；保留分发原文件以便恢复。

```sh
/usr/bin/suzaku-tool pack disable org.suzaku.en.study
/usr/bin/suzaku-tool pack enable org.suzaku.en.study
/usr/bin/suzaku-tool pack install ./en-study.json --replace
/usr/bin/suzaku-tool pack remove org.suzaku.en.study
```

`list` 和 `preview` 查看的是**下次新进程启动**会加载的磁盘状态，不代表运行中的宿主
已经更新。完成或取消当前草稿，从托盘完整退出 Suzaku，然后重新打开面板；正常受管
启动会带起宿主。只关闭窗口、只重开设置或只重启面板而保留旧宿主，都不是词库热更新。
独立启动的非受管宿主需由其启动器正常重启；不要为加载词包强杀输入服务。

包管理遵循 `XDG_DATA_HOME`，默认注册表为 `~/.local/share/suzaku/lexicons/registry.json`；
自定义 `SUZAKU_LEXICON_DIR` 必须为绝对路径，宿主、面板与管理命令需一致。
个人词包不进入配置备份，也不会随卸载程序被删除。此目录是分发样例，不是个人注册表。
