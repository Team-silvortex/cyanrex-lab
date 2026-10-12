# 当前发布者管理文件分享

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.6**，API **0.3.0**，**2026-10-12**。
本适配延续[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)、
[接收授权](large-file-recipient-grants.md)和 [Session 发布](large-file-session-publication.md)。
当前发布者可以为自己准确的文件签发／撤销接收授权；这是分享元数据，不是下载准入、
字节健康、传输或应用挂载。

## 明确配置与准确选择器

`DurableAuthSource::file_sharing_workspace(publication, publishers, recipients)` 从可信
`FileUploadPublicationStore`、`SessionFilePublisherGrantWorkspace` 和
`SessionFileRecipientGrantWorkspace` 固定 `SessionFileSharingWorkspace`。三者工作区／
authority 必须完全一致，命名空间受限且彼此不同。构造不做 IO、安装、迁移或角色转换；
认证源事务使用固定元数据，不独立调用各 store 连接池，也不由请求选择安装。

`SessionFileRecipientGrantIssue` 包含 `grant_id`、`publisher_grant_id`、`upload_id`、
`expected_file`、`recipient` 和 `validity`，这些有类型值只是选择器，不是权限或签发归属。
`expected_file` 必须与选中可信 Published 清单投影的每个版本字段一致：工作区／authority、
file ID、version ID、字节长度及 SHA-256。只比对摘要不够；请求或旧授权副本不能选择发布者账号。

签发者和发布者均由当前审计活跃 Human／成员及准确账号代际推导。新增分享时，教师／部署
身份、发布权限配置管理者或文件所有权都不能替代有效发布授权。这不是全局文件管理或委托
策略；旧可信接收 store 仍可独立记录不同的签发者／发布者归属，并由可信代码单独调用。
账本没有认证来源标记，签发者等于发布者本身不证明由 Session 签发。

## 一个授权事务内签发

`issue_session_file_recipient_grant(token, workspace, issue)` 使用认证源持有的一个
READ COMMITTED 事务，外层十秒上限，保留配置的 READ ONLY 策略：

1. 低成本 token、认证源及选择器范围检查先于 DB。当前审计身份确定操作者；本人当前 Active
   发布授权和新鲜数据库时间允许新分享，完整选中历史及原三表 SHARE 屏障持续保留。
2. 借用 Published 读取先按准确操作者／账号过滤再解码；保留原安装、关系、完整私有记录和
   指纹，以及 marker／行的 `FOR SHARE` 锁。借用记录视图不是发布确认。
3. 预期版本的所有字段须等于服务器投影版本；目标接收者必须当前是审计活跃 Human／成员，
   且认证源账号代际准确一致。
4. 接收账本直接取得 SHARE ROW EXCLUSIVE，记录当前操作者为签发者／发布者，校验有界
   全账本并保留原记录／历史／准确预期增量。
5. 恢复同步提交，刷新 `SET CONSTRAINTS ALL IMMEDIATE`，复核原写入及当前接收者，再
   复核原 Published 和发布授权。最终认证源／Registry／操作者检查后，最后一个新鲜时钟
   样本同时覆盖 Session 过期和原发布权限时间窗；之后没有 SQL，直接请求 COMMIT。
6. 只有认证源 COMMIT 确认才返回 `StoredFileRecipientGrant`。帮助器不独立开事务／提交、
   改事务策略，也不把元数据变成字节访问。

新接收授权有效期独立于发布权限，可以是未来生效或已过期元数据。发布到期／撤权不自动结束
已有接收授权。权限时钟采样在请求提交之前，不是在确认时，也不持续授权后续下载。
本命令不读文件、不检查当前分块／根目录健康，也不重建原发布者的字节回执。

## 停止已有分享

`revoke_session_file_recipient_grant(token, workspace, id, expected_revision)` 推导当前操作者，
在初始写屏障内、任何 UPDATE 前匹配记录原发布者／账号；不先拿读取 SHARE 再升级写屏障。
原签发者可以不同于发布者，保持不变。

只允许 revision 1 Active 到 revision 2 Revoked 的 CAS 转换。缺失／他人授权统一返回
PermissionDenied，不区分是否存在；本人授权的过时 CAS 返回 Recipient Conflict。
延迟工作后复核完整原记录／历史及预期全账本，再完成当前 Session 授权和 COMMIT 确认。

