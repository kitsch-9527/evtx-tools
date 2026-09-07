# 对比报告（基线解析 vs 新解析）

- 生成时间（本地）：2026-09-07 14:54:12
- 主比对：中文wevtutil文本基线 vs new-parsed/en（同语言，严格五字段）
- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）
- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）
- 对照：new-parsed/zh 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message

## 结论

- 未通过：主链（基线vs en）共比对 32801 条，一致 31870 条，差异 931 条

## 分通道统计（主：基线 vs en）

| 通道 | 基线数 | 新解析(en) | 比对数 | 一致 | 差异 | en空消息 | 状态 |
|---|---|---|---|---|---|---|---|
| Application | 3925 | 3925 | 3925 | 3833 | 92 | 0 | ok |
| Setup | 222 | 222 | 222 | 222 | 0 | 0 | ok |
| Security | 26031 | 26031 | 26031 | 26031 | 0 | 0 | ok |
| System | 2623 | 2623 | 2623 | 1784 | 839 | 0 | ok |

## 中文覆盖率对照（基线 vs zh，不比Message）

| 通道 | 新解析(zh) | zh空消息 | EventID顺序一致/比对 |
|---|---|---|---|
| Application | 3925 | 0 | 3925/3925 |
| Setup | 222 | 0 | 222/222 |
| Security | 26031 | 0 | 26031/26031 |
| System | 2623 | 0 | 2623/2623 |

## 已知事项（比对中发现，非结论）

- 新解析 CSV 内每 batch 夹带一行 BOM+表头重复（evtx-tools write_events 每批都写头），比对时已按 event_id 非数字过滤；建议上游修 storage/csv.rs 或 cmd_parse.rs。
- 基线 Date 看似UTC（带Z）实为本地时间：wevtutil /f:text 渲染本地时间但保留Z后缀（与raw XML SystemTime差+8h，Setup#0/EventRecordID=1实测）；evtx-tools convert_timestamp_local输出与raw UTC+8一致，无上游bug。基线小数秒被截断补零（.6720013→.6720000），容差覆盖。

## 差异样例 1（Application #93：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=AsusUpdateCheck Date=2026-08-21T05:03:37.6210000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=AsusUpdateCheck Time=2026-08-21 05:03:37.621425
- 新解析en Message（前300字）：AsusUpdateCheck;;CServiceControl in OnStart;;

## 差异样例 2（Application #94：C-无模板fallback（见明细））

- 基线 EventID=0 Provider=WMIRegistrationService Date=2026-08-21T05:03:44.6560000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service started.
- 新解析en EventID=0 Provider=WMIRegistrationService Time=2026-08-21 05:03:44.656955
- 新解析en Message（前300字）：The description for Event ID 0 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer

## 差异样例 3（Application #95：C-无模板fallback（见明细））

- 基线 EventID=0 Provider=IntelDalJhi Date=2026-08-21T05:03:44.9600000Z
- 基线 Message（前300字）：Intel(R) Dynamic Application Loader Host Interface Service started.
- 新解析en EventID=0 Provider=IntelDalJhi Time=2026-08-21 05:03:44.960088
- 新解析en Message（前300字）：The description for Event ID 0 from source IntelDalJhi cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the displ

## 差异样例 4（Application #96：C-无模板fallback（见明细））

- 基线 EventID=2 Provider=WMIRegistrationService Date=2026-08-21T05:03:45.0760000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service has successfully finished WMI Registration.
- 新解析en EventID=2 Provider=WMIRegistrationService Time=2026-08-21 05:03:45.076713
- 新解析en Message（前300字）：The description for Event ID 2 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer

## 差异样例 5（Application #97：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:03:45.4400000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:03:45.440834
- 新解析en Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStart;;

## 差异样例 6（Application #98：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:03:45.7530000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:03:45.753975
- 新解析en Message（前300字）：Service started/resumed;;

## 差异样例 7（Application #108：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:05:25.3670000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:05:25.367411
- 新解析en Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStop;;

## 差异样例 8（Application #110：C-无模板fallback（见明细））

- 基线 EventID=4625 Provider=Microsoft-Windows-EventSystem Date=2026-08-21T05:06:12.6980000Z
- 基线 Message（前300字）：The EventSystem sub system is suppressing duplicate event log entries for a duration of 86400 seconds. The suppression timeout can be controlled by a REG_DWORD value named SuppressDuplicateDuration under the following registry key: HKLM\Software\Microsoft\EventSystem\EventLog.
- 新解析en EventID=4625 Provider=Microsoft-Windows-EventSystem Time=2026-08-21 05:06:12.698028
- 新解析en Message（前300字）：The description for Event ID 4625 from source Microsoft-Windows-EventSystem cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on anothe

