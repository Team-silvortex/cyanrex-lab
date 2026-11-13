# 严格任务草稿导入与发布映射

状态：**C2-O 内部准备层，已收录于 0.5.3**。决策日期：**2026-10-07**。
这一纯逻辑边界将整份文本草稿转换为有界发布计划，再核对给定 Artifact 内容与计划是否一致，
构造既有的[任务内容清单](task-content-manifest.md)。它不进行文件、数据库或网络 I/O，不发布内容，
不调用 C2-M，也不挂载 HTTP。正常 Next.js 编辑器仍仅在本地工作，计划或匹配的给定内容不等于 Task 已保存。

## 导入契约与兼容范围

输入沿用本地草稿外形：`{ format: "cyanrex.task-draft", version: 1, title, payload }`。
每个 payload 项恰好包含 `{ kind: "text", id, revision, filename, language, text }`。
新的服务端导入器刻意严于 JavaScript `JSON.parse`：拒绝 BOM、重复或未知字段、位置数组记录、
非法 Unicode、不支持的版本和不安全整数修订。它不改变浏览器现有的导入行为。
`version` 和 `revision` 都要求 JSON 整数记号：即使浏览器经 `JSON.parse` 会归一化 `1.0`、`1e0`，
此处也拒绝这两种形式；浏览器的规范序列化出口不会生成它们。

本地项 ID 与修订会经过格式校验，并在适用处检查唯一性，之后丢弃。它们不选择 Task、Artifact、
服务端修订、所有者、命名空间或认证源。服务端身份与预期 Task 修订仍应在未来授权命令边界由
调用方单独传入，不能从文件名或文本摘要推导。

| 输入 | 限制与行为 |
|---|---|
| 整份 JSON | 解析前至多 8 MiB，包含转义与元数据 |
| 标题 | 非空白、至多 256 个 UTF-8 字节、无控制字符；其余原文保留 |
| Payload | 按序零至 32 项文本；有标题的空 Task 不需要虚构 Artifact |
| 正文 | 每项至多 256 KiB UTF-8；允许 TAB/LF/CR，拒绝其他控制字符 |
| 文件名 | 沿用清单展示标签规则，包括 128 个 UTF-16 单元上限；不是文件系统路径 |
| 语言 | 沿用服务端可扩展提示语法；合法提示不安装或授权语言工具 |

本地标题为空或全为空白时，在构造发布计划前拒绝。导入器不补造标题、裁剪标签、归一化 Unicode
或改写换行。本地编辑仍可保留未命名草稿；重复文件名仍合法，内容顺序和精确文本均保留。

在 C2-O 实现阶段，服务端接受超出浏览器 14 种配置白名单的提示。**2026-10-08 未发布的编辑器
跟进**让本地草稿也接受这套有界提示语法，保留未知提示，仅将本地编辑能力解析为纯文本并显示
无工具提示。这不改变 Rust 导入器及其更严格的原始 JSON 规则；合法提示仍不选择 LSP、编译器、
评估提供器或执行策略。详见[编辑器边界](editor.md)。

## 发布计划与精确绑定

[`TaskDraftPublicationPlan`](../../engine/src/services/task_draft_import/mod.rs) 提供 `parse_json`、`title`、
`items` 和 `bind_published(scope, owner, contents)`，其中内容项类型为 `PlannedTextPublication`。
这些访问器不持有 Session 或存储句柄。Unix 上的 `publication_draft(index)` 使用固定 kind
`cyanrex.task-text`、媒体类型 `text/plain`、Task 标题及该项精确字节构造 `ArtifactDraft`，
构造这一值本身不发布内容。

计划保存有界、按序的文本项及未来发布所需元数据。构造时先验证整份草稿，之后的发布才可能开始。
文件名不直接复用为 Artifact 标题：前者按 UTF-16 单元限制，后者另有非空白、256 字节契约。
相同摘要不能带来访问权，也不会把不同本地内容项合并成一个共享 Artifact。

计划不提前分配服务端 ID。绑定要求按计划位置为每项提供一个 `ArtifactContent`，给定范围、所有者、
精确修订引用、声明长度、文本字节与重新计算的 SHA-256 必须符合计划及既有文本内容契约。
缺失、多余或字节不匹配内容都会被拒绝，不返回部分清单。结果仍遵守既有 64 KiB 清单契约，
草稿正文不进入 C2-N 清单请求。

已有精确修订只要通过上述内容检查即可重用。绑定不要求其 Artifact 标题或 kind 与新发布默认值
相同，因此只编辑 Task 元数据不必重新发布未变字节。即使两个本地项正文相同，清单仍拒绝重复的
Artifact/修订坐标，但允许同一 Artifact 的不同修订。若两项正文相同，互换各自不同的引用仍可能
通过，因为计划固定的是预期字节，不是预先分配的服务端 ID 或过去发布调用的历史。

