# Cyanrex 项目文档

[翻译覆盖与边界](localization.md)说明 0.5.9 收录的界面词典与双语公开指南，包含韩语界面及明确课程回退。

本地创作新增[多文本文件追加](local-text-file-import.md)：一次确认加入整批，不替换旧草稿，
也不接通服务端保存。这是 0.5.9 收录的功能。

Cyanrex 正从 eBPF 教学应用转向面向人类、AI Agent 与计算资源的自部署协作平台。现有教学运行时
继续可用；通用身份、内容、任务和审阅服务属于显式配置的准备层，任务 payload 编辑器目前只保存
本地草稿。自主 AI 与通用 Run 编排尚未实现。

本索引覆盖整个项目，不只覆盖首个教学领域。源码版本为 **0.6.1**（**2026-10-15**），新增显式 Linux
进程／私有输入检查、目录声明、存储诊断与保留存储绑定的连接，并修正 Session 权限。
0.6.0 的布局、独立 Session 生命周期与存量限制，以及原只读存储门槛继续保留。
本地多文本创作、私人任务元数据索引及大文件准备层仍包含在内。
此前任务解耦、C2-O–AA、Next.js 15.5.27、AI 宿主适配与安全边界保持不变。源码收录不接通通用
浏览器保存、不切换在线认证、不迁移存储，也不验收部署。分日期的未发布说明保留原实施阶段、
源码版本与验证范围，不是新鲜的 0.6.1 执行记录。
以下文档区分源码发布内容、现用运行能力和目标架构，
模块存在或测试通过不表示已有在线部署。

0.5.4 收录的 [C2-T 意图记录](session-task-draft-intent.md)另加会话授权的登记与读取；
[C2-U 日志化发布](session-task-draft-dispatch.md)新增具有逐资源写前步骤的显式存活包装器，
不恢复尝试，也不接通浏览器保存。
[C2-V 日志检查](session-task-draft-journal-inspection.md)另以当前权限读取已记录步骤，
不检查资源内容、不复活包装器，也不授权重试。
[C2-W 日志步骤观察](session-task-draft-journal-observation.md)另在同一授权事务中读取一个
已记录目标的实际内容；不含正文的比较结果不是回执或恢复协议。

0.5.5 收录的 10 月 8 日 [C2-X 文本 Artifact 路由](text-artifact-http.md)另行发布文本并读取精确修订。
它仍未挂载；调用者自行组合 Task 路由，不表示已接通浏览器保存或整份草稿事务。

0.5.5 收录的独立 **[C2-Y 目录内容路由](catalog-task-content-http.md)**为另行构造的
强类型 HTTP 适配增加准确定义选择。原手动路由仍只接纳手动任务，两者均未挂载且与发布/日志
流程独立。本 10 月 11 日跟进不签发 Session、不执行评估，也不接浏览器保存。

0.5.5 收录的独立 **[C2-Z 目录发布尝试](session-catalog-task-draft-publication.md)**
在准备时不做 I/O，先冻结准确完整定义，每次显式推进发布一个 Artifact，或最后创建
目录 Draft。手动 C2-P–W 类型仍独立；内存确认前缀不等于日志、恢复、HTTP 路由或浏览器保存。

0.5.5 收录的独立 **[C2-AA 目录观察](session-catalog-task-draft-observation.md)**
经当前 Session 授权读取一个未知 C2-Z 目标。它比较完整定义/文本元数据，
不返回正文也不收编进度；匹配和不可见不证明先前提交/回滚，也不允许重试。

0.5.6 收录的独立 **[任务内容元数据索引](task-content-index.md)**，
在可信所有者或当前 Session 授权下列出私人 Schema 3 元数据。只返回有界页面，不读取 Artifact，
不接 HTTP/SDK/UI，不写入、不迁移或恢复；本跟进与此前发布及验证记录分开。

