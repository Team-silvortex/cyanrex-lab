# 原绑定平台文本制品发布

本能力已收录于源码版本 **0.6.4**，依据
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，经原保留存储绑定显式创建
一个私人文本 Artifact。笔记和配置同样是合法内容，不要求代码、Task 或 Run。

连接器本身不增加传输；独立 `TextArtifactPublishHttpState` 和
`build_text_artifact_publish_router` 将其适配为独立 HTTP；显式宿主组合见下文。
两者都不改变既有 [Task 清单换版](platform-task-replace-connection.md)，不接通浏览器正文／SDK 保存，
也不替换教学认证。原连接池 Artifact API 保持各自策略。

## 显式能力

`PlatformTextArtifactConnector::new(storage)` 无 I/O 地消费原 `PlatformStorageConnector`。
`origin()` 和 `scope()` 只返回配置元数据，不是可复用权限。唯一资源命令为：

```rust
create_session_text_artifact(
    &self,
    token: &str,
    artifact: ArtifactId,
    revision: ArtifactRevisionId,
    title: String,
    text: String,
) -> Result<ArtifactRevision, SessionArtifactError>
```

调用者提供新的 Artifact 与修订标识，不提供 owner、路径、摘要、parent、sequence、kind 或 media。
所有者由事务中的当前 Session 权威推导；教师角色不授予其他所有者的私人内容权限。

## 固定文本规范

发布固定使用 kind `cyanrex.task-text`、media type `text/plain`。文本须为 UTF-8，
按实际字节计至多 **256 KiB**，包含多字节字符。允许空文本；除制表、换行和回车外，拒绝控制字符。
标题不得为空白，至多 256 个 UTF-8 字节，且不得含控制字符。

Token、文本与标题检查先于数据库连接。该规范比通用 Artifact 存储更窄，
不将任意二进制内容改称文本，也不选择编辑器或执行策略。

## 独立 HTTP

`TextArtifactPublishHttpState::new(connector)` 只接受原绑定连接器，以其配置 Origin 为可信来源。
`build_text_artifact_publish_router(state)` 仅提供无查询的准确
`POST /platform/v1/text-artifacts`。复用既有严格五键正文：`schema_version: 1`、
`artifact_id`、`revision_id`、`title`、`text`；拒绝未知／重复字段及位置数组。
确认创建返回 **201**，只有 `schema_version` 与 `artifact`，不回显文本或签发 Cookie。

请求必须含准确 Origin、`x-cyanrex-platform-request: 1` 及专用 `cyanrex_platform_session`
Cookie。路由与凭据拒绝先于正文轮询；实际流式报文字节上限为 **2 MiB**，
**两秒正文期限**只覆盖收包，解码后的文本仍至多 **256 KiB**。
标题／文本校验先于派发；正文期限不包裹连接器命令。

每个状态默认**四个响应名额**，`with_capacity` 可配置 **1–16**；克隆共享同一独立通道。
已准入的成功或错误响应持有名额直到正文最后一帧或丢弃，而非仅到处理器返回；
它与真实文件任务名额独立。派发前失败报告 `not_attempted`，命令错误报告 `unconfirmed`，
不证明回滚或允许重试。适配器不增加命令超时、重试、读取路由、UI／SDK 合并、
监听或教学登录变更；原连接池文本路由仍独立。

## 显式宿主组合

`PlatformWorkspaceHttpState::with_text_publication_capacities` 显式启用**八条路由、七个独立响应通道**，
先验证全部七容量，再将同一保留存储 `Arc` 拆为七项能力。`new`、`with_replace_capacities`
等原构造器仍不启用发布，保持**七条路由、六个通道**。[明文](platform-loopback-host.md)和[原生 TLS](platform-loopback-tls-host.md) CLI 宿主显式选用新构造器，
各通道四名额；全局仍最多 **16 个连接**，七个四名额通道不保证 28 个并发请求。
原绑定、四个真实文本文件任务名额及私人错误结果不变，不合并旧连接池文本读取、文件／状态路由或教学 AppState。

## 同一原事务

