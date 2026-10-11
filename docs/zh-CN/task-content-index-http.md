# 私人任务内容索引 HTTP 适配

已收录于源码版本 **0.5.6**。下方分日期实施和验证保留原 0.5.5／未发布阶段及范围；
源码收录不启用网络传输或公共 API。

状态：**未发布的内部准备层**。决策日期：**2026-10-11**。源码版本：**0.5.5**。
此独立构造路由通过已有[当前会话索引](task-content-index.md)返回有界私人 Task 元数据。
它未挂载到现用应用，不新增 Session 签发、浏览器接线、OpenAPI/SDK 路由、存储安装或迁移。

这是服务层索引之后同日的独立跟进。原手动与目录内容路由保持不变：其集合路径 GET 仍返回
405，原请求仍拒绝查询。列表只使用 `/platform/v1/task-index`，不自动合并路由。

## 可信配置

Unix 上的 `platform_http::TaskContentIndexHttpState::new(source, workspace, origin)` 接收
`DurableAuthSource`、`SessionTaskContentIndexWorkspace` 和规范允许来源。
`build_task_content_index_router(state)` 返回独立 Axum 路由。构造不读取环境配置、不安装
命名空间、不启动监听，也不修改 `AppState`。

传入的工作区固定命名空间、作用域，以及仅手动或显式中立目录准入策略。请求不能选择
所有者、权限域、空间、命名空间、定义或目录。中立配置接纳手动记录和冻结目录中的完整定义，
包括有评估绑定与明确无评估绑定的定义；准入或列表均不调用提供器、评估、Review 或执行。

`new` 默认允许**四个**在途请求。可信组合可使用
`with_capacity(source, workspace, origin, max_in_flight)`，容量为 **1–16**。状态克隆及用这些
克隆构造的路由共享名额；独立新建状态的预算互相独立且相加，不是全进程内存、工作线程、
网络或代理缓冲配额。

来源配置只接受规范 HTTPS，或 localhost/字面回环地址上的 HTTP。这不配置 TLS、不认证代理、
不签发 Cookie，也不保证未来监听可安全部署。宿主仍须明确安排隔离认证源及受保护的 Session 交付。

## 端点与原始查询

唯一端点是精确的 `GET /platform/v1/task-index`，正文须为空。同一路径的其他方法返回
**405** 和 `Allow: GET`；HEAD 错误无响应正文，带 `Content-Length: 0`。其他路径，包括尾部
斜杠或别的路由前缀，返回私有 **404**。

只接受下列可选原始查询字段，先后顺序不限：

| 字段 | 接受值 |
|---|---|
| `after` | 规范小写、带连字符、非零 Task UUID |
| `limit` | 无前导零的规范十进制整数，范围 **1–32** |

没有查询时默认 **20** 条；只有 `after` 时仍使用该默认值。查询存在时须为 **1–128 字节**。
单独 `?`、空值、未知或重复字段、缺少或额外 `=`、尾部 `&`、百分号编码、`+`、空白、正负号、
小数和指数形式均拒绝。直接解析原始 URI 字节，不做 URL/表单解码或归一化。查询中的 UUID
由可信工作区补全作用域，不接受客户端提供 `TaskRef` JSON。

例如 `?limit=1` 和 `?after=10000000-0000-0000-0000-000000000001&limit=32` 是合法形状。
校验不证明游标存在或属于调用者；选择仍按当前 Session 所有者的 UUID 升序，严格从游标之后开始。

## 私有响应投影

成功响应为 **200**，准确包含 `{schema_version: 1, entries, next_after}`。每个条目有以下
七个字段；服务层 `TaskContentIndexPage` 和 `TaskContentIndexEntry` 仍不是传输 DTO，不增加
序列化或请求反序列化。

| 字段 | 含义 |
|---|---|
| `reference` | 完整带作用域的 `TaskRef` |
| `revision` | 当前任务修订 |
| `title` | 当前任务内容标题，不是定义标题 |
| `status` | 当前生命周期状态，不是评估或验收 |
| `definition_ref` | 准确定义引用，手动记录为显式 `null` |
| `updated_at` | 当前任务更新时间 |
| `payload_items` | 清单项数量，不是内容或字节可用性 |

