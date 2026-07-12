# 教师快速开始

## 1. 课程定位

建议把 Cyanrex 用于 4～8 学时的 eBPF 入门实践。系统适合讲授：

- eBPF 程序生命周期；
- hook、helper、Map、Ring Buffer；
- clang 编译错误与内核 verifier；
- 最小权限和内核可观测性边界。

它不是生产级多租户沙箱。不要让多个不受信任学生共用同一个特权 Engine。

## 2. 推荐课堂拓扑

最安全的方式是每位学生运行自己的实例：

```text
学生浏览器 -> 本机 Cyanrex Frontend -> 本机/WSL/Docker Engine -> 个人 Linux 内核
```

如果必须使用集中服务器，应为每位学生准备独立虚拟机，而不是只创建多个 Cyanrex 用户。
Engine 容器拥有访问内核 eBPF 子系统所需的高权限，应用账户隔离不能替代虚拟机隔离。

## 3. 课前准备

在每台实验机执行：

```bash
./start.sh start --mode auto --rebuild
./start.sh status
```

通过 SSH 使用服务器时建立本地隧道：

```bash
ssh -L 3000:127.0.0.1:3000 \
    -L 8080:127.0.0.1:8080 \
    USER@SERVER
```

打开 `http://localhost:3000`，登录凭据保存在 `docker/.env`。将
`CYANREX_ADMIN_TOTP_SECRET` 作为 Base32 密钥导入教师认证器；不要投屏或提交该文件。

## 4. 环境验收

进入“环境助手”，确认：

- Backend 显示预期的 `docker`、`wsl2` 或 `native-linux`；
- `clang`、`bpftool`、`kernel_btf`、`btf_dump` 为正常；
- `/sys/fs/bpf` 已挂载；
- `memlock` 满足要求；
- 总体状态为 Ready。

部分旧版 bpftool 不支持 `autoattach`，系统会使用手动 tracepoint attach 回退。这不是阻塞项。

## 5. 建议课时

| 课时 | 内容 | 实验 |
|---|---|---|
| 1 | eBPF 架构与安全模型 | 实验 1 |
| 2 | Tracepoint 与事件观察 | 实验 2 |
| 3 | Map 与状态 | 实验 3 |
| 4 | Ring Buffer 与用户态消费 | 实验 4 |
| 5 | Verifier 思维方式 | 实验 5 |

每个实验建议采用“预测—运行—解释—修改”的节奏。先让学生预测现象，再点击运行。

## 6. 教学检查点

不要只检查程序是否显示 success。要求学生说明：

1. 程序挂在哪个 hook；
2. context 的类型是什么；
3. 哪些数据保存在 Map，哪些通过 Ring Buffer 发送；
4. verifier 如何知道内存访问安全；
5. 实验结束后如何确认程序已经卸载。

## 7. 课后清理

在 eBPF 页面点击“全部卸载”，确认已挂载程序列表为空，然后执行：

```bash
./start.sh stop
```

如需清除课程数据，可以在停机后删除对应 Docker volume。删除 volume 会永久清除账户、
脚本和事件，执行前必须确认不再需要这些数据。
