# 平台本机回环会话宿主

独立[原生 HTTPS 模式](platform-loopback-tls-host.md)另加私有 TLS 材料及有界握手；下文
`serve-loopback` 仍仅用 HTTP，不是 HTTPS 失败时的回退或代理后端。

可选尾部开关按 `[--task-text-publication] [--notify-systemd] [--session-ui]` 排列：分别启用
[Task 绑定发布](platform-text-artifact-connection.md)、[服务状态通知](platform-host-notifications.md)
和下述 Session／私人 Task 浏览器壳。三个开关默认均关闭，不带开关的启动保持原样。

**源码 0.6.4 已收录**的 Linux `cyanrex-platform serve-loopback`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，在独立低权限进程中运行
三条原[共享 Session 路由](platform-session-router.md)、[私人索引](platform-task-index-connection.md)、[只读重开](platform-task-content-connection.md)
及[手动空 Draft 创建](platform-task-create-connection.md)，另加[Draft 清单换版](platform-task-replace-connection.md)。仅接受规范的 `127.0.0.1:PORT` 或
`[::1]:PORT`，端口范围 1024–65535，HTTP Origin 必须与其准确对应。不开放局域网、不提供 TLS，
也不是受支持的 TLS 反向代理后端。

0.6.4 向两个 CLI 宿主显式加入[原绑定文本发布](platform-text-artifact-connection.md)。
这是独立 POST，不自动挂接 Task，也不挂旧连接池文本／读取路由。下述 0.6.6 正文编辑切片
只有同时启用 `--task-text-publication` 和 `--session-ui` 才调用 Task 绑定 POST。

## 显式启动

命令形状固定为：

```text
cyanrex-platform serve-loopback --uid UID --gid GID --groups GID,...|none --config /absolute/private/input.json --bind 127.0.0.1:PORT
```

UID、GID 和补充组白名单必须对应实际非 root 进程，不是要求程序取得的身份。
[进程守卫](platform-process-startup.md)在接触秘密和创建运行时之前，拒绝身份、effective／
permitted／inheritable／ambient capabilities 或线程数不符，设置并核实 `no_new_privs=1`。
仅 capability bounding set 非空可以接受。这些检查不是沙箱，也不会自动降权。

[私有输入](platform-startup-input.md)必须为格式 3，带完整可信的
[目录及五类标记身份](platform-marker-expectation.md)。格式 1、2 继续适用于旧诊断，但不能
运行此宿主。Origin 必须等于 `http://` 加规范 bind authority，包含显式端口。DNS、`localhost`、
其余 127/8 地址、映射／带 scope 的 IPv6、通配／局域网地址、低端口及非规范写法均被拒绝。

配置的最终父目录须由当前身份所有且为 0700，文件须为其所有、单硬链接、0400 或 0600 普通
文件。原有基于目录描述符、拒绝符号链接、16 KiB 上限的读取及严格 JSON 校验保持不变。
宿主不生成声明，也不把诊断成功转换成运行时基线。妥善保护文件和数据库密码；释放内存选项
不等于安全清零。

启动先准备并复查单一主线程，加载及复查私有输入，验证模式／Origin／bind，再观察选中的
私有 Artifact 根，之后才创建单线程 IO／时间运行时。仅调用一次
`open_platform_storage_expected`，核对完整原目录和标记、关闭初始连接并复查原根后交付绑定。
根观察在本次启动时选定：格式 3 不声明历史根 inode。`PGOPTIONS` 即使为空也被拒绝；没有
环境数据库回退、安装器、连接池、迁移、基线收养或自动重试。仍需显式信任本地数据库对端。

无论存储工作成功与否，宿主随后复查当前线程，构造一份共享 Session 状态，注册 SIGTERM／
SIGINT，最后绑定字面地址。再次核线程后才打印 `platform loopback host ready`。绑定可能
先短暂成功、后续进程检查再失败，因此没有 Ready 不等于从未绑定 socket。

