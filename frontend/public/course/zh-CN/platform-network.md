# 当前平台功能与模块网络

**2026-10-16，已收录于 0.6.2：**[绑定会话链路](platform-session-connection.md)为原存储 → 单次新
连接 → 同一业务事务内原目录／标记与 Source 固定 → 新鲜 Session／缺席读取 → 确认提交 → 显式 HTTP
投影。无连接池回退、签发或正常应用挂载，Task／Artifact 资源组合仍独立。

[格式 3 检查](platform-marker-expectation.md)另遵循可信目录／标记声明 → 单次新连接 →
全部身份先于标记行 → 关闭／丢弃，不启动宿主。

简洁源码职责及在线／准备层边界先读[当前架构图](architecture-map.md)，本页仍保留详细功能边。

源码版本 **0.5.8**（**2026-10-13**）已收录这些跟进；分日期的 `Unreleased`/未发布标记保留
当时实施阶段、源码版本和验证范围。源码收录不代表准备层 API 已挂载、浏览器保存已接通、
在线认证已切换、存储已迁移或运行实例已经验收。

[顺序下载](large-file-download-client.md)、[清单 HTTP](large-file-manifest-http.md)及
[准确清单客户端](large-file-manifest-client.md)、所选保存、观察／取消和配对分发路由现收录于
0.5.8；分日期的 0.5.7 证据仍保持原范围。

基础源码核对日期：**2026-10-03**。当前范围：**0.5.8 源码**，包括准备层认证加固、0.5.0 的
导航/文件名修复和 C2-K/L/M/N 内容准备层，以及此前的协作命令与本地任务 payload 编辑器。
评审保留 0.4.9 基线提交及原日期证据；源码收录
不是新的发布验收报告，也不代表正在运行的部署已更新。

0.5.2 收录的 2026-10-07 [登录拒绝路径改动](collaboration-auth-source.md)另使缺失账号经过受限密码计算。
它不改变此处拓扑，不接通公共 Session 签发，也不保证等时登录。
同版收录的显式过期会话清理也仅供内部使用，每事务至多选择 128 条；未接调度、运维入口或全局存量限额。
0.5.2 的来源会话读取和对账也在解码前检查存储到期时间；后续[登记时间防护](collaboration-reconciliation.md)
另在共用读取中保护可空退休时间与两类审计时间，不改变拓扑、Schema 或年龄/排序规则。
这两块均不代表所有数据库时间已安全。

0.5.3 收录的 [C2-O](task-draft-publication.md)新增独立的纯草稿导入/发布计划及给定内容绑定坐标。
新增内部连接不发布字节，也不接通浏览器或公共保存链路。

0.5.3 收录的 [C2-P](session-task-draft-publication.md)另将计划逐步接到已有授权 Artifact 发布及 Task
创建。调用者持有的结果只在内存；步骤失败或被丢弃会停止推进，不撤销此前提交，也不接浏览器或公共路由。

0.5.3 收录的 [C2-Q](session-task-draft-observation.md)另增固定未知目标的只读分支，每次用原当前
Session 读取。不含正文的结果保持尝试状态不变，不确认先前写入结果，也不允许重试。

0.5.3 收录的 [C2-R](session-task-draft-checkpoint.md)将已分配目标及报告进度导出为有界元数据。
纯解析结果只是调用者数据，不是尝试、日志或授权；codec 自身不保存、不读取存储、不调用 C2-Q，也不提供恢复连接。

0.5.3 收录的 [C2-S](session-task-draft-inspection.md)另将解析数据与显式受信任工作区、当前 Session
组合为一次已有授权读取。无需原尝试，只比较当前元数据，不证明原调用者/正文来源或存储代次，
不恢复 C2-Q、不收编确认，也不提供日志或重试策略。

0.5.4 收录的 [C2-T](session-task-draft-intent.md)另将真实 Ready/零确认尝试接到不可变意图登记及
当前 Session 记录读取。日志绑定精确所有者/账号代次及配置的命名空间名称，不记录资源派发或
结果历史。原 C2-P `advance` 仍不要求该记录，浏览器保存仍然缺失。

0.5.4 收录的 [C2-U](session-task-draft-dispatch.md)另增消耗原尝试的包装器及 Schema 2 步骤记录。
每个资源事务前先确认 Unknown 预留，再通过精确当前身份、意图及 nonce 检查，共同提交完成标记
和 Artifact 或 Task SQL。这只是存活的内部路径，不从记录恢复，不自动重试，也不接公共/浏览器。

0.5.4 收录的 [C2-V](session-task-draft-journal-inspection.md)另在当前精确所有者/账号权限下读取
Schema 2 意图与已记录步骤前缀。计数包含最终 Task，可选 Unknown 目标不暴露 nonce。
它不读取资源、不复活包装器；记录或缺失都不授权重试、清理或收编。

0.5.4 收录的 [C2-W](session-task-draft-journal-observation.md)另在同一授权事务捕获日志后读取
一个已记录资源，再复验原完整前缀与首次身份。不含正文的结果区分记录状态与当前元数据比较；
此操作不同于 C2-V，会实际读取文件，但不提供回执、恢复、重试或公共/浏览器连接。

按“架构 × 功能 × 实现 × 成熟度”查看请使用新增的[功能张量与评分规则](capability-maturity.md)和
[机器清单](../platform-capability-tensor.json)：70 个实现坐标、114 条定向边、19 条代表链路。
本文保留路由与模块拓扑；评分和证据独立维护，不计算整体完成百分比。

Cyanrex 正从 eBPF 教学应用转向以任务为中心的协作平台。目前并存三部分：已连接的教学运行链路、
独立准备中的通用后端，以及仅在本地工作的任务编辑器。代码只是任务 payload 的一种可选内容，
任务和平台都不以代码为前提。后续重点是连接这些部分，同时不把旧教师角色或浏览器自报身份直接
变成通用平台权限。

