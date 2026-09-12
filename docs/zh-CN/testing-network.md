# 按模块与边界划分的测试网络

**2026-09-09 修复跟进**：下列 9 个问题已修复，原有回归断言均通过。修复后运行及新增并发、
取消、SQL/缓存边界测试见独立的[修复验证](../../reports/acceptance/2026-09-09-boundary-fixes/result.json)。
现有部署未更新。下文保留**修复前**快照：失败数量和“未解决”表述指当时运行，不代表当前修复状态，
原始证据未改写。

原始快照：**2026-09-09**，`main` 上尚未提交的 **0.3.7** 源码。
**原始结果未通过：9 个回归用例失败。** 这是源码级测试，不是发行验收，也不说明现有部署已包含这些改动。

先按职责划分模块，再按依赖顺序执行；单元测试、进程内 HTTP 路由、真实外部边界、
浏览器模拟响应分别记录。“模块通过”不等于所有分支、操作系统和部署拓扑都已覆盖。

## 1. 模块、功能与执行顺序

下表分数为**通过 / 执行**。Rust 便携测试不包含需要显式启用的外部集成。

| 顺序 | 模块与功能 | Rust 便携 | 外部边界 | Node / 浏览器 |
|---|---|---:|---|---|
| M00 | 配置、健康、环境、指标、版本/文档/契约同步 | 6/6 | — | 跨模块工具检查 53/53 |
| M01 | 密码/TOTP、会话、CSRF、账号变更、教师权威与单人默认教师 | 36/36 | PostgreSQL 16/20，**失败** | 单元 11/11；浏览器 3/3 |
| M02 | 教师最小发现、身份/版本/协议校验、邀请与学生加入 | 10/10 | 路由/服务邀请竞争；加入取消还由 M01 数据库测试覆盖 | 单元 3/3；浏览器 8/8 |
| M03 | 脚本所有权、保存/列表/删除、重载与文件写入失败 | 1/4，**失败** | PostgreSQL 持久化与跨用户隔离 1/1 | — |
| M04 | 模块目录/生命周期、头文件选择、摘要校验、终端命令 | 16/16 | 真实临时文件；模拟下载器，不联网下载 | 单元 2/2 |
| M05 | 模板、诊断/补全、源码校验、调试插桩、编译工作目录 | 20/20 | 不执行内核程序 | — |
| M06 | Runner 租约/配额/取消、驱动分发、Agent HMAC/防重放、任务与远端诊断 | 46/46 | 签名回环 HTTP + 真实 Clang 编译 1/1 | — |
| M07 | 课程、评定、尝试记录、快照、教师反馈/CAS、本人历史恢复 | 49/49 | PostgreSQL 迁移/CAS/重载 1/1 | 单元 9/9；浏览器 4/4 |
| M08 | 事件历史/筛选/导出/删除/未读/设置、分发、WebSocket 与重同步 | 33/35，**失败** | 真实 WebSocket 正常/突发/慢连接 3/3 场景 | 单元 13/13；浏览器 1/1 |
| M09 | 布局、权限、安全确认、草稿保护、多语言与构建 | — | 生产构建、TypeScript 通过 | 单元 6/6；浏览器 19/19 |
| M10 | OpenAPI、生成类型/操作、兼容性、SDK 请求与消费端打包 | 1/1 | 检查 68 个 OpenAPI 操作、63 个 SDK 操作 | SDK 运行时 13/13 + 包测试 3/3；类型测试通过 |
| M11 | Native 发布校验/解包、安装保护、SSH 目标复核、依赖审计 | 28/28 | SSH/Docker/安装包装层使用测试替身；审计通过 | 已计入 M00 工具检查 |

实际顺序：基础检查 → M00–M11 便携分组 → PostgreSQL/Agent/WebSocket →
生产构建/SDK/审计 → M01/M02/M07/M08/M09 浏览器分组 → 补事件 HTTP 边界 →
整组新增边界复测。失败不会阻止后续模块执行。
最终清单把每个 Rust 用例唯一归类，并核对精确筛选的执行数量，避免“选中零项却通过”。

汇总：Rust 便携 **246/251 通过**，PostgreSQL **18/22**，Agent **1/1**，
WebSocket **同一 ignored 测试的 3/3 个场景**。
共执行 **275 个不同 Rust 用例，266 通过、9 失败**，另有 2 个性能专用用例未执行。
重复运行不增加覆盖数量。前端单元 **44/44**、生产浏览器 **35/35**、
SDK 运行时/包测试 **16/16**、公共工具测试 **53/53** 全部通过。