撤销不读取发布权限或 Published 存储，不要求其运行时 schema、有效许可、字节健康或接收者
仍存在／活跃，因此发布权限被撤销后仍可停止旧分享。发布者本人仍须是当前审计活跃 Human／
成员并具有原准确账号代际；重建账号不继承此权。替已删除／退休发布者操作仍需未来明确策略，
不能隐式由教师身份代办。

## 失败与交付边界

`SessionFileSharingError` 区分 Session、Publisher、Recipient、Publication、InvalidNamespace
和 PermissionDenied。不合格发布授权、缺失／他人 Published 或错误版本选择器拒绝签发；
当前接收者生命周期、canonical 历史、原 OID、安装、额度和完整允许增量保留独立类型错误。

两个命令保留额度占用、永久授权 ID 冲突及不可变绑定，不增加接纳、续期、重放成功、自动重试、
清理或状态恢复。确定提交前错误可回滚；取消、超时或丢失确认不证明回滚。这个纯元数据命令
没有 Ready／Unknown 发布 attempt 状态。

`session_file_sharing_tdd` 包含关闭连接池配置控制及显式真实 PostgreSQL 用例。
`scripts/test-session-file-sharing.sh` 逐个选择编译后的准确忽略库存；`fileSharing` 公共守卫
只检查组合／库存，不证明真实锁或权限。测试 setup 可用旧发布器读真实字节建立可信记录，
但分享本身不做文件／字节 IO。

不增加共享协作模型／表 schema、启动安装器、公开 Session 签发、HTTP、OpenAPI／SDK／UI、部署、
在线认证切换、下载命令、传输、配对或重启恢复。下载准入仍须独立校验当前接收者、准确可信
Published 版本、Active 授权和新鲜时间，再限制真实字节准入及进行中撤权窗口。分日期成熟度证据不变。

独立[下载清单观察](large-file-download-manifest.md)现将当前接收者本人 Active 授权连接到
准确 Published 元数据及最终新鲜有效期；不返回字节或可复用许可，不改本分享适配或下方原执行记录。

## 实际执行记录 · 2026-10-12

准确基线编译成功，先建立真实 17 字节 Session 发布，再因尚未实现的分享命令返回 Recipient
`UnsupportedSchema` 而失败：夹具清理后 **0 通过／1 失败**。这是行为 Red，不是缺失符号。
实现后基线 **1 通过／0 失败**；两次运行均无编译警告。前期库存守卫在最终测试／脚本接线前
运行而失败，不计作 SQL 行为 Red；接线后的分享／接收授权聚焦守卫 **10 通过／0 失败**。

首次完整分享矩阵的十个 SQL 用例全部通过。独立复查随后只加强测试精度，不改生产或库存：
成功 head／history 必须准确定位目标授权及 canonical payload；签发期望来自原命令和服务器
操作者，撤销保留命令前捕获的不可变字段及签发者。加固后的完整十项重跑再次通过，零失败、零警告。

最终显式 PostgreSQL 16 运行使用全新私有 Unix socket 实例，不监听 TCP，不使用环境中的
应用数据库。每个脚本运行前均核对编译后的准确 ignored 库存：

| 执行脚本 | 通过的 SQL 用例 |
| --- | ---: |
| `test-session-file-sharing.sh` | 10 |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |

共 **91 个不同的实际执行 SQL 用例**，零失败、零警告；重复基线／矩阵没有重复计数。
新分享默认测试另有 3 通过、10 忽略。完整后端质量门通过 **853 个默认 Rust 用例**，
**810 个忽略用例单列**，共 82 组结果，含三个 compile-fail 文档测试；**271 个公共守卫**
全部通过，编译器警告为零。ignored 库存不计作 SQL 已执行数量。

真实数据库 blocker 而不只是经过时间，证明排队额度／CAS 写入及末端过期等待；延迟故障
验证回滚且不自动重试。COMMIT 前取消及独立排空后的回滚观察，不是一般丢失确认模拟。
测试证明提交前新鲜授权和同步提交设置，不证明持续下载权限、断电耐久性或确认时到期检查。
源码复查未发现额外实质生产缺陷；文档复查明确区分了文件／字节 IO 与数据库 IO。

106 段历史状态记录、22 份既有详细执行尾部及分日期机器能力张量 SHA-256 保持不变。
没有残留私人测试命名空间或其他数据库客户端；全新 PostgreSQL 实例已停止，可重建的数据／
socket 已删除（约 165 MiB），验证日志保留。未改变应用数据库、产品／API 版本、提交或推送。