0.5.3 收录的 [AI Agent 接入](ai-agent-integration.md)将独立设置面板接到教师专用、修订约束的配置元数据
存储；调用者 SDK 工具桥只适配明确选出的已有操作。宿主负责模型传输、密钥和批准，不新增自主
AI 身份/委托连接，也不接通缺失的通用 Task 浏览器保存。

源码 0.5.5 上独立的 **2026-10-11 未发布[任务内容索引](task-content-index.md)**连接可信
Schema 3 元数据读取及当前 Session 下的所有者私有列表。每页默认 20 条，范围 1–32，
另加一条同样严格校验的前瞻；中立目录配置接纳手动及完整配置定义。游标位置不是权限、
内容可用性或全库健康证明，不增加 Artifact/文件读取、公共连接、UI、写入、安装器、迁移或恢复。

源码 0.5.5 上同日独立的**未发布[索引 HTTP 适配](task-content-index-http.md)**只增加显式
GET 传输 → 已有当前会话索引 → 有界元数据投影。精确 `/platform/v1/task-index` 使用严格原始
分页、私有响应头与共享响应存活期名额。它仍未挂载；旧内容路由及全部浏览器、SDK/OpenAPI、
存储、执行和恢复连接保持不变。

独立的 **2026-10-11 未发布[大文件核心](large-file-core.md)**增加严格共享清单 → 有界
给定流检查/分块复制/整文件校验。预开 Unix 描述符检查观察来源元数据变化，不打开路径。
此分支与传输方式无关，不进入 Artifact/Task 存储，也不挂载 HTTP。独立 Unix
[暂存分支](large-file-staging.md)增加准确清单/块绑定 → 独立工作准入 → 私有独占分块 → 同步/读回。
独立[上传意图账本](large-file-upload-intent.md)增加受信任所有者/账号代次 → 不可变清单 → 原子逻辑配额预留，
另行的[上传绑定本地回执](large-file-upload-staging.md)连接准确意图/原来源固定 → 独占暂存字节
→ 原文件确认。独立[分块日志](large-file-chunk-journal.md)增加确认 Unknown → 独占文件工作
→ 记录确认 → 新鲜原来源/文件复核。另行的[整文件字节观察](large-file-whole-verification.md)
连接原上传绑定 → 当前有序块/整文件摘要 → 新鲜原来源检查 → 原文件快照复核，
不增加日志状态、恢复回执或发布连接。另行[持久发布元数据](large-file-publication.md)连接
原来源和完整已确认日志 → 新鲜整字节 → 不可变插入确认 → 原来源/日志/发布/同文件检查。
元数据读取不恢复一次性回执、不证明当前健康、不授权下载，也不解决调用方 Unknown。
另行[Session 私有元数据](large-file-session-metadata.md)连接当前审计 Human 绑定 → 推导 owner／
账号代次 → 本人预留或选中上传／Published 元数据 → 原身份及记录复核 → 新鲜 Session／策略检查
→ 认证源提交确认，不增加字节 IO 或发布器连接。另行[纯文件授权契约](large-file-access-contract.md)
只连接所提供账号／版本元数据 → 准确活跃及时间匹配，不认证或保存权限；发布及接收撤权独立。
审阅恢复、物理分配、审计授权、集中传输和后续桌面配对仍是
[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)中的缺失连接。
原能力张量的分日期库存与成熟度证据不变。

不在 0.5.6 标签内的[未发布发布权限后续](large-file-publisher-grants.md)增加可信签发 → 不可变
审计 → CAS 撤销，以及当前审计 Session → 本人准确账号 → 选中授权／历史屏障 → 最终授权 →
提交确认。尚无授权管理员认证或发布权限消费连接；接收授权、字节传输及在线挂载仍缺失。
此实施说明不刷新分日期张量评分。

后续 [Session 授权管理](large-file-grant-management.md)增加可信准确管理者配置 → 当前审计
操作者 → 签发目标检查或旧主体撤销 → 原完整账本／延迟工作／认证源复核 → 新鲜 Session
→ 认证源提交。无 Registry 角色权限、持久在线配置撤权、发布消费、接收者、字节或公共
连接；分日期张量评分及证据保持不变。

独立 [Session 发布](large-file-session-publication.md)增加当前准确操作者／授权准入 → 无 SQL 锁
真实字节 → 原认证源／Registry／授权重入 → 原上传／日志屏障及只创建插入 → 延迟／全账本／
最终操作者与新鲜有效期 → 认证源提交 → 原确认后复核。仅是明确内部连接，不是接收者、传输
或在线挂载；分日期张量库存、评分与证据不变。

后续[接收授权连接](large-file-recipient-grants.md)为可信准确文件签发／撤销 → 独立保留账本／
历史 → 当前本人选中 → 原三表 SHARE 屏障及最终认证源授权 → 确认元数据；尚未连接 Published
查询、下载准入或字节。发布撤权不级联，分日期张量评分不变。

10 月 12 日[发布者分享连接](large-file-sharing.md)为当前发布者 → 原本人授权及准确 Published
选中 → 当前接收者 → 接收写入／延迟准确增量 → 原记录及最终新鲜权限／Session → 认证源
COMMIT 确认；独立原所有者撤销跳过发布许可／发布读取，不增加下载或字节边，分日期张量不变。

独立[下载清单边](large-file-download-manifest.md)为当前接收者 → 本人 Active 准确文件授权 →
记录原发布者／账号及准确 Published 版本 → 原记录 → 最终新鲜授权／Session 时钟 → 认证源
提交确认 → 私有清单。未连接字节、可复用权限、传输或客户端落盘边；分日期能力张量不变。