## 2. 模块之间的连接边界

| 边界 | 具体测试 | 结果与限制 |
|---|---|---|
| 浏览器 → 认证 → 教师/学生权限 | 登录/会话路由；真实 SQL 变更与取消；浏览器退出失败、重复点击、导航取消 | **4 个 SQL 失败项**；浏览器 Engine 是模拟响应 |
| 发现 → 兼容性 → 一次性邀请 → 创建账号 | `classroom_tdd`、课堂浏览器、PostgreSQL 加入取消 | 所测不变量通过；取消请求仍可能留下已提交账号，不能当作回滚 |
| HTTP 会话 → 脚本所有者 → JSON/SQL | `module_boundaries/storage.rs`、`script_postgres.rs` | 身份/CSRF/SQL 隔离通过；**3 个文件一致性失败项** |
| 教师选头文件 → 学生读取元数据 → 编译/Runner | 选择重载/删除、摘要拒绝；编译驱动收到源码、所有者、头文件及光标 | 通过；下载器是替身，错误摘要数据不会发布为已下载头文件 |
| 保存源码 → Runner → 尝试记录 → 事件 → 教师评语 → 学生恢复 | `module_boundaries/learning_flow.rs` | 真实路由/服务/文件通过；驱动返回合成编译失败，**没有内核执行**；读取历史不触发第二次运行 |
| Runner → 驱动 / Agent 任务 | 所有权/配额/超时/取消、后端失败不回退；签名 HTTP 探针与 Clang 产物 | 通过；Agent 声明的隔离类型不等于验证了容器/VM 安全边界 |
| 事件查询/设置 → 保留历史 → 导出/删除/未读 | `module_boundaries/events.rs` | 读取/导出/CSRF/所有者隔离/容量通过；**2 个破坏性筛选失败项** |
| EventBus → WebSocket → 浏览器恢复 | 用户隔离路由；真实正常/突发/慢连接；浏览器 1013/快照交叠/换筛选 | 分层通过，不是真实浏览器到 Engine 的整条 WebSocket E2E |
| 学习存储 → PostgreSQL → 反馈版本/重载 | 旧表迁移、并发同版本写入、缺失/他人历史、存储故障 | 既有专用 PostgreSQL 用例通过 |
| OpenAPI → SDK 生成 → 打包消费端 | 契约漂移、类型、操作分发、兼容性、包导入/声明 | 通过；5 个签名 Agent 协议操作有意不纳入 63 操作 SDK |
| 教师 CLI → 已复核 SSH 目标 → 校验后的包 | 注入、主机密钥复核绑定、只执行一次、版本/校验和/权限拒绝 | 使用模拟 SSH 与安装包通过，**没有远程部署** |

真实 WebSocket 观察值（debug 构建的功能压力测试，**不是发行性能基准**）：
32 连接 / 8 用户在正常场景收到全部 8,000 份匹配事件；突发场景 32 条过载连接都请求重同步；
不读取数据的连接约 5.06 秒回收，订阅者与用户队列均归零。

## 3. 仍未解决的问题

| 编号 | 优先级 | 实际复现 | 回归用例 |
|---|---|---|---|
| TN-AUTH-01 | P1 | 旧凭据的在途登录能认证成删除后同名重建的账号 | `postgres_old_login_cannot_authenticate_as_a_recreated_account` |
| TN-AUTH-02 | P2 | 账号删除后，在途登录仍返回 200 与无效 Cookie，并关闭数据库使用 | `postgres_login_cannot_report_success_after_its_account_was_deleted` |
| TN-AUTH-03 | P2 | 会话级联删除被抑制时，删除报成功却留下有效持久会话 | `postgres_delete_suppressed_session_cascade_cannot_report_success` |
| TN-AUTH-04 | P2 | 会话 INSERT 被抑制时，登录报成功却没有有效会话 | `postgres_login_zero_row_session_insert_cannot_report_success` |
| TN-SCRIPT-01 | P1 | 读取失败的 JSON 存档被新保存覆盖，且返回 200 | `script_corrupt_snapshot_must_not_be_overwritten_by_a_successful_save` |
| TN-SCRIPT-02 | P2 | 文件保存失败，未落盘记录却发布到内存 | `script_failed_file_save_must_not_publish_an_unsaved_record` |
| TN-SCRIPT-03 | P2 | 文件删除失败，记录却已从内存移除 | `script_failed_file_delete_must_retain_the_previously_saved_record` |
| TN-EVENT-01 | P1 | 内存筛选删除保留匹配项、删除其余项；零匹配会删除全部记录 | `filtered_event_deletion_removes_matches_not_the_complement` |
| TN-EVENT-02 | P1 | 非法 severity/category/start/end 被丢弃为无筛选，最终全部删除 | `invalid_event_delete_filters_must_not_widen_into_delete_all` |

