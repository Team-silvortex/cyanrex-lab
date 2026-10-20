# 显式平台登录 HTTP 适配

**源码 0.6.3 已收录**的独立 `SessionLoginHttpState` 与 `build_session_login_router`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，把
[保留绑定登录](platform-login-connection.md)投影成显式 HTTP 适配。
独立路由仍未挂载：不启动监听、不安装存储、不改教学登录，也不接通浏览器保存；显式
[回环宿主](platform-loopback-host.md)现经共享组合运行这些路由，局域网／TLS 和浏览器集成仍属后续。

独立[共享 Session 路由](platform-session-router.md)现将本适配器与查询／退出组合到一份原存储
绑定，不改变本独立路由的范围。

## 可信构造

`SessionLoginHttpState::new(connector)` 消费一个 `PlatformLoginConnector`，以 Arc 长期保留。
`with_capacity(connector, capacity)` 接受 1–16 个响应生命周期名额，默认 4。Origin 只取原连接器；
请求、环境变量、教学 Cookie、连接池 Source 回退和转发地址均不能选择后端或 Cookie 策略。

状态克隆及用其构造的路由共享连接器、速率计数和名额，另行构造的状态有独立 HTTP 预算。
原 [Session 生命周期](platform-session-lifecycle.md)仍只查询／退出，对登录返回 404，
不会因同用路径前缀而取得签发权。

## 请求契约与顺序

只接受准确的 `POST /platform/v1/session/login`，拒绝查询、别名和尾斜杠；其他方法返回 405，
带 `Allow: POST`。HEAD 保留私有响应头但没有正文。

处理器依次执行：

1. 唯一且准确的 `x-cyanrex-platform-request: 1` 和配置 Origin，不以 Referer 回退；拒绝
   Authorization。所有 Cookie 头合计至多 8 KiB。出现任何 `cyanrex_platform_session`，包括空、
   畸形、重复或只有名字的形式，都返回 `409 session_present`，不核旧会话、不读正文、不碰存储。
   无关 `cyanrex_session` 可共存，但绝不选择、改写或清除。
2. 每个状态以单调时间、单一固定计数器，在 60 秒窗口内至多接纳 64 次合规头部请求，跨用户名和
   客户端共享。之后立即申请并发名额，不排队。已计数的忙碌、坏正文、取消及成功请求均不退款，
   登录成功不清入口计数；头部拒绝不消费这两类预算。
3. 在两秒内收集至多 32 KiB 实际流式 JSON，拒绝 Content-Encoding、不支持或重复的 Content-Type、
   trailers、正文错误和不结束的空帧；不能用 Content-Length 替代实际字节或 EOF。
4. 严格解析只含 `schema_version: 1`、`username`、`password`、`otp` 的对象。拒绝数组、重复／
   未知键、尾随 JSON 和错误类型。用户名遵守原 `LegacyUsername` 契约；解码后密码至多 4096
   字节、OTP 至多 64 字节，HTTP 层不修剪或归一化凭据。
5. 恰好调用一次保留绑定连接器，原事务、OTP 和存量策略继续生效。已分派登录外不再套 HTTP
   处理器超时、不自动重试，也不在确认成功后追加验证，直接投影响应。

状态级限流只是一种有界资源策略，不是按客户端公平服务、分布式滥用防护或 HTTP 服务器的
连接／总头部限制。一个客户端可以耗尽共享预算；更换名字、X-Forwarded-For 或 Forwarded
不会获得新桶。

## 确认后的 Cookie 交付

确认成功只返回 `200 {"schema_version":1,"authenticated":true}`，恰好发一个标记为 sensitive 的
Set-Cookie；JSON 不包含 token、用户名、角色、有效期或数据库信息：

```text
cyanrex_platform_session=<new-token>; Path=/platform/v1; HttpOnly; SameSite=Strict; Secure
```

符合既有 localhost 策略的规范环回 HTTP 只省略 Secure。策略取自固定 Origin，不读取 Host 或
代理头。不设置 Domain、Max-Age 或 Expires：这是浏览器会话 Cookie，服务器仍执行原十二小时
Session 有效期。清除继续使用既有生命周期命令确认后的同名、同 Path 策略。

这里拒绝覆盖请求已携带的 Cookie，不保证所有并发交付都不覆盖。两个同时发出的无 Cookie 登录
仍可能各自签发；响应顺序不是幂等或跨标签协调。切换规范已有会话时须先显式退出。畸形或其他
作用域的同名 Cookie 污染可能需要明确清理浏览器；适配器不猜测 Path／Domain 来清除。

Cookie 请求头不携带原作用域属性，Cookie 也不按端口隔离；不同 localhost 端口不是独立的
Cookie 安全域。参见 [RFC 6265 第 4.2.2 与 8.5 节](https://www.rfc-editor.org/rfc/rfc6265#section-8.5)。

## 失败与未知交付

连接器分派前的错误沿用私有形状，标记 `not_attempted`。所有连接器错误均为 `unconfirmed`：
坏凭据映射 401，速率／存量拒绝为 429，其余认证源／存储失败统一 503。任何失败都不设置或
清除 Cookie，不回显凭据、不提供 Retry-After 或 CORS。响应均保留 no-store、no-cache 与
nosniff；后端若返回坏 token 也拒绝交付，不暴露其内容。

名额覆盖已接纳正文收集、后端等待及未发送的响应，直到正文完成／丢弃；它不追踪请求取消后
已分派的密码作业。连接器仍有十秒工作预算、独立最终关闭等待，以及不可抢占的同步文件系统
观察；两秒正文期限不是整个请求的截止时间。

服务端成功不证明浏览器已收到或应用 Cookie。提交确认后取消可能留下已签发但未交付的
Session 和已消费 OTP；不能由此推断回滚、自动重登、收编存储 token、重置 OTP 或补偿删除会话。

## 验证与后续宿主工作

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::session_login
cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_http_api_tdd
bash scripts/test-platform-login-http.sh
```

默认私有传输夹具验证头部／正文／顺序／速率／准入／响应行为，不证明真实授权。准确 SQL
runner 只使用显式一次性 `CYANREX_TEST_DATABASE_URL`，经真实受限角色、密码／OTP 和保留绑定
存储验证。保持响应再取消的用例证明取消前已确认提交，不冒称丢失 COMMIT 确认、浏览器送达
或已部署 TLS。

宿主启动、真实 TLS／入口限制、Cookie 安全域规划和独立平台浏览器认证上下文仍待完成。
不能只把旧 `/auth/login` 换成这个 URL，原 Sidebar 还依赖旧角色。Task／Artifact／索引接线、
UI／SDK 保存及重启恢复仍独立；本适配器不修改在线路由、OpenAPI 或 SDK 契约。