新增[分块字节边](large-file-chunk-read.md)：当前准确接收资格与认证源提交确认 → 无数据库锁的
私有有界文件校验 → 原 Session／元数据重入及最终新鲜授权／Session 时钟 → 认证源提交确认 →
持有额度的不可变字节。本认证源边自身不连接 HTTP、客户端保存或重启边，分日期张量不变。

独立[二进制 HTTP 边](large-file-chunk-http.md)：严格准确 GET 与私有请求准入 → 一次既有
当前接收者分块命令 → 固定二进制投影 → 持有额度的 Bytes 帧。两种名额跨 clone／slice 存活；
首帧十秒窗口拒绝过晚轮询。路由未挂载，不是监听／客户端保存／送达边，分日期张量不变。

另行[接收器边](large-file-chunk-receiver.md)：已提供准确响应 → 严格头与立即控制准入 → 私有有界
帧／EOF／摘要核验 → 持有缓冲／控制名额的既有独占暂存任务 → 本次不透明本地完成元数据。
不连接主动获取、整文件／续传或运行边，分日期张量仍不变。

显式[客户端边](large-file-chunk-client.md)：可信固定 Origin＋准确逐次选择器／Session → 整调用
准入 → 受控 HTTPS／明确字面环回响应头 → 逐帧接收器 → 请求／缓冲／工作所有权保留的既有
独占暂存 → 不透明本地确认。此 `fetch_and_stage` 边不接整文件／续传或发现，分日期评分不变。

另行[私有拼装边](large-file-chunk-assembly.md)：原根已有 `v1` 分块 → 真实任务共享准入立即
拒绝 → 准确有序／整文件预核验 → 完整清单绑定 `f1` 独占复制 → 0400 同步及原 inode 字节
读回 → 原输入／根复核 → 本次不透明本地确认。不增加用户保存路径、网络调度、收编、续传、
发布或在线挂载边；分日期张量不变。

## 状态说明

0.5.8 收录的[顺序下载边](large-file-download-client.md)：提供期望清单 → 独立整流程
准入 → 逐索引 HTTP／接收／暂存确认 → 同根拼装。首个错误／丢弃停止未来请求，当前真实
文件任务保留流程容量；空文件跳过 HTTP，不获得远端权限。不接清单获取、用户最终保存、
续传或运行挂载，也不更新分日期评分。

独立未发布的[清单 HTTP 边](large-file-manifest-http.md)：六项严格原始选择器／私人凭据 →
HTTP 准入／实际空正文 → 一次当前接收者 Source 观察 → 有界完整清单 JSON → 正文生命周期
释放。适配本身不增加文件 IO、最终字节权限或在线挂载，分日期评分不变。

新增[准确清单客户端边](large-file-manifest-client.md)：完整期望版本 → 原流程准入（组合时）
→ 独立元数据准入 → 受控固定 GET → 严格帧 EOF／JSON／完整版本匹配 → 顺序分块确认 →
同根私有拼装。空获授权文件仍须 GET，原提供清单入口保持空文件零 HTTP；每个后续 Source
块重新授权。不接界面保存／续传、在线挂载，不重评张量。

新增[显式私有保存边](large-file-save.md)：原拼装描述符证明＋宿主独立选择私有父目录／
basename → 共用暂存 worker 准入 → 实际原源核验 → 独占新文件校验复制 → 同步／原 inode
读回 → 最终源／输出／根复核 → 本地保存元数据。不请求 GET，不接公开／普通文件夹界面、
覆盖／采用、续传、自动清理或分日期重评。

新增[授权交付调用](large-file-delivery-client.md)将这些阶段合为一次显式操作，原整流程名额跨真实
块／拼装／保存任务及取消存活；首失败停止，不重试／采用，界面保存、续传和正常挂载仍独立。

独立[本人普通目录目标](large-file-user-save.md)以明确选择允许 0700／0750／0755，不放宽
私有暂存；核对原目录 UID／GID／模式并沿用实际保存，文件选择、共享可写目录、续传和挂载仍独立。

可选[实时交付观察](large-file-delivery-progress.md)仅提供单调用最新阶段与真实块ACK前缀，
不加回调或消费队列；全部暂存不是保存完成，关闭不是任务终止或回执，界面／持久进度／续传仍独立。

可选[协作式取消句柄](large-file-delivery-cancellation.md)在请求被选定时停止单调用等待。
丢句柄不取消，已派发任务仍持资源并可能完成；请求不是即时派发屏障，也不授予清理许可。

显式[整交付HTTPS验证](large-file-delivery-https.md)以一次性原生TLS对端测试既有链路；
传输身份不是当前文件授权，loopback验收不等于已部署局域网、界面或无特权服务隔离。

显式[分发路由组合](large-file-distribution-router.md)现从同一个所给Source及固定分块workspace
派生两个端点，HTTP预算仍独立；缓存配置一致性不是Session签发、正常挂载或特权隔离。

0.5.8 标签之后未发布的[真实Source／HTTPS联验](large-file-source-https.md)增加同一条原生
授权／文件／router／TLS／保存测试链，覆盖派发前撤权及真实响应正文取消；不改生产监听、
策略或分日期张量评分。

| 状态 | 本文含义 |
|---|---|
| 已接入运行链路 | 当前 Engine 或前端已实际组装；运行仍取决于相应部署条件。 |
| 后端准备层 | 已实现内部服务或显式本地运维工具并有测试，但未接入公共路由或常规启动。 |
| 仅本地 | 浏览器功能；内容不保存到 Engine，也不执行。 |
| 待接边界 | 当前源码尚未实现的连接或能力。 |

