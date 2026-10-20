# 共享绑定的平台登录与会话路由

**源码 0.6.3 已收录**的 `PlatformSessionHttpState` 与 `build_platform_session_router`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，在同一保留存储绑定上组合
原[登录](platform-login-http.md)和 [Session 生命周期](platform-session-lifecycle.md)适配。
组合路由仍未挂载到正常教学应用；显式[回环宿主](platform-loopback-host.md)及
[原生 HTTPS 模式](platform-loopback-tls-host.md)经独立[工作区组合](platform-task-index-connection.md)运行这三条和索引路由；本构造器仍只含 Session，不改教学认证或浏览器保存。

## 一份原绑定

`PlatformSessionHttpState::new(storage)` 消费真实 `PlatformStorageConnector`，例如
[声明式存储打开](platform-storage-open.md)返回的能力。不另接 Origin、Source、工作区、命名
空间、连接池或任意路由对。构造不访问数据库或文件系统，不是健康检查。

`with_capacities(storage, login_capacity, lifecycle_capacity)` 先核原 Origin 及两项容量，再构造
能力和 HTTP 预算。每项为 1–16，`new` 默认 4 和 4。原 Origin、目录／标记观察、Artifact 根
描述符及连接选项在私有 Arc 后共享，不重新打开、发现或替换原基线。

登录和生命周期仍是两种命令能力。`PlatformSessionConnector` 不新增登录方法，
`PlatformLoginConnector` 不新增查询／退出。原公开构造器仍消费存储，均无 Clone、Debug、
序列化或存储 getter；共享构造器和原 Origin 读取仅供 crate 内实现，不新增调用方权限。

每条命令仍新建连接并保留原事务内守卫。共享句柄不等于连接池或缓存 Session 授权；构造后
根或标记替换必须在两条命令路径都被拒绝。

两种能力使用同一已配置数据库角色。组合不检查或增加登录／退出所需的业务权限，只能查询的
窄角色仍然保持窄权限；缺失权限继续由原命令私有拒绝，构造成功不证明权限就绪或进程隔离。

## 路由与响应

构造器只合并内部无 fallback 的路由集，再安装一份私有 fallback 和响应头层。原独立完整
路由保持各自原来的 fallback 行为。

| 准确路由 | 方法 | 原操作 |
|---|---|---|
| `/platform/v1/session/login` | POST | 确认已有账号登录并投影专用 Cookie |
| `/platform/v1/session` | GET | 新鲜当前 Session 验证及最小认证状态 |
| `/platform/v1/session/logout` | POST | 准确 Session 退出，确认成功才清 Cookie |

其他路径不读凭据或正文，直接私有 404。已知路径保留原方法／Allow、查询、严格 JSON 或空
正文、Origin／marker 和 Cookie 校验。HEAD 保留私有头但没有正文。不包含教学、Task、
Artifact、注册、CORS 或执行路由。

登录仍须准确 Origin 和 marker，拒绝已有平台 Cookie。生命周期 GET 须 marker 和专用 Cookie，
若有 Origin 则须准确匹配；退出仍须准确 Origin。旧 `cyanrex_session` 和转发头都不能选择权威
或传输策略。

只有成功登录会签发原单个 sensitive、host-only Cookie；只有确认退出成功才按原名字和 Path
清除。HTTPS 保留 Secure，规范环回 HTTP 只省略 Secure。JSON 不含 token、账号、角色或可复用
资源权限。组合层不改错误、不重试命令，也不在已分派变更外新增超时。

## 独立准入通道

登录与查询／退出各有一份响应生命周期信号量。保持登录响应或耗尽登录速率预算，不占生命周期
名额；生命周期饱和不占登录名额。GET 与退出共用生命周期通道，因此保持 GET 响应仍可能暂时
阻止退出。

克隆及用其构造的路由共享两条通道和同一登录尝试上下文。登录仍在每 60 秒窗口内至多接纳
64 次合规头部请求，先计速率再申请登录名额，不退款、不因成功而清零。查询／退出不消费或
重置该计数，头部拒绝仍不计登录速率。另行构造的状态有独立且可相加的 HTTP 预算。

默认是四个登录名额加四个生命周期名额，不是全局四名额。它不限制服务器连接／总头部，不保证
公平或分布式滥用防护，也不是整机内存／工作线程配额。取消释放 HTTP 名额，不证明已分派密码
作业或远端 SQL 停止；丢弃一个状态句柄也不撤销其他存活克隆。

## 未知交付与后续宿主

新鲜 GET 结果仍是快照，不授权后续资源操作。服务端确认登录／退出响应不证明浏览器收到了
Cookie；取消交付可能留下已消费 OTP 和已签发 Session，或已删除 Session 但浏览器仍持有旧
Cookie。不自动补偿、重置 OTP、收编 token、重试或恢复。

共享不解决并发无 Cookie 登录交付或其他作用域同名 Cookie 污染，原
[Cookie 与未知交付限制](platform-login-http.md)继续生效。实际进程准备、可信私有启动输入及
宿主级入口限制仅由显式回环宿主组合。TLS 和 Cookie 安全域规划仍须独立落实；不新增正常
AppState、前端认证、OpenAPI 或 SDK 路由；独立工作区构造器只另加私人索引，Task 内容、Artifact 与浏览器保存仍未接通。

## 验证

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::session_router
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_router_tdd
bash scripts/test-platform-session-router.sh
```

默认夹具验证组合路由、拒绝和独立准入；其关闭 Source 生命周期夹具只证明私有后端失败，不是
成功授权。准确 SQL runner 要求一次性 `CYANREX_TEST_DATABASE_URL`，每个组合状态只打开一份
真实绑定，验证密码／OTP 签发、查询／退出、共享预算、原对象漂移、权限失败及取消。保持响应
再取消只证明交付取消前服务端已提交，不冒称丢失 COMMIT 确认、浏览器送达或局域网／TLS 验收。
