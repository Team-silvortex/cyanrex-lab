# SSH 部署与学生课堂接入

## 约定与当前实现

| 入口 | 方向 | 授权依据 | 本轮实现 |
|---|---|---|---|
| SSH 部署 | 教师工作站 → 受管 Linux 主机 | 系统 SSH 密钥与独立核实的主机密钥 | Rust `cyanrex-release ssh plan/apply` 管理已安装的离线包 |
| 最小发现链接 | 学生 → 教师课堂 | 已核实的 HTTPS 地址、教师邀请，随后密码 + TOTP | `/join`、最小发现描述、学生邀请与撤销 |

单人模式仍默认自己是本机部署教师；加入另一位教师的课堂时，在对方实例创建的是**学生账号**，
不会继承本机教师权限。课堂邀请与 Runner Agent bootstrap 凭据完全分开；发现数据不能指令
Engine 执行 SSH 或注册高权限 Agent。教师负责应用管理，但哪些密钥能管理宿主仍由系统 SSH
权限决定，不提供浏览器 SSH Shell 或私钥上传框。

本轮**尚未实现** mDNS 广播/浏览、扫网、安装包上传、裸机初始化、自动升级/回滚、VM 生命周期、
课堂成员移除或独立无特权控制服务。第一版发现通道是教师发布链接；后续桌面 DNS-SD 可以发现
同一入口，但广播不得包含邀请码、密码、TOTP、学生名单、Agent bootstrap token 或远程命令。

## SSH 部署管理

先通过已有校验/安全解包流程将可信离线包放到 Linux 目标机，保留带版本与时间戳的目录名。
目标 `.env` 必须由 SSH 用户所有、权限为 `0600`；Docker/Compose 和必要宿主权限需预先具备。
命令不上传文件、不代造凭据、不获取 sudo、不打开端口。本机 release 工具、目标机工具和目标
发行包元数据必须使用相同版本。

在本机 OpenSSH 配置中定义主机别名及用户、密钥、端口；通过主机控制台或可信管理员独立核实
主机指纹后，再写入显式指定的 `known_hosts`。不能把未经核实的 `ssh-keyscan` 输出当认证，
遇到指纹不匹配不要关闭校验。本机 SSH 配置（包括跳板/代理命令）属于可信运维输入。密钥留在
系统 OpenSSH/ssh-agent 中，应用不读取、不转发私钥。

以下主机、目录与时间戳均为示例：

```bash
./cyanrex-release ssh plan --host classroom-host \
  --directory /srv/cyanrex/cyanrex-lab-0.3.7-20260909-010203 \
  --known-hosts /home/teacher/.ssh/known_hosts --action up
```

`plan` 不连接目标机。审阅完整主机、目录、动作、预期版本和 SSH 参数后，复制输出中的完整
`confirmation` 值，单独执行确认命令：

```bash
./cyanrex-release ssh apply --host classroom-host \
  --directory /srv/cyanrex/cyanrex-lab-0.3.7-20260909-010203 \
  --known-hosts /home/teacher/.ssh/known_hosts --action up \
  --confirm 'APPLY <已审阅计划中的哈希>'
```

查看状态用 `status`，停止用 `down`，每个动作都需重新预览。`down` 停止部署及可选 Agent，
不附加卷删除参数。确认值绑定主机、目录、动作、工具版本、已知主机文件摘要和参数，只防误操作，
不是 SSH 凭据；本机 SSH 配置变化也需重新审阅。

