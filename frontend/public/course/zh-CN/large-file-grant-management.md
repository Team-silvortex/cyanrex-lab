# 当前 Session 的文件授权管理

源码版本 **0.5.7**（**2026-10-12**）已收录此准备层；以下分日期的 0.5.6／未发布实施与
验证记录保留原范围，源码收录不代表已挂载或验收部署。

状态：**未发布的内部准备层**，源码 **0.5.6**，**2026-10-11**。
此工作区跟进不在已发布 0.5.6 标签内；API 基线仍为 0.3.0。
它沿[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)，在
[发布权限账本](large-file-publisher-grants.md)之上增加明确的宿主配置 `ManageFileGrants`。
Active 记录仍不足以授权发布、分享或访问字节。

## 可信配置与当前操作者

`DurableAuthSource::file_grant_management_workspace(publisher_workspace, manager)` 惰性创建
`SessionFileGrantManagementWorkspace`，不执行 SQL、安装或修改策略。私有字段固定既有
命名空间、工作区、安装代次及保留授权额度，加上一个准确的 `FileAccountRef` 管理者，
包含 Principal 和账号代次。工作区及管理者权限域必须匹配认证源；无请求编解码、默认
管理员、角色查询或首用户提升。

可信宿主必须独立于不可信请求创建配置。若允许请求者选择 `manager` 后即时构造句柄，
就会破坏边界。这是**宿主配置的 ManageFileGrants**，不是新增持久 Registry 权限；现有
Registry 成员／部署策略不变，不升级 schema 或角色。

`issue_session_file_publisher_grant(token, management, grant_id, subject, validity)` 从当前
经审计的 active Human 成员 Session 推导签发者，要求与配置管理者完全相等。请求不能
提供 issuer、actor 或管理策略。被指定的无角色成员可以管理；未指定的教师、工作区 owner
或部署管理员不能。

签发前后均检查目标准确的当前认证源账号、经审计的 active Human 身份及同工作区 active
成员资格；相同用户名不能继承旧账号代次。有效期必须符合纯契约，但签发不要求区间包含
当前时间：未来或已过期授权仍是元数据，不是当前许可决策。

`revoke_session_file_publisher_grant(token, management, grant_id, expected_revision)` 同样
推导撤销者，刻意不要求旧 subject 仍活跃、仍是成员或仍存在于认证源；退休／删除账号的
授权也必须能够撤销。保留账本准确的 Active 修订 1 → Revoked 修订 2 CAS、初始签发者／
主体／有效期及不可变历史。重复或过时变更冲突；到期／撤销记录仍占用配额。

## 单一认证源事务

两种命令借用账本不可复制的 pending mutation 接缝，不调用独立存储接口。只有认证源
拥有事务设置、十秒命令期限和 COMMIT。

1. 认证源查询前设置 READ COMMITTED，保留宿主 READ ONLY，设置 synchronous_commit on。
2. 固定原认证源／Registry／目标身份，验证当前操作者及准确配置管理者。
3. 签发时通过原认证源固定检查目标账号，再返回原目标命名空间。
4. 按统一三表顺序持有账本写屏障，校验有界完整账本，写入唯一预期 head／历史变更，
   保留原 schema 及完整预期账本。
5. 刷新 `SET CONSTRAINTS ALL IMMEDIATE`，复核原 schema 和完整预期账本；签发再通过
   原认证源固定复核目标身份／成员资格，随后再次核对账本。
6. 恢复 synchronous_commit on，完成原认证源／Registry／当前操作者检查，最后用新鲜
   数据库时间检查 Session。只有认证源 COMMIT 确认后才返回记录。

保留两秒锁、五秒语句期限。借用接缝不获取池、开启／提交事务、修改设置或刷新延迟工作。
目标检查是通用 Principal／账号私人工作守卫，不把文件权限藏进共享平台策略。触发器改变
路径或对象时，原关系固定拒绝合法克隆而不是收编。触发器／延迟故障会回滚明确的提交前
变更；取消、超时或回执丢失通常不证明回滚，也不授权重试、修复或清理。

## 撤权及剩余边界

登出停止该 Session；同一配置账号重新登录仍可管理。暂停、退休或移除成员会阻止命令，
但恢复同代账号／成员后，固定配置仍可生效。只替换配置**不会撤销存活 clone 或在途命令**。
撤掉配置管理者需宿主停止分发旧句柄、排空在途工作并销毁全部旧句柄或重启。在线、持久、
可审计的管理权撤销需要未来独立权限账本和屏障，不能把复制的配置快照当作实时策略。

