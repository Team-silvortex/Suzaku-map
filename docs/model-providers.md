# 模型服务：本地优先，与具体模型解耦

中、英、日文共用模型无关的 `prediction::PredictionProvider` 候选协议。
`HttpModelProvider` 根据服务协议封装请求，不根据模型名称判断行为。
旧 `LlmCompletionProvider`、`languages::llama` 与 `suzaku-tool llama` 保留兼容入口；
新增适配器实现统一预测协议，不必改动键盘事件、离线词库或语言转换。

## 从设置开始

打开 **设置 → 模型 → 服务 → 配置路径与指南**，可查看本进程解析的配置文件位置和
操作步骤。指南为只读，不读取密钥、修改配置或发送模型请求；它不是地址/模型编辑表单。
也可在终端运行 `suzaku-tool model guide`；配置损坏时，指南和路径查询仍可使用。

下文命令使用 `.deb` 安装后的 `suzaku-tool`。源码构建请在仓库根目录换成
`./target/debug/suzaku_tool`；tar 包请在解压后的包根目录换成 `./bin/suzaku_tool`。
三种入口的 `model` 参数相同，不要假定源码程序名已经加入系统 PATH。

```bash
suzaku-tool model paths
```

Linux 默认位置为 `${XDG_CONFIG_HOME:-$HOME/.config}/suzaku-ime/settings.json`；
`XDG_CONFIG_HOME` 须为绝对路径，空值或相对值会被忽略并回退到 `$HOME/.config`。
`SUZAKU_IME_CONFIG` 可覆盖该文件，显式空值会报错，不悄悄回到默认目录。
相对覆盖路径以启动进程的工作目录解析，因此建议个人部署使用绝对路径。
`model paths` 显示的是**当前进程**的解析结果，不证明已经运行的面板、IBus 或 systemd
宿主也使用相同路径。托盘的“数据管理 → 打开输入法配置目录”可打开目录；该操作可能
创建尚不存在的目录，指南和 `paths` 本身不会创建。

第一次使用云端，按下文依次完成：确认 API 地址/模型 → 离线预检 → 保存目标 →
为测试进程提供凭据 → 显式单次测试 → 为实际宿主和面板提供凭据 → 重载并启用。
语法有效、密钥格式有效、接口调用成功、日常宿主可用是四件不同的事。

### 交互式配置向导

不想拼接配置参数时，在终端运行：

```bash
suzaku-tool model setup
```

向导显示本进程将保存的配置路径，逐项询问部署位置、地址类型、协议、地址、模型 ID、
凭据环境变量名和超时。API 基址与完整生成地址是两个独立选项；协议由你明确选择，
不会根据模型名字猜测。完整地址模式默认沿用已有协议；基址模式只接受下列三种 compatible
协议，选择前会显示默认值。跨本地/云端切换时不沿用另一部署的地址。
只填写服务商给的地址和模型，不要填写密钥值。

保存前会显示配置预览；只有最后逐字输入 `save` 才会保存。向导要求标准输入和输出
均连接终端，不接受管道预灌答案；原有 `configure` 命令继续用于明确参数的脚本操作。
任意步骤输入 `:cancel`、输入结束或验证失败都不写配置；`save` 也必须以回车结束，
未结束的确认行不会保存。其他程序在向导期间更改配置时，保存会拒绝覆盖，
应重新运行向导后核对最新内容，而不是强制写入。

向导始终将云端授权设为关闭，保留输入语言、联想开关、温度和快捷键；不会读取密钥值、
联网测试或自动重载宿主。保存不是启用：仍需按下文为实际进程提供凭据、显式测试并授权，
最后从托盘重新加载模型配置。图形设置目前提供的是向导说明，不是直接编辑字段的表单。

## 源码入口与兼容边界

模型适配器的规范入口是 `suzaku_map::model`：部署配置及提供器位于
[`src/model.rs`](../src/model.rs)，发现、传输、翻译适配与测试位于 `src/model/`。
旧 [`languages::model`](../src/languages/model.rs) 公开转导出同一实现，
`languages::llm`、`languages::llama` 等兼容入口继续保留，不是第二套模型服务。

