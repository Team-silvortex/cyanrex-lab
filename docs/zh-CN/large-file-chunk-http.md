# 私有单分块二进制 HTTP 适配

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.6**，API **0.3.0**，**2026-10-12**。
显式构造的适配器把一个严格 GET 接到已有[授权分块读取](large-file-chunk-read.md)。
成功返回不可变的已校验单块，不是下载回执。路由仍**未挂载**：不增加正常应用监听、公共
OpenAPI／SDK／UI、Session 签发、客户端保存／续传、桌面配对或在线认证切换。

## 可信组装与准确请求

`FileChunkReadHttpState::new(source, workspace, origin)` 接收可信 `DurableAuthSource` 和
`SessionFileChunkReadWorkspace`。`with_capacity` 接收 1–16 个 HTTP 名额，默认四个。
Origin 沿用规范 HTTPS 或回环 HTTP 策略（localhost 或回环 IP）；构造是惰性的。状态 clone 共享 HTTP 预算，
独立状态预算可累加；Source workspace clone 另共享原已校验字节预算。

`build_file_chunk_read_router(state)` 只接受 **GET `/platform/v1/file-chunks`**。其他方法包括
HEAD 返回私有 405 与 `Allow: GET`；HEAD 无正文。未知路径返回私有 404。
原始 query 至多 512 字节，下列键各且只能出现一次，顺序不限：

| 键 | 合法值 |
| --- | --- |
| `grant_id`、`upload_id`、`file_id`、`version_id` | 小写规范非 nil UUID |
| `byte_length` | 规范无符号十进制，至多 64 GiB |
| `sha256` | 小写 64 位十六进制整文件摘要；空长度要求标准空摘要 |
| `chunk_index` | 规范无符号十进制，0–16,383 |

不通过解码或归一化接纳百分号转义、`+`、空白、空／重复字段、未知键、多余等号、符号或
前导零数字。空文件及本地清单索引由已有授权命令检查。客户端不能选择 workspace／authority、
接收者、发布者、所有者、账号代次、命名空间、根目录或路径。scope 来自宿主配置；准确文件
引用只是选择器，从来不是权限。

## 准入与当前授权

端点／query 和不支持的 Range／条件头在正文轮询或 SQL 前拒绝。不支持 `Range`、`If-Range`、
`If-None-Match`、`If-Match`、`If-Modified-Since`、`If-Unmodified-Since`；这是准确分块协议，
不是任意范围、206 或 304 缓存接口。

必须有专用规范 `cyanrex_platform_session` cookie 和 `X-Cyanrex-Platform-Request: 1` 标记。
给出的 Origin 必须与配置准确相等；GET 按已有原生客户端策略允许省略 Origin。
Authorization／Bearer 头和 query 凭据被拒绝，沿用有界 cookie 请求头策略。
轻量检查先于快速失败的 HTTP 名额准入。

准入后的 GET 必须在两秒内收集空正文；拒绝 Content-Encoding、非空／错误／trailer 正文及
不支持的媒体类型。等待不返回或无限就绪空帧不能绕过正文截止时间。可选 Content-Type 沿用
已有准确 JSON 媒体类型策略；不解码任何 JSON 请求正文。

随后只调用一次已有 `read_session_file_chunk`，由两笔原固定认证源事务围住私有文件工作，
推导当前已审计 Human／成员／账号，并要求本人 Active 准确版本接收授权。教师、发布者或
部署角色都不能绕过授权。最终同钟 Session／授权检查和认证源提交确认先于响应投影；
HTTP 层不再做 SQL、文件读取、授权采纳、修复或写入。

| 失败 | 私有 HTTP 投影 |
| --- | --- |
| 缺失、他人、过期、撤销或准确版本不符的接收权限 | 404 `not_found` |
| Source 报告无效 Session 或凭据 | 401 `invalid_session` |
| 当前身份／成员不活跃或被拒绝 | 403 `access_denied` |
| Source 字节预算满 | 503 `capacity_exhausted` |
| 已授权空文件或超出其清单的本地索引 | 400 `invalid_request` |
| 选中记录损坏、Schema／根／文件失败、超时或配置失败 | 503 `service_unavailable` |

