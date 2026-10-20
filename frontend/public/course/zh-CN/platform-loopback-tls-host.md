# 原生 HTTPS 回环会话宿主

**源码 0.6.3 已收录**的 Linux `cyanrex-platform serve-loopback-tls`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，经原生 TLS 直接运行
原三条 [Session 路由](platform-session-router.md)、[私人索引](platform-task-index-connection.md)、[内容 GET](platform-task-content-connection.md)
及[手动空 Draft POST](platform-task-create-connection.md)，另加[Draft 清单 PUT](platform-task-replace-connection.md)。复用[明文回环宿主](platform-loopback-host.md)
的原存储、进程及 HTTP 边界，不经本地明文代理。仅支持 `127.0.0.1`、`::1` 的 1024–65535
端口，不增加局域网／DNS 监听、浏览器集成或部署验收。

可选末尾 `--notify-systemd` 开关增加[服务状态通知](platform-host-notifications.md)，
不带开关的启动不变；通知不证明客户端信任或持续健康。

## 显式私有输入

不带通知开关时，命令固定为 15 个参数：

```text
cyanrex-platform serve-loopback-tls --uid UID --gid GID --groups GID,...|none --config /absolute/private/input.json --bind 127.0.0.1:PORT --cert /absolute/private/chain.pem --key /absolute/private/key.pem
```

bind 政策不变：规范 `127.0.0.1:PORT` 或 `[::1]:PORT`，拒绝 DNS、通配、映射 IPv6、其余
127/8、低端口和自动选端口。私有 JSON 仍为完整格式 3，Origin 必须准确等于 `https://` 加规范
bind authority。TLS 参数不新增 JSON 字段、格式版本、环境回退，也不改变原诊断命令。

接触秘密之前，进程守卫仍核真实非 root 身份、补充组及 effective／permitted／inheritable／
ambient capabilities 为零，并设置 `no_new_privs=1`。格式／Origin 拒绝先于 TLS 文件访问；
材料加载校验在观察 Artifact 根、创建运行时和数据库连接之前，失败也复查进程。原目录／五
标记／根守卫、唯一声明式存储打开、私有角色选择、信号注册及最终绑定／Ready 顺序共用原实现。

JSON、证书、私钥各自独立复用原描述符相对、防符号链接读取：每文件最多 **16 KiB**，当前
身份所有、单硬链接、0400 或 0600 普通文件，最终父目录须为其所有的 0700。证书也须符合
该私有文件政策。符号链接、FIFO、硬链接、宽权限、不安全祖先及快照变化均拒绝，不修复。
每次启动读取一次，不热重载，也不构成三个文件的原子事务；加载至内存不等于所有原 PEM／
口令缓冲均被安全清零。文件读取及 PEM／DER／密钥解析都是同步启动步骤，字节／数量限制
不提供可抢占或硬性时间期限；两秒握手限制仅针对后续网络步骤。

## 证书材料与客户端信任

证书文件含 1–8 个 `CERTIFICATE` PEM 块，叶证书在前。私钥仅含一个未加密 PKCS8
`PRIVATE KEY`、PKCS1 `RSA PRIVATE KEY` 或 SEC1 `EC PRIVATE KEY` 块。块外仅允许 ASCII
空白，额外文本、混合类型、多私钥、加密私钥及尾部垃圾均拒绝。BEGIN／END 标记须准确独占
一行，可用 LF 或 CRLF；标记行末填充不是块外空白。宿主不调用子进程，不展开
shell、不查系统默认证书，也不自动生成材料。

显式 ring provider 必须确认密钥与证书匹配，未知匹配结果不算成功；启动还解析叶证书，校验
IP SAN 与准确 bind IP 相符。这不是完整 PKI 校验，不证明信任、证书链签名、时效、EKU 或
撤销状态。`platform loopback host ready` 仅表示启动及绑定完成，不证明客户端会信任该证书。
客户端须使用明确可信 CA，正常校验证书链、时效和准确 IP；不要关闭验证来让登录成功。
CA 安装、证书签发／续期及客户端信任分发仍由外部管理。

IP 客户端通常没有 DNS SNI；本模式允许不带 SNI，拒绝全部 DNS SNI，即使证书也含该 DNS
名字。支持 TLS 1.2、1.3；ALPN 只可选 `http/1.1`，混合 offer 选它，只有 h2 则失败；无
ALPN 按 HTTP/1.1 处理。没有 HTTP/2、Upgrade 或明文回退。TLS 客户端证书不是认证入口，
每项操作仍由当前 Session 命令授权。禁用 Session cache、ticket、early data 和 key log，
不安装进程全局 crypto provider。

## 准入与停止

共用的 16 连接快速失败预算在派生任务和 TLS 握手前取得，持有至 HTTP 响应写出。独立两秒
完整握手期限覆盖完全空闲和部分输入；停止时取消未完成握手，失败／超时只关闭其连接。
成功 TLS 才进入原 HTTP/1.1 驱动，另有两秒完整请求头期限、32 个头、16 KiB 解析缓冲、
单个准确 Host 和每连接一条请求限制。

握手或解析失败不执行业务，也不保证私有 JSON／HTTP 头；有效 TLS 后的 Host 守卫错误
保留空的私有 400。转发头不选择 HTTPS、authority 或存储。七条工作区路由保留各自严格合同；
创建只接受 `schema_version: 1`、`task_id`、`title`，不要求 payload、代码或 Run。登录正文／
速率不变。换版 PUT 只接版本、预期修订及完整标题／已有引用清单；登录／生命周期／索引／内容／
创建／换版六个独立响应通道各默认四，可信配置 1–16，旧容量签名保持换版默认四，
`with_replace_capacities` 在拆分原绑定前验证全部六容量。不接 Artifact 发布、状态写入、浏览器保存
或 SDK/OpenAPI，不给已分派命令或完整连接加新的总超时。

沿用 30 秒软性信号排空及一秒运行时关闭等待。同步文件访问和已分派阻塞工作意味着不能保证
硬性进程退出期限。停止／断开不证明 SQL 回滚，不还原 OTP、不补偿 Session，也不授予重试。
两模式共用原 Ready／stopped／failed 固定提示及 serve 专用私有 panic hook；普通错误不披露
PEM、口令、路径或 token。

## Cookie 与验证边界

确认 HTTPS 登录给原 Path `/platform/v1`、host-only HttpOnly SameSite=Strict Cookie 增加
Secure；确认退出用同一 Secure 政策清除。JSON 不返回 token。明文回环保留原无 Secure
政策，不是 TLS 失败时的回退。

Secure 不提供 Cookie 端口隔离，真实 TCP／TLS 交付也不证明浏览器收到。不要混用该 Cookie
host 上的无关服务，也不要把 HTTPS 回环描述为已完成 Cookie 安全域规划。独立 HTTPS
host／域名、授权局域网和浏览器接入仍属后续；特权教学 Engine 及其 Cookie 不变。

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_serve_loopback_tls_defaults_tdd
bash scripts/test-platform-loopback-tls-host.sh
```

原生 TLS 测试需要 OpenSSL 有界生成一次性 CA／证书／私钥，产品宿主不调用它；不安装
系统 CA，不提交私钥。默认私有合成路由验证传输／材料，不证明 SQL 授权。八条准确 SQL
选择要求一次性 `CYANREX_TEST_DATABASE_URL` 管理权限及运行角色口令认证，其中五条验证真实
HTTPS 子进程、Secure Cookie、对端拒绝、原对象漂移、权限和可观测排空，三条验证可选明文／
TLS 通知；不证明已部署局域网或浏览器工作流。