实际组装入口是 [`state.rs`](../../engine/src/state.rs)、
[`application.rs`](../../engine/src/application.rs)、
[`SidebarLayout.tsx`](../../frontend/src/components/SidebarLayout.tsx) 与
[`pages` 目录](../../frontend/pages)。新 Task、Artifact、Review 和 durable source 服务尚未进入
当前 `AppState`，也没有注册通用 Task、Artifact、Review 的公共 API。

## 浏览器入口

排除 Next.js 的 `_app`，目前共有 **18 个页面路由**。页面存在不等于获得权限：侧栏负责导航和会话
可见性，服务端动作仍由 Engine 守卫授权。

| 路由 | 当前职责 | 连接状态 |
|---|---|---|
| `/` | 跳转到仪表盘 | 已接入运行链路 |
| `/dashboard` | 工作区导航和概览 | 已接入运行链路 |
| `/login` | 密码与 TOTP 登录、安全返回路径 | 旧认证链路 |
| `/register` | 配置允许时的公开账户注册 | 旧认证链路 |
| `/otp-setup` | 密码认证后的 TOTP 配置 | 旧认证链路 |
| `/join` | 检查教师发现信息与兼容性、确认来源、消费邀请 | 旧课堂接入链路 |
| `/account` | 带确认的密码修改与账户删除 | 旧认证链路 |
| `/ebpf` | 源码、模板、保存脚本、检查、补全、运行与历史尝试恢复 | 已连接教学和 Linux 执行 |
| `/learn` | 实验进度、尝试与学习资料 | 学习 API 与文档入口 |
| `/learn/[...slug]` | 展示同步的中英文课程文档 | 静态课程内容，可跳转实验 |
| `/teaching` | 教师查看学生概览、尝试与反馈 | 旧教学授权和存储 |
| `/modules` | 教师管理头文件目录、下载、选用和删除 | 头文件服务，不是可执行插件安装 |
| `/events` | 历史、筛选、流、未读、导出与删除 | 按所有者隔离的事件 API 与 WebSocket |
| `/helper` | 环境和 Runner 就绪情况 | Engine 只读报告 |
| `/settings` | 事件和编译设置、指标、Agent 与课堂管理面板 | 现有设置、Runner 和课堂 API |
| `/terminal` | 结构化管理命令 | 类型化分发器，不是任意 shell |
| `/tasks/new` | 任务标题、可选文本 payload、本地导入导出与编辑 | 仅本地；外层仍检查登录 |
| `/editor` | 同一任务草稿容器的兼容入口 | 仅本地；不是第二套编辑器数据模型 |

本地草稿持有已接受的内容和本地修订号。受控编辑器持有 Monaco 模型与本地语言工具，不持有服务端
任务身份或执行权限。14 种语言配置包括 JavaScript/TypeScript 语义工具、JSON/CSS/HTML 结构化工具、
基础语言支持与纯文本；这不等于通用外部 LSP 服务、文件系统工程、Rust Analyzer、Pyright 或 clangd。
详见[编辑器说明](editor.md)。

2026-10-08 未发布跟进在任务层校验可移植语言提示，不再以这些编辑配置作为白名单。合法未知提示
在导入/编辑/导出中原样保留；实际能力采用纯文本并显示无工具提示，解析本身不修改元数据、修订或撤销历史。

## 已连接的运行模块

| 模块 | 主要源码 | 功能链与限制 |
|---|---|---|
| 运行组装与配置 | [`config.rs`](../../engine/src/config.rs)、[`state.rs`](../../engine/src/state.rs)、[`application.rs`](../../engine/src/application.rs) | 环境配置 → 服务组装 → 受保护的 HTTP/WebSocket 路由；不初始化协作准备层存储。 |
| 认证与教师权威 | [`auth_service.rs`](../../engine/src/services/auth_service.rs)、[`auth.rs`](../../engine/src/routes/auth.rs) | 账户 → 密码/TOTP → HTTP-only 会话 → CSRF/角色守卫；单人部署的初始教师也负责部署管理，这是现有权威模型。 |
| 课堂连接 | [`classroom.rs`](../../engine/src/services/classroom.rs)、[`课堂路由`](../../engine/src/routes/classroom.rs) | 教师描述/链接 → 独立协议和能力检查 → 绑定学生名的邀请 → 正常账户/会话流程；发现不等于认证教师或授权执行。 |
| 保存脚本 | [`script_store.rs`](../../engine/src/services/script_store.rs)、[`脚本路由`](../../engine/src/routes/scripts.rs) | 已认证所有者 → 保存/列举/删除 → PostgreSQL 或配置的本地持久化 → 显式加载到编辑器；保存脚本还不是通用 Artifact。 |
| 模块、头文件与命令 | [`module_manager.rs`](../../engine/src/services/module_manager.rs)、[`c_header_module.rs`](../../engine/src/services/c_header_module.rs)、[`command_dispatcher.rs`](../../engine/src/services/command_dispatcher.rs) | 清单发现 → 目录/控制状态；校验后的头文件 → 选用元数据 → 编译器；结构化终端命令 → 允许的模块操作/导航；模块启停不是动态代码加载。 |
| eBPF 工具与运行时 | [`eBPF 路由`](../../engine/src/routes/ebpf.rs)、[`ebpf_loader`](../../engine/src/services/ebpf_loader)、[`runner_manager`](../../engine/src/services/runner_manager.rs) | 源码/头文件 → 编译诊断或有边界的 Runner 操作 → Linux 加载/挂载 → 精确卸载与观测；内核操作有特权，不是对抗恶意学生的沙箱。 |
| Runner Agent | [`runner_agent_client.rs`](../../engine/src/services/runner_agent_client.rs)、[`runner_job_queue.rs`](../../engine/src/services/runner_job_queue.rs)、[`Agent 程序`](../../engine/src/bin/cyanrex-runner-agent.rs) | 签名注册/心跳 → 带租约作业 → 探测或编译报告；远程编译检查不引入远程内核执行，声明的隔离类型不是 VM/容器安全边界证明。 |
| 教学与学习 | [`learning_catalog.rs`](../../engine/src/services/learning_catalog.rs)、[`learning_store`](../../engine/src/services/learning_store)、[`学习路由`](../../engine/src/routes/learning.rs) | 实验 → 运行观测 → 规则评估 → 尝试/源码快照 → 教师反馈 → 所有者历史恢复；恢复只加载源码，不自动重跑。 |
| 事件与设置 | [`event_bus.rs`](../../engine/src/services/event_bus.rs)、[`事件路由`](../../engine/src/routes/events.rs)、[`设置路由`](../../engine/src/routes/settings.rs) | 运行发布 → 所有者历史/持久化 → WebSocket 交付或重新同步 → 浏览器；导出、删除与设置各有验证及确认写入边界。此 EventBus 不是通用存储的事务 outbox。 |
| 分发与部署 | [`cyanrex-release`](../../engine/src/bin/cyanrex-release)、[`docker`](../../docker)、[`脚本说明`](../../scripts/README.md) | 校验包与来源 → 审阅 SSH 目标及已安装包 → 显式 apply/验收；Rust 原生工具优先，保留兼容 Python/shell 工具；不隐含通用存储自动迁移。 |
| 公共集成契约 | [`openapi.json`](../../engine/openapi/openapi.json)、[`sdk-js`](../../sdk-js/README.md) | 已注册路由 → OpenAPI → 生成类型/操作 → SDK 使用方；签名 Agent 协议不进入浏览器 SDK，通用准备层命令也未进入公共契约。 |

