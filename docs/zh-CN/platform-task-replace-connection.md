# 原绑定平台草稿清单换版

**源码 0.6.3 已收录**：依据[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，
独立宿主可替换当前所有者手动 Draft 的整份 manifest。[创建空任务](platform-task-create-connection.md)
后，可以修改标题、标签、顺序或对本人既有文本修订的引用，再[重开](platform-task-content-connection.md)。
代码只是可选 payload，空清单仍合法。本命令不发布新的 Artifact 字节，不接通浏览器保存、Run、
目录定义或原教学认证。

## 整份清单与乐观修订

只接受准确的 `PUT /platform/v1/tasks/{task_id}/content`，Task UUID 须规范且非零，不允许查询或
末尾斜杠。复用既有严格换版 DTO：

```json
{"schema_version":1,"expected_revision":1,"manifest":{"schema_version":1,"title":"讨论纪要","payload":[]}}
```

正文须为仅含这些字段的对象。重复／未知字段、数组、错误类型、缺字段和非法嵌套清单均在派发前
拒绝。文本项包含准确 Artifact 修订／摘要，以及展示文件名、语言；文件名或语言不授予文件系统
访问或执行能力。请求不能选择 owner、命名空间、根、工作区、目录定义或状态。Artifact 坐标须
匹配配置的工作区，并在事务中另行授权，不因成功解析而受信任。

换版要求准确的当前修订、Draft 状态以及实际改变的清单。陈旧修订返回冲突，非 Draft 拒绝转换，
相同清单拒绝无效改动。不存在和属于他人的 Task 返回相同私人缺失；不支持的目录任务或损坏的
本人记录失败关闭。教师角色不自动获得他人的私人写权限。

成功返回 **200** 及 `schema_version`、更新后的 `task`、`manifest`，不含 `contents`、Location 或
Set-Cookie。修订递增一次，更新标题／输入引用／时间，保留任务标识、owner、创建时间及 Draft
状态，并追加恰好一条配对 outbox。同一预期修订的并发请求不能都成功。响应确认 COMMIT，不证明
客户端收到，也不提供幂等／重试权。[私人索引](platform-task-index-connection.md)和内容 GET
在各自当前授权下读取已存版本。

## 原存储与完整内容验证

`PlatformTaskReplaceConnector::new(storage)` 无 I/O 地消费原 `PlatformStorageConnector`。
它仅提供换版、Origin 和 scope 访问，无连接池或调用者指定的 owner／路径。Token 和新清单 scope
检查先于连接。每次命令打开一条全新原连接，以 READ COMMITTED 运行，不覆盖显式 READ ONLY。
内容 reader 只复制保留的原根描述符，不重新打开可接受基线。

命令复核原 catalog／五个 marker，解析准确 Session 的当前账号代次及经审计的活跃 Human 成员
关系；先锁定并严格读取本人的 Task。通过修订／状态／无改动检查后，按 Artifact、修订标识统一
排序完整旧／新 payload 并集。保留重复坐标，避免有效旧摘要掩盖冲突的新 pin。每项须匹配本人
head、准确修订及 outbox，私有普通文件须通过原根、inode、长度、SHA-256 和文本规范校验。
删除最后一个 payload 也不能绕过该项旧内容的验证。

Task 换版复用原手动／目录存储的无池策略和不变 SQL。新保留绑定命令独立负责在 head／outbox
写入后执行 `SET CONSTRAINTS ALL IMMEDIATE`，然后再验 Artifact 命名空间／表结构及**全部旧新
字节**，包括已移除项，再准确读回预期 Task／event／manifest 配对。随后恢复原 Source，复核
reader／原根及原存储；所有等待后，紧接 COMMIT 前重复当前账号／成员关系及准确 Session 的
最新过期检查。不提前返回待定快照。旧连接池 API 保留原权限、worker 和延期工作策略。

每份清单最多 32 项、每项至多 256 KiB，并集最多 64 项。换版逐项校验并丢弃字节缓冲，不聚合
内容响应。每个保留绑定换版连接器有四个快速失败 blocking 工作名额；实际任务持有至完成，
取消等待者不会提前释放。其预算与只读连接器独立。连接验证、业务及关闭各十秒；同步根检查仍
可能阻塞线程。这不是同 UID／root 隔离、进程硬期限或文件系统／数据库原子事务。

确认 COMMIT 不因关闭失败降级。取消、超时、断连和派发后服务错误仍为 **unconfirmed**，不是
回滚证明，也不授权自动重试、接管、删文件或补偿。后续陈旧修订不能判定先前未知结果。测试覆盖
取消和交付丢弃，不模拟 COMMIT 确认丢失。

## HTTP 与六条独立通道

`TaskContentReplaceHttpState::new(connector)` 或 `with_capacity(connector, capacity)` 仅选择换版
能力，`build_task_content_replace_router` 仅挂载内容 PUT 路径。沿用准确 Origin、
`x-cyanrex-platform-request: 1` 和专用 Session Cookie，拒绝 Authorization；在派发**前**以
两秒收集至多 64 KiB 正文。HTTP 不对已派发写操作加整体超时。该路径其他方法返回私人 405／
`Allow: PUT`。派发前错误为 `not_attempted`；派发后均为 `unconfirmed`，包括陈旧修订和非法输入。
响应保留 no-store／no-cache／nosniff，无 CORS、重定向或 token 披露。

显式明文／原生 TLS 回环宿主现有七条具体路由、六条独立响应生命周期通道：登录、生命周期、
索引、内容读、空创建和清单换版。`with_replace_capacities(storage, login, lifecycle, index,
content, create, replace)` 在分配／拆分前验证全部六容量，将同一原存储 Arc 拆为六个能力。
各容量 1–16、默认四；克隆共享名额直至正文最后一帧／丢弃。旧三／四／五容量签名保留，新增
容量默认四。仅 Session、只读和仅空创建 builder 不变，不合并原完整连接池写路由。
SDK/OpenAPI、前端传输、LAN／DNS 监听、Task 状态写入和文件分发仍是独立边界。

## 初始化与验证

可信初始化提供原 Source 2、identity／access 2、Task 3、Artifact 1，以及既有授权／Artifact
读取行锁权限。换版另需 Task `UPDATE(revision,snapshot,manifest)` 和 Task outbox INSERT，
不需 Task INSERT 或 Artifact 写入。空到空换版也使用完整 Artifact 读取配置，不使用空创建的
仅元数据权限配置。不增加密码／TOTP SELECT、安装器、迁移、自动授权或提权。引用发布仍是另一个
受信任操作，不是 PUT 的隐式回退。

```bash
env CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_content_replace
bash scripts/test-platform-task-replace-connection.sh
```

运行器要求显式、可丢弃、具管理权限的 `CYANREX_TEST_DATABASE_URL`。准确 SQL／HTTP／宿主用例
覆盖空编辑、旧新准确修订和顺序、当前所有权、损坏、延期故障、原绑定、乐观并发、真实等待后
过期／取消、受限权限及实际回环保存／列举／重开。文件 oracle 比较名称、设备／inode、权限、
链接和字节，不声称还原外部造成的损坏。不新增文本／二进制发布、浏览器冲突／恢复 UI、目录工作、
状态转换、执行或局域网验收。
