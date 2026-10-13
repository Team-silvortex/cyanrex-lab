# cyanrex-engine

用于认证、模块编排、事件流和 eBPF 实验的 Axum 后端。

组合入口按职责拆分：`application.rs` 持有 HTTP 和访问层级，`state.rs` 组装服务，
路由处理器调用 `services/` 下的领域服务。依赖和部署边界见[系统架构](../docs/zh-CN/architecture.md)。

## 已实现服务

- Cookie session、账户管理和 TOTP 验证
- PostgreSQL 持久化及内存可用性回退
- 版本化模块发现、有界清单校验、生命周期状态和命令分派
- 持久化的用户范围事件存储和 WebSocket 流
- eBPF 编译、加载、attachment 检查、事件收集和 detach
- `bpftool` 和 Aya 运行时后端
- 用户脚本存储和 C 头文件管理

## API

完整的版本化 OpenAPI 3.1 契约生成于 [`openapi/openapi.json`](openapi/openapi.json)，
并通过公开的 `GET /openapi.json` 提供。以下列出主要端点：

- `GET /`
- `GET /health`
- `GET /openapi.json`
- `POST /auth/login`
- `POST /auth/register`
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
- `POST /auth/logout`
- `POST /auth/password/change`
- `POST /auth/delete`
- `GET /modules`
- `POST /modules/start`
- `POST /modules/stop`
- `GET /events`
- `POST /command`
- `POST /ebpf/check`
- `POST /ebpf/complete`
- `POST /ebpf/run`
- `GET /ebpf/templates`
- `GET /ebpf/attachments`
- `GET /ebpf/attachments/details`
- `POST /ebpf/detach`
- `GET /helper/environment`
- `GET /scripts`
- `POST /scripts/save`
- `POST /scripts/delete`
- `GET /settings/events`
- `POST /settings/events`
- `GET /ws/events`

`ModuleManager` 发现含有 v1 `module.json` 的直接子目录。非法目录会让 Engine 启动失败；
生命周期端点不能插入未知名称。Start/stop 只是控制面状态，绝不执行模块目录中的文件。
设置 `CYANREX_MODULES_DIR` 可覆盖默认仓库目录或镜像目录。

## 开发凭据

- 用户名：`admin`
- 密码和 TOTP secret 在首次启动时生成于 `docker/.env`
- 使用 `CYANREX_ADMIN_PASSWORD` 和 `CYANREX_ADMIN_TOTP_SECRET` 提供显式值
- 绝不提交或发布 `docker/.env`

## 密码存储

- 密码不以明文存储。
- Engine 使用 Argon2 密码 hash，并可在迁移期间验证旧 hash。
- 持久化 session token 存为 SHA-256 digest。

## 持久认证表

- `users`
- `sessions`
- SQL 迁移模板：`engine/migrations/0001_auth_users_sessions.sql`

## 验证

Aya 面向 Linux 内核 API，因此应在 Linux 上运行 Engine 检查：

```bash
cargo fmt --manifest-path engine/Cargo.toml -- --check
cargo test --manifest-path engine/Cargo.toml --locked
```

同一 crate 也构建原生发布工具迁移目标 `cyanrex-release`。它的稳定命令覆盖完整 Tag 候选、
现用内核证据、验证后的双文件解包、解包后的软件包和 Docker 镜像验证，以及健康、
现用内核与 Runner Agent 验收：

```bash
cargo run --manifest-path engine/Cargo.toml --bin cyanrex-release -- candidate --help
cargo run --manifest-path engine/Cargo.toml --bin cyanrex-release -- evidence --help
cargo run --manifest-path engine/Cargo.toml --bin cyanrex-release -- package --help
cargo run --manifest-path engine/Cargo.toml --bin cyanrex-release -- smoke --help
```

修改路由或 wire model 后，从仓库根目录重新生成并验证 API 契约：

```bash
node scripts/generate-openapi.mjs
node scripts/openapi-contract.mjs
```
