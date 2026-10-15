# Session 生命周期数据库权限

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后、2026-10-15 的未发布跟进，在
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下移除准备层 Source-only
Session 边界对整表凭据读取权限的依赖。明确受限的数据库角色可以通过
[未挂载的生命周期适配器](platform-session-lifecycle.md)查询和退出已有 Session，而不读取密码或 TOTP。
它不启动宿主、不签发 Session，也不改变教学应用认证。

## 已验证的角色边界

一次性测试使用独立 LOGIN／NOINHERIT 角色连接，不是管理员连接上伪装一个应用用户名。
角色没有源对象所有权、角色成员关系或命名空间 CREATE，仅在夹具私有 Source 中得到下列权限：

| 关系 | SELECT 列 | 行锁所需 UPDATE 列 | 表级权限 |
|---|---|---|---|
| `users` | `username`、`account_id` | `username` | 无 |
| `sessions` | `token`、`username`、`account_id`、`expires_at` | `token` | DELETE |
| `collaboration_auth_source_schema` | `singleton`、`version`、`authority_id` | `singleton` | 无 |

没有整表 SELECT／UPDATE、INSERT 或 TRIGGER 授权。对 `password_salt`、`password_hash`、
`totp_secret` 和 `otp_last_counter` 的四次真实独立投影必须各自返回 42501。
夹具还独立核对有效权限，包含恰好九个可读列与三个可更新列。这是该边界的实测要求，
不是通用部署角色模板或穷尽权限审计。

列级 UPDATE 是原有行锁命令所需的真实写权限；该角色不是只读角色，也不提供按用户的 SQL 隔离。
Session 摘要仍可读取，直接 SQL DELETE 权限也不限某台设备，因此只能由可信服务持有。
应用命令仍推导准确目标，核验原 Source／账号／Session、当前到期时间并确认提交。
登录、注册、维护及含注册表的私人业务有另外的权限需求；代码不为生产角色授予或修复权限。

## 不执行探测查询的关系锁

`RelationPin::capture(..., false)` 用一次 Simple Query batch 操作固定保留名
`cyanrex_session_source_relations_v1`：准备来自三张 `ONLY` 关系的常量投影，随即释放该语句，
从不执行或 EXPLAIN 它。PostgreSQL 的
[PREPARE](https://www.postgresql.org/docs/16/sql-prepare.html)将解析、分析、重写与后来的规划／执行分开。
测试独立观察释放语句后事务仍持有三张表的 AccessShareLock，并真实阻塞冲突 DDL。

原普通永久表、RLS、继承、分区、主键及解析 OID 检查保留。Source-only 路径另外将实际持锁
与当前 PID、当前数据库和每个解析后的关系 OID 比较。含注册表路径保留原显式表锁顺序。
PREPARE 不是 ACL 或授权证明，真实业务读取、行锁和删除仍必须拥有并实测所需权限。

即使 LIMIT 0 查询的规划也可能求值恶意视图或 RLS 的 immutable 表达式，因此这里不执行这种
探测。陷阱正控制验证这一风险；Source 拒绝不支持的结构而不求值它。原最终 Session 复核及
写后的原始源检查不变。

## 失败与取消

已有同名语句会导致失败。源不会执行、释放、收编或重试那个旧计划，不用 DEALLOCATE ALL，
也不换随机名称绕过冲突。正常成功 batch 不留下保留语句。取消或服务器 batch 中断仍可能在
一个物理连接上留下这一个固定槽；回滚不承诺已清除会话级准备状态，后续碰撞仍须失败关闭。

关系锁不会刷新此前的 REPEATABLE READ 目录快照。本改动不修复所有已有同 OID 约束漂移，
也不证明完整 schema 兼容。HTTP 失败仍私有且未确认，不清 Cookie，不授权盲目重试。

## 显式验证

明确指定一次性 `CYANREX_TEST_DATABASE_URL` 后运行：

```bash
./scripts/test-platform-session-permissions.sh
```

四个精确 SQL 入口覆盖受限生命周期、缺少必要权限、实际关系锁／DDL 等待、可执行陷阱及
保留计划碰撞。最终断言前清理合成角色、连接池和命名空间；CI 复用已有一次性 PostgreSQL
夹具，不能指向应用数据。还覆盖原表已重命名但替换视图尚未创建的中途清理。
不新增监听、UI／SDK 接线、权限迁移或已部署局域网验收。
