# 授权下载及私有保存

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**，API **0.3.0**，**2026-10-13**。
本轮沿用[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，将
[准确清单客户端](large-file-manifest-client.md)、有序私有拼装和
[显式私有保存](large-file-save.md)合成一次可信宿主调用。不挂应用、不签发 Session，
不替用户选名字、不接界面保存或恢复旧进度。

## 显式目标与单一流程名额

`FileChunkClient::fetch_granted_and_save(token, grant_id, upload_id, &expected_file, &target)`
返回原 move-only `SavedFile`，仅公开 `file() -> &FileVersionRef`。宿主在派发前打开
`FileSaveTarget`：旧本 UID 私有 0700 父目录，或以 `open_user_directory` 明确选择
[本人普通目录](large-file-user-save.md)的 0700／0750／0755，以及独立 UTF-8 名字；远端显示名
不成为目标，共享可写目录仍拒绝。构造会核验并保有当时父目录，但不预留输出，
不证明稍后派发／保存时路径仍健康，也不授予远端权限。

预期 workspace、规范非 nil Session token 形状，以及缓存目标根不同于暂存根，在准入／
HTTP 前核对。调用仅一次取得原克隆共享整文件流程 gate（默认 4，可信范围 1–16），
不再次进入旧公开下载入口准入，不新增交付 semaphore，也不跨网络等待占据暂存 worker。

父流程名额跨以下相依阶段保持：

1. 原固定清单端点和独立元数据 gate 获取完整准确版本；沿用严格头、实际消息 EOF／JSON、
   有界长度和完整版本比较。
2. 每块真实私有暂存确认后才请求下一块，旧接收／暂存预算不变，各 Source 块独立复核
   当前接收授权。
3. 在同一暂存根拼装原私有整文件，内部证明保有原只读文件／根和完整清单绑定，
   不采用相同字节的另一 inode。
4. 借用原确认，沿用三遍源／复制／输出校验、同步和原 inode 读回，独占保存至所选目标。

空文件也须清单 GET，再实际空拼装和所选保存，不造块请求。原既有清单下载／授权拼装
入口行为及错误变体保持兼容；独立保存仍不持整下载流程 lease。

## 取消与失败

实际排队／运行的文件任务同时保有共享父流程引用和原 worker／证明／目标资源。丢弃
等待者不提前归还流程名额；元数据／HTTP 等待时取消不启动后续块、拼装或保存。
文件任务已派发后，取消等待不返回确认，但原任务仍可能完成；不新起后台编排、重试、
恢复、采用或清理。

新静态错误 `FileGrantedSaveError` 为 `InvalidInput`、`CapacityExhausted`、
`Download(FileGrantedDownloadError)`、`Save(FileSaveError)`。顶层容量错误来自整流程；
元数据／块／拼装和保存错误保持原阶段含义。内部 gate 拒绝会归还父准入，不自动等待或重试。

目标父目录当前路径／权限及已有条目仍由原保存任务检查，不新增早期文件系统探测。
因此目标冲突／漂移可能发生在块和 `f1` 已创建之后；更早失败不改目标，保留源前缀。
创建后不确定输出保留为证据。重复整调用不跳过已有块，不采用 `f1`／目标，可能尚未到
保存阶段就冲突；另选文件名不是续传，只有存活原拼装确认可供独立显式复制。

成功只确认此次本地所选副本，不是持续远端权限、远端当前 Principal 来源证明、公共发布
或浏览器送达。到期／撤权不抹掉已披露本地字节，最终保存不额外 GET 或查询授权。
网络期限仍逐请求，不是整文件硬期限，也不硬取消 OS／CPU 工作。

可选[实时观察](large-file-delivery-progress.md)沿用同一操作，只提供单调用最新阶段／块ACK快照。
全部暂存不代表保存，关闭不终止真实任务、不提供回执；旧方法不报告进度，行为保持不变。

可选的[显式取消控制](large-file-delivery-cancellation.md) 现包裹同一带观察核心，只请求在协作
轮询边界结束当前等待；仅丢弃句柄不取消，已派发文件任务仍可完成但不再返回后续ACK。

## 验证及剩余连接

另行[完整HTTPS检查](large-file-delivery-https.md)以原生一次性对端验证带控制路径，不是当前
Source授权或已部署局域网证明。

`file_delivery_tdd` 使用真实 loopback HTTP 和私有文件，私有生命周期用例观察实际任务
所有权，不只是睡眠 future。`scripts/test-file-delivery.sh` 独立选择准确可选 Source SQL／
网络用例，`fileDelivery` 仅核验源组合／库存；执行结果另录，不继承单步骤旧通过记录。

正常 AppState／Session 签发、公开 HTTP／SDK／UI、文件选择／共享可写目录策略、界面／持久进度／重启／
续传、物理配额、点对点及局域网部署仍独立。私有保存的额外副本及原 ACK 描述符成本不变，
不新增吞吐／RSS 或 64 GiB 真实载荷验收。版本／依赖、已发布 0.5.7 证据、**106 段历史
状态**、**44 份既有执行尾部**及分日期能力张量均保留。

0.5.8 标签之后未发布的[真实Source／HTTPS联验](large-file-source-https.md)增加同一条原生
授权／文件／router／TLS／保存测试链，覆盖派发前撤权及真实响应正文取消；不改生产监听、
策略或分日期张量评分。

## 实际执行记录 2026-10-13

新惰性入口针对真实 loopback HTTP、私有根和 **4 MiB + 17** 清单正常编译，再在 fixture
清理后行为失败：**0 通过／1 预期失败**，`Save(Unavailable)`，零警告；不是缺 API 或
初始化失败。Green 原样重跑容量 1 基线：**1 通过**、零警告，实际清单／两块／原拼装／
所选新文件均独立核验。重复基线不另增唯一验收用例。

| 本轮实际选择 | 结果及范围 |
|---|---|
| 交付默认目标 | **10 通过／2 条可选忽略**、零警告；基线 1、行为 2、失败 3、准入 2、取消 2。真实空／17 字节／跨块输出、独立 EOF／块／整文件 SHA／模式／inode／uid／0400／单 link，默认 4 名额跨 clone／旧入口共享及独立实例、嵌套元数据容量、纯 scope／token／同根优先级、首失败停止、晚目标冲突／漂移、重复块／拼装 Conflict、真实头／部分响应取消和前缀留存 |
| 新私有生命周期 | **4 通过**、零警告；真实元数据等待取消、块 0 真 EOF 后排队暂存、公开交付在原 ACK 丢弃后的实际排队保存取消，以及真实 Created 后取消至同 inode 0400／读回完成。旧任务活着时流程和 worker 仍满，保存阶段元数据／控制 gate 已空，另根健康正控随后完整交付 |
| 新显式 Source runner | **2 条独立 SQL 通过**、零警告；真实 Session 发布／接收授权 → 4 MiB + 17／空清单及块 → 原拼装 → 明确新文件，服务端快照不变且请求顺序准确；清单后、块委派前真实撤权为 200／404，空清单撤权为 404，都不创建目标 |
| 相邻显式 runner | **2 条旧清单客户端 SQL＋1 条既有清单下载 SQL＋1 条单块 SQL＋1 条既有 OpenSSL TLS 通过**、零警告；合计本轮 **6 条独立 SQL＋1 条既有 TLS**，不是新增清单／交付 TLS 或部署局域网验收 |
| 完整后端质量门 | `scripts/quality-gate.sh --backend-only`：**1043 Rust 通过／854 条可选忽略／93 组结果**，**326 条通用检查通过**，零失败／警告；新默认／私有及未变的 **15 条 compile-fail 契约**已计入，不能另加总 |

独立生产／生命周期、Source 测试和双语复核无剩余缺陷。SQL 执行前，新空文件撤权
delta 核验的准确 grant seed 从 1 改为 3；这是夹具修正，不是生产变化或行为 Red。
文档区分目标构造时目录核验与稍后路径健康；静态守卫要求实际预检调用存在，不命中
import，并区分生产 `run` 与测试 `run_with`。最终原生聚焦组合／库存 **30 条通过**，
不替代真实授权或磁盘写入。CI 在既有文件传输显式命令同行加入准确新 runner，
未豁免行数限制或增加数据库 fallback。

日志保留于 `/tmp/cyanrex-file-delivery-FOeVCxkH`：`red.log`、`green-baseline.log`、
`green-default.log`、`green-delivery-units.log`、`source-green.log`、
`source-manifest-adjacent.log`、`source-ordered-adjacent.log`、`source-chunk-tls-adjacent.log`、
`guards-final.log`、`full-backend.log`。独立 PostgreSQL 16 仅 Unix socket、无 TCP，
最终审计零多余 schema／连接；关闭后只删除经核对的本次私有 data／socket，释放
**40,748 KiB（约 40 MiB）**。测试数据可重建，执行日志保留。

无版本／依赖／schema 变化、commit、tag 或 push；不声称新前端／SDK／安全审计、保存
界面／普通目录策略、进度／续传／重启、磁盘满／fsync／断电验收、64 GiB 真实载荷、
吞吐／RSS 或 live／局域网部署验证。SQL／TLS 是单独可选执行，旧分日期证据没有刷新。
