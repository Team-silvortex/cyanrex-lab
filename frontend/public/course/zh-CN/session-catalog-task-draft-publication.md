# 会话授权目录草稿发布

状态：**C2-Z 内部准备层，已收录于源码版本 0.5.5**。决策日期：**2026-10-11**。
源码发布：**0.5.5**（**2026-10-11**）。本 Unix 专用仅创建流程发布
[C2-O 计划](task-draft-publication.md)中的文本项，再通过已有
[会话目录内容命令](session-task-content.md)创建私人目录 Draft。调用者每次推进一个写入。
它与手动 C2-P 尝试及其观察/检查点/日志流程分开，不增加 HTTP 路由、浏览器保存、评估或恢复协议。

分日期的未发布跟进及验证记录保留原源码版本与执行范围；收录于 0.5.5 不代表重跑或扩大这些证据。

独立的 **2026-10-11 未发布 C2-AA 跟进**为仍存活的未知尝试增加
[目录目标只读观察](session-catalog-task-draft-observation.md)。它使用原 token 和一个已有当前
会话读取器，返回不含正文的比较元数据，不收编进度、不改状态，也不允许重试。

## 准备与冻结定义

`DurableAuthSource::prepare_catalog_task_draft_publication(token, workspace, &reference, plan)`
返回不透明的 `SessionCatalogTaskDraftPublication`。可信 `SessionCatalogTaskContentWorkspace`
选择冻结目录及已有 Task/Artifact 路由；准确 `TaskDefinitionRef` 不是客户端完整定义、提供器、
所有者、命名空间或模式选择。

准备不访问数据库、文件或网络。先核对规范 token 外形、来源/工作区权限域一致性，精确查找
配置引用并冻结完整定义，再分配随机 UUIDv4 Task/Artifact/修订 ID 及预期 SHA-256 摘要。
未知引用或结构准备不合法返回 `InvalidPreparation`，不派发会话命令，也不回退手工任务。
本地编辑 ID、文件名和正文摘要不选择服务端 ID。消费的计划沿用 C2-O 零至 32 项文本及
每项 256 KiB 上限。

组合包和准确版本可提供两项评估绑定齐全或明确均不绑定的定义，两者都保留
`Some(TaskDefinition)` 的目录身份。准备不认证 token、不核验当前成员关系、不安装存储，
也不执行提供器或授予任务验收。

尝试在整个生命周期固定来源、工作区、计划和定义，只保存与手动尝试隔离用途的私有 Session
指纹，不保存原始 token。调用者在 `advance(&mut self, token)` 再次提供原 token；不同或非法
token 在派发前返回 `SessionChanged`，Ready 尝试保持 Ready。指纹相等不是授权：每个实际写入
仍独立在已有事务内检查当前 Session 和所有者。

只读访问器提供 `definition`、`state`、`task_ref`、`allocated_artifacts`、
`confirmed_artifacts` 和 `confirmed_task`。完整定义只是冻结元数据，不是评估器或可复用权限。
分配目标不等于已发布记录，确认元数据也不保证字节或权限以后仍有效。元数据需保密，token 和
草稿正文不得进入日志。

## 显式发布步骤

尝试复用已有 `SessionDraftPublicationState`、`SessionDraftPublicationProgress` 及类型化错误
报告结果，但不因此成为旧手动尝试。

| 状态或结果 | 目录流程含义 |
|---|---|
| `Ready` | 可以原 token 显式推进下一写入 |
| `Unconfirmed(Artifact { index, reference })` | 首次等待前已标记准确的计划 Artifact 目标 |
| `ArtifactConfirmed { index }` | 原会话 Artifact 命令返回匹配的完整元数据，确认前缀增长，状态回到 Ready |
| `Unconfirmed(Task { reference })` | 首次等待前已标记最终目录 Task 创建 |
| `TaskConfirmed` 与 `Complete` | 目录命令返回预期 Draft/清单及冻结完整定义，继续推进返回 `Stopped` |

