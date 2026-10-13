# 带取消控制的大文件交付 HTTPS 验证

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 外部验收**，源码 **0.5.7**、API **0.3.0**，**2026-10-13**。
本[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)跟进通过真实OpenSSL loopback TLS
验证既有[带取消控制交付](large-file-delivery-cancellation.md)，不再从旧单块TLS用例推断整条链。
不增加生产API、信任策略、权限、依赖、监听或应用接线。

## 完整传输与准确对端身份

显式 `file_delivery_tls_tdd` 使用一次性证书和实际HTTPS对端。正向覆盖4MiB+17字节、17字节和
空文件的准确清单GET、顺序块、原私有拼装及宿主明确选择保存；容量1、自有目标目录，独立
检查实际字节／EOF／摘要／inode／UID／模式／link及原父目录策略。空交付仍须清单GET，不发块请求。

测试CA只经 `FileChunkClientConfig::trusted_root_pem` 显式信任，默认传输仍为HTTPS。不受信CA
或错误IP SAN须在对端收到解密HTTP之前失败，不披露选择器或专用Session Cookie；信任CA不
绕过准确对端身份。拒绝跳转不能变成HTTP降级、第二Origin请求或凭据转发。原固定Origin、
无代理／重试／跳转／解码及私有错误策略均不变。

对端提供合成清单／块响应，TLS证书只证明传输身份，不证明当前Session成员、接收grant或
不可变发布；真实Source授权／撤权有独立分日期SQL证据，本轮不替代它或接通在线服务。

## 取消与独立输出

真实HTTPS清单或后续块被hold时，显式请求仅在取消分支被选定后返回 `Cancelled`；最后快照
只保留实际ACK前缀并关闭，不由已取消续跑启动拼装／保存或报告 `Complete`。旧暂存字节是
证据，不被新调用采用成恢复尝试；独立健康夹具仍可用。

未请求而丢句柄、迟到请求、观察关闭都保留原含义。取消仍为协作式，不抢占同步poll、不kill
OS任务、不撤回已选定结果、不回滚，也不授权重试／覆盖／删除。[取消契约](large-file-delivery-cancellation.md)
保留独立真实排队／运行文件任务测试；hold TLS响应不是阻塞磁盘任务。

## 显式原生运行与清理

原生OpenSSL可用时执行 `bash scripts/test-file-delivery-tls.sh`，逐条列出并运行三条准确ignored
用例；默认Cargo不冒称执行它们。不需数据库URL、部署私钥或系统信任库改动。证书／key均
位于夹具自有0700临时父目录；包含失败／取消时，先释放hold并收尾进程、pipe、线程，再删除
该目录，不把私钥写入仓库或日志。

`fileDeliveryTls` 仅守卫准确库存、runner／CI接线、夹具所有权与有界等待、原生产策略及双语
范围，是静态组合而非TLS行为证据。CI在既有大文件runner旁显式运行TLS验收，不挂载路由。

UI保存／取消、Session签发、独立无特权数据服务、已部署局域网验收、续传／恢复、物理磁盘配额及
吞吐／RSS仍未接通。版本、发行证据、**106历史状态块**、**52份既有执行尾**、**8受保护文件**
及分日期能力张量不变；实际执行结果如下。

0.5.8 标签之后未发布的[真实Source／HTTPS联验](large-file-source-https.md)增加同一条原生
授权／文件／router／TLS／保存测试链，覆盖派发前撤权及真实响应正文取消；不改生产监听、
策略或分日期张量评分。

## 实际执行 — 2026-10-13

这是既有HTTPS实现的新外部覆盖，不是新增传输功能或人为构造的行为Red。源码仍 **0.5.7**、
API仍 **0.3.0**；没有commit、tag或push。

| 检查 | 实际原生结果 |
| --- | --- |
| 准确OpenSSL runner | **3条新用例通过**、零警告。完整交付覆盖两块4MiB+17字节、只请求元数据的空交付和17字节，controlled容量／worker1，自有0755或private0700目标，准确原GET／凭据；独立原块／f1／所选文件EOF、块／整SHA、pattern、0400、UID／GID、单link、新inode及原父身份／策略不变。句柄drop不取消，迟到请求不撤回保存结果 |
| 对端拒绝 | 不受信CA与受信CA但错误 `127.0.0.2` SAN准确返回旧包装清单 `Unavailable`，**独立计数解密HTTP字节为0**、无接收请求／文件，最后停在已关闭 `FetchingManifest`。真实HTTPS302仍为清单 `InvalidResponse`，独立HTTP毒目标 **运行中及最终drain后请求都为0**；拒绝响应头不必等旧对端完整正文写完 |
| 显式TLS取消 | 实际部分清单、后续块头、后续块部分正文hold后，明确请求返回 `Cancelled`。正文hold前真实pipe已接受前8字节响应明文，不夸称客户端已消费8字节；仅原实际chunk ACK前缀保留，另经SHA／EOF核验，无f1／目标或后续 `Complete`。新受信对端复用原staging／target准确chunk0 `Conflict`，不采用旧尝试；其独立空spyroot不冒称旧前缀。独立健康对端／根／目标仍可保存 |
| 并行及邻接 | 新 **3条TLS用例一起并行通过**；另 **1条既有单块TLS通过**。合计 **4条独立TLS用例**，不是7条；重复执行不增加覆盖计数，也不是吞吐基准 |
| 夹具／守卫修正 | 初次夹具编译有两处 `AsRawFd` 引用bound错误，现以 `&F` 借用而不移动原pipe；静态守卫的一处格式化字符串匹配改为容忍空白的断言，保留两次毒目标检查。这些不是生产行为失败或行为Red。原生执行及最终专项 **50项守卫通过**，零警告／失败 |
| 完整后端门 | 原生 `--backend-only`：**1,088项Rust通过／857条可选忽略**、**97个结果组**、**346项common通过**，零警告／失败；既有 **15条compile-fail** 原样通过。三条新增ignored TLS已在上方显式实际运行，不能从默认门推断 |
| 保留与清理 | **81份相关源码哈希**不变，既有 **106历史状态块／52执行尾／8受保护版本发行张量文件**也不变；CI仍600行、英文项目状态1,999行。原生夹具先停止并reap OpenSSL、join自有线程，再删准确临时key／根目录；最终目录及进程清单 **新增夹具残留为0**。保留日志，不保留密钥 |

日志目录：`/tmp/cyanrex-delivery-https-a7QTZfaM`，保留 `native-tls.log`、`parallel-tls.log`、
`adjacent-tls.log`、`focused-guards.log`、`full-backend.log`、`post-evidence.log`及初次编译／
格式守卫记录。写完后另重跑common／格式、课程副本、历史／源码哈希与Git身份检查，不重计后端／TLS验收。

本轮没有验收SQL、新前端／SDK套件、已部署局域网／系统证书、Session签发、正常应用路由、
无特权服务隔离、64GiB／RSS／吞吐、磁盘满／断电或任务硬取消；合成TLS对端不是当前获授权的发布者或接收者。
