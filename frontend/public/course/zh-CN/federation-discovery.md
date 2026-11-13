# 联合节点与发现

[English](../en/federation-discovery.md)

方向确认日期 **2026-11-13**，见[提案 #4](https://github.com/Team-silvortex/cyanrex-lab/issues/4)。
Cyanrex 将向去中心化的合作与教学平台演进：个人和组织运营自己的节点与工作区，
代码仍只是可选 Task payload。教师权威留在教学域内，不成为其他节点及其私人资源的全局所有者。

**0.6.6 中的 F0 和 F1a 都是纯 Rust 准备模块**，不是运行中的联合服务。
F1a 新增紧凑 TXT 数据处理和有界本地候选缓存；产品版本为 0.6.7。
独立 F1b 适配器新增显式调用的 Linux IPv4 单次查询，未挂载到两个应用。
设备配对、跨网穿透、中继传输及跨权威工作区访问属于后续切片。
源码 0.6.7 收录 F1c 独立显式 `cyanrex-discovery` 命令，不接应用启动。
F1d 新增纯公告／goodbye 线格式准备，不是运行中的公告服务。
F1e 新增纯 probe 字节及保守逐包声明观察，不是名称可用性判断。
F1f 新增独立显式的准确本机地址单播接收器，不是运行中的 probe 控制器。
F1g 新增显式有界三次探测库调用，不是公告服务或名称归属许可。
F1h 新增固定生命周期／接口及冲突冷却的有限显式重试，不是发布入口。
F1i 新增原 socket 静默续接及有界只接收复核，不是公告。
下文旧 `Unreleased`／未发布标签保留实施阶段和分日期证据，不表示仍待源码收录。

## 信任与职责

目标连接流程保留四个独立决定：

```text
不可信发现提示 → 协议／能力筛选 → 节点密钥持有证明 + 明确配对
                             → 另行限定范围的 WorkspaceGrant
```

发现可以返回兼容候选，但不使候选可信。消息中的指纹是声明，不证明发送者持有对应密钥。
后续配对必须证明身份并记录批准与撤销；另行审查的 `WorkspaceGrant` 仍需授权具体资源操作，
连通或已配对均不自动产生成员身份。

`AuthorityId` 标识现有资源命名空间，不是证书、网络端点或节点密钥。
F0 接受外部权威声明，绝不绕过现有 Source 的同权威校验。课堂邀请、Cookie Session、
私人 Task 与 Runner 引导凭据保留原含义；Runner 注册不是对等节点配对，课堂链接不是 mDNS。

## F0 契约

实现位于 `engine/src/services/federation_discovery/`。调用者提供字节、本地策略和明确当前时间。
模块不读取时钟，不访问网络、数据库或文件系统，不生成密钥。

| 值 | 含义 |
|---|---|
| `NodeFingerprint` | 规范的 64 位小写十六进制节点密钥摘要声明，与 `AuthorityId` 独立 |
| `PeerAdvertisement` | 经 `parse` 解码、`encode` 编码的严格有界 schema-1 元数据 |
| `ProtocolRange` | 本地或公告的正整数闭区间协议版本 |
| `DiscoveryPolicy` | 本地指纹、版本、支持／必需能力及明确启用的提示类别 |
| `UnverifiedPeerCandidate` | 选定协议、共同能力及允许提示；没有权限或派发方法 |
| `ConnectionHint` | 规范 IP socket 字面量或声明的中继名称；仅筛选后候选通过本地策略 |

线格式字段为 `schema_version`、`node_fingerprint`、`authority_id`、`protocol: {min, max}`、
`offered_capabilities`、`required_capabilities`、`issued_at`、`expires_at` 和 `hints`。
提示为 `{kind: "direct", address: "192.168.1.7:4433"}` 或
`{kind: "relay", relay_id: "lab.home"}`。筛选只保留操作者批准的中继标识；
仅解析不批准任何中继，也不导入中继 URL。
公告不携带工作区名称、成员名单、内容、凭据或邀请秘密；每层都拒绝未知及重复字段。

| 上限 | F0 规则 |
|---|---|
| 公告编码 | 含空白至多 4,096 字节 |
| 连接提示 | 1–8 个不同提示；拒绝零端口、DNS、URL、scope／zone 及映射 IPv6 |
| 能力 | 每列表至多 16 个规范且不重复的限定名称，必需能力必须已提供 |
| 有效期 | 正整数、JSON 安全范围内的 Unix 秒，时长 1–300 秒 |
| 新鲜度 | 过期时间严格晚于调用者当前时间；签发时间最多超前 5 秒 |
| 版本协商 | 选择交集最高版本，不低于本地最低版本 |
| 能力协商 | 双方必需能力均须受支持；输出仅包含共同能力 |

解析与筛选使用不含输入的固定错误，拒绝非法值、自身公告及不兼容节点。
筛选时重新检查时间；短期未签名有效期不等于防重放、已认证在线状态或持久注册租约。

F2 必须确定规范公钥编码及指纹派生方式，才能将声明提升为已验证身份；
F0 不生成密钥，也不将摘要与实际公钥比对。

`DiscoveryPolicy::new` 默认仅允许私网直连提示；`with_public_direct` 明确允许公网地址候选，
`with_relays` 明确允许有界的操作者指定名称。筛选按私网直连、公网候选、已批准中继的顺序输出，
各组内部保留原顺序；没有允许提示时失败，不暗中更换策略。

静态地址过滤将 RFC1918 IPv4 和 IPv6 ULA 归为私网提示，拒绝回环、未指定、组播、
链路本地／元数据、文档及其他排除的特殊用途地址。可选公网类别只是保守的字面地址候选，
不证明地址已分配、同局域网、可达，也不是完整 SSRF 防线。
后续传输必须在使用提示前绑定获批接口／出站策略与已证明的节点密钥。

## F1a 局域网数据接入

`federation_discovery::lan` 组合原 F0 解析／筛选，不增加 socket 或发现依赖。
`encode_txt` 和 `decode_txt` 处理**一条原始 DNS-SD TXT RDATA**，不解析 DNS 包、
PTR／SRV／地址，也不跨记录拼接。`MAX_TXT_RECORD_BYTES` 为 **1,200**；
封装使用 `v=1`、`c=1..6`、`d0` 至 `d5`，每片至多 **200** 数据字节，非末片恰为 200 字节。
字段顺序可变，键按 ASCII 不区分大小写；重复／未知／缺失字段失败。
长度前缀计算全部字段，截断记录和空片失败；重组 JSON 仍经完整 F0 解析。
合法 F0 公告若放不进单条记录就失败，不丢字段或跨记录分片；这些是应用配置限制，不改 F0 上限。

`LanCandidateCache` 在构造时固定本地策略、**1–8** 个已批准接口代次和 **1–64** 条容量。
`LanSource` 由本地 `interface_epoch` 与 **1–63** 位小写 ASCII 字母／数字／连字符别名组成，
首尾须为字母或数字。epoch 是受信适配器提供的接口 incarnation，不是公告 ID、稳定 OS ifindex，
也不验证真实接口。别名仅标识来源，不是身份；完整 64 位十六进制指纹放不进单个 DNS label。

| 缓存事件 | 行为 |
|---|---|
| 不同来源声明同一身份 | 分别保存，不按指纹或 authority 合并 |
| 同一活跃来源的准确规范重复 | 返回 Duplicate，不延长任何期限 |
| 活跃来源公告内容变化 | SourceConflict，保留原条目 |
| 缓存已满 | Capacity，不暗中驱逐活跃条目 |
| 移除 | 仅删除准确本地 epoch／别名，不是节点或 grant 撤销 |
| 过期 | 初次记录 TTL、F0 到期或对应 monotonic 截止任一先到即删除 |
| 时钟回退 | 清空并永久关闭，返回 ClockRollback，后续操作返回 Closed |
| 停止 | 清空关闭，不重启或恢复该缓存 |

调用者提供 `CacheTime`（墙钟和单调时钟整数秒），模块不读取真实时钟。
每个合法时间样本均推进水位，即使后续准入失败；溢出不能撤销已观察时间，也不能退回旧样本重置事件额度。
记录 TTL 必须为 **1–120 秒**；后续适配器必须取相关记录 TTL 的最小值，
将 TTL 为零的 goodbye 映射为准确来源移除。观察／移除共用每 monotonic 秒 **64 事件**额度，
包括非法公告及不存在来源的删除；快照读取不占额度。
这个固定窗口准入限制不是网络入口、CPU 或带宽防护。
快照重新核对 F0 兼容和两个期限，仅返回带来源／截止信息的 `UnverifiedPeerCandidate`；
快照不保证持续有效，也不允许自动连接。

实际后端必须在创建 socket／加入组播**之前**限制接口，
将记录和移除绑定到已核验的本地接收来源，限制包解析／队列，并等待自有 worker 停止。
接口变化或事件流故障必须关闭缓存。本轮核查但未引入 `mdns-sd`：当前构造器先启动再处理接口筛选，
启动后 disable／enable 不是原子的启动前允许名单，见[官方源码](https://docs.rs/mdns-sd/0.21.7/src/mdns_sd/service_daemon.rs.html)。
F1a 不构造该 daemon，也不声明真实局域网发现已验收。

## F1b 显式 Linux IPv4 浏览

`engine/src/services/federation_mdns/` 与纯 F0／F1a 模块分离。
`discover_once(config, policy, cancelled)` 为实验性 `_cyanrex._udp.local.` 配置发送一次固定 PTR 查询，
自持一个 socket，接收窗口为 **1 ms–5 秒**。等待可能多出一次有界接收间隔，不保证实时执行。
它不创建线程、不注册本机服务、不连接发现的业务端口；再次调用是另一次明确操作，不是调度或自动重试。
F1c 提供独立 CLI，不接 UI 或宿主启动。

`MdnsConfig` 要求准确接口名和 RFC1918 本地 IPv4 地址。创建 socket 前检查网络命名空间身份、
接口 index、地址、连续 `/1..30` 掩码、UP／MULTICAST 标志及非回环的有效主机地址。
变更时保守失败，不选择默认接口。这些是本机元数据快照，不证明物理局域网，也不能原子抵御管理员变更；
VPN 接口也可能满足这些检查。

socket 绑定 **224.0.0.251:5353**，不绑定 wildcard 或业务入口。在仅加入选定接口前，设置
`IP_MULTICAST_ALL=0`、`IP_PKTINFO`、`IP_RECVTTL`、准确组播出口和 255 跳限。
每次发送再次固定同一接口／源地址，`send_query` 拒绝任意其他载荷。
接收必须匹配选定 index／本地地址、目的组、5353 源端口、私网同子网非本机有效主机及 TTL 255；
畸形、缺失、重复或截断的控制元信息均失败。适配器不改变 capability 或网关配置。

该严格配置可能漏掉同链路不同子网的合法设备；只处理**同一包内**每实例完整 PTR → SRV／TXT →
IPv4 A 链、标准向后名称压缩及有界未知记录，不提供通用跨包 DNS 缓存、IPv6、SRV／A 后续查询、
持续浏览／known-answer suppression，也不宣称与任意 mDNS 实现互通已验收。
包至多 **4,096 字节**、**64 记录／8 事件**；仅当选定 F0 直连提示同时匹配报文发送者、A 和 SRV
端口时才进入缓存。同别名 goodbye 还必须来自此前准入的同一发送者。

解析前，接收循环限制每 monotonic 秒 **64 报文**、每轮 **256 报文**，非法报文同样占用额度；
缓存另有事件限制。预算耗尽、接口变化、socket／时钟故障或取消均返回错误，不返回部分成功列表。
预先设置取消标志会阻止接口／socket 操作；运行中在每次至多 50 ms 接收及有界解析前后检查。
任何返回都会释放自持描述符并清空缓存；已返回快照仍只是未验证、有时限的元数据，不是在线权限。

真实收发测试须显式使用 `scripts/test-federation-mdns-netns.sh`，不使用宿主局域网。
runner 锁定／离线构建，在匿名 user／network namespace 中创建一次性虚拟链路，无宿主连接或默认路由。
普通 UDP 用例在零有效 capability 下运行；仅独立地址移除夹具有权修改自己的临时 namespace。
不支持 namespace 时属于前置失败，不能回退宿主网络。
这只验证 Linux 适配夹具，不等于物理局域网、Android、配对或去中心化部署验收。

## F1c 显式发现命令

使用 `cargo build --manifest-path engine/Cargo.toml --locked --bin cyanrex-discovery`
构建 0.6.7 收录的 Linux 源码工具，再查看 `engine/target/debug/cyanrex-discovery --help`。
不安装到特权 Engine 镜像、不随两个应用启动；F1c 不改变 F0／F1a 与 F1b 的既有行为。

所有策略值均按固定顺序提供。下例为合成示例，不能原样照跑：须换成准确非 root 数字身份／组、
已审阅的接口／地址和本地兼容策略。执行发送一次组播 PTR 查询，不公告本机服务；不要使用 sudo。

```bash
engine/target/debug/cyanrex-discovery browse-lan \
  --uid 1000 --gid 1000 --groups 27,1000 \
  --interface enp3s0 --address 192.168.1.10 \
  --local-node aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --protocol 1:1 --timeout-ms 1000 --capabilities none --require none
```

本地指纹是给定的自我排除声明，不生成或证明密钥。协议为正整数闭区间的发现版本，不是产品版本。
能力 CSV 各至多 16 个不同限定名，`none` 为空列表，必需能力还须属于支持集合；这些输入不实现
被协商的业务能力。输入准确为 21 项参数，每项至多 2,048 个 UTF-8 字节，总计至多 16,384 字节。
拒绝别名、换序、重复、未知开关和环境默认值，不提供公网／中继、配置文件、持续监听或加入选项。

任何接口／socket 操作前，`prepare_platform_process` 要求准确非 root 身份／组、零当前 capability
和仅此进程的 no-new-privileges 限制。不代为降权、不读取 NSS／认证配置、不访问数据库；
发现完成及输出前重新检查调用线程。不创建 Session、成员身份或资源权限。

成功输出完整内存组装 JSON 加换行，至多 **256 KiB／64 个候选**。顶层包含 `schema_version: 1`、
`kind: "unverified_lan_snapshot"`、`snapshot_only: true`、`verified: false` 和 `candidates`。
每项包含实例、独立节点／权威声明、协商协议、共同能力和惰性 IPv4 提示。
`advertisement_expires_at` 仅是 F0 声明时间，不是较短的 DNS TTL、在线证明、可复用租约或连接许可。
不导出内部 epoch／monotonic 截止、原始报文／TXT／JSON 或传输错误。空列表成功只表示该窗口
没有兼容候选，不证明网络健康或没有设备。

退出码为 **0** 成功、**64** 用法错误、**77** 进程不安全、**74** 发现／输出失败、**69** 平台不支持。
失败不冒充空列表成功。使用方必须要求完整文档及成功退出：信号／写失败可能留下截断 stdout。
**1–5,000 ms** 仅限制网络窗口，不限制准备、解析或 stdout 背压。单进程 CLI 不安装 worker、
子进程或信号处理器，保留继承的信号配置；普通 SIGINT／SIGTERM 默认行为会终止进程、关闭描述符，
见[信号说明](https://man7.org/linux/man-pages/man7/signal.7.html)。

默认测试不在宿主局域网发现。真实命令测试显式使用 `scripts/test-federation-mdns-netns.sh --with-cli`，
并要求已有 `getsubids`／`newuidmap`／`newgidmap` 授权。只在全新 user namespace 映射一个已有授权的
subordinate UID／GID，在那里清空子进程补充组，再以嵌套非零身份、零 capability 在临时网络运行 CLI。
缺少授权即失败，不改账户、不回退宿主网络，见[unshare 说明](https://man7.org/linux/man-pages/man1/unshare.1.html)。

## F1d 纯公告准备

`federation_mdns::MdnsAnnouncement` 冻结给定 F0 声明、实例标签、主机标签、IPv4 端点及请求 TTL。
构造及 `encode_at(now)` 不执行 I/O、不读取时钟。两个标签分别要求规范小写 ASCII 字母／数字／
连字符，1–63 字节且首尾字母数字；拒绝后缀注入、自动归一化及将 64 字符指纹塞进单标签。
端点必须是声明中**唯一**提示：准确相同的非零端口 RFC1918 IPv4 socket。混合、额外、公网、
中继及 IPv6 提示均整体失败，不静默过滤。这只核对声明一致，不证明接口归属、名称唯一或服务可用。

本地公告比 F0 接收更严格：给定时间须为正 JSON 安全 Unix 秒，满足 `issued_at <= now < expires_at`。
请求 TTL 为 **1–120** 秒，四记录 TTL 均为 `min(requested_ttl, expires_at - now)`。
每次 `encode_at` 重新检查到期，并拒绝早于构造时刻的采样；不追踪连续采样，后续发送者仍须补
monotonic／回拨控制，并在每次发送前刷新接口／时间检查。编码或保留报文不刷新 F0 时间，也不是发送租约。

无压缩报文固定 ID 零、flags `0x8400`、零 question、**四条 Answer**：PTR 指向
`<instance>._cyanrex._udp.local.`，该实例的 SRV／TXT，以及 `<host>.local.` 的一条 A。
PTR 为共享 IN，SRV／TXT／A 为 cache-flush IN；这些只是线格式字段，不代表已完成唯一性探测。
SRV priority／weight 为零。完整原 F0 仍经 **1,200 字节** TXT 封装，包的每次追加都在复制前检查
**4,096 字节**硬上限；超限失败不截断。包上限**不等于**可发送 UDP payload／接口 MTU 保证。

`encode_goodbye()` 保留同一冻结记录集的 owner／type／class／RDATA 及顺序，仅将全部 TTL 设零，
声明过期后仍可生成；不证明曾公告或授权撤回他人节点。现浏览器将其映射为立即移除准确来源候选，
没有实现通用 mDNS 的一秒 goodbye 宽限。

不增加 announce 命令、探测／冲突／改名状态机、查询响应抑制、调度、发送器、服务注册或应用启动。
`MdnsSocket::send_query` 仍拒绝固定浏览查询以外的一切载荷。真实发布须另行完成名称探测、冲突
处理及生命周期后，才可发送这些唯一记录声明，见 [RFC 6762 §§8–10](https://www.rfc-editor.org/rfc/rfc6762.html#section-8)。
默认纯测试独立检查字段，并将字节接入既有解析／缓存；仅证明内部配置兼容，不是 Avahi／Bonjour、
物理局域网或去中心化部署验收。

## F1e 探测准备与保守观察

`MdnsAnnouncement::encode_probe_at(now)` 复用 F1d 冻结名称、SRV／TXT／A 数据、时间检查与包预算。
报文 ID／flags 为零，含**两条 ANY／QU question**、零 Answer／Additional 和**三条 Authority**：
实例的 SRV／TXT 与主机的 A。Authority 为 IN、不带 cache-flush；不将共享 PTR 提议为唯一记录。
这只是准备好的提案，不代表已发 probe。

`observe_probe_packet(bytes)` 使用私有共用 DNS framing 解码，不依赖 `parse_response` 或候选兼容。
完整消费报文后只返回固定 `ProbeObservation`：

| 观察 | 准确保守含义 |
|---|---|
| `ExistingResponse` | ID 零的响应在任意 RR section 含任一受保护名称的 IN 记录 |
| `CompetingProbe` | query 的相关 IN Authority 记录匹配同名 IN question，其类型为 ANY 或该记录类型 |
| `NoRelevantClaim` | 这一已解码报文不含上述相关声明，**不是**名称可用／归属证明 |

即使 RDATA 完全相同、类型未知或响应 TTL 为零，也返回 `ExistingResponse`，只保守阻止“不存在”
假设；不做 RDATA 语义验证、身份认证、活跃所有者声明或 DNS 字典序胜负判断。
相关 query Authority 若无匹配 question 则失败；query Answer 的 known-answer 缓存及 Additional
不能成为同时探测的权威数据，单独 question 也不是名称归属声明。

共用解码保留 RR section 及 QU／cache-flush 原 class 位，维持既有向后压缩、ASCII 大小写标签及
**4,096 字节／8 question／64 RR** 上限。观察器仅检查 opaque RDATA 的 framing，不解释其语义。
截断、尾随、未支持 header 或超限均失败，不冒充 `NoRelevantClaim`。原候选 parser 保持响应专用
header 准入与完整链规则；缺少 PTR 链不能使声明逃过新观察器。

不提供三次探测计时、初始随机延迟、重试／退避、改名或胜者／许可 API。后续真实适配器仍须处理
接收来源、所有相关响应、时钟／到期与停止；发布前还须完成 RFC 的探测时序及冲突规则，见
[RFC 6762 §8](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1)。特别是当前浏览 socket 只绑定
组播目的地址并拒绝单播，收不全 QU 防御回复，**不能**从其超时推出探测成功。
下文 F1f 独立覆盖受限单播接收／端口共存，F1g 另增有界尝试，不代表独立互通。
F1e 不给 CLI 或 socket 增加发送入口。

## F1f 显式只接收单播通道

Linux `MdnsUnicastReceiver::open(&VerifiedInterface)` 独立打开只接收描述符，绑定指定
**RFC1918 本机 IPv4 地址:5353**，不替换或扩大原组播 socket。两个端口 reuse 选项均不设置，
不加入组播，也不暴露 send、connect 或原始描述符 API。已有相同地址或通配地址监听时，即使
对方开启 reuse，也以 `SocketUnavailable` 失败；不改绑通配地址、不抢占、不重配 daemon，
不回退组播后宣称成功。不同准确本机地址及原准确组播地址 socket 可以共存；这不是与所有系统
mDNS 服务共存的承诺，见 [RFC 6762 §15.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-15.1)。

两通道复用有界 `recvmsg` framing 和唯一 ancillary 校验，**不**合并允许的目的地址：浏览器
仍要求 224.0.0.251，新接收器仅要求准确本机地址。两者还要求指定 ingress index、本机
`ipi_spec_dst`、TTL 255，以及源端口 5353 的非自身 RFC1918 同子网有效主机。空包、超大／截断、
缺失／重复控制信息及接口／netns／地址观测变化均失败。打开和接收前后复核接口快照，不代表
对管理员变更的原子防护。Drop 释放自有描述符，不残留 worker。

`receive()` 使用既有至多 50 ms 的 socket 超时间隔，仍受调度影响；这不是整流程 deadline、
包速率额度或准入控制。返回字节未关联已发 QU question，不证明身份、名称可用或归属。
该类型没有接入 `discover_once`、CLI 或两个应用启动；F1g 另行显式组合它。
持续冲突／发布生命周期及独立互通仍需后续完成。

显式 `scripts/test-federation-mdns-netns.sh --with-unicast` 在原两个无外网连接的自有命名空间内
新增十条默认忽略用例；可组合 `--with-cli --with-unicast` 保留全部原发现／CLI 回归。
覆盖两个打开顺序、准确本机接口、端口占用／reuse／双栈通配拒绝、畸形包、描述符／端口释放、
零 UDP 发送／TCP 打开及地址删除。合成公告／probe 字节仅来自自有测试对端；产品不发 probe，
不接触真实局域网或系统服务。

## F1g 显式单次探测

`probe_once(config, announcement, cancelled)` 是独立 Linux 库入口，不是新产品 CLI 开关或启动服务。
冻结公告必须匹配指定本机 IPv4；整轮 timeout 为 **1–2 秒**，建议两秒。准备和调度均计入期限，
一秒配置即使网络静默也可能失败。原准确组播及独立排他本机地址 socket 必须都打开后才可发包；
响应端口占用时零发送失败，不降级或重配系统 daemon。每条返回路径均释放两个描述符。

非阻塞内核随机源经有界拒绝采样选择离散均匀 0–250 ms 初始等待，不回退弱随机。
三次提案以实际成功发送完成为基准，间隔至少 250 ms，第三次后另听满 250 ms；完整时间精度
防止毫秒取整缩短首尾间隔。调度迟到不会追赶连发。这遵循探测时序目标，不保证实时调度，见
[RFC 6762 §8.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1)。

私有双 FD poll 轮换优先通道，每批每路最多消费一包，解析前共用每 monotonic 秒 **64 包／
每轮 256 包**额度，拒绝包也计数。等待受下一时点、剩余总期限／初始租约及 50 ms 上限共同约束。
发送及最终边界在同一预算内排空两路；poll 中断／故障是错误，不冒充空队列。取消、接口／netns
快照、完整墙钟水位及冻结的 monotonic F0 到期在工作前后复核；这些是相邻观测，不是对系统调用
间管理员变更或取消的原子保证。

首次发送前消费的包不当成本轮回复；之后严格 F1e 观察器遇相关响应或组播竞争提案即终止。
单播只能是匹配本轮受保护 ANY／QU 名称的响应；整轮不超过两秒。发送后被拒的 DNS／来源包
直接终止，不转成“不存在”。未使用内核到达时间戳，跨过首发边界的排队包可能保守误报冲突。
这是受限策略，不是完整 RFC header 接收、字典序胜负或任意 mDNS 互通。

只有私有 typed 发送器能编码当前提案，发送前检查 payload 加 28 字节不超过指定接口当前 IPv4
MTU，并启用内核禁止分片策略；超大多记录包不发送、不拆分，见
[RFC 6762 §17](https://www.rfc-editor.org/rfc/rfc6762.html#section-17)。原公开浏览入口仍仅发送固定
PTR 查询，公开单播类型保持只接收。

`ProbeRunObservation` 仅有 `NoConflictObserved`、`ExistingResponse`、`CompetingProbe`。
完整静默也不证明可用、身份、持续唯一或发布权限。不增加自动重试、改名、退避、公告／goodbye、
持续防御、密钥证明或 grant。下文独立 F1h 控制器增加有界冲突冷却，不增加发布生命周期；独立调用
不提供进程级限速。隔离 runner 的 `--with-probe` 仅选择测试，不是产品命令或物理局域网验收。

## F1h 有限显式重试控制器

`MdnsProbeController::new(config, announcement)` 是纯准备：检查冻结来源和每轮 1–2 秒期限，
不读时钟、随机源或接口，不开 socket。调用者显式调用 `attempt(cancelled)`，不增加线程、
自动重试、睡眠或 CLI／启动接入。**最多准入四次尝试**；准入后接口／socket 失败也计次。
预取消、首次过期和提前冷却拒绝不计次。

只有 `ExistingResponse`／`CompetingProbe` 允许下一次尝试，并从本次**完成后至少等五秒**，
不是从开始计时。`CoolingDown` 不改变次数／冷却期限；校验冻结有效期后，在读取接口／随机源／
创建 socket 之前返回。固定等待采用 [RFC 6762 §8.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1)
允许的每次失败均等待的保守简化，不据此推导字典序胜者。`NoConflictObserved` 永久完成；
其他错误（包括取消）永久关闭，替换取消标志不能复活。第四次冲突关闭，不允许第五次。

首次实际使用固定 **30 秒** monotonic 总期限、F0 绝对到期、完整精度 monotonic 租约及墙钟／
elapsed 水位。首次验证的接口保持固定；跨轮不续租声明，不接纳改名／替换接口。私有 callback
在活动 F1g I/O 前后复核这些原始边界，而非仅在尝试首尾检查。这仍是相邻前后观测，不是实时
抢占：有界 poll／系统调用可能跨过期限，下一检查会拒绝结果。每轮仍受 F1g 自身 timeout 和
入站预算限制；256 包是每轮额度，不是四轮共用一个额度。

`state()` 只是最近观测的 `Ready`／`CoolingDown`／`Finished`／`Closed`，不在空闲时监控过期。
`retry_after()` 仅为调度提示，`Some(Duration::ZERO)` 不授予权限，下一次显式调用仍须重新检查。
每轮释放两个 socket，冷却／完成态不保留 FD、监听或发送器。此前未公告，Drop 不发 goodbye。
静默观测后关闭再重开不能安全衔接发布，可复制的结果不是许可。公告、持续防御及撤销仍需另行
设计保留 I/O 的生命周期。

限额仅属于这个对象；新建控制器或直接调用 `probe_once` 可绕过该历史，**不是进程／宿主／全局
限速器**。不增加改名、认证、配对或 grant；`--with-controller` 仅选择隔离验收测试。

## F1i 保留静默上下文

`probe_retained_once(config, announcement, cancelled)` 是独立显式单轮入口，借用冻结公告，
使用同一三次 probe 运行层，不接受给定观察结果，也不重复绑定。
`RetainedProbeOutcome` 只有 `ExistingResponse`、`CompetingProbe` 和 `Quiet(MdnsRetainedProbe)`；
仅静默分支移交**原来的两个 socket**、排队数据、原始起点、timeout、租约、时钟水位及已消费
入站计数。原接口快照直接复制，不重新发现；移交期间不关闭重开 socket。

句柄不可 `Clone`，没有公开构造器、FD 导出、重配、续租或发送入口。`review(cancelled)` 非阻塞
排空原双通道，沿用发送后分类和同一预算；即使 poll 为空，返回前仍须新鲜最终检查。静默复核
只返回历史 `NoConflictObserved`；相关声明、格式／来源拒绝、取消、接口或时间失效均永久关闭
并释放两个通道，后续复核返回 `Closed`。`close()` 是幂等本地释放；Drop 不发第四次 probe、
公告或 goodbye。

静默完成时固定 **250 ms 复核窗口**，这是应用限制，不是 RFC 名称归属期；反复复核不续期。
原始整轮 1–2 秒 timeout、完整精度 F0 租约／绝对到期及入站限制仍有效，可能更早到期。
取消只在显式操作时采样，不后台监控；空闲过期句柄仍持有 FD，直至 review、close 或 Drop。
保留 FD 不等于保留有效性；检查仍是相邻观测，不是实时抢占或对管理员变更的原子保护。

原 `probe_once` 和 F1h `attempt` 仍在返回前释放全部 socket；F1h 局部 guard **不**随保留句柄
逸出。新独立入口不继承控制器的重试历史或三十秒上限，静默观察值及该控制器都不能制造这个
上下文。这不是进程级限速、发布许可、完整响应器或公告／防御／撤销生命周期；后者需要另行
限时的保留 I/O 组合，不能把短窗口当无限租约。不接 CLI／启动、配对或 grant；
`--with-retained` 仅选择受保护的隔离测试，不是产品联网开关。

## 交付阶段

| 阶段 | 范围 | 状态 |
|---|---|---|
| F0 | 有界公告、兼容协商及策略过滤后的未验证提示 | 纯准备模块，未挂载 |
| F1 | 明确启用的局域网 mDNS／DNS-SD 适配、有界候选缓存、过期与接口控制 | 源码 0.6.7 收录 F1a TXT／缓存、F1b Linux 单查询、F1c CLI／F1d–e 准备／F1f 单播／F1g 探测／F1h 有限重试／F1i 保留静默复核；发布生命周期、更广互通及 UI 仍待完成 |
| F2 | 持久节点密钥、挑战证明、明确邀请／批准、撤销与防重放 | 计划中，尚无已配对节点注册表 |
| F3 | 原生直连与穿透，失败时使用明确配置的自托管加密中继 | 计划中，尚未选择传输依赖 |
| F4 | 窄范围跨权威工作区授权及另行核验的资源命令 | 计划中，现有跨权威拒绝不变 |
| F5 | 多节点／离线／中继失败验收、Android／浏览器桥接及运维控制 | 计划中，尚无去中心化部署验收 |

局域网发现只作用于本地链路，不是认证，见 [RFC 6762](https://www.rfc-editor.org/rfc/rfc6762.html#section-21)。
跨网直连无法保证；[STUN 本身不是完整穿透方案](https://www.rfc-editor.org/rfc/rfc8489.html)，
有些网络需要中间中继，见 [TURN](https://www.rfc-editor.org/rfc/rfc8656.html)。
F0／F1a 保持纯计算；F1b 仅为上述受限单查询配置，不是完整 mDNS 或穿透实现。

传输目标为局域网优先、跨网能直连则直连、否则使用操作者配置且可替换的中继。
设计不要求唯一官方目录、账号服务或中继；路径不可用时应明确显示离线。
加密必须到达已证明身份的对端，而非只到中继；连接元数据仍可能可见。
自动 UPnP／PCP／NAT-PMP 端口映射、网关变更及公网监听不属于隐含同意。
浏览器／Android 连接另需适配及证书验收，不能假设拥有原生 UDP API。

## 验证与不变边界

`engine/tests/federation_discovery_tdd.rs` 及子模块覆盖严格线格式、版本与能力协商、有效期边界、
自身声明、地址分类、显式启用及固定错误。`engine/tests/federation_lan_tdd.rs` 覆盖来源／缓存／时间／准入，
`engine/tests/federation_lan_txt_tdd.rs` 覆盖单记录封装及 TXT → F0 → 缓存组合。
`scripts/tests/federationDiscovery.test.mjs` 递归守卫纯模块／未挂载的源码边界与双语文档；
这些测试不执行真实发现、打洞或身份认证。
独立 `federation_mdns_*_tdd` 目标覆盖适配器及明确启用的隔离 UDP 边界；
默认测试忽略经守卫的真实网络夹具，而非接触宿主局域网。

**2026-11-13 未发布 F1b 限定验证**通过 **49** 条适配器默认用例（13 条私有单元与 36 条集成）、
**105** 条选定既有 F0／F1a／协作契约用例，另在两个隔离网络命名空间中通过全部 **7** 条真实 UDP。
公共／格式门禁通过 **884** 条脚本检查及其工具、格式检查。先补入失败的 goodbye 预算回归，再修正
终止错误分类，没有放宽拒绝。这不是完整后端／前端／SDK 门禁或远端 CI 结果，未执行物理局域网、
配对或穿透；runner 和清理范围见[测试指南](testing-guide.md)。

**2026-11-13 未发布 F1c 限定验证**通过 **40** 条新增默认 CLI 用例及 **161** 条选定既有发现／
进程／协作契约用例，共 **201** 条。显式命名空间验收通过 **6** 条新 CLI 与原 **7** 条 UDP；
零参数 runner 另行复验同一批 7 条，不算新增覆盖。公共／格式门禁通过 **885** 条脚本检查及工具、
格式检查。首轮真实 CLI 因夹具身份映射在查询前失败，仅修正该映射，未放宽产品进程门禁；
帮助／诊断管道关闭也先补失败回归，再修正固定退出码。这是本地限定证据，不是完整后端／前端／
SDK 门禁、物理局域网或新的远端 CI 结果。

**2026-11-13 未发布 F1d 限定验证**通过 **30** 条新增纯 Rust（27 集成、3 私有 writer）及
**201** 条选定既有用例，共 **231** 条，零失败／忽略。原有 **13** 条隔离发现／CLI 网络用例
另行回归通过，没有发送 F1d 报文，不能算公告验收。公共／格式门禁通过 **886** 条脚本检查及工具、
格式检查。缺少 API／源码守卫先失败，包 writer 的精确预算测试也先失败再实现。
本切片不声明完整后端／前端／SDK 门禁或新远端 CI；公告发送、名称冲突生命周期和独立互通仍待完成。

**2026-11-13 未发布 F1e 限定验证**通过 **46** 条新增纯 Rust（44 probe 集成、2 私有 framing）及
**231** 条选定既有用例，共 **277** 条，零失败／忽略／警告。解码抽取后原 **13** 条隔离发现／CLI
回归通过，没有发送新 probe 或 QU 单播。公共／格式门禁通过 **887** 条脚本检查及工具、格式检查。
缺失 API／源码守卫先失败；初版冗余 QU／flush 字段产生两条 dead-code 警告，改存原 class 值后消除，
没有压制 lint。这不是完整后端／前端／SDK 门禁、新远端 CI、真实探测或名称可用证明。

**2026-11-13 未发布 F1f 限定验证**通过新增准确目的地／来源私有单元及此前 **277** 条选定 Rust，
共 **278** 条，零失败／忽略／警告。全新无外网连接命名空间中，**10** 条新真实单播与原 **13** 条
发现／CLI 全部通过，共 **23** 条不同用例。不带 CLI 的 `--with-unicast` 另通过重叠的 17 条，
原零参数另通过同一批原 7 条，不重复计算覆盖。新目标默认运行另确认 **10 条忽略**、不执行网络。
缺失 API、metadata 和源码守卫先失败再实现；公共／格式门禁通过 **888** 条脚本检查及工具、格式检查。
未接触宿主局域网、应用数据库，未执行产品 probe、新远端 CI 或部署。

**2026-11-13 未发布 F1g 限定验证**通过 **69** 条新增私有 Rust（26 时序、32 限额、9 分类／
精度／前置及 2 MTU／poll）和此前 **278** 条选定用例，共 **347** 条，零失败／忽略／警告。
全新无外网连接命名空间中，**13** 条新真实 probe 和原 **23** 条发现／CLI／单播均通过，共
**36** 条不同用例；不带 CLI 的 `--with-probe` 另通过重叠的 20 条。新目标默认另确认 **13 条忽略**，
不执行网络。公共／格式门禁通过 **889** 条脚本检查及工具、格式检查。缺少 API、时序／限额、
亚毫秒及 poll 中断回归先失败再实现。复审发现中断 poll 被当成空 drain，现硬失败，不自动重试或
放宽断言；新增源码守卫曾因多行格式匹配失败，仅修正 regex，原八条守卫不变。
这不是完整后端／前端／SDK、物理局域网、完整 mDNS、公告、配对、部署或新远端 CI 验收。
对端仅使用合成声明，不使用应用数据库。

**2026-11-13 未发布 F1h 限定验证**通过 **58** 条新增私有 Rust（37 重试 gate、14 固定生命周期、
5 控制器前置及 2 内部 guard 顺序）和此前 **347** 条，共 **405** 条，零失败／忽略／警告。
全新无外网连接命名空间中，**8** 条新控制器网络用例与原 **36** 条一起通过，共 **44** 条不同
用例；不带 CLI 的 `--with-controller` 另通过重叠的 15 条。普通控制器用例零 capability，只有
独立受限接口改名用例持有自有 netns 的 NET_ADMIN；父进程恢复原名后再跑未变的地址删除用例。
新目标默认另确认 **8 条忽略**、不执行网络。公共／格式门禁通过 **890** 条脚本及工具／格式检查，
十条源码守卫和七条 runner 参数检查均通过。缺少 API、gate／生命周期及跨轮活动 guard 回归
先失败再实现。复审发现仅在尝试首尾检查会漏掉后续一轮中途原租约到期，现私有活动 I/O guard
保持该原始边界，不自动重试或放宽断言。这只验收对象内冷却，不是完整后端／前端／SDK 门禁、
进程级限速、保留 I/O 的公告生命周期、物理局域网、配对、部署或新远端 CI 验收。

**2026-11-13 未发布 F1i 限定验证**通过 **38** 条新增私有 Rust（15 复核窗口、16 原边界移交、
4 保留入口前置及 3 关闭句柄／分类）和此前 **405** 条，共 **443** 条，零失败／忽略／警告。
全新无外网连接命名空间中，**11** 条新保留用例与原 **44** 条一起通过，共 **55** 条不同用例；
不带 CLI 的 `--with-retained` 另通过重叠的 18 条。十条普通保留用例零 capability，一条自有
netns NET_ADMIN 改名用例要求关闭，父恢复准确接口名后再跑原变更用例。核验排队状态、反复
复核保持同一 socket inode、准确流量计数及完整 FD／端口释放。新目标默认确认 **11 条忽略**、
不执行网络。公共／格式通过 **891** 条脚本及工具／格式检查；十一条源码守卫、八条 runner
参数检查均通过。缺少入口、复核窗口和原边界移交回归先失败再实现；窗口 Red／Green 只有
工具输出，未另存日志。复审补上即使排空为空也必须最终新鲜检查；新增源码守卫曾因多行格式
匹配失败，仅修正 regex，原十条不变。这只验收短期未公告连续性，不是控制器移交、发布、
完整后端／前端／SDK、物理局域网、认证、部署或新远端 CI；不使用应用数据库或宿主局域网。

[架构地图](architecture-map.md)、[平台链路](platform-network.md)与[安全指南](security.md)
保留当前宿主和授权边界。F0 不新增路由、监听、配置开关、Schema、权限、迁移、运行依赖或部署服务，
F1a 同样遵守这些限制。F1b（包括显式 F1c 调用）仍仅打开明确请求的组播 socket；F1f 独立准确
本机地址接收器需要显式库调用。现有宿主路由、认证、存储、依赖和应用启动不变；
F1g／F1h／F1i 独立显式库调用不新增调用者权限或产品启动接入。
