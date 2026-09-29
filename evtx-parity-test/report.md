# 对比报告（基线解析 vs 新解析）

- 生成时间（本地）：2026-09-30 03:23:09
- 主比对：基线（zh-CN 渲染，G0 自动探测） vs new-parsed/zh（同语言，严格五字段）
- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）
- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）
- 对照：new-parsed/en 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message
- 豁免（ADR 0006）：开启——白名单命中 17 条已移出 diff 计入 exempted（--no-exemptions 关闭复核）

## 结论

- 未通过：主链（基线vs en）共比对 44890 条，一致 44852 条，差异 21 条

## 分通道统计（主：基线 vs zh）

| 通道 | 基线数 | 新解析(zh) | 比对数 | 一致 | 差异 | zh空消息 | 状态 |
|---|---|---|---|---|---|---|---|
| Application | 10742 | 10742 | 10742 | 10739 | 2 | 0 | ok |
| Setup | 676 | 676 | 676 | 676 | 0 | 0 | ok |
| Security | 27366 | 27366 | 27366 | 27366 | 0 | 0 | ok |
| System | 6106 | 6106 | 6106 | 6071 | 19 | 0 | ok |

## 副侧覆盖率对照（基线 vs en，不比Message）

| 通道 | 新解析(en) | 空消息 | EventID顺序一致/比对 |
|---|---|---|---|
| Application | 10742 | 0 | 10742/10742 |
| Setup | 676 | 0 | 676/676 |
| Security | 27366 | 0 | 27366/27366 |
| System | 6106 | 0 | 6106/6106 |

## 已知事项（比对中发现，非结论）

- 新解析 CSV 内每 batch 夹带一行 BOM+表头重复（evtx-tools write_events 每批都写头），比对时已按 event_id 非数字过滤；建议上游修 storage/csv.rs 或 cmd_parse.rs。
- 基线 Date 看似UTC（带Z）实为本地时间：wevtutil /f:text 渲染本地时间但保留Z后缀（与raw XML SystemTime差+8h，Setup#0/EventRecordID=1实测）；evtx-tools convert_timestamp_local输出与raw UTC+8一致，无上游bug。基线小数秒被截断补零（.6720013→.6720000），容差覆盖。

## 差异样例 1（Application #9751：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1001 Provider=CodexSandboxService.OpenAI.Codex Date=2026-09-25T09:42:16.0230000Z
- 基线 Message（前300字）：递归太深；堆栈溢出。
- 新解析zh EventID=1001 Provider=CodexSandboxService.OpenAI.Codex Time=2026-09-25 09:42:16.023506
- 新解析zh Message（前300字）：N/A

## 差异样例 2（Application #9752：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1002 Provider=CodexSandboxService.OpenAI.Codex Date=2026-09-25T09:42:16.0240000Z
- 基线 Message（前300字）：窗口无法在已发送的消息上操作。
- 新解析zh EventID=1002 Provider=CodexSandboxService.OpenAI.Codex Time=2026-09-25 09:42:16.024189
- 新解析zh Message（前300字）：N/A

## 差异样例 3（System #132：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-08-21T05:03:38.7900000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-08-21 05:03:38.790802
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 4（System #159：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-08-21T05:03:47.9620000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-08-21 05:03:47.962030
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 5（System #273：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-08-21T05:05:54.8680000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-08-21 05:05:54.868527
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 6（System #292：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-08-21T05:05:58.6570000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-08-21 05:05:58.657466
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 7（System #482：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=6008 Provider=EventLog Date=2026-08-21T09:31:08.4640000Z
- 基线 Message（前300字）：上一次系统的 9:16:12 在 ?2026/?8/?21 上的关闭是意外的。
- 新解析zh EventID=6008 Provider=EventLog Time=2026-08-21 09:31:08.464621
- 新解析zh Message（前300字）：上一次系统的 9:16:12 在 ‎2026/‎8/‎21 上的关闭是意外的。

## 差异样例 8（System #516：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-08-21T09:30:50.1560000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-08-21 09:30:50.156278
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 9（System #535：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-08-21T09:30:53.9610000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-08-21 09:30:53.961913
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 10（System #2725：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=6008 Provider=EventLog Date=2026-09-04T09:36:51.6170000Z
- 基线 Message（前300字）：上一次系统的 8:56:02 在 ?2026/?9/?4 上的关闭是意外的。
- 新解析zh EventID=6008 Provider=EventLog Time=2026-09-04 09:36:51.617668
- 新解析zh Message（前300字）：上一次系统的 8:56:02 在 ‎2026/‎9/‎4 上的关闭是意外的。

## 差异样例 11（System #2751：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-09-04T09:34:57.8330000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-09-04 09:34:57.833017
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 12（System #2779：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-09-04T09:35:01.6380000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-09-04 09:35:01.638642
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 13（System #2833：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-09-04T09:36:23.7890000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-09-04 09:36:23.789167
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 14（System #2851：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-09-04T09:36:27.5610000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-09-04 09:36:27.561395
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 15（System #2982：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-09-04T09:38:01.9990000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-09-04 09:38:01.999169
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 16（System #3001：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-09-04T09:38:05.9490000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-09-04 09:38:05.949876
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 17（System #3005：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=23 Provider=Microsoft-Windows-Wininit Date=2026-09-04T09:38:13.9400000Z
- 基线 Message（前300字）：启动应用反回滚： 初始化已完成，状态为：;;HRESULT： STATUS_SUCCESS;;需要新计时器： true;;等待系统更新完成的新计时器： true;;以前的闩锁计时器存在，但被注册表禁用： false
- 新解析zh EventID=23 Provider=Microsoft-Windows-Wininit Time=2026-09-04 09:38:13.940522
- 新解析zh Message（前300字）：启动应用反回滚： 初始化已完成，状态为： HRESULT： 0x10000000 需要新计时器： true 等待系统更新完成的新计时器： true 以前的闩锁计时器存在，但被注册表禁用： false

## 差异样例 18（System #3780：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-09-10T09:49:50.1940000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-09-10 09:49:50.194739
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

## 差异样例 19（System #3799：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=32 Provider=e1dexpress Date=2026-09-10T09:49:53.9680000Z
- 基线 Message（前300字）：Intel(R) Ethernet Connection (12) I219-V;; 网络链接已以 1Gbps 全双工建立。
- 新解析zh EventID=32 Provider=e1dexpress Time=2026-09-10 09:49:53.968015
- 新解析zh Message（前300字）：Intel(R) Ethernet Connection (12) I219-V Se ha establecido el enlace de red en 1 Gbps dúplex completo.

## 差异样例 20（System #3947：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=36 Provider=VMnetAdapter Date=2026-09-10T09:52:01.7930000Z
- 基线 Message（前300字）：() Starting up the Adapter Driver for VMware Virtual Networks.
- 新解析zh EventID=36 Provider=VMnetAdapter Time=2026-09-10 09:52:01.793567
- 新解析zh Message（前300字）：一个用户已达到卷 00000000010000000000000024000840240008400000000000000000000000000000000000000000 上的配额阈值。

还有 1 条差异未展示，见 report.diff。