仅接受字面 ASCII 主机别名和绝对路径，拒绝 Shell 片段、穿越和任意附加动作参数。强制严格
主机校验、非交互公钥认证；禁用 Agent/X11 转发、隐式端口转发、自动更新主机密钥和复用已有
主连接。底层语义参考 [OpenSSH 客户端](https://man.openbsd.org/ssh)与
[配置手册](https://man.openbsd.org/ssh_config)。

目标先校验工具/包版本、受 checksum 约束的元数据与部署控制文件，再检查 `.env` 所有者和
私有权限，最后调用固定 `deploy.sh up/down/status`。Checksum 只能相对可信包发现文件变化，
不能证明恶意发布者或失陷宿主可信；镜像内容仍需已有发行工具验收，不是新的完整镜像/内核验收。

SSH 带连接超时、保活和 10 分钟总期限，失败不重试。断连/超时不代表已回滚：先查看状态和
宿主日志。命令不自动修改防火墙、监听地址、CORS、TLS、证书或生产配置。

## 最小发现与学生邀请

默认关闭。在 Engine 私有部署配置中同时设置三项课堂信息，例如：

```dotenv
CYANREX_CLASSROOM_ID=23d40d83-19de-43d3-85fe-506d86592a70
CYANREX_CLASSROOM_NAME=Lab A
CYANREX_CLASSROOM_PUBLIC_URL=https://teacher.example
CYANREX_CORS_ORIGINS=https://teacher.example
NEXT_PUBLIC_ENGINE_URL=https://teacher.example:8443
CYANREX_SECURE_COOKIES=true
CYANREX_ALLOW_REGISTRATION=false
CYANREX_ALLOW_TOTP_BOOTSTRAP=false
CYANREX_ALLOW_MISSING_ORIGIN=false
```

以上均为非秘密示例，不是可直接上线的网络配置。为本实例生成稳定、非零 UUID，升级保留；
名称不要含敏感信息。PUBLIC_URL 为**前端 Origin**，不能含账号密码、路径、query 或 fragment。
局域网必须 HTTPS；HTTP 只用于可信回环/SSH 隧道。课堂 ID 只是连续性标识，**不是密码学指纹**。
必须通过可信渠道核实完整 HTTPS 地址和证书；局域网私有 CA 需另外受控安装。

教师需自行提供 TLS 入口和防火墙/来源限制，保持 Compose 回环绑定（共享 bind 设置也会发布
PostgreSQL）。不能为了发现而直接暴露特权 Engine/数据库。前端/API 必须满足现有同站 Cookie 策略。

`NEXT_PUBLIC_ENGINE_URL` 在 Next.js **构建时**写入前端资源和 CSP。源码 Compose 已传递
对应 build arg，打包脚本也传递导出的该变量。部署前按目标 API 地址重建前端/包，仅修改离线
`.env` 不能重写旧镜像；native/WSL 开发启动器会传递该 URL 和 CORS/Secure Cookie 设置。
固定构建为 `localhost:8080` 的前端不能直接用于局域网学生。

1. 学生打开教师发布的 `/join`，从**预配置** Engine 请求
   `GET /.well-known/cyanrex-classroom`，不携带凭据、不缓存、不跟随重定向。
2. 描述只含服务类型、课堂 ID/名称、产品版本、协议范围、入口 URL、能力，不含教师用户名、
   名单、内核细节或密钥。入口必须属于当前前端，发现数据不能切换 API Origin。
3. 教师打开**课堂 → 学生发现与邀请接入**，核实指定学生用户名后确认签发私密邀请。这就是
   对该学生的接入授权，不是开放注册或自动批准任意局域网发现请求。
4. 学生经可信渠道核对显示的前端/API 地址和课堂 ID，明确勾选确认，再给指定用户名设密码。
   最终确认列出课堂、地址、ID、用户名，不部署软件或执行 eBPF。
5. 成功后仅显示本次 TOTP 密钥，并本地生成二维码。保存到认证器后，以密码 + OTP 正常登录；
   加入不会自动签发登录会话。

每个邀请使用随机 64 个十六进制字符的能力凭据，绑定一个规范化学生用户名，10 分钟有效、仅用
一次。Engine 内存只保存 SHA-256 摘要和非秘密清单，最多 256 个有效邀请；重启全部作废。
令牌放在 URL **fragment**，不进入 query；发请求前清除地址栏及 Next 历史 state，不写入
localStorage/sessionStorage。邀请和 TOTP 响应均为 `Cache-Control: no-store`。不要把私密
链接放入广播、公告、日志、截图或共享剪贴板历史。

清单不会再次返回私密链接。撤销仅阻止尚未使用的邀请，不删除已加入账号或撤销已有会话。
公开注册/bootstrap 可继续关闭，邀请不能抢占教师预留名字，也不接受客户端角色字段。

邀请在异步创建账号**之前**原子消费，即使创建失败或响应丢失也不恢复或盲目重试。已有账号应
正常登录；若账号已创建但 OTP 响应丢失，应核实并受监督恢复，本轮没有教师重置账号/OTP UI。
持久化沿用已有规则，纯内存降级账号仍会随重启丢失。

数据库初始化失败也必须走完整的内存降级注册：账号实际保存且能登录后才返回成功，重名仍会
拒绝；这不代表持久化接入。持久化部署的认证数据库尚不可用时，临时账号的退出、改密及删除
也会被阻止，恢复步骤见[会话与数据库故障](security.md#会话与数据库故障)；能内存登录不代表
持久化已恢复。自定义 `CYANREX_TOTP_ISSUER` 会在接入链接中进行 URL 编码，
中文及 URL 分隔字符不会再破坏密钥或 TOTP 参数。

## 版本兼容与限制

课堂接入协议独立于产品版本，也独立于 Agent v1。当前支持 `1..1` 和 `student-invite-v1`。
客户端显式提交课堂 ID、协议版本、产品/客户端版本与必需能力。产品 patch 不同不单独阻止兼容
接入；产品版本只用于展示，不证明安全补丁已安装，也不是允许使用已知漏洞版本的依据。
不支持的协议或必需能力返回 `426`，不消费邀请；课堂 ID 不符返回 `409`。不自动降级、换教师
地址或回退公开注册。错误/错用户/过期/已用/撤销邀请返回 `403`，请求体上限 4 KiB，教师写操作
仍需会话和 CSRF。

SDK 提供 `classroom.discovery/invitations/invite/revoke/join` 及 generated operationId，
课堂请求拒绝非回环 HTTP、重定向和缓存。API 使用者仍需自行核实教师地址，DTO/兼容检查不是认证。

这是当前 Engine 的账号接入，**不是多学生内核隔离**。不要邀请不可信学生共享特权运行环境；
教师所有的无特权控制服务与独占学生 VM 仍是[目标架构](classroom-isolation.md)。发现学生自管
宿主不代表认可其权威。测试使用合成邀请、假的 SSH/部署脚本和浏览器拦截响应；真实局域网 TLS、
SSH 安装及 VM 隔离仍需在明确指定主机上完成验收。
