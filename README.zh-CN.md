# cyanrex-lab

[English](README.md)

版本：`0.6.5`

Cyanrex 是自托管的协作与教学平台。Task 管理目标及可选的内容载荷，任务不必包含代码，也不必
发起执行。eBPF 教学是首个支持的领域，不是所有任务的定义。

现有教学应用、本地任务创作和独立平台宿主有各自的入口与持久化边界。先选择与你要做的事情
对应的阅读路径。

[文档入口](docs/zh-CN/README.md) · [当前状态](docs/zh-CN/project-status.md) ·
[架构速览](docs/zh-CN/architecture-map.md) · [专题参考目录](docs/zh-CN/reference-index.md) ·
[更新日志](CHANGELOG.md)

## 从这里开始

- **学习：** 从[学生指南](docs/zh-CN/student-guide.md)进入五个基础实验。
- **教学：** 阅读[教师指南](docs/zh-CN/teacher-guide.md)，了解课堂和部署管理权限。
- **部署与维护：** 选择 [Docker 指南](docker/README.zh-CN.md)或
  [离线包运维指南](docker/README-DEPLOY.zh-CN.md)；完整命令和 API 见
  [运行时参考](docs/zh-CN/runtime-reference.md)。
- **平台开发：** 先看[架构速览](docs/zh-CN/architecture-map.md)、
  [能力链路图](docs/zh-CN/platform-network.md)和[贡献指南](CONTRIBUTING.zh-CN.md)。
- **API 集成：** 从 [JavaScript SDK](sdk-js/README.zh-CN.md)开始，不把准备层路由当成现用 API。

[文档入口](docs/zh-CN/README.md)按角色组织阅读路径；
[专题参考目录](docs/zh-CN/reference-index.md)集中列出契约、实现指南与历史记录。

## 当前能力

| 范围 | 当前可用路径 | 关键边界 |
|---|---|---|
| 教学 | 已认证浏览器流程、eBPF 检查／运行、学习记录与教师反馈 | 共享内核执行不等于学生隔离 |
| 任务创作 | 本地草稿、可选文本载荷、14 种编辑器语言配置、经确认的导入与下载 | 尚无通用浏览器服务端保存、自动保存或 Run 派发 |
| 平台宿主 | 显式启动的 Linux 回环 HTTP／原生 TLS，提供有界 Session、本人私人 Task 命令及可选 Task 绑定文本发布 | 独立启动与存储；无浏览器正文保存或局域网部署 |
| 大文件传输 | 经授权的发布、授权记录、已验证分块及显式交付组件 | 未接入普通应用界面，不提供重启恢复 |
| SDK 与 AI 适配 | 类型化 Engine 客户端、宿主配置及显式 Agent 工具格式适配 | 未实现自主 AI 编排或模型提供商传输 |

界面支持英语、简体中文、西班牙语、日语和韩语。课程正文为英语和简体中文；
其它界面语言会明确提示课程语言回退。

缺口与下一步见[项目状态](docs/zh-CN/project-status.md)，实现与证据细节见
[能力成熟度](docs/zh-CN/capability-maturity.md)。已发布改动及 tag 后的安全修复分别记录在
[CHANGELOG.md](CHANGELOG.md)。

## 快速开始

在可信 Linux／WSL2 主机或可丢弃 Docker 环境启动现有教学应用。
授予内核权限前先阅读[安全指南](docs/zh-CN/security.md)。

```bash
./start.sh start --mode docker
./start.sh status
./start.sh diagnose
./start.sh logs
./start.sh stop
```

使用 `--mode wsl` 观察 WSL2 Linux 内核，或 `--mode native` 使用原生 Linux。
eBPF 始终运行于 Linux 内核；Docker Desktop 观察的是虚拟机内核，不是桌面系统本身。

默认本地入口：

- 前端：`http://localhost:3000`
- Engine 健康检查：`http://localhost:8080/health`
- PostgreSQL：`127.0.0.1:15432`，不向局域网暴露

初始 `admin` 账户的角色为 `teacher`，同时管理教学与部署；单人使用时自己就是教师。
初始密码和 TOTP 材料生成在私有、已被 Git 忽略的 `docker/.env` 中，不要公开该文件。
默认不开放注册。

