# 持久接收授权

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布元数据准备**，源码 **0.5.6**，API **0.3.0**，**2026-10-11**。
本独立账本延续[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)及
[准确授权契约](large-file-access-contract.md)，记录可信宿主提供的接收授权，并支持当前
Session 本人接收授权元数据读取。它不授权下载、不认证管理者、不连接传输，也未挂载到应用。

## 准确文件与独立账号

`FileRecipientGrantStore` 固定明确 PostgreSQL 连接池、工作区、RFC 4122 version-4 安装 ID、
受限命名空间和 `1..=MAX_FILE_RECIPIENT_GRANTS`（256）额度。构造不连接数据库。
`install_empty_namespace()` 只在全新命名空间显式安装 schema 1，不接纳旧关系、不迁移数据、
不启用在线认证。
原始搜索路径只接受准确裸名或规范引号目标，即便不存在的额外条目也拒绝；每个事务仍固定
第一次原始写法。

`issue(id, issuer, file, publisher, recipient, validity)` 分别保存三个独立提供的
`FileAccountRef`，均含 Principal 和准确账号代际，authority 必须与配置工作区一致。
签发者不必是文件发布者，自己接收自己的文件也有效。可信归属不证明审计身份、当前发布权限
或文件已发布。

不可变 `FileVersionRef` 包含工作区／authority、file ID、version ID、字节长度与 SHA-256。
它可以来自合法的声明清单，不读取字节，也不检查 Published 记录。后续下载准入仍须单独比对
准确可信发布版本及发布者账号，只比对摘要不够。

授权 ID 在本账本内永久唯一。同一文件、发布者和接收者可以有多份不同 ID 的授权，不隐式去重，
也不加版本／接收者唯一限制。[发布授权账本](large-file-publisher-grants.md)可以独立使用同一 ID。
撤销发布权限不会撤销接收授权，必须单独撤销每份接收授权。教师、部署和所有权角色都不自动
生成这两种授权。

## 持久记录与撤销

显式 `0022_collaboration_file_recipient_grants.sql` 包含三个永久普通表：

- `collaboration_file_recipient_grant_schema` 固定 schema、范围、安装、额度及命名空间／
  marker／head／history OID。
- `collaboration_file_recipient_grants` 保存 ID、接收者／发布者／签发者代际、revision、有效期
  及上限 4096 字节的 canonical `grant_payload BYTEA`。完整文件引用只在 payload 内，不重复文件列。
- `collaboration_file_recipient_grant_changes` 保存有序不可变 payload、操作者代际及数据库时钟
  产生的有界 `recorded_at` Unix 秒。revision 1 的操作者必须等于签发者。

解码核对严格 canonical payload、全部重复 head／history 字段和完整选中历史。文件、发布者、
接收者、签发者及有效期不可更改。授权以 revision 1 Active 开始；
`revoke(id, actor, expected_revision)` 只允许 CAS 转为 revision 2 Revoked，保留签发者并准确
记录独立撤销者。没有重新激活、续期、更新、删除、接纳或重试操作。

所有保留授权占用额度，包括过期、已撤销和旧账号代际记录。写入在检查重复 ID 和额度之前，
先校验有界完整账本；重复冲突先于额度错误。全账本校验也拒绝孤立、多余或损坏的历史。

可信写入在读之前选择 READ COMMITTED，保留配置的 READ ONLY，并按序取得三表
SHARE ROW EXCLUSIVE 屏障。`SET CONSTRAINTS ALL IMMEDIATE` 后复核原命名空间／关系形状、
安装、完整预期记录及账本，恢复同步提交，只有 COMMIT 确认后才返回 `StoredFileRecipientGrant`。
取消、超时或丢失确认不证明未提交。

## 当前 Session 本人元数据

`DurableAuthSource::file_recipient_grant_workspace(...)` 构造惰性的
`SessionFileRecipientGrantWorkspace`。`get_session_file_recipient_grant(token, workspace, id)`
从当前审计活跃 Human 身份、工作区成员资格及账号代际推导接收者，不接受请求提供的接收者，
也不要求教师或授权管理角色。

认证源持有一个 READ COMMITTED 事务，外层上限十秒。借用读取保留原命名空间／schema 固定和
完整选中历史指纹。三表 SHARE 屏障先于 marker／行的 `FOR SHARE` 锁，持续到最终新鲜
Session／Registry／策略复核和认证源 COMMIT 确认。借用帮助器不独立连接、开事务或提交，
不覆盖 READ ONLY 策略。