撤销发布授权不撤管理者配置或独立接收授权，不删除已发布文件，也不停止正在进行的字节
工作。授权管理不授予发布、下载、他人私人 Artifact 或部署权限。旧可信存储记录不会因
新增入口而变成数据库可证明的 Session 签发；存储归属不能独立证明调用了哪个 API、
历史宿主配置如何授权。

不增加实际发布权限消费、接收账本、字节 IO、HTTP／SDK／UI、在线挂载、Session 签发、
启动安装、迁移、重启恢复或自动重试。原本人 Session 元数据读取与可信独立账本仍是
各自独立的 API。

后续 [Session 发布](large-file-session-publication.md)独立消费当前发布授权，跨无 SQL 锁文件
工作复核原认证源／Registry／授权身份，并共享插入及最终认证。管理授权仍不自动授予发布权。
后续切片另留执行证据，下方管理记录不变。

## 验证

默认 `session_file_grant_management_tdd` 检查惰性配置和廉价准入；显式
`scripts/test-session-file-grant-management.sh` 核对编译库存，以 `--ignored --exact` 执行
一次性 PostgreSQL 用例。公共 `fileGrantManagement` 守卫检查源码组合及双语文档，不替代
运行时授权或部署验收。本切片执行证据单独记录，不改原账本分日期结果。

## 执行记录 2026 10 11

真正 Session 编译 Red：实际签发进入惰性借用写入接缝时 **0 通过／1 预期失败**，
**零 Rust 警告**；夹具先清理再断言失败。实现后显式 PostgreSQL 16 runner **10 项精确
用例全部通过**，零警告；每项先核对编译库存。完整后端门禁也通过新增 3 项默认配置／
准入测试。

控制验证无角色配置管理者、未指定教师／部署管理员拒绝、准确管理者 Principal／账号代次、
旧配置 clone 继续生效、自动签发／撤销归属、原签发者保留、CAS、当前目标／认证源／审计
成员检查。重建管理者需要新明确宿主配置；退休／删除主体仍可撤销。关闭独立授权安装池
后，命令仍只用认证源事务完成。

立即及延迟故障真正到达写入，拒绝后比较全部认证源／Registry／授权表行与 OID，覆盖
当前操作者／目标／Session 变化、抑制写入、其他账本篡改、SQL／路径故障及合法的原
认证源／Registry／目标关系替换。移除各合成故障后显式健康命令通过，证明夹具可恢复；
不是实现重试策略，也不是对任意触发器副作用的全局审计。

真实锁控制覆盖签发／撤销等待授权 marker 后到期、marker 安装代次新鲜漂移，以及暂态
写入后的延迟刷新取消。独立取消后观察仅证明受控提交前实验中的回滚。已进入的命令
阻挡真正并发 `source.logout` 至提交；顺序先登出再命令也拒绝后续管理。另一个持有 Session
行锁后删除的分支是明确 SQL 注入，不是另一种真正并发登出顺序。不声称任意 COMMIT
回执丢失实验。

释放真实屏障后并行驱动两个排队调用者；递归 backend 阻塞链证明两写者实际等待。连接
配置 RR／off-sync 时，新鲜命令读取仍只有一个额度／CAS 胜者。保留 READ ONLY，之后恢复
连接默认值，撤销仍占额度。延迟 sync 探针只观察刷新点为 `on`，不是断电持久性。
marker 修改持有者预先取得表写锁，避免夹具升级锁死锁；固定权限域的其他工作区明确
返回 `ScopeConflict`，不是 `ScopeNotFound`。

相邻干净 runner **12 项账本 + 7 项本人 Session 读取 + 10 项私人文件元数据**全部通过，
零警告；连同本切片共 **39 项精确数据库用例**。全部使用新建、无 TCP 监听的私有 Unix
socket 夹具，不访问现有应用数据库。

清除测试／环境数据库变量后的 `quality-gate.sh --backend-only --no-npm-install` 通过
**841 项默认 Rust 测试**（含 3 项 compile-fail doctest）、**257 项公共检查**和全部长度／
版本／课程／OpenAPI／API／SDK 生成／操作工具检查；**771 项 opt-in 仍忽略**，零 Rust
警告。不重跑或声称前端／SDK 运行、最新依赖审计、全 opt-in 库存、跨平台、真正 LAN
传输或部署验收。106 个旧项目状态证据块、16 份旧详细执行尾部及分日期能力张量保持不变。
版本仍 **0.5.6**、API **0.3.0**；没有提交、推送、在线安装或迁移。临时数据库已停止，
生成的数据／socket 已删除（约 92 MiB，可由测试重建）；执行日志保留于
`/tmp/cyanrex-file-management-xHPacXWT/`。