0.5.6 另收录 **[索引 HTTP 适配](task-content-index-http.md)**，
在精确 `GET /platform/v1/task-index` 投影该当前会话页面。它仍未挂载，严格解析原始分页查询，
私有名额持续至响应正文结束；原内容路由、Session 签发、SDK/OpenAPI 及浏览器行为不变。

**0.5.6 [大文件准备层](large-file-distribution.md)**提供了独立组合的流式、暂存、上传配额、日志、
核验、发布与私人元数据服务。当时发布者／接收者契约只比较给定快照，持久授权签发、恢复和
两种网络传输尚未接通。0.5.7 另收录授权及分块传输；[顺序下载跟进](large-file-download-client.md)
现收录于 0.5.8。正常应用分发、界面保存对话框、续传及桌面互传仍未接通。

## 按问题选择入口

| 你想了解什么 | 先读这里 |
|---|---|
| 项目定位与整体边界 | [当前架构图](architecture-map.md)、[详细系统架构](architecture.md)、[平台与教学概念](concepts.md) |
| 当前全部模块及连接关系 | [当前平台功能地图](platform-network.md) |
| 按架构、功能、实现和成熟度切片 | [功能张量与证据评分](capability-maturity.md) |
| 哪些已完成以及下一步 | [项目状态](project-status.md)、[目标架构](next-architecture.md) |
| 查看新的独立平台 Session 边界 | [Session 生命周期](platform-session-lifecycle.md)；0.6.0，只查当前会话并确认退出，不签发或挂载正常应用 |
| 不读取密码或 TOTP 地查询／退出已有 Session | [Session 数据库权限](platform-session-permissions.md)；0.6.0 之后未发布，实测列权限及非执行式关系锁，不是已挂载宿主或只读角色 |
| 在显式选出的登录入口限制持久会话存量 | [Session 存量限制](durable-session-stock.md)；0.6.0 可信准备层，不是公开签发器或自动过期清理 |
| 核对显式宿主配置与数据库目录身份 | [宿主目录预检](platform-host-preflight.md)；0.6.0 只读准备层，不是存储健康检查或已挂载宿主 |
| 在工作线程之前核对显式 Linux 服务进程 | [进程启动检查](platform-process-startup.md)；0.6.0 之后未发布，准确身份／能力及 no-new-privileges，不是沙箱或运行宿主 |
| 与原目录比较后交出一条显式本地数据库连接 | [数据库连接器](platform-database-connection.md)；0.6.0 之后未发布，无自动重试，不是连接池或对端认证 |
| 首次连接前声明原始目录身份 | [目录声明](platform-catalog-expectation.md)；0.6.0 之后未发布，严格私有版本 2 与不可变比较，不是认证发现或运行宿主 |
| 在 Linux 进程门禁后检查私有启动配置 | [私有启动输入](platform-startup-input.md)；0.6.0 之后未发布，严格 JSON／文件检查，不连接或启动宿主 |
| 从该私有输入执行一次只读存储诊断 | [存储检查](platform-storage-check.md)；0.6.0 之后未发布，实际根／目录／标记检查及关闭连接，不保留运行基线 |
| 对保留的原目录、标记及根观察建立新连接 | [保留存储绑定的连接](platform-storage-connection.md)；0.6.0 之后未发布，调用方显式选定观察，不是连接池、当前授权或运行宿主 |
| 复核明确选出的原私有 Artifact 目录 | [Artifact 根预检](platform-artifact-root.md)；0.6.0 纯元数据观察，不是内容健康检查或运行能力交接 |
| 将五项存储版本与已有作用域字段绑定到原目录 | [存储标记观察](platform-storage-markers.md)；0.6.0 标记准备层，不是完整 schema 兼容或权限健康检查 |
| 审阅集中大文件分发和后续桌面互传 | [分发提案](large-file-distribution.md)、[流式核心](large-file-core.md)、[私有分块暂存](large-file-staging.md)、[上传意图与逻辑配额](large-file-upload-intent.md)、[本地回执](large-file-upload-staging.md)、[分块写前日志](large-file-chunk-journal.md)、[整文件字节观察](large-file-whole-verification.md)、[持久发布元数据](large-file-publication.md)、[Session 私有元数据](large-file-session-metadata.md)、[纯发布／接收契约](large-file-access-contract.md)；正常应用分发及桌面互传未接通 |
| 查看持久发布权限账本及本人 Session 读取 | [持久发布权限](large-file-publisher-grants.md)；收录于 0.5.7，不在 0.5.6 标签内，不是授权管理认证或发布权限消费 |
| 通过明确配置的管理者签发／撤销授权 | [Session 授权管理](large-file-grant-management.md)；宿主指定准确账号，不继承角色，不是持久在线管理权撤销或发布权限消费 |
| 使用当前发布授权发布已暂存上传 | [Session 发布](large-file-session-publication.md)；最终授权认证源提交及原字节复核，不是下载、网络传输或恢复 |
| 通过当前接收者查看准确文件授权 | [持久接收授权](large-file-recipient-grants.md)；独立撤销与本人元数据读取，不是接收授权管理或下载权限 |
| 分享本人 Published 版本或停止旧分享 | [发布者文件分享](large-file-sharing.md)；签发要当前发布权限，撤销要准确原发布者，不是下载准入 |
| 查看所授权准确 Published 的下载清单 | [接收者清单观察](large-file-download-manifest.md)；当前接收者及最终授权／Session 有效期，不是字节健康或可复用下载许可 |
| 读取当前授权的一个已校验文件块 | [分块快照](large-file-chunk-read.md)；私下校验至多 4 MiB 并重入原授权，不是 HTTP 或客户端送达 |
| 查看独立未挂载的二进制分块路由 | [单块 HTTP 适配](large-file-chunk-http.md)；严格准确版本 GET 及字节所有者预算，不是已挂载局域网送达或客户端保存 |
| 校验并私有暂存已提供二进制响应 | [HTTP 分块接收器](large-file-chunk-receiver.md)；真正 EOF／摘要之后独占本地暂存，不是主动获取、整文件保存或续传 |
| 从可信固定 Origin 显式获取单块 | [受控 HTTP 客户端](large-file-chunk-client.md)；HTTPS／明确字面环回及保存任务持有请求准入，不是运行挂载或整文件保存／续传 |
| 把旧式分块拼成一个私有文件 | [私有整文件拼装](large-file-chunk-assembly.md)；实际有序／整文件字节及原 inode 读回，不是用户选路径保存、续传或发布 |
| 顺序下载分块并私有拼装 | [顺序下载客户端](large-file-download-client.md)；0.5.8 已收录，流程／真实任务共享名额，不是界面保存、续传或已挂载送达 |
| 经独立 JSON GET 查看获授权的完整清单 | [接收者清单 HTTP](large-file-manifest-http.md)；0.5.8 已收录，准确当前接收者，不是字节权限 |
| 获取准确元数据并组合获授权私有下载 | [准确清单客户端](large-file-manifest-client.md)；0.5.8 已收录，空文件仍须 GET，不是界面保存／续传或在线挂载 |
| 将原确认输出保存至明确选择的新私有文件 | [显式私有保存](large-file-save.md)；0.5.8 已收录，原描述符证明与独占复制／读回，不是界面对话框或续传 |
| 下载准确授权版本并保存一份明确选择的私有副本 | [授权下载及保存](large-file-delivery-client.md)；同一整流程名额跨实际文件任务，不是界面保存、续传或在线服务 |
| 明确选择本人普通目录而不放宽私有暂存 | [本人普通目录保存](large-file-user-save.md)；0700／0750／0755 及原 UID／GID／模式核验，不是共享可写目录或界面选择 |
| 观察当前交付阶段及已确认块，不误判保存完成 | [实时交付进度](large-file-delivery-progress.md)；单调用最新快照，不是持久进度、回执、续传或界面 |
| 显式停止一次交付等待而不误杀文件任务 | [协作式取消](large-file-delivery-cancellation.md)；句柄drop不取消、结果决定竞争，既有任务仍持资源，不自动清理 |
| 通过真实HTTPS验证完整带取消交付链 | [原生HTTPS交付检查](large-file-delivery-https.md)；一次性准确对端、独立输出与取消，不是已部署局域网或grant权威 |
| 从同一受信配置组合清单与分块端点 | [分发路由](large-file-distribution-router.md)；一个Source／workspace、独立预算及私有兜底，不是正常挂载或特权隔离 |
| 联验真实授权源与HTTPS交付 | [真实Source／HTTPS](large-file-source-https.md)；显式联合SQL／文件／TLS用例，不是已部署局域网、在线挂载或Session签发 |
| 编辑笔记、代码或配置等任务内容 | [任务 payload 编辑器](editor.md) |
| 配置 AI Agent 宿主并适配明确选出的工具 | [AI Agent SDK 接入](ai-agent-integration.md) |
| 查看内部服务端内容结构与精确快照校验 | [任务内容清单](task-content-manifest.md) |
| 查看独立内容持久化与原子编辑 | [任务内容存储](task-content-store.md) |
| 不读取正文，只列出私人任务元数据 | [任务内容元数据索引](task-content-index.md) |
| 查看独立未挂载的 HTTP 元数据列表 | [任务内容索引 HTTP 适配](task-content-index-http.md) |
| 查看同一事务内的当前会话与内容字节检查 | [会话授权任务内容](session-task-content.md) |
| 查看显式构造且尚未挂载的手动 HTTP 边界 | [任务内容 HTTP 适配](task-content-http.md) |
| 经独立未挂载 HTTP 适配选择准确配置定义 | [目录任务内容 HTTP 适配](catalog-task-content-http.md) |
| 经独立未挂载路由发布文本或读取精确修订 | [文本 Artifact HTTP 适配](text-artifact-http.md) |
| 查看严格草稿导入与纯发布映射 | [任务草稿发布计划](task-draft-publication.md) |
| 查看手动会话逐步发布及部分结果 | [会话授权草稿发布](session-task-draft-publication.md) |
| 通过独立尝试与冻结定义发布目录草稿 | [会话授权目录草稿发布](session-catalog-task-draft-publication.md) |
| 观察未知步骤的目标而不恢复尝试 | [草稿目标只读观察](session-task-draft-observation.md) |
| 按冻结完整定义观察目录尝试目标 | [目录草稿目标只读观察](session-catalog-task-draft-observation.md) |
| 导出报告进度为数据而不复活尝试 | [草稿元数据检查点](session-task-draft-checkpoint.md) |
| 另行提供当前权限检查一个检查点目标 | [检查点目标显式检查](session-task-draft-inspection.md) |
| 登记和读取不可变意图而不执行发布 | [会话草稿意图记录](session-task-draft-intent.md) |
| 先登记步骤，再在资源事务内共同提交完成标记 | [日志化会话草稿发布](session-task-draft-dispatch.md) |
| 检查日志记录进度而不恢复尝试 | [会话草稿日志检查](session-task-draft-journal-inspection.md) |
| 保留原日志身份并观察一个已记录步骤的资源 | [日志步骤观察](session-task-draft-journal-observation.md) |
| 使用或讲授现有 eBPF 流程 | [学生指南](student-guide.md)、[教师指南](teacher-guide.md)及下方实验 |
| 部署或运维可信实例 | [安全](security.md)、[课堂接入与 SSH](classroom-connection.md)、[Runner Agent](runner-agent.md)、[故障排查](troubleshooting.md) |
| 开发或验证改动 | [贡献指南](../../CONTRIBUTING.md)、[当前测试指南](testing-guide.md)、[SDK](../../sdk-js/README.md)、[工具](../../scripts/README.md) |
| 查阅历史证据 | [开发历史](development-history.md)、[历史验收](acceptance.md)、[历史测试网络](testing-network.md)、[0.3.8 功能基线](functional-network.md) |

