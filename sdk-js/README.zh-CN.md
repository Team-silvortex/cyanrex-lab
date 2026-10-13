# @cyanrex/sdk-js

Cyanrex Lab Engine API 的强类型浏览器和 Node.js 客户端。

本包描述已注册的现用 HTTP／WebSocket 协议，尚未公开通用协作 Task／Artifact／Review 的
Session 命令，也不持久化本地任务 payload 编辑器。不要从 Rust 方法名臆造任务端点，也不要把
本地草稿 ID 当成服务端引用。新平台 API 必须一起完成明确路由、授权、OpenAPI 和 SDK 工作；
参见[项目状态](../docs/zh-CN/project-status.md)和[测试指南](../docs/zh-CN/testing-guide.md)。

## 使用

```ts
import { CyanrexApiError, CyanrexClient } from "@cyanrex/sdk-js";

const cyanrex = new CyanrexClient("http://localhost:8080");

await cyanrex.auth.login({
  username: "admin",
  password: "your-password",
  otp: "123456",
});

const modules = await cyanrex.modules.list();
const check = await cyanrex.ebpf.check("int main(void) { return 0; }");

// The generated operationId surface stays synchronized with OpenAPI.
const health = await cyanrex.operation("getHealth");
await cyanrex.operation("postModulesStart", {
  body: { name: "module-network" },
});

try {
  await cyanrex.command.dispatch({ commandType: "StartModule", moduleName: "module-network" });
} catch (error) {
  if (error instanceof CyanrexApiError) {
    console.error(error.status, error.message, error.details);
  }
}
```

客户端覆盖认证、模块、教师管理命令总线、事件、设置、脚本、学习、eBPF 检查／运行／挂载、
Runner 状态／管理任务和环境诊断。每个方法都接受可选 `{ signal }` 用于取消。生成的 `operation()`
层覆盖全部 65 个浏览器操作，并根据 OpenAPI 推导必需正文、查询参数和响应类型。五个签名
Runner Agent 协议操作仍与浏览器 SDK 隔离。`client.request<T>()` 保留用于向前兼容调用。

生成的分派器还保留非 JSON 行为：`getEventsExport` 返回 `ApiDownload`，`getWsEvents` 解析为
经过认证的 WebSocket URL。原命名空间仍是稳定、面向任务的外观；operationId 接口属于扩展。

事件连接要求会话 cookie 和允许的 Origin／Referer，原生客户端同样如此；URL 辅助函数不会设置
这些请求头，也不管理重连。帧仍是原始 Event 记录。关闭代码 `1013` 表示可能发生广播滞后：
应退避重连并刷新保留的 `/events` 历史。发送缓慢也可能异常结束；不保证完整重放。
迁移和恢复限制见[事件流恢复](../docs/zh-CN/event-stream.md)。

仅扩展的命名空间基线及废弃窗口见[稳定性政策](STABILITY.zh-CN.md)。质量门禁拒绝删除或重命名
已记录的公开客户端路径。

课堂接入使用 `classroom.discovery()`、`invitations()`、`invite({ username })`、`revoke(inviteId)`、
`join(request)` 和生成的 operation ID。除回环地址外须使用 HTTPS；禁用缓存／重定向，写操作保留
CSRF 防护。必须独立确认教师的准确来源；发现元数据和兼容产品补丁不证明身份。邀请绑定姓名、
仅可消费一次、有效十分钟。加入只创建学生账号，返回一次 TOTP 初始化信息，不自动登录。
不要记录私有链接或自动重试已消费／结果不确定的邀请。撤销邀请不撤销已有账号或会话。
参见[课堂连接](../docs/zh-CN/classroom-connection.md)。

教师（包括兼容旧管理员）可对已有实验提交写入学生可见的反馈：

```ts
const { attempts } = await cyanrex.learning.teacherAttempts("student");
const attempt = attempts[0];
if (attempt) {
  await cyanrex.learning.saveTeacherFeedback({
    username: attempt.username,
    attempt_id: attempt.id,
    comment: "Explain why the pointer is safe before dereferencing it.",
    expected_revision: attempt.teacher_feedback?.revision ?? 0,
  });
}
```

评论为去除首尾空白后的纯文本（1–2000 个 Unicode 字符）。服务端指定审阅者和时间，学生通过
`learning.attempts()` 读取。过期修订返回状态 `409` 的 `CyanrexApiError`：重新读取、比较后再明确
提交，不自动重试覆盖。反馈不改变自动验收。Node 客户端须按会话说明配置 `csrfOrigin`。