现有教学门面已通过 [`TaskCatalog`](../../engine/src/services/task_catalog.rs) 连接内置
[`EbpfTeachingPack`](../../engine/src/domain_packs/ebpf_teaching/mod.rs)，抽取版本化定义与类型化规则
证据，但未改变旧教学 HTTP/存储链路。Domain Pack 是受信任的内置代码，不是下载式插件、执行调度器
或授权提供者。

0.5.5 收录的元数据拆分新增 [`TaskDefinitionCatalog`](../../engine/src/services/task_catalog/definitions.rs)：
显式配置的冻结元数据不依赖评估代码；原 `new` 注册一个包。Task 草稿和会话目录工作区可直接使用元数据，
`TaskCatalog<P>` 保留类型化评估。旧构造器、精确版本检查与授权事务保持兼容，不增加路由、
Schema、权限或执行连接。

仍基于源码 0.5.4 的 **2026-10-11 未发布多包跟进**新增可信显式元数据组合：1–256 个配置目录
→ 最多 256 个展平定义 → 已有 Task 草稿、Schema 2 会话目录或 Schema 3 存储/会话内容构造器。
嵌套组合遵守同一定义总上限；同包互不重叠注册及不同准确版本保留顺序，完整引用重复直接拒绝，
不去重或覆盖。它扩展内部配置，不组合提供器，不发现或认证包，也不增加执行、公共 HTTP 或
手工草稿发布流程的连接。

源码 0.5.4 上独立的 **2026-10-11 未发布评估绑定跟进**允许同一元数据路径同时准入有绑定及
明确无绑定的定义。Schema/策略两个 JSON 字段必须存在，同时有引用或同时为 null；无绑定目录
Task 不是手工 Task。已有完整定义检查拒绝同一引用下增加或移除绑定；类型化提供器注册/派发
和评估结果仍要求完整绑定。内部 Rust 字段改为成对 `Option`，旧已绑定 JSON 不变，旧二进制
拒绝新双 null 记录。不增加拓扑、授权、Schema 编号、迁移或公共 HTTP/编辑器/Review/执行连接。
详见[定义契约](task-domain-boundary.md)。

## 协作后端准备层

**2026-10-08 未发布的 C2-N 跟进**在已有准备层 HTTP 路径新增状态共享的请求/响应容量：
低成本检查 → 满额即拒的名额 → 正文/C2-M/序列化 → 响应正文完成或丢弃。默认四个名额，
可信配置允许 1–16；状态克隆共享，新建状态则独立。它不增加公共连接、全局内存/工作线程配额
或恢复链，不属于原 0.5.0 HTTP 证据或已发布的 0.5.4 版本。

独立的 **0.5.5 C2-X** [文本 Artifact 路由](text-artifact-http.md)经当前会话命令发布固定类型
文本并读取精确修订。其 2 MiB 传输 / 256 KiB 文本限制和状态族共享准入不改变 C2-N 的
64 KiB 预算；独立状态的容量相加。它仍未挂载，不自动合并或调用 Task 路由。

共享模型区分 Authority、Principal、Workspace、成员关系、部署权限、Task、不可变 Artifact 修订及
Review。它们使用带范围的引用，不是用户名、路径、编辑器 ID 或客户端角色声明。
概念起点见[协作基础](collaboration-foundation.md)。

**2026-10-08 目录内容跟进，源码 0.5.4，Unreleased** 将冻结定义目录与已有 Schema 3 内容存储、
Session 内容执行器组合。显式目录句柄准入完整的准确定义；手动句柄仍不带定义，两者互相
拒绝对方模式的记录。标题/标签/文本引用仍是实例内容，不是提供器的 Evidence。沿用旧新字节
及最终授权检查，不改 Schema、不执行规则，也不新增公共连接。原 HTTP 和 C2-P–W 发布
配置仍仅支持手动任务。