每次推进至多调用一个已有当前会话命令。Artifact 步骤发布新文本，固定 kind
`cyanrex.task-text`、媒体类型 `text/plain`，使用计划标题及准确原始字节；正文相同也分别分配
引用。最后一项 Artifact 步骤不会同时创建 Task；空计划直接进入目录创建，不虚构 Artifact
或 owner。本流程不复用调用者给定修订、不修订 Artifact，也不替换已有任务。

Artifact 确认须匹配分配引用、权限域、共同所有者、序号一、无父修订、kind/标题/媒体类型及
字节长度。全部项确认后，共用纯清单 helper 要求每个计划项恰有一份确认，再把这些回执与
原始文本绑定。这是给定值自洽检查，不是额外文件读取或可复用授权。

最终 `create_session_catalog_content_task` 在自己的事务中独立重读真实所属文本，核验当前
Session、命名空间身份和完整配置定义。确认必须匹配预期任务引用、所有者、Draft 状态、
修订一、清单及 `snapshot.task.definition == Some(frozen_complete_definition)`。
只匹配引用或接受 `None` 都不足够，绑定选择、summary 及其余定义元数据都参与完整相等检查。
不派发 Evidence 校验、评估、Review、执行或自动验收。

## 取消与结果不确定

把尝试保存在可能取消的 future 之外。丢弃尚未 poll 的 advance 不派发工作；已派发步骤先记为
`Unconfirmed` 再等待命令，成功返回后不再等待便保存完整确认。派发后错误、取消或确认不匹配，
都保留已确认 Artifact 前缀及准确未知目标，并禁止继续推进。`InvalidConfirmation` 不计为进度，
也不撤销可能已经提交的写入。

Artifact 发布和最终任务创建仍分别提交事务，任务步骤失败不会撤销此前已确认发布。不删除
不可变文件、不撤销先前写入、不重试未知目标，也不报告整份草稿回滚。逐命令期限和锁限制
不变，不新增整条流程期限、并发尝试配额或全局存量上限。

销毁尝试或退出进程会丢失内存记录。确认前缀元数据不是持久回执、幂等键、恰好一次保证或
恢复权限；另行准备的新尝试可能生成逻辑重复工作。

## 与手动及日志流程隔离

不透明目录尝试没有 `Clone`、`Debug`、`Serialize` 或 `Deserialize`，不能转换为
`SessionTaskDraftPublication`，也不提供检查点、日志、恢复或重试方法。独立 C2-AA 观察不把
它转换为手动尝试，也不收编确认；它复用已有不含正文的报告 DTO。
已有 C2-Q–W 函数仍只接受原手动尝试/检查点/日志类型，共用状态/错误/进度不能跨越此类型边界。

与 C2-P 只共享两项纯检查：Artifact 回执匹配及清单构造。手动最终任务 matcher 继续要求
`definition == None`，目录 matcher 则独立要求冻结的完整 `Some` 定义。原手动方法、检查点
传输格式、日志 Schema 及派发/检查/观察语义不变。

C2-N、C2-X、C2-Y HTTP 路由均不调用本步骤流程。不增加正常应用、公共 OpenAPI/SDK、Session
签发、浏览器编辑器、迁移或部署组合。发布计划不等于原子批次、经过认证的检查点历史或重启恢复验收。

## 验证边界

专用默认目标及精确临时 PostgreSQL runner 独立于 C2-P 选择目录准备、逐步确认及不确定结果
边界。源码守卫检查不透明类型及没有手动/日志/公共接线；纯检查不能替代真实 Session、事务或
文件测试。当前选择清单见[测试指南](testing-guide.md)，实际分日期结果及限制见
[项目状态](project-status.md)。

[功能张量](capability-maturity.md)只扩展 W15 的实现引用，不改坐标、评分、证据或拓扑。
新目录源码不继承旧手动发布的执行或部署证据。
