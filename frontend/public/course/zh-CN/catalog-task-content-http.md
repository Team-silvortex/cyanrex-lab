# 显式目录任务内容 HTTP 适配

状态：**C2-Y 内部准备层，已收录于源码版本 0.5.5**。决策日期：**2026-10-11**。
源码发布：**0.5.5**（**2026-10-11**）。本独立路由将有界 HTTP 请求接到既有
[会话目录内容命令](session-task-content.md)，从可信配置选择准确定义，不接收客户端定义正文。
它未挂载到现用应用，不签发 Session、不连接浏览器编辑器，不改现用 OpenAPI/SDK、不迁移记录，
也不执行评估规则。

分日期的未发布跟进及验证记录保留原源码版本与执行范围；收录于 0.5.5 不代表重跑或扩大这些证据。

## 可信组合

Unix 上的 `platform_http::CatalogTaskContentHttpState::new(source, workspace, origin)` 显式接收
`DurableAuthSource`、强类型 `SessionCatalogTaskContentWorkspace` 及规范允许来源。
`build_catalog_task_content_router(state)` 返回独立 Axum 路由；构造不读取环境配置、不安装存储、
不启动监听，也不修改 `AppState` 或正常 `build_router`。

工作区保留冻结定义目录，包括显式组合的多个包，以及两项评估绑定齐全或明确均不绑定的定义。
请求只选择已准入的准确引用，不选择目录、提供器、命名空间或模式。无绑定定义仍创建带
`Some(TaskDefinition)` 的目录 Task，不变成手工工作或自动通过；完整定义相等和原文本/Session
检查继续生效。

`new` 默认四个在途请求；`with_capacity(source, workspace, origin, max_in_flight)` 接受
**1 至 16** 的整数，否则返回 `InvalidCapacity`。状态克隆及分别用其构造的路由共享预算。
独立构造的目录、手动或文本状态各有预算，容量相加，不组成全局配额。

配置来源要求规范 HTTPS，或 localhost/字面回环地址上的 HTTP。这不配置 TLS、信任代理或
保证未来监听安全；宿主仍需分别安排隔离的认证源/存储安装、受保护入口及有效持久 Session 的交付。

## 请求与响应契约

`/platform/v1/catalog-tasks` 前缀只存在于此显式构造路由。路径任务 ID 必须是规范 UUID。
四种方法和后缀与手动适配器一致，但创建在原清单之外必须指定准确 `definition_ref`：

| 方法与路径 | 精确请求字段 | 成功结果 |
|---|---|---|
| `POST /platform/v1/catalog-tasks` | `schema_version: 1`、`task_id`、`definition_ref`、`manifest` | 201，返回已提交目录任务与清单 |
| `GET /platform/v1/catalog-tasks/{task_id}` | 无正文、无查询 | 200，返回已提交任务、清单及有序文本 |
| `PUT /platform/v1/catalog-tasks/{task_id}/content` | `schema_version: 1`、`expected_revision`、`manifest` | 200，返回已提交替换 |
| `POST /platform/v1/catalog-tasks/{task_id}/status` | `schema_version: 1`、`expected_revision`、`status` | 200，返回已提交状态变更 |

`definition_ref` 必须是只有 `package`、`name`、`version` 的对象；其中 `package` 必须是只有
`name`、`version` 的对象，例如：

```json
{
  "package": {"name": "example.editorial", "version": 3},
  "name": "example.editorial.prepare",
  "version": 1
}
```

该示例只有在宿主配置了对应准确条目时才准入。名称及正的 JSON 安全整数版本沿用
[定义契约](task-domain-boundary.md)，不回退到 latest、不发现目录或认证发布者。根记录、引用和
包都必须是对象；未知/重复字段、位置数组、非法名称或版本直接拒绝。漏掉 `definition_ref`
不默认为手工工作。客户端不能发送完整定义、所有者、命名空间、评估策略，或切换手工/目录模式。

