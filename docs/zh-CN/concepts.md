# 平台与 eBPF 概念

Cyanrex Lab 现在包含两组概念：领域无关的协作模型，以及第一个已运行领域使用的 eBPF 术语。
本文把它们分开，避免将内容编辑、执行和批准视为同一件事。平台契约与显式后端准备层已存在，
但在线教学应用仍使用原有 API 和存储，本地任务编辑器也尚未连接这些后端存储。实现边界见
[系统架构](architecture.md)和[平台链路地图](platform-network.md)。

## 平台概念

### Authority 与 Workspace

**Authority** 标识一个持久实例权威，不是主机名、HTTPS 身份、`CYANREX_INSTANCE_ID` 部署标签，
也不是其他实例上的权限。**Workspace** 在一个 Authority 内组织工作和成员，其引用同时携带
两层身份。两个实例使用相同的局部 ID，不会让对象或权限等同。

这些属于平台准备层概念。类型和存储存在，不代表现有教学应用已经成为公共多工作区服务。

### Principal 与 Session

**Principal** 是参与者身份，不是用户名或角色。契约区分 human、agent、service、machine，
教师/学生是权限角色，不是 Principal 种类。`agent` 为未来 AI 参与者保留身份类别，不表示 AI
行为已经实现。

**Session** 证明当前经过认证的账号上下文。准备层持久来源跟踪账号代次：删除账号后重建同名
账号，不能继承旧身份或 Grant。命令适配器在同一事务内验证 Session、绑定、成员与策略；提交
Principal ID、序列化角色或旧权限预览，不能替代这一检查。

### 成员与部署权

**Membership** 将 Principal 与 Workspace 关联，带有活跃/暂停状态及局部角色预设。它与实例
部署 Grant 分开；工作区所有者不自动获得 SSH 凭据、特权执行或部署管理权限。

在线教学应用仍由教师统一负责教学和部署，单人部署账号默认就是教师。这种便利不使他人的私有
内容公开；准备层私人工作适配器也坚持本人范围，教师同样受限。课堂中审阅学生教学尝试，是另一项
已经存在的明确权限。

### 任务定义与领域包

**TaskDefinition** 描述准确的包/任务版本、证据 Schema 和评估策略。内置**领域包**通过类型化
TaskProvider 提供定义和领域规则，共享 TaskCatalog 验证并查找准确版本，不假设所有任务都含
代码、需要编译器或必须产生 Run。

eBPF 教学包负责实验模板、源码证据和内核相关规则。其他领域测试夹具证明共享边界可用，不表示
已经交付文档审阅流程。注册定义既不是安装可执行插件，也不是执行授权。`modules/` 清单目录是
另一项功能，不会动态安装 TaskProvider。

### Task 与 payload

**Task** 是工作项：归谁所有、可选的任务定义、准确输入引用、当前状态和修订号。代码可以是
**payload** 的一部分，但代码不等于任务本身。本地编辑器允许草稿包含零个或多个文本项；后端
Task 保存 Artifact 引用，不把编辑器模型或源码直接嵌入任务记录。

“草稿”可能指三个不同对象：浏览器 TaskDraft、后端创建参数 TaskDraft，以及处于 Draft 状态的
持久 TaskSnapshot。本地 ID 和内容项修订不能当作服务端 Artifact/Task 身份，导出本地 JSON
也不等于发布或服务端保存。

当前持久状态包括 Draft、Ready、InProgress、Blocked、InReview、Cancelled，没有 Accepted/Done。
修订号是乐观并发检查：命令必须指定期望版本，其他修改会令旧期望失效。取消 Task 只结束工作项，
不会结束运行进程或内核挂载。

### Artifact 与准确引用

**Artifact** 独立于任务类型承载内容。持久版本不可变，修改字节须创建新版本，而不是覆盖旧版本。
**ArtifactRef** 固定工作区、Artifact ID、版本 ID 及 SHA-256 摘要，不存在浮动 `latest` 引用。
相同摘要不会合并所有权，也不授予另一份内容的访问权。

有元数据或能成功解码还不够，授权读取会检查实际内容。Artifact 发布和 Draft Task 输入换版是
两个操作：换版检查预期 Task 修订及旧、新输入，不删除被替换的内容。任务更新失败也不能被解释
为允许删除已经发布的版本。详见[输入换版](session-task-revisions.md)。

### Assessment 与 Review

**Assessment** 是提供器的进程内规则结果，包含 Passed/NotPassed 及反馈，不是人工签署、
持久 Review 或验收 Task 的权限。目录元数据准入不执行这些规则，也不把任意内容转换为合法的
类型化证据。

**ReviewRecord** 针对准确 Artifact 目标和证据表达判断。人工判断包括 Approved、
ChangesRequested、Commented，规则判断是另一种形式；修改意见会保留更早的版本。当前 Session
路径支持对本人 Artifact 的私人人工 Review，不是通用跨用户审阅或 AI 判断。摘要检查只证明
字节身份，不证明证据相关、评估策略执行过，或某项任务应该被验收。

