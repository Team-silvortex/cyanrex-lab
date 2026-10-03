# 显式任务内容 HTTP 适配

状态：**0.5.0 收录的 C2-N 准备层**。决策日期：**2026-10-03**。
此独立构造的路由将有界 HTTP 请求交给 [C2-M 会话内容命令](session-task-content.md)。
**现用应用没有挂载它**；它不签发登录凭据，也不连接浏览器编辑器。既有路由、认证、OpenAPI
及生成式 SDK 保持不变。

## 可信组合

Unix 上的 `platform_http::TaskContentHttpState::new` 显式接收 `DurableAuthSource`、
`SessionTaskContentWorkspace` 和一个规范来源；`build_task_content_router` 返回独立 Axum 路由。
构造过程不读环境、不安装 schema、不启动监听，也不修改 `AppState` 或正常的 `build_router`。

配置来源要求 HTTPS，或 localhost/字面回环地址上的 HTTP；必须是无用户信息、路径、查询、片段
或通配符的规范 Origin。这条准入规则不会配置 TLS、信任代理、验证证书或限制未来监听地址。
实际挂载前，宿主必须明确隔离的认证源/存储安装、安全入口、Cookie 签发及登录生命周期；
激活的认证源表不能与旧写入方共用。

## 请求准入

每个请求使用独立 `cyanrex_platform_session` Cookie，并携带单个
`x-cyanrex-platform-request: 1` 请求头。跨全部 Cookie 字段只能出现一次该名称，值必须是规范
非零 UUID；Cookie 头总量最多 8 KiB。旧 `cyanrex_session`、Bearer 或 URL 凭据不提供平台权限。
即使同时提供合法平台 Cookie，任何 Authorization 头仍会被拒绝。

每个变更请求必须有单个、精确匹配配置的 `Origin`。读取可不提供 Origin，但提供后也必须匹配。
变更来源缺失、null、重复或不匹配都会拒绝；不回退 Referer，不采用环境绕过，也不提供 CORS。
固定请求标记**不是秘密 CSRF token**。这些检查发生在读取正文之前，但不代替当前 Session 授权。

独立 Cookie 名防止浏览器自动复用旧入口凭据，**不会**给底层可重用持久 token 增加 audience
绑定，也不防止窃取后的重放。本切片不签发或清除 Cookie；未来签发方必须保护交付并使用适当的
Secure、HttpOnly、SameSite 属性，不把凭据放入 URL、本地导出或日志。

## 请求与响应契约

下述路径只存在于显式构造的路由。路径任务 ID 必须是规范 UUID；查询、GET 正文、内容编码及
不支持的方法被拒绝。变更只接受 JSON；**完整 HTTP 正文连同信封最多 64 KiB**，读取期限
为 **2 秒**，按实际输入流计量，不只相信 `Content-Length`，连续空帧也受期限约束。
媒体类型只接受精确的 `application/json` 或 `application/json; charset=utf-8`；GET 如提供
媒体类型也遵循此规则。重复媒体类型、内容编码及尾部字段被拒绝。已知路由的方法错误只声明
该路由允许的方法；HEAD 错误不带响应正文。

| 方法与路径 | 精确请求字段 | 成功结果 |
|---|---|---|
| `POST /platform/v1/tasks` | `schema_version: 1`、`task_id`、`manifest` | 201，返回已提交任务与清单 |
| `GET /platform/v1/tasks/{task_id}` | 无正文、无查询 | 200，返回已提交任务、清单及有序文本 |
| `PUT /platform/v1/tasks/{task_id}/content` | `schema_version: 1`、`expected_revision`、`manifest` | 200，返回已提交替换 |
| `POST /platform/v1/tasks/{task_id}/status` | `schema_version: 1`、`expected_revision`、`status` | 200，返回已提交状态变更 |

根记录必须是对象；未知/重复字段、位置数组、无效版本、非整数/不安全修订及枚举对象变体都会
拒绝。清单采用[服务端契约](task-content-manifest.md)，不是浏览器草稿 JSON。状态必须是现有
生命周期中的字符串，不表达验收或执行。请求不能选择所有者、空间、命名空间、文件根或认证源。

写入成功返回 `{schema_version: 1, task, manifest}`。读取增加按清单顺序排列的 `contents`，
每项为 `{artifact, text}`。文本来自 C2-M 校验的精确所属修订，不是另行绕过授权读文件。
允许零至 32 项、每项最多 256 KiB；JSON 转义及元数据会增加响应体积。这是单请求约束，
不是全进程内存或并发配额。不存在与不属于当前人的任务使用相同的未找到响应。

本适配不开放 Artifact 发布/换版、整份草稿导入、任务列表、目录准入、共享、Review、执行或文件
删除。客户端不能把正文放进清单来保存新文本；内容发布仍是独立、尚未开放的公共边界。

## 错误与结果不确定

错误使用版本化 JSON，包含 `error.code` 和 `error.outcome`，不泄露底层存储错误、所有者细节或
凭据。路由的所有响应，包括未知路径、拒绝方法、格式错误及存储不可用，都带私有 `no-store`
及 `nosniff`。

`not_attempted` 表示在调用 C2-M 前拒绝此 HTTP 请求；调用后产生的任何错误都保守地标记为
`unconfirmed`，包括未找到和冲突，不能据此承诺变更已回滚。已完成的不存在任务读取也在调用后
返回未找到，不因此授予重试权限。

正文解析后沿用底层十秒命令期限；HTTP 适配不额外为已发出的变更套一个竞争超时。没有
Retry-After、自动重试、幂等回执、结果对账或确认提交前的成功响应。断开、超时与响应丢失不证明
回滚；编辑失败也不会删除独立发布的内容。

## 验证及下一边界

默认进程内 HTTP 用例检查准入、JSON、传输上限、私有响应头及不可用认证源；静态守卫检查
正常应用、API 和 SDK 没有接线。
一次性 PostgreSQL 用例通过路由调用真实 C2-M 事务，核验成功及授权/存储失败。实际数量和
未覆盖范围见[项目进度](project-status.md)。

这些检查不代表真实监听、浏览器到服务端、代理/TLS 或已部署验收。下一步仍需安全 Session
签发和显式安装、内容发布与草稿映射，然后接入浏览器保存、读取和冲突处理。只有明确增加目标
部署接线时，现用 OpenAPI 与 SDK 才能对外声明这个接口。
