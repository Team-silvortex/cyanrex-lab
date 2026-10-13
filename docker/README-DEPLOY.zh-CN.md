# Cyanrex Lab 离线部署

运行包参数保留在 `manifest.env`。`release-metadata.json` 记录可审计构建信息：包身份／时间、
Git 修订及干净／有修改状态、可用时匹配的附注版本 Tag、镜像引用／构建方式、Compose 模板来源
和流式计算的 `cyanrex-images.tar` SHA-256。每个镜像引用还绑定 Docker 内容 ID；安装验收忽略
继承的镜像覆盖，在加载后再次核对 ID。JSON 由 `checksums.sha256` 覆盖，但二者均不是数字签名。

## 包内容

- `docker-compose.yml` 和 `.env.example`
- `deploy.sh`、`run.sh`、`stop.sh`
- `runner-agent.sh`、`runner-agent-smoke.sh`、`live-kernel-smoke.sh` 和原生 `cyanrex-release` CLI
- 临时洁净宿主机安装验收脚本 `install-smoke.sh`
- 包含 PostgreSQL、Engine、前端镜像的 `cyanrex-images.tar`
- `manifest.env` 和机器可读 `release-metadata.json`
- `checksums.sha256`、`LICENSE` 和快速阅读文档

## 一次性发布验收

仅在一次性 Docker 宿主机上新解压的包内运行。它拒绝覆盖已有 `.env`，创建临时凭据和专用
PostgreSQL 卷，在禁止拉取镜像的条件下启动、验证登录和远程编译，最后移除自行生成的状态。

```bash
./install-smoke.sh
```

包内验收需要 Docker、Bash、curl、OpenSSL 和标准 shell／校验工具；使用捆绑 Rust CLI 处理
JSON，不要求宿主机 Python。创建运行配置前，`./cyanrex-release package verify .` 核对准确
解压文件集及元数据。加载镜像后，`./cyanrex-release package verify-loaded-images .` 核对受
校验和保护的元数据与 Docker 镜像 ID；第二条命令不重新检查每个包文件。

除启动特权 Engine 外，默认路径不修改宿主机内核。在一次性特权 Linux 验收宿主机上，可将真实
Aya 挂载／ring-buffer 事件／准确卸载周期设为安装 smoke 的必需部分：

```bash
CYANREX_SMOKE_RUN_LIVE_KERNEL=1 \
CYANREX_KERNEL_SMOKE_REPORT=/safe/output/live-kernel-acceptance.json \
./install-smoke.sh
```

真实检查拒绝非空管理员挂载集，并验证没有残留受跟踪挂载。指定证据路径时，拒绝覆盖已有文件，
记录候选元数据 hash、环境、唯一事件和清理结果。附注 Tag 候选启用它；常规无特权工具检查不启用。

包内验证器严格离线核对 schema 和候选绑定；须提供生成报告时使用的同一包元数据：

```bash
./cyanrex-release evidence verify /safe/output/live-kernel-acceptance.json \
  --release-metadata ./release-metadata.json
```

解压包含四个文件的 Tag 工作流产物前，可信的匹配源码 Tag 检出可运行
`cyanrex-release candidate verify /path/to/artifact --extract-to /path/to/new-output`。它拒绝不安全
归档成员、验证所有包校验和、将归档元数据绑定外部证据，再只写已验证普通文件且不覆盖已有
输出。无证据的两文件包可在可信源码检出中用 `cyanrex-release package extract`；
`scripts/release-package.py` 保留兼容入口。这些检查证明一致性，不证明发布者真实性；
签名仍是独立发布决策。

设置 `CYANREX_SMOKE_KEEP=1` 可保留失败 smoke 栈用于诊断。默认端口已占用时，选择另一
回环地址，例如 `CYANREX_SMOKE_BIND_ADDRESS=127.0.0.2`；非回环 smoke 绑定被拒绝。

## 正常部署

```bash
cp .env.example .env
# Replace every credential placeholder and review bind settings.
./run.sh
./deploy.sh status
./deploy.sh logs
./stop.sh
```

包默认只发布到回环。远程访问优先使用 SSH 隧道或 TLS 反向代理。确需局域网发布时，修改
`CYANREX_BIND_ADDRESS` 之前先配置宿主机防火墙和可信用户访问控制。

主栈健康后，可启用可选的无特权编译 Agent：

```bash
./runner-agent.sh start
./runner-agent-smoke.sh # prefers the adjacent native acceptance client
```

只有外部编排器负责就绪检查时，才设置 `CYANREX_DEPLOY_WAIT_FOR_HEALTH=0`。