启动仍要求配置角色具备原预检与业务权限，不自动授予任何权限。发布仅额外需要配置 Artifact
命名空间中 `collaboration_artifacts`、`collaboration_artifact_revisions`、
`collaboration_artifact_outbox` 的 INSERT。组合角色服务全部通道，须保留既有登录凭据 SELECT、
Task 写入及 Artifact／Task 行锁列权限；不增加整表 UPDATE／DELETE／DDL。可信初始化和账号创建仍独立。

## HTTP 入口

私有 Hyper HTTP/1.1 驱动最多准入 16 个并发连接。每连接只处理一条请求，最多 32 个头、
16 KiB 解析缓冲，完整请求头须在两秒内到达，完全不发首字节也受限。满额时关闭额外接入
socket，不排队等待请求名额。许可覆盖响应写出，已完成任务会及时回收，不积累历史任务句柄。

请求必须为 HTTP/1.1、origin-form URI，且仅有一个与规范 authority 准确相同的 Host。
拒绝 CONNECT 和 Upgrade；转发头不选择 authority、scheme 或数据库。不支持 HTTP/2、
WebSocket、连接复用或第二条流水线命令。

Host 守卫拒绝使用空的私有响应，带 no-store／no-cache／nosniff。原始解析失败可直接关闭
连接，或返回不经过路由私有头中间件的解析器错误。两类都不是业务 JSON 合同，也不签发
Cookie 或提供 CORS。

资源处理器仍为 POST `/platform/v1/session/login`、GET `/platform/v1/session`、POST
`/platform/v1/session/logout`、GET `/platform/v1/task-index`、GET `/platform/v1/tasks/{task_id}` 和
POST `/platform/v1/tasks`、PUT `/platform/v1/tasks/{task_id}/content` 及 POST
`/platform/v1/text-artifacts` 进入处理器。Task 创建正文仅含 `schema_version: 1`、`task_id` 和非空标题；
所有者、作用域、初始 Draft 状态及空 payload 由服务端派生，不要求代码或 Run，不发布载荷或
接通完整浏览器保存。独立换版 PUT 只接 `schema_version`、`expected_revision`、`manifest`，修改手动
Draft 标题或其已有本人准确文本引用；两种 Task 写入都不发布 Artifact。独立文本 POST 只接版本、
Artifact／修订 ID、标题和文本，确认 201 只返回元数据，不回显文本。实际报文正文限制为 2 MiB／两秒，
与解码文本 256 KiB 限制独立；Hyper 16 KiB 解析缓冲不变，不是正文总上限。严格 DTO／正文、Origin、marker、Cookie、方法
及准入规则均保留，包括登录正文 32 KiB／两秒限制、共享的每 60 秒 64 次合格登录预算，以及
登录／生命周期／索引／内容／创建／换版／发布七个独立响应通道。两个 CLI 宿主显式使用
`with_text_publication_capacities`，各通道四名额，可信容量范围 1–16；先验证全部七容量，
再将同一原存储 Arc 拆为七项能力。原构造器仍不启用发布，保留七路由／六通道；
`with_replace_capacities` 仍验证六容量。全局 16 连接限制不变，不保证 28 请求并发。
两种 Task 写入正文均保留 64 KiB／两秒限制；文本发布另保留四个真实文件任务名额，
不在完整连接或已分派业务操作外再套超时。

确认登录才写 Path `/platform/v1` 的专用 host-only HttpOnly SameSite=Strict Cookie；回环
HTTP 不加 Secure。确认退出才清 Cookie。TCP 交付不证明浏览器收到，Cookie 也不按端口
隔离。应使用可信本地账号，在更广泛的浏览器或局域网部署前规划独立、Cookie 安全的 HTTPS 域名，
不要把此明文监听转发为 TLS 后端。

## 可选浏览器 Session 壳