旧 `LlmCompletionProvider` 实现与两个英文插件工厂集中在私有
[`model/legacy.rs`](../src/model/legacy.rs)；工厂继续由 `model` 公开转导出，
既有模型路径和旧 trait 调用方式不变。模型核心、发现和传输直接使用
`prediction::PredictionError` / `PredictionCancellation`，不再经旧错误/取消别名取用。
旧别名本来就指向相同协议类型，因此这不是另一套错误/取消语义；兼容测试仍显式使用旧接口。

输入预测协议仍由 `prediction::PredictionProvider` 拥有；显式翻译仍使用
`languages::translation::TranslationProvider`，由
[`model/translation.rs`](../src/model/translation.rs) 实现 HTTP 适配，并将共享传输的
`PredictionError` 映射为翻译错误。旧桥隔离不等于完全解耦：兼容层仍依赖旧联想接口和
英文插件策略，模型核心仍经 `prediction/prompt` 间接依赖语言策略，预测链的
`prediction/policy` 也仍包含语言策略，
翻译接口也仍属于 `languages`。此次整理不代表独立 crate 或无语言依赖的纯协议层，
也不改变配置、请求格式、错误检查顺序、发现/取消预算、候选排序或模型能力。

## 输入预测协议 v1（开发版）

实际调用链为：引擎 → 有界异步 worker → `PredictionProvider` → 后端适配器。
Linux 宿主和独立面板都走此入口；本地发现、云端授权及传输仍由 HTTP 适配器负责。
这是应用内可序列化的任务协议，**不是新增网络监听服务，也不是要求模型直接输出协议包**。
它与配置中的 `llm_protocol` 不同：后者选择 Ollama 或 compatible 的 HTTP 外层格式。

公开类型位于 [`src/prediction/mod.rs`](../src/prediction/mod.rs)：

| 类型 | 契约 |
| --- | --- |
| `PredictionRequest` | `version`、`request_id`、`input`、`output`、`limits` |
| `PredictionInput` | 语言 ID、精确原始草稿、本地转换、会话提交上下文、置信度与降级状态 |
| `PredictionOutput` | v1 只接受 `complete_draft_replacement`：完整替换当前可编辑草稿，不是追加 token 或提交命令 |
| `PredictionResponse` | 对应版本和请求编号，以及候选列表；编号由适配器回传，不让语言模型生成 |
| `PredictionCandidate` | 纯正文、相对排序提示、可选 `word` / `sentence` 类型；没有 UI 标签或原样输入类型 |
| `PredictionCancellation` / `PredictionError` | 协作取消、结构错误和后端失败；失败保持离线输入可用 |

`PredictionRequest::new(id, input)` 使用 v1 默认预算：最多 6 条候选，原始草稿最多
256 个 Unicode 字符、会话提交上下文最多 160 字符；本地转换传入上限为 416 字符。
候选默认最多新增 160 字符，只有精确匹配且不超过 256 字符的本地转换前缀才不计入
新增预算；其他替换正文整体限 160 字符。调用方可缩小候选数和新增字符预算，不能扩张
全局上限。HTTP 适配器过滤超限项而不截断正文。权重不是概率，其影响仍由引擎限幅。

请求和响应支持 Serde JSON 往返，保留 Unicode、缩进及连续空格。worker 在调用前
验证请求、调用后验证版本/编号/数量/字符预算/控制字符/有限权重，同时保留自身的
revision 屏障，不信任响应编号来替代本地过期判断。结构校验不是语言质量认证；
语言前缀与补全规则、词句混排分别属于任务策略和引擎。

2026-10-06 补齐了所有后端共用的草稿保护：结构校验后，worker 通过
[`prediction/policy.rs`](../src/prediction/policy.rs) 逐项过滤丢失英文前文/缩进、
仅回显原稿或生成请求字段的候选，再交给面板/IBus 限额与混排。坏候选不会遮住同批
有效候选，全无效时保留完整本地候选并报告 `NoCandidates`。
英文以原始草稿为准，不强制采用本地首选词；有本地补全证据的半词如 `hel` 不能
变成 `hel there`，但仅仅未收录本地词库的词不会因此禁止独立后端继续预测。
拼音、罗马音与同音转换不受英文前缀规则约束。字段回显判断只检查生成段，保留用户
已输入的 JSON、Unicode 前缀以及合法的中日文转换。HTTP 原有的转换前缀和短句策略仍
独立保留；这些检查不代表对任意语言的正确性保证。

