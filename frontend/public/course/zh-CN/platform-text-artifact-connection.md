# 原绑定平台文本制品发布

源码 **0.6.5** 已收录下文 Task 绑定连接器、严格 HTTP 及默认关闭的工作区／CLI 开关。
其 `Unreleased` 标题／锚点保留实施阶段，不表示尚未发布；分日期证据保持原范围。
七个 UI 资源仍无正文保存客户端。

原非 Task 绑定能力已收录于源码版本 **0.6.4**，依据
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
等原构造器仍不启用发布，保持**七条路由、六个通道**。未加下述 Task 绑定标志时，
[明文](platform-loopback-host.md)和[原生 TLS](platform-loopback-tls-host.md) CLI 宿主显式选用此构造器，
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

## Unreleased：绑定 Task 的发布能力

`PlatformTaskTextArtifactConnector::new(storage)` 是另行显式选择的能力，配有下述独立 HTTP 适配，
以及显式启用的工作区／CLI 组合，不是浏览器操作。
`create_session_task_text_artifact(token, expected: TaskContentSnapshot, artifact, revision, text)`
接受完整预期手动 Task／清单快照以及新的 Artifact／修订标识；Artifact 标题从该预期 Task 推导，
文本仍遵循前述固定规范。给定 owner 仅与当前 Session 操作者作准确一致性比较，
不是选择账号、授权或替代认证；教师同样不能为其他所有者发布。

Token、结构化作用域／清单及文本规范检查先于唯一原连接。
同一 READ COMMITTED 事务中的一个 `PrivateWorkContext` 随后：

1. 解析当前经审计 Session 权威，锁定准确的当前本人 Task；作用域、所有者、预期修订、
   Draft 状态及完整 Task／清单基线须先于字节发布全部匹配。
2. 在 Task 行锁等待后调用 `recheck_selected_for`，复核原 Source、当前操作者及准确 Session 的
   新鲜到期时间，再回到同一个已固定 Task 目标，之后才派发文件工作。
3. 经保留 writer 创建新 Artifact、执行延期约束，核新准确 head／修订／outbox 与实际字节，
   然后重读并比较未改变的完整 Task。
4. 复核原根／存储，最后在获确认 COMMIT 之前完成新鲜 Source／操作者／Session 检查；
   不使用分离 Task GET 或可复用授权布尔值。

这是 **Task 元数据守卫**，不验证旧载荷字节，也不代表完成 Task 保存。
它不修改 Task 修订／清单／outbox、不自动挂接新引用、不原子发布整份草稿或恢复保存尝试。
挂接仍须另行授权换版；换版失败不能撤销这里已确认的发布。未知结果仍保留文件，
不自动重试、收编或清理，详见后文。
它不是幂等机制，也不限制每个 Task 修订只能发布一次：不同新标识可针对同一未变基线分别发布；
重复标识只产生冲突，不恢复先前回执。

相对不绑定 Task 的发布规范，可信初始化还需 Task SELECT 及仅供行锁的 `UPDATE(task_id)` 列权限。
不需要 Task 业务 UPDATE／INSERT／DELETE、Task outbox INSERT、Artifact head UPDATE、密码／TOTP SELECT，
不自动授权，也不覆盖明确 READ ONLY。三项 Artifact INSERT 与既有 marker 锁不变。
该能力沿用原保留根，独立持有四个真实 worker 名额，不安装 Schema 或选择新存储绑定。

既有严格五键文本 POST 及七个可选 UI 资源不变，默认 CLI 启动仍为八条路由／七个通道。
下述独立适配器和显式启用组合选择此能力，浏览器、SDK 与教学认证仍未接通；
不增加额外监听、Cookie 签发器、迁移或跨所有者权限。

## Unreleased：独立 Task 绑定 HTTP

`TaskTextArtifactPublishHttpState::new(connector)` 只接受绑定 Task 的原保留连接器。
`build_task_text_artifact_publish_router(state)` 只提供无查询、无尾斜线的准确
`POST /platform/v1/tasks/{task_id}/text-artifacts`；准确路径上的其他方法返回私人 **405**、
`Allow: POST`。独立构造器仍保留，只有下述显式构造器或标志才启用工作区／CLI 组合；
可选 Session 页、正常应用及 SDK 不调用它。

正文严格包含五字段：`schema_version: 1`、`expected`、`artifact_id`、`revision_id` 和 `text`。
`expected` 严格包含 `task` 和 `manifest`，表示完整既有手动 Task 快照及文本清单，不是期望写入的新 Task。
规范路径标识必须匹配 `expected.task.reference.task_id`；可信作用域、owner 权威以及清单／Task
结构一致性在派发前检查，真实所有权和当前状态仍只由连接器的当前 Session 事务确认。
Artifact 标题从该 Task 推导，不另接标题、所有者选择或路径。

全部记录层都只接受对象，包含 Task／reference／owner／workspace、输入引用条目及清单内部记录；
未知、语义重复、缺失字段及记录位置数组均在派发前拒绝。`definition` 必须存在且明确为 `null`；
status 只接受既有六种字符串，不接受单元枚举对象。Schema、规范 UUID、摘要和 JSON 安全整数
规则不变。时间戳按时间值固定，等价 RFC3339 时区偏移可表示同一时刻，不承诺规范 UTC 字符串拼写。
单个已知字段名使用 JSON 转义仍合法，反转义后重复的语义字段名则拒绝。
这些报文专用 DTO 不修改共享存储模型的反序列化行为。

