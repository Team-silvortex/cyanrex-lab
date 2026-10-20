# 平台本机回环会话宿主

独立[原生 HTTPS 模式](platform-loopback-tls-host.md)另加私有 TLS 材料及有界握手；下文
`serve-loopback` 仍仅用 HTTP，不是 HTTPS 失败时的回退或代理后端。

可选末尾 `--notify-systemd` 开关增加[服务状态通知](platform-host-notifications.md)，不改变
下列存储、传输和排空契约；不带开关的命令保持原样。

**源码 0.6.3 已收录**的 Linux `cyanrex-platform serve-loopback`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，在独立低权限进程中运行
三条原[共享 Session 路由](platform-session-router.md)、[私人索引](platform-task-index-connection.md)、[只读重开](platform-task-content-connection.md)
及[手动空 Draft 创建](platform-task-create-connection.md)，另加[Draft 清单换版](platform-task-replace-connection.md)。仅接受规范的 `127.0.0.1:PORT` 或
`[::1]:PORT`，端口范围 1024–65535，HTTP Origin 必须与其准确对应。不开放局域网、不提供 TLS，
也不是受支持的 TLS 反向代理后端。

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

启动仍要求配置角色具备原预检权限，不添加登录、查询、退出或任务业务权限；可信初始化和账号创建
仍是独立步骤。

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

只有 POST `/platform/v1/session/login`、GET `/platform/v1/session`、POST
`/platform/v1/session/logout`、GET `/platform/v1/task-index`、GET `/platform/v1/tasks/{task_id}` 和
POST `/platform/v1/tasks` 和 PUT `/platform/v1/tasks/{task_id}/content` 进入处理器。创建正文仅含 `schema_version: 1`、`task_id` 和非空标题；
所有者、作用域、初始 Draft 状态及空 payload 由服务端派生，不要求代码或 Run，不发布载荷或
接通浏览器保存。独立换版 PUT 只接 `schema_version`、`expected_revision`、`manifest`，修改手动
Draft 标题或其已有本人准确文本引用；两种写入都不发布 Artifact。严格 DTO／正文、Origin、marker、Cookie、方法
及准入规则均保留，包括登录正文 32 KiB／两秒限制、共享的每 60 秒 64 次合格登录预算，以及
登录／生命周期／索引／内容／创建／换版六个独立响应通道。各默认四名额，可信组合可配置 1–16；
旧三／四／五容量构造器保留签名，换版默认四，`with_replace_capacities` 在拆分原绑定前验证全部六容量。
两种写入正文均保留 64 KiB／两秒限制，不在完整连接或已分派业务操作外再套超时。

确认登录才写 Path `/platform/v1` 的专用 host-only HttpOnly SameSite=Strict Cookie；回环
HTTP 不加 Secure。确认退出才清 Cookie。TCP 交付不证明浏览器收到，Cookie 也不按端口
隔离。应使用可信本地账号，在浏览器或局域网接入前规划独立、Cookie 安全的 HTTPS 域名，
不要把此明文监听转发为 TLS 后端。

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
```

默认测试使用真实 TCP 加私有合成路由，以及真实 CLI 拒绝路径；合成传输测试不绕过生产启动。
八条准确 PostgreSQL 用例要求一次性 `CYANREX_TEST_DATABASE_URL` 管理权限，验证真实
子进程启动、登录／查询／退出、原身份／根漂移、权限、可观测等待及信号停止。

该显式二进制模式不挂载原教学 AppState、前端认证、Artifact 发布、Task 状态写入、独立
Artifact 路由、OpenAPI 或 SDK 操作；Task 写入仅有独立空 Draft POST 和预期修订清单 PUT。TLS／域名规划、授权
局域网访问、已安装服务托管及浏览器集成仍是后续独立工作。
