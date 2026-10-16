# 为 cyanrex-lab 贡献

cyanrex-lab 是免费、开源、可自部署的项目，正在从 eBPF 教学应用发展为不绑定特定领域的协作平台。
项目继续支持教学运行时，同时独立开发通用身份、Task、Artifact 和 Review 服务。
欢迎为平台、领域包、运行时、编辑器、课程材料、翻译、测试和部署工具贡献改进。

## 开始之前

- 修改服务边界、路由、认证或部署模式之前，先阅读[架构概览](docs/zh-CN/architecture.md)。
- 修改特权 Engine 行为、eBPF 加载、远程访问或课堂隔离之前，先阅读[安全指南](docs/zh-CN/security.md)。
- 新建 issue 之前，先检索已有 issue。
- 选择开发入口之前，阅读[当前项目进度](docs/zh-CN/project-status.md)和[功能关系图](docs/zh-CN/platform-network.md)，
  区分现用服务、仅在本地工作的 UI 和显式准备层。
- 大功能或架构变更应先提交提案 issue，以便讨论平台或领域用例、授权边界和兼容性影响。
- 按[安全政策](SECURITY.zh-CN.md)通过私密渠道报告漏洞。

## 开发环境

启动器支持 Docker、WSL2 和原生 Linux 模式。首次配置推荐使用 Docker：

```bash
./start.sh start --mode docker
./start.sh status
./start.sh logs
./start.sh stop
```

报告启动问题之前，使用 `./start.sh diagnose` 收集环境信息。
前置条件和各模式的操作说明见[主 README](README.zh-CN.md)与[故障排查指南](docs/zh-CN/troubleshooting.md)。

## 修改流程

### 选择正确的边界

- 通用身份、内容、任务和审阅契约属于共享协作模型，不应放进 eBPF 控制器。
  代码只是可选的任务 payload；Task 不一定需要执行 Run。
- 领域定义和类型化评估规则属于显式领域包。注册目录条目不授予执行、读取、已验证证据或 Task 接受权限。
- 本地编辑器状态属于持有它的任务草稿。不能把本地 ID、文件名、语言标签或浏览器确认当成
  服务端权限、文件系统访问权或已持久化的 Artifact 引用。
- 直接存储层假定调用者拥有可信的所有权。不可信命令需要当前 Session 的事务防护、精确
  namespace/scope 检查和写后验证，不能只做一次脱离事务的权限预检。
- 现用 AuthService／教学数据表与准备层存储保持分离。不能借附带重构接入启动安装器、
  自动迁移、新回退行为或跨用户访问。
- 源码层面的控制／执行分离并不隔离特权 Engine。Runner Agent 是计算工作节点；
  AI 编排和可执行插件需要独立评审边界。

### 验证修改

仓库采用测试驱动开发：

1. 新增或更新描述预期行为的测试。
2. 用最小实现使测试通过。
3. 在保持测试通过的前提下重构。
4. 行为改变时更新用户文档和翻译。

后端路由修改应更新 `engine/tests/` 下的测试。前端权限变化应按需更新
`frontend/tests/sidebarPermissions.test.mjs`。

服务、事务、浏览器、SDK 和真实外部边界的覆盖范围见[当前测试指南](docs/zh-CN/testing-guide.md)。
默认 Cargo 测试有意跳过需显式启用的数据库／内核用例；模拟 Engine 响应的浏览器夹具
不能证明生产授权或服务端保存已成立。

提交 pull request 之前，运行完整本地质量门禁：

```bash
./scripts/quality-gate.sh
```

也可以运行针对性的检查：

```bash
./scripts/quality-gate.sh --backend-only
./scripts/quality-gate.sh --frontend-only
./scripts/quality-gate.sh --permissions-only
./scripts/quality-gate.sh --security
./scripts/quality-gate.sh --format-only
```

前端检查通过 `npm ci` 安装依赖。锁文件对应的依赖已经安装且未变更时，
可用 `--no-npm-install` 缩短本地执行时间。

### 本地构建的磁盘占用

