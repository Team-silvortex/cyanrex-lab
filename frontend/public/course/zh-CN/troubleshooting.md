# 故障排查手册

源码 **0.6.3** 已收录显式回环／TLS 宿主、受限 Session 签发及私人 Task 创建／列举／重开／清单换版。
不接通浏览器／SDK 保存、局域网或存储迁移；早期实施阶段标签保留原范围与证据。

Unreleased [原绑定文本发布](platform-text-artifact-connection.md)由两个 CLI 宿主显式选择（八路由／七通道）；
原工作区构造器仍保持七路由／六通道。

先确认所用路径：现有教学运行时、本地任务草稿，还是显式初始化的协作准备层。三者的持久化与授权
契约不同，详见[当前功能地图](platform-network.md)。尚未实现的功能不等于部署故障。

## 先判断失败在哪一层

| 现象 | 优先检查 |
|---|---|
| 页面打不开 | Frontend 容器、端口、SSH 隧道 |
| 登录失败 | `docker/.env`、系统时间、TOTP |
| clang unavailable | Engine 健康状态、账户权限、网络 |
| compile 失败 | C 语法、头文件、类型、宏 |
| load 失败 | verifier、BTF、权限、内核能力 |
| attach 失败 | hook 名称、bpftool 版本、tracefs |
| 运行成功但无事件 | 触发条件、采样、事件过滤、读取通道 |
| 卸载不干净 | attachment 列表、pin path、Engine 日志 |

## 任务草稿与协作准备层

| 现象 | 含义与安全处理 |
|---|---|
| 导航或重启后任务草稿丢失 | 草稿只在页面内存中，不是服务端记录。离开前显式下载重要内容；尚无自动保存/恢复 |
| Rust/Python/C++ 没有类型错误或项目导航 | 这些本地配置只有高亮/片段，不应按 clang 故障排查，也不会安装语言服务器后自动接入 LSP |
| 下载成功但服务端没有 Task | 下载只是本地导出，公共任务保存尚未接通 |
| 准备层命令返回过时修订 | 保留编辑内容，重新授权读取当前记录，不猜测修订号强制覆盖 |
| Task 存储不支持当前 Schema | Schema 2 要求全新专用安装。保留旧数据，不改元数据、不删表，也不让在线旧写入者共用该空间 |
| Artifact 发布后任务换版失败 | 已发布不可变版本可能仍存在，不能删文件、接管内容或自动重放命令 |
| 教师不能读取其他成员的通用 Task | 私人准备层 API 从 Session 推导所有者，课堂教学权不等于跨用户资源授权 |

受支持操作见[编辑器](editor.md)、[换版契约](session-task-revisions.md)和
[初始化指南](collaboration-provisioning.md)。遇到这些情况，不应对现用部署执行迁移或故障注入测试。

## 服务与日志

```bash
./start.sh status
./start.sh diagnose
./start.sh logs engine
./start.sh logs frontend
docker compose -f docker/docker-compose.yml ps
```

`./start.sh logs` 跟随全部服务日志，追加服务名可只查看该服务。Ctrl-C 只退出日志查看，不停止
服务。状态／日志查看不再占用启动锁，打开日志流不会挡住同实例停止。启动与停止仍串行；
并行查看的状态只是观察，不证明启停已经完成。

`--debug` 输出安全提示，不再追踪 shell 命令和凭据赋值。不要用 `bash -x` 生成准备分享的启动
日志；工具或服务自身仍可能输出敏感信息，分享前需要审阅。

源码启动器的实例名、端口和绑定地址统一按“显式 CLI 参数 → 非空继承环境变量 → `docker/.env`
→ 内置默认值”选择。停止、状态、日志和两种诊断入口与启动使用相同规则；操作该实例时需重复
相同 CLI／环境覆盖，启动器不会记忆上一次目标。
项目名固定为 `cyanrex-<instance-id>`，不接受 `CYANREX_COMPOSE_PROJECT` 覆盖。实例名须为
1–63 个小写字母、数字、下划线或连字符；非法值直接报错，不再静默清洗成另一实例。
例如可运行 `./start.sh diagnose --instance-id room-a --engine-port 18080`。

这五个非秘密目标字段接受独立的单行字面赋值，支持可选 `export`、外层单双引号、空白、行尾注释
和 CRLF，重复键取最后值；不支持动态表达式和多行值。只读命令不会执行 `.env` 或生成凭据。
启动仍将其余配置作为可信 shell 文件，在已选实例下执行；后续若改写已锁目标则拒绝部署。
默认数据库卷表达式会跟随最终实例，显式自定义卷仍需自行保证不同实例不共用。
Compose 显式接收 `docker/.env` 的绝对路径，不受调用目录或 `COMPOSE_ENV_FILES` 选出的其他文件影响。
Compose 须支持 `--env-file` 和仓库内模板；过旧版本不会回退成隐式配置。`.env` 仍须符合 Compose
dotenv 语法，启动时求值不意味着支持任意 shell 程序。

