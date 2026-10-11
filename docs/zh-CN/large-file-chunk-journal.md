# 持久文件分块写前日志

已收录于源码版本 **0.5.6**。下方分日期实施和验证保留原 0.5.5／未发布阶段及范围；
源码收录不启用网络传输或公共 API。

状态：**Unreleased Unix 受信任准备层**，源码 **0.5.5**，日期 **2026-10-11**。
这是[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)的独立后续：创建文件前确认
Unknown 分块记录，本地字节核验后记录确认。不恢复尝试、不授权重试、不发布文件或接通传输。
[上传账本](large-file-upload-intent.md)和[本地暂存适配](large-file-upload-staging.md)保持原独立契约。

## 显式组合

`FileUploadChunkJournal::new(pool, workspace, installation_id, namespace, max_entries)` 冻结
受信任配置，不连接或安装。安装 ID 须是 RFC 4122 第 4 版 UUID；专用命名空间名遵循上传
存储的限制规则。全局条数额度为 **1–16,384** 个分块坐标，不是每次上传额度或磁盘预留。
显式 `install_empty_namespace` 只接受全新空命名空间，安装新日志自身的 Schema 1，
不向旧上传 Schema 1 加表，也不迁移任一存储。

`JournaledFileUploadStaging::new(local, journal)` 接收既有 `FileUploadStaging` 和日志。
两者工作区须匹配，配置的命名空间名须不同；共享本地适配的控制额度与既有文件工作额度，
不叠加第二份工作预算。只有原发行包装器及其克隆可执行自己的尝试，另行构造的包装器
不能执行其他包装器的尝试。构造不做 SQL 或文件系统工作。

`prepare(upload_id, owner, account_id, index)` 捕获原上传和日志固定信息及准确分块元数据。
不创建日志条目、不检查当前文件根健康，也不恢复此前尝试。
所有者及账号代次仍是受信任归属，不是凭据。

## 已提交状态与本地确认

| 边界 | 记录结果 | 内存结果 |
|---|---|---|
| Ready 尝试被控制额度拒绝 | 不写入 | 仍 Ready |
| 暂存获得准入 | 尚不代表有行 | 首次 await 前转为 Unknown |
| 原来源检查及 Unknown 事务确认 | 一个固定 Unknown 坐标 | 仍 Unknown；派发文件工作前再复核原来源 |
| 独占暂存及原文件检查 | 仍 Unknown | 尚不给成功回执 |
| 准确确认事务得到确认 | Confirmed 记录 | 返回回执前再复核来源及保留的原文件 |
| 最终观察通过 | Confirmed 记录 | Confirmed 及原文件本地回执 |

唯一坐标是日志固定工作区内的**上传 ID + 块序号**；来源/根绑定键与 nonce 是不变绑定字段，
状态只从 Unknown 转为 Confirmed；这些都不是额外坐标。任何已有坐标均冲突，包括相同元数据、Unknown 与 Confirmed。
在同一日志中，新包装器或变化的来源/根不能绕过冲突。错误或取消后不自动释放、重置、
降级、覆盖、收编、删除或重试。
冲突与条数额度属于该日志安装。新日志不导入旧状态或授予继续执行权限；来源/根不变时
文件系统目标仍冲突，变化配置也不是恢复协议。

Unknown 事务未获确认时绝不派发文件工作，即使后续元数据读取看到了该行。
取消释放等待方控制名额，但不会停止已派发的阻塞工作；实际排队/运行工作结束前不释放
工作名额，输出仍保留。创建前失败可能保留已提交
Unknown 而没有文件；最终日志提交后失败可能留下 Confirmed 记录，而调用方仍为 Unknown
并收到 `Unconfirmed`。存储与文件系统只做分别观察，不是跨存储原子事务；
同 UID/root 与数据库管理员仍受信任。

## 紧凑记录与原固定信息

每行包含受信任所有者/账号代次、上传/块坐标、完整来源和打开根的绑定键、整文件/块长度、
块摘要、私有第 4 版 nonce 和记录状态，不为每块重复存整份清单。
来源键绑定原预留完整 typed 清单及上传存储安装/命名空间/表代次，不是数据库或集群身份；
nonce 内部生成，不能通过元数据读取
或导入检查点取得。

命名空间/搜索路径/表 OID、配置的安装身份与持久额度须匹配原日志固定信息。
写入使用 READ COMMITTED，按固定顺序对两张日志表取 SHARE ROW EXCLUSIVE 锁，保留
既有只读策略。校验有界完整账本、精确分块长度关系及同上传绑定一致性，执行延迟约束后
比较完整预期账本，再请求同步提交。固定尺寸字段超长时，先以有界 SQL 投影拒绝再解码。