派发后的 outcome 为 `unconfirmed`；轻量／正文／HTTP 准入失败为 `not_attempted`。
错误不包含 SQL 原文、隐藏密钥、路径、账号身份或字节前缀，不带 Retry-After、Set-Cookie，
也不自动重试。取消不证明 SQL 回滚或已经派发的 IO 被终止。

## 二进制响应与字节所有权

另行[受控客户端](large-file-chunk-client.md)从宿主冻结 Origin 显式请求本路由；原生 GET 省略
UI Origin，仍使用原 cookie／标记策略。不挂路由，不使整文件、续传或部署流程上线。

另行[接收器](large-file-chunk-receiver.md)校验已提供响应再本地独占暂存，不挂载本路由、
不主动请求，也不增加整文件保存／续传。

成功为 200、`application/octet-stream`、准确 Content-Length 及固定
`Content-Disposition: attachment; filename="chunk.bin"`。全部响应有 `Cache-Control: no-store`、
`Pragma: no-cache`、`X-Content-Type-Options: nosniff`；不增加 CORS、ETag、Last-Modified 或 Content-Range。

六个固定元数据头只暴露选中的公开版本坐标：`x-cyanrex-file-transfer-version: 1`、
`x-cyanrex-file-id`、`x-cyanrex-file-version-id`、`x-cyanrex-chunk-index`、
`x-cyanrex-chunk-sha256`、`x-cyanrex-file-sha256`。单块摘要来自原服务端选中清单，已经按
真实字节验证；整文件摘要只是声明元数据，**不是新鲜整文件重哈希**。
用户显示文件名及私有绑定密钥不进入响应头。

Source 只移动结果与 HTTP 名额进入私有零复制 `Bytes::from_owner` 所有者。
未轮询响应、已完成但未领取的响应、数据帧及任何 Bytes clone／slice 保有所有者时，两种
名额都继续占用。正文结束不释放仍被外部数据持有的名额；最后一个所有者丢弃才归还两种预算。
这比既有 JSON 准入包装更严格；旧接口在最终帧／结束／错误／丢弃时释放，已准入 JSON 错误
仍沿用旧行为。它不是全局 RSS、带宽或代理缓冲配额；可信消费方仍能在所有者之外深复制字节。

**首帧十秒截止时间**从 Source 返回后立即开始。首次轮询过晚只返回静态正文错误，不产生
数据帧，并丢弃两种名额；没有自主计时器，已过期但未轮询的正文仍占额度直到轮询／丢弃。
数据帧交出之后，Bytes 所有者按存活期保留预算，不是按送达超时释放。
响应此前已经构造成 200 与 Content-Length；过晚轮询的正文错误不替换成 JSON 或 503，
响应头可能已经发出，因此不能视为一块完整成功响应。

授权是最终认证源快照，不在首帧轮询或每帧重新授权；之后撤销无法收回已批准的字节。
首帧窗口只限制命令之后的响应延迟，不是撤权传播、socket 送达或远端回执的时限。
认证源各阶段截止时间与真实任务存活期的工作额度保持独立。

## 验证范围

`file_chunk_read_http_tdd` 检查进程内请求与真实响应帧；
`scripts/test-file-chunk-http.sh` 显式运行一次性 PostgreSQL／合成文件库存。
私有解析／正文单元覆盖词法及所有者／期限行为；`fileChunkHttp` 公共守卫只检查源码组合、
文档和库存。它们不代替已挂载 TLS／局域网传输、客户端落盘、最终整文件核验、续传或部署
兼容性评审。旧执行记录及分日期能力张量保持不变。

## 实际执行记录 2026 年 10 月 12 日

已编译行为基线使用真实 Published 字节和当前 Session 分享；惰性 503 响应在预期 200 处失败：
**0 通过、1 条预期失败、0 警告**，断言前已清理文件和 Schema。实施后的同一基线
**1/1 通过**，零失败、零警告。缺少符号或环境拒绝不计行为 Red。

