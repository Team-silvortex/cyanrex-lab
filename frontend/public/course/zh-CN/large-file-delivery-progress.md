# 大文件实时交付进度观察

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**，API **0.3.0**，**2026-10-13**。
本[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)跟进为
[授权交付](large-file-delivery-client.md)增加有界本地观察，不增加持久进度或传输方式。
可信宿主可显示阶段及本调用已确认的暂存前缀，成功／失败仍须依据原调用结果处理。
快照不授予续传、重试、清理、文件访问或远端权限。

## 一次调用和一个最新快照

`FileDeliveryObserver::channel()` 返回仅移动的生产者与 `FileDeliveryProgress` 读取者。
宿主把生产者消费进 `FileChunkClient::fetch_granted_and_save_observed`，其他参数仍是旧交付
方法的Session token、grant／upload ID、期待版本和显式目标。返回仍为
`Result<SavedFile, FileGrantedSaveError>`；旧方法走同一核心但不观察，错误、准入与传输不变。

通道初始没有快照。`latest()` 复制当前 `Option<FileDeliverySnapshot>`；`changed().await`
返回复制的最新快照，生产者结束且未读变更已消费后返回 `None`。两方法均不泄露watch借用。
单个可覆盖槽只持计数与阶段，慢读可能跳过中间更新；不读／丢弃读取者不增加等待消费、队列、
回调、任务、监听器或额外流程名额。观察属于单次调用，不由客户端clone共享，也不能重复供
另一交付调用使用。

`FileDeliverySnapshot` 私有字段通过 `phase()`、`staged_chunks()`、`staged_bytes()`、
`total_bytes()`、`total_chunks()` 读取。`total_bytes` 是宿主期待版本长度，不是远端字节
观察；可选总块数只在完整准确清单校验后出现。快照不含文件ID、路径、名字、摘要、Session
token、origin、错误文本或payload；生产者／读取者及快照不是HTTP／SDK请求、序列化检查点或回执。

## 确认边界与阶段

| `FileDeliveryPhase` | 含义及计数边界 |
|---|---|
| `FetchingManifest` | 纯输入检查及原整流程准入通过；无清单／块ACK，暂存计数为零，总块数未知 |
| `StagingChunks` | 完整准确清单已校验；每个真实私有块暂存ACK后才推进，不依据HTTP头、正文流量或单独网络EOF |
| `Assembling` | 有序块ACK前缀完整，即将等待原拼装；全部暂存字节不等于所选文件保存 |
| `Saving` | 原私有整文件拼装ACK已返回，即将等待新目标保存；这个阶段不是保存完成 |
| `Complete` | 原所选保存已返回`SavedFile`；快照仍是元数据，不是该仅移动确认或持续权限 |

空文件仍须清单GET、实际空拼装和所选保存；0／0块或字节不代表 `Complete`。
计数只描述本次活跃等待者观察到的ACK，不是当前文件健康、永久完整性、持久上传状态或恢复位图。

## 取消与失败

错误或等待者丢弃会结束观察，不伪造 `Complete`。关闭后仍可读最后快照，但这不证明已派发
任务停止、失败、回滚或完成。真实排队／运行块、拼装及保存任务仍持原worker／flow与文件资源
至完成，却不持进度生产者，也不发后续观察。丢弃读取者不取消交付；原入口须另行丢弃调用／
停止等待，可选[显式取消控制](large-file-delivery-cancellation.md) 则在该边界保留类型化返回。
两种入口都不硬取消OS文件工作。

原首失败停止、独占条目、原inode证明和目标目录策略不变，保留的块、`f1`及不确定目标不被
采用。完整暂存前缀或通道关闭不能授权另一次调用、恢复、重试、覆盖或清理。报告不增加GET
或认证查询；每个后续块仍独立授权。

## 验证与剩余连接

`file_delivery_progress_tdd` 使用真实loopback HTTP和文件，私有生命周期用例另验证实际任务
所有权；`fileDeliveryProgress` 仅守卫组合／库存。既有显式交付Source用例独立验证当前授权与
真实所选输出，实际执行结果录于下文，不推导此前切片通过即代表本轮验收。

界面显示／文件选择、HTTP／SDK／在线挂载、Session签发、持久进度／重启／续传、物理配额、
点对点与已部署局域网性能仍独立。版本／依赖、已发布0.5.7证据、**106段历史状态**、
**48份既有执行尾部**、**8份受保护文件**及分日期能力张量保持不变。

