# 保留原存储绑定的任务内容读取

**源码 0.6.3 已收录**的独立平台宿主，按[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，
通过准确 `GET /platform/v1/tasks/{task_id}` 重开当前所有者的已保存 Task。它在
[私人元数据索引](platform-task-index-connection.md)之后返回原任务快照、清单和经验证的文本
payload。空 payload 合法，不要求代码、编辑器、评估或 Run。浏览器重开／保存、Artifact 发布
及通用二进制 payload 仍是独立边界；已有引用的[Draft 清单换版](platform-task-replace-connection.md)
现另有保留绑定的写能力。代码只是可选 payload，本轮不接通浏览器保存。

独立[空任务创建](platform-task-create-connection.md)现可只凭任务标识和标题创建当前所有者的
手动 Draft；这不是载荷发布，也不增加此读取器的写方法。

## 原存储与准确内容

`PlatformTaskContentConnector::new(storage)` 消费真实 `PlatformStorageConnector`，构造不做
I/O。Origin、作用域、Task／Artifact 命名空间和原根描述符只能来自该绑定。请求只提供任务
标识和已有会话凭据，不能选择所有者、命名空间、根路径或可信 Artifact 引用。此连接器没有
写方法或连接池回退。初始宿主只读手动任务；目录任务需要另行冻结的完整定义，不静默转换。
旧的手动／目录连接池 API 保留各自显式配置。

每次读取建立一条验证原存储的新连接及 READ COMMITTED 事务，核对原目录和五种标记，
解析准确 Session 账号世代和经审计的活跃 Human 成员身份，并先锁定属于当前所有者的 Task，
再读取 Artifact。教师角色不能越权读取他人的私人内容；缺失和属于他人的 Task 返回相同的
经授权缺席结果。

共用读取器验证 Task 头与最新事件一致、清单完整；即使任务为空，也选择并固定 Artifact
命名空间。payload 按统一 Artifact／修订顺序锁定，按清单原顺序返回。每份准确修订须与
所有者、head、修订记录和 outbox 一致，私有普通文件长度及 SHA-256 必须匹配。固定旧修订
仍读取旧内容，不用较新 head 替代。UTF-8、文本 profile 及 payload 绑定全部验证后才允许返回。

字节工作之后重新核对 Task 元数据并恢复原 Source，宿主再检查原根与原存储。全部等待完成后，
最终重新核对原命名空间／关系 pin、当前成员及准确会话的新鲜到期状态，再 COMMIT。确认
提交之前，不返回部分内容、待确认元数据或可复用授权。取消或失败不代表回滚，也不允许重试、
收编或清理。

## 原始描述符与有界文件工作

Artifact 读取器复制原存储观察保留的描述符，并与可见的 no-follow 根核对，不把新打开的
目录当成可接受新基线。文件读取保留本人所有的 0700 根、单链接 0400 普通文件、准确 inode／
长度／摘要检查，拒绝替换、丢失、符号链接或不私密内容。这不隔离同 UID／root，也不是文件
系统和数据库的原子事务。

每个保留绑定的内容连接器有四个 fail-fast 阻塞工作名额，由实际 blocking closure 持有直到
工作完成；HTTP 等待者取消或超时不会提前释放，也不会回退无限排队。原显式连接池 Artifact
API 保留旧 worker 策略。连接验证、业务事务和关闭各有独立十秒预算；同步根元数据检查仍可能
阻塞当前线程，没有硬期限。确认 COMMIT 不会因关闭连接失败而撤销。

## 只读 HTTP 与准入

`TaskContentReadHttpState::new(connector)` 或 `with_capacity(connector, capacity)` 只选择保留
绑定的读取能力，Origin 从连接器派生，构造不打开存储。独立 `build_task_content_read_router`
只拥有准确任务项路径，不合并原完整 Task 路由；集合创建、`/content` 替换和 `/status` 修改
在此独立只读路由中仍不开放；宿主另行组合的创建／换版不扩展该读取器。规范任务项仅允许 GET，其他方法私有返回 405、`Allow: GET`。原始编码别名、
非规范标识和任何查询在凭据或正文工作之前拒绝。

沿用专用 Cookie、请求 marker、可省略但携带时必须准确的 GET Origin、空正文、编码及两秒
正文规则。缺失／他人 Task 返回私有已分派 404。响应保留 no-store／no-cache／nosniff，不提供
CORS、Cookie 签发、跳转或 token。成功 JSON 沿用 `schema_version`、本人的 `task`、`manifest`
及按序 `contents: [{artifact, text}]`；空 payload 为 `contents: []`。文本是惰性数据，不是
HTML、执行请求、文件系统路径或编辑器要求。

当前文本 profile 最多 32 项，每项不超过 256 KiB；JSON 转义可使响应大于原始文本总量。
入站元数据限制和小型索引投影不是内容响应上限；列表存在也不证明稍后一定能重开字节。

原工作区构造器保持七条路由：三条 Session、元数据索引、此任务项 GET、独立空任务 POST 及清单换版 PUT。
`PlatformWorkspaceHttpState::with_capacities(storage, login, lifecycle, index)` 保留原签名，
内容／创建／换版默认四；`with_content_capacities(storage, login, lifecycle, index, content)` 也保留
签名，创建／换版默认四；`with_create_capacities(storage, login, lifecycle, index, content, create)`
保持签名，换版默认四。新增 `with_replace_capacities(storage, login, lifecycle, index, content, create,
replace)`，在分配或拆分原绑定之前验证全部六个容量，仍不启用发布。两个 CLI 宿主显式选择
`with_text_publication_capacities` 增加[私人文本 POST](platform-text-artifact-connection.md)，为八路由／七通道；
全部七容量全验后才拆分同一原存储 Arc。本读取连接器不增加写方法或权限。CLI 各通道四名额，
全局仍最多 16 连接，不保证 28 请求并发；各可信容量范围 1–16。克隆共享持续至正文最后一帧／
drop 的各自名额，通道彼此独立，不是全局连接、内存、worker 或分布式滥用配额。原 Session-only
组合仍准确只有三条。正常教学 AppState、前端认证、SDK/OpenAPI、局域网／DNS 监听、通用 Run
和文件分发挂载保持不变。

## 初始化与验证

不增加安装器、迁移、自动授权或 READ WRITE 覆盖。可信初始化须提供原 Source 2、身份／访问
2、Task 3、Artifact 1 Schema、读取权限及授权／head 行锁所需的指定列 UPDATE 权限。此读取
不需要密码／TOTP SELECT、Artifact 或 Task 写入，但不等于 PostgreSQL 只读事务。显式
READ ONLY 角色策略或缺少权限会失败，不改用特权连接池。

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_content_read
cargo test --manifest-path engine/Cargo.toml --locked --lib services::artifact_store::reader
bash scripts/test-platform-task-content-connection.sh
```

准确 runner 要求显式可丢弃的 `CYANREX_TEST_DATABASE_URL` 管理权限。八条真实 SQL／HTTP／
宿主用例覆盖原绑定／描述符、按序准确修订与空 payload、账号／所有者隔离、损坏、窄权限、
可观测 Task／Artifact 等待后过期、取消、响应名额及实际无能力回环进程。worker 单测覆盖等待者
取消后的名额存活期；这不代表浏览器集成或课堂／局域网部署验收。
