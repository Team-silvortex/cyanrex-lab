# TDD 政策

本仓库默认采用测试驱动开发。

这项政策覆盖现有教学运行时、通用协作准备层和任务持有的编辑器。
完整矩阵见[当前测试指南](docs/zh-CN/testing-guide.md)；[历史测试网络](docs/zh-CN/testing-network.md)
记录的是过去的一次执行，不是今天的完整测试套件。

## 必须遵循的循环

1. 先编写或更新一个失败测试（Red）。
2. 用最少代码让测试通过（Green）。
3. 保持测试通过并重构（Refactor）。

## 后端路由修改的最低要求

- 每条新路由必须至少有一个成功用例。
- 行为变化必须有回归测试。

## 服务与跨模块修改

- 测试通用契约时，不假定教学角色、代码 payload 或 Run。
- 持久化命令应在可丢弃存储中覆盖所有权／scope、过期修订、no-op 拒绝、原子事件、
  写后验证和不确定提交结果。
- Session 命令必须覆盖撤销、过期、账户 incarnation，以及等待／取消边界；
  可信直接存储的成功路径不能验证授权。
- 本地编辑器应覆盖父级持有的状态、过期回调、确认目标和拒绝的修改。
  浏览器夹具必须说明哪些 Engine 调用被模拟，以及是否使用真实 worker。
- 添加通用能力时保留既有教学和 SDK 契约。为需显式启用的边界添加明确 CI 测试选择，
  并检查每个被选择的测试名确实存在。

## 命令

```bash
cargo test --manifest-path engine/Cargo.toml --locked
```

这是默认 Rust 测试套件，不是全部 PostgreSQL、浏览器、SDK 或特权内核验收。
更广的默认门禁使用 `./scripts/quality-gate.sh`；单独启用的测试套件见测试指南。

## 可选学习流程集成检查

迁移／并发测试应使用可丢弃 PostgreSQL 数据库。它会创建和删除自己的随机 schema；
不要指向生产数据库。

```bash
CYANREX_TEST_DATABASE_URL=postgresql://USER@localhost/TEST_DATABASE \
  cargo test --manifest-path engine/Cargo.toml --lib postgres_feedback -- --ignored
```

教师／学生浏览器流程应先构建前端，在本地端口 3217 提供服务，再运行
`npm --prefix frontend run test:learning-browser`。该可选测试要求已安装 Playwright 运行时和
Chromium。`CYANREX_PLAYWRIGHT_MODULE` 可以指向已有 Playwright 模块，
`CYANREX_CHROMIUM_PATH` 可以指向已有浏览器程序。用 `CYANREX_UI_BASE_URL` 覆盖前端地址，
并用 `CYANREX_UI_ENGINE_URL` 匹配其配置的 Engine URL。所有 Engine 调用均被模拟；
这项检查验证 UI 行为，不验证现用认证或内核执行。后端路由测试另行覆盖授权、CSRF、所有权、
持久化错误和修订冲突。