## 实际执行 2026-10-13

惰性观察入口与真实 **4 MiB + 17** loopback交付、容量1、私有暂存和本人 **0755** 目标
一起正常编译，fixture清理后因 `Save(Unavailable)` 失败：**0通过／1预期失败**、零警告。
这是行为Red，不是缺API或环境失败。Green重跑准确基线：**1通过**、零警告，独立核验块／
整文件SHA、EOF、字节模式、inode／UID／0400／单link与最终 `Complete`；重复基线不增加独立用例。

| 本轮选择 | 实际结果与范围 |
|---|---|
| 新默认目标 | **10通过**、无ignored、零警告：基线1、真实响应边界2、预检1、失败3、空文件1、读取者1、独立通道1。真实清单头／部分JSON仍总块数未知，后续块头／部分字节只保留前一实际ACK；未poll／坏输入／满flow无事件或额外GET，纯scope／token／同源拒绝优先于容量。清单／块首错即停，全部暂存但整摘要坏停在`Assembling`，目标Conflict停在`Saving`，空0／0只在实际保存后完成。慢读／丢弃读取者不阻止输出或占名额，新调用不采用旧前缀 |
| 新私有单元 | **5通过**、零警告：四个真实公开生命周期覆盖已hold清单取消、块0真实EOF后排队暂存、空文件排队拼装、Created阶段保存取消。观察关闭后原任务仍持flow／worker，孤儿任务可完成实际0400／摘要／读回却不再报告ACK或`Complete`。第五个只检验复制／覆盖watch值、最后未读值及读取者drop，是类型／通道行为，不是物理保存证据 |
| 扩展交付Source | **既有2条SQL通过**、零警告。大文件与空文件分别经旧私有0700对照、带观察的user0755／user0750，形成六对独立源／目标，核验实际输出、准确HTTP顺序、最终计数／关闭及完整服务端快照不变。真实清单后块撤权为200／404，零ACK停在`StagingChunks`；已撤空清单总量0／总块数未知停在`FetchingManifest`，两者均不创建拼装／目标 |
| 相邻显式执行 | **4条SQL＋既有OpenSSL TLS1条通过**、零警告：清单客户端2、已供清单下载1、单块客户端1。连同扩展交付为**本轮6条独立SQL＋既有TLS1条**，不是新观察交付TLS或已部署局域网验收 |
| 完整后端质量门 | `scripts/quality-gate.sh --backend-only`：**1073条Rust通过／854条opt-in ignored／95组结果**、**336条公共检查通过**、零失败／警告。新默认／私有用例与不变的**15条compile-fail契约**已包含其中，不是额外总数 |

独立生产／生命周期、默认fixture、Source及当前文档复核未发现阻塞缺陷。Source一处断言
已收窄：未连接观察器的空值仅作旧API对照，不单独证明旧核心不发事件；实际旧`None`组合
与关闭视图隔离另核验。原生聚焦组合／库存守卫**40通过**、格式化后核心选择**15通过**，
静态守卫不是真实IO或授权证据。既有SQL runner库存与600行CI工作流不变。

日志保留于 `/tmp/cyanrex-file-delivery-progress-8dwqZdaJ`：`red.log`、`green-baseline.log`、
`green-defaults.log`、`green-private.log`、`sql-and-tls.log`、`focused-guards.log`、
`guards-after-fmt.log`、`common-guards.log`、`full-backend.log`，另有数据库审计／停止／清理记录。
隔离PostgreSQL16仅用Unix socket、无TCP，最终审计额外schema／连接均为0；停止后仅删除
校验过的本人data／socket目录，释放**40,612 KiB（约40 MiB）**。测试数据可重建，日志保留。

本轮未改版本／依赖／schema，未commit／tag／push。不代表界面显示／文件选择、前端／SDK／
安全审计、公开Session／HTTP／SDK入口、持久进度／恢复／续传、暂停、OS工作硬取消、磁盘满／
fsync／断电验收、64 GiB真实文件或吞吐／RSS／已部署局域网结果；显式opt-in SQL／TLS不属于
默认门。**106段历史状态／48份既有执行尾部／8份受保护文件**逐字节保持不变，包括分日期
张量及已发布证据。
