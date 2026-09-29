# 对比报告（基线解析 vs 新解析）

- 生成时间（本地）：2026-09-29 15:32:02
- 主比对：基线（zh-CN 渲染，G0 自动探测） vs new-parsed/zh（同语言，严格五字段）
- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）
- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）
- 对照：new-parsed/en 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message
- 豁免（ADR 0006）：开启——白名单命中 19 条已移出 diff 计入 exempted（--no-exemptions 关闭复核）

## 结论

- 未通过：主链（基线vs en）共比对 44890 条，一致 43356 条，差异 1515 条

## 分通道统计（主：基线 vs zh）

| 通道 | 基线数 | 新解析(zh) | 比对数 | 一致 | 差异 | zh空消息 | 状态 |
|---|---|---|---|---|---|---|---|
| Application | 10742 | 10742 | 10742 | 10429 | 311 | 0 | ok |
| Setup | 676 | 676 | 676 | 676 | 0 | 0 | ok |
| Security | 27366 | 27366 | 27366 | 27366 | 0 | 0 | ok |
| System | 6106 | 6106 | 6106 | 4885 | 1204 | 0 | ok |

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

## 差异样例 1（Application #93：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=AsusUpdateCheck Date=2026-08-21T05:03:37.6210000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=AsusUpdateCheck Time=2026-08-21 05:03:37.621425
- 新解析zh Message（前300字）：AsusUpdateCheck;;CServiceControl in OnStart;;

## 差异样例 2（Application #94：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=0 Provider=WMIRegistrationService Date=2026-08-21T05:03:44.6560000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service started.
- 新解析zh EventID=0 Provider=WMIRegistrationService Time=2026-08-21 05:03:44.656955
- 新解析zh Message（前300字）：无法找到来自源 WMIRegistrationService 的事件 ID 0 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 3（Application #95：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=0 Provider=IntelDalJhi Date=2026-08-21T05:03:44.9600000Z
- 基线 Message（前300字）：Intel(R) Dynamic Application Loader Host Interface Service started.
- 新解析zh EventID=0 Provider=IntelDalJhi Time=2026-08-21 05:03:44.960088
- 新解析zh Message（前300字）：无法找到来自源 IntelDalJhi 的事件 ID 0 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 4（Application #96：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=2 Provider=WMIRegistrationService Date=2026-08-21T05:03:45.0760000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service has successfully finished WMI Registration.
- 新解析zh EventID=2 Provider=WMIRegistrationService Time=2026-08-21 05:03:45.076713
- 新解析zh Message（前300字）：无法找到来自源 WMIRegistrationService 的事件 ID 2 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 5（Application #97：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:03:45.4400000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:03:45.440834
- 新解析zh Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStart;;

## 差异样例 6（Application #98：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:03:45.7530000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:03:45.753975
- 新解析zh Message（前300字）：Service started/resumed;;

## 差异样例 7（Application #108：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:05:25.3670000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:05:25.367411
- 新解析zh Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStop;;

## 差异样例 8（Application #110：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=4625 Provider=Microsoft-Windows-EventSystem Date=2026-08-21T05:06:12.6980000Z
- 基线 Message（前300字）：EventSystem 子系统正在取消 86400 秒持续时间内重复的事件日志项。可以通过下列注册表项下名为 SuppressDuplicateDuration 的 REG_DWORD 值控制取消超时: HKLM\Software\Microsoft\EventSystem\EventLog。
- 新解析zh EventID=4625 Provider=Microsoft-Windows-EventSystem Time=2026-08-21 05:06:12.698028
- 新解析zh Message（前300字）：无法找到来自源 Microsoft-Windows-EventSystem 的事件 ID 4625 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 86400 SuppressDuplicateDuration Software\Microsoft\EventSystem\EventLog 消息资源存在，但在消息表中找不到该消息。

## 差异样例 9（Application #111：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:06:12.7320000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:06:12.732464
- 新解析zh Message（前300字）：Service started/resumed;;

