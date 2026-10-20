# Linux 平台进程启动检查

源码 **0.6.3** 已收录显式回环／TLS 宿主、受限 Session 签发及私人 Task 创建／列举／重开／清单换版。
不接通浏览器／SDK 保存、新字节发布、局域网或存储迁移；早期实施阶段标签保留原范围与证据。

未发布的[回环宿主](platform-loopback-host.md)现于私有输入及运行时创建前复用这些门禁；
下文 `check-process` 诊断仍保持只检查行为。

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后、2026-10-15 的未发布跟进，在
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下增加显式 Linux 启动门禁。
`cyanrex-platform check-process` 核对已选定的服务身份，并限制自身 no-new-privileges 标志。
它不启动宿主、不读取秘密、不连接数据库，也不监听端口。
[Session 数据库权限](platform-session-permissions.md)仍是另一项要求。

## 显式身份策略

`PlatformProcessIdentity::new(uid, gid, groups)` 要求数字 UID／GID 和完整的预期补充组白名单。
零及 `u32::MAX` 被拒绝。补充组可以为空；非空时必须严格递增、唯一、非零且至多 128 项。
不查询账号名，不采用环境默认值，也不把当前用户的全部组自动收编为安全策略。
允许的组仍可能赋予重要文件／socket 权限，须由操作者审阅。

例如，**仅在这些数字已通过独立服务身份审阅时**运行：

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-process --uid 1000 --gid 1000 --groups 1000
```

明确要求空补充组时使用 `--groups none`。参数顺序固定；缺失、乱序、重复、多余、非规范、
超限或无效参数以 64 退出。进程状态不安全或不可读时以 1 退出，错误通用且不回显参数。
仅成功打印 `platform process check passed; no host started` 并以 0 退出。
非 Linux 构建拒绝检查，不采用弱化回退。

## 首个运行线程之前

`prepare_platform_process` 要求单线程 main，须在运行时工作线程、秘密、数据库连接及监听之前
调用。它从固定 `/proc/thread-self/status` 描述符接受至多 16 KiB 的记录，额外探测一字节以拒绝
超限。它核实文件系统为 procfs，拒绝缺失、重复或畸形必需字段，并交叉核对原生 UID／GID／
补充组及 no-new-privileges 系统调用。

实际、有效、保存及文件系统 UID／GID 全部须等于显式非零策略，补充组须精确匹配。
effective、permitted、inheritable 和 ambient capabilities 均须为零，当前 PID／TID 及单主线程
状态须匹配。非主线程或观察到额外线程时会在设置标志前拒绝。调用方仍须保持启动顺序；线程
快照不证明此前没有构造运行时、读取秘密、连接或监听，也不审计兄弟线程凭据，不能补救已经
运行的 Engine。

只有以上检查通过后，才设置并回读 `no_new_privs=1`，然后重新核对身份、能力及单线程状态。
[内核契约](https://docs.kernel.org/userspace-api/no_new_privs.html)阻止 exec 通过 set-ID／文件
capabilities 获得新权限，新创建线程继承该标志，且标志不可撤销。代码不自动降低或修复
UID／GID／组／能力。bounding set 可保持非零；它不是当前能力，也不替代四组清零及该标志。

返回的 `PlatformProcessStartup` 没有 Debug、Clone 或序列化。`verify_current_thread` 将当前
调用线程与原进程、原策略及 `no_new_privs=1` 重新比较，不提供持续约束或资源访问权。
独立命令重复准备，再核对一个新建线程，最后核对主线程后才报告成功。

后续[私有启动输入检查](platform-startup-input.md)增加 `verify_before_runtime`，额外要求单主线程。
独立 `check-config` 分支在私有文件读取前及解析后调用它；`check-process` 仍不读取秘密或启动宿主。

## 失败与部署限制

标志设置不可逆，后续失败仍可能保留设置；错误不恢复旧状态，也不允许自动重试。
应停止启动，并通过可信启动器调查。生产代码不会为了通过检查而清除继承的能力。

这不是沙箱或已部署宿主验收。继承描述符、同 UID 访问、补充组权限、用户命名空间、文件系统
访问及资源上限仍需独立部署策略。数据库安全传输、每条连接的物理目标固定、入口、Session
签发及 Task／浏览器连接仍是后续门槛。教学 Engine 行为及已有存储不变。

## 验证

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_process_startup_tdd
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_host::process_startup::tests
```

真实子进程覆盖无能力子进程、身份／组不匹配、通用参数错误、已有标志及父进程身份／能力／
标志／环境不变。正向夹具在 exec 前仅主动降低**自己的合成子进程**能力，生产代码不执行此步骤。
已有非主线程、多线程测试宿主被拒绝；纯状态测试独立覆盖线程数、保存／文件身份、各能力集、
上限、损坏及未知系统调用结果。合成 root／能力值只是策略拒绝测试，不是实际取得特权。