缺失、他人接收者和旧代际授权返回经授权的 `None`；他人 payload 在解码前过滤，选中损坏记录
则拒绝。本人未来生效、过期和已撤销记录仍可作为元数据读取；Active 不是下载权限，这个读取
不执行授权访问时间窗。直接可信 `get` 仍是选中记录的 READ ONLY repeatable-read 元数据查询，
不是完整账本健康检查，也不是当前 Session 决策。

`SessionFileRecipientGrantError` 区分 Session、Grant 和 InvalidNamespace，不在这个只读阶段
引入未使用的管理权限错误或角色转换。

## 测试与后续交付门槛

`file_recipient_grant_tdd` 覆盖可信账本，`session_file_recipient_grant_tdd` 覆盖当前本人读取。
`scripts/test-file-recipient-grants.sh` 和 `scripts/test-session-file-recipient-grants.sh` 要求显式
临时数据库，并逐个准确选择 ignored 用例。`fileRecipientGrant` 公共守卫只检查库存与组合，
不证明 SQL 锁、认证或真实字节交付行为。

当前认证管理者签发／撤销接收授权仍是独立门槛。下载准入还须在适当保留屏障内检查准确可信
Published 版本、当前接收者、Active 授权和新鲜时间；真实字节及客户端交付另有生命周期。
本轮没有下载命令、字节 IO、HTTP、OpenAPI／SDK／UI、传输、启动安装器、部署、配对、恢复
或在线认证切换。旧执行记录和分日期成熟度评分不变，也不把旧元数据观察升级成下载权限。

后续 10 月 12 日[发布者本人分享](large-file-sharing.md)在本账本上增加独立当前 Session
签发和原发布者撤销：签发以当前发布权限比对本人准确可信 Published，撤销不要求发布许可
仍有效或接收者仍存在。不增加下载消费端或全局管理委托，也不改下方本账本／读取执行记录。

后续[下载清单观察](large-file-download-manifest.md)为一次当前元数据结果消费选中原 Active
授权：准确所授权发布者／Published 版本、完整原历史及最终新鲜授权／Session 有效期。
不放宽旧元数据读取、不访问文件，也不把任一返回值变成可复用字节许可；下方执行记录不变。

## 实际执行记录 · 2026-10-11

两个准确基线均先编译成功，再在真实缺失行为处失败：全新账本安装、当前无角色接收者读取，
分别在清理夹具后得到 **0 通过／1 失败**。前期 scaffold 语法及测试 URL 的 empty-host 错误
在这些运行前已修正，不计作行为 Red。

额外原始路径回归确认鬼目录确实不存在，却得到 `Ok(None)` 而非 `UnsupportedSchema`，在
清理后得到 **0 通过／1 失败**。只将新账本的原始目标限制为裸名或规范引号后通过；原事务
身份检查、旧发布账本策略和共同命名空间机制没有放宽或改变。

首次完整账本矩阵通过基线，随后独立性夹具的命名空间前缀加 UUID 共 64 字符而失败。
仅缩短夹具前缀，生产 63 字节限制不变；业务断言 panic 先清理准确夹具，再保留原失败。
完整账本重跑及首次完整 Session 矩阵均通过且无警告。

最终显式 PostgreSQL 16 执行使用全新私有 Unix socket 实例，不监听 TCP，不使用环境中的
应用数据库；每个脚本执行前均核对编译后的准确 ignored 库存：

| 执行脚本 | 通过的 SQL 用例 |
| --- | ---: |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |

共 **81 个实际执行的 SQL 用例**，零失败、零警告。新账本默认测试另有 3 通过、12 忽略，
新 Session 默认测试有 3 通过、7 忽略。完整后端质量门通过 **850 个默认 Rust 用例**，
**800 个忽略用例单列**，共 81 组结果，含三个 compile-fail 文档测试；**266 个公共守卫**
全部通过，编译器警告为零。ignored 库存不计作 SQL 已执行数量；前端逻辑和公开 API 未变。

两次独立源码复查未发现额外实质缺陷。真实 COMMIT 前延迟等待取消及独立排空回滚观察，
不是一般丢失确认模拟。同步探针只检查延迟刷新时的设置，不证明 fsync、断电或确认时耐久性。
行等待和历史追加阻塞依据真实数据库 backend blocker，不仅凭经过时间判断。

106 段历史状态记录、20 份既有详细执行尾部及分日期机器能力张量 SHA-256 保持不变。
执行后没有残留私人测试命名空间或其他数据库客户端；全新 PostgreSQL 实例已停止，
可重建的数据／socket 已删除（约 181 MiB），验证日志保留。未改变应用数据库、版本、提交或推送。