## 平台契约与实现依据

先理解概念模型，再选择具体适配器。直接存储接口的所有者过滤不等于认证；公共教学 API 仍使用原有
运行时，通用 Session 命令不会自动成为 HTTP 接口，也不会替换在线登录。

| 边界 | 设计与实现文档 |
|---|---|
| 基础与作用域身份 | [基础契约](collaboration-foundation.md)、[身份注册表](collaboration-identity-store.md)、[成员与策略](collaboration-access-store.md) |
| 审计生命周期与命令 | [策略审计](collaboration-policy-audit.md)、[身份生命周期](collaboration-identity-lifecycle.md)、[当前会话命令](collaboration-session-commands.md) |
| 持久认证 | [账号与会话来源](collaboration-auth-source.md)、[账号删除](collaboration-account-deletion.md)、[密码修改](collaboration-password-change.md) |
| 显式初始化与观察 | [全新引导](collaboration-bootstrap.md)、[本地初始化 CLI](collaboration-provisioning.md)、[只读对账](collaboration-reconciliation.md) |
| 任务与领域定义 | [任务领域解耦](task-domain-boundary.md)、[任务实例](task-instance-store.md)、[目录准入](session-catalog-tasks.md) |
| 不可变内容 | [制品修订](artifact-revision-store.md)、[私人制品命令](session-artifact-commands.md) |
| 任务内容访问与修改 | [私人手工任务](session-task-commands.md)、[准确输入](session-task-inputs.md)、[Draft 输入换版](session-task-revisions.md) |
| 判断与历史 | [审阅记录](review-record-store.md)、[会话授权的私人人工审阅](session-review-commands.md) |

