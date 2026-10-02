# ADR-011：受控的本地权限实例初始化

状态：**C1-J 内部准备层，收录于 0.4.7**。在
[C1-I 原子引导](collaboration-bootstrap.md)之上增加原生 `cyanrex-provision` 运维入口。
它让全新权限实例可以经过核对后显式初始化；不开放匿名设置接口，不切换运行中的 Engine，
不迁移已有账号，也不把首个公开注册者提升为教师。

## 运维信任边界

仅用于显式准备的**隔离测试数据库与全新具名 Schema**。Schema 须预先存在，且不含任何关系、
函数或类型。`public`、系统 Schema、多重搜索路径、缺失 Schema、已有数据与部分安装都会拒绝。
CLI 不创建/删除 Schema，不接管或修复旧安装。在线认证与 CSRF 接入尚未完成，不能让旧
AuthService 共用这些已经启用准备层的表。

工具依赖 Unix 文件语义，面向 Linux/WSL2；只接受本机 PostgreSQL socket 或字面量
`127.0.0.1`/`::1`。TCP 使用显式端口且不启用 TLS，仅供可信本机端点使用，也可连接用户另行
建立并验证的 SSH 转发；不支持远端主机名/DNS，也不自动建立隧道。选择端点不等于认证服务器，
必须独立核对 socket/转发、数据库和 PostgreSQL 角色。物理目标指纹用于防误操作，不是证书，
也不能防御恶意数据库管理员或拥有相同标识的数据库克隆。

连接配置全部来自私有 JSON 文件。独立进程在启动 Tokio 前清除继承的 `PG*`，使用不读取 pgpass
的 SQLx 构造方式；不读取 `.env`、`DATABASE_URL`、`CYANREX_*`、Engine 状态或旧 home 账号配置。
CLI 不记录 SQL 语句。数据库服务端日志、交换分区、进程内存、已失陷的系统/root 和同一用户身份的
其他进程不在这层密钥交付保证之内。

数据库角色需要新 Schema 的 USAGE/CREATE，以及执行 `pg_catalog.pg_control_system()` 以读取
系统标识的权限；由可信数据库管理员预先准备，CLI 不给自己加权。新二进制目前从源码通过 Cargo
使用；发行打包、启动器、Compose 与服务启动流程不变。

## 准备与核对

使用 `cargo build --manifest-path engine/Cargo.toml --locked --bin cyanrex-provision` 编译；也可将
`cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-provision --` 放在下列
命令参数之前。构建或查看帮助不会初始化数据库。

用可信本地编辑器，在当前用户所有的 `0700` 目录中准备文件。输入须为当前用户所有的普通文件，
无组/其他用户访问权限、无特殊权限位且只有一个硬链接，通常为 `0600` 或只读 `0400`。路径须为
绝对路径，各级目录所有权可信，不遍历软链接或 `..`。配置最多 16 KiB。以下仅为合成示例：为实际
authority 和 Workspace 选择并保留新的规范 UUID，填写目标数据库凭据；不要提交文件或粘贴到日志。

```json
{
  "format_version": 1,
  "transport": { "kind": "unix", "directory": "/var/run/postgresql" },
  "port": 5432,
  "database": "cyanrex_staging",
  "database_user": "cyanrex_operator",
  "database_password": "<数据库口令；使用 peer 认证时填空字符串>",
  "schema": "c1_trial",
  "authority_id": "1223c6e9-823d-4cea-998a-a92f019b35fc",
  "workspace_id": "60a9845d-0687-4f19-a4f5-2ebf64bed4ba",
  "username": "first-teacher"
}
```

已验证的本机 TCP 端点可将 `transport` 改为
`{"kind":"loopback","address":"127.0.0.1"}`。数据库/角色名限定为有界 ASCII 标识；Schema 名
使用小写字母、数字和下划线，不能以数字开头。未知/重复 JSON 字段、不支持的格式版本或非法 ID 均拒绝。

```bash
cyanrex-provision plan --config /absolute/private/target.json
```

`plan` 和 `inspect` 使用只读、可重复读的目录事务。计划展示端点、数据库/角色/Schema、
authority/Workspace/用户名、实际系统/数据库/Schema 标识和固定初始策略：活动的所有者/教师
成员关系、显式部署 Grant、两类审计基线，且**不签发 Session**。仅空命名空间且具备 Schema 权限
时返回 SHA-256 `confirmation`。指纹绑定这些非敏感目标及策略，不包含口令。计划和指纹都不是
命名空间预留，也不是授予非可信调用者权限的令牌。

## 显式执行与密钥交付