### Run 与 Runner Agent

**Run** 是目标架构中某次执行的持久表示，与请求执行的 Task 分开。平台目前有 Run 身份/引用
契约，没有通用持久 Run 存储或调度器。现有 eBPF 运行、报告、Runner 租约及 Agent 作业保留教学
专属实现，不会自动转成平台 Run 记录。

**Runner Agent** 是远程计算节点，不是 AI 参与者。当前 Agent 可探测环境，并按配置提供隔离的
仅编译检查；`/ebpf/run` 仍在本地执行。Agent 注册健康或取得租约，不证明通用任务完成、不授权
审阅，也不为本地 Engine 增加内核隔离。

### 业务事件与遥测

**EventEnvelope** 通过事件身份、聚合对象及修订、操作者和关联字段表达业务事实。
Task、Artifact、Review 存储把状态与 outbox 记录原子写入。这是存储侧准备，不是已经实现的
outbox 投递/重放服务，也不保证恰好一次执行。

现有 **EventBus** 承载有界运行遥测和浏览器事件，采用异步持久化，具有明确缺口和降级限制。
这些事件不是认证审计记录，不能仅因文本描述了成功操作就升级为业务事实。

### 编辑器语言服务与执行

受控文本编辑器为选中的 payload 提供本地语言支持。14 种语言配置中，JavaScript/TypeScript、
JSON、CSS、HTML 使用较完整的内置工作线程，其他语言提供基础能力；并未连接 rust-analyzer、
Pyright 或 clangd 服务。语言辅助既不运行内容，也不授予运行权限。独立 eBPF 编辑器的 clang
诊断与语义服务仍使用经过认证的教学端点。

### 不同类型的版本

产品版本、核心契约 Schema、存储 Schema、定义/策略版本及对象修订互相独立。源码发布版为
0.4.9；当前 Task 存储要求全新 Schema 2，而核心契约 Schema 仍为 1。旧 Task Schema 1
命名空间会被拒绝，不自动迁移。产品版本升高，不会替换任务冻结的定义，也不会把 Review 转移到
新内容。

## eBPF 教学概念

以下概念描述已有 eBPF 教学领域，仍适用于在线编辑器，但不成为所有平台 Task 或 payload 的要求。

## 1. 一条程序如何运行

```text
C 源码
  -> clang 编译为 BPF 字节码
  -> 内核 verifier 验证安全性
  -> loader 加载程序和 Map
  -> attach 到 hook
  -> 内核事件触发程序
  -> Map/Ring Buffer/trace 输出数据
  -> 用户态读取并展示
```

Cyanrex 的结果区将上述过程拆成 compile、load 和 attach，排错时先判断失败发生在哪一层。

## 2. Hook

Hook 是 eBPF 程序被调用的位置。代码中的 `SEC("...")` 描述程序类型和挂载点。

- `SEC("xdp")`：网卡驱动接收路径的早期阶段；
- `SEC("tracepoint/category/name")`：稳定的内核 tracepoint；
- `SEC("kprobe/function")`：动态探测内核函数，兼容性要求更高；
- `SEC(".maps")`：Map 定义，不是可执行程序；
- `SEC("license")`：程序许可证。

## 3. Context

内核调用 eBPF 程序时会传入 context。例如 XDP 使用 `struct xdp_md *ctx`。
Context 能访问哪些字段由程序类型决定。输入 `ctx->` 时，Cyanrex 会请求 clang 给出真实字段。

## 4. Helper

eBPF 不能随意调用内核函数，只能调用当前程序类型允许的 helper，例如：

- `bpf_ktime_get_ns()`：读取单调时钟；
- `bpf_get_current_pid_tgid()`：读取进程/线程标识；
- `bpf_map_lookup_elem()`：查询 Map；
- `bpf_ringbuf_reserve()`：预留 Ring Buffer 记录。

有些 helper 只允许 GPL 兼容程序使用，因此示例会声明 GPL license。

## 5. Map

Map 是内核 eBPF 程序和用户态之间共享的状态容器。

- Hash：按 key 保存 value；
- Array：固定索引，访问成本稳定；
- Per-CPU Array：每个 CPU 独立保存数据，减少锁竞争；
- Ring Buffer：按时间顺序传递变长事件。

Map 查询可能返回 NULL，使用返回值前必须判空。

## 6. Verifier

Verifier 通过静态分析证明程序满足安全约束。它不会“猜测程序大概安全”。常见要求：

- 指针来源已知；
- 内存访问范围可证明；
- Map 查询和 Ring Buffer 预留结果已判空；
- 循环次数有明确上界；
- 所有执行路径都能终止；
- helper 参数类型符合约定。

写 eBPF 的关键不是让代码在人看来正确，而是让安全性能够被 verifier 证明。

## 7. CO-RE 与 BTF

BTF 描述内核类型。`vmlinux.h` 可以由当前内核 BTF 生成。CO-RE 程序借助类型和字段信息，
减少不同内核版本之间的适配成本，但它不保证任意程序能在所有内核上运行。
