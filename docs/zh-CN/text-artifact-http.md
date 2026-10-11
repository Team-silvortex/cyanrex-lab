# 显式文本 Artifact HTTP 适配

状态：**C2-X 内部准备层，已收录于源码版本 0.5.5**。决策日期：**2026-10-08**。
源码发布：**0.5.5**（**2026-10-11**），路由仍未挂载。

分日期的未发布跟进及验证记录保留原源码版本与执行范围；收录于 0.5.5 不代表重跑或扩大这些证据。

此独立构造的路由通过既有当前 Session Artifact 命令发布一个私人文本制品，并读取一个精确
修订。它补充独立 [C2-N 任务内容路由](task-content-http.md)不提供的文本发布传输。两个路由
均未挂载到现用应用，不连接浏览器草稿、不签发 Session，也不改变在线认证、OpenAPI、生成式
SDK 或发布日志。

## 可信组合与准入

Unix 上的 `TextArtifactHttpState::new(source, workspace, origin)` 显式接收
`DurableAuthSource`、`SessionArtifactWorkspace` 与规范允许来源；
`build_text_artifact_router(state)` 返回独立 Axum 路由。工作区提供范围、命名空间与文件根，
请求不能选择这些值、所有者或认证源。构造不读环境、不安装 schema、不监听，也不改 `AppState`。

Origin、专用 `cyanrex_platform_session` Cookie、规范 token 和
`x-cyanrex-platform-request: 1` 复用 [C2-N 准入规则](task-content-http.md#请求准入)。
不回退 Bearer/旧 Cookie，不提供 CORS 或 Session 签发。这些检查不配置 TLS、不交付安全
Cookie，也不代替每条来源命令的当前授权。

默认容量为 **四**；可信 `with_capacity` 只接受 **1–16**，否则返回 `InvalidCapacity`。
同一文本 Artifact 状态的克隆共享名额，分别用这些克隆构造的路由也共享。新建状态与任务内容
状态有**独立、可叠加的预算**。低成本端点/请求头检查先于立即准入；容量满时返回私有 503
`capacity_exhausted` / `not_attempted`，不轮询正文、不调用 SQL，也不带 `Retry-After`。

名额覆盖正文收集、原来源命令、JSON 序列化与成功/错误响应正文，至最终帧/结束、错误或丢弃
才释放，处理函数返回并不释放名额。取消仅释放准入，不证明回滚，也不终止已派发的阻塞 I/O。
这不是跨路由/全进程内存、工作线程、网络或代理缓冲配额。

## 发布一个文本制品

`POST /platform/v1/text-artifacts` 只接受一个 JSON 对象，字段精确如下：

| 字段 | 要求 |
|---|---|
| `schema_version` | 整数 `1` |
| `artifact_id`、`revision_id` | 既有规范、非零标识符格式 |
| `title` | 非空白、无控制字符，最多 256 UTF-8 字节；不补造标题 |
| `text` | 解码后最多 256 KiB UTF-8 字节；允许空文本；除 TAB/LF/CR 外不接受控制字符 |

未知/重复字段、位置数组及无效字段形态均被拒绝。适配器固定 kind 为 `cyanrex.task-text`、
媒体类型为 `text/plain`，请求不能选择 kind、所有者、范围、路径或自报长度/摘要。
请求 ID 是显式服务端契约标识符，不代表给本地编辑器 ID 授权。

完整 JSON 传输正文另有 **2 MiB** 上限与**两秒**读取期限；实际流受限，不依赖正确的
`Content-Length`，连续空帧也受约束。JSON 转义计入传输上限，解码文本仍限 256 KiB。
C2-N 任务请求保持 **64 KiB** 上限。查询、编码正文、尾部字段及不支持的媒体/方法沿用
共享传输规则拒绝。

发布成功返回 **201** 与 `{schema_version: 1, artifact: ArtifactRevision}`。即使 ID/文本
相同，重复发布也返回 **409**，不收编已有修订，也不变成幂等回执。发布与后续任务命令各自
提交；任务命令失败不授权删除 Artifact。

## 读取一个精确修订

`GET /platform/v1/text-artifacts/{artifact_id}/revisions/{revision_id}/{sha256}` 不接受
查询或正文；原始路径 ID 与摘要必须规范。当前授权及精确引用/摘要检查通过后返回 **200**，
正文为 `{schema_version: 1, artifact: ArtifactRevision, text}`。存在新 head 仍可读取历史
修订，合法的非首序号/父修订元数据允许保留，不静默改读 head。

返回修订必须符合固定文本 kind/媒体类型，以及有界 UTF-8 文本规则。其它 kind/媒体、二进制/
控制字符或超限文本返回私有 **503** 与 `unconfirmed`，不会成为成功文本投影。通用读取器
可先取得最多 **1 MiB**，之后才做更严格的文本检查；256 KiB 不是峰值内存或工作线程上限。
缺失与他人修订均为 **404**，错误摘要或存储损坏仍为 **503**，不降级为缺失或授予重新发布权限。

## 结果与任务边界

响应沿用私有 `no-store` / `nosniff` JSON 信封。本地准入/解析失败为 `not_attempted`；
一旦调用来源创建/读取命令，所有错误均保守标记为 `unconfirmed`。原命令期限不变，不新增
竞争 HTTP 命令超时。错误、取消或响应丢失不证明回滚、历史确认，也不授予重试权限。

可信调用方可从成功响应取得精确 Artifact 引用，构造 C2-K 清单，再**另行**请求 C2-N。
代表性准备链只是调用方独立的进程内夹具 HTTP 序列，不是文本路由直接调用任务路由、
自动合并路由、原子批次或已挂载公共流程。每次请求重新授权；部署路由/fallback 组合、
Session 签发与浏览器保存/冲突处理仍须显式设计。

不提供换版/删除 API、收编、自动重试、尝试恢复、日志派发、安装器、在线认证切换或新部署权限。

## 验证范围

2026-10-08，本片 **12 条默认契约/传输用例、10 条精确一次性 PostgreSQL 用例与三项新增公共
守卫通过**。这是独立证据，不继承 C2-N 执行；首条合法 70 KiB POST 在拒绝桩下真实失败，
实现后通过。

独立目标为 `text_artifact_http_tdd`，精确一次性数据库 runner 为
`scripts/test-platform-text-artifact-http.sh`。契约/传输单测和源码守卫不等于 SQL 执行；
实际分日期结果见[项目状态](project-status.md)。边界包括文本/传输上限、历史修订、私有错误、
当前权限、响应正文持有及发布引用到任务的显式组合。进程内 HTTP 与合成存储不证明真实监听、
浏览器、代理/TLS、部署或重启/恢复验收。
