# 当前 Session 私有文件元数据

已收录于源码版本 **0.5.6**。下方分日期实施和验证保留原 0.5.5／未发布阶段及范围；
源码收录不启用网络传输或公共 API。

状态：**Unreleased Unix 准备层**，源码 **0.5.5**，**2026-10-11**。
本轮延续[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，授权创建本人上传预留，
以及精确读取本人的上传或 Published 元数据。它不授权真正发布、访问文件字节或向接收者分发。

## 显式配置与命令

`DurableAuthSource::file_upload_workspace(namespace, scope, installation_id, limits)` 返回
`SessionFileUploadWorkspace`；`file_publication_workspace(namespace, scope, installation_id,
max_publications)` 返回独立的 `SessionFilePublicationWorkspace`。两者固定可信配置，
校验时没有 SQL 或文件系统 IO，也不安装任何表。目标必须已经显式安装对应 schema，且工作区、
安装代次和容量配置完全匹配。认证源及身份注册表所在命名空间必须与选中的元数据命名空间分离。

| 命令 | 返回元数据 | 不提供的权限 |
|---|---|---|
| `reserve_session_file_upload(token, workspace, upload_id, manifest)` | 一条不可变逻辑上传预留 | 不接受调用方 owner/account，不确认字节、物理磁盘分配或发布权限 |
| `get_session_file_upload(token, workspace, upload_id)` | 本人精确预留或 `None` | 不证明全账本健康、已验证分块进度或恢复的尝试 |
| `get_session_file_publication(token, workspace, upload_id)` | 本人精确 `FileUploadPublicationRecord` 或 `None` | 不调用发布器，不提供历史回执、当前字节健康、恢复或下载授权 |

服务端从当前 Session 及经过审计的身份绑定推导 Principal 和账号代次。要求工作区、Human
Principal 及成员资格都活跃，不要求教学角色或部署授权。没有角色引用的活跃成员也能管理本人的
私有上传意图。Agent／服务身份、暂停或退休绑定、重建账号不能继承旧账号代次的记录。
其他所有者的记录表现为不存在，不向调用方泄露。

## 认证源拥有的单一事务

所有 SQL 都在认证源连接池的一笔借用事务内执行，包括元数据访问。存储句柄不取得另一连接池的
连接，也不提交认证源事务。命令在守卫首次快照前设置 READ COMMITTED，不受认证连接池默认
REPEATABLE READ 影响。保留配置的 READ ONLY 并安全拒绝：即使只读业务元数据，也需要带锁的
认证和注册表检查，不能把这些命令声明为 PostgreSQL 只读事务。

守卫固定最初的认证源命名空间及认证源／注册表关系身份，解析当前审计过的所有者，再选择独立
固定的目标。上传预留保留固定写栅栏，检查完整有界账本，跨所有账号代次执行字节及数量配额，
并先检查永久 upload／version 冲突，再检查配额。只插入一条记录，强制执行延迟约束，并复核
准确的预期记录、完整账本及原目标 schema。随后重新选择认证源，复核原认证源／注册表关系和
当前策略，在等待之后用新鲜数据库时间检查 Session 到期。只有 COMMIT 确认后元数据才返回。
延迟约束执行前及提交前都会重新打开同步提交。

精确读取保留原 schema 及完整行指纹，不审计无关行。schema 配置行及存在的选中记录使用 FOR SHARE，持锁直到
最终权限检查及 COMMIT，防止权限等待期间并发更新改变该观察。`None` 是经过授权的选中缺失，
不是禁止后续插入的锁。选中本人损坏记录安全拒绝；私有读取不会为了回答而解码隐藏的外人行。
旧可信存储 getter 保持独立的 READ ONLY、REPEATABLE READ 快照及可信调用方提供所有者的语义。

事务沿用十秒总期限、两秒锁超时及五秒语句超时。取消或提交确认丢失不能证明回滚；不可用结果
之后上传可能已经存在。元数据读取不恢复原命令回执，也不授权接管、自动重试、覆盖或释放配额。
数据库管理员仍属于可信边界；关系检查不能标识整个数据库集群。

## 权限与传输门槛

私有工作守卫不是文件发布者或接收者策略。教师权威仍用于实例部署管理，不能据此发布另一账号
的文件或读取其私有元数据。[可信发布流水线](large-file-publication.md) 保持独立，不被这些命令
调用。这里不打开暂存文件、分块日志、尝试或回执。

仍需经过审查的发布权限、精确版本接收者授权、物理磁盘准入，以及回执丢失／重启协调契约。
无特权集中传输服务及后续经过认证的桌面配对尚未接通。这一适配器不增加 HTTP 路由、启动安装、
在线 AuthService 切换、OpenAPI／SDK／浏览器连接或迁移。

## 测试

默认 `session_file_metadata_tdd` 使用不可用的惰性连接池，验证配置及低成本 token／scope 准入，
不需要数据库。`scripts/test-session-file-metadata.sh` 明确枚举每个 ignored 用例，再通过
`--ignored --exact` 在可丢弃 PostgreSQL 中运行。SQL 用例覆盖 owner／账号代次推导、无需文件 IO
的 Published 正向读取、失效 Session／策略、账号重建、缺失或损坏记录、配额、配置默认值、
触发器故障、原关系替换、真实等待及取消。`sessionFileMetadata` 通用守卫只检查源码组合与清单，
不能替代 SQL 行为验证。

## 执行记录

**2026-10-11**，源码 **0.5.5 Unreleased** 本轮在独立初始化、仅使用 Unix socket 的 PostgreSQL 16
夹具中通过三个默认目标测试及十个精确 SQL 用例。默认目标忽略这十个 SQL 用例。最初预留测试
在空适配器上失败，实现后通过。真实 INSERT 触发器的合法表克隆另行复现了丢失原认证源关系
身份的问题；保留最初的固定身份后，回滚测试通过。Published 正向读取也捕获并修正了配置表拼名。

同一夹具另通过 **72 个相邻 SQL 用例**：上传意图 12、可信发布 13、Session Task 17、Session
Artifact 18、可信／Session 内容索引 12。这些证明选定的存储及授权回归，不是部署、网络、前端、
SDK、磁盘写满、断电或大文件吞吐验收。未注入一般 COMMIT 确认丢失，也没有专门在取得选中读锁
之后安排最终认证等待的故障。

独立后端质量闸门通过 **813 个默认 Rust 测试**及 **245 个通用脚本检查**，无失败或编译警告。
其中 **742 个 ignored 用例**不计为通过。源码 0.5.5、公共 API 基线 0.3.0 及历史成熟度证据未变。
