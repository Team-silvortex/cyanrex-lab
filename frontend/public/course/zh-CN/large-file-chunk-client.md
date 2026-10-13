# 受控单块 HTTP 客户端

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布 Unix 准备层**，源码 **0.5.6**，API **0.3.0**，**2026-10-12**。
显式原生客户端从可信固定 Origin 获取一个准确分块，再复用[接收器](large-file-chunk-receiver.md)。
这是[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)的下一段链路，不是已挂载分发
服务、整文件下载器、续传机制或桌面配对协议。

## 可信配置

0.5.8 收录的同一客户端另提供[顺序私有下载](large-file-download-client.md)
`fetch_and_assemble`，独立流程名额进入实际保存／拼装任务；原单块 API、传输策略及逐块
期限不变。本文分日期的单块执行记录不是新整流程验收结果。

`services::file_chunk_client::FileChunkClient::new_https(origin, staging)` 接收已经打开的可信
`ChunkStaging` 和规范 Origin，不是请求 URL。`new_loopback` 显式选择明文，仅支持规范
**字面** IPv4 环回或 `::1`；拒绝 `localhost`／DNS、数字别名、userinfo、路径、query、fragment
及非环回 HTTP。Origin 至多 2048 字节，须与解析后的 Origin 序列化准确一致，不自动归一化或跳转。

`with_config` 接收 `FileChunkClientConfig`，其中 `FileChunkTransport::{Https, LoopbackHttp}`、
默认容量 **4**、响应头期限 **10 秒**、接收器 idle **5 秒**、total **60 秒**。容量 1–16，
响应头期限 1–60 秒并支持小数；接收器沿用既有 idle／total 边界。可选 `trusted_root_pem`
只用于 HTTPS，为非空、有界 **64 KiB** PEM bundle；只提取解析证书作为附加 trust roots，不配置
client identity。宿主不得提供私钥。至少要求一个可用证书，不代表严格只证书／规范 PEM 语法，
也不保证拒绝全部混合文本。
默认根加宿主明确提供的根仍正常验证证书及 hostname／IP SAN；环回 HTTP 拒绝提供证书 bundle。

客户端私有且自有，不接外部 reqwest client／builder、任意 URL／请求头／代理／cookie jar 或
危险证书策略。构造不发请求、不创建或改暂存根权限；配置不是公共 wire 载荷或可序列化秘密。
克隆共享原配置及预算，宿主替换配置不会撤销存活克隆／请求。配置 HTTPS hostname 不代表
DNS／地址固定、SSRF 防火墙、当前服务端账号权限或访问不可信网络的许可。

## 准确原生 GET

`fetch_and_stage(token, grant_id, upload_id, &manifest, index)` 先校验暂存工作区、选中块及
规范非 nil 的 36 字节 Session UUID；不匹配在准入／联网前失败。调用者已经提供准确类型清单，
本客户端不自行获取或认证该清单。只有远端当前接收者命令授予字节权限。

目标固定 `/platform/v1/file-chunks`，使用既有七个原始规范选择器：`grant_id`、`upload_id`、
`file_id`、`version_id`、`byte_length`、`sha256`、`chunk_index`。token、owner、命名空间、隐藏
密钥、路径或模式不进 query。请求无正文；单个敏感 `Cookie: cyanrex_platform_session=<token>`、
标记 `x-cyanrex-platform-request: 1`、octet-stream Accept、identity Accept-Encoding。
token 逐次提供，不存默认配置、不返回、不记录、不进入 Debug／错误字符串。

原生 GET 按[既有 HTTP 策略](large-file-chunk-http.md)省略 Origin，不把传输 Origin 冒充另行
配置的浏览器／UI Origin；不生成 Authorization、旧 cookie、Range、条件头、Referer 或 Content-Type。