使用 `cyanrex.learning.attempt(attemptId, { signal })` 或
`cyanrex.operation("getLearningAttempt", { query: { attempt_id: attemptId } }, { signal })` 读取当前
会话用户的一份提交。返回原始源码、实验／模板上下文和当前反馈，不更新提交、不执行代码。
没有用户名覆盖参数，工作人员也一样；不存在／他人记录返回 `404`，无效 ID 返回 `400`，记录
存储错误返回 `500`。响应标记 `Cache-Control: no-store`。

教师现在负责教学与部署管理，包括预置的单人账号。登录／会话响应将旧管理员身份表示为
`teacher`；schema 保留 `admin` 以兼容。operation 访问标签 `admin`、`staff` 是稳定历史类别，
均要求教师权威；OpenAPI 的 `x-cyanrex-roles` 明确记录这一点。命名空间、operation ID 和请求／
响应形式保持兼容。客户端不能仅因为会话是 `teacher` 而非 `admin` 就隐藏部署控件。

公开请求／响应模型来自 Engine OpenAPI component schema。手工设计的命名空间保持稳定，
质量门禁拒绝过期生成类型。完整生成映射也有明确的类型子路径：

```ts
import type { OpenApiSchemas } from "@cyanrex/sdk-js/openapi";

type RunnerJob = OpenApiSchemas["RunnerJobView"];
```

无需创建客户端即可读取 operation 元数据：

```ts
import { openApiOperations } from "@cyanrex/sdk-js/operations";
import type { OpenApiOperationInput } from "@cyanrex/sdk-js/operations";

type EventInput = OpenApiOperationInput<"getEvents">;
console.log(openApiOperations.getEvents.access); // authenticated
```

## AI Agent 集成

`@cyanrex/sdk-js/agents` 导出 `createAgentBridge`、提供方工具／调用／结果适配和配置验证器。
工具须明确选择；修改操作还要求可信宿主逐调用批准回调。格式覆盖 OpenAI Responses／Chat、
Anthropic Messages、Gemini GenerateContent、MCP 和自定义宿主。
`client.aiAgents.settings()`／`updateSettings()` 管理仅教师可用的连接元数据；凭据引用仅由
宿主提供的回调解析，不由 Engine 或浏览器解析。这些辅助函数不调用模型、不启动自主 Agent、
不改变服务端权威。示例、限制和结果不确定情形见[AI Agent 集成](../docs/zh-CN/ai-agent-integration.md)。

## 兼容基线

SDK 构建前检查已提交的 `0.3.0` OpenAPI 基线。原输入必须继续接受，成功响应必须保留既有
保证；访问层级改变须明确重置兼容性。增加操作或可选输入／输出可以通过。

运行 `npm run check:compatibility` 检查候选协议。用
`node ../scripts/api-compatibility.mjs --write-baseline` 替换基线，仅适用于经评审的有意破坏性发布；
常规生成和质量检查不会重写基线。

运行 `npm run check:surface` 检查手工客户端命名空间基线。根据稳定性政策，写入基线只用于
已接受的新增项或独立评审的破坏性发布。

## 会话与 CSRF

浏览器请求使用 `credentials: "include"`；浏览器持有 `HttpOnly` 会话 cookie 并添加 `Origin`。
Node.js 客户端捕获登录返回的 `cyanrex_session` cookie，用于后续调用。配置 Engine 允许的
前端来源，使非安全方法的请求满足基于 Origin 的 CSRF 检查：

```ts
const cyanrex = new CyanrexClient("http://localhost:8080", {
  csrfOrigin: "http://localhost:3000",
});
```

与外部 cookie jar 集成时使用 `getSessionCookie()`、`setSessionCookie()` 和自定义 `fetch`。
不要将 Node 管理的会话 cookie 暴露给浏览器 JavaScript。

## 开发

需要 Node.js 18 或更新版本。

```bash
npm ci
npm run generate # after intentionally changing OpenAPI schemas or operations
npm run check:compatibility
npm run check
npm run test:package
```

`npm run build` 在 `dist/` 生成 ESM JavaScript、声明、声明映射和源码映射。
`npm run test:package` 验证 npm 清单试运行、相对声明／运行时导入和消费者形式的全部导出加载。
兼容与发布政策正式批准前，包保持私有。
