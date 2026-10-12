# 文件发布者授权的持久元数据

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布的内部准备层**，源码 **0.5.6**，**2026-10-11**。
这份工作树跟进不在已发布的 0.5.6 标签内；公共 API 基线仍为 0.3.0。
这一步延续[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，持久记录发布者授权
元数据及准确的发放／撤销历史。独立当前 Session 读取器只返回主体本人的记录。
存储的 Active 授权和成功的元数据读取都不授权发布、分享或访问字节；接收者授权与传输服务仍独立。

## 显式账本和历史

`FilePublisherGrantStore::new(pool, scope, installation_id, namespace, max_grants)` 只冻结可信配置，
不连接或安装存储；连接池必须已经选中该命名空间。`install_empty_namespace` 使用独立 Schema 1
标记、头表和变更表显式安装全新空命名空间，不迁移或收编既有／部分安装的数据。
每次操作保留并复核工作区、安装 UUID、原命名空间／表 OID、准确普通表形状及配置限额；
拒绝 RLS、临时／遮蔽关系、继承及改变的 search path。

`issue(grant_id, issuer, subject, validity)` 共同创建修订 1、Active 状态和首条变更。
`issuer`、`subject` 都是包含账号代次的准确 `FileAccountRef`，必须属于配置的 Authority。
它们是**可信调用方归因**，不是凭证或已验证的 Registry 管理权。签发者的教师、工作区所有者或
部署角色不会自动产生发布权限，也不会认证这个受信任接口。

`revoke(grant_id, actor, expected_revision)` 只接纳原 Active 修订 1，共同创建 Revoked 修订 2
及第二条变更。原签发者、主体和有效期保持不变，并记录所提供的准确操作者／账号。
不重激活、不改有效期或主体，不提供命令重放、隐式重试或清理；重复 ID、修订／状态失配均冲突。
历史使用规范 `grant_payload` 字节及受检的数据库整数 Unix 秒 `recorded_at`；时间不证明认证签发者、
单调时钟或可信历史权限。

已安装的 `max_grants` 为 1–256，覆盖全部保留头，包括过期／已撤销记录和旧账号代次。
每个授权至多两条变更。写入按固定表顺序串行，等待后使用 READ COMMITTED，校验有界完整账本，
刷新延迟约束，再于请求 COMMIT 前比较原 Schema 及完整预期头／历史；不关闭已配置的 READ ONLY。
只有确认提交后才返回记录；确认丢失可能已经提交，不授予重试权。

## 当前 Session 本人元数据

`DurableAuthSource::file_publisher_grant_workspace(namespace, scope, installation_id, max_grants)`
构造独立 `SessionFilePublisherGrantWorkspace`，不安装或改变现用认证。
`get_session_file_publisher_grant(token, workspace, grant_id)` 从当前、审计一致、活跃 Human
会员关系推导主体 Principal／账号，不接受客户端主体 ID；认证和授权命名空间必须分开。

读取器借用一个来源拥有的 READ COMMITTED 事务，保留原来源／Registry 和目标固定身份，
验证已选择头及完整历史，在确认 COMMIT 前复查当前策略与新鲜 Session 到期时间。
所选择读取按共同的标记／头／变更表顺序限制授权表写入，包含历史追加，持续到事务结束；
标记、已选头及历史行还保留 FOR SHARE 锁。这种粗粒度边界可能阻塞其他授权写入；
仍使用十秒命令、两秒锁和五秒语句限时。独立受信任 getter 保留另行 READ ONLY、REPEATABLE READ。

缺失或其他主体／账号的记录返回经授权的 `None`；所选择本人坏记录明确拒绝，不审计无关授权。
重建账号不能继承旧记录；无角色的活跃会员可读本人元数据，教师或部署管理者不能读他人的记录。
过期和 Revoked 记录仍可作为本人元数据查看。返回值只是观察，不是能力；不能把 Active 状态
缓存成后续操作的许可。

## 真正发布权限之前的边界

Registry 仍没有持久 `ManageFileGrants` 权限；后续独立 [Session 管理适配](large-file-grant-management.md)
使用明确可信宿主管理者配置，不借用部署管理权或教学角色。新证据不改下方原账本／读取的
执行记录。实际发布必须在受保护命令内另做当前审计发布权限决策；
后续 [Session 发布适配](large-file-session-publication.md)现补上该独立消费端，跨文件工作保留
原授权／认证源／Registry 固定，最终在屏障内认证源提交。它不升级可信历史归属，也不改原执行证据。
接收者管理和下载准入还必须绑定准确的受信任 Published 版本及发布者账号，分别处理撤权和
正在进行的字节工作。

后续独立[接收账本](large-file-recipient-grants.md)提供可信持久化及本人读取，不是上述管理或
下载消费端。发布撤权不级联到其保留授权，本发布账本及执行记录保持不变。

此账本不读取文件、上传存储、发布记录或接收者，不新增挂载 HTTP 路由、Session 签发、
OpenAPI／SDK／浏览器 API、启动安装器、迁移、文件恢复或网络传输。
既有[所有者私人文件元数据](large-file-session-metadata.md)及[纯授权契约](large-file-access-contract.md)
继续各自独立的职责。

## 验证

`file_publisher_grant_tdd` 覆盖受信任存储，`session_file_publisher_grant_tdd` 覆盖另行当前 Session
元数据读取；默认用例通过关闭的连接池验证配置和廉价准入。
`scripts/test-file-publisher-grants.sh`、`scripts/test-session-file-publisher-grants.sh` 核对库存并用
`--ignored --exact` 显式执行真实一次性 PostgreSQL 用例。公共 `filePublisherGrant` 检查源码组合、
库存与双语说明，不替代 SQL 授权、传输或部署验收。

## 执行记录 2026 10 11

账本编译 Red：在显式全新安装器处 **0 通过／1 预期失败**，有 **5 个暂态骨架／夹具警告**。
账本基线 Green 后独立编译 Session Red，本人元数据读取处 **0 通过／1 预期失败**，同样有
**5 个暂态警告**。账本／Session 首次基线 Green 分别残留 3／1 个警告；最终实现移除这些
未使用骨架导入／方法警告。

显式 PostgreSQL 16 账本 runner **12 条精确用例**通过，Session runner **7 条**通过。
每条执行前都核对已编译库存；读取／失败比较全部普通表行及 OID，成功变更保留准确原历史。
账本控制覆盖 CAS／签发者不变、跨代次保留额度、独立 RR 写者阻塞链、损坏／孤立／超量库存、
抑制写入、延迟精确增量篡改、合法克隆替换原关系、路径／标记漂移和临时遮蔽拒绝。
取消发生在 COMMIT 前的真实延迟 `SET CONSTRAINTS ALL IMMEDIATE` 工作，不是提交确认丢失
实验；同步探针只观察延迟刷新，不证明掉电持久性。

Session 控制覆盖本人到期／撤销／未来生效记录、无角色成员及教师隔离、旧账号代次、
成员／身份／会话生命周期、范围／安装／形状拒绝、关闭独立安装池、已配置 RR／READ ONLY、
标记／头／历史真实行等待后的数据库时间到期及取消，以及确被读事务表 SHARE 屏障阻塞的
独立追加后端。追加实验起点是历史行等待，不是最终认证查询。本目标不新增来源／Registry
克隆实验、完整外层超时或并发退出两种顺序测试；相邻私有文件回归保留其独立原关系覆盖。

首次组合运行暴露夹具假设：第二写者通过第一写者排队，错误工作区为 ScopeConflict，
持行锁后不能越过存储屏障安全升级表锁。修正控制追踪两个真实阻塞链、保留精确错误并提前
准入标记写锁；这些夹具修正不放宽生产准入。

相邻 `test-session-file-metadata.sh` 首次通过九条后，发现旧夹具按顺序等待两个已排队调用方：
未轮询的调用方若持有 Authority 锁，其同伴就超时。改用 `tokio::join!` 同时推进后，精确
成功／额度拒绝和全部快照检查不变；修复的精确用例独立通过，随后完整 **10 条 runner**
全通过，包括原认证源／Registry 关系替换。新增 19 条及最终相邻 runner 均无 Rust 警告。
全部 SQL 只使用新建的私有 Unix socket 一次性夹具，没有访问或修改既有应用数据库。

移除测试／环境数据库变量后，`quality-gate.sh --backend-only --no-npm-install` 通过
**838 项默认 Rust 测试**（包括 3 项 compile-fail 文档测试）、**253 项公共检查**及全部长度／
版本／课程／OpenAPI／API／SDK 生成和运维工具检查，**761 项显式用例 ignored**，零 Rust 警告。
新目标贡献 **6 项默认用例**。本轮没有重跑或声称前端运行时、SDK 运行时、新鲜 npm 审计、
全部显式库存、跨平台执行、真实局域网传输或部署验收。106 段既有项目状态证据、14 份原详细
执行记录尾部及分日期能力张量不变。版本仍 **0.5.6**，API 基线 **0.3.0**；没有提交、推送、
在线安装或迁移。私有合成数据库已停机并删除生成数据，执行日志保留。