源码 0.5.4 上独立的 **2026-10-11 未发布 [C2-Y 目录内容适配](catalog-task-content-http.md)**
只通过另行构造的强类型状态提供 `/platform/v1/catalog-tasks`。创建选择服务端准入的准确定义，
读、编辑、状态变更保留既有目录命令的完整元数据、旧新文本及当前 Session 检查。有绑定和
无绑定定义仍是目录工作，不执行评估；原手动 JSON/工作区不变。安全、64 KiB/两秒请求限制和
响应正文准入共用，但独立状态族预算相加。不挂载正常应用、不发现目录、不切换客户端模式，
不自动合并、不签发 Session、不调用 C2-P–W、不接 OpenAPI/SDK/浏览器，也不迁移。

源码 0.5.4 上独立的 **2026-10-11 未发布 [C2-Z 目录草稿发布](session-catalog-task-draft-publication.md)**
把原 C2-O 计划接入独立强类型目录尝试，不属于已发布标签。不做 I/O 的准备先冻结准确完整定义，
再分配目标；每次推进发布一个 Artifact，或最后创建目录 Draft，在内存保留确认前缀及终止的
未知步骤。完整 `Some` 定义确认与手动 `None` matcher 分开。共享两项纯回执/清单检查不把
新尝试接到旧 C2-Q–W 观察/检查点/日志类型，也不接 HTTP、提供器、恢复、签发或浏览器保存。

源码 0.5.4 上独立的 **2026-10-11 未发布 [C2-AA 目录观察](session-catalog-task-draft-observation.md)**
对仍存活 C2-Z 尝试的准确未知目标增加一次当前授权读取，不属于已发布标签。它比较冻结完整
定义与有序文本，保留类型化损坏错误，不改变尝试状态。W16 只新增实现引用，不新增评分、
证据或拓扑；匹配/不可见不证明先前提交/回滚，也不允许重试。