显式 `--session-ui` 仅启用十三个编译内嵌资源：`/platform/`、`/platform/ui/app.js`、
`/platform/ui/labels.js`、`/platform/ui/tasks.js`、`/platform/ui/task-edit.js`、`/platform/ui/task-contract.js`、`/platform/ui/request.js` 和
`/platform/ui/style.css`，另加 `/platform/ui/body-contract.js` 及 `/platform/ui/task-save/` 下的
`taskContentSave.js`、`taskSaveContract.js`、`taskDraft.js`、`taskPayloadLanguage.js`；不带开关时这些路径仍不存在。在配置的
字面回环 Origin 打开 `/platform/`，可用英语、简体中文、西班牙语、日语和韩语，语言选择
只保存在页面内存。它不是 Next.js、目录服务器、代理或教学应用替代品。

0.6.6 共享 `request.js` 由 Session、读取器、显示信息和正文编辑器调用。
它只接受这些调用的规范相对路径／方法，正文发布仅接受准确 Task 绑定路径，固定同源 Cookie 模式及 marker，
拒绝重定向、凭据／头覆盖和非 Task 绑定发布路径。十秒覆盖等待与正文收集，响应规范仍分别有界，
调用方不能自行扩大预算：

| 既有请求 | 响应字节 | 正文帧数 |
|---|---:|---:|
| Session 检查／登录／退出 | 4 KiB | 4,096 |
| Task 索引 | 128 KiB | 16,384 |
| Task 正文读取 | 50 MiB | 16,384 |
| 显示信息 PUT | 128 KiB | 4,096 |
| Task 绑定文本 POST | 128 KiB | 4,096 |

登录 JSON 仍至多 32 KiB，清单 PUT 至多 64 KiB，Task 绑定文本 POST 至多 2 MiB；GET／退出无正文。
帮助层先完整读取受原字节／帧上限保护的 clone，EOF 后做 fatal UTF-8 检查，再以原 Response 的
原生 `arrayBuffer()` 消费，逐字节核对长度与内容；原生消费不能抢在 guard EOF 前。失败同时发起
两个 tee 分支取消，不先等待其中一个，不回退、不增加 HTTP。会保留多份有界字节／字符串副本，
不只是一份响应大小的内存，也不是进程级内存配额。帮助层只返回原始 UTF-8，
严格 JSON／回执校验、当前代次、确认结果和失权 UI 仍由调用方持有。取消只中止浏览器收集，
不终止服务端工作，迟到响应不能重新激活写入；不改变 Cookie 部署或新增 API。

外部脚本／样式与严格 CSP 将其限制在同一 Origin；只调用现有专用平台 Session 登录／检查／
经确认退出、私人 Task 索引／内容 GET、既有清单 PUT，以及双开关启用后的 Task 绑定文本 POST。
登录成功后另作一次只读检查，验证浏览器 Cookie 往返，不据此确认用户名
或角色。密码／OTP 在交出请求时清空输入，不宣称安全内存清零；秘密不进入 URL、浏览器存储或日志。
未知／畸形响应及十秒浏览器等待期限均保留未知，不证明回滚，不自动重试写入。退出需确认，
默认焦点为取消。Cookie 的 host／Path／Secure 政策及不按端口隔离的边界均不变。

观察到活跃 Session 后，用户显式加载每页 20 项的本人私人列表，再选择 Task 重开；不自动
列举或选择，翻页替换而非累积。索引元数据不证明内容可读；空 payload 合法，文本以惰性纯文本
显示，支持重复文件名及合法未知语言标记，不接编辑器或 Run。

索引报文上限 128 KiB，内容报文上限 50 MiB，正文最多 16,384 帧，浏览器等待上限十秒；
定时器不能抢占同步解析。严格 JSON 拒绝重复／转义键和不安全数值，展示前核对准确作用域、
修订、有序引用和本地 SHA-256；解码后仍最多 32 项、每项 256 KiB。Task 401／403、新的 Session
检查、确认退出开始派发及 pagehide 都清除旧私人数据；代次守卫丢弃迟到响应。不接
Task 创建或执行；正常 Next.js 本地草稿编辑器仍独立，不新增认证 API 或数据库权限。

