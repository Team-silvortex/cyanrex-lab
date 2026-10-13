# cyanrex 前端

用于 Cyanrex eBPF 实验的 Next.js 控制界面。

页面组合共享组件和功能控制器；运行时端点位于 `src/config/`，特权行为仍留在 Engine。
完整前端／Engine 边界见[系统架构](../docs/zh-CN/architecture.md)。

## 已实现页面

- `/dashboard`：服务概览和快捷操作
- `/ebpf`：Monaco 编辑器、模板、脚本持久化、运行时控制和 attachment 管理
- `/helper`：Engine 与 eBPF 环境诊断
- `/modules`：模块目录和生命周期控制
- `/events`：事件筛选、导出、删除和实时更新
- `/settings`：每用户事件保留设置
- `/terminal`：教师管理的模块命令总线和 eBPF 工作区交接（兼容旧管理员；不是系统 shell）
- `/login`、`/register`、`/otp-setup`、`/account`：账户和 TOTP 流程

界面支持简体中文、英文、西班牙语、日语和韩语（`ko`，한국어）。
可在登录页或侧栏选择韩语，当前浏览器会记住选择。
865 条自有界面文案完整翻译，包含本地编辑器说明和安全确认。
课程正文仍为英文／中文；韩语正文缺失时明确提示回退到英文。
`npm run test:korean-browser` 覆盖生产页面切换、撤销、确认、小屏布局、偏好保存和课程回退。
范围见[翻译说明](../docs/zh-CN/localization.md)。

## 开发

```bash
npm ci
npm run dev
```

验证生产构建：

```bash
npm run build
```

## 编译器与编辑器智能回归

`npm run test:compiler-check` 运行可移植的请求／期限／取消测试，纳入 CI 和前端质量门禁。
`npm run test:compiler-diagnostics-browser` 还会在 Chromium 中，用受控 fetch 响应和暂停时钟
运行真实 React hook。它通过已安装 Next.js／TypeScript 依赖构建临时夹具，不需要运行中的
应用、Engine、账户、网络访问或内核工作。设置 `CYANREX_PLAYWRIGHT_MODULE` 指向已安装
Playwright 模块；必要时设置 `CYANREX_CHROMIUM_PATH` 指向 Chromium 程序；这里不新增 vendored 副本。

既有 `npm run test:safety-browser` 包含生产编辑器的远程过期／取消和显式源码转交确认。
它需要另行启动本地生产前端，地址为 `CYANREX_UI_BASE_URL`（默认 `http://localhost:3217`）；
Engine 请求仍是夹具。这不是真实 Engine／Agent 或 LAN 验收。见[问题排查 03](../docs/zh-CN/functional-network-bug-hunt-03.md)。

`npm run test:semantic-completion` 覆盖编辑器持有的传输／缓存／取消，也纳入 CI 和前端质量门禁。
`npm run test:editor-intelligence-browser` 在 React StrictMode 下，使用模拟 Monaco 模型／fetch
和暂停时钟，运行实际控制器与语言提供器实现。它需要相同的可选 Playwright／Chromium 配置，
而不需要运行中的 Engine。生产安全测试还会在真实 Monaco 中插入语义建议和多行片段，并验证
元数据错误后的显式本地头文件检查。不同测试边界见[问题排查 04](../docs/zh-CN/functional-network-bug-hunt-04.md)。

## 手动运行与 attachment 回归

`npm run test:runtime-request` 覆盖私密传输、完整 body 期限和运行时响应验证，纳入 CI 和前端质量门禁。
`npm run test:runtime-lifecycle-browser` 在 React StrictMode 下，用模拟响应和暂停浏览器时钟运行
实际控制器；覆盖重复准入、导航／草稿变化、迟到响应和清单核对，不需要运行中的前端或 Engine。
它需要上述相同的可选 Playwright／Chromium 配置。

`npm run test:runtime-safety-browser` 使用另行构建并启动的前端、真实 Monaco 和模拟的 Engine
HTTP／WebSocket 流量。设置 `CYANREX_UI_BASE_URL`，并让 `CYANREX_UI_ENGINE_URL` 匹配构建时的
`NEXT_PUBLIC_ENGINE_URL`。与 `test:safety-browser`、`test:auth-session-browser` 一起运行，可覆盖
确认／导航保留。测试不加载真实程序，也不联系现用 Engine。浏览器取消不等于内核回滚；
决定是否重试不确定操作前，应刷新清单。证据与剩余限制见[问题排查 05](../docs/zh-CN/functional-network-bug-hunt-05.md)。

## 调试会话与事件恢复回归

`npm run test:breakpoint-lifecycle-browser` 使用真实 React hook、模拟编辑器模型和 socket，以及
暂停时钟；检查会话／Engine 变化、已声明的插桩行、有界命中历史、重连和精确编辑器／模型清理。
`npm run test:breakpoint-safety-browser` 使用已构建应用和真实 Monaco，浏览器可选设置与运行时
测试相同。另行运行既有 `test:event-stream-browser`，可覆盖事件中心共享的恢复控制器。

扩展后的可移植 `test:event-stream` 仍纳入 CI 和质量门禁。浏览器测试仍是可选项，并模拟全部
Engine 流量；不加载内核程序。F9／行号栏变化用于准备之后的手动运行；清空请求的断点不会
移除已经在内核运行的 probe。见[问题排查 06](../docs/zh-CN/functional-network-bug-hunt-06.md)及
[事件恢复](../docs/zh-CN/event-stream.md)。

## 事件历史与读取确认回归

`npm run test:event-page` 覆盖筛选验证、响应解析和私密请求期限，纳入 CI 和前端质量门禁。
`npm run test:event-page-browser` 在 React StrictMode 下挂载真实 Events 页面、Sidebar 和确认组件。
只有路由／页面存储和 Engine 传输使用夹具，并提供受控响应和暂停时钟；不需要运行中的服务器。
它使用上述可选 Playwright／Chromium 设置，以及已安装的 Next.js／TypeScript 工具链。

`npm run test:event-actions-browser` 运行已构建前端的原生 JSON 下载、四语言 320 px 非法区间
控件和显式读取确认重试。与既有事件、确认、运行时、断点和退出登录生产浏览器测试一起运行。
全部 Engine 请求均被模拟。读取确认仍影响当前账户的全部事件；没有增加筛选 ID 或持久读取
确认。见[问题排查 07](../docs/zh-CN/functional-network-bug-hunt-07.md)。
