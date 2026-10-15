# 平台 Session 生命周期

于 **2026-10-15** 收录至 **0.6.0** 源码，见[发布验证](../../reports/releases/0.6.0/README.md)。
下列日期说明保留原实施范围，不是新的部署验收。

2026-10-13 的 0.5.9 之后未发布跟进新增显式、**未挂载**的 Session 生命周期适配器。
它能查询当前持久 Session 并确认退出，不签发 Session、不启动平台宿主，也不改变教学应用认证。
后续阶段依据[独立低权限平台宿主提案](https://github.com/Team-silvortex/cyanrex-lab/issues/3)推进。

这一边界面向未来的同源平台宿主。构造状态既不连接存储，也不初始化存储；可信组合方必须提供
已准备的 `DurableAuthSource` 和一个规范 HTTPS Origin。开发时可显式使用规范环回 HTTP，
但这不允许局域网明文部署。

## 精确操作

| 请求 | 确认成功的响应 | Cookie 效果 |
|---|---|---|
| `GET /platform/v1/session` | `200 {"schema_version":1,"authenticated":true}` | 无 |
| `POST /platform/v1/session/logout` | `200 {"schema_version":1,"ok":true}` | 仅清除专用平台 Cookie |

GET 调用原 Source 所有的 `validate_session`。缺失、过期、撤销或同名重建账号均不能让旧 Session
获得认证。响应不包含账号、角色、到期时间、token 或资源。已认证快照不是可复用的 Task、Artifact
或文件权限；后续每次资源操作仍须经过原事务持有的授权边界。

POST 直接调用原 Source 的 `logout`，不先另做一次认证预检。Source 也可确认规范未知 token 的
缺席。只有确认成功才允许清 Cookie；不撤销所有设备 Session，也不清除旧教学 Cookie。

精确已知路径拒绝任何查询参数。额外路径段和尾斜杠不是别名。错误方法返回 `405` 及
`Allow: GET` 或 `Allow: POST`；HEAD 保留私有响应头，正文为空。

## 浏览器与传输边界

请求须具有唯一且精确的 `x-cyanrex-platform-request: 1`，以及 `cyanrex_platform_session` 中
唯一的规范非零 UUID token。重复的平台 Cookie 值、畸形 token、Bearer 凭据或仅携带旧
`cyanrex_session` 都会被拒绝。Cookie 头字节继续使用原 8 KiB 预算；旧 Cookie 可以共存，
但不被选中或清除。

POST 须提供与配置精确一致的 Origin。GET 可不提供 Origin，但提供时必须精确匹配。
Referer 不是 Origin 回退。两种请求正文必须真正为空，沿用原两秒收集期限；编码正文、不兼容
媒体类型、尾部帧和正文故障在 Source 操作之前失败。没有 CORS 授权或重定向回退。

HTTPS 退出确认后只返回一个仅限本宿主的清除响应头：

```text
cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure
```

显式环回 HTTP 仅省略 `Secure`，不设置 Domain。未来签发器须采用相同名称和 Path 策略；
本适配器不提供签发器。服务端响应成功不证明浏览器已经收到或应用该清除响应头。

全部响应使用 `Cache-Control: no-store`、`Pragma: no-cache` 和
`X-Content-Type-Options: nosniff`，不提供 CORS、Location 或 Retry-After 响应头。

## 故障和容量语义

错误保留私有格式 `{"schema_version":1,"error":{"code":...,"outcome":...}}`。
派发前拒绝标记为 `not_attempted`。Source 确认 Session 无效时返回 `401 invalid_session`；
Source 故障返回通用 `503 service_unavailable`。两种派发后错误均保守标记为 `unconfirmed`，
不清 Cookie，也不暴露存储诊断。不确定结果不证明回滚，也不授权自动重试退出。

凭据检查先于立即拒绝式容量准入，准入先于正文收集和 Source 工作。状态克隆共享默认四个名额，
只允许显式配置 1–16。名额覆盖已准入请求和未送出的响应正文，直到正文结束或丢弃。
另行构造的新状态具有独立预算；这不是全局 Session 存量限制、SQL 工作线程预算或送达证明。
取消请求不证明其数据库事务已经回滚。

## 验证

`engine/tests/platform_session_http_tdd.rs` 包含八项默认传输回归和七项须显式启用的一次性
PostgreSQL 用例。SQL 用例覆盖当前／无效 Session、保留另一设备的精确退出、HTTPS／环回清除、
被抑制或重定向的 DELETE、实际行锁等待期间过期、DELETE 锁超时、响应容量和取消。
等待依据真实数据库阻塞证据；随机新命名空间在最终行为断言之前清理。

默认目标不需要数据库配置：

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_http_tdd
```

SQL 套件须显式提供一次性 `CYANREX_TEST_DATABASE_URL`，再运行
`./scripts/test-platform-session-http.sh`。不要把故障注入测试指向应用数据。
运行器执行前核对每个精确测试名称，不回退到 `DATABASE_URL`。
测试及源码清单不等于服务已经挂载或局域网部署已安全验收。

## 尚未接通的宿主与签发工作

正常应用挂载、登录／注册、秘密交付、UI／SDK／OpenAPI 变化、数据库安装或迁移及隐式清理
过期 Session 均未加入。现用 AuthService、内容路由、Source Schema 和原事务防护不变。

独立宿主还需要真正低权限的进程、明确数据库角色和命名空间、本人私有 Artifact 存储及安全
同源局域网传输。对外 Session 签发还须具备入口／速率策略、活动 Session 存量限制、时序策略，
以及安全的不确定提交／秘密交付协议。通过这些门槛之后，才接通私人 Task 保存、列表、重开
和修订冲突处理。

后续[显式 Session 存量策略](durable-session-stock.md)在可信受限登录中限制持久行数，包含
过期记录。不将本路由变为签发器，不向原准备层写者强制施加策略，也不完成剩余宿主和
签发门槛。
