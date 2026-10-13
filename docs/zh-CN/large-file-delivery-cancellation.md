# 大文件交付等待的协作式取消

源码版本 **0.5.8**（**2026-10-13**）已收录此准备层；下方分日期的 0.5.7／未发布实施与
验证保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.7**，API **0.3.0**，**2026-10-13**。
本[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)跟进为
[带观察的交付](large-file-delivery-progress.md)增加显式本地取消请求。请求被选定时停止本次
等待与续跑，不终止已派发文件或远端工作；Cancelled不代表回滚、无输出、撤权、安全删除或重试许可。

## 一个请求句柄和一个被消费的令牌

`FileDeliveryCancelHandle::channel()` 返回仅移动句柄和 `FileDeliveryCancellation`。
宿主把取消令牌与原 `FileDeliveryObserver` 消费进
`FileChunkClient::fetch_granted_and_save_controlled`，其他参数仍是旧方法的Session token、
grant／upload ID、期待版本及显式目标。返回 `SavedFile` 或独立 `FileControlledSaveError`：
`Cancelled`／`Delivery(FileGrantedSaveError)`；旧方法、错误、传输配置和进度阶段不变。

`cancel(self)` 只发送一次unit请求并消费句柄，不是停止确认、文件任务abort、当前grant撤销或
远端取消命令；调用结束后再调用也无害。**未调用cancel而丢弃句柄不会取消交付**，无信号的
通道关闭只禁用取消分支，不被当作请求或忙循环。两能力不暴露原通道、clone、凭据、路径或序列化。

## 输入优先级与协作式选择

原纯scope／token／缓存同源目标检查优先，已请求取消或flow满时坏输入仍为
`Delivery(InvalidInput)`。合法输入的预先取消先于内核心poll，无flow准入、进度事件、HTTP或
文件派发；否则原带观察核心仍拥有同一整流程及独立预算，不双重准入、不新增worker。

外层等待在同次poll两分支都ready时优先选择显式取消，这是**协作式**而非即时派发屏障。
一次同步poll可能跨过已ready的ACK或启动下一工作，之后才观察到并发发送的取消。不能抢占
同步摘要、文件调用或已准入远端工作；正常成功／错误可能赢得竞争，以返回结果而非按钮／send
为准。已选定结果和已返回确认不被迟到取消撤回。

## 保留任务与进度

取消胜出时丢弃当前活跃调度future并返回 `Cancelled`；选择之后不启动后续步骤，但已派发
暂存／拼装／保存任务仍持原flow、worker、缓冲／证明及目标至完成。取消令牌与进度生产者均不
进入阻塞任务，控制器本身不新增任务、回调、重试、清理、凭据或网络请求。

观察关闭并保留等待者最后快照，排队工作可完成却不推进该前缀或报告 `Complete`。取消的保存
可能留下部分或完整已核验0400文件，即使没有返回 `SavedFile`。空0／0、全部暂存百分比或
通道关闭都不是保存回执或任务终止证明；只丢弃进度读取者不取消调用，调用者drop仍保留旧等待取消语义。

既有块、私有 `f1` 和不确定输出是证据，不是可恢复尝试或覆盖／采用／删除许可。每个后续
准入块仍独立检查当前源授权；本地取消不改grant，也不收回已披露字节。

## 验证与剩余连接

另行[完整HTTPS检查](large-file-delivery-https.md)覆盖实际TLS等待和所选保存，不替代真实磁盘
任务或当前Source授权测试。

`file_delivery_cancel_tdd` 使用真实loopback HTTP及文件，私有生命周期区分实际取消结果与
任务完成；`fileDeliveryCancel` 仅核验组合／库存。既有显式Source另核验授权、服务端快照
不变和所选输出，实际结果记录如下。

界面取消按钮、正常应用／HTTP／SDK挂载、Session签发、持久进度、暂停／重启／续传、物理
配额、任务kill／回滚及已部署局域网性能仍独立。版本／依赖、已发布0.5.7证据、**106段历史
状态**、**50份既有执行尾部**、**8份受保护文件**及分日期能力张量保持不变。

## 实际执行 — 2026-10-13