新增后端只需实现 `PredictionProvider::predict`，并通过引擎的
`configure_prediction_provider` 接入；既可以适配聊天模型，也可以直接返回预测结构。
适配器应先检查取消和请求有效性，限制自己的 I/O，返回 `PredictionResponse::new`
关联的完整结果。端点、模型标识、认证、采样参数和发现缓存均不属于输入协议。
旧 `configure_prediction` 通过 `LegacyPredictionAdapter` 桥接；旧实现不需新增方法，
旧请求的字段与候选 struct literal 保留。旧候选列表的逐项过滤行为也保留。

[`prediction/prompt.rs`](../src/prediction/prompt.rs) 集中管理聊天模型的输入字段、
语言提示、精简 schema 和候选内容解析；HTTP 模块只组合供应商 envelope 并处理
传输及外层完成状态。当前仍发送原来的双字符串候选提示，不因分层改变小模型负担。
v1 不增加桌面周边文本采集、不扩大上下文、不支持 token 流式展示、任意编辑范围或
工具调用；这些需要后续明确协议扩展与独立验收，不能用此重构宣称预测质量已提升。

显式[文本翻译](translation.md)使用独立的 `TranslationProvider` 接口和翻译提示，共用同一个
`HttpModelProvider` 配置与安全传输。翻译总超时为 30 秒，不使用候选联想的 20–5000 ms
低延迟预算；翻译本身不要求联想开关开启，但云端授权和凭据检查完全相同。

## 三个独立选项

| 选项 | 可选值 | 默认值 |
| --- | --- | --- |
| `llm_scope` | `local` / `cloud` | `local` |
| `llm_protocol` | `auto` / `ollama` / `openai-compatible` / `chat-completions` / `chat-completions-low` | `auto` |
| `llm_model` | 服务公布的模型 ID；本地可用 `auto` | `auto` |

`auto` 协议将 `/api/chat` 识别为 Ollama，其他地址使用 chat completions。
可以显式指定协议以支持自定义生成路径。compatible 适配器使用 `messages`、`model`、
`temperature`、`max_tokens` 和非流式 `choices[].message.content`；
**不是所有厂商的专有 API 都受支持**，不同协议需新增适配器或使用兼容网关。