**0.6.4 显示信息保存切片**只允许本人明确修改既有手动 Draft 的标题及已有 payload 的
显示文件名／语言提示，不改 Artifact 引用、顺序、字节或条数。预览绑定目标与修订，默认焦点为
取消；确认后仅发一次带预期修订的 PUT，完整规范正文不超过 64 KiB。只有 HTTP 200 与严格匹配的
三字段 Task／manifest 回执才确认显示信息已保存；回执限制为 128 KiB／4,096 帧，十秒仅约束
浏览器等待，不抢占解析或服务端工作。所示正文仍是先前核验副本，不是新鲜读取。
冲突、畸形／丢失响应及取消保留未知，不重放。仅修改这些元数据时仍只发该 PUT，不发文本 POST。

**0.6.6 正文保存切片**还要求默认关闭的 `--task-text-publication`。宿主明确声明该能力后，
本人手动 Draft 的既有文本槽才可编辑；槽的数量和顺序固定，不增加、删除或重排。
未修改槽保留准确原文；textarea 的最小替换保留未变前后缀及原 CRLF／混合换行。
任一正文改变才进入正文流程：确认前冻结完整本地草稿、目标与原 Task／manifest，校验最多
32 槽、每槽 256 KiB、完整转义草稿 8 MiB，再按原槽顺序逐项 POST 全部槽。
每次携带相同的完整原基线；只有 HTTP 201 及严格匹配原 Task 标题、分配标识、owner／scope、
长度和摘要的 Artifact 回执才推进。全部确认后才以原 `expected_revision` PUT 新标题和新准确引用，
只有严格 HTTP 200 Task／manifest 回执才确认保存。Artifact 前缀确认不代表 Task 已保存。
成功视图使用已确认元数据和冻结的已提交文本，不代表新鲜正文读取。

正文预览及离开确认默认取消。取消、冲突、未知、401／403 响应头或 pagehide 会封闭计划并保留
确认前缀及未知步骤；迟到回执不能推进。失权或离页立即清私人 DOM，后续 Session 不能恢复、
重放、收编或删除这些结果，不自动重试。失败可能留下已发布而未挂接的私人 Artifact。

四个 ESM 镜像由 `scripts/generate-platform-task-save-assets.mjs` 使用 Node 内置类型剥离，
从 `frontend/src/features/tasks/` 下同名四份 TypeScript 源生成，不独立维护。`--check` 和六项
生成测试拒绝漂移。Node 24.21 已做完整验证，22.22 仅验证生成后 ESM 运行；经官方 SHA-256 核验的
Node 22.23.3 已通过生成器 `--check` 与全部六项测试，输出字节与 24.21 一致。

有 dirty 或未知工作时，Task 导航、Session 检查、登录及退出均需明确丢弃确认，默认取消。
显示信息请求的 401／403 响应头立即清私人 DOM；上下文变化／pagehide 同样将工作隔离于封闭页面
内存，后续 Session 不能恢复／重放，明确丢弃后才可继续。这不是安全清零、回滚或恢复，也不增加
SDK 集成、新 HTTP API 或数据库权限。

## 停止与失败

SIGTERM 或 SIGINT 停止接入，并请求现存连接优雅关闭。软性等待 30 秒后会中止、回收
剩余本地 future，并报告失败。运行时关闭最多等待一秒只限制等待，不证明已分派密码工作
结束。同步文件系统检查可能阻塞当前线程及其定时器，不保证硬性进程退出期限。对端协议错误
只关闭该连接；accept 失败或任务 panic 会使宿主失败，排空其他工作，不自动重试。

信号触发且正常排空时打印 `platform loopback host stopped`。受处理的启动或运行错误均向 stderr
打印中性的 `platform loopback host failed` 并退出 1，命令形状错误退出 64。不打印驱动
细节、路径、密码或 token。只有此 serve 模式安装 panic hook，省略 panic 内容、位置及
backtrace，只打印同一固定提示；任务 panic 后宿主返回失败可能再次提示。主线程 panic 退出码、
abort 和 OOM 不属于正常 Result 失败。运行后失败不声称从未启动宿主。取消、断开或停止不证明 SQL
回滚，不撤销已签发 Session、不还原 OTP，也不授予重试或补偿权限。