另行创建私有密码文件，内容为 8–4096 字节 UTF-8。末尾单个 LF/CRLF 视为文件分隔符；保留空格，
拒绝中间的换行、回车或 NUL。人工核对计划后，将指纹复制到显式命令中；不要把计划和执行直接接成无人值守管道。

```bash
cyanrex-provision apply --config /absolute/private/target.json \
  --confirm <已核对的64位十六进制指纹> \
  --password-file /absolute/private/password \
  --enrollment-file /absolute/private/NEW-enrollment.json
```

执行会重新核对实际目标、空状态与确认指纹。连接池更换连接时再次核对物理目标；C1-I 仍是事务
权威，会在三类安装器锁后重新确认命名空间为空。目标变更或 Schema 已被占用都不能初始化账号。
多个进程竞争至多一个成功；计划不绕过数据库的即时检查。

调用前先在当前用户所有的 `0700` 父目录下排他创建 `0600` 文件，将**不含密钥的待对账标记**与
目录同步落盘。不接管或覆盖已有路径、软链接、硬链接、非普通文件或权限宽松的输出父目录。
写入时核对文件和父目录描述符是否仍对应可见路径。

只有 C1-I 确认 COMMIT 后，已持有的文件才写入账号/Principal 坐标及 TOTP 注册材料；文件和目录
同步成功后才报告成功。终端 JSON 只返回 `committed_enrollment_saved` 与非敏感坐标，绝不包含
数据库口令、账号密码、TOTP 密钥、注册 URI 或 Session 令牌。将私有注册材料导入目标认证器，
并按密钥保管政策保存；仍需正常密码/TOTP 登录，这不会把在线 Engine 切换到新账号。

## 失败处理与检查范围

数据库提交和文件交付**不是一个原子事务**。CLI 不自动删除输出标记、重试初始化、重设凭据或补建管理者。

| 结果 | 含义 | 后续动作 |
|---|---|---|
| 输入/预检错误，退出码 2 | 尚未调用引导；输出预留失败可能留下空/部分标记文件 | 修正显式输入，重新核对计划；保留已有文件供检查 |
| `bootstrap_unconfirmed`，退出码 4 | 已调用引导，但提交未确认；标记不证明回滚 | 保留证据，先对账准确目标，再决定是否显式重试 |
| `committed_delivery_unconfirmed`，退出码 3 | 已确认数据库提交，但文件交付或最终终端响应失败 | 不得重建；保留完整/部分私有输出并核对交付状态 |
| 成功退出码 0 | 只读观察成功，或 `apply` 已确认提交且保存注册材料 | 按具体命令状态理解，不等于在线部署验收 |

```bash
cyanrex-provision inspect --config /absolute/private/target.json
```

检查仅报告目录对象占用与物理目标标识。`empty` 只是读取时的视图，可能还有未提交/执行中的
初始化；`occupied` 不证明权限实例健康，也不能证明哪个命令提交、谁操作或密钥已交付。
它不读取/重发存储的 TOTP，不校验完整审计图，不修复数据，也不提供幂等回执。不确定写入发生后，
可信维护流程须先确认没有操作仍在执行，再核对来源、注册表和审计的实际状态。重复初始化无法找回
丢失密钥。[C1-K](collaboration-reconciliation.md)已提供 `reconcile`，有界只读核对来源、注册表
和完整审计图；不证明密钥交付、命令结果或允许重试。注册材料恢复、经审阅的迁移/还原仍是后续
工作；不能通过删表、改 ID 或移除证据来强行重试。

## 验证

[CLI 测试](../../engine/tests/provision_cli_tdd.rs)覆盖参数/配置拒绝、私有有界输入、软硬链接/FIFO、
存储不可用、终端脱敏及无运行时降级。[PostgreSQL 测试](../../engine/tests/provision_postgres_tdd.rs)
覆盖九个显式场景：只读计划/检查、提交后的注册材料及登录、过期目标确认、已有/部分安装、危险输出、
进程竞争、锁超时、提交后路径替换和短写、进程取消。2026-10-02 本地隔离 PostgreSQL 16 上，8 条默认
用例和 9 条真实数据库用例全部通过；CI 按精确名称选择每条数据库用例。
同一组测试也通过回环 TCP 与 SCRAM 口令认证验证。使用数据库强制只读事务的非超级用户时，
计划/检查仍成功，尝试执行则不创建账号。

C1-I 的数据库级故障钩子仍通过测试专用 advisory guard 隔离。短写测试只对测试子进程设置文件大小
限制，没有生产故障注入开关。完整开发回归统计见[项目状态](project-status.md)。这些结果不表示已验收
线上数据库、前端生产构建、迁移、部署、远程 CI 或特权内核路径。
