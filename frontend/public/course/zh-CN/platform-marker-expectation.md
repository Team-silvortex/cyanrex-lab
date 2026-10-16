# 显式平台标记身份声明

**2026-10-16 收录于 0.6.2**，此跟进将五张存储标记表的原身份与完整
[目录声明](platform-catalog-expectation.md)共同核验，遵循
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)。显式路径不再仅凭相同版本／
作用域接纳同名替换；不启动宿主或签发方。

## 声明与检查

`PlatformStorageMarkerExpectation::new(catalog, marker_oids, marker_owner_oids)` 消耗完整
`PlatformCatalogExpectation`，不另收第二份配置。两个数组的固定顺序如下：

| 位置及 JSON 名称 | 配置命名空间中的表 |
|---|---|
| 0，`source` | Source：`collaboration_auth_source_schema` |
| 1，`identity` | Source：`collaboration_identity_schema` |
| 2，`access` | Source：`collaboration_access_schema` |
| 3，`task` | Task：`collaboration_task_schema` |
| 4，`artifacts` | Artifact：`collaboration_artifact_schema` |

十项均为非零 `u32`；五个 relation OID 同属 `pg_class`，必须互异。所有者可以共享，但不能等于
声明的运行角色 OID；不同 PostgreSQL 目录间的数值重合及 `u32::MAX` 合法。构造不做 IO。
不透明声明没有 Debug、Clone、序列化、身份 getter 或转换为观察值的 API。可信操作者须独立
选定原身份；自动信任首次连接采样值，不能建立历史来源。

`inspect_platform_storage_markers_expected(config, connection, expected)` 在 SQL 前拒绝完整配置
差异，并保留 SQLx／原始外层事务拒绝。原十秒检查预算内的一笔自有 REPEATABLE READ、
READ ONLY 事务依次：

1. 在首个目录 SELECT 前锁定五个固定限定表名。
2. 匹配真实物理目录，再取得全部五表形状、OID／所有者及持锁证据，不读标记行。
3. 在第一条标记行查询之前比较全部十项身份。
4. 读取固定版本及配置作用域，复验形状／目录，确认 COMMIT 后才返回真实
   `PlatformStorageMarkerObservation`。

原观察复验也共用此提前身份比较。普通 `inspect_platform_storage_markers` 仍在所给真实目录
观察下检查当前标记，不由此推导历史声明。

## 私有输入格式 3

保留[格式 2 输入](platform-catalog-expectation.md)全部字段，将 `format_version` 改为 **3**，
新增必需的 `marker_identity` 对象如下。数字是合成示例，不是部署默认值：

```json
{
  "source": {"oid": 301, "owner_oid": 102},
  "identity": {"oid": 302, "owner_oid": 102},
  "access": {"oid": 303, "owner_oid": 102},
  "task": {"oid": 304, "owner_oid": 102},
  "artifacts": {"oid": 305, "owner_oid": 102}
}
```

同时要求完整 `catalog_identity` 和 `marker_identity`。每层记录只能是完整命名对象；缺失／null、
重复／未知字段、位置数组、错误类型、零、溢出、重复 relation OID 及运行角色拥有的标记均拒绝。
全部字段计入原 **16 KiB** 输入上限，Schema 版本／作用域由原契约固定，不接额外字段自选。

| 格式 | 身份字段 | 诊断含义 |
|---|---|---|
| 1 | 两字段均禁止，含 null | 当前目录及标记 |
| 2 | 必须 `catalog_identity`；禁止 `marker_identity`，含 null | 声明目录、当前标记 |
| 3 | 两份声明均完整 | 声明目录及五标记身份 |

`check-config` 只校验语法／内部一致性，不联网或打开配置 Artifact 根。格式 3 [check-storage](platform-storage-check.md)
直接调用 `check_platform_database_storage_markers_expected(config, endpoint, password, expected)`，
一次新连接、声明检查及成功或失败后的关闭，共享十秒数据库预算，不回退弱检查。原进程／私有
文件／根门禁、通用输出及运行时销毁后的根／进程复验不变。全部观察丢弃，不导出基线。

## 限制与验证

可信断言不认证自身来源，无法区分 OID 重用或保留全部配置身份的物理克隆。格式 3 不固定
Artifact 根历史 inode 或全部业务表，不证明完整 schema 健康／当前 Session 权限，也不在密码
交付前认证对端。同步根检查仍没有硬期限。不增加池、重试、修复、迁移、监听、签发或正常应用
挂载；[保留会话组合](platform-session-connection.md)仍是独立路径。
超时／取消不证明远端回滚，也不允许自动重试。

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_marker_expectation_values
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
bash scripts/test-platform-marker-expectation.sh
```

只有最后一条需要显式一次性 `CYANREX_TEST_DATABASE_URL`、数据库／角色管理能力及运行角色密码
认证。脚本先核准确选择器存在；夹具身份从独立管理员查询取得，不来自被测观察器。故障仅影响
自有一次性存储，在最终断言前恢复／回收资源，不是局域网／TLS 部署或可用宿主验收。

2026-10-16 在隔离 PostgreSQL 中五条新增及 36 条相邻 SQL 回归全部通过。后端／通用门禁通过
1,196 条 Rust 测试和 426 项通用检查；918 条显式启用的 Rust 测试默认忽略，不计为通过。
临时实例已停止并删除一次性数据，本切片没有重跑前端／SDK 构建、已部署局域网／TLS 或内核验收。
