# Cyanrex eBPF 教学手册

Cyanrex 是一个面向 eBPF 入门教学的实验系统。它把源码编辑、clang 诊断、语义补全、
内核加载、事件观察和程序卸载放进同一个 Web 界面。

## 推荐阅读顺序

### 教师

1. [教师快速开始](teacher-guide.md)
2. [课程知识地图](concepts.md)
3. [安全与课堂部署](security.md)
4. 浏览全部实验并提前试跑
   - eBPF 页面中的课程路径：
     - `learning/foundations/beginner/fundamentals`
     - `learning/foundations/intermediate/protocols`
     - `learning-plus/cases/advanced/forensics`
     - `learning-plus/track/practice/operators`

### 学生

1. [学生快速开始](student-guide.md)
2. [课程知识地图](concepts.md)
3. 按顺序完成实验：
   - [实验 1：认识执行链路](labs/01-first-program.md)
   - [实验 2：观察 execve 系统调用](labs/02-trace-execve.md)
   - [实验 3：使用 eBPF Map 计数](labs/03-map-counter.md)
   - [实验 4：使用 Ring Buffer 传递事件](labs/04-ring-buffer.md)
   - [实验 5：读懂 Verifier 与调试程序](labs/05-verifier-debugging.md)
4. 遇到问题时查看[故障排查手册](troubleshooting.md)

## 课程完成标准

完成本课程后，学习者应能：

- 解释用户态、eBPF 程序、内核 hook 和 verifier 的关系；
- 根据场景选择 XDP、tracepoint 等 hook；
- 使用 Map 保存状态，使用 Ring Buffer 上报事件；
- 理解边界检查、空指针检查和有界循环为什么必要；
- 根据 clang 与 verifier 日志定位常见错误；
- 安全卸载程序并确认实验环境恢复干净。

## 运行模式

| 模式 | 实际目标内核 | 推荐场景 |
|---|---|---|
| WSL2 | WSL2 Linux 内核 | Windows 个人学习 |
| Docker | Linux 宿主机或 Docker Desktop VM 内核 | 快速试用、统一课堂环境 |
| Native Linux | 当前 Linux 内核 | 深入实验、最佳兼容性 |

eBPF 永远运行在 Linux 内核中。Windows 和 macOS 的 Docker 模式观察的是虚拟机内核，
不是桌面操作系统本身。

## 可选运行参数（高级）

事件量很大时，可在 `docker/.env` 调整持久化告警行为，减少教学现场告警噪音并便于排障：

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED`（默认：`true`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT`（默认：`80`）
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT`（默认：`40`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS`（默认：`10000`）

## CI 与合并门禁

- CI 流程已加入聚合任务 `CI gate`（位于 `.github/workflows/ci.yml`）。
- `CI gate` 会依赖 `security-audit`、`file-lengths`、`engine`、`frontend` 和 `permissions`，并在任一任务失败时直接失败。
- 建议在分支保护中只配置必需检查项为 **`CI gate`**，这样合并统一受该门禁控制。