锁定 reqwest **0.12.28** 客户端显式禁用协议自动重试（`retry(reqwest::retry::never())`）、
跳转、环境代理和 gzip／brotli／deflate／zstd 解码，使用 HTTP/1、禁用空闲连接复用、不开启
cookie 存储。`Accept-Encoding: identity` 只是请求，不是信任对端；接收器仍拒绝不允许的响应
变换。服务端状态／cookie 不构成自动重试许可。

## 期限、准入与保存结果

工作区／编号／token 先于立即拒绝的请求准入。容量满返回 `CapacityExhausted`，不开请求。
请求期限覆盖连接／TLS／发送至响应头，并核对返回后的单调时钟；不把 reqwest 全正文
`.timeout()` 当作仅响应头期限。取得头后，接收器独立用 idle 与固定 total 覆盖完整帧、消息 EOF
和单块 SHA-256，文件派发仍在全部核验之后。

转换为 `let response: http::Response<reqwest::Body> = response.into();`，保留库暴露的帧与错误，
不用只取数据的 `.chunk()`、`.bytes()`、`.bytes_stream()`。核验对象是传输层暴露的 HTTP 头／
消息帧；HTTP/1 Content-Length 定界**不能**证明原始 TCP 连接 EOF 或检测全部 socket 后缀，
另一消息／多余 wire 字节不会自动变成额外正文帧。本客户端不是原始 wire 解析器或跨传输定界证明。

整调用请求名额通过窄 **crate-private** 接收路径与已校验缓冲、接收名额一同进入私有
`LeasedCursor`；真实排队／运行既有保存任务跨取消持有全部资源，字节先于名额释放。文件派发
后名额不只留在 async 外壳。公开“已提供响应”行为及其先检查头再准入顺序不变。

文件派发前取消／超时释放本地准入，不把未经校验的前缀写盘；不承诺强杀 DNS、OS／网络工作、
远端命令或已经派发的文件工作。原旧式 `v1` 独占／禁止跟随、0400 同步／原 inode 读回、Conflict
和保留 `Unconfirmed` 语义不变。取消／超时不代表回滚、删除、收编或重试许可。

成功仍为只移动 `ReceivedFileChunk`，只确认本次本地暂存，不代表持续远端权限、服务端工作区／
身份认证证明、远端送达回执、新鲜整文件校验或可重建进度。完成元数据不持有字节。预算克隆共享、
独立构造可叠加，不是全局 socket／resolver／进程内存／物理磁盘或多实例配额。

## 接线与测试

[二进制路由](large-file-chunk-http.md)仍未挂载；测试仅在独占环回监听显式接入它。生产启动、
AppState、OpenAPI／SDK／UI、Session 签发、Schema、自动信任／发现及部署不变。集中调度、
最终整文件拼装／保存、续传和节点互传仍独立；版本仍 0.5.6／API 0.3.0。

另行[私有拼装](large-file-chunk-assembly.md)现接已暂存旧式分块，核验一个独占整文件输出。
它不属于 `fetch_and_stage`，不是多块网络调度、用户选路径最终保存或恢复下载进度。

`file_chunk_client_tdd` 区分默认真实环回 HTTP／文件测试及两个 ignored 选择：一项真实当前
Source／PostgreSQL 桥接、一项 OpenSSL TLS。TLS 测试生成临时 CA／leaf／key，核对不可信 CA
和错误 IP SAN 在披露 HTTP 前拒绝，正确显式可信 TLS 后真实 17 字节私有暂存；不提交私钥。
`scripts/test-file-chunk-client.sh` 要求显式临时数据库及已安装 OpenSSL，先列准确 ignored 名再
执行，接在既有 CI 步骤内。`fileChunkClient` 守卫只检查组合／库存，实际执行证据另记。

此前 **106 状态块**、**32 详细执行尾部**及分日期能力张量保留，不重新评分或改标成新鲜证据。
中英当前说明及生成课程副本描述新链路，不宣称局域网部署或完整下载验收。

## 执行记录 2026-10-12