只有存在已校验前瞻记录时，`next_after` 才是最后一条返回条目的完整 `TaskRef`；否则为显式
`null`。取其 `task_id` 作为同一可信路由下一页的 `after`。空页是列表成功，不是未找到：

```json
{"schema_version":1,"entries":[],"next_after":null}
```

响应不含所有者、文本、清单、Artifact 引用、完整定义或总数。页面是私人当前元数据，不是
固定的跨请求快照、完整清单、内容可用性检查或保存回执；UUID 顺序也不承诺时间顺序。

## 请求准入与授权

请求复用专用 `cyanrex_platform_session` Cookie 和单个
`x-cyanrex-platform-request: 1` 标记。跨全部 Cookie 头只能有一个规范非零 UUID，头总量不超过
8 KiB。GET 可省略 Origin，但提供时须单个且精确匹配。任何 Authorization 头都直接拒绝，
即使同时提供有效的平台 Cookie；旧 Cookie 不授予平台权限。不提供 CORS 或 Referer 回退，
固定标记不是秘密 CSRF token。本适配不新增 token
audience 绑定、不签发 Cookie，也不防止已窃取 Session 的重放。

端点/查询与凭据检查先于满额即拒的准入。容量满时返回 **503**
`capacity_exhausted` / `not_attempted`，不轮询正文，也不派发 Session/SQL 工作。
已准入 GET 只接受**零个实际正文字节**，拒绝内容编码，沿用**两秒整份正文期限**，连续空帧
也受期限约束。如提供 Content-Type，只接受精确 `application/json` 或
`application/json; charset=utf-8`，不能重复。

处理函数只调用 `list_session_content_tasks`。其原来源事务先派生当前账户和成员资格，再验证
有限任务头、清单及最新 outbox，包含前瞻和空页所需列；提交确认前复核命名空间身份、账户、
成员和准确 Session，并使用新鲜数据库时间。损坏或完整定义漂移让整页失败，不过滤隐藏记录。
不读取 Artifact 命名空间、blob 或文件，因此正文重新打开必须失败时，列表仍可能成功。

名额覆盖原命令、JSON 投影及响应正文，直到 EOF、错误或丢弃；已准入错误响应同样持有名额。
取消释放准入，不终止已派发阻塞工作，也不证明先前回滚。原 Session/SQL 期限不变，不新增
命令超时、重试队列、`Retry-After` 或恢复协议。

## 错误与验证范围

所有响应使用 `no-store`、`no-cache` 和 `nosniff`。错误只包含版本化 `error.code` 和
`error.outcome`，不返回原始存储诊断、token 或所有权细节。派发前拒绝使用 `not_attempted`；
所有服务错误保守使用 `unconfirmed`。无效/过期 Session 映射 401，成员权限拒绝映射 403，
元数据损坏或存储不可用映射通用 503 `service_unavailable`。这些结果不是先前写入回执或重试许可。

独立目标 `task_content_index_http_tdd` 覆盖进程内请求。其**八条 SQL 用例**属于单列源码库存：
混合页面；所有者隔离与非法查询；生命周期与再创号；完整任务头/清单/outbox 和前瞻故障；
空页缺列；Artifact 独立性；真实行锁等待后的过期；取消、连接池恢复及响应容量。
精确 runner 为 [`scripts/test-task-content-index-http.sh`](../../scripts/test-task-content-index-http.sh)；
[`taskContentIndexHttp` 守卫](../../scripts/tests/taskContentIndexHttp.test.mjs)只检查源码接线、
严格投影及选择库存，不替代数据库行为验证。
源码库存不是执行数量；分日期实际结果见[项目状态](project-status.md)，维护选择见[测试指南](testing-guide.md)。

HTTP 模块沿用原有 Unix 准备层边界；这些检查不证明交叉平台编译、真实监听、浏览器/TLS/代理
行为或安装实例验收。不新增 SQL 路径、写命令、安装器、提供器、Session 签发、SDK/OpenAPI/UI
连接、迁移或恢复。

实现依据：[可信状态与路由](../../engine/src/platform_http/task_index/mod.rs)、
[查询与响应契约](../../engine/src/platform_http/task_index/contract.rs)、
[会话派发](../../engine/src/platform_http/task_index/handlers.rs)及
[独立测试入口](../../engine/tests/task_content_index_http_tdd.rs)。