Rust 开发和测试构建保留行级调试信息，并关闭增量编译，以限制反复测试时的磁盘增长。
发布构建不变。需要查看局部变量的调试会话可设置 `CARGO_PROFILE_DEV_DEBUG=2`
（测试则使用 `CARGO_PROFILE_TEST_DEBUG=2`）覆盖默认值。设置 `CARGO_INCREMENTAL=1`
可重新启用增量编译，用更多磁盘空间换取更快的重复构建。

停止本地构建及使用构建产物的进程后，可用 `cargo clean --manifest-path engine/Cargo.toml`
回收 Rust 构建产物；下次构建会重新编译依赖。前端 `.next/`、SDK `dist/` 和 TypeScript
`*.tsbuildinfo` 也是生成产物，不是源码或备份。常规缓存清理不得包含运行数据、`.env` 文件、
`.run/` 维护配置、数据库卷、迁移备份或回滚镜像。

## 仓库规则

- 维护的源码文件不得超过 600 行。
- 文档文件不得超过 2,000 行。
- 安全敏感操作不得包含隐藏回退行为。
- 不得提交密码、session token、TOTP secret、私钥、数据库转储、学生数据或原始事件日志。
- 不得将特权 Engine 直接暴露给不可信网络。
- 保持修改聚焦；无关重构应放在独立 pull request 中。
- 保持向后兼容，除非 pull request 明确说明迁移路径。

运行 `./scripts/check-file-lengths.sh` 可直接验证行数限制。

## 发布流程

`engine/Cargo.toml` 是版本的规范来源。发布时必须更新全部同步版本目标，将相关 `Unreleased`
条目移入 `CHANGELOG.md` 中注明日期的版本章节，并在给版本提交打 tag 前通过完整质量门禁。

在精确的版本提交上创建带注释的 `v<version>` tag。不要移动已有发布 tag；已发布产物需要
修正时，应发布后续补丁版本。分支和 tag 的发布仍是显式操作，本地验证不能意外发布：

```bash
./scripts/quality-gate.sh
git commit -m "0.5.0"
git fetch origin --tags --prune
node scripts/release-preflight.mjs --version 0.5.0
git tag -a v0.5.0 -m "cyanrex-lab 0.5.0"
node scripts/release-preflight.mjs --tag v0.5.0
git push origin main
git push origin v0.5.0
```

发布负责人应在发布前核验 tag 目标、分发包校验和、解包后的冒烟结果及 changelog 链接。
推送版本 tag 还会触发专门的发布工作流：核验不可变元数据、构建离线 Docker 包、验证标签绑定的
解包与 live-kernel 安装冒烟，并核验候选包。远程结果需要另行确认；工作流不创建 GitHub Release、
不发布 npm 包，也不部署服务。

## 课程材料与翻译

项目文档从[索引](docs/zh-CN/README.md)开始。当前能力概述放在项目进度中，源码关系放在架构／
平台关系图中，以前的执行放在开发历史或注明日期的证据中。不要改写历史 hash、版本或失败数量，
使其看起来像新的验收结果。
更新 `docs/` 下的英文和中文源文件后，运行 `node frontend/scripts/sync-course-docs.mjs`；
`frontend/public/course/` 由这些源文件生成，不是另一处独立编写文档的入口。

新界面字符串和课程材料以英文为主要源语言。条件允许时，应在同一个 pull request 中
更新对应简体中文内容。其他受支持的界面语言是西班牙语、日语和韩语（`ko`）。新界面键必须
加入全部五种合并词典，保留插值参数、协议标识和插入代码。韩语界面不表示已有韩语课程正文；
正文来源补齐之前，应保留明确的回退提示。

课程贡献应包含：

- 清晰的学习目标；
- 内核、能力和工具的前置条件；
- 每个 attachment 或资源的安全清理路径；
- 足够稳定、便于学习者辨认的预期输出；
- 对常见 verifier 或环境失败的解释；
- 能可靠检查课程时提供自动验证。

示例不能假定共享课堂 Engine 是安全边界。

## Pull request

Pull request 应说明：

- 要解决的平台、领域或运维问题；
- 选择的行为与重要取舍；
- 如何测试；
- 安全、迁移、部署或翻译方面的影响。

可见 UI 变化适合附截图。附日志或图片之前，移除账户名、主机名、IP 地址、token 和学生信息。

提交贡献即表示同意按仓库的 [Apache License 2.0](LICENSE) 分发该贡献。