## 差异样例 10（Application #113：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=0 Provider=WMIRegistrationService Date=2026-08-21T05:06:13.1310000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service started.
- 新解析zh EventID=0 Provider=WMIRegistrationService Time=2026-08-21 05:06:13.131162
- 新解析zh Message（前300字）：无法找到来自源 WMIRegistrationService 的事件 ID 0 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 11（Application #114：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=AsusUpdateCheck Date=2026-08-21T05:06:13.1510000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=AsusUpdateCheck Time=2026-08-21 05:06:13.151862
- 新解析zh Message（前300字）：AsusUpdateCheck;;CServiceControl in OnStart;;

## 差异样例 12（Application #115：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=0 Provider=IntelDalJhi Date=2026-08-21T05:06:13.1720000Z
- 基线 Message（前300字）：Intel(R) Dynamic Application Loader Host Interface Service started.
- 新解析zh EventID=0 Provider=IntelDalJhi Time=2026-08-21 05:06:13.172532
- 新解析zh Message（前300字）：无法找到来自源 IntelDalJhi 的事件 ID 0 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 13（Application #116：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=RtkAudioUniversalService Date=2026-08-21T05:06:13.3410000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=RtkAudioUniversalService Time=2026-08-21 05:06:13.341845
- 新解析zh Message（前300字）：RtkAudioUniversalService;;CppWindowsService in OnStart;;

## 差异样例 14（Application #151：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=2 Provider=WMIRegistrationService Date=2026-08-21T05:06:16.5600000Z
- 基线 Message（前300字）：Intel(R) WMI Registration Service has successfully finished WMI Registration.
- 新解析zh EventID=2 Provider=WMIRegistrationService Time=2026-08-21 05:06:16.560692
- 新解析zh Message（前300字）：无法找到来自源 WMIRegistrationService 的事件 ID 2 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: 消息资源存在，但在消息表中找不到该消息。

## 差异样例 15（Application #323：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=asus Date=2026-08-21T05:08:23.2070000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=asus Time=2026-08-21 05:08:23.207814
- 新解析zh Message（前300字）：Service stopped;;

## 差异样例 16（Application #351：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4430000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.443628
- 新解析zh Message（前300字）：Log on event received User1 ;;

## 差异样例 17（Application #352：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4490000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.449453
- 新解析zh Message（前300字）：Logon: 1 ;;

## 差异样例 18（Application #353：B-裸参数未渲染（见明细））

- 基线 EventID=0 Provider=igfxCUIService2.0.0.0 Date=2026-08-21T05:09:28.4840000Z
- 基线 Message（前300字）：操作成功完成。
- 新解析zh EventID=0 Provider=igfxCUIService2.0.0.0 Time=2026-08-21 05:09:28.484614
- 新解析zh Message（前300字）：Received Post Shell Event 1 ;;

## 差异样例 19（Application #410：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=8300 Provider=Microsoft-Windows-System-Restore Date=2026-08-21T05:35:17.9580000Z
- 基线 Message（前300字）：Scoping started for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1.
- 新解析zh EventID=8300 Provider=Microsoft-Windows-System-Restore Time=2026-08-21 05:35:17.958668
- 新解析zh Message（前300字）：无法找到来自源 Microsoft-Windows-System-Restore 的事件 ID 8300 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1 消息资源存在，但在消息表中找不到该消息。

## 差异样例 20（Application #411：D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细））

- 基线 EventID=8300 Provider=Microsoft-Windows-System-Restore Date=2026-08-21T05:35:27.6030000Z
- 基线 Message（前300字）：Scoping started for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy2.
- 新解析zh EventID=8300 Provider=Microsoft-Windows-System-Restore Time=2026-08-21 05:35:27.603017
- 新解析zh Message（前300字）：无法找到来自源 Microsoft-Windows-System-Restore 的事件 ID 8300 的描述。 本地计算机上未安装引发此事件的组件，或者安装已损坏。可以安装或修复本地计算机上的组件。 如果该事件产生于另一台计算机，则必须在该事件中保存显示信息。 以下是包含在事件中的信息: \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy2 消息资源存在，但在消息表中找不到该消息。

还有 1495 条差异未展示，见 report.diff。