这些检查仅针对给定值，即使全部匹配，也可能是调用者一致伪造的数据；绑定不证明内容已发布、
存储来源可信、提交已确认或存在可复用权限。组合本纯层的发布方必须通过当前 Session Artifact 命令取得
结果，C2-M 修改 Task 前仍须独立复核引用、所有者与 Session。

## 未发布的浏览器侧既有 Draft 保存尝试

**0.6.6 已收录并扩展**；本节标题保留实施阶段锚点。[`taskContentSave.ts`](../../frontend/src/features/tasks/taskContentSave.ts) 与
[`taskSaveContract.ts`](../../frontend/src/features/tasks/taskSaveContract.ts) 提供纯逻辑、单次内容保存尝试。
`prepareTaskContentSave(draft, {task, manifest}, guard, allocations)` 冻结输入并异步计算本地 SHA-256，
不做 fetch、Cookie、环境、存储或 UI 操作；`TaskDraftWorkspace` 及其控制器仍只在本地工作。

`prepareTaskBoundContentSave(draft, {task, manifest}, guard, allocations)` 独立准备
[Task 绑定发布契约](platform-text-artifact-connection.md#unreleased独立-task-绑定-http)。
每个 publish 步骤在外层固定 `task_id`，正文严格只有 `schema_version`、`expected`、
`artifact_id`、`revision_id` 和 `text`。每次 `expected` 都是同一份深冻结的完整**原始**
Task／manifest，不是期望写入的新清单，也不随发布前缀改写基线。哈希前检查全部完整发布包络
不超过 2 MiB，每项正文仍限 256 KiB。新工厂不会隐式选择或回退到旧的不绑定工厂；旧外形和行为保持兼容。

Task 绑定 Artifact 确认要求**原 Task 标题**，即使本地草案同时改了标题。只有最后独立确认的
清单 PUT 使用新标题和新准确引用，其 `expected_revision` 仍为原 Task 修订。确认 Artifact 不等于
Task 已保存。合成共享样本 `engine/tests/fixtures/task-bound-content-save.json` 同时由浏览器计划器和
严格 Rust HTTP 适配器验证，只证明报文一致，不证明真实存储或权限。

严格基线必须是 Task／manifest 元数据一致的既有手工 Draft，固定目标、所有者、工作区及服务端预期修订。
本地项 ID／修订不上传；新的 Artifact／修订 UUID 由调用者显式分配。
guard 中的 `generation`、`revision`、`contextEpoch` 只阻止本地陈旧工作，不认证调用者或授予权限。
本模块不创建新 Task：调用方须先取得空 Task 的确认 201 基线，或明确读取既有 Task。

每项文本只交出一次发布描述，给定 Artifact 元数据须与分配 ID、固定 owner／scope 和计划文本摘要准确一致，
才能推进下一项。全部项之后才交出带固定 `expected_revision` 的完整清单 PUT；空／仅标题 payload 直接 PUT。
只有准确 Task 响应符合冻结尝试且修订为 **预期 + 1** 才完成，不自动清除当前编辑器 dirty 状态。

交出描述时即先记为 pending，此时尚无实际传输；只能保守称结果未知，不能证明已经派发 I/O。
停止、取消、guard 陈旧、冲突或非法确认都保留确认前缀及 pending，不重试、收编、删除、导入检查点或恢复。
guard 陈旧后晚到的回执不能确认该 pending。公开确认只检查给定 DTO 的一致性，即使匹配也不证明
网络来源或 COMMIT；可信传输及结果处理仍由调用方负责。

**0.6.6** 可选平台 Session 页通过 `body-contract.js` 及四个生成 ESM 镜像调用绑定计划器，
只有同时启用默认关闭的 `--task-text-publication` 和 `--session-ui`，并明确收到宿主能力声明后，
才允许本人手动 Draft 的既有固定文本槽编辑。数量与顺序固定；未修改槽和 textarea 未变前后缀
保留原 CRLF／混合换行。只有正文变化才按原槽顺序 POST 全部槽，每次携带相同完整原基线；
严格 HTTP 201 Artifact 回执匹配原 Task 标题后才能推进，最后才以原修订 PUT 新标题及新引用，
严格 HTTP 200 才确认 Task 保存。仅改元数据仍只 PUT，不发 POST。

确认前限制为最多 32 槽、每槽 256 KiB、完整转义草稿 8 MiB。取消、未知、冲突、401／403 响应头
或 pagehide 封闭计划并保留确认前缀及未知步骤；迟到回执不能推进，不自动重试、收编、删除或恢复。
失权／离页清私人 DOM，后续 Session 不能恢复工作；已确认发布可能仍未挂接 Task。

十三个资源为旧八个加纯桥及 `task-save/{taskContentSave,taskSaveContract,taskDraft,taskPayloadLanguage}.js`。
`scripts/generate-platform-task-save-assets.mjs` 用 Node 内置类型剥离从同名四份 TypeScript 源生成，
六项测试与 `--check` 检查漂移。Node 24.21 已完整验证，22.22 仅验证 ESM 运行；经官方 SHA-256
核验的 Node 22.23.3 已通过生成器 `--check` 与全部六项测试，输出字节与 24.21 一致。
正文浏览器 runner 新增 13 条 mock API 用例；[独立真实正文 runner](testing-guide.md#可选的独立宿主-session-网页)
必须分别选择明文和 TLS。明文已通过自然登录、两次严格发布、独立 Task 换版及准确 SQL／文件核验，
冲突／撤销首 POST 拒绝且无新写入；新增真实第二 POST 503／401 用例均核验通过，保留首确认
Artifact 且 Task 不变；两条原 Session／正文 TLS 选择另在隔离工作机通过并确认 SQL 停止。
真实迟到确认／BFCache 和部署仍未验收。
正常 Next 草稿与 SDK 仍未调用这些计划器；不增加 Session 入口、Origin 配置、代理／CORS 放宽
或旧教学 AuthService Cookie 回退，也不接 Cookie 安全部署、DNS 或局域网。
`npm --prefix frontend run test:task-content-save` 只选择纯状态／契约测试，
不证明实网浏览器保存、认证、持久恢复或部署验收；下方分日期 C2-O 证据保持原样。

## 秘密与不可信内容

草稿正文只是数据，包括代码、标记和配置。解析及绑定不执行正文、不读取其中路径、不访问 URL，
也不启动语言工具。未知凭据、所有者、路由和文件系统字段不能扩展允许的封装结构。根据
[目标架构](next-architecture.md)，Session token、Agent 密钥及认证材料应与任务记录和导出分离。

用户仍可能把秘密粘贴进合法文本。格式校验不是秘密检测、脱敏或可安全分享的保证。调用方必须
避免将原始草稿及返回字节写入诊断日志，也不能静默删除或改写文本。单份文档限制不是进程总内存、
并发或持久存量配额。
计划类型不提供包含全文的 `Debug` 表示，但这不能阻止调用方主动记录通过访问器取得的正文。

## 失败与下一边界

独立的0.5.3 收录的 [C2-P 逐步流程](session-task-draft-publication.md)现消费本计划，显式执行当前会话
发布与 Task 创建。它在内存保留确认前缀，遇到未知步骤便停止；本纯 API 不因此获得副作用或结果处理职责。

源码 0.5.4 上独立的 **2026-10-11 未发布 [C2-Z 目录步骤流程](session-catalog-task-draft-publication.md)**
通过独立强类型工作区及准确定义引用消费同一计划，不属于已发布标签。不做 I/O 的准备在随机
目标分配前冻结完整定义；后续每次推进发布一个 Artifact，或最后创建目录 Task。计划格式、
限制及纯绑定语义不变。新不透明尝试不能转换为手动 C2-P–W 流程，不新增检查点/日志/观察/
恢复或 HTTP/浏览器连接。

本片没有需要撤销的发布副作用，不实现多项上传、操作回执、幂等、结果核对、重试或清理策略。
真实 Artifact 发布与 Task 创建/替换仍是独立事务。后续流程若只确认了部分发布，就必须保留
这一区别；失败、超时或取消不能被描述为整份草稿已回滚，也不允许删除不可变内容或盲目重试写入。

[C2-N 路由](task-content-http.md)仍未挂载，只接受既有有界清单请求，不因新增导入器而获得正文
上传功能或 8 MiB 请求额度。安全 Session 签发、显式安装、公共发布及浏览器保存/读取/冲突处理
仍需另行实现。本准备层不迁移 Schema、公共 OpenAPI、SDK、在线认证或本地编辑器契约。

## 验证范围

回归边界是纯导入、计划构造及给定内容一致性：在不使用凭据、数据库或浏览器上传的情况下覆盖
大小限制、严格 JSON、Unicode、空标题/空 payload、扩展语言提示、有序绑定和不一致字节。
2026-10-07 的 18 项新增导入/绑定测试，与既有 12 项清单及 14 项给定内容回归均通过；
独立一项内部发布值单测验证默认 kind/media、精确字节和 Task 标题归属，也已通过。
本地浏览器草稿解析套件的 22 项通过，其中三项为新增共享夹具回归。这些是限定范围的运行，
不是完整质量门禁。
执行记录归[项目状态](project-status.md)；这些纯检查不代表浏览器保存已接通或部署已验收。
[功能张量](capability-maturity.md)将其记录为独立准备层坐标，公共与浏览器缺边仍然保留。