`chat-completions` 是显式选择的现代低延迟配置档：相同的消息和非流式回复格式，
改发 `max_completion_tokens`，并设置 `reasoning_effort: "none"`；原温度和 token 数量预算保留。
只用于服务明确支持这些参数及关闭推理的模型，不根据模型名称、域名或密钥猜测能力，
不自动重试或回退到另一格式。原 `openai-compatible`/`auto` 请求保持原样。
参数依据见 [Chat Completions 参考](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
和[推理参数兼容说明](https://developers.openai.com/api/docs/guides/latest-model)。

`chat-completions-low` 为需要低推理的服务提供独立显式配置档：使用同样的
`max_completion_tokens`，固定 `reasoning_effort: "low"`，不发送温度等采样参数。
候选总预算仍为 16–512 tokens（默认 256），包含推理和可见输出；低推理不保证在预算内
一定能生成候选。翻译继续保留自身的 512/4096 预算。原 `chat-completions` 的
`none` 与温度参数不变，不按模型名自动选档或自动切换重试。
官方 GPT-6 Astra 与 GPT-6.1 Sol 不支持 `none`，且启用推理时不应发送温度参数，
因此测试它们时需明确选择这个档位；其他厂商须核对其参数兼容性。
见[官方迁移参数说明](https://developers.openai.com/api/docs/guides/latest-model#migration-quickstart)。

## 词与句子的统一输出

0.6.6 把默认请求精简为 **2 条完整草稿**：一个词补全和一个短句续写，减少小型 CPU 模型
反复生成字段名与分类标签的开销。Ollama 使用简单的双字符串数组 schema；compatible
服务通过提示请求同样格式：

```json
{"candidates":["hello","hello world"]}
```

响应仍兼容最多 6 项的旧 `{text, kind}` 对象、最多 3 项的字符串数组，以及 compatible
逐行纯文本；不要求现有服务同步换格式。分类、提取词候选和词句混排仍由语言层负责，
不保证模型每次都给出两种类型。默认输出上限仍是 256 tokens；拒绝截断
JSON、未知类型和控制字符。日语无类型的新模型候选可保守识别已知词干加名词后缀，
以及“日本語入力”这类输入领域复合词。整体限 2–8 个汉字，完整词干须为现有日语 Word；
多字词头还须为现有 Word，且词干限语言、文字、数字或音声类对象。不任意分词或递归推断，
不把带助词、空格或标点的句子当成词。本地同文项和明确的模型类型保持原有分类；
这不是完整日语词性分析器。开发版统一了长草稿的长度预算：完整候选可保留最多 256 字符的
精确输入/转换前缀，再新增最多 160 字符（总长最多 416）；没有匹配前缀的转换结果仍限 160 字符。
解析与引擎混排使用同一预算，不截断前文，不放宽响应体大小或控制字符校验；超过 256 字符的
输入仍走有界本地回退。模型的排序提示经本地限幅，不能覆盖输入首项或把展示注解写进正文。
具体混排与按键见 [IBus 候选体验](ibus-candidates.md)。
英文候选从响应解析、混排到提交都保留已输入的精确前缀，包括缩进、连续空格和列表标记；
仅清除前缀以外的响应留白，不补造模型漏掉的前缀。原样输入仍不会作为新联想接受。
生成部分若包含至少两个不同的、带引号与冒号的请求字段键（例如 `local_conversion`
与 `raw_composition`），也作为请求数据回显丢弃。即使外层 JSON 正常，它也不是输入联想。
此保护只检查新增文本，不删除用户已经输入的字段名，也不充当通用语言质量判定器。

开发版英文请求额外区分三种状态：未完成词 `complete_word`、已输入分隔空格等续写场景
`next_word`，以及 `a` / `can` 等既可能是完整词又可能是词头的 `complete_or_continue`。
请求保留 `raw_composition`、`local_conversion`、`committed_context` 和 `suggestion_mode`，
不再重复发送 `word_prefix` / `text_before_word` 或要求 3+3 条候选；默认置信度、空手写提示
也不再占用输入。非默认置信度、降级状态和显式手写提示仍保留。候选必须是完整草稿替换文本，
不是仅返回末词。这个规则同时用于 Ollama 和 compatible 协议，不依据具体模型名称分支，
也不覆盖调用方自定义系统提示、温度或生成预算。
IBus 混排还能从有效英文句子中提取独立词候选，细节与回归范围见
[英文补全质量回归](ibus-candidates.md)。

## 本地发现

```bash
suzaku-tool model configure
suzaku-tool model discover
suzaku-tool model status
```

以上 `configure` 恢复本地自动配置，保留输入语言、联想开关、超时和温度；不会开启联想。
启用联想后，后台第一次需要模型时也会自动发现。默认只检查三个回环地址：

- Ollama：`http://127.0.0.1:11434/api/chat`，读取 `/api/tags`。
- llama.cpp：`http://127.0.0.1:8080/v1/chat/completions`，读取 `/v1/models`。
- 其他兼容桌面服务：`http://127.0.0.1:1234/v1/chat/completions`，读取 `/v1/models`。

优先选择名称/元数据为 LLaMA 的已安装模型，其次选择其他本地模型。不会指定版本、
下载文件、启动服务、自动预热、遍历磁盘、扫描其他端口或访问云端。
本地请求只允许字面回环 IP 或 `localhost`（固定映射到回环），不走 DNS、代理或重定向。
发现最多 800 ms；生成受 `llm_timeout_ms` 总预算约束，发现也计入预算。
发现成功缓存 60 秒、失败缓存 5 秒，配置重载创建新缓存。慢模型不会阻塞离线候选。
本地生成遇到模型/接口不存在（404）或服务不可用时，会清除对应的成功缓存，让下一次
输入请求重新发现；不会自动重试刚才失败的生成。超时、限流和云端失败不触发重新发现。

指定其他模型或端口：

```bash
suzaku-tool model configure --model my-local-model --endpoint http://127.0.0.1:8080/v1/chat/completions --protocol openai-compatible
```

自定义端点不扫描其他服务。自动选择需要服务提供模型清单；显式指定 compatible 模型时，
可以只提供生成接口。Ollama 本地模型在生成前会检查清单，拒绝被标记为远程的模型，
包括自定义别名；清单结果会缓存，因此仍建议服务设置 `OLLAMA_NO_CLOUD=1`。
回环地址是本地服务边界，Suzaku 无法识别任意兼容网关内部是否继续代理到远端。
本地认证头暂不支持；不要把密钥写进 URL。

`model warmup` 仅用于本地 Ollama，必须显式执行；发送空消息，空闲 5 分钟后释放。
`model probe [all|en|zh-Hans|ja]` 使用固定合成示例，不读取真实输入；它会实际运行模型，
云端已授权时会发送这些示例，并可能产生服务费用。`status` 对云端只显示配置、不发请求。

## 云端配置与授权

以下域名和模型 ID 都是占位值，需要换成你选择的服务配置。
`auto` 模型名不用于云端，不会自动枚举云端账号或选择服务。
`--endpoint` 接受完整生成地址，保持原样；新增的 `--base-url` 专用于 compatible API
基址，去掉尾斜杠后追加 `/chat/completions`，**不猜测或补加 `/v1`**。例如
`https://models.example/v1/` 得到 `/v1/chat/completions`，只提供域名则得到
`/chat/completions`。两种选项互斥；已经完整的 `/chat/completions`、`/api/chat`、
`/responses` 不能当作基址，Responses 专有协议也不会因此变成受支持的接口。

### 离线预检与单次测试

先检查拟用配置，无须保存或读取密钥值：

```bash
suzaku-tool model help
suzaku-tool model check \
  --scope cloud \
  --base-url https://models.example/v1 \
  --model your-model-id \
  --api-key-env SUZAKU_MODEL_API_KEY
```

`check` 不联网、不写配置；不带覆盖参数时检查现有配置，不恢复本地默认值。
它显示最终生成地址、配置协议/生效协议、模型、超时、授权和密钥环境变量名，
只验证配置语法，不能证明密钥可用、服务可达或宿主已获得该环境变量。
`--base-url` 未指定协议时选择 `openai-compatible`，也可显式选择 `chat-completions`
或 `chat-completions-low`；
搭配 `auto` 或 `ollama` 会报错。
帮助命令不依赖可读取的设置文件，配置损坏时也可查看。

普通 `check` 不读取密钥值。只有显式运行 `model check --credentials` 时，才检查本进程中
配置所指环境变量是否存在、是否符合凭据格式；仅报告状态，不输出值、不联网鉴权。
缺失或无效时返回失败。未配置凭据引用表示无认证请求，不代表服务一定允许匿名访问。
它不读取 Seahorse、不查看其他进程环境，也不证明运行宿主拿到了密钥。

将密钥安全注入当前测试进程环境后，可显式运行一次英文合成测试：

```bash
suzaku-tool model probe en --once \
  --scope cloud \
  --base-url https://models.example/v1 \
  --model your-model-id \
  --api-key-env SUZAKU_MODEL_API_KEY \
  --cloud-consent true \
  --timeout-ms 5000
```

此命令可能产生服务费用。覆盖参数仅驻留本次进程内，不保存、不切换输入法或开启联想；
改变目标仍会撤销旧授权，需同次明确授权。`--once` 默认英文，也可选 `zh-Hans` 或 `ja`，
不能搭配 `all`；它跳过预热，只测该语言首个固定样例。显式 compatible 模型不会请求
模型清单，最多发送一次生成请求，不自动重试。测试通过统一预测 v1 入口验证候选，
离线候选输出不算模型通过；HTTP 错误仅报告安全摘要和状态码，不输出原始响应体。
这是接口与候选结构检查，不是完整语言质量、GUI 或日常输入法宿主验收。
原有不带 `--once` 的 `probe` 保留七个三语样例；本地 Ollama 仍可能先检查清单或预热。
若服务要求现代 token 参数和显式关闭推理，在同一命令加 `--protocol chat-completions`；
若服务要求启用低推理，则明确使用 `--protocol chat-completions-low`，不要附带温度参数。
一次失败不会自动更换协议或扩大生成预算。

### 保存与应用

确认接口后再保存；加 `--dry-run` 可预览这次保存的效果而不联网、不落盘：
保存时须保持与通过 `probe` 的配置相同的协议、地址和模型。下例演示
`openai-compatible`；若测试时用了 `chat-completions`，这里也须改成
`--protocol chat-completions`；测试用的是低推理档则保留 `chat-completions-low`。
不要在保存时退回另一种请求格式。

```bash
suzaku-tool model configure \
  --scope cloud \
  --protocol openai-compatible \
  --endpoint https://models.example/v1/chat/completions \
  --model your-model-id \
  --api-key-env SUZAKU_MODEL_API_KEY
```

此时仍未授权发送输入。将密钥安全注入**运行宿主的进程环境**后，再明确开启：

```bash
suzaku-tool model configure --cloud-consent true
```

配置仅保存环境变量名 `SUZAKU_MODEL_API_KEY`，密钥值在请求时读取。没有 `--api-key`
参数，密钥不会进入配置、备份、托盘或错误消息。无需认证的服务可用 `--api-key-env none`。
单纯在终端 `export` 不会改变已运行宿主的环境；若由 systemd 用户服务启动，需通过自己的
安全环境配置方式注入，再重启宿主。不要把密钥贴进工单、命令行参数或提交到仓库。

### 在终端安全测试凭据

下面是 Bash 的**单次测试进程**示例，适用于已经保存了目标、凭据引用
`SUZAKU_MODEL_API_KEY` 并明确授权的配置。输入隐藏，密钥不进入命令历史；子 shell 退出后
不改变原终端环境。`probe` 仍可能产生云端费用，不要把它放进自动重复执行的脚本。

```bash
(
  IFS= read -r -s -p 'Model API key: ' SUZAKU_MODEL_API_KEY || exit 1
  printf '\n'
  export SUZAKU_MODEL_API_KEY
  suzaku-tool model check --credentials &&
    suzaku-tool model probe en --once
)
```

如果只想检查格式，不想发请求，停在 `check --credentials` 即可。无需认证的本地或
兼容服务不要运行此密钥输入示例。真实生成仍受云端授权、配置和候选期限约束。

### 让日常宿主获得凭据

当前产品只解析配置指向的**进程环境变量**，尚无内置 Seahorse/Secret Service 条目绑定。
把密钥存进 Seahorse 是安全存储步骤，不等于 Suzaku 已会自动读取；之前的单次实测由
专门的临时启动器注入，不能当作已安装宿主的通用能力。

独立面板、systemd 用户服务和 IBus 直接启动是不同入口；凭据须分别由可信的启动机制
传入实际进程。仅对终端 `export`、重启 systemd 服务或从托盘重载 JSON，都不能把当前
终端的变量传进另一个已运行进程。我们不提供明文密钥环境文件、全局
`systemctl --user set-environment/import-environment` 或读取 `/proc` 环境的捷径。
在尚未为你的启动方式配置安全注入时，保留离线输入或仅使用上面的显式测试入口；
不要把缺凭据误判为服务或词库故障。

配置后在托盘选 **重新加载模型配置**，再按需开启 **LLM 联想**。生成必须同时满足：
联想开启、云端明确授权、配置有效、所选密钥可用、非隐私输入字段。

0.5.7 面板的自动候选使用启动时/宿主确认的配置；温度或手写提示刷新不会偷偷采用磁盘上的
新模型目标。配置文件初始损坏时只保留离线输入，不改用默认服务；外观缓存的联想 On 也不能
覆盖输入法配置的 Off。确认重载切换提供器后，保留当前草稿、清旧提交上下文与未使用译文。
翻译发送前另外核对磁盘上的目标和授权，冲突或读取失败时不发请求，需先重新加载。
详见 [第六轮模型链路修复](bug-audit-model-2026-09-13.md)；保存配置不等于即时重载运行中的宿主。

compatible 完成状态只接受 `stop`，以及兼容旧接口的缺失字段/`null`；工具转交、未知状态、
错误类型、截断/过滤回复不能进入候选或翻译。失败仍保留本地输入，不会执行模型工具调用。
会发送当前输入片段、本地转换和本次焦点会话内最近最多 160 字符提交上下文；
不采集桌面周边文本、不落盘输入历史。服务商如何保留和处理数据取决于你选择的服务。
未授权、断网、超时、证书错误或无有效候选时仍可用离线候选。

HTTPS 使用 Rustls 验证证书。不跟随重定向、不使用系统代理、不自动重试，响应上限 128 KiB。
`llm_timeout_ms` 可设为 20–5000 ms。云端接口的超时/格式兼容不代表模型语言质量已验证。
云端 Client 和请求准备也计入同一请求期限；准备结束、真正发送前再次检查取消与剩余时间。
已作废或耗尽预算的任务不会因此重新获得完整期限。
CLI 更换端点、模型、协议、部署位置或密钥引用时撤销旧授权，需要再次明确授权。
手动编辑 JSON 等同直接修改配置，务必同时核对 `llm_cloud_consent`。

回到本地：`suzaku-tool model configure`，然后从托盘重载。
撤销云端授权：`suzaku-tool model configure --cloud-consent false`，然后从托盘重载。
编辑、提交、切框、禁用联想或重载会作废旧结果，并通知在途候选请求取消。当前本地 HTTP
提供器在已连接的读写过程中约每 50 ms 检查一次取消，关闭旧连接，让最新草稿不必等待旧
推理的完整期限；模型清单请求同样支持取消，取消不会写入“未找到模型”的缓存。
云端 HTTPS 在同一 worker 内驱动异步请求，等待响应头和读取正文期间约每 25 ms 检查取消，
取消后丢弃在途等待，让最新草稿继续使用原有单 worker；不为每次编辑另起脱离的请求线程。
这些检查间隔不是端到端延迟保证：冷启动 Client 构造、线程调度、本地连接与共享发现锁
仍可能等待；底层已启动的操作系统 DNS 解析也不保证立即停止。未实现协作取消的自定义
提供器仍等待原调用完成/超时，再丢弃旧结果。翻译仍使用自身的异步期限。
取消不能撤回服务已收到的数据，也不保证服务立即停止计算或计费。完整响应校验、
120 ms 防抖和默认 1200 ms 候选期限不变，见 [本地取消审计](bug-audit-model-cancellation-2026-09-24.md)。
重载模型配置保留当前 IBus 草稿；更换模型、端点、协议、部署位置或密钥引用时，清除
旧服务的已提交上下文与撤销历史，新服务只接收当前草稿。语言切换仍会清空原组合。

### 读取候选预测状态

独立输入面板会在原有标题行显示等待、已收到模型响应或失败原因；候选未变化的超时、
缺密钥、未授权等失败也会刷新提示，不增加窗口高度或遮盖候选。提示属于该面板自己的
引擎，不显示在原生伴随、私密原生上下文、设置、详情报告或悬浮球中，也不保存到配置。

IBus 宿主的状态请查看托盘中的“输入法宿主上次报告”。这是打开菜单或显式刷新时取得的
宿主快照，不是独立面板或当前输入框的实时进度。已关闭、当前上下文未运行、尚未授权、
等待结果和空闲状态分别显示；空闲不承诺输入后一定发出请求，超长草稿等保护条件仍有效。

“已收到模型响应”不保证出现新的 AI 标记行：模型与本地候选同文时，只保留本地来源并加权。
诊断只翻译应用自有说明，原始参数保持原样；不据一条提示猜测请求已发送、自动重试或扩大期限。

## 备份与迁移

旧配置未设置新字段时按本地协议处理，显式模型名称不变；新安装默认自动发现。
数据备份支持新配置字段，仅含密钥环境变量名，不含值。恢复预览和实际应用都会将
云端授权关闭，避免把旧机器的发送许可带到新机器。离线配置与面板偏好照常恢复。

协议参考：[Ollama 模型清单](https://docs.ollama.com/api/tags)、
[llama.cpp server](https://github.com/ggml-org/llama.cpp/tree/master/tools/server)。