每次调用打开一条全新原连接，以 `BEGIN ISOLATION LEVEL READ COMMITTED` 运行。
不覆盖数据库角色明确的 `READ ONLY` 只读策略，也不回退连接池。

1. 复核原 catalog、全部五个存储 marker 和保留 Artifact 根。
2. 在配置工作区解析当前 Session 账号代次及经审计的活跃 Human 成员关系，
   保留原 Source 与目标命名空间身份。
3. 创建 sequence 为 1、无 parent 的 Artifact head、不可变修订及配对 outbox。
   私有文件独占创建、同步并读回。
4. 在检查延期副作用前执行 `SET CONSTRAINTS ALL IMMEDIATE`。
5. 重新核对命名空间结构、**完整准确的当前 head**、修订／outbox 配对及实际字节。
   若延期触发器安装了另一个有效 head，旧准确修订仍可读也不能确认本次创建。
6. 恢复原 Source，完成 reader／根及原存储复核；所有守卫等待之后，
   再查当前 actor 和准确 Session 的新鲜到期时间。
7. 只有确认 COMMIT 后才返回待定修订。

发布 Artifact 不创建 Task，也不替换清单；将结果中的准确修订关联到 Draft，
仍是另一个独立授权操作。

## 原文件与真实任务额度

Writer 继承保留的原目录描述符，检查可见的 no-follow 根，不收编重新打开的可接受目录。
文件名由服务端从类型化标识与摘要派生，以描述符相对方式独占创建，不覆盖或收编既有文件。
私有普通文件的所有者、单链接、`0400` 权限、长度、SHA-256，以及原根／inode 均须核对。

每个连接器持有**四个快速失败的真实阻塞任务名额**，与其他连接器预算独立。
已排队或运行的真实文件任务持有名额直到完成，覆盖同步与读回；取消等待 future 不提前释放。
这不是四个并发事务的限制。原连接池存储保留原 worker 策略。

连接验证、业务和关闭各有十秒预算；同步根检查仍可能阻塞线程。
这些预算及文件／SQL 检查不是进程硬期限、文件系统与数据库原子事务，也不隔离同 Unix 身份或 root。

## 初始化与未知结果

可信初始化提供已声明的原 Source、identity／access、Task 和 Artifact 命名空间；
连接器不安装或迁移任何 Schema。既有窄 Session／注册表／存储 marker 权限保持不变；
Artifact 元数据需要 SELECT 及其 schema marker 的行锁列权限。发布另需对
`collaboration_artifacts`、`collaboration_artifact_revisions`、
`collaboration_artifact_outbox` 的 `INSERT`。不需要密码／TOTP SELECT、Task 写入、
整表 UPDATE 或自动授权。新插入 head 已由原事务持有，不要求 Artifact head UPDATE 权限。
组合宿主角色保留既有登录凭据 SELECT 和 Task／Artifact 行锁列权限，仅额外增加三项 Artifact INSERT；
初始化不能撤掉旧通道所需权限。

SQL 尚未确认时，blob 可能已经存在。取消、超时、断连或提交错误留下未知结果，不证明回滚。
应保留已写文件，不自动删除、重试发布或收编同名 blob；后续冲突也不是先前尝试的回执。
已确认 COMMIT 不因关闭连接失败而降级。

## 验证与剩余接线

只使用明确选择的可丢弃存储：

```bash
bash scripts/test-platform-text-artifact-connection.sh
bash scripts/test-platform-retained-text-http.sh
bash scripts/test-platform-text-publication-host.sh
```

运行器要求可丢弃且具管理权限的 `CYANREX_TEST_DATABASE_URL`，默认 Cargo 选择不执行忽略 SQL 用例。
测试须区分派发前校验和实际发布，核对准确元数据与文件，覆盖延期故障、当前权限、原绑定，
以及取消后真实 worker 额度。第二个运行器选择原绑定 HTTP 目标，而非旧连接池文本适配。
第三个清点六条真实 SQL／工作区／明文／TLS 选择；运行器库存不等于通过结果。

浏览器冲突／未知结果处理、通用 SDK／OpenAPI、跨用户授权和恢复均属后续工作。
这里不增加 Artifact 读取路由、额外监听、Cookie 签发器、Task 状态转换或 Run。
