# 私有平台启动配置检查

**2026-10-16 未发布**的[格式 3](platform-marker-expectation.md)同时要求完整目录及五标记身份
声明。格式 1／2 保留原诊断语义，均禁止 `marker_identity`，包括 null；原私有文件门禁及
16 KiB 限制不变。

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后、**2026-10-15** 的未发布跟进，在
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下增加仅 Linux 的
`check-config`。它在[进程门禁](platform-process-startup.md)通过后读取显式私有 JSON，校验
格式及已有纯配置契约，再丢弃输入；不连接数据库、不打开配置的 Artifact 根、不创建工作线程，
也不启动宿主。

## 执行检查

仅在已独立审阅服务数字身份及完整补充组策略后使用：

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-config --uid 1000 --gid 1000 --groups 1000 --config /private/platform/input.json
```

空补充组须明确写 `--groups none`。参数顺序固定；缺失、乱序、重复、多余参数、畸形身份及
相对配置路径以 64 退出。进程、文件或配置失败均以 1 退出，使用同一通用错误，不回显路径、
值、解析器或系统原始错误。成功以 0 退出，仅打印
`platform configuration check passed; no host started`。

进程门禁先于文件打开。新增的 `verify_before_runtime` 要求原身份、当前能力清零、
no-new-privileges 及单主线程，在读取前及解析后各检查一次，包括解析失败时。这些快照不证明
此前没有工作线程、秘密、连接或监听；独立的 `check-process` 保留原工作线程检查。

## 私有输入文件

显式绝对 UTF-8 路径最多 4096 字节，拒绝空段、点／父目录段、重复／末尾分隔符及控制字符。
逐级描述符相对遍历使用 `O_NOFOLLOW`。祖先须由 root 或有效 UID 拥有，且不可被组／其他用户
写入；root 所有的 sticky 祖先（如 `/tmp`）只允许出现在直接父目录之前。直接父目录必须由
有效 UID 拥有，权限准确为 `0700`。

叶子须为有效 UID 所有、单硬链接的普通文件，权限准确为 `0400` 或 `0600`，最多 **16 KiB**。
非阻塞打开避免 FIFO 等待写端，读取前再检查类型及元数据，并额外探测一字节。读取后比较
设备／inode、身份、权限、链接数、长度及修改／变更时间；重新无符号链接遍历可见父目录和
叶子，与原持有描述符比较。

命令不改权限、不创建输入路径，也不修复不安全文件。它不阻止同 UID 或 root 竞争，元数据
比较不是并发锁；同步文件 I/O（包括 NFS／FUSE）没有硬耗时上限。丢弃秘密缓冲区不等于安全清零。

## 必需 JSON 对象

每层记录都必须是对象且字段完整，拒绝未知／重复字段、位置数组、错误类型、无效 UTF-8 和
尾随 JSON 内容。版本 1 格式如下；这些是合成值，不是发现或已批准的部署坐标：

```json
{
  "format_version": 1,
  "origin": "https://platform.example:8443",
  "authority_id": "00000000-0000-0000-0000-000000001069",
  "workspace_id": "00000000-0000-0000-0000-00000000106a",
  "namespaces": {
    "source": "source_private", "task": "task_private", "artifacts": "artifacts_private"
  },
  "database": {
    "database": "platform_db", "role": "platform_runtime",
    "system_identifier": 123, "database_oid": 456,
    "transport": {"kind": "loopback", "address": "127.0.0.1"},
    "port": 5432, "password": "replace-with-explicit-database-password"
  },
  "session_limits": {"per_account": 4, "total": 16},
  "artifact_root": "/private/platform/artifacts"
}
```

传输也可写为 `{"kind":"unix","directory":"/private/postgres"}`。
复用[本地端点契约](platform-database-connection.md)：端口非零，传输为字面量环回 IP，或完整
socket 文件名不超过 100 字节的规范 Unix 目录。密码字段必需，可显式为空，最多 4096 个
UTF-8 字节且无 NUL。

原校验器检查规范可信 Origin、非零 authority／workspace UUID、三个不同私有命名空间、
显式物理数据库坐标和 Session 上限。`artifact_root` 仅检查规范绝对 UTF-8 路径语法，不存在
也可通过；不证明目录身份、存储标记、角色权限、审计历史或数据库可用。

此检查不读取或修改 PG 变量、pgpass、服务文件或 `.env`，也不构造 SQLx 选项。例如 PGOPTIONS
污染不影响本次纯配置检查，但另行构造的连接器仍拒绝捕获到的 PGOPTIONS。成功不会产生原始
目录观察，也不允许收编首次连接；可信基线取得、数据库认证、Artifact 根检查和实际宿主组合
仍须单独推进。

## 版本 2 的显式目录声明

[版本 2](platform-catalog-expectation.md)保留版本 1 全部字段，并要求 `catalog_identity`，
包含角色 OID、数据库所有者 OID，以及每个命名空间的 OID／所有者 OID。
版本 1 禁止此字段，即使值为 null；版本 2 不发现或补齐缺失身份。新增记录同样严格要求
对象形式、拒绝重复／未知字段，并计入 16 KiB 总限制。`check-config` 只校验声明，
不认证或观察对应对象。

## 内部模式归属

2026-10-16 的内部整理统一严格记录的缺失／提供解码，只选择一种私有存储检查模式：
版本 1 当前诊断、版本 2 目录声明，或版本 3 目录及标记声明。不增加 JSON 字段、不放宽拒绝
规则，也不在解析时执行 I/O。声明记录负责构造强类型期望，只有选择器持有版本／存在性
规则；CLI 与库继续分别拥有 runtime、检查和关闭职责，详见
[平台准备层职责](architecture-map.md#平台准备层职责)。

## 验证

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_config_input_tdd
cargo test --manifest-path engine/Cargo.toml --locked --test platform_process_startup_tdd
```

默认测试覆盖严格记录、字节边界、真实私有文件、通用错误及旧进程命令兼容。具有等待上限的
子进程夹具只清除自己的能力，生产不执行此操作。带阳性对照的 inotify 检查错误 UID 在打开
所给 FIFO 前被拒绝，以及被污染的密码／服务／`.env` 文件未读。夹具在最终断言前回收自有
子进程与路径，并保持父进程身份、标志及环境不变。这些只是本地配置检查，不是数据库、TLS、
已部署局域网、Session 签发或浏览器保存验收。

后续独立的 [check-storage 命令](platform-storage-check.md)消耗版本 1、2 或 3 校验输入，执行
一次只读根／数据库诊断；版本 2 先比较显式目录声明，版本 3 还在行读取前核原五标记身份。
`check-config` 仍直接丢弃输入，
不打开这些目标；两者都不建立保留的
宿主基线或启动宿主。
