# 真实授权源与 HTTPS 联验

状态：**源码 0.5.9 收录的 Unix 外部测试覆盖**，API **0.3.0**，**2026-10-13**。
本[提案 #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)跟进原不在0.5.8标签内，分日期执行保留原范围，
将[真实Source交付](large-file-delivery-client.md)、[配对路由](large-file-distribution-router.md)
与[受控HTTPS客户端](large-file-delivery-https.md)放进同一条验收链。只增加测试覆盖，
不增加生产监听器、信任策略或Session签发。

## 一条真实授权与字节路径

显式临时PostgreSQL夹具通过原命令建立已审计身份、Session、Published文件和独立接收grant，
字节来自原Source文件。一个所给Source／分块workspace构造一个分发router；客户端仍按原
准确期望版本及逐调用私有Session cookie请求，不从HTTPS身份推断文件权限。

临时OpenSSL终止loopback HTTPS，解密后的原method、raw URI、重复headers及实际正文进入
该router。测试传输写回真实status、响应头及Body frames，不重新生成清单或分块响应。
缺失Content-Length时可按实际正文准确size hint补传输长度，类似正常HTTP服务接线，但仍
要求实际所有帧及EOF；connection-close只作传输分帧，不增加响应或授权契约。

原binary Bytes owner在逐帧管道写入／持有期间存活，不先压成整正文Vec再早释放原Source／
HTTP名额。测试观察及OpenSSL／内核缓冲不计入既有预算，本层不证明全局内存配额。

## 三类独立边界

- 完整controlled交付覆盖真实Published大／短／空文件，核对准确GET、私有凭据、所选文件
  实际字节／摘要／所有者／权限及不变Source记录；空文件也必须请求当前授权清单。
- 第一块真实ACK之后，在后块进入Source之前撤权。专用 `BeforeDispatch` 门闩不是响应头
  持有：Source一旦批准字节快照，之后撤权不能收回它。后块私有404仅保留旧确认前缀，
  不产生最终拼装或所选保存；已撤权空文件在清单阶段拒绝。
- 显式取消在真实Source返回后持有实际TLS清单或后块正文。八字节管道前缀只证明pipe接受，
  不声称客户端已消费；Cancelled结束客户端等待／续跑，不撤销已批准字节、杀OS任务或
  回滚输出。先停止旧peer，再用独立健康peer／控制流程复核。

保留原错误投影：证书／非超时传输失败为元数据Unavailable，响应头期限超时为TimedOut，
Source私有拒绝在对应阶段为InvalidResponse。此映射不声称新增三例已实跑所有超时／证书
路径；不发明更丰富远端错误、重试、采用、删除、续传或持续grant核验。

## 执行与清理

三条 `source::source_https` 用例属于 `file_delivery_tdd`，默认显式忽略。
`bash scripts/test-file-delivery.sh` 精确执行六条Source：原三条和新增三条Source／HTTPS联验，
同时要求显式临时 `CYANREX_TEST_DATABASE_URL` 和已安装OpenSSL。既有CI入口执行同一库存；
`fileSourceHttps` 静态守卫只核接线和证据范围，不冒称SQL／TLS行为。

完整成功正控先等待responder实际正文EOF计数，再停止连接；客户端按Content-Length完成
不等于responder已轮到最后一次EOF polling。后块持有实测Source字节容量；配对HTTP／Source
名额的独立证据仍在原router lifetime用例，不重新标作本轮新增TLS探测。

原生pipe、门闩、router／帧等待和证书命令均有界。先停止owned responder与派发等待、释放
门闩、kill／reap OpenSSL并join线程，再drain原Source工作，最后删除该夹具SQL namespace及
私有目录／key材料；不使用在线数据库、系统CA、既有输出或无关目录。

这仍是一次性loopback联验。正常应用／UI／SDK挂载、安全公共Session签发、无特权数据服务
启动器、已部署局域网验收、恢复及物理磁盘策略仍独立。版本／依赖、分日期张量评分、已发布
报告、**106历史状态块**及**56既有执行尾**不变。下列为新执行证据，不拿旧独立SQL或模拟
TLS运行代替本条联合路径。

## 执行记录 — 2026-10-13，0.5.8 后未发布

- 新外部用例编译通过、无告警；未修改生产行为，不冒称生产Red。格式化后一度有十五条
  聚焦守卫中的两条仅因空白差异拒绝；改为格式无关比较后十五条全部通过。
- 首轮显式原生运行九条唯一用例全部通过。复查发现测试夹具停机／计数的潜在竞态，并非
  已观测生产故障；成功正控改为最多十秒等待真实responder EOF计数，准确数量断言不放宽。
- 最终原生复跑九条唯一用例再次通过：三条原Source、三条新增真实Source／HTTPS、三条
  原controlled模拟TLS。新增三条是SQL与TLS覆盖的交集，不在两个总数中重复相加；默认
  Cargo仍不自动执行任何外部数据库／TLS用例。
- 真实所选短文件、4 MiB + 17字节、空文件在核验夹具清理之后通过独立字节／摘要／EOF、
  所有者／权限和独立inode断言；原Source记录、原文件及SQL对象快照除准确主动grant变化
  外保持不变。
- 第一块ACK后、后块进入Source前撤权，得到实际私有404及原索引InvalidResponse，仅保留
  已校验前缀。显式响应头／正文取消关闭旧观察且不产生Complete；停止持有binary的peer
  后实际Source字节ACK恢复，独立健康peer／目录保存完整校验输出。
- 十三组相关静态检查实际通过 **65** 条。完整后端／通用门通过：**1,097 Rust通过**、
  **861显式忽略**、**98结果组**、**356通用检查**，零失败、零Rust告警；最终本层守卫亦
  **5/5** 通过。
- 摘要守卫确认 **484个engine/src文件**、**510个受保护文件**不变，原 **106历史状态块／
  56执行尾／8历史受保护文件**不变。源码／API仍为 **0.5.8／0.3.0**，已发布0.5.7与0.5.8
  报告／标签没有重写。
- SQL／TLS全部结束后，停止本轮独占UID1000、0700、仅socket临时PostgreSQL，删除准确的
  临时data／socket目录，回收 **57,740 KiB**。日志保留于 `/tmp/cyanrex-source-https-57DyWUCm`
  （`native-final.log`、`full-backend.log`及guard／build／stop日志）；未删除项目缓存、既有
  输出、在线数据库、老旧数据或系统信任材料。

本轮不冒称新前端／SDK构建、浏览器／已部署局域网／内核验收、安全审计或离线发布包。
仅增加测试、文档和显式runner库存，不证明正常应用挂载、权限隔离、全局内存配额或重启恢复。