通过 `/ebpf` 和 `/learn` 进入教学。`/tasks/new`（`/editor` 为兼容入口）管理本地任务草稿；
下载草稿不等于服务端已保存 Task。[编辑器指南](docs/zh-CN/editor.md)说明持久化与离开页面的边界。

**独立平台宿主**使用单独的[显式启动指南](docs/zh-CN/platform-loopback-host.md)。
启动教学应用不会初始化或迁移协作存储。
默认宿主仍为八路由／七响应通道；显式 `--task-text-publication` 才启用第九路由／第八通道，
针对本人 Draft 基线发布，但不保存该 Task。可选 Session UI 仍是七个资源，不含正文保存客户端。
完整启动参数、账户配置和运行时端点见[运行时参考](docs/zh-CN/runtime-reference.md)。

## 下一代架构

Task、Artifact、Run 和 Review 分别负责任务意图与生命周期、不可变内容修订、一次执行尝试和
版本绑定的判断。Identity 与 Workspace 限定所有权，当前 Session 在存储事务内授权命令。

[架构速览](docs/zh-CN/architecture-map.md)说明当前接线；
[详细架构](docs/zh-CN/architecture.md)解释实现边界；
[下一代设计](docs/zh-CN/next-architecture.md)描述目标，不代表通用 Run、AI 编排、跨用户协作
或运行中迁移已经完成。

## 仓库结构

```text
frontend/
engine/
sdk-js/
modules/
docker/
scripts/
docs/
reports/
```

`frontend/public/course/` 从 `docs/` 生成，先编辑源文档再同步。
文档职责和完整专题库存见[专题参考目录](docs/zh-CN/reference-index.md)。

## 部署与更新

源码工作区和解压后的离线包使用不同的运维命令。
[部署指南](docker/README-DEPLOY.zh-CN.md)说明包验证、日常健康观察和安全更新；
[运行时参考](docs/zh-CN/runtime-reference.md#25-构建分发包)保留打包与验证后提取的完整操作。

安装烟测与真实内核验收应在可丢弃主机上执行，不应在正在使用的课堂实例运行。
依赖或锁文件更新不会修补运行中的进程或容器，需要重新构建并部署。
不要把特权 Engine 直接暴露给不可信网络。

## API 契约

现有 Engine 在 `GET /openapi.json` 提供生成的 [OpenAPI 3.1 契约](engine/openapi/openapi.json)。
[Engine 参考](engine/README.zh-CN.md)和 [JavaScript SDK 指南](sdk-js/README.zh-CN.md)说明运行时访问与集成。
独立平台宿主不会自动成为该公开 Engine／SDK 契约的一部分。

认证、eBPF、Runner、脚本、学习和持久化的完整说明见
[运行时 API 参考](docs/zh-CN/runtime-reference.md#认证-api已实现)。

## 工程规则

遵循 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md) 和 [TDD.zh-CN.md](TDD.zh-CN.md)。
从仓库根目录运行检查：

```bash
./scripts/quality-gate.sh --security
```

安全审计选项需要 `cargo-audit`，可通过 `cargo install cargo-audit --locked` 安装。
[测试指南](docs/zh-CN/testing-guide.md)区分默认检查、显式数据库／浏览器夹具和特权验收。
模拟后端的浏览器夹具通过，不等于真实授权或服务端保存已经验收。

### CI 门禁（`ci-gate`）

`CI gate` 汇总安全审计、格式、Rust、前端、SDK、权限和分发任务。
完整说明见[运行时 CI 参考](docs/zh-CN/runtime-reference.md#ci-门禁ci-gate)。

### Tag 候选验收

新的注释版本 tag 会触发不可变元数据及离线包验收，不会发布 GitHub Release 或部署服务。
遵循[发布流程](CONTRIBUTING.zh-CN.md#发布流程)，分日期的执行结果保留在
[发布报告](reports/releases/0.6.5/README.md)，不再展开于首页。

## 参与贡献

Cyanrex 以 [Apache License 2.0](LICENSE) 自由开源。欢迎贡献协作能力、教学领域、载荷工具、
翻译与运维改进。请遵守[行为准则](CODE_OF_CONDUCT.zh-CN.md)，通过
[私密安全报告流程](SECURITY.zh-CN.md)反馈漏洞。