| 模块 | 主要源码 | 已实现连接与剩余限制 |
|---|---|---|
| 文本 Artifact HTTP（0.5.5 C2-X） | [独立路由](text-artifact-http.md)、[`text_artifact`](../../engine/src/platform_http/text_artifact/mod.rs) | 当前 Session → 固定文本发布/精确修订 → 私有 JSON；无公共挂载、签发、浏览器/SDK 连接、日志流程、收编或恢复。 |
| 契约与旧系统投影 | [`models/collaboration`](../../engine/src/models/collaboration)、[`legacy_workspace.rs`](../../engine/src/services/legacy_workspace.rs) | 领域中立值类型与部分旧权限的纯预览；不是迁移或运行时授权。 |
| 持久化身份与策略 | [`collaboration_identity_store`](../../engine/src/services/collaboration_identity_store) | 显式命名空间 → 绑定、有效成员关系、部署授权与审计修订；不会隐式提升教师权限或收编环境中的数据库。 |
| 持久化认证与命令 | [`durable_source`](../../engine/src/services/auth_service/durable_source) | 账户世代/会话 → 同一事务内的审计身份、策略与生命周期命令。收录于 0.5.1 的来源 Schema 2 随登录/改密原子消费 OTP，仅全新安装，拒绝 Schema 1 不迁移；公共登录与账户端点仍使用旧 `AuthService`。 |
| 初始化与核对 | [`cyanrex-provision`](../../engine/src/bin/cyanrex-provision)、[`reconciliation`](../../engine/src/services/collaboration_identity_store/reconciliation) | 显式目标绑定的空命名空间初始化、私密交付配置材料及有界只读图核对；不是启动钩子、修复工具或旧数据导入器。 |
| Task 实例存储 | [`task_store`](../../engine/src/services/task_store) | 定义快照 + 精确输入 → 修订约束的生命周期 + 原子 outbox；当前新安装存储 schema 为 2，拒绝 schema 1，不做迁移。 |
| Artifact 修订存储 | [`artifact_store`](../../engine/src/services/artifact_store) | 私有不可变文件 + 元数据/outbox → 精确范围、修订和摘要读取；发布独立于任务变更，数据库写入失败可能留下私有文件。 |
| Review 存储 | [`review_store`](../../engine/src/services/review_store) | 精确目标/证据修订 → 不可变判断与评论历史；人工和规则判断分离，都不自动验收 Task。 |
| 会话授权的私人工作 | [`task_commands`](../../engine/src/services/auth_service/durable_source/task_commands)、[`private_work`](../../engine/src/services/auth_service/durable_source/private_work) | 新鲜 Session + 当前账户/成员关系 → 本人 Task、Artifact 与人工 Review，验证命名空间及写后状态；教师也不能借此读取他人私人工作。 |
| 目录准入与 Draft 换版 | [目录命令](session-catalog-tasks.md)、[换版命令](session-task-revisions.md) | 精确的服务端准入定义元数据；Draft 换版验证旧、新输入并只增加一个任务修订；不运行领域 provider，不生成证据或验收工作。 |
| 内容元数据（0.5.0 C2-K） | [清单契约](task-content-manifest.md)、[`task_content.rs`](../../engine/src/services/task_content.rs) | 给定 Task/Artifact 快照 → 元数据、所有者与字节一致性；不读写存储，不授权当前 Session。 |
| 内容存储（0.5.0 C2-L；0.5.5 目录模式） | [独立 Schema 3 存储](task-content-store.md)、[`content.rs`](../../engine/src/services/task_store/content.rs) | 显式手动/目录元数据 → Task/清单/outbox 原子保存并增一版；仍是可信存储，无 Session/Artifact 字节适配、公共 API 或 Schema 2 迁移。 |
| 授权内容（0.5.0 C2-M；0.5.5 目录模式） | [会话内容适配器](session-task-content.md)、[`content.rs`](../../engine/src/services/auth_service/durable_source/task_commands/content.rs) | 显式手动/目录工作区 + 当前 Session → 旧新精确 Artifact 文本 → Schema 3 原子编辑 → 最终授权与提交；无现用应用/浏览器接线。 |
| HTTP 内容边界（0.5.0 C2-N） | [显式路由](task-content-http.md)、[`platform_http`](../../engine/src/platform_http/mod.rs) | 有界 HTTP → C2-M → 私有版本化响应；仅独立构造，没有主应用挂载、Session 签发、公共发布或浏览器接线。 |
| 目录内容 HTTP（0.5.5 C2-Y） | [强类型独立适配](catalog-task-content-http.md)、[`catalog_task`](../../engine/src/platform_http/catalog_task/mod.rs) | 准确配置定义 + 有界 HTTP → 会话目录内容命令 → 私有 Task/清单/文本投影。手动模式独立；无评估、C2-P–W、自动合并、应用/浏览器/SDK 挂载或迁移。 |
| 目录草稿发布（0.5.5 C2-Z） | [不透明仅创建尝试](session-catalog-task-draft-publication.md)、[`catalog_draft_publication`](../../engine/src/services/auth_service/durable_source/catalog_draft_publication/mod.rs) | 准确冻结定义 + C2-O 计划 → 显式 Session Artifact 步骤 → 完整定义确认的目录 Draft。仅内存部分结果；无手动转换、日志/检查点、重试/恢复、HTTP/浏览器或评估；独立 C2-AA 观察不收编进度。 |
| 目录目标观察（0.5.5 C2-AA） | [只读比较](session-catalog-task-draft-observation.md)、[`observation.rs`](../../engine/src/services/auth_service/durable_source/catalog_draft_publication/observation.rs) | 准确未知目标 → 一个已有当前 Session Artifact/目录 Task 读取器 → 不含正文的当前比较。完整定义与有序字节，损坏仍报错；无状态改变、历史确认、回滚推导或重试。 |
| 草稿发布映射（0.5.3 C2-O） | [严格导入器](task-draft-publication.md)、[`task_draft_import`](../../engine/src/services/task_draft_import/mod.rs) | 严格草稿 JSON → 有界发布值；给定精确内容 → 清单。无 I/O、服务端 ID 分配、发布历史证明、授权派发或浏览器保存。 |
| 会话草稿发布（0.5.3 C2-P） | [仅创建步骤](session-task-draft-publication.md)、[`draft_publication`](../../engine/src/services/auth_service/durable_source/draft_publication/) | 固定内存尝试 → 每步一个会话 Artifact 写入 → C2-M Task 创建；保留确认前缀，未知步骤停止，无持久恢复、整条原子性或浏览器路由。 |
| 草稿目标观察（0.5.3 C2-Q） | [只读比较](session-task-draft-observation.md)、[`observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/observation.rs) | 精确未知目标 → 一个授权读取 → 不含正文的当前比较；不改状态、不证明先前提交、不推导回滚或提供恢复。 |
| 草稿元数据检查点（0.5.3 C2-R） | [有界检查点数据](session-task-draft-checkpoint.md)、[`checkpoint.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/checkpoint.rs) | 尝试 → 元数据导出/解析；报告进度只是调用者数据，不是保存、来源证明、授权或复活尝试。 |
| 检查点目标检查（0.5.3 C2-S） | [显式授权检查](session-task-draft-inspection.md)、[`inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/inspection.rs) | 解析数据 + 受信任工作区 + 当前 Session → 一次已有读取；仅元数据比较，不恢复原尝试，不提供来源证明、日志或重试。 |
| 不可变草稿意图（0.5.4 C2-T） | [意图登记与读取](session-task-draft-intent.md)、[`draft_intent_journal`](../../engine/src/services/draft_intent_journal/)、[`intent.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/intent.rs) | 真实 Ready/零确认尝试 → 会话绑定的不可变记录；新 Session 读取要求相同所有者/账号代次，不查询目标、不复活尝试、不接资源写前派发。 |
| 日志化草稿派发（0.5.4 C2-U） | [存活日志化包装器](session-task-draft-dispatch.md)、[`draft_publication/dispatch.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch.rs)、[`draft_intent_journal/dispatch.rs`](../../engine/src/services/draft_intent_journal/dispatch.rs) | 确认 Unknown 步骤 → 重新授权的资源事务共同提交标记/业务 → 内存确认。仅全新 Schema 2；原 C2-P 独立，无重启/重试或浏览器连接。 |
| 草稿日志检查（0.5.4 C2-V） | [已记录前缀检查](session-task-draft-journal-inspection.md)、[`journal_inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_inspection.rs)、[`inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs) | 当前精确所有者/账号 → Schema 2 意图/完整前缀 → 元数据计数及可选 Unknown。不读资源/文件、不泄露 nonce、不恢复尝试或重试。 |
| 日志步骤观察（0.5.4 C2-W） | [一个已记录目标](session-task-draft-journal-observation.md)、[`journal_observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_observation.rs) | 捕获完整意图/前缀/首次身份 → 真实 Artifact 或当前 Task 内容读取 → 精确复验及新鲜授权/提交。不写业务、不提供回执、恢复或重试，也不调用 C2-V 公共 API。 |

## 跨模块链路与尚未连接的边界