## 差异样例 9（Application #111：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:06:12.7320000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:06:12.732464
- 新解析en Message（前300字）：Service started/resumed;;

## 差异样例 10（Application #113：C-无模板fallback（见明细））

- 基线 EventID=0 Provider=WMIRegistrationService Date=2026-08-21T05:06:13.1310000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service started.
- 新解析en EventID=0 Provider=WMIRegistrationService Time=2026-08-21 05:06:13.131162
- 新解析en Message（前300字）：The description for Event ID 0 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer

## 差异样例 11（Application #114：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=AsusUpdateCheck Date=2026-08-21T05:06:13.1510000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=AsusUpdateCheck Time=2026-08-21 05:06:13.151862
- 新解析en Message（前300字）：AsusUpdateCheck;;CServiceControl in OnStart;;

## 差异样例 12（Application #115：C-无模板fallback（见明细））

- 基线 EventID=0 Provider=IntelDalJhi Date=2026-08-21T05:06:13.1720000Z
- 基线 Message（前300字）：Intel(R) Dynamic Application Loader Host Interface Service started.
- 新解析en EventID=0 Provider=IntelDalJhi Time=2026-08-21 05:06:13.172532
- 新解析en Message（前300字）：The description for Event ID 0 from source IntelDalJhi cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the displ

## 差异样例 13（Application #116：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:06:13.3410000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:06:13.341845
- 新解析en Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStart;;

## 差异样例 14（Application #151：C-无模板fallback（见明细））

- 基线 EventID=2 Provider=WMIRegistrationService Date=2026-08-21T05:06:16.5600000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service has successfully finished WMI Registration.
- 新解析en EventID=2 Provider=WMIRegistrationService Time=2026-08-21 05:06:16.560692
- 新解析en Message（前300字）：The description for Event ID 2 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer

## 差异样例 15（Application #224：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1004 Provider=Microsoft-Windows-Security-SPP Date=2026-08-21T05:07:11.6530000Z
- 基线 Message（前300字）：The Software Protection service has successfully installed the license.;;License Title=Office 19 Publishing License (Public);;License Id=cc94d1c3-85e3-4b36-986e-0d3935872741
- 新解析en EventID=1004 Provider=Microsoft-Windows-Security-SPP Time=2026-08-21 05:07:11.653417
- 新解析en Message（前300字）：The Software Protection service has successfully installed the license. License Title=Office 19 Publishing License (Public) License Id=cc94d1c3-85e3-4b36-986e-0d0xea98a6e5

## 差异样例 16（Application #323：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=asus Date=2026-08-21T05:08:23.2070000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=asus Time=2026-08-21 05:08:23.207814
- 新解析en Message（前300字）：Service stopped;;

## 差异样例 17（Application #351：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4430000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.443628
- 新解析en Message（前300字）：Log on event received User1 ;;

## 差异样例 18（Application #352：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4490000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.449453
- 新解析en Message（前300字）：Logon: 1 ;;

## 差异样例 19（Application #353：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4840000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析en EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.484614
- 新解析en Message（前300字）：Received Post Shell Event 1 ;;

## 差异样例 20（Application #360：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=1001 Provider=Windows Error Reporting Date=2026-08-21T05:09:43.2130000Z
- 基线 Message（前300字）：Fault bucket 2221876170411248103, type 4;;Event Name: APPCRASH;;Response: 不可用;;Cab Id: 0;;;;Problem signature:;;P1: RstMwService.exe;;P2: 17.8.0.1065;;P3: 5dee8560;;P4: RstMwService.exe;;P5: 17.8.0.1065;;P6: 5dee8560;;P7: c0000005;;P8: 0000000000066aa5;;P9: ;;P10: ;;;;Attached files:;;\\?\C:\Program
- 新解析en EventID=1001 Provider=Windows Error Reporting Time=2026-08-21 05:09:43.213902
- 新解析en Message（前300字）：Fault bucket 2221876170411248103, type 4 Event Name: APPCRASH Response: 不可用 Cab Id: 0 Problem signature: P1: RstMwService.exe P2: 17.8.0.1065 P3: 5dee8560 P4: RstMwService.exe P5: 17.8.0.1065 P6: 5dee8560 P7: c0000005 P8: 0000000000066aa5 P9: P10: Attached files: \\?\C:\ProgramData\Microsoft\Windows

还有 911 条差异未展示，见 report.diff。