两项“写入/删除被抑制”的认证测试使用故障注入触发器，不代表默认表结构会自行抑制写入。
4 项认证问题延续上一轮；**3 项脚本与 2 项事件问题是本轮新发现**。
这 5 项保留为普通失败测试，没有改成 ignored；本轮没有修改业务实现，也没有宣称已修复。

事件删除反向选择位于 `EventBus::delete_from_history_filtered`：它调用读取用的
`filter_events`，却把匹配项重新发布为保留历史。非法筛选值则被路由规范化/日期解析
静默丢弃，进而进入无筛选删除分支。修复前建议不要使用事件筛选删除。
所有故障注入只作用于临时合成数据。

## 4. 安全复现

精确用例名与 target 见
[`plan.json`](../../reports/acceptance/2026-09-09-test-network/plan.json)，命令与结果见
[`result.json`](../../reports/acceptance/2026-09-09-test-network/result.json)。
证据中的路径和临时凭据替换为占位符，需在本机解析，不能直接执行占位字符串。

新增便携边界不连接数据库：

```bash
env -u DATABASE_URL -u CYANREX_TEST_DATABASE_URL \
  cargo test --manifest-path engine/Cargo.toml --locked --test module_boundaries_tdd \
  -- --nocapture --test-threads=1
```

修复前预期 **6 通过、5 失败、1 ignored**；ignored 是单独执行的 PostgreSQL 脚本测试。
既有模块可从清单选择 target 与精确名称：

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib -- --exact TEST_NAME --nocapture
```

数据库测试保持 `DATABASE_URL` 未设置，仅把 `CYANREX_TEST_DATABASE_URL` 指向新建的、
可销毁的 PostgreSQL，并设 `CYANREX_DB_FALLBACK=true`；
详见[数据库隔离步骤](acceptance.md#复现不加载内核的集成测试)。
脚本测试自建 schema、直接核对 SQL 行数而不是接受文件回退，断言任务失败后也会清理该 schema。
无论测试成功与否，最后都应销毁临时数据库。

浏览器测试使用**生产前端构建**：设置仅用于模拟的 `NEXT_PUBLIC_ENGINE_URL`，
并匹配 `CYANREX_UI_ENGINE_URL`、`CYANREX_UI_BASE_URL`、
`CYANREX_PLAYWRIGHT_MODULE` 与 `CYANREX_CHROMIUM_PATH`。
按清单逐个运行 `frontend/tests/*.browser.mjs`，Engine 响应都是模拟的。
首轮开发服务器结果 34/35；未改动的事件流测试在那里超时，在生产构建通过，最终整体 35/35。
该替身会替换全局 WebSocket，而开发 HMR 也依赖它；不把不合适测试环境的失败直接判为业务缺陷，
也不从运行历史中隐藏。

## 5. 明确未证明 / 未执行的路径

- 真实局域网教师/学生浏览器链路：TLS、信任变化、版本差异、断网与真实数据库组合；
  浏览器模拟和服务测试不能替代它。
- PostgreSQL 事件持久化队列/工作线程、重启及 SQL 到内存回退一致性；
  脚本 SQL 故障/取消路径（超出本次已通过的所有者/重载/并发保存）。
- 当前源码的真实 eBPF 加载/事件/卸载、多内核及 WSL2/Docker Desktop 矩阵、
  VM/容器隔离和逃逸防护；此前 VM 证据保留为历史，本轮没有重跑。
- 真实 SSH 上传/安装/重启、离线候选包安装、候选制品绑定的内核证据。
  模拟发布工具不能证明这些；未复用之前的 VM 或现有部署。
- 两项 ignored 事件总线纯性能测试；没有建立新的 release 构建 benchmark 基线。

本轮新增 **12 个测试用例**；未提交、推送、安装候选包、重启部署或加载宿主机内核程序。
临时 PostgreSQL 只绑定回环地址，数据存于 tmpfs。最终资源清理及源码输入哈希见结果记录。
