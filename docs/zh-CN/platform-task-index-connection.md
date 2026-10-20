# 保留原存储绑定的任务元数据索引

**源码 0.6.3 已收录**的独立平台宿主，按[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)
新增当前所有者的私人 Task 列表。任务允许没有任何 payload；代码、编辑器、领域定义和 Run
都不是前提。此切片只接元数据发现；独立[内容重开](platform-task-content-connection.md)现另加任务项 GET，
两者都不等于浏览器保存、跨用户教学审阅或任务验收。
独立[空任务创建](platform-task-create-connection.md)现只允许当前所有者创建无 payload 的手动
Draft，不将索引变为写入口。
独立[Draft 清单换版](platform-task-replace-connection.md)按预期修订修改标题或已有本人准确文本
引用，不扩展此读取能力。

## 原存储与当前权限

`PlatformTaskIndexConnector::new(storage)` 消费真实 `PlatformStorageConnector`，Origin、Task
命名空间和工作区只能来自原配置。不接收客户端自报所有者、替代 Source、连接池或新基线，
也不导出存储句柄。构造不做 I/O。初始宿主只接纳手动 Task；所选目录 Task 缺少冻结配置的
完整定义时整页拒绝，不静默跳过，也不降级为手动任务。

每条命令在连接前拒绝错误凭据和分页参数，然后建立一条验证原存储的新连接。READ COMMITTED
事务先核对原目录和五种标记，再核对准确 Session 账号世代及经审计的活跃 Human 成员身份，
只读取该所有者的 Task 头和最新事件。共用严格读取器验证所选快照、清单及一条预读后构造
有界页面。Source 与 Task 命名空间保留首次准确身份，不用后续观察替换原 pin。

准备阶段先恢复原 Source，再重复宿主原存储检查。所有这些等待之后，最终重新核对原 pin、
当前成员和准确 Session 的新鲜到期状态；只有确认 COMMIT 才返回元数据。登录结果或先前
会话查询不能授权这次读取，教师角色也不能读取他人的私人任务元数据。

连接验证、业务事务和最终关闭各保留独立十秒预算。同步 Artifact 根元数据检查没有硬期限，
也不在整个业务事务中锁住根目录。已确认提交不会因关闭失败而撤销；取消或超时不证明远端
回滚，也不允许自动重试。

此路径不读取 Artifact 记录或字节，但保留绑定的宿主仍要求完整原存储可验证，包括 Artifact
标记及根目录。旧的显式连接池索引保留较窄的纯元数据依赖，不是原绑定失败时的回退入口。

## 显式 HTTP 组合

`TaskContentIndexHttpState::with_platform_storage(storage, capacity)` 只选择保留绑定的后端，
Origin 从原存储派生。原 Source 构造器及独立路由继续保留原合同。

`PlatformWorkspaceHttpState::new(storage)` 与 `build_platform_workspace_router` 在登录、
Session 生命周期、索引、内容读取、空任务创建及清单换版之间共享同一原存储。`with_capacities(storage,
login, lifecycle, index)` 保留签名，内容与创建默认四；`with_content_capacities` 保留签名，创建默认四。
`with_create_capacities` 保留五容量签名，三者均保持换版默认四；新增 `with_replace_capacities(storage, login, lifecycle, index,
content, create, replace)` 在拆分前验证全部六容量，各为 1–16、默认四。克隆共享各自持续至响应正文
结束／丢弃的名额，六个通道彼此独立，不是全宿主连接或工作线程配额。

显式 Linux `serve-loopback` 与 `serve-loopback-tls` 使用下列七条路由：

| 准确路径 | 方法 | 操作 |
|---|---|---|
| `/platform/v1/session/login` | POST | 已有账号登录及专用 Cookie |
| `/platform/v1/session` | GET | 新鲜当前会话状态 |
| `/platform/v1/session/logout` | POST | 确认准确 token 退出 |
| `/platform/v1/task-index` | GET | 当前所有者的私人任务元数据 |
| `/platform/v1/tasks/{task_id}` | GET | 当前所有者的准确空／文本任务内容 |
| `/platform/v1/tasks` | POST | 当前所有者的手动空 Draft，仅版本／任务标识／标题 |
| `/platform/v1/tasks/{task_id}/content` | PUT | 当前所有者的 Draft 标题／已有准确文本引用，须预期修订 |

原 `build_platform_session_router` 仍只有前三条。Task 写入仅有空任务 POST 及清单换版 PUT；Artifact 发布、
状态写入、独立 Artifact、文件分发、教学和执行路由仍未挂载。宿主原明文／TLS 入口、通知和排空规则不变；正常教学 AppState、前端
认证、SDK/OpenAPI 及局域网／DNS 监听仍未接通。

索引请求保留[原严格 HTTP 合同](task-content-index-http.md)：专用 Cookie 与请求 marker、
GET 可不带 Origin 但携带时必须准确、空正文、只接受规范 `after` 与 `limit` 查询。默认
20 条，允许 1–32 条外加一条严格验证的预读；每条只含引用、修订、标题、状态、定义引用、
更新时间和 payload 数量。游标只是位置，不是权限。私有 no-store／no-cache／nosniff 响应
不含所有者、清单、Artifact pin、凭据或可复用授权，索引响应不设置或清除 Cookie。

## 初始化与验证

宿主不安装、不迁移、不授权，也不覆盖显式 READ ONLY 角色策略。可信初始化须提供已有
Source 2、身份／访问 2、Task 3 Schema 及业务权限。关系 pin 不需要读取密码或 TOTP；
仍须账号／Session 指定列读取和行锁权限、经审计 Registry 的读取／锁定及 Task 头／事件读取／锁定。
HTTP 只读不等于 PostgreSQL 只读事务：授权与头一致性仍使用行锁。缺失权限私有失败，不回退特权池。

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::workspace_router
bash scripts/test-platform-task-index-connection.sh
```

准确 SQL runner 要求显式可丢弃的 `CYANREX_TEST_DATABASE_URL` 管理权限，覆盖所有者隔离、
空 payload 任务、撤销成员、原存储漂移、真实锁等待与最终过期、严格预读、缺少权限、HTTP
取消和响应正文准入。真实回环子进程用例证明宿主接线，不证明已部署课堂或浏览器工作流。
