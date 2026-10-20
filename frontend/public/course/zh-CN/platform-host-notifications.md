# 平台宿主服务状态通知

**源码 0.6.3 已收录**：Linux [明文](platform-loopback-host.md)及
[原生 HTTPS](platform-loopback-tls-host.md) Session 宿主可选发送服务状态通知。
在原有 serve 命令末尾添加单个 `--notify-systemd` 开关；诊断命令不接受它。
不加开关时保留原有行为，`NOTIFY_SOCKET` 不会自行启用通知。
这只是[提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下有界的服务管理接入点，
不是安装器、持久服务或部署验收。

## 显式通知目标

启用后，可信服务管理器必须提供 `NOTIFY_SOCKET`。宿主在原有进程检查之后、读取私有配置或
TLS 文件之前，仅读取一次该值。缺失、空值、过长或不支持的值都返回现有中性宿主失败。
仅支持 Unix datagram 的绝对路径或以 `@` 开头的 Linux 抽象名称；拒绝 VSOCK 等传输。
目标由运维人员选择，宿主不认证对方就是 systemd；该值不选择 Origin、监听地址、数据库、
角色、凭据或存储基线。

宿主连接并保留一个非阻塞、close-on-exec datagram socket，不创建或删除接收端，不通过重新连接
追随替换目标，不调用 `systemd-notify`，也不重试失败通知。原生 datagram 仅含 `READY=1` 或
`STOPPING=1`。启用 `SO_PASSCRED` 的接收端可取得内核给出的真实发送者 PID、UID 和 GID；
宿主不发送自报 PID、文件描述符、状态文字、路径或秘密。通知不进行无界队列等待；Unix 路径查找
仍是同步工作，没有硬性墙钟期限。

## 就绪与停机顺序

仅在完整私有输入、可选 TLS 材料检查、原始根目录及声明存储检查、初始客户端关闭、路由构造、
信号注册、最终监听绑定和进程复查全部完成后，发送一次 `READY=1`。只有发送成功，才打印原有
stdout Ready 通知；发送失败会关闭刚绑定的监听并返回失败，不接收业务工作。
此前的错误不发送 Ready 或 Stopping。没有 Ready 不代表监听从未短暂绑定。

HTTP 驱动因信号或已处理的运行失败停止时，先丢弃监听，再尝试一次 `STOPPING=1`，然后通知已有
连接关闭并执行原有软排空。Stopping 发送失败会使最终结果失败，但不能跳过排空。
不新增最终 stopped datagram；干净退出及原有 stdout stopped 通知保持原义。
主线程 panic、abort、OOM 或外部杀进程不因此获得新的排空保证。

发送成功只代表内核接受了 datagram，不代表管理器确认。Ready 只是本次启动观察，不是持续数据库
健康、客户端证书信任、浏览器 Cookie 收到或授权续期证明。Stopping 不代表排空完成、SQL 回滚、
OTP 恢复、Session 补偿、重启恢复或重试许可。不新增 watchdog、心跳、reload、socket activation、
PID 文件或自动重启。

## 手动服务接入

对已经独立审查的 systemd 服务，直接以 `cyanrex-platform` 作为主进程执行，在命令末尾添加
`--notify-systemd`，并选择 `Type=notify`、`NotifyAccess=main` 和 `Restart=no`。
这些设置遵循 [systemd 通知协议](https://www.freedesktop.org/software/systemd/man/latest/sd_notify.html)及
[服务通知访问策略](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#NotifyAccess=)。
保持 `WatchdogSec` 禁用，宿主不发送 watchdog 通知。不要用包装器提前报告 Ready，也不要用
教学 Engine 的 HTTP 健康请求替代此宿主的就绪观察。

运维人员仍需选择并审查实际固定非 root UID、主 GID 和完整附加组。systemd 的 `User=` 可能从
NSS 初始化组；`SupplementaryGroups=` 是追加，而非任意精确替换。宿主 `--groups` 检查仍有最终
约束力，任何差异均拒绝。私有输入／根目录访问与原有数据库权限需要独立配置，服务命令不修复它们。
当前教学 Docker／离线包也不是平台宿主部署包。

管理器启动／停止期限及最终强杀是运维人员的独立策略。应为宿主 30 秒软排空及运行时关闭等待
留出空间，同时认识到同步工作使硬性退出期限无法保证。管理器超时或强杀不证明回滚，也不授权
自动重试。本改动不安装 unit、不启用服务，不改变系统 CA、防火墙、局域网／DNS 绑定、
教学认证、浏览器路由或存储 schema。

## 验证

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_notify_defaults_tdd
bash scripts/test-platform-loopback-tls-host.sh
```

默认测试使用真实 Unix datagram 和清空能力的 CLI 子进程。一次性 PostgreSQL 选择通过真实启动与
TCP／HTTPS Session 工作验证通知顺序，包括可观测启动等待、Ready 发送拒绝与 Stopping
发送失败后真实 SQL 仍然排空。
这些测试不安装服务，不证明 PID 1 归属判断、已部署服务策略、持久恢复或多机／浏览器验收。