惰性客户端骨架在真实独占环回服务器／私有根下干净编译。Red 为 **0 通过／1 个预期失败**、
`Unavailable`、零 warning：没有发请求或保存块。服务器／根先清理再断言行为，不是缺符号、
socket 被拒或权限失败。Green 为 **1 通过／0 失败**、零 warning；准确真实请求及独立
EOF／SHA-256／0400／单硬链接核验通过，不是占位结果。

| 新鲜选择 | 实际结果及范围 |
|---|---|
| 客户端默认 target | **14 通过**、**2 ignored**、零 warning；真实 17 字节／4 MiB + 17 HTTP 和文件、规范配置／类型输入、坏头／短流／坏摘要、跳转目标零命中、独立头／正文期限及取消／准入 |
| 独立代理环境 | 同 14 项在 HTTP／HTTPS／ALL_PROXY 指向不可用环回端口 9、NO_PROXY 空的单个测试进程内通过，没有修改全局环境 |
| 私有分块工作单元 | **4 通过**、零 warning；旧接收器排队／运行，加 **2 项新用例**：真实客户端 HTTP EOF→排队保存、实际运行保存持有 caller 名额；跨取消保有真实准入／inode 工作，重复保存冲突 |
| 新显式外部 runner | **1 真实 Source SQL + 1 OpenSSL TLS 通过**、零 warning；当前接收者经真实 socket、服务端快照不变、后来撤权拒绝且旧本地字节保留；未知 CA／错 IP SAN 的独立解密 HTTP 字节计数为零，正确可信 SAN 后真实私有暂存 |
| 相邻显式 SQL runner | **3 接收器 + 6 二进制 HTTP + 10 Source reader 通过**、零 warning，覆盖原授权／固定、损坏存储、所有权、到期、等待及取消 |
| 完整后端门禁 | `scripts/quality-gate.sh --backend-only`：**919 Rust 通过**、**841 opt-in ignored**、**87 结果组**、**296 公共检查通过**、零 warning／失败 |

**20 个唯一新鲜 SQL**及一项显式 TLS 在默认验收之外执行；新增 14 默认与 2 单元已含于 919，
不能重复相加。ignored 包含 SQL／内核／外部选择，不是自动执行清单。没有将此前 903／19 或
117 用例记录改标，没有新跑前端构建、SDK 套件、安全审计、实际局域网／TLS 部署、整文件／
保存／续传验收、磁盘满／fsync／断电、互传或 RSS／吞吐基准。TLS 使用合成临时证书，不是
已安装部署验收。OpenSSL 先预留再释放端口供进程 bind，进程／就绪及健康正控核对夹具，
不代表原子排除所有外部端口竞争。

全部 Cargo／格式／SQL 在编辑冻结后由 root 串行执行。原生私有文件／socket 执行保留生产
所有者／父目录检查。PostgreSQL 最终审计返回自身准确 data 目录、无 TCP、**零夹具 Schema、
零其他客户端**；停止后仅删除已验证 `data`／`socket`，释放 **84432 KiB（约 82 MiB）**
可重建测试数据。TLS 临时私钥／进程／线程均由夹具清理。日志保留在
`/tmp/cyanrex-file-chunk-client-eGBxz16e`，未删除部署／仓库数据。版本、Cargo 依赖／lockfile、
正常启动及现用 API／SDK／UI 组合不变，没有提交／推送。
**106 旧状态块**、**32 旧执行尾部**及能力张量不变。组合守卫修正了导入计时器名称与实际
send 位置、清单 `chunk(index)` 与只取数据响应 `.chunk()` 的区别；源码文本守卫失败不是
行为 Red。TLS 对端字节计数增强负控后已复跑最终显式 runner；中英说明及课程副本已同步，
证据后的 `scripts/quality-gate.sh --format-only` 通过 **296 公共检查**、零 warning／失败，
课程／版本／OpenAPI 漂移、工具及 Rust 格式检查均通过；最终尾注同步后再核对同一门禁。
