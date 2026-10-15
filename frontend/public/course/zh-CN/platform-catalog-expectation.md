# 显式平台目录身份声明

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后的未发布跟进，属于
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)。
可信操作者可以在首次连接前声明原始角色、数据库所有者及三个命名空间，避免把已经发生的
有效同名替换收编为连接基线。声明是操作者断言，不是数据库观察、授权或健康凭证。

## 类型声明与比较

`PlatformCatalogExpectation::new(config, role_oid, database_owner_oid, namespace_oids,
namespace_owner_oids)` 消耗完整可信宿主配置。两个命名空间数组均按 **Source、Task、Artifact**
排序。八个 OID 位置都必须非零，命名空间 OID 两两不同；允许共用所有者，但运行角色不能
拥有数据库或任何命名空间。独立 PostgreSQL 目录之间允许 OID 数值重叠，`u32::MAX` 有效。

声明没有 Debug、Clone、序列化或转成观察值的 API。构造不读数据库，不发现或补齐坐标。
可信调用方须独立建立这些值，不能从可能已经被替换的首次运行连接中取得并自动信任。

`inspect_platform_database_expected(config, connection, expected)` 在 SQL 前拒绝完整配置差异，
再执行已有的只读[目录检查](platform-host-preflight.md)，逐项比较角色、所有者及命名空间身份，
只返回真实且匹配的观察值。同名、等价权限或新观察到的有效对象都不能替代声明 OID。

`PlatformDatabaseConnector::with_catalog_expectation(config, expected, endpoint, password)`
仅做纯构造。每次显式 `connect()` 只尝试一次，在原十秒预算内核对不可变声明，通过后才交出
连接。身份不符返回固定 `Unavailable`；配置不符在拨号前返回 `InvalidConfig`。
旧观察值构造器保留原契约，[本地传输、密码及日志边界](platform-database-connection.md)不变。

## 私有 JSON 版本 2

保留[启动输入](platform-startup-input.md)全部字段及私有文件规则，把 `format_version` 改成 2，
并提供必需的 `catalog_identity`。以下是完整合成示例，不是批准的部署坐标：

```json
{
  "format_version": 2,
  "origin": "https://platform.example:8443",
  "authority_id": "00000000-0000-0000-0000-000000001069",
  "workspace_id": "00000000-0000-0000-0000-00000000106a",
  "namespaces": {
    "source": "source_private", "task": "task_private", "artifacts": "artifacts_private"
  },
  "database": {
    "database": "platform_db", "role": "platform_runtime",
    "system_identifier": 123, "database_oid": 456,
    "transport": {"kind": "loopback", "address": "127.0.0.1"},
    "port": 5432, "password": "replace-with-explicit-database-password"
  },
  "session_limits": {"per_account": 4, "total": 16},
  "artifact_root": "/private/platform/artifacts",
  "catalog_identity": {
    "role_oid": 101, "database_owner_oid": 102,
    "source": {"oid": 201, "owner_oid": 102},
    "task": {"oid": 202, "owner_oid": 102},
    "artifacts": {"oid": 203, "owner_oid": 102}
  }
}
```

版本 1 继续支持，但禁止此字段，即使值为 null。版本 2 拒绝缺失／null 声明、每层未知或
重复字段、位置数组、错误类型、零／负数／越界 OID 及不一致所有权。不会从数据库补齐
任何身份；包含声明的整个输入仍限制为 16 KiB。

`check-config` 仅校验声明格式及内部一致性，丢弃输入，不联网；不证明 OID 存在或可信。
[check-storage](platform-storage-check.md) 在版本 2 下调用
`check_platform_database_storage_expected`：先比较目录，再检查五个当前标记，检查成功或失败
都关闭连接，销毁运行时后复验原 Artifact 根及进程。版本 1 仍只诊断当前对象。
两种格式都不导出保留基线，也不启动宿主。

## 范围与验证

目录身份不固定过去的 marker relation ID 或本次调用之前的根身份，不证明完整 schema 兼容、
业务数据、当前 Session 权限或交出连接后的持续保护。无法区分 OID 重用或完整保留全部配置
身份的物理克隆。核验发生在数据库认证之后，不在密码交付之前；须独立信任本地对端。
不增加连接池、自动重试、安装、迁移、签发或监听；实际宿主组合和原标记／根绑定仍须单独推进。

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_catalog_expectation_value_tdd
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
bash scripts/test-platform-catalog-expectation.sh
```

SQL 脚本要求显式一次性 `CYANREX_TEST_DATABASE_URL`、角色／数据库管理能力及运行角色密码认证，
允许仅对管理员使用本地 trust。四个准确入口覆盖管理员独立提供 OID、十二项配置差异、八项
错误身份、首次连接前有效 namespace 替换、后续角色／所有者／权限漂移及真实 CLI 新旧格式对照。
测试服务器配置 SCRAM 时夹具使用 SCRAM，脚本本身不强制具体密码机制。
测试保留不变快照，在最终断言前回收自有资源；这些是本地回归，不是多机局域网／TLS 或
浏览器保存验收。
