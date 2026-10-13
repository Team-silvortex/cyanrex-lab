# 接收者清单 HTTP

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**，API **0.3.0**，**2026-10-12**。
本跟进位于已发布 0.5.7 标签之后，按[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
将既有[当前接收者清单命令](large-file-download-manifest.md)投影为显式构造、未挂载的 HTTP
路由。响应是私人元数据，不是字节健康或可复用传输权限。

## 可信组合与准确 GET

`platform_http::FileDownloadManifestHttpState::{new, with_capacity}` 接受既有
`DurableAuthSource`、`SessionFileDownloadWorkspace` 及规范可信浏览器 Origin。
`build_file_download_manifest_router(state)` 只构造此独立路由。缓存的
`workspace.file_scope()` 是 crate-private，来自原发布存储；请求不能选择 authority、工作区、
所有者、接收者、存储命名空间或文件根。构造不安装、不做 SQL／文件 IO，也不签发 Session。

只支持准确 **`GET /platform/v1/file-manifests`**。原始 query 必须恰有六项选择器：
`grant_id`、`upload_id`、`file_id`、`version_id`、`byte_length`、`sha256`，顺序可变。
缺失、重复、未知、空或格式错误的字段在准入／SQL 前失败。原始 query 至多 **512 字节**，
不接受百分号编码、加号或额外等号。ID 须为非空规范小写带连字符 UUID；长度须为规范 ASCII
十进制且不超过 **64 GiB**；摘要遵循共享小写 SHA-256／版本契约，零长度须准确空摘要。
`chunk_index`、请求提供 scope／owner、路径、Range 及所有 `if-*` 选择器都拒绝，不静默忽略。
六项仅标识目标，不授予权限。

其他路径私有 404，其他方法为 405 并带 `Allow: GET`；HEAD 仍拒绝，正文为空且长度零。
端点／query 核对先于凭据和准入，不轮询正文。沿用凭据策略，要求
`x-cyanrex-platform-request: 1` 及一个规范非空 `cyanrex_platform_session` cookie。
GET 可省略 Origin；提供时须准确等于配置的浏览器 Origin。Authorization 头、重复目标
cookie 及无效凭据都拒绝；不是 CORS、Session 签发或从传输主机名推导权限。

准入后实际请求须为空，沿用**两秒**收集期限、媒体／编码核对、trailer／错误拒绝及空帧
协作让出机制。不把声明的 Content-Length 当作实际空 EOF 的证明。

## 当前观察与 JSON 响应

恰好一次 `get_session_file_download_manifest` 调用负责当前接收者／账号推导、准确独立
Active 授权与 Published 记录选择、原 Source／Registry／记录复核以及最终数据库时钟的
Session／授权有效性。既有十秒 READ COMMITTED 认证源事务及配置的 READ ONLY 策略不变。
教师、部署、发布者和管理者角色都不能替代准确接收授权。

成功返回**直接完整 `FileTransferManifest` JSON**，含既有 `schema_version: 1`、scope／
版本、惰性显示名、分块几何、有序分块摘要及整摘要，不包装请求提供的清单、不暴露授权、
账号、隐藏绑定、nonce、路径或发布回执。`Content-Type` 为 `application/json`，显式
Content-Length 是实际序列化字节数。类型序列化后核对共享 **1152 KiB** 清单上限。
底层 **16,384 块**及显示名上界也约束此私人物化；长度检查不是分配失败恢复、实测 RSS 或
全局内存配额。

每个响应都有 `no-store`、`no-cache`、`nosniff`，没有响应 cookie、跳转、CORS、重试提示、
二进制分块头、Content-Disposition、range、ETag 或条件缓存状态。
缺失／他人／已撤销、最终时刻无效或不准确的授权／版本目标均为私有 404 `not_found`；
无效 Session 为 401，所映射的当前身份拒绝为 403；已选择记录损坏、配置或存储失败保持
脱敏 503，不伪装空／缺失清单。当前发布权限及发布者退休／删除不隐式撤销独立接收授权；
原记录的发布者／账号仍用于选择准确 Published 记录。

既有 schema-1 错误信封区分派发前 `not_attempted` 与派发后 `unconfirmed`，只含固定代码，
不含原始 SQL 诊断、token、所有者或隐藏键。只读元数据错误仍不证明一般回滚或重试保证。

## 响应生命周期与待连接边界

HTTP 准入默认 **4**，可信范围 **1–16**；状态克隆及分别使用这些克隆构造的路由共享名额，
覆盖请求收集、Source 工作、序列化及未发送 JSON 响应。满门限在轮询正文／SQL 前返回
503 `capacity_exhausted`／`not_attempted`；已准入错误正文也持有名额。最后帧／结束、错误、
正文丢弃或请求取消释放名额。分别构造的状态各有可叠加 HTTP 预算；清单准入与既有
分块／下载门限独立。

与二进制适配不同，元数据正文没有 Source 字节工作租约或首帧十秒窗口。正文结束后保留
JSON Bytes 不保留 HTTP 准入；取消不强杀网络／数据库，也不证明回滚，不增加后台计时器
或续跑。返回快照可在后来撤销后继续存在，有效性是 Source 最终采样核对，不是正文轮询时刻、
送达确认或后续各块持续权限。

原[顺序下载入口](large-file-download-client.md)仍要求调用者提供期望清单；独立新增的
[准确清单客户端](large-file-manifest-client.md)现获取此端点，并可组合顺序私有下载与拼装。
[二进制分块路由](large-file-chunk-http.md)继续独立授权每个后续请求。此元数据适配不读文件
字节，也不私有暂存；两个路由都未挂到正常 AppState／应用启动。Session 签发、发现／列表、
公开 OpenAPI／SDK／UI、用户选路径最终保存、续传／恢复、在线迁移／切换及桌面配对仍独立。
版本和依赖不变。

## 验证入口

显式[分发路由](large-file-distribution-router.md)从一个所给分块workspace派生元数据配置并共享
所给Source，保留原handler及独立JSON预算；旧独立builder仍兼容且未挂载。

`file_download_manifest_http_tdd` 覆盖关闭 Source 的严格准入及 opt-in 真实当前接收者 SQL。
`scripts/test-file-download-manifest-http.sh` 先检查编译后的准确 ignored 清单，再执行。
私有契约单元覆盖类型解析／错误／投影边界；`fileManifestHttp` 守卫只检查组合与清单，不代替
权限或送达验证。当前中英文说明及课程副本描述此边，不改 **106 段历史状态**、
**38 份旧执行尾部**、已发布 0.5.7 验证或分日期能力张量。

## 实际执行 · 2026-10-12

惰性 API 骨架对实际 17 字节 Session 发布及独立当前无角色 Human 接收授权正常编译。
夹具完整清理后，行为 Red 是 **0 通过／1 个预期失败**，实际 HTTP **503 而非 200**，
零 warning。同一准确用例 Green **1 通过**、零 warning，验证直接完整清单 JSON、实际
Content-Length、固定私人响应头，以及 Source／Registry／授权／上传／日志／Published／
文件快照不变。最初夹具尝试出现根 scope 缺失 `E0425` 及未使用 reexport 警告，引入实际
嵌套夹具的 scope 后两者消除；该准备失败不算行为 Red。

| 新鲜选择 | 实际结果与范围 |
|---|---|
| 默认清单 HTTP 目标 | **12 通过／8 opt-in ignored**，零 warning；关闭 Source，规范路径／方法／HEAD／query／凭据、实际空正文／帧／错误／两秒期限、默认四名额克隆共享、独立状态预算、满门限优先顺序、取消、JSON 最后帧／丢弃及保留 Bytes 对照 |
| 私有契约单元 | **10 通过**，零 warning；六项类型选择器／可信 scope、规范词法及几何上界、私人错误映射、准确直接 JSON／Content-Length、声明最多 16,384 块的元数据及编码后字节上限，不执行 64 GiB 载荷 |
| 新准确 SQL runner | **8 条唯一用例通过**，零 warning；完整 17 字节／空／4 MiB + 17 清单、准确当前接收者／无角色绕过、Source 派发前正文等待期间撤权、当前生命周期、选中损坏及他人过滤、仅原 Source IO、真实锁／时钟的到期／生效、SQL 取消／排锁及未发送／已结束／丢弃 JSON 响应生命周期 |
| 相邻 SQL runner | **10 既有清单＋6 二进制 HTTP＋1 顺序下载通过**，零 warning；含新 runner 共 **25 条唯一新鲜 SQL**，基线 Green 重跑不额外计数 |
| 完整后端门禁 | `scripts/quality-gate.sh --backend-only`：**973 Rust 通过／850 opt-in ignored／90 结果组**、**311 公共检查通过**，零失败／warning；12 条新默认及 10 条私有单元已计入 |

原生组合／库存定向检查 **20 通过**。独立源码、测试及中英文复查未发现剩余可行动缺陷。
测试复查保留等待任务完成前的 abort-on-drop 所有权，补有界收尾／恢复探测及完整二进制
响应头缺席断言。未新跑前端／SDK／安全审计、浏览器、清单客户端获取、实际元数据网络／
TLS 部署、字节健康／64 GiB 载荷、吞吐／RSS、用户保存／续传或在线认证迁移验收。
相邻顺序下载桥接的真实环回请求，不把此进程内 Axum／SQL 元数据矩阵变成新的网络送达证明。
ignored 库存不等于默认验收，25 条显式 SQL 与默认 973 分开计数。

所有源码编辑者冻结后由 root 串行执行 Cargo。最终临时 PostgreSQL 16 审计确认准确自有
私有数据路径、不监听 TCP、**零夹具 Schema、零其他客户端**。停止后只删除已验证的
`data`／`socket`，释放 **94308 KiB（约 92 MiB）**可重建测试数据；日志保留于
`/tmp/cyanrex-download-manifest-http-5ndLJ7Q5`，未移除项目、部署或无关缓存数据。
版本／依赖、在线 API／SDK／UI、已发布 0.5.7 报告、**106 段旧状态**、**38 份旧执行尾部**
及能力张量不变。中英文源文档／课程副本已同步；未提交／推送。
