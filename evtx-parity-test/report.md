# 对比报告（基线解析 vs 新解析）

- 生成时间（本地）：2026-09-29 23:39:29
- 主比对：基线（zh-CN 渲染，G0 自动探测） vs new-parsed/zh（同语言，严格五字段）
- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）
- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）
- 对照：new-parsed/en 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message
- 豁免（ADR 0006）：开启——白名单命中 18 条已移出 diff 计入 exempted（--no-exemptions 关闭复核）

## 结论

- 未通过：主链（基线vs en）共比对 44890 条，一致 44715 条，差异 157 条

## 分通道统计（主：基线 vs zh）

| 通道 | 基线数 | 新解析(zh) | 比对数 | 一致 | 差异 | zh空消息 | 状态 |
|---|---|---|---|---|---|---|---|
| Application | 10742 | 10742 | 10742 | 10726 | 14 | 0 | ok |
| Setup | 676 | 676 | 676 | 676 | 0 | 0 | ok |
| Security | 27366 | 27366 | 27366 | 27366 | 0 | 0 | ok |
| System | 6106 | 6106 | 6106 | 5947 | 143 | 0 | ok |

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

## 差异样例 1（Application #547：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-08-21T09:33:59.8920000Z
- 基线 Message（前300字）：[11024:16136:0821/093359.890:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-08-21 09:33:59.892664
- 新解析zh Message（前300字）：[11024:16136:0821/093359.890:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 2（Application #2972：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1001 Provider=Windows Error Reporting Date=2026-08-30T09:46:53.8230000Z
- 基线 Message（前300字）：故障存储段 2202004278532310831，类型 5;;事件名称: crashpad_log;;响应: 不可用;;Cab ID: 0;;;;问题签名:;;P1: msedgewebview2setup.exe;;P2: 153.0.4234.8;;P3: EdgeInstallerError|msedgewebview;;P4: 0x39;;P5: ;;P6: ;;P7: ;;P8: ;;P9: ;;P10: ;;;;附加文件:;;\\?\C:\Windows\SystemTemp\msedge_installer.log;;\\?\C:\ProgramData\Microsoft\W
- 新解析zh EventID=1001 Provider=Windows Error Reporting Time=2026-08-30 09:46:53.823052
- 新解析zh Message（前300字）：故障存储段 2202004278532310831，类型 5 事件名称: crashpad_log 响应: 不可用 Cab ID: 0 问题签名: P1: msedgewebview2setup.exe P2: 153.0.4234.8 P3: EdgeInstallerError|msedgewebview P4: 0x39 P5: P6: P7: P8: P9: P10: 附加文件: \\?\C:\Windows\SystemTemp\msedge_installer.log \\?\C:\ProgramData\Microsoft\Windows\WER\Temp\WER.310a2ed

## 差异样例 3（Application #3429：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1001 Provider=Windows Error Reporting Date=2026-09-01T12:46:10.9150000Z
- 基线 Message（前300字）：故障存储段 1584263963967378820，类型 5;;事件名称: crashpad_log;;响应: 不可用;;Cab ID: 0;;;;问题签名:;;P1: msedgewebview2setup.exe;;P2: 153.0.4234.13;;P3: EdgeInstallerError|msedgewebview;;P4: 0x1f7;;P5: ;;P6: ;;P7: ;;P8: ;;P9: ;;P10: ;;;;附加文件:;;\\?\C:\Windows\SystemTemp\msedge_installer.log;;\\?\C:\ProgramData\Microsoft
- 新解析zh EventID=1001 Provider=Windows Error Reporting Time=2026-09-01 12:46:10.915606
- 新解析zh Message（前300字）：故障存储段 1584263963967378820，类型 5 事件名称: crashpad_log 响应: 不可用 Cab ID: 0 问题签名: P1: msedgewebview2setup.exe P2: 153.0.4234.13 P3: EdgeInstallerError|msedgewebview P4: 0x1f7 P5: P6: P7: P8: P9: P10: 附加文件: \\?\C:\Windows\SystemTemp\msedge_installer.log \\?\C:\ProgramData\Microsoft\Windows\WER\Temp\WER.4ea2c

## 差异样例 4（Application #3675：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-02T11:07:12.9710000Z
- 基线 Message（前300字）：[15004:15088:0902/110712.971:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-02 11:07:12.971972
- 新解析zh Message（前300字）：[15004:15088:0902/110712.971:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 5（Application #3676：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-02T11:07:28.4420000Z
- 基线 Message（前300字）：[27712:30628:0902/110728.441:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-02 11:07:28.442776
- 新解析zh Message（前300字）：[27712:30628:0902/110728.441:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 6（Application #4281：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-04T09:41:41.6530000Z
- 基线 Message（前300字）：[20816:21360:0904/094141.651:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-04 09:41:41.653063
- 新解析zh Message（前300字）：[20816:21360:0904/094141.651:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 7（Application #5841：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-10T09:56:16.3300000Z
- 基线 Message（前300字）：[15680:4604:0910/095616.329:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-10 09:56:16.330736
- 新解析zh Message（前300字）：[15680:4604:0910/095616.329:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 8（Application #7768：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-17T11:51:04.5630000Z
- 基线 Message（前300字）：[38300:48632:0917/115104.562:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-17 11:51:04.563219
- 新解析zh Message（前300字）：[38300:48632:0917/115104.562:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 9（Application #7769：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-17T11:51:59.1400000Z
- 基线 Message（前300字）：[43076:50516:0917/115159.139:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-17 11:51:59.140383
- 新解析zh Message（前300字）：[43076:50516:0917/115159.139:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 10（Application #7770：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Chrome Date=2026-09-17T11:56:11.8890000Z
- 基线 Message（前300字）：[2396:12344:0917/115611.888:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Chrome Time=2026-09-17 11:56:11.889913
- 新解析zh Message（前300字）：[2396:12344:0917/115611.888:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 11（Application #9751：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1001 Provider=CodexSandboxService.OpenAI.Codex Date=2026-09-25T09:42:16.0230000Z
- 基线 Message（前300字）：递归太深；堆栈溢出。
- 新解析zh EventID=1001 Provider=CodexSandboxService.OpenAI.Codex Time=2026-09-25 09:42:16.023506
- 新解析zh Message（前300字）：N/A

## 差异样例 12（Application #9752：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1002 Provider=CodexSandboxService.OpenAI.Codex Date=2026-09-25T09:42:16.0240000Z
- 基线 Message（前300字）：窗口无法在已发送的消息上操作。
- 新解析zh EventID=1002 Provider=CodexSandboxService.OpenAI.Codex Time=2026-09-25 09:42:16.024189
- 新解析zh Message（前300字）：N/A

## 差异样例 13（Application #10663：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Edge Date=2026-09-29T09:23:02.8750000Z
- 基线 Message（前300字）：[42052:40148:0929/092302.874:INFO:chrome\browser\extensions\extension_garbage_collector.cc:212] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Edge Time=2026-09-29 09:23:02.875115
- 新解析zh Message（前300字）：[42052:40148:0929/092302.874:INFO:chrome\browser\extensions\extension_garbage_collector.cc:212] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 14（Application #10689：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=256 Provider=Edge Date=2026-09-29T10:23:06.6860000Z
- 基线 Message（前300字）：[10116:38528:0929/102306.685:INFO:chrome\browser\extensions\extension_garbage_collector.cc:212] Garbage collection for extensions on file thread is complete.
- 新解析zh EventID=256 Provider=Edge Time=2026-09-29 10:23:06.686255
- 新解析zh Message（前300字）：[10116:38528:0929/102306.685:INFO:chrome\browser\extensions\extension_garbage_collector.cc:212] Garbage collection for extensions on file thread is complete.!S!

## 差异样例 15（System #17：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:12.2920000Z
- 基线 Message（前300字）：卷 C: (\Device\HarddiskVolume4) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:12.292166
- 新解析zh Message（前300字）：卷 C: (\Device\HarddiskVolume4) is healthy. No action is needed.

## 差异样例 16（System #36：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:13.8180000Z
- 基线 Message（前300字）：卷 D: (\Device\HarddiskVolume7) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:13.818907
- 新解析zh Message（前300字）：卷 D: (\Device\HarddiskVolume7) is healthy. No action is needed.

## 差异样例 17（System #37：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:14.4290000Z
- 基线 Message（前300字）：卷 F: (\Device\HarddiskVolume6) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:14.429749
- 新解析zh Message（前300字）：卷 F: (\Device\HarddiskVolume6) is healthy. No action is needed.

## 差异样例 18（System #38：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:15.0580000Z
- 基线 Message（前300字）：卷 E: (\Device\HarddiskVolume1) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:15.058621
- 新解析zh Message（前300字）：卷 E: (\Device\HarddiskVolume1) is healthy. No action is needed.

## 差异样例 19（System #39：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:15.2430000Z
- 基线 Message（前300字）：卷 \\?\Volume{903de723-f029-45cf-a8ab-3d4195bc9b92} (\Device\HarddiskVolume5) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:15.243994
- 新解析zh Message（前300字）：卷 \\?\Volume{903de723-f029-45cf-a8ab-3d4195bc9b92} (\Device\HarddiskVolume5) is healthy. No action is needed.

## 差异样例 20（System #60：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=98 Provider=Microsoft-Windows-Ntfs Date=2026-08-21T05:02:59.4100000Z
- 基线 Message（前300字）：卷 \\?\Volume{d849e7c4-ff65-481b-aba4-7db8188d6758} (\Device\HarddiskVolume8) 运行状况良好。无需执行任何操作。
- 新解析zh EventID=98 Provider=Microsoft-Windows-Ntfs Time=2026-08-21 05:02:59.410638
- 新解析zh Message（前300字）：卷 \\?\Volume{d849e7c4-ff65-481b-aba4-7db8188d6758} (\Device\HarddiskVolume8) is healthy. No action is needed.

还有 137 条差异未展示，见 report.diff。
