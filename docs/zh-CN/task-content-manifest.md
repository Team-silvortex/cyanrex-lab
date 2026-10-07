# 任务内容元数据与精确绑定

状态：**0.5.0 收录的 C2-K 内部准备层**。决策日期：**2026-10-03**。
本轮实现有类型的内容清单与纯快照校验，**不保存元数据、不开放 HTTP 命令、不切换认证，也不连接浏览器保存。**

本地编辑器有文件名与语言，准备层 Task 存储只有标题和精确 Artifact 输入引用。新契约明确这些内容
如何对应，不向既有持久记录偷偷加字段，也不把浏览器 ID 当成服务端权威。

## 内容清单契约

[`TaskContentManifest`](../../engine/src/models/collaboration/content.rs)序列化为
`{ schema_version: 1, title, payload }`。每个有序项只含
`{ kind: "text", filename, language, artifact }`；`artifact` 是完整既有 `ArtifactRef`，包括
authority/workspace、Artifact ID、修订 ID 与 SHA-256。不存在浮动的最新版本、文件路径、所有者、
Session、本地项 ID、本地修订、正文或执行字段。

字段私有，构造与反序列化执行相同的结构校验。清单、内容项、Artifact 和 workspace 必须是 JSON
对象，不接受位置数组；`kind` 必须是字符串 `"text"`，不接受枚举形状对象。未知字段被拒绝。
`parse_json` 还会在解析前拒绝超过 64 KiB 的输入。
解析不是查询：精确引用仍可能不存在、不可读、被伪造或超出操作者权限。

| 字段 | 契约 |
|---|---|
| 标题 | 检查空白后非空，最多 256 UTF-8 字节，无控制字符；其他内容原样保留 |
| payload | 有序 0–32 项；有标题的空任务不需要虚构代码或 Artifact |
| 文件名 | 1–128 个 UTF-16 编码单元的展示标签；禁止斜杠、反斜杠、控制字符、`.` 和 `..`；不裁剪或规范化 |
| 语言 | 可扩展的 1–64 字节提示，匹配 `[a-z][a-z0-9_+.-]*`；不是执行器、provider、策略或编辑能力授权 |
| 引用 | 同一 workspace；同 workspace/Artifact/修订重复时拒绝，即使摘要不同；同一 Artifact 的不同修订仍可分别出现 |

文件名可以重复。为兼容既有本地元数据，标签保留 Windows 保留名、前后空格和 bidi 格式字符。
它不能用作路径或受信任终端/HTML 输出；导出器须独立处理安全文件名。元数据校验不调用浏览器下载名清洗器。

后端语言提示不封闭在前端 14 种配置中。保留一个新提示不安装或选择语言服务器。当前前端导入本地草稿时
仍拒绝未知语言 ID，本轮没有添加回退或改变导入行为。

## 精确快照校验

`validate_task_snapshot` 比较清单标题与给定 `TaskSnapshot` 的有序精确引用，并核对任务 workspace
和 owner authority。它不约束生命周期状态，也不声称修订是最新版本；历史快照同样可以被描述。

[`validate_task_content_snapshot`](../../engine/src/services/task_content.rs)继续按顺序检查给定的
`ArtifactContent` 列表：

1. 数量与清单逐项对应，不能缺项或多项。
2. 每个完整 Artifact 引用都相同，内容所有者等于 Task 所有者。
3. 声明字节长度等于实际字节，每项最多 256 KiB。
4. 正文是合法 UTF-8，允许 TAB/LF/CR，但拒绝其他控制字符。
5. 从这些精确字节重新计算的 SHA-256 必须匹配固定摘要。

空文本、CRLF、Unicode 组合形式和标记文本保持原样，不求值、不规范化、不按扩展名推断执行。
Artifact kind/media-type 标签不替代字节校验，也不选择编译器。验证器没有文件、数据库或网络 I/O，
不返回凭据、已认证所有者句柄或可重用的授权回执。

调用方完全可能同时伪造一组自洽快照。因此这些检查只证明内部一致性，不证明可信来源、实际存储存在或
当前权限。未来调用方必须通过当前 Session 事务适配器取内容，保留命名空间固定、写后检查和最终授权。
此前校验成功不能授权后续保存或 Review。

## 兼容范围与剩余工作

本契约**不是**浏览器的 `cyanrex.task-draft` 导入格式。后者允许空标题、本地 ID/修订与正文，完整 JSON
上限为 8 MiB，并采用 JavaScript 解析语义；本清单要求非空服务端标题、精确服务端引用、不带正文并严格
校验版本化 JSON。BOM、重复对象字段和无效 Unicode 会被拒绝。不默默补标题，不把本地 ID 转成 Task ID，
也不把本地修订转换为服务端预期修订。

0.5.3 收录的 [C2-O 发布映射](task-draft-publication.md)新增独立的严格整份草稿导入器：拒绝空白标题、
丢弃经过校验的本地 ID/修订，接受服务端可扩展语言提示，并将给定精确内容绑定为本清单。
它不发布或授权保存，也不改变浏览器现有 JSON 解析器及语言白名单。

本 C2-K 切片不改变既有 `TaskSnapshot`、`ArtifactRevision`、Task schema 2、core schema 1、outbox、公共 API 和 SDK。
同样收录于 0.5.0 的独立 [C2-L 存储](task-content-store.md)在 Schema 3 命名空间和专用列中持久化清单，
不塞进 32 KiB Task 快照。Artifact 标题不充当文件名，两者的长度和语义
不同。当前 Draft 输入换版命令仍不能保存仅修改标题、文件名或语言的操作。

C2-L 可信存储提供元数据原子更新，[C2-M](session-task-content.md)另行组合当前 Session 和 Artifact
文本字节检查；[C2-N](task-content-http.md)定义尚未挂载的 HTTP 准入层，仍缺 Session 签发及
浏览器保存、读取、冲突和结果不确定处理。
内容发布仍独立于 Task 变更；失败不允许删除内容或盲目重试。分享、类型化领域证据、规则 Review、验收、
通用执行与迁移分别保留边界。

## 验证范围

[`task_content_contract_tdd.rs`](../../engine/tests/task_content_contract_tdd.rs)覆盖构造与 JSON 校验一致性、
长度、Unicode 标签、可扩展提示、精确引用、重复项、空 payload 和 Task 快照匹配。
[`task_content_binding_tdd.rs`](../../engine/tests/task_content_binding_tdd.rs)覆盖字节、摘要、所有者与顺序
对应，不连接数据库或浏览器。实际结果记录在[项目进度](project-status.md)，不是服务端保存或部署验收。
[功能张量](capability-maturity.md)将本能力单列为准备层坐标，不提升仍缺失的浏览器保存桥评分。