以下是本地原生执行，不是UI／部署验收或新版本。源码仍为 **0.5.7**、API **0.3.0**；
依赖、schema、OpenAPI、张量评分、commit、tag、push均未改变。

| 检查 | 实际结果与边界 |
| --- | --- |
| Red → Green | 真实容量1、4MiB+17字节基线先编译成功，在夹具清理后准确因空壳返回 `Delivery(Save(Unavailable))` 而 **0通过／1预期失败**，不是缺API或设置失败。同一唯一句柄、真实清单／两块／拼装／自有0755保存基线随后 **通过**；独立文件、EOF、块／整SHA、inode／UID／0400／单link、父目录策略和精确HTTP序列均检查 |
| 新默认目标 | **10通过**、无ignored、零警告：基线1、预检2、句柄drop1、显式HTTP等待取消2、空／短成功1、旧错包装1、独立pair1、读取者生命周期1。预请求取消无事件／HTTP／文件，坏scope／token／同源目标仍优先。真实hold头／部分JSON及后续块部分字节返回 `Cancelled`，仅保留先前ACK前缀；迟到请求不撤回成功／错误，新调用对保留字节Conflict，不采用旧尝试 |
| 新私有单元 | 按准确清单逐条执行 **5通过**、零警告。四条实际公开入口request取消覆盖hold清单、真实块EOF后排队暂存、空文件排队拼装、真实Created保存；任务仍持flow／worker，释放后孤儿任务完成真实校验字节／0400，却无后续调度或 `Complete`。第五条仅检验通道关闭／请求及两ready分支优先级，不是文件系统证据 |
| 并行测试修正 | 首次全量并行暴露新测试的错误假设：flow归还不等于worker同刻归还，库组 **379通过／1失败／114忽略**。真实暂存IO已结束后LeasedCursor先释放flow，外层任务稍后释放worker。仅修正新测试：有界实际只读校验仅遇 `CapacityExhausted` 让出执行权再等待，其它错误／超时严格失败；保留hold期间断言，生产逻辑不变。五条准确单元和全量并行门随后通过；初次测试未使用导入也已删除，没有压制警告 |
| 本轮可选授权／TLS | **6条独立SQL＋1条既有TLS通过**、零警告：交付2、授权清单2、整下载1、块客户端1，另实际精确IP-SAN／受信CA暂存。交付六组大／空输出分别走旧private0700、observed自有0755、controlled自有0750入口，独立输出与服务端快照严格检查。新controlled入口真实清单后撤权及已撤权空清单仍返回准确嵌套旧错，并覆盖不用句柄drop／迟到请求。这些Source用例证明授权与包装，不是显式取消SQL工作或整controlled TLS验收 |
| 组合守卫 | 专项 **45通过**；新增五条取消检查仅证明静态组合／库存，不重复计为原生行为证据 |
| 完整后端门 | 原生 `--backend-only`：**1,088项Rust通过／854条可选忽略**、**96个结果组**、**341项common通过**，零警告／失败；既有 **15条compile-fail** 原样通过。上述SQL／TLS单独实际执行，不能从ignored行推断 |
| 保留／清理 | 既有 **106状态块／50执行尾／8受保护哈希**逐字节不变；课程副本、版本／OpenAPI／工具、行长／格式检查通过，CI仍600行、英文状态仍1,999行。专用PostgreSQL16只用准确socket／55457端口，无TCP监听、多余schema或其它客户端；已停止，仅删除核验自有 `data`／`socket`：**40,812KiB≈40MiB**，日志保留 |

证据目录：`/tmp/cyanrex-file-delivery-cancel-eov87TS9`，保留 `red.log`、`green.log`、
`defaults.log`、`private.log`、`sql.log`、`focused-guards.log`、`full-backend.log`、
`post-evidence.log`，以及初次导入警告／并行失败日志；临时数据库不保留。
写完记录后另重跑common／格式、课程同步、历史保护和HEAD／origin／版本tag身份检查，
不将其重计为后端或SQL验收。

本轮没有测前端／SDK套件、部署局域网、64GiB／RSS／吞吐、物理配额、突然断电或OS任务硬取消。
UI保存／取消控件与正常应用／Session挂载仍未接入；返回取消不是文件不存在的证明，也不授权
重试、覆盖、采用或删除不确定输出。
