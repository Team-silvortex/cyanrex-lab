# 故障排查手册

## 先判断失败在哪一层

| 现象 | 优先检查 |
|---|---|
| 页面打不开 | Frontend 容器、端口、SSH 隧道 |
| 登录失败 | `docker/.env`、系统时间、TOTP |
| clang unavailable | Engine 健康状态、账户权限、网络 |
| compile 失败 | C 语法、头文件、类型、宏 |
| load 失败 | verifier、BTF、权限、内核能力 |
| attach 失败 | hook 名称、bpftool 版本、tracefs |
| 运行成功但无事件 | 触发条件、采样、事件过滤、读取通道 |
| 卸载不干净 | attachment 列表、pin path、Engine 日志 |

## 服务与日志

```bash
./start.sh status
./start.sh logs engine
./start.sh logs frontend
docker compose -f docker/docker-compose.yml ps
```

健康检查：

```bash
curl http://127.0.0.1:8080/health
```

服务默认只绑定回环地址。远程服务器上浏览器不能直接访问 `SERVER:3000`，应建立 SSH 隧道。

## 登录与 TOTP

确认：

- 使用的是当前 `docker/.env`，不是旧截图或 README 示例；
- 手机和服务器时间正确；
- 没有连续输错 5 次；连续失败会锁定账户 5 分钟；
- `CYANREX_ROTATE_ADMIN_CREDENTIALS` 已恢复为 `false`。

不要在聊天、Issue 或课堂投屏中展示 `.env`。

## clang 实时检查

状态一直为 `unavailable` 时：

1. 确认已经用管理员账户登录；
2. 检查 Engine 健康状态；
3. 确认源码没有超过 256 KiB；
4. 查看 Engine 日志中 clang 是否存在；
5. 等待其他编译任务完成，系统最多并发处理两个任务。

语义补全会调用后端 clang。网络短暂失败时，本地 snippets 仍然可用。

## BTF 与 vmlinux.h

环境助手中 `kernel_btf` 或 `btf_dump` 失败时检查：

```bash
ls -l /sys/kernel/btf/vmlinux
bpftool btf dump file /sys/kernel/btf/vmlinux format c >/dev/null
```

Docker 模式需要把宿主/虚拟机内核的 BTF 暴露给 Engine。没有 BTF 时，依赖 `vmlinux.h` 的
CO-RE 示例无法工作，但只使用稳定 UAPI 头文件的简单程序仍可能运行。

## bpffs 与权限

```bash
mount | grep /sys/fs/bpf
ls -ld /sys/fs/bpf
ulimit -l
```

不要为了绕过错误手工 `chmod 777 /sys/fs/bpf`。应修复启动方式、挂载和容器能力配置。

## 自动挂载不可用

旧版 bpftool 可能没有 `autoattach`。Cyanrex 会尝试手动 tracepoint attach。
如果程序类型需要明确的网络接口、cgroup 或其他 target，教学系统可能只能完成 load，
不能自动选择正确挂载目标。此时应根据实验说明提供 target，而不是随机挂载。

## 有程序但没有事件

依次确认：

1. attachment 列表中确实有程序；
2. 执行了能触发 hook 的操作；
3. 运行时间尚未结束；
4. 采样率没有过低；
5. Events 页面过滤条件正确；
6. Ring Buffer 结构与读取端预期一致；
7. 没有因为 Ring Buffer 满而持续 reserve 失败。

## 最后的清理

先在页面执行“全部卸载”。如果 Engine 异常退出，重启 Engine 后检查 attachment 和 bpffs。
不要使用通配符删除整个 `/sys/fs/bpf`，因为那里可能还有其他软件加载的程序和 Map。
