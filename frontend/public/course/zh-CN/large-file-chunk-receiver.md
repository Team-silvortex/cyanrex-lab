# HTTP 分块校验接收与私有暂存

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.6**，API **0.3.0**，**2026-10-12**。
这是处理**已提供响应**的独立客户端原语，不是主动 HTTP 客户端、局域网部署或下载界面。
遵循[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，衔接未挂载的
[二进制分块适配](large-file-chunk-http.md)。

## 组合与结果

`services::file_chunk_receiver::FileChunkReceiver::new(staging)` 接收已经打开的可信 Unix
`ChunkStaging`，仅缓存工作区并建立本地控制预算；不创建目录、改权限、查询 SQL 或主动联网。

`receive_and_stage(&manifest, index, response)` 接收 `http::Response<B>`，其中
`B: http_body::Body<Data = axum::body::Bytes> + Send + 'static`，不要求 `Unpin`。调用者提供
准确类型清单、块编号及私有目的地。响应元数据不能选择工作区、文件根、输出名称、Source
隐藏密钥或授权主体。链路如下：

1. 匹配缓存工作区及清单选定块，验证响应头。
2. 接收器控制名额满则立即拒绝；准入后收集私有有界缓冲。
3. 核验准确长度、真正正文 EOF 及单块 SHA-256，之后才允许文件派发。
4. 将已核验缓冲及其名额移入原 `ChunkStaging::stage_chunk` 工作。
5. 仅在**本次调用**的本地暂存成功后返回不透明 `ReceivedFileChunk` 元数据。

结果仅暴露 `file()`、`chunk_index()`、`byte_length()` 和 `chunk_sha256()`，没有公开构造器、
Clone／序列化、字节、路径或隐藏密钥访问器。它确认一次本地暂存，不代表持续路径健康、当前
远端权限、远端回执、整下载完成、重建、续传凭证或持久确认日志。

## 严格响应头

仅接收状态 **200**。计入重复头的全部条目，最多 **64 项**、名称和值合计 **8192 字节**。
忽略有界未知头，但它们不具有路径或权限意义。以下十二个字段必须各出现一次并准确匹配：

| 响应头 | 要求值 |
|---|---|
| `content-length` | 选定块长度的规范十进制，不是整文件长度 |
| `content-type` | `application/octet-stream` |
| `content-disposition` | `attachment; filename="chunk.bin"` |
| `x-cyanrex-file-transfer-version` | `1` |
| `x-cyanrex-file-id` | 预期准确文件 UUID |
| `x-cyanrex-file-version-id` | 预期准确版本 UUID |
| `x-cyanrex-chunk-index` | 选定块编号的规范十进制 |
| `x-cyanrex-chunk-sha256` | 选定块准确摘要 |
| `x-cyanrex-file-sha256` | 清单**声明**的预期整文件摘要 |
| `cache-control` | `no-store` |
| `pragma` | `no-cache` |
| `x-content-type-options` | `nosniff` |

任何 `content-encoding`、`content-range`、`transfer-encoding`、`trailer`、`set-cookie`、
`location`、`etag`、`last-modified` 或 `retry-after` 都拒绝。不合并重复头，不归一化十进制／
头别名，不解压、不采纳重定向、不获取 cookie、不拼接 Range。整文件头仅与清单比较，单块
调用**没有新鲜重算整文件摘要**。

工作区／块编号错误为 `InvalidInput`，响应／帧／摘要错误为 `InvalidResponse`。
只有工作区／块编号与响应头检查先于名额及正文轮询；帧／实际摘要核验在准入和收集后、
暂存前进行。接收名额满返回 `CapacityExhausted`，不轮询正文、不碰文件。
私有缓冲预留失败为 `Unavailable`；文件错误保留 `Staging(ChunkStagingError)`，不能当作
成功或可重试回执。接收器不返回或记录远端原始错误字符串、响应头。

## 字节、EOF 与时间

私有自有缓冲受选定块长度限制，至多 **4 MiB**，准入后才分配。超长帧在复制／哈希**之前**
拒绝。复制／哈希按 **64 KiB** 小段合作让出执行；传入帧自身的分配不在此本地配额内。

逐帧轮询到真正 `None` EOF；`content-length`、`size_hint()` 和 `is_end_stream()` 都不能证明
完成。短正文、多余数据、正文错误和 trailer——包括**全部预期字节之后**的错误／trailer——
均失败且不创建文件。空数据帧不刷新 idle，并主动让出执行，避免一直 ready 饿死计时器。

默认 **四个控制名额**、**空闲五秒**、**总计六十秒**。`with_limits` 支持 1–16 名额、idle
1–60 秒，total 不小于 idle 且不超过 300 秒。只有接收的非空字节刷新 idle；total 不刷新。
显式单调时钟检查与有界帧等待覆盖到 EOF／摘要，不覆盖随后磁盘工作。可信正文若在同步
`poll_frame` 内阻塞执行器，这个超时无法强制杀死它。

克隆共享同一接收预算，独立接收器实例的预算可叠加。既有暂存另有克隆共享阻塞工作准入。
这不是全局网络、代理、传入正文、进程内存、物理磁盘或多实例配额。

## 既有文件边界与取消

复用 [Unix 暂存](large-file-staging.md)，不用 Source 上传命名空间。本地条目继续绑定
`v1-<SHA256(full typed manifest)>-index`；不从响应读取 Source `u1` 密钥。可信提供根为 0700；
独占／禁止跟随创建。已有普通文件、符号链接、硬链接、目录、FIFO 都冲突，不覆盖、不收编、
不删除、不自动第二次尝试。

既有工作再次核验字节及 EOF，封为 0400，同步文件与目录，读回原文件 inode 并核对原根。
创建前不可用与创建后的 `Unconfirmed` 不同；后者可能留下部分或完整输出。不自动清理含糊
结果，不宣称回滚。

派发前取消收集会释放缓冲／接收名额，不创建条目。派发后，私有 reader 将**已核验缓冲及接收
名额一起持有在真实排队／运行阻塞任务内**，取消 async 等待者不能提前释放它们或暂存工作位。
名额在缓冲之后释放。已完成但丢弃／未认领的元数据结果不持有字节；重试既有输出不能补造
原完成确认。

## 明确缺失

另行[受控客户端](large-file-chunk-client.md)现通过窄私有名额交接提供固定 Origin 显式请求；
不把本公开“已提供响应”API 变成请求器，不改其验证／准入顺序。

不创建 reqwest 请求／客户端，不增加凭据、Session 查询、Source 策略、Schema、SQL、应用
监听、OpenAPI／SDK／浏览器／桌面挂载或用户选择路径。后续调用者须独立管理远端 Origin、
TLS、重定向／代理、凭据、取消及请求生命周期。转入已提供的 reqwest 响应必须保留帧错误／
trailer，不能用只取数据的便利迭代抹掉 EOF 证据。
核验针对已提供头／帧，不追溯上游原始 wire；上游若已解压或改写头，本层无法重建该历史。
后续调用者若需要保证原始 wire 契约，须另行禁止未经请求的解码／变换。

整文件拼装／校验、最终目的地原子保存、保存对话框、多接收者调度、续传／清理日志、当前权限
复查及节点发现仍独立。测试通过不会开启局域网分发或改变部署。

## 验证

`file_chunk_receiver_tdd` 将默认恶意正文／私有文件测试与 ignored 真实 Source／SQL 桥接分开。
`scripts/test-file-chunk-receiver.sh` 在显式临时数据库 URL 下按准确名称执行全部已接 ignored
测试；`fileChunkReceiver` 守卫只核对组合及库存。私有单元验证真实排队／运行所有权，不只模拟
reader 存活。实际执行证据单列于下方，库存不等于执行证据。

旧分日期 project-status 块、详细执行尾部及能力张量不改写。版本仍 0.5.6／API 0.3.0；本轮
不授权提交、推送或在线迁移。

## 实际执行 · 2026-10-12

冻结的惰性骨架在真实已授权 200／17 字节 Source HTTP 响应下成功编译；基线为
**0 通过／1 个预期失败**、`InvalidResponse`、**零 warning**。Red 是没有真实客户端暂存，
不是缺符号或文件系统被拒绝。初次运行的旧 helper 重导出未使用 warning 通过**新夹具**消费
该导出消除，同一行为 Red 已干净复跑。扩展矩阵第一次 Green 编译发现新测试 helper 未限定
引用，改成 `fixture::observe_chunk` 修复测试编译；这不是行为 Red。随后干净 Green 为
**1 通过／0 失败**、零 warning，核对了真实独立根文件、独立重开、长度／摘要／权限／单硬链接
及服务端完整快照不变。

| 新鲜选择 | 实际结果及边界 |
|---|---|
| 新接收器默认 target | **14 通过**、**3 ignored**、零 warning；准确头、预算、真正 EOF／!Unpin、恶意／尾部帧、idle／total／ready-empty、派发前取消、私有根漂移及既有目标冲突 |
| 私有接收器工作单元 | **2 通过**、零 warning；真实排队／运行任务跨取消保有接收／暂存准入，完成原字节／inode／0400、拒绝重复保存并可接健康新版本 |
| 公开结果 compile-fail 契约 | **3 通过**、零 warning；真实 E0451 私有构造、E0599 无 Clone、E0277 无 Serialize，不是缺 import |
| 新显式真实 Source／SQL runner | **3 通过**、零 warning；授权 17 字节、4 MiB + 17 两块、拒绝／损坏响应、客户端冲突及服务端额度恢复 |
| 相邻真实 SQL runner | **6 HTTP + 10 Source reader 通过**、零 warning；原接收授权／固定、损坏、字节所有者存活、到期、等待及取消 |
| 完整后端门禁 | `scripts/quality-gate.sh --backend-only`：**903 Rust 通过**、**839 ignored**、**86 结果组**、**291 公共检查通过**、零 warning／失败 |

上述 **19 个唯一新鲜 SQL 用例**在默认门禁 ignored 清单之外显式执行。新增 14 默认、2 工作单元
及 3 compile-fail 已含于 903，不能再相加；库存不是 SQL 执行。没有将此前 117 用例的运行改标
为本轮证据，没有新跑前端构建、SDK 套件、安全审计、局域网／TLS 部署、整文件保存／续传、
真实磁盘满／fsync／断电、RSS／吞吐基准或点对点测试。

全部 Cargo、格式与 SQL 在编辑冻结后由 root 串行执行。原生私有文件／Unix socket 执行保留
生产所有者／父目录检查，未为沙箱映射放宽。独占 PostgreSQL 仅监听自己的 Unix socket，无 TCP；
最终审计为**零夹具 Schema、零其他数据库客户端**。停止后只删除已验证的 `data`／`socket`，
释放 **103184 KiB（约 101 MiB）**可重建测试数据；日志保留在
`/tmp/cyanrex-file-chunk-receiver-ElTANZ7D`，没有删除部署或仓库输出。**106 个旧状态块**、
**30 个旧详细执行尾部**及分日期能力张量逐字节不变。中英当前说明及生成课程副本已同步，
证据落盘后的 `scripts/quality-gate.sh --format-only` 通过：**291 公共检查**、零 warning／失败，
课程／版本／OpenAPI 漂移、工具及 Rust 格式检查均通过。尾注课程同步后再复跑同一门禁收尾。