## 验证及剩余工作

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_serve_loopback_defaults_tdd
bash scripts/test-platform-loopback-host.sh
bash scripts/test-platform-text-publication-host.sh
bash scripts/test-platform-task-reader-browser.sh
bash scripts/test-platform-task-metadata-browser.sh
bash scripts/test-platform-task-body-browser.sh
bash scripts/test-platform-browser-session.sh
bash scripts/test-platform-browser-tasks.sh
bash scripts/test-platform-browser-task-metadata.sh
bash scripts/test-platform-browser-task-body.sh
```

默认测试使用真实 TCP 加私有合成路由，以及真实 CLI 拒绝路径；合成传输测试不绕过生产启动。
八条准确 PostgreSQL 用例要求一次性 `CYANREX_TEST_DATABASE_URL` 管理权限，验证真实
子进程启动、登录／查询／退出、原身份／根漂移、权限、可观测等待及信号停止。
发布 runner 另清点六条真实 SQL／工作区／明文／TLS 选择；库存不是通过结果或浏览器验收。

浏览器 Session runner 清点明文与 TLS 两条真实 SQL／浏览器选择，要求显式的一次性
`CYANREX_TEST_DATABASE_URL`、绝对 `CYANREX_PLAYWRIGHT_MODULE` 和 `CYANREX_CHROMIUM_PATH`，
以及 OpenSSL、`certutil` 和可用的隔离浏览器／NSS 信任环境。不修改用户／系统信任，不绕过
证书验证。本机 legacy NSS 正确拒绝原账号；下文独立容器已通过原浏览器 TLS 选择，原生 socket 验收不能代替它。

Task-reader 浏览器 runner 使用生产资源与 mock API，12 条用例只证明 UI 行为，不证明 SQL 授权。
独立 Task browser runner 精确选择一条真实明文宿主 SQL／浏览器用例，核私人分页／重开和权限
撤销且不写 Task／Artifact，不宣称 TLS 浏览器覆盖。
显示信息夹具另列举 12 条 mock API 浏览器用例，其 SQL／浏览器 runner 精确选择一条真实明文
宿主显示信息保存／冲突／撤权用例，不能代替 TLS 浏览器验收。
正文夹具 `scripts/tests/platformTaskBody.browser.mjs` 另列举 13 条 mock API 用例，验证固定槽、
顺序发布／换版和失败封闭。独立正文 SQL／浏览器 runner 必须分别选择明文与 TLS，不以明文替代 TLS。
2026-11-13 明文选择已通过自然登录、两次原基线／原标题 POST 201 与独立 PUT 200，另核 Artifact
head／revision／outbox、文件 `0400`／摘要及 Task 修订／outbox。冲突和撤销会话均在首个 POST
停止且无新写入。独立真实部分前缀用例已验证首 Artifact 确认后，第二 POST 遇 SQL 行锁超时 503
或准确会话撤销 401，前缀仍保留；两条均通过准确 SQL／文件／UI 核验。它们不等于迟到确认／真实 BFCache 或
部署验收。本机旧 NSS 仍阻挡原账号 TLS；两条原 TLS 选择已在
[可选隔离容器入口](testing-guide.md#隔离浏览器-tls-工作机)分别通过，含准确 SQL／文件及一次性数据库停止确认。

该显式二进制模式不挂载原教学 AppState、Next.js 前端认证、Task 状态写入、旧连接池 Artifact 读取、
OpenAPI 或 SDK 操作；Task 写入仅有空 Draft POST 和预期修订清单 PUT，独立文本 POST 仅创建私人 Artifact。TLS／域名规划、授权
局域网访问、已安装服务托管、正常应用／SDK 保存及更广的浏览器／TLS 验收仍是后续独立工作。