直接运行 `./scripts/debug-system.sh` 时，缺失可选配置不会中断诊断，Compose／HTTP 检查失败后
仍继续。单次环回健康请求忽略 curl 用户／代理配置，连接限时
3 秒、总请求限时 10 秒，非法端口直接跳过；其他工具不受此 HTTP 期限约束。

健康检查：

```bash
curl -q --noproxy '*' -fsS --connect-timeout 3 --max-time 10 http://127.0.0.1:8080/health
```

离线包安装实例可先在包目录执行 `./cyanrex-release package verify-controls .`，再与可信的同版
副本比较控制文件。它只核元数据及三个部署控制文件，不读 `.env` 或镜像正文，不运行 Docker／
SSH 或变更服务；不是全包、配置、健康或签名验证。包内使用 `./deploy.sh status` 和
`./deploy.sh logs engine`，而不是源码目录的 `start.sh` 路径；包内 `README-DEPLOY.md` 提供日常运维步骤。

浏览器环境助手要求显式发起新检查。再次检查会先清除旧结果；失败、不完整或超时不会保留
之前的成功就绪显示，刷新页面也不恢复旧报告。更换实例或会话后应重新检查；显示结果只是
观察快照，不表示已经部署或服务持续健康。
10 秒的浏览器等待期限不保证 Engine 检查或子进程已经停止。

Docker 发布端口默认回环绑定，native/WSL 启动目前设置 `ENGINE_HOST=0.0.0.0`。检查真实监听并
限制局域网入口；远程访问优先使用经核对的 SSH 隧道，不直接开放未保护的 `SERVER:3000`，详见[安全指南](security.md)。

## 登录与 TOTP

确认：

- 使用的是当前 `docker/.env`，不是旧截图或 README 示例；
- 手机和服务器时间正确；
- 没有连续输错 5 次；连续失败会锁定账户 5 分钟；
- `CYANREX_ROTATE_ADMIN_CREDENTIALS` 已恢复为 `false`。

不要在聊天、Issue 或课堂投屏中展示 `.env`。

## clang 实时检查

状态一直为 `unavailable` 时：

1. 确认当前会话已登录；学生也可使用检查，不需要教师权限；
2. 检查 Engine 健康状态；
3. 确认源码没有超过 256 KiB；
4. 查看 Engine 日志中 clang 是否存在；
5. 等待其他编译任务完成，系统最多并发处理两个任务。

语义补全会调用后端 clang。网络短暂失败时，本地 snippets 仍然可用。

## BTF 与 vmlinux.h

环境助手中 `kernel_btf` 或 `btf_dump` 失败时检查：

```bash
ls -l /sys/kernel/btf/vmlinux
bpftool btf dump file /sys/kernel/btf/vmlinux format c >/dev/null
```

Docker 模式需要把宿主/虚拟机内核的 BTF 暴露给 Engine。没有 BTF 时，依赖 `vmlinux.h` 的
CO-RE 示例无法工作，但只使用稳定 UAPI 头文件的简单程序仍可能运行。

## bpffs 与权限

```bash
mount | grep /sys/fs/bpf
ls -ld /sys/fs/bpf
ulimit -l
```

不要为了绕过错误手工 `chmod 777 /sys/fs/bpf`。应修复启动方式、挂载和容器能力配置。

## 自动挂载不可用

旧版 bpftool 可能没有 `autoattach`。Cyanrex 会尝试手动 tracepoint attach。
如果程序类型需要明确的网络接口、cgroup 或其他 target，教学系统可能只能完成 load，
不能自动选择正确挂载目标。此时应根据实验说明提供 target，而不是随机挂载。

## 有程序但没有事件

依次确认：

1. attachment 列表中确实有程序；
2. 执行了能触发 hook 的操作；
3. 运行时间尚未结束；
4. 采样率没有过低；
5. Events 页面过滤条件正确；
6. Ring Buffer 结构与读取端预期一致；
7. 没有因为 Ring Buffer 满而持续 reserve 失败。

## 最后的清理

先在页面执行“全部卸载”。如果 Engine 异常退出，重启 Engine 后检查 attachment 和 bpffs。
不要使用通配符删除整个 `/sys/fs/bpf`，因为那里可能还有其他软件加载的程序和 Map。
