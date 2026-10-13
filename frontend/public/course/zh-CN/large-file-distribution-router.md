# 文件分发端点的显式组合

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**、API **0.3.0**，**2026-10-13**。
本[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)跟进用一套受信配置组合既有
[清单GET](large-file-manifest-http.md)和[二进制块GET](large-file-chunk-http.md)，不再手工选择
两套可错配路由；不挂正常应用、不签发Session，也不把特权Engine变成隔离服务。

## 一个认证源与一个工作区

`FileDistributionHttpState::new(source, workspace, origin)` 只收一个 `DurableAuthSource` 和
一个 `SessionFileChunkReadWorkspace`。crate-private缓存接缝检查既有authority／scope关系，
克隆该工作区内准确download配置，不再分别提供publication／recipient。两个端点用同一所给
Source和规范平台Origin，不允许通过此API注入两份原router。

构造惰性，不连SQL、不打开新路径、不安装schema、不核验OID／存储健康、不认证调用者或证明
配置来源；请求仍走原当前Session／接收grant／Published检查及原source固定。缓存一致性既
不是字节许可，也不证明已初始化或已无特权隔离。

`with_capacities(source, workspace, origin, manifest_capacity, chunk_capacity)` 分别设置独立HTTP
名额，每类1–16、默认各4，按规范Origin→两个容量范围→缓存工作区匹配的顺序校验。独立
`FileDistributionHttpConfigError` 为静态 `InvalidOrigin`／`InvalidCapacity`／`InvalidWorkspace`，
不改旧错误枚举。state clone及据其分别build的router共享各自HTTP预算；另建state预算相加，
克隆所给workspace仍保留原字节预算。

## 两个准确端点与一个私有兜底

`build_file_distribution_router(state)` 合并不带fallback的内部路由表，复用原处理器，不合并
两份完整public router。仅注册 `/platform/v1/file-manifests`、`/platform/v1/file-chunks`；原
handler仍限定GET、六／七raw选择器、私有凭据、可选准确GET Origin和空正文。唯一fallback
返回私有404 `not_found`／`not_attempted`，不poll正文、不认证或调SQL；一次共享响应头层覆盖
成功／错误。不增加教学、登录、能力发现或控制端点。

旧独立builder保留原fallback／响应头行为。请求不能选择scope、owner、namespace、grant签发者
或存储根；协议1与公开OpenAPI不变。

## 独立响应生命周期

元数据HTTP名额持续至普通JSON正文最后帧／结束／错误／drop，保留JSON `Bytes`不保留该名额。
二进制字节直到最后一个保留owner的 `Bytes` clone或非空slice丢弃才释放HTTP与Source两名额，
即使正文已EOF／drop。原十秒首帧窗口、实际工作名额及取消含义不变；组合不增加全局配额、
timer、缓冲、后台任务、重试、清理或传输回执。

慢字节消费者不耗尽独立元数据HTTP预算；元数据仍只是当前资格观察，不授权后续字节。后续
Source准入重新检查撤权，却不能收回已批准快照；HTTP或字节预算满时可先返回容量错误。
配对端点不是持续授权。

## 验证与剩余连接

`file_distribution_http_tdd` 覆盖默认closed-Source路由、受信配置、准确选择器／凭据／正文、
独立／共享名额及取消／错误正文释放，不伪造binary owner或冒称SQL授权。既有显式交付Source
现将原请求全部送入同一个组合router；另条Source生命周期用例验证实际byte clone／slice、
元数据独立及后续撤权。显式临时数据库URL下执行 `bash scripts/test-file-delivery.sh` 的准确库存。

`fileDistributionHttp` 仅核验组合／库存。双语当前文档及课程副本描述该边，不改 **106历史状态块**、
**54既有执行尾**、**8受保护文件**、分日期能力张量或已发布0.5.7证据。UI／SDK挂载、安全Session
签发、无特权数据服务启动器、已部署局域网验收、续传／恢复及物理空间策略仍待完成；下方分层记录实际执行。

## 实际执行 — 2026-10-13

以下均在 `/home/chiharukiryu/cdev/cyanrex-lab` 原生执行，日志保留于
`/tmp/cyanrex-distribution-router-oiOxZGHU`；没有commit、tag、push或改版本。

- 编译后的基线
  `baseline::distribution_http_routes_manifest_and_chunk_gets_to_one_explicit_closed_source_with_private_errors`
  完成两条合法请求及fixture清理后，清单GET实际404对要求的closed-Source 503产生行为Red：
  **0通过、1失败**。组合实现后原断言不变，Green **1通过**；新默认目标共 **9通过、0忽略**。
  更早的getter类型别名编译错误在Red前修复，不冒充行为证据；默认测试是刻意关闭连接池，
  不冒称SQL授权成功。
- delivery runner的 **3条唯一真实Source用例通过**。前两条现在全部通过一个组合入口：
  大／空文件分别保存至private、observed-owned、controlled-owned选定输出，保留独立字节／
  哈希、准确GET库存、保存所有者／权限及Source／存储快照检查；另一条验证清单后实际撤权
  拒绝分块及已撤权空文件清单。实际客户端通过loopback TCP进入一个router，不手选两套配置。
- 第三条使用真实SQL／文件及进程内Axum：同一state clone分别build的router共享binary HTTP
  名额，原workspace仍保留Source字节预算。实际批准17字节帧在正文EOF／drop后，其clone和
  非空slice仍占两预算。元数据独立成功；未poll的JSON正文占自身HTTP名额，终帧后保留JSON
  Bytes不再占槽。快照仅有准确接收grant撤权差量；已批准slice仍可读，最后binary slice
  丢弃后两个端点均重新走当前授权并返回私有404，而非容量错误。
- 相邻原链路另跑 **8条清单HTTP SQL、6条binary HTTP SQL、2条granted manifest-client SQL**，
  加上delivery 3条为 **19条唯一SQL，全通过、零警告**。仅使用本轮自建临时PostgreSQL、
  私有Unix socket、禁用TCP，不使用用户／在线数据库。
- 原controlled HTTPS runner另行通过 **3条唯一TLS**：大／空／短文件完整选定输出、未知CA／
  错IP SAN／重定向拒绝、显式取消。使用临时OpenSSL peer，既不是TLS与Source SQL联验，
  也不证明已部署局域网、正常应用挂载或无特权隔离。
- **60条聚焦静态guard通过**，只证明组合／库存。清除数据库及注入环境变量后的原生
  `--backend-only` 检查共 **1097条Rust通过、98组成功结果、858条显式忽略、351条通用静态
  检查通过**，包含 **15条compile-fail doctest**，最终零警告／失败。完整检查没有暗中执行
  忽略项；上述19条SQL和3条TLS为独立显式运行。本轮没有跑frontend／SDK构建或新的依赖／安全审计。
- 冻结基线核验 **83个相关源码文件** 在允许组合接缝外不变，**106历史状态块、54既有执行尾、
  8受保护文件** 不变；分日期能力张量、公开OpenAPI、依赖／版本清单及已发布0.5.7证据均保留。
  共享旧测试辅助中的未用独立harness只在delivery测试目标标注允许，不增加生产警告屏蔽。
- 全部SQL完成后核对并停止本轮准确owned server，仅删除其 `data`／`socket` 两目录，释放
  **59268 KiB（约58 MiB）**，保留日志。这只是测试资源清理，不是新增分发清理或磁盘策略。