默认目标 **9 条通过**，六条 SQL 明确忽略，零警告。首次受限环境执行时，九条都停在私有根
准备阶段（`ChunkStaging::open` 返回 Unavailable）；相同代码用原生用户权限执行后全部通过。
这项环境记录单独保留，没有放松描述符／所有者检查或改变生产行为。
**九条私有单元**也通过：五条严格原始 query、四条字节所有者／期限。
私有单元只使用合成所有者，不能制造公共已验证 Source proof。

精确 `test-file-chunk-http.sh` 完整库存实际执行 **六条不同 SQL 用例**，全部通过，零失败、零警告：

| 模块 | 实际覆盖边界 |
| --- | --- |
| baseline | 真实授权 17 字节、全部固定二进制／隐私头及完整不变元数据／文件快照 |
| behavior | 真实 4 MiB 加 17 字节、选择完整块／尾块及错误本地索引；未请求尾块缺失不妨碍健康所选块，发布撤权不级联 |
| authorization | 无接收权限的发布者／教师、缺失上传／授权、逐项错误文件／版本／长度／摘要、过期／未来／撤权、暂停／登出及恢复；间隙内原授权撤销和发布 nonce 漂移 |
| storage | 所选摘要错／短／长字节、Published 清单损坏及关系标记错误；私有 503、无前缀／修复、注入状态不变及健康恢复 |
| response_lifetime | 未轮询／丢弃响应、真实数据帧、clone／非空 slice、已完成未领取响应均占两种预算；最后丢弃才释放，真实 10.1 秒后首帧轮询只报错不交数据 |
| waits | 真实最终发布标记等待跨 Session／授权到期；同一 captured workspace 仍占字节额度；取消后锁及额度排空、快照不变和健康恢复 |

快照覆盖 Auth／Registry、两种授权、上传／日志／Published 行及关系 OID，还有根与暂存字节／
身份。全部新矩阵失败会先清理合成 fixture 再重新抛出。首笔事务连接归还屏障、后续真实
PostgreSQL 阻塞观察及 Source 控制流共同辨认文件工作之后的最终等待；通用 pool 回调自身
不能区分 COMMIT 和回滚，也不是文件工作线程回调。

执行前复审修正了一处测试：它原先检查原 fixture workspace，而不是独立构造的 captured
workspace 的字节预算。现在按同一 captured 配置断言，没有改动生产行为。
认证源与 HTTP 首帧限额仍不同于真实 socket／局域网送达或远端保存。

十一条相邻 runner 也重新实际执行：接收账本 12、Session 接收 7、发布账本 12、Session 发布权限 7、
授权管理 10、Session 元数据 10、Session 发布 10、可信发布 13、分享 10、下载清单 10、原分块读取 10。
加上新增 HTTP 六条，共 **117 条不同且明确执行的 SQL 用例**，零失败、零警告；
基线重跑及忽略库存列举不重复计入。

一次性数据库在关闭前没有遗留测试命名空间或其他客户端，也无 TCP 监听。
只删除可重建的数据／socket：**219,248 KiB**，约 **214 MiB**；日志保留于
`/tmp/cyanrex-file-chunk-http-vGGaSKEu`，没有改变应用数据库。

PostgreSQL 关闭之后，后端完整质量门禁通过 **884 条默认 Rust 用例**、**85 组结果**，
另核对 **836 条忽略用例**及 **286 条公共守卫**；零失败、零编译警告。
新增九条默认 HTTP 与九条私有单元已包含在 884 内，不额外重复计数；原四条真实文件任务
存活期测试及三条只移动／禁止序列化 compile-fail 契约也仍在这一总数内通过。

中英源文档及生成课程副本同步；**106 个历史状态块**、**28 段既有详细执行记录尾部**
及分日期能力张量 SHA-256 均不变。没有改变产品／API 版本、应用挂载或数据库迁移，未提交或推送。
