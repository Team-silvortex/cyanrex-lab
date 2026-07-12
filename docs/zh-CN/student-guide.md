# 学生快速开始

## 1. 启动系统

在项目目录执行：

```bash
./start.sh start --mode auto
```

第一次启动需要下载镜像和依赖，时间会比后续启动长。启动完成后打开：

- 前端：`http://localhost:3000`
- Engine 健康检查：`http://localhost:8080/health`

登录信息由启动器随机生成并保存在 `docker/.env`。将其中的
`CYANREX_ADMIN_TOTP_SECRET` 作为 Base32 密钥导入认证器，然后使用当前六位动态码登录。
不要把该文件或密钥发送给别人。

## 2. 认识界面

- **Dashboard**：查看系统概况；
- **eBPF**：编写、检查、运行和卸载程序；
- **Environment Helper**：检查内核、BTF、bpftool 和挂载点；
- **Events**：查看内核事件与平台诊断；
- **Modules**：管理教学模块和 C 头文件；
- **Scripts**：eBPF 页面中的脚本保存区；
- **Account**：修改密码和管理账户。

## 3. 编辑器能力

编辑器不只是高亮文本：

- 停止输入约 700ms 后执行真实 clang 检查；
- 红色波浪线表示错误，黄色表示警告；
- 输入 `ctx->` 可获得真实结构体字段补全；
- `Ctrl/Cmd + 单击`可跳转到当前文件中的函数定义；
- Quick Fix 可以补充常见头文件和 GPL license；
- Outline 可以查看 hook、函数和 Map。

clang 状态含义：

- `checking`：正在检查；
- `passed`：语法检查通过；
- `issues`：存在编译错误；
- `unavailable`：后端不可用或当前账户无权检查。

## 4. 一次完整实验

1. 先进入环境助手并运行检查；
2. 在 eBPF 页面选择模板；
3. 阅读代码并预测输出；
4. 等待 clang 状态变成 `passed`；
5. 点击“编译并运行”；
6. 在结果区区分 compile、load、attach 三个阶段；
7. 在 Events 页面观察事件；
8. 回到 eBPF 页面卸载程序；
9. 确认已挂载程序列表为空。

## 5. 实验纪律

- 不要删除自己无法解释的 bpffs 文件；
- 不要把 Engine 端口暴露到公共网络；
- 不要无限提高采样率或事件保留量；
- 不要运行来源不明的 eBPF 程序；
- 每个实验结束都要卸载程序。

eBPF verifier 能阻止大量非法内存访问，但它不是允许随意运行未知内核代码的理由。
