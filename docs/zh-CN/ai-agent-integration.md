# AI Agent SDK 辅助适配与连接配置

收录于源码版本 0.5.3。决策日期：2026-10-07。这层服务 AI Agent 宿主，不是 Linux Runner Agent 签名协议；
不实现自主参与者、委托、模型推理或通用 Task 执行。

## 连接设置

教师在 **设置 → AI Agents** 管理 ID、名称、协议、服务地址、模型、凭据引用、启用状态及可选默认项。
协议包括 OpenAI Responses、OpenAI 兼容 Chat Completions、Anthropic Messages、Gemini GenerateContent
与自定义。模型名称由用户填写；选择协议不测试兼容性、不启动连接。

填 `OPENAI_API_KEY` 等宿主符号引用，不能填真实密钥。浏览器没有密钥输入/保存，Engine 不读环境
密钥、不解析引用、不连接模型。另行信任的 Agent 宿主负责密钥解析和模型传输，不能把 Engine Cookie
或 CSRF 请求头转发给模型服务。启用配置不等于获准执行工具。

教师专用 `GET /settings/ai-agents` 读取列表；`POST` 用
`{ expected_revision, default_profile_id, profiles }` 替换，保留 CSRF，全部响应私有且 `no-store`。
确认成功返回 `{ ok: true, settings }`，修订恰好加一。SDK 入口是 `client.aiAgents.settings()` 与
`client.aiAgents.updateSettings(request)`。页面先验证读取，再确认准确目标。当前页面仍存活时，
失败/未知保存保留只读草稿，必须显式重新加载才能再保存，加载失败不能丢草稿。确认前取消不写入、
不锁草稿；导航、Engine 切换或卸载会丢弃内存草稿并忽略晚到响应，没有持久恢复。丢失响应不是回滚，
不能自动重试或接纳配置内容已变的确认响应。

上限：16 个唯一配置；ID 为 `[a-z][a-z0-9_-]{0,63}`；名称/模型 1–128 个 Unicode scalar 字符，
非空白、无控制字符；URL 至多 2048 字符。只接受绝对 HTTPS，或原始 `localhost`、`127.0.0.1`、
`[::1]` 上的 HTTP，不带用户信息、空白、查询或片段。其他 IP 拼法不能靠 URL 自动改写成为回环。
凭据引用为 null 或 `[A-Z][A-Z0-9_]{0,63}`，默认项必须存在且启用，u32 修订拒绝溢出。
整个 JSON 请求独立限制 32 KiB；拒绝未知、重复和 API-Key 字段。

元数据保存在 `CYANREX_DATA_DIR/ai-agents/<runtime_instance_id>/settings.json`。构造或读取缺失文件
不写默认值；损坏、不安全或不可读的存储直接失败。同目录原子替换与 store 克隆共享准入保证
已派发调用取消后仍保持顺序；独立 store 或进程不共享锁。随数据目录备份，不是密钥库、迁移或
跨进程协调服务。
后端要求 Unix，其他平台返回存储不可用（503）。数据根目录默认 `./data`，实例名来自净化后的
`CYANREX_INSTANCE_ID`（默认 `default`）。`ai-agents` 和实例目录必须由服务用户拥有、权限 `0700`；
快照须为 `0600`、单硬链接普通文件，路径不能经过符号链接。备份恢复需保留所有权、权限和实例名，
不会自动修改已有共享父目录权限。

## 显式工具桥接

```ts
import { CyanrexClient } from "@cyanrex/sdk-js";
import { createAgentBridge, formatAgentResults } from "@cyanrex/sdk-js/agents";

const client = new CyanrexClient("https://engine.example", {
  csrfOrigin: "https://teacher.example",
}); // 宿主分别管理 Engine 会话和模型传输。
const bridge = createAgentBridge(client, { operations: ["getHealth", "getLearningAttempts"] });
const tools = bridge.tools("openai_responses");
// 用自己的模型客户端发送 tools；这里只接收完整 function_call 记录。
const results = await bridge.executeCalls("openai_responses", functionCallRecords, { signal });
const outputs = formatAgentResults("openai_responses", results);
// 回传 outputs 由宿主负责，桥接器不连接模型服务。
```

宿主必须明确选择有限工具列表。审核目录有 28 个现有 JSON 操作（23 个读、5 个写入/诊断），
不会开放全部 SDK。认证、课堂邀请、所有设置、管理命令、内核运行/解绑、模块变更和 Runner
任务派发被排除；准备层 Task/Artifact/Review 没有公共 SDK 路由。现有服务端角色/所有者检查仍有效。

写操作必须同时设置 `allowMutations: true` 和可信 `approveMutation({ call, operationName })`，
每次调用单独批准，回调收到冻结的准确目标。模型自报 `approved: true` 无法授权，诊断也适用。
请接真实用户确认或已审核的宿主策略，不能用无条件放行回调。

整个批次先验证再派发，逐项串行执行；批次不是事务，单项拒绝/派发失败后仍继续后续项，宿主须逐项
处理结果。拒绝未知工具/参数和原型敏感键。每批最多 32 个唯一调用，单项参数 64 KiB，
每个 bridge 1024 个已消耗 ID，单项结果 JSON 复制限 1 MiB。拒绝/未知结果也消耗 ID，
不能在该 bridge 重试；内存账本不是跨重启/新 bridge 的持久去重。派发失败返回通用未知错误，
不泄露私有异常、不证明回滚；不能换 ID 盲重试写入。取消依赖宿主传输/审批配合，不另加总期限或
HTTP 流总内存上限；结果上限是在操作返回后检查。
忽略取消的晚到成功也不能确认，取消后不派发余下调用。

格式包括 `openai_responses`、`openai_chat_completions`、`anthropic_messages`、
`gemini_generate_content`、`mcp`、`custom`。OpenAI 显式 `strict: false` 保留可选输入，只规范化
完整非流式记录。Gemini/MCP 要稳定字符串调用 ID；缺失或为数字时由宿主明确关联
`{ id, name, arguments }` 后使用 `custom`。JSON-RPC 外壳、Gemini thought signature 和对话元数据
仍由宿主管理。这是格式辅助层，不是模型客户端、MCP 服务端或框架工作流引擎。

另导出 `validateAiAgentProfile`、`validateAiAgentSettings` 和宿主专用
`resolveAgentCredential(profile, resolver)`，只用显式回调解析，不能把结果送给模型或日志。
只能解析宿主白名单内的引用，不能按配置任意读取环境变量。工具结果可能包含私人代码、评语或
运维数据，宿主负责同意、脱敏及披露策略。

## 验证与依据

入口为 `ai_agent_settings_tdd`、前端 `test:ai-agents`、显式 `test:ai-agents-browser`、SDK
`npm run check` 和公共 `agentToolGenerator`/`aiAgentSettingsContract` 回归。合成记录与浏览器夹具
不证明真实模型连接，实际执行记录见[项目状态](project-status.md)。

协议依据：[OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling)、
[Anthropic tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)、
[Gemini FunctionDeclaration](https://ai.google.dev/api/generate-content#FunctionDeclaration)、
[MCP tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)。