紧凑账本检查不独立重建其他上传的全部清单，也不证明每个 Confirmed 行的历史来源。
来源检查仍是独立短事务，不持 SQL 锁做文件 IO。原文件核验保留原只读描述符，
拒绝替换文件项，而非按路径收编匹配字节；元数据行不能重造该私有回执。

## 只读观察

`get(upload_id, owner, account_id, index)` 以准确受信任所有者/账号过滤，返回选中记录快照
或无可见记录。`UploadChunkRecord` 暴露块长度/摘要及
`PersistentChunkState::{Unknown, Confirmed}`，不暴露绑定键、nonce、路径、可运行尝试或回执。
读取验证选中元数据及 Schema，不验整个账本或当前来源/文件字节。不存在不是回滚证明，
记录 Confirmed 不是当前完整性或续传许可；所有块均记录 Confirmed 也不证明有序整文件
摘要或发布。

## 验证与剩余边界

`file_upload_chunk_journal_tdd` 包含默认无数据库准入及 opt-in PostgreSQL/文件用例。
`scripts/test-file-upload-chunk-journal.sh` 须提供明确可销毁数据库，逐项检查准确忽略测试名
再运行。源码守卫只检查库存/接线，不证明 SQL 行为；下方日期记录保留实际测试与验收边界。

本分块日志 API 不增加当前 Session 命令、HTTP 监听、SDK/OpenAPI、浏览器保存、启动安装、自动迁移、
网络端点、发布/接收授权、物理磁盘配额或清理 API；旧本地路径不要求此日志。
恢复仍需另行审阅来源/根代次与字节核对，观察状态本身不授予执行权限。

后续[整文件校验](large-file-whole-verification.md)不查询此日志，独立核对当前字节。
核验结果不会把 Unknown/Confirmed 状态转换为历史回执。
独立[发布准备层](large-file-publication.md)则同时要求完整原 Confirmed 目标行与新鲜整字节。
其仅首次创建元数据不提供恢复、当前接收授权或传输，也不改变本分块写前协议。

## 实际执行记录 · 2026-10-11

实际 TDD Red 是全新安装确认断言：目标成功编译，stub 返回 `StorageUnavailable`，
夹具清理后断言失败。不是缺失符号导致的编译失败，也不是整条暂存链的 Red。
该临时构建有 2 条库警告和 18 条夹具警告；实现安装器后，同一断言通过。

独立日志目标随后通过 **3 条无数据库用例**及 **11 条准确 opt-in PostgreSQL 用例**。
真实 deferred INSERT/advisory 锁等待时，Unknown 确认前目录无文件且 reader 未调用；
真正输入屏障处可独立观察已提交 Unknown。确认后合法改变来源，仍不派发文件。
运行中 worker 取消后保留自己的额度直到结束。deferred UPDATE/来源锁屏障证明：
晚到来源漂移或同字节替换 inode 可让持久行已 Confirmed，而调用者仍 Unknown。
其余覆盖准确文件保留、紧凑记录污染、Schema/查询固定、只读观察、已占坐标和
实际串行容量准入，并污染连接的 repeatable-read/synchronous-commit 默认值。

相邻上传绑定暂存 **9/9**、上传意图账本 **12/12** 实际 PostgreSQL 用例也通过。
隔离且不带数据库 URL 的 `scripts/quality-gate.sh --backend-only` 通过
**233 项公共检查**及 **71 个结果块中的 801 条 Rust 测试**，最终 **零警告**。
其中 **711 条 ignored** 仍是 opt-in 库存，不是额外执行通过数。这些运行不证明网络
丢确认、真实 fsync/磁盘满/断电处理、重启恢复、整文件最终核验、当前 Session/HTTP
接线、浏览器/桌面传输或特权验收。

SQL 运行使用本轮独占、只监听 Unix socket 的临时 PostgreSQL 16，创建全新私有来源及
日志命名空间。夹具命名空间/文件均已移除，无其他数据库客户端；服务停止后删除了约
64 MiB 数据目录。日志保留于 `/tmp/cyanrex-chunk-journal-oDq9MmuD/`：
`red-install.log`、`green-install.log`、`default-journal.log`、`postgres-journal.log`、
`postgres-upload-staging.log`、`postgres-upload-intent.log` 和 `backend-quality.log`。

两种语言课程副本同步；原有 **104** 段带日期项目状态记录及
`docs/platform-capability-tensor.json` 逐字节不变。源码仍 **0.5.5**、API **0.3.0**；
本块未 commit、push、打 tag 或发布。