准备层不会迁移现有教学记录。Task Schema 2 仅支持全新安装，存储格式版本不等于软件版本。代码是
可选任务内容，不是 Task 的必填字段。发布内容、替换任务输入、执行 Run 和记录 Review 是不同操作，
彼此不能推导成功，也不因此扩大跨用户权限。

## 现有运行时参考

教学包是首个实际运行的领域，保留教师管理部署、学生学习记录和内核执行边界。以下操作指南应与
尚未接入现用运行时的通用平台命令区分阅读。

- [学习存储与评语](learning-storage.md)、[事件流与恢复](event-stream.md)。
- [桌面与局域网隔离目标](classroom-isolation.md)：设计边界，不代表 VM 初始化已完成。
- [根目录快速开始与运行时 API](../../README.md#quick-start)。
- [模块清单](../../modules/README.md)：声明式目录，不等于可执行领域包。
- [故障排查](troubleshooting.md)：区分本地草稿、现有运行时和准备层存储的问题。

## 推荐阅读顺序

### 教师

1. [教师快速开始](teacher-guide.md)
2. [项目进度状态](project-status.md)
3. [系统架构](architecture.md)
4. [Runner Agent 使用指南](runner-agent.md)
5. [课程知识地图](concepts.md)
6. [安全与课堂部署](security.md)
7. 浏览全部实验并提前试跑
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
- `CI gate` 会依赖 `security-audit`、`file-lengths`、`engine`、`frontend`、`sdk`、`permissions` 和
  `distribution`，并在任一任务失败时直接失败。
- 建议在分支保护中只配置必需检查项为 **`CI gate`**，这样合并统一受该门禁控制。
- 推送注解版本 Tag 会触发 `Release Candidate Validation`：它把干净的 Tag 提交绑定到新构建的
  离线发行包，完成精确镜像安装及真实 Aya 挂载/事件/detach 验收后，将结果保留为 30 天的
  工作流制品，并附带独立校验和保护、绑定候选产物的内核证据；统一离线验证器会在不解包的
  情况下流式检查归档、拒绝危险成员、复验包内所有文件，并把元数据与该证据交叉绑定；需要
  解包时，只有完整验证通过后才会以非覆盖方式逐文件写入。CI 与 Tag 验收均已使用该路径，
  不再直接展开 `tar`。该流程不会创建或签名 GitHub Release。
