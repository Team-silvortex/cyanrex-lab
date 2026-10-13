# 准确清单客户端与获授权下载

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**，API **0.3.0**，**2026-10-12**。
本轮沿用[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，不属于已发布的
0.5.7 标签，连接独立[清单 GET](large-file-manifest-http.md)与既有
[顺序私有下载器](large-file-download-client.md)。不挂正常应用路由、不签发 Session、
不切换在线认证，也不增加面向用户的保存操作。

## 两个显式入口，同一原始组合

`FileChunkClient::fetch_manifest(token, grant_id, upload_id, expected_file)` 返回完整校验后的
`FileTransferManifest`。`fetch_granted_and_assemble` 先获取清单，再顺序请求每块、等待其实际
私有暂存确认，最后在同一原根拼装。两者共享原可信组合；独立元数据只使用 HTTP 和缓存
作用域，不调用接收器或文件任务，组合入口才复用接收器／暂存。
原 `fetch_and_assemble(..., supplied_manifest)` 保持兼容，仍不获取元数据；它的空文件仍是
零 HTTP。新获授权入口即使为空文件，也必须先成功完成一次元数据 GET。两者都不是续传。

准入和网络 IO 前，期望工作区必须等于缓存的原暂存作用域，Session token 必须是规范非零、
小写连字符 UUID。强类型 grant／upload ID 已满足共用契约。目标只由配置 origin 加固定
`/platform/v1/file-manifests` 构成，准确六项选择器为 `grant_id`、`upload_id`、`file_id`、
`version_id`、`byte_length`、`sha256`。请求不能选择另一 URL、作用域、owner 或路径。

每次原生 GET 发送一项敏感 `cyanrex_platform_session` Cookie、
`x-cyanrex-platform-request: 1`、`Accept: application/json` 和 `Accept-Encoding: identity`。
不发送 Origin、Authorization、请求正文、Range 或条件选择器。配置的传输 origin 不是
浏览器 Origin 权威。既有 HTTPS 证书／SAN、可选公共 CA、明确字面环回 HTTP opt-in、
无代理／重定向／重试／解压、HTTP/1 及关闭空闲连接池保持不变。TLS 不证明当前授权。

## 有界响应、实际消息 EOF 与准确版本

只接受 HTTP 200。非 200 和重定向统一静态 `InvalidResponse`，不信任其正文，不转发诊断。
至多 **64 项响应头／8192 字节名称和值**，重复字段独立计数。下列各项准确出现一次：

| 响应头 | 准确接受值 |
|---|---|
| Content-Type | `application/json` |
| Content-Length | 正数规范 ASCII 十进制，至多 **1152 KiB** |
| Cache-Control | `no-store` |
| Pragma | `no-cache` |
| X-Content-Type-Options | `nosniff` |

拒绝编码、范围、transfer-encoding、trailer、Cookie、重定向、ETag、Last-Modified、
Retry-After、Content-Disposition、全部六项二进制文件传输元数据头，以及所有
`access-control-*` 响应头。其他有界响应头不提供权限或文件系统目的地。

实际 reqwest 响应转为帧 Body，不用便利 JSON／bytes 接口预收集。收集器一直轮询到真正
HTTP 消息 **EOF**，不信任大小提示或提前结束标记。短正文、额外字节、trailer、非数据帧和
正文错误一律拒绝，包括声明长度已满之后的错误。收集字节 Vec 使用可失败的
`try_reserve_exact`，不代表所有 Serde／模型、帧、Box 或传输分配均可恢复。复制以 **64 KiB** 分段
让出执行并检查时钟。立即就绪的空帧让出执行，但不刷新 idle。只有接受的非空字节进展刷新
空闲时限；总时限从响应头完成起固定。等待响应头和正文使用既有分离配置及返回后的时钟
检查。HTTP 消息 EOF 是库帧层边界，不证明原始 TCP 后缀或连接 EOF。

原始字节直接进入共用严格 `FileTransferManifest::parse_json`，不用会抹掉重复字段的 JSON
Value 中间层。Schema、未知／嵌套重复字段、UTF-8、展示名、分块几何及有序摘要均沿用
模型检查。完整 `FileVersionRef::from_manifest` 必须等于 `expected_file`：authority／工作区、
文件／版本 ID、长度及整文件摘要，而非少数回显头。末尾非空白不能通过解析。
同步有界解析前后检查时钟，不声称能强制抢占。

**16,384 块／64 GiB** 是元数据声明上限，不是实际负载吞吐、RSS 或全局内存配额证明。
完成后保留强类型元数据不保留准入。匹配清单不是认证的远端主体来源、送达回执、可复用
字节权限或持续授权；每个后续 Source 分块请求仍独立授权。

## 准入、取消及证据保留

独立、克隆共享的清单 gate 沿用既有容量：默认 **4**，可信范围 **1–16**。覆盖响应头、
正文 EOF、严格解析及完整版本比较，独立于旧分块请求及整流程 gate。独立构造客户端的
预算相加。`FileManifestClientError` 只公开 `InvalidInput`、`CapacityExhausted`、`TimedOut`、
`Unavailable`、`InvalidResponse`，均为静态文字，不暴露 token、路径、远端正文或传输诊断。

组合入口在元数据 GET **之前**取得原整流程名额，之后复用下载核心而不再次准入，因此
容量一不会自锁。`FileGrantedDownloadError` 区分输入／流程耗尽与 `Manifest(...)`、
`Download(FileDownloadError)`；旧公开下载错误枚举不增加变体。元数据阶段取消释放两种
名额，尚无文件任务；实际排队／运行暂存或拼装时，既有任务持有共享流程 lease 跨等待者
取消存活，不启动下一网络请求或后台续跑。时限逐请求计算，不是整文件硬截止，也不强制
杀死 OS／数据库／CPU 工作。

元数据失败不请求分块，后续块或拼装错误立即停并保留已确认／不确定证据。重复调用仍遇到
既有独占目标 Conflict，不采用、跳过、删除、清理、重试或恢复进度。本地整文件确认沿用
拼装器实际 EOF／摘要／读回含义，不是最终远端授权或用户保存。

## 验证与剩余边界

`file_manifest_client_tdd` 包含真实环回默认测试及显式 ignored 当前 Source 桥接。
`scripts/test-file-manifest-client.sh` 强制显式一次性数据库，先检查编译后的准确 ignored
清单，再只执行该清单。私有帧 Body 单元补测可能先被 HTTP 库拒绝的边界。
`fileManifestClient` 守卫只检查组合／清单，不证明真实授权；原生执行结果另录于下文。

原拼装确认可另进入[显式私有保存](large-file-save.md)，复制到可信宿主选择的新文件；
该操作不请求 GET、不授予远端访问。正常 AppState／监听挂载、平台 Session 签发、公开 OpenAPI／SDK／UI、发现、用户选路径
最终保存、进度／续传、桌面点对点、迁移及局域网部署验收仍独立。版本／依赖、已发布
0.5.7 验证、**106 个历史状态块**、**40 段既有执行尾部**及分日期能力张量保持不变。

新增[授权交付调用](large-file-delivery-client.md)将这些阶段合为一次显式操作，原整流程名额跨真实
块／拼装／保存任务及取消存活；首失败停止，不重试／采用，界面保存、续传和正常挂载仍独立。

独立[本人普通目录目标](large-file-user-save.md)以明确选择允许 0700／0750／0755，不放宽
私有暂存；核对原目录 UID／GID／模式并沿用实际保存，文件选择、共享可写目录、续传和挂载仍独立。

## 实际执行记录 2026-10-12

两个新惰性 API 入口先针对真实环回 HTTP／私有根正常编译，再在夹具清理后出现行为 Red：
**0 通过／2 个预期失败**，分别为 `Unavailable`、`Manifest(Unavailable)`，零 warning。
同两例 Green 重跑 **2 通过**、零 warning。Source hook 首次编译缺自身 `fixture::scope`
重导出（`E0432`），增加独立 Source 作用域桥接后修正；该编译错误不算行为 Red。

首次默认运行 **15 通过／1 失败／2 ignored**，零 warning：测试正文很短且立即结束，声明的
Content-Length 却超限，Hyper 可在头部 flush 前拒绝，得到 `Unavailable`。只给该夹具实际
首正文帧增加已登记清理屏障，稳定先观察响应头；未放宽生产上限或通用拒绝断言。
首次静态守卫还发现返回后时钟变量期望陈旧，改成实际捕获的 `body_started`，不是改动时限。

| 新鲜选择 | 实际结果与范围 |
|---|---|
| 默认清单客户端目标 | **16 通过／2 opt-in ignored**，零 warning；真实六项原生 GET／完整元数据，头／字节／严格 JSON／版本拒绝，最大声明元数据和实际有界 JSON，重定向目标零命中，真实头／idle／固定 total，克隆／默认四／独立预算及旧块 gate 独立，取消、容量一组合、空 GET、首失败／前缀保留及重复 Conflict |
| 新私有契约／帧／工作单元 | **16 通过**，零 warning；4 头、9 收集器、3 生命周期，含虚假 hint／提前结束、真正 None、长度之后错误／trailer／额外字节、立即空帧让出、Pending EOF、原响应头时钟、最大摘要数组、元数据取消及实际排队分块／空拼装名额 |
| 新显式 Source 网络 runner | **2 条唯一 SQL 通过**，零 warning；真实 Session 发布及独立接收授权的 4 MiB + 17／空文件，实际元数据 → 顺序块确认 → 同根 `f1`，独立 EOF／SHA／模式／inode／0400／单链接及服务端快照不变；元数据后、块派发前真实撤权得到 200／404 且无文件，已撤权空元数据 404 且无拼装 |
| 取消回归选择 | **29 通过／7 opt-in ignored**，零 warning；含新 3 生命周期及共用核心的既有运行保存／拼装单元，不冒充新公开入口独立运行工作 hook |
| 相邻显式 runner | **8 既有清单 HTTP＋6 二进制 HTTP＋1 提供清单下载＋1 单块 SQL＋1 既有 OpenSSL TLS 通过**，零 warning；含新 2 条 SQL 共 **18 条唯一新鲜 SQL＋1 既有 TLS**，不是新的元数据 TLS 部署证明 |
| 完整后端门禁 | `scripts/quality-gate.sh --backend-only`：**1005 Rust 通过／852 opt-in ignored／91 结果组**、**316 公共检查通过**，零失败／warning；新 16 默认和 16 私有单元已计入 |

原生组合／库存定向守卫 **25 通过**。独立生产、Source 测试及中英文复查无剩余可行动缺陷；
澄清独立元数据只做 HTTP，组合才接收／操作文件，可失败分配只指正文收集。
不意味着新前端／SDK／安全审计、浏览器／界面保存、续传、元数据 TLS 部署、64 GiB 载荷、
吞吐／RSS、持续授权或在线认证迁移验收。单独 ignored SQL 不是默认验收，重复基线和
定向单元也不增加完整门禁的唯一用例数。

生产代码先冻结后执行私有单元；全部代码编辑者冻结后由 root 串行执行集成／SQL／完整门禁。
最终临时 PostgreSQL 16 审计确认准确自有数据路径、不监听 TCP、**零夹具 Schema、零其他
客户端**。停止后只删除已验证的 `data`／`socket`，释放 **59084 KiB（约 58 MiB）**可重建
测试数据；日志保留于 `/tmp/cyanrex-manifest-client-j2GSrodQ`，未移除项目／部署或无关缓存。
版本／依赖、在线 API／SDK／UI、已发布 0.5.7 报告、**106 段旧状态**、**40 份旧执行尾部**
和能力张量不变。中英文源文档／课程副本同步；未提交／推送。
