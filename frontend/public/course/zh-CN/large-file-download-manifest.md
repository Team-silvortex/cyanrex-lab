# 接收者下载清单观察

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.6**，API **0.3.0**，**2026-10-12**。
本轮延续[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，将[接收授权](large-file-recipient-grants.md)
与准确可信 Published 元数据相连。当前符合条件的接收者可读取所授权版本的完整清单，包含
有序分块摘要。清单是敏感元数据，不是可重复使用的下载许可、当前字节健康、发布确认或后续字节披露权限。

## 可信配置与准确请求

`DurableAuthSource::file_download_workspace(publication, recipients)` 从可信
`FileUploadPublicationStore` 和 `SessionFileRecipientGrantWorkspace` 固定
`SessionFileDownloadWorkspace`。工作区／authority 必须准确相同，两个受限命名空间须不同。
构造不做 IO、安装、迁移或角色转换；只使用认证源事务，独立 store 连接池无需保持打开。

`SessionFileDownloadRequest` 包含 `grant_id`、`upload_id` 和 `expected_file: FileVersionRef`。
它们只是选择器，不是客户端提供的授权、接收者、发布者、安装或许可。接收者由当前审计
活跃 Human／成员及准确账号代际推导。教师、部署、授权管理者和文件所有者身份都不能
代替接收授权；本人下载也要显式授权。

## 一次当前清单观察

`get_session_file_download_manifest(token, workspace, request)` 在认证源持有的一个
READ COMMITTED 事务内运行，外层十秒上限，保留配置的 READ ONLY 策略，只有完成后才返回
`FileTransferManifest`：

1. 低成本 token／认证源／请求范围检查先于 DB。借用接收读取按当前准确接收者／账号
   先过滤再解码；原 schema、完整选中历史及指纹持续保留，持有三表 SHARE 屏障和
   marker／行的 `FOR SHARE` 锁。
2. 授权必须 Active，并与完整请求 `FileVersionRef` 一致。由记录原发布者／账号选择
   Published 所有者，不用当前接收者或请求指定发布者。
3. 准确 upload、发布者／账号、file ID／version ID 条件在发布正文解码前过滤；可信清单
   投影的完整版本还须与授权／请求一致，包括工作区／authority、字节长度和 SHA-256。
   原 schema、隐藏绑定、nonce、完整 canonical 清单及指纹在 marker／行锁内持续保留。
4. 复核两个原 pending 记录及命名空间／关系身份。最终当前认证源／Registry／接收者
   检查后，一个新鲜数据库时钟样本同时覆盖原接收授权半开有效期与准确 Session 过期。
   此后没有 SQL，直接请求 COMMIT；只有认证源提交确认后才交付清单。

资格以最后样本为准：未来授权可在真实等待期间生效，但最终过期或已撤销授权不能返回清单。
这不是确认时新鲜性或返回后持续授权。发布权限到期／撤销、发布者退休／删除不会隐式撤销
独立接收授权；Published 仍须符合原发布者／账号绑定，重建账号不继承它。

返回清单保留已有几何及元数据上限，最多 16,384 个分块；不包含活权限、原文件描述符或
恢复的发布回执。本命令不读文件，不查上传／分块账本或发布权限存储。

## 失败及后续字节边界

`SessionFileDownloadError` 区分 Session、Recipient、Publication、InvalidNamespace 和
PermissionDenied。缺失／他人接收授权、非 Active、错误完整版本、缺失／不匹配 Published
及最终无效授权时间窗都拒绝观察。他人接收正文及授权所有者／文件／版本以外的发布正文
在解码前过滤；选中损坏元数据保留类型错误，不伪装成不存在。这不是恒定时间或完整时间探测防护。

旧本人接收／本人发布读取保留原元数据策略，其快照不能替代当前资格检查。本轮不写入、
释放额度、清理、重试、接纳、恢复，也不增加共享模型／表 schema、启动安装器或在线认证切换。
取消或超时不交付清单，也不是对所有 COMMIT 结果的普遍证明。

真实下载仍要独立单块读取：私有有界字节、准确长度／摘要、原描述符／根目录／来源绑定、
随实际工作／响应保留的独立额度，以及披露前的新鲜当前授权。通用 checked copy 可能在
发现摘要错误前先向 sink 写出前缀，不能直接接客户端并宣称整块已验证。
本轮不连接字节命令、HTTP、OpenAPI／SDK／UI、传输、客户端落盘、重启续传或桌面配对。
分日期成熟度评分及历史执行记录保持不变。

`session_file_download_tdd`、`scripts/test-session-file-download.sh` 覆盖显式临时 SQL；
`fileDownloadManifest` 公共守卫只检查组合／库存，不证明真实权限、等待或交付。

后续独立[已校验分块读取](large-file-chunk-read.md)实现至多 4 MiB 私下字节校验和原授权重入，
不把此处返回清单复用为权限，也不接通 HTTP／客户端送达。仅清单命令及下方原执行记录不变。

新增[准确清单客户端](large-file-manifest-client.md)可获取完整匹配元数据并组合顺序私有下载，
不替代后续块的当前授权。独立未发布的[清单 HTTP 适配](large-file-manifest-http.md)
现经准确元数据 GET 投影此命令，
不挂载、不授予后续字节权限；原命令及下面的历史执行记录不变。

## 实际执行记录 · 2026-10-12

编译后的准确基线先建立真实 17 字节 Session 发布及 Session 签发分享，再因缺失清单行为
返回 Recipient `UnsupportedSchema` 而失败：清理夹具后 **0 通过／1 失败**，零警告。
不是缺失符号或零测试结果。同一基线实现后 **1 通过／0 失败**，新取消测试当时有一条
暂态未使用 import 警告；只删除该 import 后，最终默认运行 **3 通过／0 失败／10 忽略**，
零警告。完整准确 SQL 矩阵随后 **10 个不同用例**全通过，零失败、零警告；单独基线 Green
是同一用例复跑，不算第十一个 SQL 用例。

真实同发布者 Published 分别正交隔离 file ID 与 version ID，均是相同真实 17 字节内容；
健康及损坏的他人正文都先过滤。当前接收者、账号代际、Active 状态、半开有效期及选中
canonical／历史损坏均有独立拒绝与健康对照。发布者暂停、退休、删除及发布权限撤销不会
隐式撤销独立接收者的元数据访问。安装连接池关闭、配置分块路径不可用仍可进行认证源
持有的清单观察；这不证明字节健康或可真实下载。

真实 backend blocker 证明撤权、历史追加及独占 DDL 写入排在原读取者之后；DDL 探针在
健康对照前恢复原物理表／OID，不把通用表副本当成合法认证安装。复查将不变快照移到
独占写入排队之前，避免夹具阻塞自己释放读取者。真实等待后的数据库时间证明 Session
到期、授权到期及 not-before 生效均以最终授权点为准。五类选中行／marker 取消等待独立
排空锁，并保持完整元数据／文件快照。这不是丢失 COMMIT 确认、重启恢复、当前字节健康、
下载吞吐或客户端交付验收。

最终 PostgreSQL 16 执行使用全新私有 Unix socket 实例，不监听 TCP，不使用环境中的应用
数据库；每个脚本运行前均核对编译后的准确 ignored 库存：

| 执行脚本 | 通过的 SQL 用例 |
| --- | ---: |
| `test-session-file-download.sh` | 10 |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |
| `test-session-file-sharing.sh` | 10 |

共 **101 个不同的实际执行 SQL 用例**，零失败、零警告。停止临时数据库后，完整后端质量门
通过 **856 个默认 Rust 用例**，**820 个忽略用例单列**，共 83 组结果，含三个 compile-fail
文档测试；**276 个公共守卫**全部通过，编译器警告为零。ignored 库存和单独基线复跑不计作
额外已执行 SQL 用例。

独立源码／测试／证据复查未发现额外实质生产缺陷；双语源文档及生成课程副本已同步。
**106 段历史状态记录**、**24 份既有详细执行尾部**及分日期机器能力张量 SHA-256 保持不变。
停止 PostgreSQL 前没有残留私人测试命名空间或其他数据库客户端；可重建的数据／socket
已删除（约 **182 MiB**），验证日志保留在 `/tmp/cyanrex-file-download-8QpeUo`。
未改变应用数据库、产品／API 版本、提交或推送。