| 链路 | 当前状态 |
|---|---|
| 文本发布 → 精确引用 → 独立 Task 内容 HTTP 请求 | W23 ⇢ W13 ⇢ W12 ⇢ W11 ⇢ E02 是调用者控制的独立进程内夹具序列。不从 TextRouter 调 TaskRouter，无自动合并、原子批次、公共挂载或浏览器保存；各命令重新授权。 |
| 准确定义引用 → 独立目录 HTTP → 会话内容 | C2-Y 在 W13 源码切片内新增独立强类型适配，使用原目录事务检查，不改手动请求、不调用提供器，也不连接浏览器保存/公共部署。新增源码不刷新分日期张量拓扑或证据。 |
| 冻结目录定义 + 计划 → Artifact 前缀 → 目录 Draft | C2-Z 在 W15 源码实现内增加独立强类型尝试，每步单独重新授权；无批次事务、持久回执或旧手动/日志类型转换。此源码序列不刷新张量评分、证据或拓扑。 |
| 目录未知目标 → 当前读取器 → 比较元数据 | C2-AA 只扩展 W16 实现引用，不改张量拓扑/证据。原 token 准入先于当前读取器授权；比较不变成确认、回滚证明、收编或重试。 |
| 浏览器 → 旧 Session → 教学/脚本/事件操作 | 已连接；权限和所有权由 Engine 执行，不依赖侧栏隐藏。 |
| 源码 → 本地 Runner → 观测 → 教学包 → 尝试 → 教师反馈 | 已连接教学链路；规则评估结果不是通用持久化 Review。 |
| 浏览器远程检查 → 所有者作业 → 签名 Agent → 编译报告 | 已连接可选编译链路；Agent 失败不授予自动回退到本地执行的权限。 |
| 本地 TaskDraft → 选中 payload → 受控编辑器 → 本地导出 | 完全在浏览器内连接；登录/未读请求属于外层界面，不是保存 payload。 |
| 持久化 Session → 本人 Artifact 修订 → 本人 Task 精确输入 → Draft 换版 | 后端准备链已实现；尚无公共 API 或浏览器适配器。 |
| 持久化 Session → 精确的本人 Artifact 目标/证据 → 人工 Review 历史 | 后端准备链已实现；尚无跨用户审阅权限与验收。 |
| 旧账户/脚本/尝试 → 持久化身份/Artifact/Task/Review | 迁移、旧写入方隔离与切换待实现；没有隐式双写或数据收编。 |
| 浏览器保存任务 → 发布 Artifact → 创建/换版 Task → 冲突恢复 | W10–W14 提供内容契约、未挂载 HTTP 及纯映射，W15 组合内部仅创建步骤，W16 观察当前目标而不提供恢复；W23 另提供未挂载文本 HTTP 发布。仍缺显式登录/安装、公共派发、持久恢复与浏览器保存/冲突处理。 |
| 给定 Artifact 内容 → 草稿计划绑定 → 任务内容清单 | W02 ⇢ W14 ⇢ W10 只核对给定字节和元数据，不调用存储、不授权当前会话或证明过去发布；本地 U03 → W14 仍未连接。 |
| 草稿计划 → 显式会话发布步骤 → Task 创建 | W14 ⇢ W15 ⇢ W12 分别使用 I04 与 W05 进行当前授权/发布；独立事务、内存结果记录，不接浏览器或自动恢复。 |
| 未知尝试 → 当前目标观察 | W15 ⇢ W16 使用 I04 及一次 W05 或 W12 读取；观察时点独立，不返回正文或恢复状态。这条读分支不新增代表写入链。 |
| 仍存尝试 → 元数据检查点 | W15 ⇢ W17 导出报告进度，不含正文或凭据；codec 不调用读取器或复活尝试，下方 C2-S 检查是独立操作。 |
| Ready 尝试 → 不可变意图记录 → 授权记录读取 | 独立内部 C2-T 路径绑定当前所有者/账号代次。登记不要求或触发 C2-P 派发，读取既不观察目标，也不重建尝试。 |
| 消耗后的存活尝试 → Unknown 日志步骤 → 资源与标记共同提交 | C2-U 在当前绑定身份下显式连接一个资源与其步骤；每步成功后可继续，不确定结果或包装器丢失都不提供重试/恢复路径。 |
| 已记录日志步骤 → 显式当前授权检查 | W20 ⇢ W21 经 W19 与 I04 读取独立意图/步骤元数据；这是数据/读取边，不自动调用检查、不查询资源，也不新增恢复链。 |
| 已记录序号 → 同事务资源观察 | W22 与 W21 共用私有日志捕获，依赖 W19/I04 及一次 W05/W12 借用事务读取；不调用 W21 公共方法、不传递权限，也不增加代表恢复链。 |
| 检查点数据 → 显式当前授权检查 | W17 ⇢ W18 使用 I04 及一个 W05 或 W12 读取；无 W17 → W16 边，不证明历史调用者/提交，不收编状态或新增代表恢复链。 |
| 通用 Task → 执行计划 → 类型化证据 → 规则 Review → 验收 | 组合流程待实现；目录元数据与进程内规则评估不能单独建立此链路。 |
| 教师管理的隔离学生运行环境、托管 VM 生命周期、外部 LSP 进程 | 待实现；现有 Linux 执行和本地语言 worker 不构成这些隔离边界。 |

尤其需要区分：将 Task 变为 `Cancelled` 不会取消 Runner 作业，任务输入换版不会改写旧 Artifact 或
Review。这些影响必须通过显式适配器连接，并单独定义权限与失败语义。

## 地图维护方式

新增页面、组装字段、公共路由、provider、持久化存储或跨模块适配器时，同步更新本文、
[功能张量](capability-maturity.md)及对应英文版。标明连接是公开、内部还是仅本地，并说明失败边界。通过
[当前测试指南](testing-guide.md) 查找测试，不以旧通过数量代替验证。

[0.3.8 功能网络](functional-network.md) 及其机器清单保留历史源码快照；
[0.3.7 测试网络](testing-network.md) 是带独立修复证据的历史报告。不要刷新它们的数量与源码指纹，
让其看起来覆盖当前源码版本。分日期验收证据见[项目进度](project-status.md)，设计约束见
[整体架构](architecture.md)。