创建复制完整配置定义；替换与状态请求不能更换该定义。已有命令保留评估绑定选择、准确身份
和清单约束，核验完整持久元数据、旧新所属文本、预期任务修订及当前 Session。模式错误直接拒绝，
不升级或重新解释记录。不派发提供器、类型化 Evidence、评估、Review、执行或 Task 验收。

写入返回 `{schema_version: 1, task, manifest}`；读取另有按清单顺序排列的 `contents`，每项为
`{artifact, text}`。这些是已确认会话结果的共用投影，不是另行绕过授权读文件。清单允许零至
32 个精确文本引用，每项最多 256 KiB。提供清单不会发布新字节；
[C2-X 文本发布](text-artifact-http.md)仍是独立命令。

## 共用 HTTP 准入

目录适配复用[手动适配器的安全和传输规则](task-content-http.md)：单个
`cyanrex_platform_session` Cookie、单个 `x-cyanrex-platform-request: 1` 标记、变更精确匹配
配置 Origin，不接纳 Authorization 头或旧 Cookie 权限，不提供 CORS，也不回退到 Referer。
固定标记不是秘密 CSRF token，底层可复用持久 token 不增加 audience 绑定；本适配不签发或清除 Cookie。

完整 JSON 请求最多 **64 KiB**，正文读取期限 **2 秒**。查询、GET 正文、重复媒体类型、
不支持的编码、尾部字段及方法沿用共用拒绝规则。端点/请求头检查先于满额即拒的准入和正文读取；
满额返回 `503 capacity_exhausted`、`not_attempted`，不轮询正文、不调用 Session 命令，
不设置准入等待队列或 `Retry-After`。

已准入名额跨正文收集、原会话命令、序列化及响应正文存活，直到最终帧/结束、错误或丢弃；
错误响应同样如此，不在处理函数返回时提前释放。取消释放名额，不证明回滚，也不终止已派发的
阻塞 I/O。原十秒会话命令期限不变，不增加竞争变更超时、响应交付期限或全进程内存配额。

## 错误与结果不确定

所有响应复用私有 `no-store`/`nosniff` 策略。错误只返回带版本的 `error.code`/`error.outcome`
信封，不暴露目录内容、所有权或存储诊断。

| 边界 | HTTP 结果 | outcome |
|---|---|---|
| Session 派发前的请求/准确引用 JSON 格式错误 | 400 `invalid_request` | `not_attempted` |
| 可信配置不存在对应准确定义 | 400 `invalid_request` | `unconfirmed` |
| 完整定义漂移、持久模式不符或引用字节非法 | 503 `service_unavailable` | `unconfirmed` |
| 派发前容量满 | 503 `capacity_exhausted` | `not_attempted` |

未知定义在调用会话命令后映射，即使内部查找可能在数据库操作前拒绝，也不构成 HTTP 回滚证据。
所有已派发的服务错误均保守标为 `unconfirmed`；不存在与他人所有任务使用相同私有未找到响应。
错误不授予重试、清理、收编或补造确认的权限。

## 手动兼容与验证边界

`TaskContentHttpState` 与 `/platform/v1/tasks` 仍仅支持手动任务；原请求 JSON 继续拒绝定义
字段，原强类型工作区也不接纳目录记录。新路由有独立 fallback，不自动与手动或文本路由合并。
可信宿主必须明确组合监听；客户端不能切换模式或发现配置目录。

专用进程内 HTTP 目标和精确临时 PostgreSQL runner 检查本适配；源码守卫检查强类型组合及
没有应用/API/SDK/浏览器接线。实际执行数量和限制单列于[项目进度](project-status.md)，
不与源码库存混算。这些检查不证明真实监听、代理/TLS 部署或浏览器保存链路。

C2-P–W 草稿发布、观察和日志流程保持独立且仅支持手动任务。C2-Y 不调用它们、不强制使用其
日志，也不提供发布/任务创建原子性。Session 签发、公共部署组合、浏览器映射、冲突/结果不确定
处理、迁移、共享、Review 权限及执行仍是独立边界。
