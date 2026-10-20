# 原绑定平台空任务创建

**源码 0.6.3 已收录**：依据[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，
独立低权限平台宿主现在可以创建所有者私有的空白手动 Task。最小后端链路为：创建空白 Draft →
[列出私人元数据](platform-task-index-connection.md) → [重开已存任务](platform-task-content-connection.md)。
代码是可选 payload，不要求编辑器、评估或 Run。独立[Draft 清单换版](platform-task-replace-connection.md)
现可修改标题或已有本人准确文本引用；Artifact 发布和浏览器保存仍未接通，原教学认证和本地草稿路径不变。

## 严格的空任务契约

只接受准确的 `POST /platform/v1/tasks`，不允许查询或末尾斜杠。正文必须是仅含三个字段的对象：

```json
{"schema_version":1,"task_id":"11111111-1111-4111-8111-111111111111","title":"小组讨论"}
```

Task 标识符须为有效非零类型化 UUID。标题沿用清单规则：非空白、至多 256 个 UTF-8 字节，
不含禁止的控制字符。未知／重复字段、数组、缺字段及错误类型均在派发前拒绝。调用者不能传入
owner、scope、命名空间、状态、修订、目录定义、manifest 或 payload。成功返回 **201** 及既有
`schema_version`、`task`、`manifest` 投影：手动 Draft、修订 1、无定义或输入引用、`payload: []`。
响应不含 `contents`、Location 或 Set-Cookie。成功表示数据库已确认 COMMIT，不表示客户端已收到。

所有者只来自当前经审计的活跃 Human 成员关系及准确 Session 的账号代次。教师角色不获得别人
的私人任务。标识符对应有效的本人手工任务，或属于他人时，均返回相同冲突；不支持或损坏的
本人记录则失败关闭并报告不可用。不接管、不覆盖已占用任务；并发创建至多一个确认成功者。
这不是幂等或结果恢复接口。

## 同一原绑定与同一事务

`PlatformTaskCreateConnector::new(storage)` 无 I/O 地消费真实 `PlatformStorageConnector`。
Origin、scope、命名空间、catalog／五个 marker 身份及根描述符均取自原绑定，不接受请求、环境
或连接池回退。每次命令打开一条全新原连接，使用 READ COMMITTED 事务；不覆盖显式 READ ONLY
角色策略。

写入前复核原存储、当前 Source／Registry 授权及 Task 状态。即使零 payload，仍锁定配置的
Artifact 命名空间；不创建 Artifact 业务行或文件。仅元数据校验会证明四张 Artifact 表的关系锁，
核对原结构和 scope，不要求 Artifact 业务列 SELECT。原内容读取仍保留原有业务读权限要求，
创建权限不会隐式授予重开权限。

共用的无池 Task 策略写入 head 和恰好一条配对 outbox。执行 `SET CONSTRAINTS ALL IMMEDIATE`
后，再次验证原 Artifact 目标和表结构，再准确读回预期 Task head／event／manifest 配对。
这包括拒绝 marker 未变、但延期触发器启用 Artifact RLS 的副作用。随后恢复原 Source，复核
原根和保留存储；全部等待结束后，紧接 COMMIT 前重复当前 owner／账号／成员关系及准确 Session
的最新过期检查。确认提交前不对外返回待定结果。

连接验证、业务工作和关闭连接各保留独立十秒预算；HTTP 不对已派发写操作设置整体超时。
同步根检查仍可能阻塞当前线程，不构成进程硬期限或文件系统／数据库原子事务。确认提交不因
关闭失败而降级。取消、超时、断连或服务错误都是 **unconfirmed**，不是回滚证明，也不授权
重试、接管或清理。后续冲突也不能判定先前未确认请求的结果。测试覆盖取消和交付丢弃，
没有模拟 COMMIT 确认丢失。

## HTTP 授权与独立容量

`TaskCreateHttpState::new(connector)` 或 `with_capacity(connector, capacity)` 只选择空创建能力；
`build_task_create_router` 只挂载集合路由。请求必须包含准确选定的 Origin、
`x-cyanrex-platform-request: 1` 和专用 `cyanrex_platform_session` Cookie；拒绝 Authorization。
这些头部只是入口检查，实际写权限由事务判定。JSON 类型和编码规则沿用私人 API，实际流式正文
限制为 64 KiB、两秒，均在派发前结束。集合其他方法返回 405／`Allow: POST`；独立只读构建器
仍然没有集合路由。

响应保留 `no-store`、`no-cache`、`nosniff`，无 CORS、重定向、token 披露或 Cookie 签发。
派发前错误为 `not_attempted`；派发后错误均为 `unconfirmed`，包括通用私人冲突。不暴露存储路径
或驱动诊断。

明文和原生 TLS 回环宿主现有七条具体路由：三条 Session、私人索引 GET、任务项 GET、空创建
POST 及独立清单换版 PUT。`PlatformWorkspaceHttpState` 将同一个原存储 Arc 拆为六个能力、六条独立快速失败准入
通道。各容量 1–16、默认四；克隆共享名额，持续到响应正文最后一帧／丢弃。这不是全局内存、
连接、worker、持久存储配额或分布式滥用防护。

`with_create_capacities(storage, login, lifecycle, index, content, create)` 及旧 `with_capacities`、
`with_content_capacities` 签名保留，换版默认四；新增 `with_replace_capacities(storage, login,
lifecycle, index, content, create, replace)` 在分配／拆分前验证全部六容量。仅 Session、独立只读及
仅创建路由不变，不合并原完整连接池写路由。SDK/OpenAPI、前端传输、LAN／DNS 监听和教学
AppState 仍是独立边界。

## 初始化、验证与剩余工作

可信初始化须提供原 Source 2、identity／access 2、Task 3、Artifact 1 结构，授权／head 读取和
列 UPDATE 行锁权限，以及 Task head／outbox INSERT。本创建路径不要求密码／TOTP SELECT 或
Artifact 业务 SELECT／写入；重开另需原有明确读取配置。不新增安装器、迁移、自动授权、角色
提权、配额策略或自动重试。

```bash
env CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_create
bash scripts/test-platform-task-create-connection.sh
```

运行器要求显式、可丢弃、具管理权限的 `CYANREX_TEST_DATABASE_URL`。十条 HTTP 单测和八条准确
真实 SQL／HTTP／宿主用例覆盖严格输入、所有权、并发重复、原绑定／根漂移、即时及延期副作用、
可观测等待后过期与取消、受限角色权限、响应准入和真实低权限回环进程。不实现浏览器保存／恢复、
文本／二进制发布、目录定义、状态转换、Run 派发或局域网部署；标题／已有引用换版属于独立
保留绑定能力，不是本仅创建路由的方法。