请求必须携带准确可信 Origin、marker 和专用 Session Cookie，拒绝 Authorization。
低成本路由／头检查先于准入与正文轮询；实际报文收集至多 **2 MiB**、**两秒**，解码文本仍至多
**256 KiB**。正文／文本规范检查先于派发，HTTP 期限不包裹已准入连接器命令。
每个状态默认**四个响应名额**，可配置 **1–16**；克隆共享，已准入成功／错误都持有到正文完成或丢弃。
它与连接器真实 worker 名额独立；默认 CLI 组合不变，显式启用 Task 绑定发布才增加独立响应通道。

只有确认发布返回 **201**，复用只有 `schema_version` 与 `artifact` 的元数据响应，
不含文本、Task、Location 或 Set-Cookie。派发前失败为 `not_attempted`，连接器错误为 `unconfirmed`；
适配器单独将 Artifact 标识冲突投影为 **409 `conflict`**，Artifact 陈旧修订为 **409 `stale_revision`**，
不修改旧 Task 错误映射。不存在与他人 Task 仍同为私人 404。
它不挂接内容、不修改 Task、不验证旧载荷字节，也不提供重试、清理、整份草稿原子性、幂等、恢复或浏览器保存。

## Unreleased：显式启用 Task 绑定工作区与 CLI

`PlatformWorkspaceHttpState::with_task_text_publication_capacities(storage, login, lifecycle,
index, content, create, replace, publish, task_publish)` 在原八条路由上增加 Task 绑定 POST。
先验证**全部八项容量**（各为 **1–16**），再将同一原存储 `Arc` 拆为八项能力，
不重新打开或观察可替换基线。此组合为**九条路由／八个独立响应通道**，仍保留旧的
`POST /platform/v1/text-artifacts`；启用标志不意味着整个宿主只允许绑定 Task 的发布。

原构造器及签名默认值不变：`new`／`with_replace_capacities` 为七路由／六通道且不启用任何发布，
`with_text_publication_capacities` 为八路由／七通道且不启用 Task 绑定发布；状态克隆共享原通道预算。
独立 Task 绑定构造器仍拥有自身私人 fallback／响应头；显式工作区组合只合并无 fallback 路由，
沿用工作区既有私人响应策略。

在[明文启动](platform-loopback-host.md#显式启动)或[原生 TLS 启动](platform-loopback-tls-host.md#显式私有输入)
的必需参数之后，加默认关闭的 `--task-text-publication`。可选后缀顺序必须为：

```text
[--task-text-publication] [--notify-systemd] [--session-ui]
```

标志只用于 `serve-loopback` 和 `serve-loopback-tls`，诊断模式、重复标志、别名、后缀乱序和多余参数均拒绝。
不加时两种 CLI 都保持原八路由／七通道；加上后明确选择新构造器，各通道四名额。
它不自动启用 `--session-ui`，也不为七个既有资源添加正文编辑客户端。
进程／私有输入／原存储检查、字面回环绑定、原生 TLS 验证、Cookie 策略及信号排空保持原边界；
不增加环境变量／JSON 配置项、Schema 安装、授权、迁移或教学／SDK 回退。

两个发布连接器分别保留**四个真实阻塞 worker 名额**，启用后的两者可合计持有八个文件任务，
并非共享四名额。读取／换版的 worker 预算仍独立，八个四名额响应通道也不保证 32 请求并发；
全局网络准入仍为 **16 个连接**。可信初始化须保留组合角色的原登录、Task、Artifact／行锁权限，
本次启用不增加额外授权。它仍不验证旧载荷字节、不修改／挂接 Task、不重试、清理、恢复未知结果，
也不代表浏览器 TLS 或局域网验收。

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
bash scripts/test-platform-task-text-artifact-connection.sh
bash scripts/test-platform-task-text-artifact-http.sh
bash scripts/test-platform-task-text-publication-host.sh
```

运行器要求可丢弃且具管理权限的 `CYANREX_TEST_DATABASE_URL`，默认 Cargo 选择不执行忽略 SQL 用例。
测试须区分派发前校验和实际发布，核对准确元数据与文件，覆盖延期故障、当前权限、原绑定，
以及取消后真实 worker 额度。第二个运行器选择原绑定 HTTP 目标，而非旧连接池文本适配。
第三个清点六条真实 SQL／工作区／明文／TLS 选择；运行器库存不等于通过结果。

第四个运行器显式清点七条忽略的 Task 绑定 SQL 用例：本人发布且 Task／Source 不变，
他人／不存在／作用域拒绝，陈旧／篡改／状态／撤销拒绝，延期 Task 漂移与未知 blob 保留，
原根／marker 漂移，窄权限／READ ONLY，以及实际观察 Task 行等待跨过 Session 到期且零新文件。
默认构造器形状测试独立；库存和源码守卫都不能证明这些用例已通过。

第五个运行器选择六条准确且显式忽略的 Task 绑定 HTTP SQL 用例，以真实保留存储验证独立适配器。
它不挂载 CLI、不开始浏览器保存、不核旧载荷字节，也不代表部署验收；选择库存不等于通过结果。

第六个运行器选择六条准确且显式忽略的工作区／宿主用例，包含使用临时显式客户端 CA 的真实
明文／原生 TLS 子进程。库存不等于通过结果或浏览器 TLS／局域网验收；它保留原存储、当前所有者／
修订、响应预算、未知 blob、默认关闭兼容性及信号停止检查，发布仍不代表 Task 已保存。

浏览器冲突／未知结果处理、通用 SDK／OpenAPI、跨用户授权和恢复均属后续工作。
这里不增加 Artifact 读取路由、额外监听、Cookie 签发器、Task 状态转换或 Run。
