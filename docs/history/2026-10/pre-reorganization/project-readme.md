# Suzaku Map · 朱雀

**Published baseline: [0.10.0 / build 50](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0), exact source `05ca67930b2a6784860f98954248c2b6663d4617`.** All eight gates of [CI 40676977245](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/40676977245) passed. Accepted artifact `12813215443` supplied all four Linux package/checksum assets; safe archive inspection, source/manifest checks and independent upload re-download byte/hash comparison passed before publication as Release `426593249` on 2026-10-28. Linux Alpha prerelease, not latest stable. See the [publication receipt and asset fingerprints](../../../releases/0.10.0.md#publication-receipt-2026-10-28). The personal installation and cancelled automation remain unchanged.

<!-- release-0.10.0-preparation:start -->
**0.10.0 / build 50 源码准备：三语日常与工作交流里程碑。** 相对 0.9.9 汇总七批词库补充，共新增 66 个语境、132 条作者句，复用已有词形和读音。当前总量：英文 6216 词形 / 968 语境 / 1900 句；中文 3442 读音 / 959 语境 / 1916 句；日文 638 读音 / 447 语境 / 893 句。包含 Linux 偏好锁的持锁进程归属修复；此前偶发 `Busy` 的原因仍未完全确定，启动后自动恢复持久化的行为不变。详见 [0.10.0 说明](../../../releases/0.10.0.md)。

<!-- release-0.10.0-local-validation:start -->
**本版本本地验收完成：**完整 Rust **2982 通过 / 0 失败 / 32 项原有显式忽略**，321 个唯一、零筛除结果组，含 Doc-tests；53 项新增测试各计一次。另有 **19/19 隔离 UI**、严格全目标全功能 Clippy、315 项 Python 检查、250 个审计链接、五处版本权威来源与 shell 语法检查通过。默认/行内 IBus 四分区和自定义快捷键门禁也分别通过，独立于 Rust/UI 计数；本机缺少 ShellCheck，该检查留待远端 CI。下方各批次的“当前未提交”、旧版本号和有界结果均保留为历史快照；完整证据与验收边界见 [0.10.0 说明](../../../releases/0.10.0.md)。
<!-- release-0.10.0-local-validation:end -->
<!-- release-0.10.0-remote-validation:start -->
**发布验收完成：**准确源码 `05ca67930b2a6784860f98954248c2b6663d4617` 的 [CI 40676977245](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/40676977245) 八门全部成功；accepted artifact `12813215443` 的两个安装包及两个校验文件完成归档/manifest/源码校验，上传后重新下载的实际字节和 SHA-256 全部一致。[0.10.0 / build 50](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0) 已作为 Linux Alpha prerelease（非 latest stable）发布，Release ID `426593249`；旧 0.9.7 回执及下方准备期 pending 均保留为历史。
<!-- release-0.10.0-remote-validation:end -->
<!-- release-0.10.0-preparation:end -->

**Previous published baseline: [0.9.7 / build 47](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.9.7), exact source `354030b1107b20ad2447a9f93aaa6cf6beba1c40`.** All eight gates of [CI 40081142818](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/40081142818) passed; accepted artifact `12556460951` and all four uploaded Linux assets were independently validated and re-downloaded for byte comparison before publication. Linux Alpha prerelease, not latest stable. The source-preparation pending and earlier uncommitted labels below are historical. The personal installation is unchanged and the cancelled automation remains cancelled.

<!-- day-coordination-followup:start -->
**当前未提交：日常安排与协调跟进（2026-10-28）。** 三语各新增 4 个语境、8 条候选句，共 12 个语境、24 条句子：英文涵盖快速通话、预留时间、链接反馈和时间协调；中文涵盖日程、集合点、预计到达和提醒；日文涵盖返信待ち、予約確認、合流地点和延長。复用已有词形和读音。当前总量：英文 6216 词形 / 968 语境 / 1900 句；中文 3442 读音 / 959 语境 / 1916 句；日文 638 读音 / 447 语境 / 893 句。完整库及 99 个相关集成目标的同轮回归为 1350 通过、0 失败、2 项原有忽略；覆盖候选提交、拼音声调、日语假名、模拟模型不可用及推荐离线包共存。下方各批次及偏好锁修复记录保留为历史快照；版本仍为 0.9.9，本轮改动未提交。
严格全目标 Clippy 与格式检查通过。
<!-- day-coordination-followup:end -->

<!-- daily-errands-followup:start -->
**当前未提交：日常事务与出行跟进（2026-10-28）。** 三语各新增 3 个语境、6 条候选句，共 9 个语境、18 条句子：英文涵盖天气、收据和充电；中文涵盖申请表、备用电池和停车场；日文涵盖申请书、送り状和順序。复用已有词形和读音。当前总量：英文 6216 词形 / 964 语境 / 1892 句；中文 3442 读音 / 955 语境 / 1908 句；日文 638 读音 / 443 语境 / 885 句。全功能库及 89 个相关集成目标的独立回归为 1286 通过、0 失败、2 项原有忽略。首轮因新增英文句子的 clears 和 papers 改变索引，调整句子复用旧词后保留旧断言通过；另一次完整库回归中的偏好临时锁 Busy 失败没有在单独测试及最终回归中重现，原因未确认。下方结果均为历史快照；本轮不代表全仓、真实模型或安装验收。
严格全目标 Clippy、格式和词库结构检查通过；本批和之前的未提交改动仍在工作区。
<!-- daily-errands-followup:end -->

<!-- daily-service-followup:start -->
**当前未提交：生活与服务跟进（2026-10-28）。** 三语各新增 3 个语境、6 条候选句，共 9 个语境、18 条句子：英文涵盖点餐确认成分、休息补水和抵达消息；中文涵盖取件凭证、预约凭证和报修；日文涵盖失物、受付时间和故障。复用已有词形和读音。当前总量：英文 6216 词形 / 961 语境 / 1886 句；中文 3442 读音 / 952 语境 / 1902 句；日文 638 读音 / 440 语境 / 879 句。全功能库及 85 个相关集成目标的独立回归为 1255 通过、0 失败、2 项原有忽略。首轮“售后申请”改变旧中文候选快照，原失败保留；改用“取件凭证”后完整相关回归通过，旧断言和候选规则不变。下方记录为历史快照；本轮不代表全仓、真实模型或安装验收。
严格全目标 Clippy、格式和词库结构检查通过；本批和之前的未提交改动仍在工作区。
<!-- daily-service-followup:end -->

<!-- daily-outing-services:start -->
**当前未提交：日常出门与服务沟通（2026-10-28）。** 三语各新增 3 个语境、6 条候选句，共 9 个语境、18 条句子，涵盖出门检查、随身物品、用餐和服务预约、包裹、服务台。当前总量：英文 6216 词形 / 958 语境 / 1880 句；中文 3442 读音 / 949 语境 / 1896 句；日文 638 读音 / 437 语境 / 873 句。全功能库及 79 个相关集成目标的独立回归为 1217 通过、0 失败、2 项原有忽略。首轮新增“取号”续写改变旧 qu hao 向量，原失败保留；改用“服务台”后重跑通过，旧断言和候选规则不变。以下各批次为历史快照；本轮不代表全仓、真实模型或安装验收。
严格全目标 Clippy、格式和词库结构检查通过；本批与之前的未提交改动仍在工作区。
<!-- daily-outing-services:end -->

<!-- work-collaboration:start -->
**当前未提交：工作协作与文件沟通（2026-10-28）。** 三语各新增 3 个语境、6 条候选句，覆盖工作阻塞、审阅确认、时间调整、文件权限与网络沟通，复用已有词形和读音。当前总量：英文 6216 词形 / 955 语境 / 1874 句；中文 3442 读音 / 946 语境 / 1890 句；日文 638 读音 / 434 语境 / 867 句。全功能库测试及 78 个相关集成目标通过，库测试有 2 项原有忽略；6 项新测试覆盖输入候选、提交、合成模型不可用及离线包共存。严格全目标 Clippy 与格式检查通过。沙箱内首次全功能回归因本地套接字权限失败，允许本地套接字后重跑通过；旧批次结果仍是历史快照，不代表本轮全仓、真实模型或安装验收。
<!-- work-collaboration:end -->

<!-- work-followup:start -->
**当前未提交：工作安排与进度跟进（2026-10-28）。** 三语各新增3个语境、6条候选句，覆盖行动项、期限、进度及交接；复用已有词形和读音。当前英文6216词形/952语境/1868句，中文3442读音/943语境/1884句，日文638读音/431语境/861句。本轮有界回归 1205 通过、0失败、2 原有忽略，涵盖完整库与77个相关集成目标；6项新增测试覆盖罗马音、拼音及声调、假名、候选提交、原文保留、模型不可用和推荐离线包共存。严格全目标 Clippy 和格式检查通过。旧数据顺序、模型接口和生产候选预算保持；版本仍为0.9.9，未提交。

本批次及下方各批次的数量和验证结果保留为当时快照，当前总量以最新记录为准。
<!-- work-followup:end -->

<!-- work-clarification:start -->
**当前未提交：工作澄清与异步回复（2026-10-28）。** 三语各新增3语境、6条作者句，共9语境、18句，复用已有词形/读音；EN6216/949/1862（64层，末层34/68）、ZH3442/940/1878、JA638/428/855。独立有界回归实际1482过、0失败、2原ignore，共115唯一组（完整lib+114集成目标），新12项各计一次；严格全target/all-feature Clippy -D warnings通过（67.26秒）。原before4过8失败、首预检新套件10过2失败和首115组的reply快照失败证据保留，不叠加为本轮通过；仅新测试纠正xiu gai yi jia同时完整拼出旧“修改衣架”的歧义预期：新两句固定索引5/6、跨六行页界可选择，完整jian和汉字仍双句首页。reply仅追加两新句，完整四项向量逐字段确认后精确逆回，旧hash未改；用語以完整ようご为纯假名正例，宽よう仅测literal与旧要点。旧作者/读音/rank、25包/SDK、生产算法及配额不变；有限作者表达，不执行工作或发消息。版本仍0.9.9/build49，本批不在已推送f3a7417内，未提交/推送/安装；不声明全仓、Doc-tests、UI/原生、实模、CI或性能验收，不恢复自动化。
<!-- work-clarification:end -->

<!-- release-0.9.9-source:start -->
**0.9.9 / build 49 源码更新：仅提交与推送，不创建对外发行。** 五批日常及工作交流新增三语 48 个语境、96 条作者句，零新词形或读音。总量 EN 6216 / 946 / 1856（64 层）、ZH 3442 / 937 / 1872、JA 638 / 425 / 849；英文空 words 末层 31 语境、62 作者句，6216 首次 rank 与旧读音保持。本版独立完整 Rust 实际 2929 过、0 失败、32 原 ignore，313 唯一组，包含完整 lib、10 个 binary/main、301 个集成套件及 Doc-tests，零筛除；七个新套件的 64 项各计一次。另 19 项私有 Xvfb/D-Bus/XDG 隔离 UI 门禁和严格全 target/all-feature Clippy -D warnings 实际通过（64.19 秒），不加完整 Rust 总数。原候选短池可随作者数据变化；旧 goldens、语料与算法保持，精确 test-only 逆回不冒称旧尾项仍在短池。先前 pending/未提交与有界记录原样保留为历史，不借作本版通过。准确提交的远端 CI 另行确认；不声明实模、原生 IBus/Wayland、物理键盘、性能、包或安装验收。不创建 0.9.9 tag、GitHub Release、安装包，不更改个人安装/服务/配置，不恢复已取消 timer；已发布 0.9.7 仍是下载基线。

Earlier pending/uncommitted and bounded labels below describe their original snapshots, not the current 0.9.9 source-validation status.
<!-- release-0.9.9-source:end -->

<!-- work-communication:start -->
**当前未提交：工作进度、需求范围、会议协调与交接反馈（2026-10-26）**。每语新增 4 个语境、8 条作者句，共 12 语境、24 作者句，零新词形或读音。总量 EN 6216 / 946 / 1856（64 层）、ZH 3442 / 937 / 1872、JA 638 / 425 / 849；英文空 words 末层 31 语境、62 作者句。独立有界回归实际 1470 过、0 失败、2 原 ignore，114 唯一组（完整 lib + 113 集成目标），新 work 12 项与严格兼容 2 项各计一次；严格全 target/all-feature Clippy -D warnings 通过（64.94 秒）。真实 before 4 过 / 8 失败、首轮预检真实 180 过 / 4 失败 / 25 组原证据保留；独立最终预检 186 过 / 26 组及私有子进程不叠加。日语 A/B 182 条有序记录中 180 条不变，仅两模式 henk 池改变；旧返却/返金 Word 被挤出短输入池，不能称原池保持。xu'qi 保留旧续期 Word 歧义；担当保留旧担当者 Word，均不为旧 Word 虚增作者句。かい 仅作宽根/literal 边界，会議的纯假名正例使用完整かいぎ；不宣称任意短英文候选池不变。仍为 0.9.8 / build 48，未提交；不声明全仓、Doc-tests、UI/原生、实模、安装、远端 CI 或性能验收，不提交推送、不执行工作/日历/消息或启用自动化。
<!-- work-communication:end -->

<!-- daily-apology:start -->
**当前未提交：认错、迟回、漏看与更正说明（2026-10-26）**。新增三语 9 个语境、18 条作者句，零新词形或读音。总量 EN 6216 / 942 / 1848（64 层）、ZH 3442 / 933 / 1864、JA 638 / 421 / 841；英文空 words 末层 27 语境、54 作者句。独立有界回归实际 1446 过、0 失败、2 原 ignore，110 唯一组（完整 lib + 109 集成目标），新 12 项各计一次；严格全 target/all-feature Clippy -D warnings 通过（71.49 秒）。真实 before 4 过 / 8 失败、独立预检 146 过 / 19 组及私有子进程不叠加。日文謝罪偏正式书面表达；saiso 等短输入保留旧再送信 Word 歧义，不为旧 Word 虚增作者句，也不实际发送/重发消息。仍为 0.9.8 / build 48，未提交；不声明全仓、Doc-tests、UI/原生、实模、安装、远端 CI 或性能验收，不升版、提交、安装或启用自动化。
<!-- daily-apology:end -->

<!-- daily-reassurance:start -->
**当前未提交：报平安、关心近况与慢慢来（2026-10-26）**。新增三语 9 个语境、18 条作者句，零新词形或读音。总量 EN 6216 / 939 / 1842（64 层）、ZH 3442 / 930 / 1858、JA 638 / 418 / 835；英文空 words 末层 24 语境、48 作者句。独立有界回归实际 1413 过、0 失败、2 原 ignore，106 唯一组（完整 lib + 105 集成目标），新 12 项各计一次；严格全 target/all-feature Clippy -D warnings 通过（59.91 秒）。真实 before 4 过 / 8 缺、独立预检 113 过 / 15 组及私有子进程不叠加。日文居眠り仅为诚实的疲倦/迟回说明，不是诊疗、状态检测或自动消息。仍为 0.9.8 / build 48，未提交；不声明全仓、Doc-tests、UI/原生、实模、安装、远端 CI 或性能验收，不升版、提交、安装或启用自动化。
<!-- daily-reassurance:end -->

<!-- daily-farewell:start -->
**当前未提交：聊天收尾与告别（2026-10-26）**。本批新增三语 9 个语境、18 条作者句，不增词形或读音。总量为 EN 6216 / 936 / 1836（64 层）、ZH 3442 / 927 / 1852、JA 638 / 415 / 829；英文空 words 末层现有 21 个语境、42 条作者句。独立有界回归实际 1384 过、0 失败、2 原 ignore，103 唯一组（完整 lib + 102 集成目标），新 12 项与独立兼容性 2 项各计一次；严格 Clippy -D warnings 通过（65.06 秒）。首轮 11 组预检 80 过 / 2 失败如实保留；完整 130 向量 A/B 中 129 不变，仅 i s 合理新增一个 Word，严格断言当前全向量后仅作 test-only 历史投影，旧 golden、语料、配额和运行时不改。before 4 过 8 缺、独立重跑 12 组预检 84 过及诊断/私有子进程不叠加。日文包含拜访结束和面对面送别，并非聊天逐句翻译。仍为 0.9.8 / build 48，未提交；不声明本轮全仓、Doc-tests、UI、原生、实模、性能、安装或 CI 验收，不发包、不改个人配置或自动化。
<!-- daily-farewell:end -->

<!-- daily-consideration:start -->
**当前未提交：婉拒、体谅与不给压力（2026-10-26）**。新增三语9组语境、18条作者句，零新词形/读音：EN6216/933/1830（64层）、ZH3442/924/1846、JA638/412/823。英文末层words仍为空，现18语境36作者句，6216首次rank及全部中日读音保持。独立有界回归实际1343过、0失败、2原ignore，96唯一组零筛除（完整lib+95集成目标），新12项只计一次；严格locked/offline全target/all-feature Clippy-Dwarnings通过（60.94s）。Cargo/父等待/独立核验均实际0。真实before4过8缺、独立7组预检57过与私有子进程均不叠加。仅有限作者输入，不推断偏好、发送消息或执行任务；生产算法、模型协议、25包/SDK及分页配额不变。仍0.9.8/build48，未提交；不是本轮全仓/Doc-tests/UI/原生/实模/性能/安装/CI验收，不升版、发包、替换个人安装或恢复自动化。下方旧发布与研发记录原样保留。
<!-- daily-consideration:end -->

<!-- release-0.9.8-source:start -->
**0.9.8 / build48 源码版本准备：只提交与推送，不创建对外发行。** 五批日常交流新增45组三语语境、90条作者句，零新词形或读音。EN6216/930/1824（64层）、ZH3442/921/1840、JA638/409/817；全部6216英文首次rank、既有中日读音/flags/顺序保持。新版本独立完整Rust实际2865过、0失败、32原ignore，306唯一组含Doc-tests且零筛除，60新项只计一次。另19项隔离UI门禁、严格locked/offline全target/all-feature Clippy-Dwarnings实际通过（60.24s），Python合计100项（含17项临时文件工具测试）独立记录，不加Rust总数。手工本地产物保留工具及文档纳入源码，不执行清理；生产输入算法、模型协议、25包/SDK和分页配额保持。远端准确提交CI尚待独立确认；不声明新包验收、实模、物理键盘、性能或个人安装。不创建0.9.8标签、GitHub Release或安装包，不恢复定时任务；已发布0.9.7仍是下载基线。

## Earlier development records (historical)

The pending and uncommitted labels below describe their recorded snapshots; they are not the current0.9.8 source-validation status.
<!-- release-0.9.8-source:end -->

<!-- daily-rescheduling:start -->
**当前未提交：日常临时变动、改期与计划调整（2026-10-25）**。复用既有词形和读音，新增三语9组语境、18条作者句：EN6216/930/1824（64层），ZH3442/921/1840，JA638/409/817。英文继续扩充第64层daily_presence_dialogue，words仍为空，本层现15语境30作者句，不增第65层；全部6216首次rank保持。独立有界回归1318过、0失败、2原ignore，93唯一组零筛除（完整lib+92集成目标），新12项只计一次；严格locked/offline全target/all-feature Clippy-Dwarnings实际main/owning均0（72.87s main process）。终局QA实际Cargo主进程/独立核验均0，父等待进程实际0，与Cargo主退出码分别记录。本轮真实before为4过8失败，main/owning实际101/1；before、独立预检及私有子进程结果不加终局总数。有限作者输入，不读取日历或代为改约；生产算法、模型协议、25包/SDK及分页配额保持。本轮不是全仓/Doc-tests/UI/原生键盘/实模/性能/安装/远端CI验收。仍0.9.7/47未提交，不在已发布0.9.7包内；不提交推送、发布、替换日常安装或恢复定时任务。下方旧记录完整保留为历史。
<!-- daily-rescheduling:end -->

<!-- daily-alternatives:start -->
**当前未提交：日常备选、选择与商量（2026-10-25）**。复用既有词形和读音，新增三语9组语境、18条作者句：EN6216/927/1818（64层），ZH3442/918/1834，JA638/406/811。英文继续扩充第64层`daily_presence_dialogue`，`words`仍为空，不增第65层；全部6216首次rank保持。独立有界回归1280过、0失败、2原ignore，87唯一组零筛除（完整lib+86集成目标），新12项只计一次；严格locked/offline全target/all-feature Clippy-Dwarnings实际main/owning均0（70.63s main process）。终局QA实际Cargo主进程/独立核验均0，父等待进程实际0，与Cargo主退出码分别记录。真实before为4过8失败，main/owning实际101/1；另保留首RED3过9失败：新fixture把作者句子串误认成独立JSON值，仅精确修该guard，作者常量、test名字和旧断言不动后重跑before。before、独立预检及私有子进程结果不加终局总数。有限作者输入，不推断真实偏好或代做决定；生产算法、模型协议、25包/SDK及分页配额保持。本轮不是全仓/Doc-tests/UI/原生键盘/实模/性能/安装/远端CI验收。仍0.9.7/47未提交，不在已发布0.9.7包内；不提交推送、发布、替换日常安装或恢复定时任务。下方旧记录完整保留为历史。
<!-- daily-alternatives:end -->

<!-- daily-rephrasing:start -->
**当前未提交：日常改口、换述与不确定表达（2026-10-25）**。复用既有词形和读音，新增三语9组语境、18条作者句：EN6216/924/1812（64层），ZH3442/915/1828，JA638/403/805。英文继续扩充现有第64层`daily_presence_dialogue`，`words`仍为空，不增第65层；全部6216首次rank保持。独立有界回归1242过、0失败、2原ignore，82唯一组零筛除（完整lib+81集成目标），新12项只计一次；严格locked/offline全target/all-feature Clippy-Dwarnings实际main/owning均0（58.68s main process）。终局QA实际Cargo主进程/独立核验均0，父等待进程实际0；父等待进程状态与Cargo退出码分别记录。首有界轮父等待进程143且Cargo主退出码未获证实，保留为未验收；真实before为4过8失败（main/tool101）；before、5目标预检、首未验收轮及子进程结果不加终局总数。仅有限作者输入，不是硬编码翻译、状态感知或事实核查，也不验证内容真伪；生产算法、模型协议、25包/SDK及分页配额保持。本轮不是全仓/Doc-tests/UI/原生键盘/实模/性能/安装/远端CI验收。仍0.9.7/47未提交，不在已发布0.9.7包内；不提交推送、发布、替换日常安装或恢复定时任务。下方旧记录完整保留为历史。
<!-- daily-rephrasing:end -->

<!-- daily-turntaking:start -->
**当前未提交：日常接话与配合（2026-10-25）**。复用既有词形和读音，新增三语9组语境、18条作者句：EN6216/921/1806（64层），ZH3442/912/1822，JA638/400/799。英文扩充现有第64层`daily_presence_dialogue`，`words`仍为空，不增第65层；全部6216首次rank保持。首次独立有界回归1406过、0失败、2原ignore，110组零筛除（完整lib+109集成目标），属于测试模块接线修正前的源码。首次严格Clippy因重复加载测试helper失败（exit101）；仅修两处测试模块引用。修正后的独立终局8集成目标/8组零筛除，75过、0失败、0ignore，新12项已含一次；严格locked/offline全target/all-feature Clippy-Dwarnings实际通过（10.52s）。两次测试结果分别记录，不相加，也不把修正前1406项称为修正后完整源码验收。仅有限接话、等候与配合输入表达；生产算法、模型协议、25包/SDK及分页配额保持。本轮不是全仓/Doc-tests/UI/原生键盘/实模/性能/安装/远端CI验收。仍0.9.7/47未提交，不在已发布0.9.7包内；不感知状态、不执行任务、不发送消息，不提交推送、发布、替换日常安装或恢复定时任务。下方旧记录完整保留为历史。
<!-- daily-turntaking:end -->

<!-- daily-presence:start -->
**当前未提交：暂离与回来继续交流（2026-10-25）**。新增三语9组语境、18条作者句，零新词形/读音：EN6216/918/1800（64层），ZH3442/909/1816，JA638/397/793。英文追加独立末层，原63层及全部6216首次rank保持；25包/SDK、生产算法、模型协议和分页配额不变。独立有界回归1316过、0失败、2原ignore，100唯一组（完整lib+99集成目标）零筛除，新12项只计一次；严格全target/all-feature Clippy-Dwarnings通过（58.49s）。仅有限输入表达，不探测在线状态、发送消息或安排任务；不是全仓/Doc-tests/UI/原生键盘/实模/性能/安装/远端CI验收。仍0.9.7/47未提交，不在已发布0.9.7包内，不替换日常安装、不恢复定时任务。下方旧准备期pending和未提交记录保留为历史。
<!-- daily-presence:end -->

<!-- release-0.9.7:start -->
**New source candidate: 0.9.7 / build 47 — Everyday dialogue fallback.** Six batches add 57 trilingual contexts / 114 authored sentences, three English forms and one Chinese Word reading. One independent versioned full Rust run passes 2,805 tests / zero failures / 32 original ignores in 301 unfiltered groups including Doc-tests; all 70 new cases occur once. Separately, 19 isolated UI gates and strict all-target/all-feature locked-offline Clippy with warnings denied pass (61.25s main process), not added to the Rust total. Only six data strings were withdrawn to restore old Chinese pools; production algorithms, model protocols, 25 packs/SDK and candidate budgets remain unchanged. Exact-commit remote CI and accepted-package validation remain pending; no 0.9.7 tag, download or publication is claimed. The six uncommitted batch records below remain historical. The personal installation and cancelled automation remain unchanged. See [0.9.7 preparation](../../../releases/0.9.7.md).
<!-- release-0.9.7:end -->

**当前未提交：日常进度反馈（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/915/1794（63层），ZH3442/906/1810，JA638/394/787。独立单次有界回归1194过、0失败、2原ignore，83组零筛除（完整lib+82指定集成目标），11新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（59.61s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅有限进度表达，不执行任务、发送消息或自动催办；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**当前未提交：日常澄清与确认（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/912/1788（63层），ZH3442/903/1804，JA638/391/781。独立单次有界回归1319过、0失败、2原ignore，99组零筛除（完整lib+98指定集成目标），11新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（62.53s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅有限澄清表达，不执行预约、任务或通用问答；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**当前未提交：日常沟通与安排（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/909/1782（63层），ZH3442/900/1798，JA638/388/775。独立单次有界回归1215过、0失败、2原ignore，83组零筛除（完整lib+82指定集成目标），10新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（71.93s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅日常沟通表达，不实际发送消息或安排预约；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**Uncommitted follow-up — small everyday essentials (2026-10-24).** Before 0.10.0, high-frequency authored EN/ZH/JA fallback remains the focus. This batch adds three English forms, `comb`, `handkerchief` and `hairbrush`, plus nine trilingual contexts / 18 authored sentences; no new Chinese/Japanese readings. The existing author-token index registers the three forms, while layer63 `daily_essentials_dialogue.words` stays empty. Totals: EN 6,216 / 906 / 1,776 (63 layers); ZH 3,442 / 897 / 1,792; JA 638 / 385 / 769. One independent bounded run passes 1,311 tests, zero failures and two original explicit ignores in 93 unfiltered groups (complete library plus 92 selected integration targets); the 11 new cases are included once. Strict all-target/all-feature Clippy with warnings denied passes. The retained first bounded run failed one old Japanese replay; only the new sock-author context was replaced by an existing raincoat word before the independent final rerun. Old 6,213 first-seen ranks, production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 remain unchanged. This is finite location dialogue, not item sensing, advice or device control. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is uncommitted and absent from the published 0.9.6 assets.

**Uncommitted follow-up — personal items and everyday accessories (2026-10-24).** Before 0.10.0, authored EN/ZH/JA fallback coverage remains the main development focus. This batch adds 12 contexts / 24 authored sentences, with no new word forms or readings: EN 6,213 forms / 63 layers / 903 contexts / 1,770 sentences; ZH 3,442 / 894 / 1,786; JA 638 / 382 / 763. One independent bounded run passes 1,135 tests, zero failures and two original explicit ignores in 70 unfiltered groups (complete library plus 69 selected integration targets); the 11 new cases are included once. Strict all-target/all-feature Clippy with warnings denied passes. Production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 are unchanged. This is finite text, not item sensing or accessory advice. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is uncommitted and absent from the published 0.9.6 assets.

**Uncommitted follow-up — living-room small items (2026-10-24).** Adds one Chinese Word reading (`yao'kong'qi → 遥控器`) and nine contexts / 18 authored sentences across EN/ZH/JA. English reuses all 6,213 forms: 62 layers / 897 contexts / 1,758 sentences; Chinese 3,442 readings / 891 contexts / 1,780 sentences; Japanese 638 readings / 379 contexts / 757 continuations. One independent bounded run passes 1,093 tests, zero failures and two original explicit ignores in 66 unfiltered groups (complete library plus 65 selected integration targets). The 12 new cases are included once; strict all-target/all-feature Clippy with warnings denied passes. Old data, historical windows/hashes, production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 remain unchanged except the explicit data append and current test-total maintenance. This is finite text, not item sensing or device/battery advice. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is not in the published 0.9.6 assets.

<!-- release-0.9.6:start -->
**New source candidate: 0.9.6 / build 46 — Everyday fallback and candidate continuity.** This includes six vocabulary batches (20 English builtin forms, 31 Chinese and 16 Japanese Word readings, 81 contexts and 162 authored sentences) plus three bounded Chinese/Japanese candidate-compatibility repairs. The independent versioned full Rust run passes 2,735 tests / zero failures / 32 original ignores in 294 unfiltered groups including Doc-tests. Separately, 19 exact isolated UI gates and strict all-target/all-feature Clippy with warnings denied (58.69s) pass. Exact-commit remote CI and accepted-package validation remain pending; no 0.9.6 tag, download or publication is claimed yet. The cancelled automation stays cancelled and the personal installation is unchanged. Earlier source/uncommitted/pending records below remain historical. See [0.9.6 release preparation](../../../releases/0.9.6.md).
<!-- release-0.9.6:end -->

A local-first, continuous-writing input method with word and sentence candidates,
optional language models, and a compact floating companion panel.

**[0.10.0 · Linux / IBus Alpha prerelease](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0), not latest stable.** Current validation targets **Ubuntu 24.04,
amd64, IBus, X11 or GNOME with XWayland**. Keep your usual input method available
as a fallback. Other platforms remain experimental.

**Historical publication record: 0.9.5 / build 45, source `de42d39`.** The exact-commit
[CI run 39894489515](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/39894489515)
passed all eight required gates. Accepted artifact `12476299897` supplied the Linux
`.tar.gz`, `.deb` and their two `.sha256` files; the four release assets were independently
validated and re-downloaded for byte comparison before publication. This is a non-latest
Linux Alpha prerelease; the personal installation was not changed and the cancelled
publication automation was not restored. Preparation-time pending statuses below are historical,
not the current publication status. See [0.9.5 preparation scope](../../../releases/0.9.5.md).
This intermediate Release is selected for retirement under the
[selective release policy](../../../linux-packaging-data.md#从-080-开始的发布要求);
its source tag and release notes remain. Use the 0.10.0 download linked above.

**Uncommitted follow-up — thermos and table-item locations (2026-10-24).** Adds nine contexts and 18 authored sentences across EN/ZH/JA, with no new word forms/readings. Totals: EN 6,213 / 894 / 1,752 (61 layers); ZH 3,441 / 888 / 1,774; JA 638 / 376 / 751. One independent bounded run passes 1021 tests, zero failures and two original ignores, in 60 unfiltered groups (complete library plus 59 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (1m 07s). New tests share a test-only helper; old suites are not migrated. Production algorithms, candidate budgets, models/protocols, 25 packs/SDK and versions stay unchanged. These are finite location expressions, not item sensing/control. Not full-repository, native, UI, real-model, installation, performance or CI acceptance. Earlier records remain historical.

**Uncommitted follow-up — wallet, coins and receipt locations (2026-10-24).** Adds nine contexts and 18 authored sentences across English, Chinese and Japanese without new word forms/readings. Totals: EN 6,213 / 891 / 1,746 (60 layers); ZH 3,441 / 885 / 1,768; JA 638 / 373 / 745. The independent bounded run passes 983 tests, zero failures and two original ignores, across 55 unfiltered groups (complete library plus 54 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (1m 06s). This is finite find-item text, not payment processing or financial advice. Production algorithms, model protocols, candidate budgets and 25 packs/SDK stay unchanged. No new full-repository, Doc-tests, native, real-model, installation or CI acceptance. Earlier snapshots below remain historical.

**Uncommitted follow-up — washroom item locations (2026-10-24).** Adds nine contexts and 18 authored sentences across English, Chinese and Japanese, reusing existing words/readings. Totals: EN 6,213 forms / 888 contexts / 1,740 sentences (59 layers); ZH 3,441 / 882 / 1,762; JA 638 / 370 / 739. The independent bounded run passes 929 tests, with zero failures and two original ignores, across 48 unfiltered groups (complete library plus 47 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (18.67s). This is not a new full-repository, Doc-tests, physical-keyboard, real-model or installation acceptance. No production algorithm, model protocol, pack/SDK byte or version changes. Earlier snapshots below remain historical.

<!-- entryway-validation-closeout:start -->

**Validation closeout — 2026-10-24 (uncommitted).** The fixed compatibility matrix passes **805 tests / zero failures / two original ignores**, across 20 unfiltered groups. All 44 new checks are included once. Chinese recovery excludes already-reachable authors and unsupported readings; Japanese nasal-parent preservation also handles literal Kana forms, only at the default twelve-row capacity. The earlier 22-failure complete run and exit-143 interruption remain recorded, not acceptance. The independent complete run passes 2,705 tests, zero failures and 32 original ignores, across 291 unique unfiltered groups including Doc-tests. Strict all-target/all-feature Clippy passes with warnings denied (55.99s). No version, commit, push, personal installation or automation change. The preparation snapshots below retain their earlier validation scope.

<!-- entryway-validation-closeout:end -->

**Current uncommitted follow-up: entryway and leaving home.** Adds six English forms, nine Chinese/five Japanese Word readings,
18 contexts and36 authored sentences from the accessory-inclusive970-file dirty snapshot, not clean HEAD.
Totals: **EN6,213/885/1,734 (58 layers), ZH3,441/879/1,756, JA638/367/733**; existing carry vocabulary and optional-pack words are not counted as globally new.
Japanese “鍵の置き場所” composes old Words; `teikiire` retains both adjacent vowels. This is finite find-item dialogue, not lock/weather/location control.
Two narrow Chinese repairs preserve verified adopted-syllable authors and a uniquely sourced terminal-`ng` pair; they do not add arbitrary prediction or polyphone inference.
Real failing before/after/repair runs remain saved; the four original entryway specifications and author wording are unchanged.
Focused acceptance passes **647 tests / zero failures / two original ignores**, across7 unfiltered groups (library plus6 targets); all35 new checks are included once.
Fresh independent complete regression and strict Clippy are still pending; older2642/full or735/bounded receipts are not this batch's final acceptance.
Version0.9.5/45 stays uncommitted/outside published packages; no installation, CI, automation, native/model or performance acceptance is added.
See the [entryway work record](development.md#daily-entryway-and-leaving-home--2026-10-23).

The accessory block below retains the preceding snapshot's scope and receipts, not the latest entryway totals or acceptance.

**Current uncommitted follow-up: desk accessories and cable storage.** Adds six English forms,
10 Chinese and five Japanese Word readings,18 contexts and36 authored sentences from the recycling-inclusive dirty snapshot.
Totals: **EN6,207/879/1,722 (57 layers), ZH3,432/873/1,744, JA633/361/721**.
Existing chargers/adapters/cases are reused; Japanese “充電器の置き場所” is a composed context, not a new dictionary Word.
This is finite location/packing dialogue, not electrical operations or advice; runtime/model logic, budgets and25 packs/SDK stay fixed.
The real before run found missing candidates; the first after run has18passes/one English failure at the broad `connec` root.
Only two new test tuples narrow that probe to `connecto`; old ranks/quotas and the failed log are retained.
The new bounded run passes **735 tests / zero failures / three original ignores**, across23 unfiltered groups: complete library plus22 integration targets.
Strict Clippy (58.85s), formatting,83 Python guards,250 links,tensor and preservation checks pass; this is not a new full-workspace run.
Version0.9.5/45 remains uncommitted/outside published packages; no installation, CI, automation, UI/native/model or performance acceptance is added.
See the [accessory work record](development.md#daily-desk-accessories-and-cable-storage--2026-10-23).

The recycling block below retains the previous snapshot's totals and validation, not the latest accessory batch's results.

**Current uncommitted follow-up: recycling and household coordination.** This finite
data-only batch adds **eight builtin English forms, 12 Chinese and six Japanese Word
readings, 18 contexts and 36 trilingual authored sentences**. `bins` already existed in
the optional English family pack and is counted only as newly builtin. Examples include
`cardboard / bin liners`, “纸箱先压平／回收日是哪天” and “段ボール／収集日／ペットボトル”.
Current totals are **EN 6,201 / 873 / 1,710 (56 layers)**, **ZH 3,422 / 867 / 1,732**,
**JA 628 / 355 / 709** (forms/readings, contexts, sentences). Source version/build remains
0.9.5/45; this follow-up is not in the published packages. Model/runtime algorithms, budgets,
25 recommended packs and SDK are unchanged. These are input expressions, not a collection
service, local sorting rules or a promise that all short-root candidate pools are unchanged.
The tests-first run recorded missing candidates in all three languages and the private-pack
workflow. The first focused run passed all 19 new tests but also exposed one stale current-total
assertion; that failure is retained, not labeled an all-green run. The first complete run then
found two historical English replay mismatches: the final token of `one\nline` now also
completes to `liner / liners`. A test-only projection checks the entire measured five-row
vector before removing those two added tail rows; the old corpus/goldens and runtime behavior
are unchanged. The fresh independent full run now passes **2,642 tests / zero failures /
32 original ignores**, across279 unfiltered groups including Doc-tests, with all19 new tests
included once. Strict all-target/all-feature Clippy with warnings denied (1m01s), formatting,
83 Python guards,250 audit links and source preservation checks pass. The earlier failed runs
remain recorded; these results do not add new UI/native-keyboard, real-model, installation or
performance acceptance. No installation, CI run or automation is started for this batch.
See the [recycling work record](development.md#daily-recycling-and-household-coordination--2026-10-23).

### Historical release and source-preparation snapshots

The following baseline/candidate and uncommitted labels retain their original development-time
scope. The kitchen and bedding batches are now included in published 0.9.5; the newer recycling
batch above is not. Their old local counts are not new-batch or remote-release acceptance.

**Published baseline: 0.9.3 / build 43.** It includes five everyday-vocabulary batches,
immutable-catalog query indexes and Japanese sentence-slot repairs. Its Linux release
and four package/checksum assets are published; this does not update the personal installation.
The **2,456 Rust tests**, **19 isolated UI checks** and strict Clippy recorded during
preparation remain historical local evidence, separate from exact-commit release acceptance.
See [0.9.3 preparation and release gates](../../../releases/0.9.3.md). Earlier
"uncommitted/unreleased" paragraphs retain their development-time scope; those five
follow-ups are now in 0.9.3. Its assets do not contain the newer batches below.

**New source candidate: 0.9.5 / build 45 — Kitchen and bedtime fallback.**
It includes the two kitchen/bedding batches after the0.9.4 source baseline `d2b0a1a`:
**14 builtin English forms,28 CJK Word readings,30 contexts and60 authored sentences**.
Current totals are **EN6,193/867/1,698 (55 layers)**, **ZH3,410/861/1,720** and
**JA622/349/697**. Runtime/model logic, earlier ranks/data,25 packs/SDK and all budgets
are unchanged; bounded short-root candidate pools may still change as vocabulary grows.
The new independent versioned full run passes **2,623 tests /zero failures /32 original ignores**,
across275 unfiltered groups including Doc-tests;83 Python guards,250 links and preservation checks pass.
Separately, **19 exact isolated UI gates** and strict all-target/all-feature Clippy
(1m03s, warnings denied) pass. Exact-commit CI/accepted-four-package checks remain pending;
the19 UI checks are not added to the full total.
Only after acceptance may the four Linux assets be published as an Ubuntu24.04 amd64/glibc2.39
Alpha prerelease, not latest. This does not restore the cancelled automation or update the
personal installation. See [0.9.5 scope and gates](../../../releases/0.9.5.md).
The source-candidate headings and uncommitted statuses below retain their historical scope;
the earlier2623/52.51s/419 receipts are not this new version's acceptance.

**Current source candidate: 0.9.4 / build 44 — Everyday spaces and candidate continuity.**
The dressing, room-comfort, stationery, reading and plants batches are now included in this
candidate: **28 English forms, 82 CJK Word readings, 90 contexts and 180 authored sentences**.
It also includes bounded Japanese nasal-progress/spare-word repairs and the offline catalog's
per-source versus merged-layer budget correction. The 25 recommended packs, SDK, model contract,
six-row page size and default twelve-row candidate list are unchanged.
The versioned complete Rust run passes **2,585 tests /zero failures /32 original ignores**,
across267 unfiltered groups. Separately, **19/19 isolated UI checks**, strict Clippy and source
preservation checks pass. The exact new commit must still pass every required CI
and accepted-package check before its four Linux assets can be published as a non-latest Alpha
prerelease. There is no new 0.9.4 download or personal-installation update yet.
See [0.9.4 scope and release gates](../../../releases/0.9.4.md).

**Current uncommitted follow-up: bedding and bedtime preparation.**
After the kitchen-inclusive working snapshot, this finite batch adds **six builtin English
forms, eight Chinese and three Japanese Word readings, 12 contexts and 24 trilingual authored
sentences**. Examples include `sheet / pillows / bedside`, “枕头／被子／床单换好了吗” and
“シーツ／毛布／パジャマ”; the existing 目覚まし Word is reused, not counted again.
Current totals are **EN 6,193 / 867 / 1,698 (55 layers)**, **ZH 3,410 / 861 / 1,720**,
**JA 622 / 349 / 697** (forms/readings, contexts, sentences). This is everyday input vocabulary,
not medical advice, sleep-effect guidance, alarm control or appliance actions. Version0.9.4/44,
runtime logic, model contracts,25 packs/SDK and all budgets are unchanged. No commit, installation,
CI query or resumed publication automation is part of this work. This batch's own tests-first
evidence and validation are in the [bedding work record](development.md#daily-bedding-and-bedtime-preparation--2026-10-23),
separate from the completed kitchen and earlier snapshots below.
The independent complete all-features run passes **2,623 tests /zero failures /32 original
ignores**, across275 unfiltered groups including Doc-tests; all19 new checks are included once.
Strict all-target/all-feature Clippy passes with warnings denied (52.51s), alongside formatting,
83 Python guards,250 audit links and preservation checks. This does not imply every short-prefix
pool is unchanged or new UI/native/model/install/performance acceptance.

**Uncommitted follow-up after `d2b0a1a`: kitchen preparation, tableware and clearing up.**
The finite batch adds **eight builtin English forms, 12 Chinese and five Japanese Word
readings, 18 contexts and 36 trilingual authored sentences**. Examples include `bowls`,
`cutting`, “我先把菜洗好／菜板／筷架” and “スプーン／フライ返し／水切りかご”; existing
筷子 and 箸 are retained, not counted again. Current totals are **EN 6,187 / 863 / 1,690
(54 layers)**, **ZH 3,402 / 857 / 1,712**, **JA 619 / 345 / 689** (forms/readings,
contexts, sentences). These are everyday input expressions, not recipes, health advice or
actions on kitchen appliances. Version/build stays0.9.4/44; runtime logic, model contracts,
25 packs/SDK and budgets are unchanged. No commit, installation or resumed release follow-up
is part of this work. See the [kitchen work record](development.md#daily-kitchen-preparation-tableware-and-clearing-up--2026-10-23)
for this batch's own tests-first evidence and validation, not the older results below.
The independent complete all-features run passes **2,604 tests /zero failures /32 original
ignores**, across271 unfiltered groups; all19 new checks are included once. Strict
all-target/all-feature Clippy passes with warnings denied (50.42s). This is not new
UI/native-keyboard, real-model, performance-timing or installation acceptance.

### Development-time batch snapshots included in 0.9.4

The following “uncommitted” statuses and 0.9.3/43 checks retain their original snapshot scope;
they are not versioned 0.9.4, remote CI, package or installation acceptance.

**Current uncommitted follow-up: balcony plants and everyday plant-related dialogue.**
Adds **eight builtin English forms, 12 Chinese and five Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include “花盆／叶子／帮我拿一下喷壶” and
“植木鉢／水やり／窓辺”. Current totals are **EN 6,179 / 857 / 1,678 (53 layers)**,
**ZH 3,390 / 851 / 1,700**, **JA 614 / 339 / 677** (forms/readings, contexts, sentences).
These are finite input expressions, not horticultural guidance, plant sensing or automatic watering.
Existing watering/balcony words are retained, not counted again; new entries can change bounded
short-prefix choices. The 25 packs/SDK, model contract and budgets are unchanged. A narrow Japanese
default-twelve-row spare-slot rule restores a proven parent Word without changing earlier rows,
raw input or slot metadata; smaller/larger limits retain their old behavior.
Version/build remains 0.9.3/43, with no commit, installation or package change.
The independent complete all-features rerun passes **2,585 tests /zero failures /32 original
explicit ignores**, across267 unfiltered groups including Doc-tests; all24 new checks are included
once. Strict all-target/all-feature Clippy passes (51.34s), as do formatting,83 Python guards,
250 audit links and preservation checks. The two failed earlier runs remain recorded; this is not
new native/UI/model-quality, latency or installation acceptance.
See the [plants work record](development.md#daily-plants-and-balcony-dialogue--2026-10-20)
for this batch's own validation; the reading/stationery results below retain their earlier scope.

**Current uncommitted follow-up: reading, book pages and bookmarks.**
Adds **four English forms, 12 Chinese and four Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include `bookmark`, “书签／这页还没看完”
and “栞／表紙／本棚”. Current totals are **EN 6,171 / 851 / 1,666 (52 layers)**,
**ZH 3,378 / 845 / 1,688**, **JA 609 / 333 / 665** (forms/readings, contexts, sentences).
This is finite fallback vocabulary, not book content, reading-state tracking or file actions.
Old reading order/flags and English ranks, 25 packs/SDK, model contracts and budgets are unchanged;
new compound entries can change bounded short-prefix choices. Version/build remains 0.9.3/43,
without commit, installation or package changes. The independent complete all-features run passes
**2,561 tests /zero failures /32 existing explicit ignores**, across262 unfiltered groups including
Doc-tests; all19 new checks are included once. Strict all-target/all-feature Clippy passes
with warnings denied (50.58s); formatting,83 Python guards,250 audit links and preservation checks pass.
Detailed validation is recorded separately
in the [reading work record](development.md#daily-reading-pages-and-bookmarks--2026-10-20);
the completed stationery/room results below are historical, not this batch's acceptance.

**Current uncommitted follow-up: paper, stationery and everyday writing.**
Adds **eight English forms, 12 Chinese and four Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include `inside my pencil case i keep`,
“铅笔有点钝了／这个字写错了／先拿张草稿纸” and “付箋／定規／筆箱”.
Current totals are **EN 6,167 / 845 / 1,654 (51 layers)**, **ZH 3,366 / 839 / 1,676**,
**JA 605 / 327 / 653** (forms/readings, contexts, sentences). Existing pen, paper and writing
entries are not counted as new; bounded short-prefix competition can replace old generated choices.
This is finite fallback data, not handwriting recognition,
open-ended composition or actions on notes/files. Model contracts,25 packs/SDK and all budgets
are unchanged; version/build remains0.9.3/43, with no commit, release or installation change.
After the documented new-test oracle corrections, an independent complete all-features run
passes **2,542 tests / zero failures / 32 existing explicit ignores**, across258 unfiltered
top-level groups including Doc-tests; all24 new checks are counted once. This is not the first
23/1 focused run plus six English checks combined. Strict all-target/all-feature Clippy passes
(49.91s), as do formatting,83 Python guards,250 audit links and source-schema checks; these are
not new native/UI/model/performance/installation tests. Details are in the
[stationery work record](development.md#daily-stationery-and-writing--2026-10-20).
The room/dressing totals and results below retain their earlier snapshot scope, not this batch's acceptance.

**Current uncommitted follow-up: indoor light, ventilation and room comfort.**
After the dressing snapshot, it adds **eight English forms, 12 Chinese and five Japanese
Word readings, 18 contexts and 36 trilingual authored sentences**. Examples include
`the air in this room feels stuffy`, “开窗通通风／把窗帘拉开／灯光有点刺眼” and
“照明／カーテン／風通し”. Totals are **EN 6,159 / 839 / 1,642 (50 layers)**,
**ZH 3,354 / 833 / 1,664**, **JA 601 / 321 / 641** (forms/readings, contexts, sentences).
These are finite input expressions, not ventilation advice, sensing or device controls.
The original forms/readings, 25 optional packs/SDK, model contract and list budgets remain protected.
The first five-target run records **23 passes and one actual Japanese first-page failure**:
`kankise` reaches 換気扇, but its second authored sentence is at index eight. A narrow
authored-stem proof fixes it while preserving older ticket pagination. A separate bounded
merge fix retains all16 pack slots without raising single-source or per-pack limits.
The independent complete all-features rerun passes **2,518 tests / zero failures / 32 existing
explicit ignores**, across253 unfiltered top-level groups, including all38 new checks once.
Both surfaced old regressions pass; focused93 and captured child output are not added again.
Strict all-target/all-feature Clippy passes (49.21s), as do formatting,83 Python guards,
250 source-audit links and exact preservation checks; these are not new native/UI/model/install tests.
This data is not in published 0.9.3 or the personal installation. Version/build is unchanged;
see the [current work and retained failure](development.md#daily-room-comfort-and-terminal-nasal-completion--2026-10-20).
Earlier batch totals and test counts below describe their own snapshots, not this current batch.

**Uncommitted follow-up after `6edfa3e`: daily dressing, fit and accessories.**
Adds **12 Chinese and four Japanese Word readings, 18 contexts and 36 trilingual
authored sentences**, reusing all 6,151 English forms without changing their ranks.
Examples include `my shirt feels tight`, “拉链卡住了／鞋带又松了／裤腰有点紧” and
`fasuna-` → “ファスナー”. Totals are now **EN 6,151 / 833 / 1,630 (49 layers)**,
**ZH 3,342 / 827 / 1,652**, **JA 596 / 315 / 629** (forms/readings, contexts, sentences).
These are finite input suggestions, not clothing recommendations or open-ended generation.
The 25 optional packs/SDK, model contract and candidate budgets are unchanged.
The new complete all-features regression passes **2,480 tests / zero failures / 32 existing
explicit ignores**, across245 unfiltered top-level groups. This total includes the24 new tests.
Strict all-target/all-feature Clippy, formatting and83 Python guards also pass; no new UI/native,
real-model, installation or performance-timing acceptance is claimed.
This data is not in the pinned 0.9.3 acceptance commit or the personal installation;
see [current validation and limitations](development.md#daily-dressing-fit-and-accessories--2026-10-20).

朱雀是以 Linux 为核心的连续写作输入法：用字母、拼音或罗马音起笔，同时获得词与句子候选。
这是供试用和反馈的 Alpha，并非已覆盖所有桌面和应用的稳定版。

## What works today

- English-first completion, Chinese Pinyin and Japanese Romaji/Kana with small offline vocabularies.
  Word and phrase/sentence candidates appear together when available; English continuations use
  the preceding phrase and preserve straight/curly apostrophes.
- The [local vocabulary](lexicons.md) is an immediate fallback when optional models
  are disabled, missing or failing. It covers common words and authored short sentences, not
  open-ended generation; local choices remain usable while model requests are pending.
- Optional data-only [offline packs and a small authoring SDK](../../../linux-packaging-data.md#离线词库包与制包-sdk),
  grouped by language/topic. CLI installation and enable/disable apply at the next host/panel startup;
  no executable plugins, live reload, automatic downloads or settings-panel package manager yet.
- Continuous drafts: spaces and punctuation keep writing open; Enter or a candidate click submits.
  Labels distinguish candidate type, model origin and ranking weight, not confidence.
- Linux dead-key/Compose sequences produce editable accents and symbols using the system Compose
  table, without changing the keyboard layout or Caps Lock state.
- Optional local or explicitly authorized HTTPS cloud models through Ollama and compatible chat APIs.
  Local discovery prefers an already installed LLaMA; no model is bundled or automatically downloaded.
- A floating panel/orb, screen keyboard, tray activation/restoration, guardian themes, sharper text,
  compact searchable settings and optional system title bars.
- Eight interface and translation languages: English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese. Translation needs a configured model and an explicit action;
  these are **not** eight native input dictionaries.
- Linux configuration backup and preview-first restore. Packages do not collect personal data.

The earlier **basic washcare-and-paper vocabulary** (now in the 0.9.3 source candidate) adds eight English forms
(`paper`, `soap`, `shampoo`, `toothpaste`, `tissue`, `tissues`, `conditioner`, `mouthwash`),
eight Chinese Word readings, six Japanese Word readings and **36 authored sentences**.
Current totals: **6,151 English forms / 827 contexts / 1,618 sentences (48 layers)**;
**3,330 Chinese readings / 821 contexts / 1,640 sentences**;
**592 Japanese readings / 309 contexts / 617 continuations**.
It also fixes first-page sentence slots for a verified unique Japanese long-vowel completion
such as `senganryo` → `洗顔料`, without altering its greedy conversion or Word weights.
The scoped rerun passes **1,872 tests, zero failures, five existing explicit ignores**;
26 new tests are included, not added again. This is not full-repository, native/UI, real-model,
installation or performance-timing acceptance. The 25 optional packs/SDK, model contract,
old reading order and first 6,143 English ranks remain unchanged. Not committed or installed;
see the [current vocabulary and validation record](development.md).

An **uncommitted Japanese sentence-slot repair** keeps both authored continuations on the first
page when a verified conversion is still in progress, including `汚れ ga`. The conversion stays
first at its original score; existing Words, literal text, model competition and 0–12 limits are
preserved. At six slots, literal text no longer displaces the second continuation. Frozen new
regressions and the unchanged old cleaning target pass. The expanded rerun records **1,846 passes,
zero failures and five existing explicit ignores**, plus strict Clippy; not a native/UI/install run.
See the [current repair record](development.md). Not committed, published or installed.

The preceding **uncommitted candidate-computation follow-up** replaces repeated English context scans,
Chinese continuation scans and Japanese written-form conversions with immutable startup-catalog
indexes. Five paired warm release replays improve mean candidate time by about **20% English,
83% Chinese, 29% Japanese and 40% Japanese with nine packs**; English has substantial run-to-run
variation. All **419 complete before/after candidate vectors** remain identical, with unchanged
vocabulary, ranking, budgets and model contracts. This is not a UI, native-keyboard, cold-start or
real-model speed claim. See [method, limits and current validation](development.md).
These changes are not in the published packages or the personal installation.
Broader scoped regression records **1,837 passes, one pre-existing Japanese first-page failure,
and five existing explicit ignores**; the saved before library reproduces the same failure.
That performance-only pass left the issue unfixed; the subsequent sentence-slot repair above addresses
it. Its failed result is retained, not rewritten as all-green. Strict Clippy passed for that snapshot;
the timing percentages precede the ranking repair, not a new measurement of current UI/model latency.

The earlier **uncommitted household-restocking vocabulary follow-up** adds **18 Chinese and 6 Japanese
Word readings, plus 36 trilingual authored sentences** for remaining supplies, refill packs,
checking spares and shopping-list reminders. Its working-tree totals at the time were
**6,143 English forms / 821 contexts / 1,606 sentences (47 layers)**,
**3,322 Chinese readings / 815 contexts / 1,628 sentences**, and
**586 Japanese readings / 303 contexts / 605 continuations**. These finite authored phrases
reuse existing English tokens such as `detergent`; the 6,143 indexed forms and their first-occurrence
order, model-independent fallback, 25 optional packs and SDK are unchanged. Basic English index
gaps remained at that point for `paper`, `soap`, `shampoo`, `toothpaste`, `tissue` and `tissues`; this does not complete
the household-supplies dictionary. The phrases do not track inventory, update a shopping list
or buy anything. Version/build remains **0.9.2/42**; this batch is uncommitted and not included
in the four published 0.9.2 assets or the personal installation. This batch's scoped regression
passes **1,027 checks / zero failures / two pre-existing opt-in model ignores**, plus strict Clippy;
this is not full-repository, native-keyboard, real-model or installation acceptance.
See the [lexicon record](lexicons.md).

The preceding **uncommitted storage-and-moving vocabulary follow-up** adds **18 Chinese and 6 Japanese
Word readings, plus 36 trilingual authored sentences** for making room, keeping things separate,
moving objects and returning them to their place. That batch's totals were
**6,143 English forms / 815 contexts / 1,594 sentences (46 layers)**,
**3,304 Chinese readings / 809 contexts / 1,616 sentences**, and
**580 Japanese readings / 297 contexts / 593 continuations**. This finite local fallback remains
model-independent; the English indexed forms and their first-occurrence order, 25 optional packs
and SDK are unchanged. These are input suggestions, not actions on objects or files. The new
data is not in the published 0.9.2 packages or the existing personal installation. Local scoped
regression passes **1,004 checks / zero failures / two pre-existing opt-in model ignores**, plus strict
Clippy; this is not full-repository, native-keyboard or installation acceptance. See the
[lexicon record](lexicons.md) for the exact scope and reviewed candidate changes.

The preceding **uncommitted call-recovery vocabulary follow-up** adds 24 Chinese/Japanese Word readings
and 36 trilingual authored sentences for frozen video, audio/video delay, changing headphones
and rejoining a call. These are input suggestions, not call/device controls. That batch's totals were
**6,143 English forms / 809 contexts / 1,582 sentences (45 layers)**,
**3,286 Chinese readings / 803 contexts / 1,604 sentences**, and
**574 Japanese readings / 291 contexts / 581 continuations**. See the
[lexicon record](lexicons.md) and actual scoped acceptance in
[development notes](development.md). Packages and the personal installation are unchanged.

The preceding **uncommitted route-coordination follow-up after 0.9.2's locked source** adds
**36 Chinese/Japanese Word readings and 44 trilingual authored sentences**. It covers
journey updates, the other entrance/exit, meeting positions and changed routes, not GPS or
live navigation. That batch's totals were **6,143 English forms / 803 contexts /
1,570 sentences (44 layers)**, **3,268 Chinese readings / 797 contexts / 1,592 sentences**,
and **568 Japanese readings / 285 contexts / 569 continuations**. All 23 frozen new checks
pass unchanged, including 25 complete old candidate-vector controls and a private nine-pack
Japanese fixture. Broader local acceptance is recorded separately in [development notes](development.md).
This batch does not change the locked 0.9.2 CI commit or its packages; an existing installation
still needs a later rebuild/upgrade. Earlier batch totals below retain their historical scope.

The released 0.9.2 source `4108bc0` includes the messaging, appointment/walk-in and aftercare/packaging
follow-ups below: **100 Chinese/Japanese Word readings and 122 authored sentences/continuations**
over 0.9.1, with the same 6,143 English indexed forms. It also includes the measured English/Japanese
candidate-computation optimizations. Historical “unreleased” batch records below describe their
development-time status, not separate current releases. Exact commit
`4108bc0deb8d08d9c421cfbd9c47cd8ed45e3883` passed all eight required gates in
[CI 39084362121](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/39084362121).
Its accepted `.deb`, `.tar.gz` and both checksum files passed package acceptance and
post-upload download verification. They were published as a Linux Alpha prerelease,
not latest stable. The [historical release notes](../../../releases/0.9.2.md) retain their source-preparation record;
this intermediate Release is selected for retirement, while its source tag and notes remain.
The later publication result is recorded here. Subsequent route, call, storage and restocking data is not included.
Historical local preparation of that exact 0.9.2 source passed **2,313 Rust checks / zero failures / 32 explicit ignores**,
the separate **19/19 private UI gates**, strict Clippy and formatting. Those local results
did not replace remote CI, clean-container installation or accepted-package checks, and do not
accept the current uncommitted vocabulary follow-ups.

Six configurable input shortcuts are available in **Settings → Shortcuts**:
previous/next candidate, previous/next page, adoption and English/Chinese switching.
Bindings support Ctrl or Alt, optional Shift, and a letter or Space (except reserved
Alt+Space combinations), with conflict checks,
recording cancellation and per-action/all-binding reset. They are input-context shortcuts,
not global hotkeys; desktop/application interception remains possible. See [shortcut rules](../../../shortcuts.md).

Released 0.9.1 extends everyday fallback for sharing, finding belongings, borrowing and returning items,
and adds independent Chinese character readings for known-word composition. Compared with the
0.9.0 source baseline, built-ins add **132 sentences/continuations**, **88 Chinese readings**
and **36 Japanese readings**; English indexed forms are unchanged. Its released totals are
**6,143 English indexed forms / 773 contexts / 1,510 sentences**,
**3,178 Chinese readings / 767 contexts / 1,532 sentences**, and
**522 Japanese readings / 262 contexts / 523 continuation records**.
Chinese candidate-source fixes retain authored continuations across selected partial-word and
homophone conflicts without increasing list limits. Missing readings and full-pool branch
competition remain; builtin 里/那 are not added. The model and offline-pack responsibilities
remain separated behind the existing public APIs.

The **local-input performance work included in 0.9.2 source** reduces warm candidate-computation mean
time by about **34% for English, 54% for Japanese, and 58% with nine Japanese packs** in five
alternating release-build replay pairs on this machine. All 419 sampled candidate vectors retain
exact text, labels, types, sources, score bits and order. English stops rendering after six unique
sentence choices; Japanese indexes immutable vocabulary membership, not users' drafts.
A Chinese memoization trial was withdrawn for insufficient measured benefit; no Chinese,
GUI/native-input, cold-start or model-latency speedup is claimed. See the
[replay tool](../../../../examples/input_hotpath_bench.rs), [equivalence test](../../../../tests/input_hotpath_equivalence.rs)
and [measurement limitations](development.md). Related local regression passes **1,417 checks**
with zero failures and three existing explicit ignores; strict all-target/all-feature Clippy passes.
This is scoped engine acceptance, not a complete native-input or release run, and does not change
vocabulary or the installed app.

The historical **aftercare and packaging development batch** adds six English contexts / twelve
sentences, eighteen Chinese Word readings / six contexts / twelve sentences, and ten Japanese
Word readings / five contexts / ten continuations: **28 CJK readings and 34 authored sentences**.
That batch's totals were **6,143 English indexed forms / 795 contexts / 1,554 sentences**
(43 ordered layers), **3,244 Chinese readings / 789 contexts / 1,576 sentences**, and
**556 Japanese readings / 279 contexts / 557 continuation records**. Examples include
`i still have the receipt`, `bao'zhuang` → `包装`, `gou mai ping zheng zen me ti gon` →
`购买凭证怎么提供`, and `hoshousho` → `保証書`. Existing return, refund, receipt and repair
vocabulary was already available; explicit Words and authored requests do not determine store
policies, refund eligibility or real after-sales progress. 包装 deliberately gains an exact Word.
The [English](../../../../tests/daily_aftercare_english.rs), [Chinese](../../../../tests/daily_aftercare_chinese.rs),
[Japanese](../../../../tests/daily_aftercare_japanese.rs),
[baseline compatibility](../../../../tests/daily_aftercare_compatibility.rs) and
[isolated nine-pack](../../../../tests/daily_aftercare_packs.rs) fixtures were fixed before the append.
Before-data checks confirm the new gaps; the first after-data focused run passes all **23 new
checks** without changing the five fixtures, including all 18 sampled complete old vectors.
Final scoped regression has **1,408 passes / zero failures / three existing explicit ignores**:
112 integration targets plus the complete library, with focused repetitions counted only once.
It adds parcel, Chinese/English shopping-pack and fallback coverage to the prior scope, not a
full-repository, native-input or installation run. Strict all-target/all-feature Clippy passes
(42.67 seconds), as do formatting, static guards and byte-preservation checks; the
[execution record](development.md) gives their exact scope.
At that batch's acceptance, all three follow-ups were uncommitted and
unreleased at **0.9.1 / build 41**, with unchanged algorithms, model contracts, optional packs and
actual installation. They are now included in the published **0.9.2 / build 42**; only the
separately described English/Japanese hot-path work changes production algorithms.
The two earlier batch descriptions below retain their own historical totals
and scoped acceptance records; they are not the latest totals or this batch's acceptance.

The messaging follow-up was still **unreleased at its 0.9.1 / build 41 checkpoint**, adding missed-message,
reply and arrangement-confirmation examples: eight English contexts / sixteen sentences,
24 Chinese Word readings / eight contexts / sixteen sentences, and twelve Japanese Word readings /
six contexts / twelve continuations. That batch reached **6,143 English indexed forms /
781 contexts / 1,526 sentences**, **3,202 Chinese readings / 775 contexts / 1,548 sentences**, and
**534 Japanese readings / 268 contexts / 535 continuation records**. Examples include
`i only just saw your messag` → `i only just saw your message`,
`gang kan dao ni de xiao x` → `刚看到你的消息`, and `messe-ji` → `メッセージ`.
The [English](../../../../tests/daily_messaging_english.rs), [Chinese](../../../../tests/daily_messaging_chinese.rs),
[Japanese](../../../../tests/daily_messaging_japanese.rs) and
[reviewed compatibility](../../../../tests/daily_messaging_compatibility.rs) tests cover this finite data append;
they do not certify physical keys, a real model or a new installation. One useful Japanese Word
addition intentionally enriches an old second page and lowers four existing sentence weights by
0.5 through the unchanged mixed-index formula; identical weights or rankings are not promised.
No conversion algorithm, provider contract, new indexed English form or installed package is changed.
This follow-up is **not included in the published 0.9.1 packages**.

The subsequent historical **appointment and walk-in follow-up** adds eight English contexts / sixteen
sentences, 24 Chinese Word readings / eight contexts / sixteen sentences, and twelve Japanese Word
readings / six contexts / twelve continuations: **36 CJK readings and 44 authored sentences**.
That batch's totals were **6,143 English indexed forms / 789 contexts / 1,542 sentences**
(42 ordered layers), **3,226 Chinese readings / 783 contexts / 1,564 sentences**, and
**546 Japanese readings / 274 contexts / 547 continuation records**. Records include
`qu'hao` → `取号`, `do i need to book ahead`, and `yoyakuhenkou` → `予約変更`.
Appointment, opening-hour and queue vocabulary already existed; some longer text was already
composable as a Sentence. This adds explicit Word entries and their own authored contexts,
not the first ability to type those characters, live opening hours or an actual booking service.
The [English](../../../../tests/daily_appointments_english.rs), [Chinese](../../../../tests/daily_appointments_chinese.rs),
[Japanese](../../../../tests/daily_appointments_japanese.rs),
[baseline compatibility](../../../../tests/daily_appointments_compatibility.rs) and
[isolated nine-pack](../../../../tests/daily_appointments_packs.rs) tests define the intended scope.
Before-data runs confirm the language gaps. All **23 new checks** pass unchanged, including the
two isolated pack workflows. Final scoped regression has **1,348 passes / zero failures / two
existing opt-in diagnostic ignores**; strict Clippy and preservation checks pass. This is not
a complete new native/release acceptance; [the execution record](development.md) lists the
exact included targets and explicit terminal-n filter, without counting focused repeats twice.
At this historical acceptance, both follow-ups remained uncommitted at **0.9.1 / build 41**, with unchanged algorithms/model
contracts and no installed-package, service or publication change.

The optional data-only catalog contains **25 packs: 8 English, 8 Chinese and 9 Japanese**,
including shopping, internet and additional Japanese topics. The unchanged installation limit is
**16 packs, including disabled packs**; catalog availability does not automatically install or enable them.
These finite presets do not read personal routines, contacts or typing history, and are not
open-ended model generation or a full Chinese/Japanese dictionary.

Chinese unfinished Pinyin and adopted mixed-language drafts retain intended spacing,
word/sentence choices and exact undo. Shared-engine committed-context undo cancels obsolete model work
without removing the current draft or deliberately selected candidate. Late replies/errors
cannot overwrite a newer draft; local candidates remain immediately usable during model failure.
The prediction protocol remains model-independent; no model is bundled or required.
Native-panel previews retain complete text and absolute selection indexes.
Japanese preset continuations better retain unfinished Romaji and grammatical endings before
punctuation. Standalone number choices can be undone; confirmed configuration changes invalidate
old adoption history even while a native view is showing. Held Back gestures cannot undo newly
adopted text after repaint or an undo/reapply roundtrip.

Optional pack changes still require host/panel restart. Space continues writing; deliberate
adoption and exact submission remain separate. Linux candidate preferences now persist by default,
learning only confirmed choices; the tray exposes save/clear status and can clear them at any time.
The local cache stores keyed fingerprints and counts, not raw drafts, but is still sensitive data.
See [0.9.2 release notes](../../../releases/0.9.2.md),
[lexicon scope](lexicons.md) and [known limitations](../../../known-limitations.md).
Source publication does not update an existing installation. Native Wayland, real-model quality,
broader Japanese morphology and non-Linux native behavior remain outside current acceptance.

## Install the Linux preview

Get the matching `.deb` and `.sha256` from the
[0.10.0 Linux Alpha prerelease](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0).
For 0.7.x, source tags can precede downloadable packages; use a release with attached packages or
build from source. A commit, version bump or source tag does not automatically create a Release.
Routine patches can accumulate toward selected milestones; each selected Release still needs
[complete, verified packages](../../../linux-packaging-data.md#从-080-开始的发布要求).
Use the verified 0.10.0 assets in the download directory on Ubuntu 24.04 amd64.
A working tree can contain later follow-ups absent from those packages; the Release notes
and source tag identify the shipped data:

```bash
sha256sum -c suzaku_0.10.0_amd64.deb.sha256
sudo apt install ./suzaku_0.10.0_amd64.deb
```

Installation alone does not activate an input method or start a user service. Register once
**as your desktop user, without sudo**, then open the panel:

```bash
/usr/bin/suzaku-tool linux-register install
/usr/bin/suzaku-tool linux-register verify
/usr/bin/suzaku-panel
```

Registration adds Suzaku and enables its user host service, preserving the active input method.
Choose **Activate Suzaku** in the tray to type; **Release / restore** returns to the previously
observed input method. Full quit restores it before stopping the managed host; hiding to tray keeps
the host running. GNOME tray visibility depends on the desktop's tray support.

Finish/cancel the draft and fully quit before upgrading. To remove, quit first, run
`/usr/bin/suzaku-tool linux-register uninstall` without sudo, then `sudo apt remove suzaku`.
Settings and backups are retained. The `.tar.gz` is an alternative, not a static universal binary.
See [Linux installation and data](../../../linux-packaging-data.md) for dependencies, tarball use,
service behavior and safe restore. Checksums provide integrity, not a signature.

## Type continuously

In a normal text field, type `hel`, press **2** to adopt `hello`, then **Space** and `world!`.
The entire `hello world!` remains editable in the candidate area/companion panel until **Enter** or
a candidate click. The target application's text is unchanged before submission. With no active
draft, ordinary digits go directly to the application, so `2026` and `12.5` need no candidate keys.

| Key | Action in an active draft |
| --- | --- |
| 1–6 | Adopt that candidate into the editable draft |
| Alt+0–9 / numeric keypad | Enter literal digits |
| Space / punctuation | Continue the draft, without submitting |
| Tab / Shift+Tab, arrows | Navigate candidates |
| Shift+Enter | Adopt the selected candidate without submitting |
| Immediate Backspace after completion | Restore the previous spelling |
| Ctrl+Backspace in English | Delete the last whitespace-delimited draft word |
| Enter / primary candidate click | Submit the candidate |
| Esc | Cancel the draft |

By default, **Ctrl+Shift+Space** switches English ↔ Chinese, even with an empty draft; from Japanese it
switches to English. It preserves unsubmitted text, does not repeat while held and leaves system
input-source shortcuts and lock states unchanged. It is local to Suzaku's ordinary IBus fields,
not a global activation or interface-language shortcut. A custom language binding replaces
this default until cleared. Japanese remains available in the tray.

In the focused panel, **F1** opens Settings → Shortcuts and **Ctrl+,** opens settings.
The optional home-row IBus profile adds **Alt+J/K** navigation, **Alt+H/L** paging and
**Alt+;** adoption in public, nonempty drafts. See the [shortcut reference](../../../shortcuts.md)
for scope, conflicts and repeat behavior; these are not global activation hotkeys.

Password/PIN and explicitly declared numeric/decimal/phone fields bypass these candidate rules.
Applications must report input purpose correctly. See [IBus behavior](../../../ibus-candidates.md)
for literal input, selection, privacy and recovery after an unconfirmed send.

## Choose the panel layout

Settings → Appearance → **Panel Layout** offers **Auto** (the default), **Follow caret** and
**Bottom dock**. Auto shows its detected choice: desktop/unknown environments prefer caret
following; Android and explicitly identified Linux phone/tablet or mobile/gaming sessions
prefer the bottom layout. Detection runs at startup, without input-event monitoring or network
access; touchscreen presence and screen size alone do not select a mobile layout. Explicit saved
choices remain overrides, while saving Auto keeps the preference portable between machines.
Bottom dock is a phone/tablet-style layout saved with the display settings. It
centers a borderless panel along the current input monitor's bottom edge, puts candidates
above larger screen-keyboard keys, and keeps candidate paging available when the keyboard
is folded. Short screens reduce its width before clipping content. Switching back restores
the previous floating width; the orb can still be used in either mode.

After a public native draft wakes the expanded bottom keyboard, submitting a candidate leaves
it available for the next screen-keyboard draft. It does not automatically open for every empty
field. This is a floating overlay, not a system keyboard that reserves desktop space or moves
other applications. Positioning depends on the window system and client cursor information;
native Wayland placement and physical touch comfort are not yet qualified.

## Models and privacy

Model suggestions are opt-in. Default discovery checks fixed loopback endpoints for models already
served locally. Suzaku does not install or start Ollama/llama.cpp for you.

```bash
/usr/bin/suzaku-tool model discover
/usr/bin/suzaku-tool model status
```

Follow the [model guide](../../../model-providers.md) to configure providers. Cloud use needs explicit
consent and may incur charges. Key values come from named environment variables, not settings/backups.
A local gateway can forward data elsewhere; trust and configure the server separately.

Enabled suggestions can send the current draft, local conversion and up to 160 characters of
committed context from the current focus session. Explicit translation sends its source draft.
Suzaku does not collect surrounding application text or write an input history to disk.
0.8.8 saves bounded, decaying IBus candidate preferences locally by default on Linux.
Confirmed explicit choices can adjust existing candidates; private fields do not learn or use them.
The tray can clear preferences and reports pending or failed deletion truthfully. Fingerprints,
counts and their key persist across restarts; this is not encryption or anonymity. Explicit
`SUZAKU_PREFERENCE_MODE=memory` keeps a host in memory-only mode without deleting an existing
disk cache. See [candidate learning](../../../ibus-candidates.md#偏好频率缓存).
See [privacy boundaries](../../../privacy.md); third-party provider retention is outside this guarantee.

## Build and contribute

Rust 1.95.0 is the CI toolchain. Install the dependencies in [CONTRIBUTING.md](../../../../CONTRIBUTING.md), then:

```bash
cargo build --locked --release --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features -- --test-threads=1
```

Native tests use private IBus/Xvfb sessions, not your active desktop. macOS/Windows CI checks
compilation, not native input acceptance. Report bugs with synthetic text and exact reproduction
keys, never real typing logs or credentials. For security concerns, read [SECURITY.md](../../../../SECURITY.md).

## Documentation

- [Code responsibilities and dependency boundaries / 代码职责与依赖边界](functional-network.md#当前-rust-模块与职责) — start here for architecture work; runtime routes and maturity evidence are separate
- [Current functional network / 当前功能链路网络](functional-network.md) — architecture/function maturity inventory and audited input paths
- [Input and candidates](../../../ibus-candidates.md) · [Model providers](../../../model-providers.md)
- [Keyboard shortcuts / 快捷键](../../../shortcuts.md) — EN/ZH toggle, opt-in IBus home-row layout and in-panel reference
- [Translation](../../../translation.md) · [Interface languages](../../../interface-languages.md)
- [Linux installation/data](../../../linux-packaging-data.md) · [Known limitations](../../../known-limitations.md)
- [0.9.1 release notes](../../../releases/0.9.1.md) · [Development history and architecture](development.md)

MIT licensed; see [LICENSE](../../../../LICENSE). Packages include dependency license metadata and available
license/notice files. External model weights have their own licenses.
