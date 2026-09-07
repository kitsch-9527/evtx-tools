# 不匹配项分组样本（每组一条）

- 生成时间：2026-09-04 17:42:25
- 口径：report.diff 逐条分组，键 =（分类首字母，通道，Provider，EventID），共 131 组 3097 条
- 基线口径：compared=32801 matched=29704 diff=3097（report.json 2026-09-04 16:46:54）
- 用法：同组只看一条样本即可；修完一类后重跑 06-compare，用组条数验证是否归零

## 分组总览

| # | 类 | 通道 | Provider | EID | 条数 | 疑因 | 状态 |
|---|---|---|---|---|---|---|---|
| 1 | B | Application | AsusUpdateCheck | 0 | 4 | 裸参数未渲染 | 待修 |
| 2 | C | Application | WMIRegistrationService | 0 | 3 | 无模板fallback | 待修 |
| 3 | C | Application | IntelDalJhi | 0 | 3 | 无模板fallback | 待修 |
| 4 | C | Application | WMIRegistrationService | 2 | 3 | 无模板fallback | 待修 |
| 5 | B | Application | RtkAudioUniversalService | 0 | 4 | 裸参数未渲染 | 待修 |
| 6 | B | Application | igfxCUIService2.0.0.0 | 0 | 9 | 裸参数未渲染 | 待修 |
| 7 | C | Application | Microsoft-Windows-EventSystem | 4625 | 1 | 无模板fallback | 待修 |
| 8 | D | Application | Microsoft-Windows-Security-SPP | 1004 | 1 | 真渲染分歧待查 | 待修 |
| 9 | D | Application | Microsoft-Windows-Security-SPP | 1003 | 51 | 真渲染分歧待查 | 待修 |
| 10 | B | Application | asus | 0 | 16 | 裸参数未渲染 | 待修 |
| 11 | C | Application | SecurityCenter | 1 | 2 | 无模板fallback | 待修 |
| 12 | B | Application | SecurityCenter | 15 | 52 | 裸参数未渲染 | 待修 |
| 13 | D | Application | Windows Error Reporting | 1001 | 165 | 真渲染分歧待查 | 待修 |
| 14 | D | Application | Microsoft-Windows-CertificateServicesClient-CertEnroll | 87 | 2 | 疑似compare-?清洗artefact | 待修 |
| 15 | B | Application | CoworkVMService | 1 | 2 | 裸参数未渲染 | 待修 |
| 16 | D | Application | Microsoft-Windows-AppXDeploymentServer/Operational | 2562 | 86 | 真渲染分歧待查 | 待修 |
| 17 | D | Application | Microsoft-Windows-Security-SPP | 16388 | 324 | 阶梯选错模板（真分歧） | 待修 |
| 18 | D | Application | Microsoft-Windows-Security-SPP | 8230 | 324 | 疑似compare-分号归一化artefact | 待修 |
| 19 | D | Application | Microsoft-Windows-Security-SPP | 8198 | 24 | 疑似compare-分号归一化artefact | 待修 |
| 20 | B | Application | System Restore | 8196 | 1 | 裸参数未渲染 | 待修 |
| 21 | B | Application | VSS | 8231 | 12 | 裸参数未渲染 | 待修 |
| 22 | B | Application | System Restore | 8194 | 9 | 裸参数未渲染 | 待修 |
| 23 | D | Application | Microsoft-Windows-System-Restore | 8300 | 10 | 疑似compare-?清洗artefact | 待修 |
| 24 | D | Application | Microsoft-Windows-System-Restore | 8301 | 10 | 疑似compare-?清洗artefact | 待修 |
| 25 | D | Application | Microsoft-Windows-System-Restore | 8303 | 4 | 疑似compare-?清洗artefact | 待修 |
| 26 | D | Application | Microsoft-Windows-System-Restore | 8302 | 6 | 疑似compare-?清洗artefact | 待修 |
| 27 | D | Application | VSS | 8224 | 76 | 真渲染分歧待查 | 待修 |
| 28 | C | Application | Microsoft-Windows-Search | 1003 | 1 | 无模板fallback | 待修 |
| 29 | B | Application | Chrome | 256 | 3 | 裸参数未渲染 | 待修 |
| 30 | B | Application | Chromium | 256 | 1 | 裸参数未渲染 | 待修 |
| 31 | B | Application | System Restore | 8216 | 66 | 裸参数未渲染 | 待修 |
| 32 | B | Application | EdgeWebView | 256 | 11 | 裸参数未渲染 | 待修 |
| 33 | B | Application | WXWorkUpgrader | 0 | 27 | 裸参数未渲染 | 待修 |
| 34 | B | Application | MsiInstaller | 1040 | 54 | 裸参数未渲染 | 待修 |
| 35 | B | Application | MsiInstaller | 1042 | 54 | 裸参数未渲染 | 待修 |
| 36 | B | Application | MsiInstaller | 11707 | 20 | 裸参数未渲染 | 待修 |
| 37 | D | Application | MsiInstaller | 1033 | 55 | 真渲染分歧待查 | 待修 |
| 38 | B | Application | ESENT | 102 | 3 | 裸参数未渲染 | 待修 |
| 39 | D | Application | ESENT | 105 | 3 | 真渲染分歧待查 | 待修 |
| 40 | D | Application | ESENT | 330 | 7 | 真渲染分歧待查 | 待修 |
| 41 | D | Application | ESENT | 325 | 1 | 真渲染分歧待查 | 待修 |
| 42 | D | Application | VSS | 8231 | 1 | 真渲染分歧待查 | 待修 |
| 43 | D | Application | System Restore | 8194 | 1 | 真渲染分歧待查 | 待修 |
| 44 | D | Application | MsiInstaller | 11707 | 34 | 真渲染分歧待查 | 待修 |
| 45 | D | Application | Application Error | 1000 | 8 | 真渲染分歧待查 | 待修 |
| 46 | B | Application | PDFShellExt | 0 | 9 | 裸参数未渲染 | 待修 |
| 47 | D | Application | VBScriptDeprecationAlert | 4096 | 1 | 真渲染分歧待查 | 待修 |
| 48 | D | Application | MsiInstaller | 11500 | 5 | 真渲染分歧待查 | 待修 |
| 49 | B | Application | MsiInstaller | 11708 | 1 | 裸参数未渲染 | 待修 |
| 50 | B | Application | MsiInstaller | 11728 | 2 | 裸参数未渲染 | 待修 |
| 51 | D | Application | MsiInstaller | 1035 | 2 | 真渲染分歧待查 | 待修 |
| 52 | D | Application | System Restore | 8216 | 2 | 真渲染分歧待查 | 待修 |
| 53 | D | Application | MsiInstaller | 1040 | 1 | 真渲染分歧待查 | 待修 |
| 54 | D | Application | MsiInstaller | 1042 | 1 | 真渲染分歧待查 | 待修 |
| 55 | B | Application | Microsoft-Windows-CAPI2 | 4108 | 1 | 裸参数未渲染 | 待修 |
| 56 | B | Application | Microsoft-Windows-Defrag | 258 | 8 | 裸参数未渲染 | 待修 |
| 57 | B | Application | Dwminit | 0 | 1 | 裸参数未渲染 | 待修 |
| 58 | B | Application | Microsoft-Windows-CAPI2 | 4109 | 3 | 裸参数未渲染 | 待修 |
| 59 | B | Application | Microsoft-Windows-CAPI2 | 4111 | 1 | 裸参数未渲染 | 待修 |
| 60 | D | Application | VSS | 12289 | 1 | 真渲染分歧待查 | 待修 |
| 61 | B | Application | System Restore | 8193 | 2 | 裸参数未渲染 | 待修 |
| 62 | B | Application | System Restore | 8211 | 2 | 裸参数未渲染 | 待修 |
| 63 | B | Application | Everything | 0 | 1 | 裸参数未渲染 | 待修 |
| 64 | B | Application | ESENT | 300 | 2 | 裸参数未渲染 | 待修 |
| 65 | D | Application | ESENT | 301 | 2 | 真渲染分歧待查 | 待修 |
| 66 | B | Application | ESENT | 302 | 2 | 裸参数未渲染 | 待修 |
| 67 | D | Application | ESENT | 326 | 2 | 真渲染分歧待查 | 待修 |
| 68 | B | System | EventLog | 6009 | 3 | 裸参数未渲染 | 待修 |
| 69 | B | System | EventLog | 6005 | 3 | 裸参数未渲染 | 待修 |
| 70 | D | System | Microsoft-Windows-Kernel-General | 25 | 3 | 真渲染分歧待查 | 待修 |
| 71 | D | System | Microsoft-Windows-Kernel-Boot | 153 | 3 | 真渲染分歧待查 | 待修 |
| 72 | D | System | Microsoft-Windows-Kernel-Boot | 20 | 3 | 真渲染分歧待查 | 待修 |
| 73 | D | System | Microsoft-Windows-Kernel-Boot | 25 | 3 | 真渲染分歧待查 | 待修 |
| 74 | D | System | Microsoft-Windows-Kernel-Boot | 27 | 3 | 真渲染分歧待查 | 待修 |
| 75 | D | System | Microsoft-Windows-Kernel-General | 20 | 3 | 真渲染分歧待查 | 待修 |
| 76 | D | System | Microsoft-Windows-Ntfs | 98 | 28 | 真渲染分歧待查 | 待修 |
| 77 | D | System | Microsoft-Windows-Kernel-Power | 172 | 3 | 真渲染分歧待查 | 待修 |
| 78 | D | System | Microsoft-Windows-Kernel-Processor-Power | 55 | 36 | 真渲染分歧待查 | 待修 |
| 79 | B | System | MEIx64 | 2 | 3 | 裸参数未渲染 | 待修 |
| 80 | D | System | Microsoft-Windows-Wininit | 19 | 3 | 真渲染分歧待查 | 待修 |
| 81 | D | System | Microsoft-Windows-Wininit | 25 | 3 | 真渲染分歧待查 | 待修 |
| 82 | D | System | Microsoft-Windows-Wininit | 23 | 3 | 真渲染分歧待查 | 待修 |
| 83 | D | System | Microsoft-Windows-Wininit | 14 | 3 | 真渲染分歧待查 | 待修 |
| 84 | D | System | Microsoft-Windows-Wininit | 15 | 3 | 真渲染分歧待查 | 待修 |
| 85 | D | System | LsaSrv | 6156 | 3 | 真渲染分歧待查 | 待修 |
| 86 | D | System | Microsoft-Windows-Directory-Services-SAM | 16962 | 3 | 真渲染分歧待查 | 待修 |
| 87 | D | System | Microsoft-Windows-Directory-Services-SAM | 16977 | 3 | 疑似compare-?清洗artefact | 待修 |
| 88 | D | System | Microsoft-Windows-Directory-Services-SAM | 16983 | 3 | 疑似compare-?清洗artefact | 待修 |
| 89 | D | System | Service Control Manager | 7045 | 53 | 疑似compare-尾冒号artefact | 待修 |
| 90 | D | System | Service Control Manager | 7000 | 4 | 真渲染分歧待查 | 待修 |
| 91 | D | System | Microsoft-Windows-Kernel-General | 16 | 275 | 疑似compare-?清洗artefact | 待修 |
| 92 | D | System | Microsoft-Windows-HttpService | 112 | 51 | 真渲染分歧待查 | 待修 |
| 93 | D | System | Service Control Manager | 7023 | 6 | 真渲染分歧待查 | 待修 |
| 94 | B | System | VMnetAdapter | 36 | 3 | 裸参数未渲染 | 待修 |
| 95 | B | System | e1dexpress | 32 | 3 | 裸参数未渲染 | 待修 |
| 96 | D | System | User32 | 1074 | 1 | 疑似compare-尾冒号artefact | 待修 |
| 97 | B | System | EventLog | 6006 | 1 | 裸参数未渲染 | 待修 |
| 98 | D | System | EventLog | 6013 | 16 | 真渲染分歧待查 | 待修 |
| 99 | D | System | Microsoft-Windows-Kernel-Power | 109 | 1 | 真渲染分歧待查 | 待修 |
| 100 | D | System | Microsoft-Windows-Kernel-Power | 577 | 1 | 真渲染分歧待查 | 待修 |
| 101 | D | System | Microsoft-Windows-Kernel-PnP | 219 | 8 | 真渲染分歧待查 | 待修 |
| 102 | B | System | VMnetuserif | 1 | 2 | 裸参数未渲染 | 待修 |
| 103 | B | System | vmx86 | 34 | 2 | 裸参数未渲染 | 待修 |
| 104 | B | System | hcmon | 0 | 89 | 裸参数未渲染 | 待修 |
| 105 | D | System | Microsoft-Windows-Kernel-Power | 566 | 190 | 真渲染分歧待查 | 待修 |
| 106 | D | System | Microsoft-Windows-TPM-WMI | 1808 | 2 | 真渲染分歧待查 | 待修 |
| 107 | B | System | Win32k | 267 | 14 | 裸参数未渲染 | 待修 |
| 108 | B | System | EventLog | 6008 | 1 | 裸参数未渲染 | 待修 |
| 109 | D | System | Microsoft-Windows-Kernel-Boot | 18 | 1 | 真渲染分歧待查 | 待修 |
| 110 | D | System | Microsoft-Windows-WindowsUpdateClient | 20 | 22 | 真渲染分歧待查 | 待修 |
| 111 | D | System | Microsoft-Windows-WPDClassInstaller | 24577 | 2 | 真渲染分歧待查 | 待修 |
| 112 | D | System | Microsoft-Windows-WPDClassInstaller | 24579 | 2 | 真渲染分歧待查 | 待修 |
| 113 | D | System | Microsoft-Windows-DriverFrameworks-UserMode | 10000 | 2 | 疑似compare-?清洗artefact | 待修 |
| 114 | D | System | Microsoft-Windows-DriverFrameworks-UserMode | 10001 | 1 | 真渲染分歧待查 | 待修 |
| 115 | D | System | Microsoft-Windows-UserPnp | 20003 | 2 | 疑似compare-?清洗artefact | 待修 |
| 116 | C | System | Microsoft-Windows-Configuration-Change-Monitor | 1 | 40 | 无模板fallback | 待修 |
| 117 | D | System | Microsoft-Windows-Time-Service | 35 | 7 | 真渲染分歧待查 | 待修 |
| 118 | D | System | Microsoft-Windows-Kernel-General | 1 | 26 | 真渲染分歧待查 | 待修 |
| 119 | B | System | Virtual Disk Service | 3 | 1 | 裸参数未渲染 | 待修 |
| 120 | B | System | Virtual Disk Service | 4 | 1 | 裸参数未渲染 | 待修 |
| 121 | B | System | NetBT | 4311 | 4 | 裸参数未渲染 | 待修 |
| 122 | D | System | Microsoft-Windows-Kernel-General | 11 | 9 | 真渲染分歧待查 | 待修 |
| 123 | D | System | Microsoft-Windows-DriverFrameworks-UserMode | 10002 | 1 | 真渲染分歧待查 | 待修 |
| 124 | D | System | disk | 51 | 1 | 真渲染分歧待查 | 待修 |
| 125 | D | System | Microsoft-Windows-Kernel-General | 15 | 2 | 疑似compare-?清洗artefact | 待修 |
| 126 | D | System | LsaSrv | 6041 | 1 | 疑似compare-?清洗artefact | 待修 |
| 127 | B | System | disk | 157 | 1 | 裸参数未渲染 | 待修 |
| 128 | D | System | Microsoft-Windows-Ntfs | 140 | 1 | 真渲染分歧待查 | 待修 |
| 129 | B | System | Ntfs | 50 | 515 | 裸参数未渲染 | 待修 |
| 130 | D | System | Microsoft-Windows-Wininit | 26 | 1 | 真渲染分歧待查 | 待修 |
| 131 | D | System | Service Control Manager | 7038 | 4 | 真渲染分歧待查 | 待修 |

## 分组样本（每组一条）

### 1. [B] Application / AsusUpdateCheck / EID=0（共 4 条，裸参数未渲染）

首条索引：Application #93

[Application #93] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=AsusUpdateCheck
    Date=2026-08-21T05:03:37.6210000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=AsusUpdateCheck
    Time=2026-08-21 05:03:37.621425
    Msg=AsusUpdateCheck;;CServiceControl in OnStart;;

### 2. [C] Application / WMIRegistrationService / EID=0（共 3 条，无模板fallback）

首条索引：Application #94

[Application #94] C-无模板fallback（见明细）
  baseline: EventID=0 Provider=WMIRegistrationService
    Date=2026-08-21T05:03:44.6560000Z
    Msg=Intel(R) WMI Registration Service started.
  new-en:   EventID=0 Provider=WMIRegistrationService
    Time=2026-08-21 05:03:44.656955
    Msg=The description for Event ID 0 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: The message resource is present but the message is not found in the string/message tabl

### 3. [C] Application / IntelDalJhi / EID=0（共 3 条，无模板fallback）

首条索引：Application #95

[Application #95] C-无模板fallback（见明细）
  baseline: EventID=0 Provider=IntelDalJhi
    Date=2026-08-21T05:03:44.9600000Z
    Msg=Intel(R) Dynamic Application Loader Host Interface Service started.
  new-en:   EventID=0 Provider=IntelDalJhi
    Time=2026-08-21 05:03:44.960088
    Msg=The description for Event ID 0 from source IntelDalJhi cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: The message resource is present but the message is not found in the string/message table.

### 4. [C] Application / WMIRegistrationService / EID=2（共 3 条，无模板fallback）

首条索引：Application #96

[Application #96] C-无模板fallback（见明细）
  baseline: EventID=2 Provider=WMIRegistrationService
    Date=2026-08-21T05:03:45.0760000Z
    Msg=Intel(R) WMI Registration Service has successfully finished WMI Registration.
  new-en:   EventID=2 Provider=WMIRegistrationService
    Time=2026-08-21 05:03:45.076713
    Msg=The description for Event ID 2 from source WMIRegistrationService cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: The message resource is present but the message is not found in the string/message tabl

### 5. [B] Application / RtkAudioUniversalService / EID=0（共 4 条，裸参数未渲染）

首条索引：Application #97

[Application #97] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=RtkAudioUniversalService
    Date=2026-08-21T05:03:45.4400000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=RtkAudioUniversalService
    Time=2026-08-21 05:03:45.440834
    Msg=RtkAudioUniversalService;;CppWindowsService in OnStart;;

### 6. [B] Application / igfxCUIService2.0.0.0 / EID=0（共 9 条，裸参数未渲染）

首条索引：Application #98

[Application #98] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=igfxCUIService2.0.0.0
    Date=2026-08-21T05:03:45.7530000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=igfxCUIService2.0.0.0
    Time=2026-08-21 05:03:45.753975
    Msg=Service started/resumed;;

### 7. [C] Application / Microsoft-Windows-EventSystem / EID=4625（共 1 条，无模板fallback）

首条索引：Application #110

[Application #110] C-无模板fallback（见明细）
  baseline: EventID=4625 Provider=Microsoft-Windows-EventSystem
    Date=2026-08-21T05:06:12.6980000Z
    Msg=The EventSystem sub system is suppressing duplicate event log entries for a duration of 86400 seconds. The suppression timeout can be controlled by a REG_DWORD value named SuppressDuplicateDuration under the following registry key: HKLM\Software\Microsoft\EventSystem\EventLog.
  new-en:   EventID=4625 Provider=Microsoft-Windows-EventSystem
    Time=2026-08-21 05:06:12.698028
    Msg=The description for Event ID 4625 from source Microsoft-Windows-EventSystem cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: 86400 SuppressDuplicateDuration Software\Microsoft\EventSystem\EventLog The m

### 8. [D] Application / Microsoft-Windows-Security-SPP / EID=1004（共 1 条，真渲染分歧待查）

首条索引：Application #224

[Application #224] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1004 Provider=Microsoft-Windows-Security-SPP
    Date=2026-08-21T05:07:11.6530000Z
    Msg=The Software Protection service has successfully installed the license. License Title=Office 19 Publishing License (Public) License Id=cc94d1c3-85e3-4b36-986e-0d3935872741
  new-en:   EventID=1004 Provider=Microsoft-Windows-Security-SPP
    Time=2026-08-21 05:07:11.653417
    Msg=The Software Protection service has successfully installed the license. License Title=Office 19 Publishing License (Public) License Id=cc94d1c3-85e3-4b36-986e-0d0xea98a6e5

### 9. [D] Application / Microsoft-Windows-Security-SPP / EID=1003（共 51 条，真渲染分歧待查）

首条索引：Application #322

[Application #322] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1003 Provider=Microsoft-Windows-Security-SPP
    Date=2026-08-21T05:07:14.7100000Z
    Msg=The Software Protection service has completed licensing status check. Application Id=55c92734-d682-4d71-983e-d6ec3f16059f Licensing Status= 1: 01eb852c-424d-4060-94b8-c10d799d7364, 1, 0 [(0 [0xC004F014, 0, 0], [()()()()()()()()])(1 )(2 )(3 )] 2: 040fa323-92b1-4baf-97a2-5b67feaefddb, 1, 0 [(0 [0xC004F014, 0, 0], [()()()()()()()()])(1 )(2 )(3 )] 3: 0724cb7d-3437-4cb7-93cb-830375d0079d, 1, 0 [(0 [0xC004F014, 0, 0], [()()()()()()()()])(1 )(2 )(3 )] 4: 0ad2ac98-7bb9-4201-8d92-312299201369, 1, 0 [(0 [
  new-en:   EventID=1003 Provider=Microsoft-Windows-Security-SPP
    Time=2026-08-21 05:07:14.710607
    Msg=The Software Protection service has completed licensing status check. Application Id=55c92734-d682-4d71-983e-d6ec3f16059f Licensing Status= 1: 01eb852c-424d-4060-94b8-c10d799d7364, 1, 0 [(0 [0xC004F014, 0, 0], [(?)(?)(?)(?)(?)(?)(?)(?)])(1 )(2 )(3 )] 2: 040fa323-92b1-4baf-97a2-5b67feaefddb, 1, 0 [(0 [0xC004F014, 0, 0], [(?)(?)(?)(?)(?)(?)(?)(?)])(1 )(2 )(3 )] 3: 0724cb7d-3437-4cb7-93cb-830375d0079d, 1, 0 [(0 [0xC004F014, 0, 0], [(?)(?)(?)(?)(?)(?)(?)(?)])(1 )(2 )(3 )] 4: 0ad2ac98-7bb9-4201-8d92-

### 10. [B] Application / asus / EID=0（共 16 条，裸参数未渲染）

首条索引：Application #323

[Application #323] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=asus
    Date=2026-08-21T05:08:23.2070000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=asus
    Time=2026-08-21 05:08:23.207814
    Msg=Service stopped;;

### 11. [C] Application / SecurityCenter / EID=1（共 2 条，无模板fallback）

首条索引：Application #326

[Application #326] C-无模板fallback（见明细）
  baseline: EventID=1 Provider=SecurityCenter
    Date=2026-08-21T05:08:59.4210000Z
    Msg=The Windows Security Center Service has started.
  new-en:   EventID=1 Provider=SecurityCenter
    Time=2026-08-21 05:08:59.421375
    Msg=The description for Event ID 1 from source SecurityCenter cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: The message resource is present but the message is not found in the string/message table.

### 12. [B] Application / SecurityCenter / EID=15（共 52 条，裸参数未渲染）

首条索引：Application #341

[Application #341] B-裸参数未渲染（见明细）
  baseline: EventID=15 Provider=SecurityCenter
    Date=2026-08-21T05:09:10.4110000Z
    Msg=Updated Windows Defender status successfully to SECURITY_PRODUCT_STATE_ON.
  new-en:   EventID=15 Provider=SecurityCenter
    Time=2026-08-21 05:09:10.411963
    Msg=Windows Defender;;SECURITY_PRODUCT_STATE_ON;;

### 13. [D] Application / Windows Error Reporting / EID=1001（共 165 条，真渲染分歧待查）

首条索引：Application #342

[Application #342] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1001 Provider=Windows Error Reporting
    Date=2026-08-21T05:09:10.5530000Z
    Msg=Fault bucket , type 0 Event Name: WinSetupDiag02 Response: 不可用 Cab Id: 0 Problem signature: P1: 1.6 P2: 4 P3: 9 P4: 0 P5: 0x0 P6: 0x0 P7: 26220 P8: ge_release P9: 26100 P10: ge_release_flt Attached files: \\\C:\Windows\Panther\setupact.log \\\C:\Windows\Panther\diagerr.xml \\\C:\Windows\Panther\WinSetupMon.log \\\C:\Windows\INF\setupapi.setup.log \\\C:\Windows\INF\setupapi.dev.log \\\C:\Windows\INF\setupapi.offline.log \\\C:\ProgramData\Microsoft\Windows\WER\Temp\WER.9d20e26a-80dd-4a61-994f-c4e3
  new-en:   EventID=1001 Provider=Windows Error Reporting
    Time=2026-08-21 05:09:10.553301
    Msg=Fault bucket , type 0 Event Name: WinSetupDiag02 Response: 不可用 Cab Id: 0 Problem signature: P1: 1.6 P2: 4 P3: 9 P4: 0 P5: 0x0 P6: 0x0 P7: 26220 P8: ge_release P9: 26100 P10: ge_release_flt Attached files: \\?\C:\Windows\Panther\setupact.log \\?\C:\Windows\Panther\diagerr.xml \\?\C:\Windows\Panther\WinSetupMon.log \\?\C:\Windows\INF\setupapi.setup.log \\?\C:\Windows\INF\setupapi.dev.log \\?\C:\Windows\INF\setupapi.offline.log \\?\C:\ProgramData\Microsoft\Windows\WER\Temp\WER.9d20e26a-80dd-4a61-99

### 14. [D] Application / Microsoft-Windows-CertificateServicesClient-CertEnroll / EID=87（共 2 条，疑似compare-?清洗artefact）

首条索引：Application #344

[Application #344] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=87 Provider=Microsoft-Windows-CertificateServicesClient-CertEnroll
    Date=2026-08-21T05:09:13.0340000Z
    Msg=SCEP Certificate enrollment for 本地系统 via https://INTC-KeyId-786a375f4d54b3145451d4be93760f742424fe78-v2.microsoftaik.azure.net/templates/Aik/scep failed: PkiStatus(11): SCEPDispositionPendingChallenge EnrollStatus(32): EnrollUnknown 操作成功完成。 0x0 (WIN32: 0) SubmitDone SubmitV2Attestation: Bad Request {"Message":"V2 Protocol AIK certificate requests with P-256 ECC public keys are not supported. Public key algorithm: 1.2.840.10045.2.1, Key length: 256."} HTTP/1.1 400 Bad Request Date: Thu, 20 Aug 20
  new-en:   EventID=87 Provider=Microsoft-Windows-CertificateServicesClient-CertEnroll
    Time=2026-08-21 05:09:13.034410
    Msg=SCEP Certificate enrollment for 本地系统 via https://INTC-KeyId-786a375f4d54b3145451d4be93760f742424fe78-v2.microsoftaik.azure.net/templates/Aik/scep failed: PkiStatus(11): SCEPDispositionPendingChallenge EnrollStatus(32): EnrollUnknown 操作成功完成。 0x0 (WIN32: 0) SubmitDone SubmitV2Attestation: Bad Request {"Message":"V2 Protocol AIK certificate requests with P-256 ECC public keys are not supported. Public key algorithm: 1.2.840.10045.2.1, Key length: 256."} HTTP/1.1 400 Bad Request Date: Thu, 20 Aug 20

### 15. [B] Application / CoworkVMService / EID=1（共 2 条，裸参数未渲染）

首条索引：Application #355

[Application #355] B-裸参数未渲染（见明细）
  baseline: EventID=1 Provider=CoworkVMService
    Date=2026-08-21T05:09:36.4070000Z
    Msg=Claude VM Service starting. Detailed logs: C:\ProgramData\Claude\Logs\cowork-service.log
  new-en:   EventID=1 Provider=CoworkVMService
    Time=2026-08-21 05:09:36.407739
    Msg=Claude VM Service starting. Detailed logs: C:\ProgramData\Claude\Logs\cowork-service.log;;

### 16. [D] Application / Microsoft-Windows-AppXDeploymentServer/Operational / EID=2562（共 86 条，真渲染分歧待查）

首条索引：Application #357

[Application #357] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=2562 Provider=Microsoft-Windows-AppXDeploymentServer/Operational
    Date=2026-08-21T05:09:39.1690000Z
    Msg=N/A
  new-en:   EventID=2562 Provider=Microsoft-Windows-AppXDeploymentServer/Operational
    Time=2026-08-21 05:09:39.169589
    Msg=MSIXDeployment;;windows.applicationData;;DeleteMachineFolder;;Package Microsoft.BingSearch_1.1.40.0_x64__8wekyb3d8bbwe removed machine folder C:\ProgramData\Microsoft\Windows\AppRepository\Families\ApplicationData\Microsoft.BingSearch_8wekyb3d8bbwe: 系统找不到指定的路径。;;

### 17. [D] Application / Microsoft-Windows-Security-SPP / EID=16388（共 324 条，阶梯选错模板（真分歧））

首条索引：Application #365

[Application #365] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=16388 Provider=Microsoft-Windows-Security-SPP
    Date=2026-08-21T05:12:42.3090000Z
    Msg=The current time is 2026-08-20T21:12:42Z : 2026-08-20T21:12:42Z.
  new-en:   EventID=16388 Provider=Microsoft-Windows-Security-SPP
    Time=2026-08-21 05:12:42.309783
    Msg=Authorized upgrade for PID: 2026-08-20T21:12:42Z.

### 18. [D] Application / Microsoft-Windows-Security-SPP / EID=8230（共 324 条，疑似compare-分号归一化artefact）

首条索引：Application #367

[Application #367] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8230 Provider=Microsoft-Windows-Security-SPP
    Date=2026-08-21T05:12:43.7090000Z
    Msg=The rules engine successfully re-evaluated the schedule. Kernel policies: Security-SPP-Action-StateData (REG_SZ) =AppId=55c92734-d682-4d71-983e-d6ec3f16059f KernelTimebombDate=2027/10/31:23:43:26 LastConsumptionReason=0xc004f056 LastNotificationId=VolumeUnlicensed LicenseState=SL_LICENSING_STATUS_NOTIFICATION PartialProductKey=VCFB2 ProductKeyType=Volume:GVLK SkuId=e0c42288-980c-4788-a014-c080d2e1926e ruleId=eeba1977-569e-4571-b639-7623d8bfecc0 uxDifferentiator=ENVIRONMENT volumeActivationOrder=
  new-en:   EventID=8230 Provider=Microsoft-Windows-Security-SPP
    Time=2026-08-21 05:12:43.709768
    Msg=The rules engine successfully re-evaluated the schedule. Kernel policies: Security-SPP-Action-StateData (REG_SZ) =AppId=55c92734-d682-4d71-983e-d6ec3f16059f;KernelTimebombDate=2027/10/31:23:43:26;LastConsumptionReason=0xc004f056;LastNotificationId=VolumeUnlicensed;LicenseState=SL_LICENSING_STATUS_NOTIFICATION;PartialProductKey=VCFB2;ProductKeyType=Volume:GVLK;SkuId=e0c42288-980c-4788-a014-c080d2e1926e;ruleId=eeba1977-569e-4571-b639-7623d8bfecc0;uxDifferentiator=ENVIRONMENT;volumeActivationOrder=

### 19. [D] Application / Microsoft-Windows-Security-SPP / EID=8198（共 24 条，疑似compare-分号归一化artefact）

首条索引：Application #373

[Application #373] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8198 Provider=Microsoft-Windows-Security-SPP
    Date=2026-08-21T05:12:44.8950000Z
    Msg=License Activation (slui.exe) failed with the following error code: hr=0x8007007B Command-line arguments: RuleId=eeba1977-569e-4571-b639-7623d8bfecc0 Action=AutoActivate AppId=55c92734-d682-4d71-983e-d6ec3f16059f SkuId=e0c42288-980c-4788-a014-c080d2e1926e NotificationInterval=1440 Trigger=TimerEvent
  new-en:   EventID=8198 Provider=Microsoft-Windows-Security-SPP
    Time=2026-08-21 05:12:44.895969
    Msg=License Activation (slui.exe) failed with the following error code: hr=0x8007007B Command-line arguments: RuleId=eeba1977-569e-4571-b639-7623d8bfecc0;Action=AutoActivate;AppId=55c92734-d682-4d71-983e-d6ec3f16059f;SkuId=e0c42288-980c-4788-a014-c080d2e1926e;NotificationInterval=1440;Trigger=TimerEvent

### 20. [B] Application / System Restore / EID=8196（共 1 条，裸参数未渲染）

首条索引：Application #399

[Application #399] B-裸参数未渲染（见明细）
  baseline: EventID=8196 Provider=System Restore
    Date=2026-08-21T05:34:52.3510000Z
    Msg=System Restore has been enabled (Process = C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv Volume = \\\Volume{6b025cbd-1f6f-4816-bbf5-95cb382894c0}\).
  new-en:   EventID=8196 Provider=System Restore
    Time=2026-08-21 05:34:52.351772
    Msg=C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv;;\\?\Volume{6b025cbd-1f6f-4816-bbf5-95cb382894c0}\;;;00000000A7000000A1000000000000002D2C192F07000000000000000000000000000000;;

### 21. [B] Application / VSS / EID=8231（共 12 条，裸参数未渲染）

首条索引：Application #400

[Application #400] B-裸参数未渲染（见明细）
  baseline: EventID=8231 Provider=VSS
    Date=2026-08-21T05:35:03.0120000Z
    Msg=Volume Shadow Copy Service information: Snapshot creation initiated for Snapshot Set {c91319e4-54d7-45f6-9c76-ac3f97860d11}. Process command line: C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv Process Caller Stack: Process[0]: svchost.exe Process[1]: services.exe Process[2]: wininit.exe
  new-en:   EventID=8231 Provider=VSS
    Time=2026-08-21 05:35:03.012048
    Msg={c91319e4-54d7-45f6-9c76-ac3f97860d11};;C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv;;svchost.exe;;services.exe;;wininit.exe;;

### 22. [B] Application / System Restore / EID=8194（共 9 条，裸参数未渲染）

首条索引：Application #401

[Application #401] B-裸参数未渲染（见明细）
  baseline: EventID=8194 Provider=System Restore
    Date=2026-08-21T05:35:06.9930000Z
    Msg=Successfully created restore point (Process = C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv Description = Windows 更新).
  new-en:   EventID=8194 Provider=System Restore
    Time=2026-08-21 05:35:06.993529
    Msg=C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv;;Windows 更新;;;0000000067020000590200000000000022CE28677C6DDA79E28C1C000000000000000000;;

### 23. [D] Application / Microsoft-Windows-System-Restore / EID=8300（共 10 条，疑似compare-?清洗artefact）

首条索引：Application #410

[Application #410] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8300 Provider=Microsoft-Windows-System-Restore
    Date=2026-08-21T05:35:17.9580000Z
    Msg=Scoping started for shadowcopy \\\GLOBALROOT\Device\HarddiskVolumeShadowCopy1.
  new-en:   EventID=8300 Provider=Microsoft-Windows-System-Restore
    Time=2026-08-21 05:35:17.958668
    Msg=Scoping started for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1.

### 24. [D] Application / Microsoft-Windows-System-Restore / EID=8301（共 10 条，疑似compare-?清洗artefact）

首条索引：Application #412

[Application #412] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8301 Provider=Microsoft-Windows-System-Restore
    Date=2026-08-21T05:36:05.5070000Z
    Msg=Scoping completed for shadowcopy \\\GLOBALROOT\Device\HarddiskVolumeShadowCopy1.
  new-en:   EventID=8301 Provider=Microsoft-Windows-System-Restore
    Time=2026-08-21 05:36:05.507321
    Msg=Scoping completed for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1.

### 25. [D] Application / Microsoft-Windows-System-Restore / EID=8303（共 4 条，疑似compare-?清洗artefact）

首条索引：Application #413

[Application #413] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8303 Provider=Microsoft-Windows-System-Restore
    Date=2026-08-21T05:36:05.5070000Z
    Msg=Scoping unsuccessful for shadowcopy \\\GLOBALROOT\Device\HarddiskVolumeShadowCopy1 with error 0x80070057.
  new-en:   EventID=8303 Provider=Microsoft-Windows-System-Restore
    Time=2026-08-21 05:36:05.507326
    Msg=Scoping unsuccessful for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1 with error 0x80070057.

### 26. [D] Application / Microsoft-Windows-System-Restore / EID=8302（共 6 条，疑似compare-?清洗artefact）

首条索引：Application #415

[Application #415] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8302 Provider=Microsoft-Windows-System-Restore
    Date=2026-08-21T05:36:13.4870000Z
    Msg=Scoping successfully completed for shadowcopy \\\GLOBALROOT\Device\HarddiskVolumeShadowCopy2.
  new-en:   EventID=8302 Provider=Microsoft-Windows-System-Restore
    Time=2026-08-21 05:36:13.487150
    Msg=Scoping successfully completed for shadowcopy \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy2.

### 27. [D] Application / VSS / EID=8224（共 76 条，真渲染分歧待查）

首条索引：Application #418

[Application #418] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8224 Provider=VSS
    Date=2026-08-21T05:38:27.5120000Z
    Msg=The VSS service is shutting down due to idle timeout.
  new-en:   EventID=8224 Provider=VSS
    Time=2026-08-21 05:38:27.512406
    Msg=;2D20436F64653A2020434F525356434330303030303737322D2043616C6C3A2020434F525356434330303030303735342D205049443A202030303031363037322D205449443A202030303031343639362D20434D443A2020433A5C57494E444F57535C73797374656D33325C76737376632E6578652020202D20557365723A204E616D653A204E5420415554484F524954595C53595354454D2C205349443A532D312D352D313820;;

### 28. [C] Application / Microsoft-Windows-Search / EID=1003（共 1 条，无模板fallback）

首条索引：Application #541

[Application #541] C-无模板fallback（见明细）
  baseline: EventID=1003 Provider=Microsoft-Windows-Search
    Date=2026-08-21T09:33:20.4560000Z
    Msg=The Windows Search Service started.
  new-en:   EventID=1003 Provider=Microsoft-Windows-Search
    Time=2026-08-21 09:33:20.456287
    Msg=The description for Event ID 1003 from source Microsoft-Windows-Search cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: The message resource is present but the message is not found in the string/message

### 29. [B] Application / Chrome / EID=256（共 3 条，裸参数未渲染）

首条索引：Application #547

[Application #547] B-裸参数未渲染（见明细）
  baseline: EventID=256 Provider=Chrome
    Date=2026-08-21T09:33:59.8920000Z
    Msg=[11024:16136:0821/093359.890:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.
  new-en:   EventID=256 Provider=Chrome
    Time=2026-08-21 09:33:59.892664
    Msg=[11024:16136:0821/093359.890:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.;;

### 30. [B] Application / Chromium / EID=256（共 1 条，裸参数未渲染）

首条索引：Application #548

[Application #548] B-裸参数未渲染（见明细）
  baseline: EventID=256 Provider=Chromium
    Date=2026-08-21T09:34:19.0140000Z
    Msg=N/A
  new-en:   EventID=256 Provider=Chromium
    Time=2026-08-21 09:34:19.014727
    Msg=[22200:6160:0821/09:34:19.013:INFO:extension_garbage_collector.cc(184)] Garbage collection for extensions on file thread is complete.;;

### 31. [B] Application / System Restore / EID=8216（共 66 条，裸参数未渲染）

首条索引：Application #556

[Application #556] B-裸参数未渲染（见明细）
  baseline: EventID=8216 Provider=System Restore
    Date=2026-08-21T09:34:49.2680000Z
    Msg=Skipping creation of restore point (Process = C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv Description = Windows 更新) as there is a restore point avaliable which is recent enough for System Restore.
  new-en:   EventID=8216 Provider=System Restore
    Time=2026-08-21 09:34:49.268014
    Msg=C:\WINDOWS\system32\svchost.exe -k netsvcs -p -s wuauserv;;Windows 更新;;;0000000063020000590200000000000022CE28677C6DDA79E28C1C000000000000000000;;

### 32. [B] Application / EdgeWebView / EID=256（共 11 条，裸参数未渲染）

首条索引：Application #566

[Application #566] B-裸参数未渲染（见明细）
  baseline: EventID=256 Provider=EdgeWebView
    Date=2026-08-21T09:57:04.7040000Z
    Msg=N/A
  new-en:   EventID=256 Provider=EdgeWebView
    Time=2026-08-21 09:57:04.704368
    Msg=[31600:31212:0821/095704.703:INFO:chrome\browser\extensions\extension_garbage_collector.cc:188] Garbage collection for extensions on file thread is complete.;;

### 33. [B] Application / WXWorkUpgrader / EID=0（共 27 条，裸参数未渲染）

首条索引：Application #589

[Application #589] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=WXWorkUpgrader
    Date=2026-08-21T11:59:13.0590000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=WXWorkUpgrader
    Time=2026-08-21 11:59:13.059769
    Msg=Service stopped;;

### 34. [B] Application / MsiInstaller / EID=1040（共 54 条，裸参数未渲染）

首条索引：Application #624

[Application #624] B-裸参数未渲染（见明细）
  baseline: EventID=1040 Provider=MsiInstaller
    Date=2026-08-21T16:31:50.0180000Z
    Msg=Beginning a Windows Installer transaction: F:\xiaz\dnGREP.5.0.49.x64.msi. Client Process Id: 33124.
  new-en:   EventID=1040 Provider=MsiInstaller
    Time=2026-08-21 16:31:50.018895
    Msg=F:\xiaz\dnGREP.5.0.49.x64.msi;;33124;;(NULL);;(NULL);;(NULL);;(NULL);;

### 35. [B] Application / MsiInstaller / EID=1042（共 54 条，裸参数未渲染）

首条索引：Application #627

[Application #627] B-裸参数未渲染（见明细）
  baseline: EventID=1042 Provider=MsiInstaller
    Date=2026-08-21T16:32:03.8860000Z
    Msg=Ending a Windows Installer transaction: F:\xiaz\dnGREP.5.0.49.x64.msi. Client Process Id: 33124.
  new-en:   EventID=1042 Provider=MsiInstaller
    Time=2026-08-21 16:32:03.886428
    Msg=F:\xiaz\dnGREP.5.0.49.x64.msi;;33124;;(NULL);;(NULL);;(NULL);;(NULL);;

### 36. [B] Application / MsiInstaller / EID=11707（共 20 条，裸参数未渲染）

首条索引：Application #629

[Application #629] B-裸参数未渲染（见明细）
  baseline: EventID=11707 Provider=MsiInstaller
    Date=2026-08-21T16:32:05.6010000Z
    Msg=Product: dnGrep 5.0.49 (x64) -- Installation completed successfully.
  new-en:   EventID=11707 Provider=MsiInstaller
    Time=2026-08-21 16:32:05.601613
    Msg=Product: dnGrep 5.0.49 (x64) -- Installation completed successfully.;;(NULL);;(NULL);;(NULL);;(NULL);;(NULL);;;7B44453037423631432D374230442D343642392D414541412D4537453946353443453444427D;;

### 37. [D] Application / MsiInstaller / EID=1033（共 55 条，真渲染分歧待查）

首条索引：Application #630

[Application #630] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1033 Provider=MsiInstaller
    Date=2026-08-21T16:32:05.6020000Z
    Msg=Windows Installer installed the product. Product Name: dnGrep 5.0.49 (x64). Product Version: 5.0.49.0. Product Language: 1033. Manufacturer: dnGrep Community Contributors. Installation success or error status: 0.
  new-en:   EventID=1033 Provider=MsiInstaller
    Time=2026-08-21 16:32:05.602132
    Msg=dnGrep 5.0.49 (x64);;5.0.49.0;;1033;;0;;dnGrep Community Contributors;;(NULL);;;7B44453037423631432D374230442D343642392D414541412D4537453946353443453444427D3030303036313566333861643336343634646665653465393062356435663439353264373030303030393034;;

### 38. [B] Application / ESENT / EID=102（共 3 条，裸参数未渲染）

首条索引：Application #733

[Application #733] B-裸参数未渲染（见明细）
  baseline: EventID=102 Provider=ESENT
    Date=2026-08-22T05:31:09.4690000Z
    Msg=svchost (41104,P,98,0) DS_Token_DB: The database engine (10.00.26220.0000) is starting a new instance (0).
  new-en:   EventID=102 Provider=ESENT
    Time=2026-08-22 05:31:09.469574
    Msg=svchost;;41104,P,98,0;;DS_Token_DB: ;;0;;10;;00;;26220;;0000;;

### 39. [D] Application / ESENT / EID=105（共 3 条，真渲染分歧待查）

首条索引：Application #734

[Application #734] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=105 Provider=ESENT
    Date=2026-08-22T05:31:09.5160000Z
    Msg=svchost (41104,D,0,0) DS_Token_DB: The database engine started a new instance (0). (Time=0.051 seconds) Additional Data: RBSOn = 0 Internal Timing Sequence: [1] 0.000689 +J(0) +M(C:0K, Fs:165, WS:636K # 636K, PF:3024K # 3024K, P:3024K) [2] 0.000445 +J(0) +M(C:8K, Fs:178, WS:704K # 704K, PF:1280K # 1280K, P:1280K) [3] 0.000017 +J(0) +M(C:0K, Fs:7, WS:24K # 24K, PF:68K # 68K, P:68K) [4] 0.000180 +J(0) +M(C:0K, Fs:86, WS:344K # 344K, PF:192K # 192K, P:192K) [5] 0.002020 +J(0) +M(C:0K, Fs:112, WS:44
  new-en:   EventID=105 Provider=ESENT
    Time=2026-08-22 05:31:09.516373
    Msg=svchost;;41104,D,0,0;;DS_Token_DB: ;;0;;0.051;; [1] 0.000689 +J(0) +M(C:0K, Fs:165, WS:636K # 636K, PF:3024K # 3024K, P:3024K) [2] 0.000445 +J(0) +M(C:8K, Fs:178, WS:704K # 704K, PF:1280K # 1280K, P:1280K) [3] 0.000017 +J(0) +M(C:0K, Fs:7, WS:24K # 24K, PF:68K # 68K, P:68K) [4] 0.000180 +J(0) +M(C:0K, Fs:86, WS:344K # 344K, PF:192K # 192K, P:192K) [5] 0.002020 +J(0) +M(C:0K, Fs:112, WS:448K # 448K, PF:24K # 24K, P:24K) [6] 0.002021 +J(0) +M(C:0K, Fs:49, WS:196K # 196K, PF:12K # 12K, P:12K) [7] -

### 40. [D] Application / ESENT / EID=330（共 7 条，真渲染分歧待查）

首条索引：Application #735

[Application #735] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=330 Provider=ESENT
    Date=2026-08-22T05:31:09.5940000Z
    Msg=svchost (41104,D,35,0) DS_Token_DB: The database [C:\WINDOWS\system32\config\systemprofile\AppData\Local\DataSharing\Storage\DSTokenDB2.dat] format version is being held back to 8920 (0x22d8) due to application parameter setting of 0x410022D8 (8920 | JET_efvAllowHigherPersistedFormat). Current default engine version: 9620 (0x2594).
  new-en:   EventID=330 Provider=ESENT
    Time=2026-08-22 05:31:09.594438
    Msg=svchost;;41104,D,35,0;;DS_Token_DB: ;;C:\WINDOWS\system32\config\systemprofile\AppData\Local\DataSharing\Storage\DSTokenDB2.dat;;0x410022D8 (8920 | JET_efvAllowHigherPersistedFormat);;8920 (0x22d8);;9620 (0x2594);;

### 41. [D] Application / ESENT / EID=325（共 1 条，真渲染分歧待查）

首条索引：Application #736

[Application #736] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=325 Provider=ESENT
    Date=2026-08-22T05:31:09.6620000Z
    Msg=svchost (41104,D,35,0) DS_Token_DB: The database engine created a new database (1, C:\WINDOWS\system32\config\systemprofile\AppData\Local\DataSharing\Storage\DSTokenDB2.dat). (Time=0 seconds) Additional Data: lgposCreate = 00000001:0001:0268, dbv = 1568.20.0 (8920) Internal Timing Sequence: [1] 0.000114 +J(0) +M(C:0K, Fs:1, WS:4K # 4K, PF:4K # 4K, P:4K) [2] 0.006476 -0.000114 (1) WT +J(CM:0, PgRf:0, Rd:0/0, Dy:0/0, Lg:3480/2) +M(C:0K, Fs:3, WS:12K # 12K, PF:0K # 0K, P:0K) [3] 0.022415 -0.000001 
  new-en:   EventID=325 Provider=ESENT
    Time=2026-08-22 05:31:09.662562
    Msg=svchost;;41104,D,35,0;;DS_Token_DB: ;;1;;C:\WINDOWS\system32\config\systemprofile\AppData\Local\DataSharing\Storage\DSTokenDB2.dat;;0;; [1] 0.000114 +J(0) +M(C:0K, Fs:1, WS:4K # 4K, PF:4K # 4K, P:4K) [2] 0.006476 -0.000114 (1) WT +J(CM:0, PgRf:0, Rd:0/0, Dy:0/0, Lg:3480/2) +M(C:0K, Fs:3, WS:12K # 12K, PF:0K # 0K, P:0K) [3] 0.022415 -0.000001 (1) WT +J(0) +M(C:0K, Fs:13, WS:44K # 48K, PF:24K # 28K, P:24K) [4] 0.009307 -0.005254 (1) WT +J(0) +M(C:0K, Fs:60, WS:240K # 236K, PF:88K # 84K, P:88K) [5]

### 42. [D] Application / VSS / EID=8231（共 1 条，真渲染分歧待查）

首条索引：Application #892

[Application #892] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8231 Provider=VSS
    Date=2026-08-22T22:42:13.4350000Z
    Msg=Volume Shadow Copy Service information: Snapshot creation initiated for Snapshot Set {f041ca75-8033-4bfd-bcde-c63fef451a4b}. Process command line: C:\WINDOWS\winsxs\amd64_microsoft-windows-servicingstack_31bf3856ad364e35_10.0.26100.9202_none_a536c20177411c9f\TiWorker.exe -Embedding Process Caller Stack: Process[0]: TiWorker.exe Process[1]: svchost.exe Process[2]: services.exe
  new-en:   EventID=8231 Provider=VSS
    Time=2026-08-22 22:42:13.435319
    Msg={f041ca75-8033-4bfd-bcde-c63fef451a4b};;C:\WINDOWS\winsxs\amd64_microsoft-windows-servicingstack_31bf3856ad364e35_10.0.26100.9202_none_a536c20177411c9f\TiWorker.exe -Embedding;;TiWorker.exe;;svchost.exe;;services.exe;;

### 43. [D] Application / System Restore / EID=8194（共 1 条，真渲染分歧待查）

首条索引：Application #893

[Application #893] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8194 Provider=System Restore
    Date=2026-08-22T22:42:16.7350000Z
    Msg=Successfully created restore point (Process = C:\WINDOWS\winsxs\amd64_microsoft-windows-servicingstack_31bf3856ad364e35_10.0.26100.9202_none_a536c20177411c9f\TiWorker.exe -Embedding Description = Windows 模块安装程序).
  new-en:   EventID=8194 Provider=System Restore
    Time=2026-08-22 22:42:16.735154
    Msg=C:\WINDOWS\winsxs\amd64_microsoft-windows-servicingstack_31bf3856ad364e35_10.0.26100.9202_none_a536c20177411c9f\TiWorker.exe -Embedding;;Windows 模块安装程序;;;0000000067020000590200000000000022CE28677C6DDA79E28C1C000000000000000000;;

### 44. [D] Application / MsiInstaller / EID=11707（共 34 条，真渲染分歧待查）

首条索引：Application #1403

[Application #1403] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=11707 Provider=MsiInstaller
    Date=2026-08-25T05:08:51.8210000Z
    Msg=Product: Autodesk Genuine Service -- Installation operation completed successfully.
  new-en:   EventID=11707 Provider=MsiInstaller
    Time=2026-08-25 05:08:51.821993
    Msg=Product: Autodesk Genuine Service -- Installation operation completed successfully.;;(NULL);;(NULL);;(NULL);;(NULL);;(NULL);;;7B34453545323132452D463838312D344337342D394631422D3542314144434234424237427D;;

### 45. [D] Application / Application Error / EID=1000（共 8 条，真渲染分歧待查）

首条索引：Application #1520

[Application #1520] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1000 Provider=Application Error
    Date=2026-08-25T18:18:27.3720000Z
    Msg=Faulting application name: DllHost.exe, version: 10.0.26100.8941, time stamp: 0x7b4ce33e Faulting module name: combase.dll, version: 10.0.26100.9202, time stamp: 0xf820c5c7 Exception code: 0xc0000005 Fault offset: 0x000000000019d6b8 Faulting process id: 0xAB80 Faulting application start time: 0x1DD347A3F16110C Faulting application path: C:\WINDOWS\system32\DllHost.exe Faulting module path: C:\WINDOWS\System32\combase.dll Report Id: 691d4074-0d83-4b23-a0d3-d13cdc6920ff Faulting package full name:
  new-en:   EventID=1000 Provider=Application Error
    Time=2026-08-25 18:18:27.372513
    Msg=Faulting application name: DllHost.exe, version: 10.0.26100.8941, time stamp: 0x7b4ce33e Faulting module name: combase.dll, version: 10.0.26100.9202, time stamp: 0xf820c5c7 Exception code: 0xc0000005 Fault offset: 0x000000000019d6b8 Faulting process id: 0xab80 Faulting application start time: 0x1dd347a3f16110c Faulting application path: C:\WINDOWS\system32\DllHost.exe Faulting module path: C:\WINDOWS\System32\combase.dll Report Id: 691d4074-0d83-4b23-a0d3-d13cdc6920ff Faulting package full name:

### 46. [B] Application / PDFShellExt / EID=0（共 9 条，裸参数未渲染）

首条索引：Application #1733

[Application #1733] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=PDFShellExt
    Date=2026-08-26T15:20:03.9960000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=PDFShellExt
    Time=2026-08-26 15:20:03.996640
    Msg=no office installed;;

### 47. [D] Application / VBScriptDeprecationAlert / EID=4096（共 1 条，真渲染分歧待查）

首条索引：Application #1781

[Application #1781] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=4096 Provider=VBScriptDeprecationAlert
    Date=2026-08-26T18:25:37.8850000Z
    Msg=N/A
  new-en:   EventID=4096 Provider=VBScriptDeprecationAlert
    Time=2026-08-26 18:25:37.885101
    Msg=VBScript is scheduled for deprecation. Our telemetry indicates that your system is currently utilizing VBScript. We strongly recommend identifying and migrating away from any VBScript dependencies at the earliest. The following process has been detected as using VBScript. The associated process tree and call stack are provided below to assist in identifying the scenario in which VBScript was invoked. ;;ProcessName = "msiexec.exe";;ProcessTree = "msiexec.exe;msiexec.exe;services.exe;wininit.exe";

### 48. [D] Application / MsiInstaller / EID=11500（共 5 条，真渲染分歧待查）

首条索引：Application #2020

[Application #2020] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=11500 Provider=MsiInstaller
    Date=2026-08-27T15:54:29.2820000Z
    Msg=Product: X86 Debuggers And Tools -- Error 1500. Another installation is in progress. You must complete that installation before continuing this one.
  new-en:   EventID=11500 Provider=MsiInstaller
    Time=2026-08-27 15:54:29.282874
    Msg=Product: X86 Debuggers And Tools -- Error 1500. Another installation is in progress. You must complete that installation before continuing this one.;;(NULL);;(NULL);;(NULL);;(NULL);;(NULL);;;7B36323230433836422D384642342D344144392D343036302D3430433932343043413342367D;;

### 49. [B] Application / MsiInstaller / EID=11708（共 1 条，裸参数未渲染）

首条索引：Application #2027

[Application #2027] B-裸参数未渲染（见明细）
  baseline: EventID=11708 Provider=MsiInstaller
    Date=2026-08-27T15:54:37.4620000Z
    Msg=Product: X86 Debuggers And Tools -- Installation failed.
  new-en:   EventID=11708 Provider=MsiInstaller
    Time=2026-08-27 15:54:37.462005
    Msg=Product: X86 Debuggers And Tools -- Installation failed.;;(NULL);;(NULL);;(NULL);;(NULL);;(NULL);;;7B36323230433836422D384642342D344144392D343036302D3430433932343043413342367D2C2031363032;;

### 50. [B] Application / MsiInstaller / EID=11728（共 2 条，裸参数未渲染）

首条索引：Application #2037

[Application #2037] B-裸参数未渲染（见明细）
  baseline: EventID=11728 Provider=MsiInstaller
    Date=2026-08-27T15:55:00.1040000Z
    Msg=Product: X86 Debuggers And Tools -- Configuration completed successfully.
  new-en:   EventID=11728 Provider=MsiInstaller
    Time=2026-08-27 15:55:00.104371
    Msg=Product: X86 Debuggers And Tools -- Configuration completed successfully.;;(NULL);;(NULL);;(NULL);;(NULL);;(NULL);;;7B36323230433836422D384642342D344144392D343036302D3430433932343043413342367D;;

### 51. [D] Application / MsiInstaller / EID=1035（共 2 条，真渲染分歧待查）

首条索引：Application #2038

[Application #2038] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1035 Provider=MsiInstaller
    Date=2026-08-27T15:55:00.1040000Z
    Msg=Windows Installer reconfigured the product. Product Name: X86 Debuggers And Tools. Product Version: 10.1.18362.1. Product Language: 1033. Manufacturer: Microsoft Corporation. Reconfiguration success or error status: 0.
  new-en:   EventID=1035 Provider=MsiInstaller
    Time=2026-08-27 15:55:00.104723
    Msg=X86 Debuggers And Tools;;10.1.18362.1;;1033;;0;;Microsoft Corporation;;(NULL);;;7B36323230433836422D384642342D344144392D343036302D3430433932343043413342367D3030303066363363373038643162316465666466323962656131646630663164626530363030303030393034;;

### 52. [D] Application / System Restore / EID=8216（共 2 条，真渲染分歧待查）

首条索引：Application #2051

[Application #2051] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=8216 Provider=System Restore
    Date=2026-08-27T16:00:49.0740000Z
    Msg=Skipping creation of restore point (Process = C:\WINDOWS\Temp\{F020E532-F189-4025-BB34-83530D89A9ED}\.be\winsdksetup.exe -q -burn.elevated BurnPipe.{2485C5B0-48D1-4FEC-82BD-13FB679B429D} {708B4A6D-C35F-408E-A6F2-E672DEEC77E1} 47368 Description = Windows Software Development Kit - Windows 10.0.19041.685) as there is a restore point avaliable which is recent enough for System Restore.
  new-en:   EventID=8216 Provider=System Restore
    Time=2026-08-27 16:00:49.074770
    Msg=C:\WINDOWS\Temp\{F020E532-F189-4025-BB34-83530D89A9ED}\.be\winsdksetup.exe -q -burn.elevated BurnPipe.{2485C5B0-48D1-4FEC-82BD-13FB679B429D} {708B4A6D-C35F-408E-A6F2-E672DEEC77E1} 47368;;Windows Software Development Kit - Windows 10.0.19041.685;;;0000000063020000590200000000000022CE28677C6DDA79E28C1C000000000000000000;;

### 53. [D] Application / MsiInstaller / EID=1040（共 1 条，真渲染分歧待查）

首条索引：Application #2253

[Application #2253] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1040 Provider=MsiInstaller
    Date=2026-08-27T16:04:21.6340000Z
    Msg=Beginning a Windows Installer transaction: C:\ProgramData\Package Cache\{0E2FEA3B-C853-DE2A-8A04-BB7D5BF010E0}v10.1.19041.685\Installers\Windows SDK for Windows Store Apps DirectX x86 Remote-x86_en-us.msi. Client Process Id: 15988.
  new-en:   EventID=1040 Provider=MsiInstaller
    Time=2026-08-27 16:04:21.634844
    Msg=C:\ProgramData\Package Cache\{0E2FEA3B-C853-DE2A-8A04-BB7D5BF010E0}v10.1.19041.685\Installers\Windows SDK for Windows Store Apps DirectX x86 Remote-x86_en-us.msi;;15988;;(NULL);;(NULL);;(NULL);;(NULL);;

### 54. [D] Application / MsiInstaller / EID=1042（共 1 条，真渲染分歧待查）

首条索引：Application #2258

[Application #2258] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1042 Provider=MsiInstaller
    Date=2026-08-27T16:04:24.2500000Z
    Msg=Ending a Windows Installer transaction: C:\ProgramData\Package Cache\{0E2FEA3B-C853-DE2A-8A04-BB7D5BF010E0}v10.1.19041.685\Installers\Windows SDK for Windows Store Apps DirectX x86 Remote-x86_en-us.msi. Client Process Id: 15988.
  new-en:   EventID=1042 Provider=MsiInstaller
    Time=2026-08-27 16:04:24.250299
    Msg=C:\ProgramData\Package Cache\{0E2FEA3B-C853-DE2A-8A04-BB7D5BF010E0}v10.1.19041.685\Installers\Windows SDK for Windows Store Apps DirectX x86 Remote-x86_en-us.msi;;15988;;(NULL);;(NULL);;(NULL);;(NULL);;

### 55. [B] Application / Microsoft-Windows-CAPI2 / EID=4108（共 1 条，裸参数未渲染）

首条索引：Application #2409

[Application #2409] B-裸参数未渲染（见明细）
  baseline: EventID=4108 Provider=Microsoft-Windows-CAPI2
    Date=2026-08-27T20:55:57.0300000Z
    Msg=Successful auto delete of third-party root certificate:: Subject: <OU=Class 3 Public Primary Certification Authority, O="VeriSign, Inc.", C=US> Sha1 thumbprint: <4F65566336DB6598581D584A596C87934D5F2AB4>.
  new-en:   EventID=4108 Provider=Microsoft-Windows-CAPI2
    Time=2026-08-27 20:55:57.030609
    Msg=OU=Class 3 Public Primary Certification Authority, O="VeriSign, Inc.", C=US;;4F65566336DB6598581D584A596C87934D5F2AB4;;

### 56. [B] Application / Microsoft-Windows-Defrag / EID=258（共 8 条，裸参数未渲染）

首条索引：Application #2492

[Application #2492] B-裸参数未渲染（见明细）
  baseline: EventID=258 Provider=Microsoft-Windows-Defrag
    Date=2026-08-28T05:44:17.4750000Z
    Msg=The storage optimizer successfully completed 碎片整理 on 资料 (E:)
  new-en:   EventID=258 Provider=Microsoft-Windows-Defrag
    Time=2026-08-28 05:44:17.475566
    Msg=碎片整理;;资料 (E:);;;00000000ED010000A00100000000000022B651A2296BEDD85E9C8D030000000000000000;;

### 57. [B] Application / Dwminit / EID=0（共 1 条，裸参数未渲染）

首条索引：Application #2541

[Application #2541] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=Dwminit
    Date=2026-08-28T09:27:53.9340000Z
    Msg=The Desktop Window Manager process has exited. (Process exit code: 0xc00001ad, Restart count: 1, Primary display device ID: Intel(R) UHD Graphics 630)
  new-en:   EventID=0 Provider=Dwminit
    Time=2026-08-28 09:27:53.934297
    Msg=0xc00001ad;;1;;Intel(R) UHD Graphics 630;;

### 58. [B] Application / Microsoft-Windows-CAPI2 / EID=4109（共 3 条，裸参数未渲染）

首条索引：Application #2633

[Application #2633] B-裸参数未渲染（见明细）
  baseline: EventID=4109 Provider=Microsoft-Windows-CAPI2
    Date=2026-08-28T19:46:12.1940000Z
    Msg=Successful auto property update of third-party root certificate:: Subject: <CN=Baltimore CyberTrust Root, OU=CyberTrust, O=Baltimore, C=IE> Sha1 thumbprint: <D4DE20D05E66FC53FE1A50882C78DB2852CAE474>.
  new-en:   EventID=4109 Provider=Microsoft-Windows-CAPI2
    Time=2026-08-28 19:46:12.194103
    Msg=CN=Baltimore CyberTrust Root, OU=CyberTrust, O=Baltimore, C=IE;;D4DE20D05E66FC53FE1A50882C78DB2852CAE474;;

### 59. [B] Application / Microsoft-Windows-CAPI2 / EID=4111（共 1 条，裸参数未渲染）

首条索引：Application #2636

[Application #2636] B-裸参数未渲染（见明细）
  baseline: EventID=4111 Provider=Microsoft-Windows-CAPI2
    Date=2026-08-28T19:46:12.1970000Z
    Msg=Successful auto update of third-party root list with effective date: 2026年8月25日 15:24:34.
  new-en:   EventID=4111 Provider=Microsoft-Windows-CAPI2
    Time=2026-08-28 19:46:12.197227
    Msg=‎2026‎年‎8‎月‎25‎日 15:24:34;;

### 60. [D] Application / VSS / EID=12289（共 1 条，真渲染分歧待查）

首条索引：Application #2727

[Application #2727] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=12289 Provider=VSS
    Date=2026-08-29T05:31:27.0680000Z
    Msg=Volume Shadow Copy Service error: Unexpected error DeviceIoControl(\\\GLOBALROOT\Device\HarddiskVolumeShadowCopy8 - 000000000000028C,0x00560034,000001E708509BD0,0,000001E708508BA0,4096,[0]). hr = 0x800701b1, 指定不存在的设备。 . 操作: 正在处理 PostFinalCommitSnapshots 上下文: 执行上下文: System Provider
  new-en:   EventID=12289 Provider=VSS
    Time=2026-08-29 05:31:27.068830
    Msg=DeviceIoControl(\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy8 - 000000000000028C,0x00560034,000001E708509BD0,0,000001E708508BA0,4096,[0]);;0x800701b1, 指定不存在的设备。 ;; 操作: 正在处理 PostFinalCommitSnapshots 上下文: 执行上下文: System Provider;;;2D20436F64653A20494E434943484C4830303030303532312D2043616C6C3A2053505251534E504330303030303339382D205049443A202030303032383936382D205449443A202030303034343232302D20434D443A2020433A5C57494E444F57535C53797374656D33325C737663686F73742E657865202D6B2073777072762D20557365723A

### 61. [B] Application / System Restore / EID=8193（共 2 条，裸参数未渲染）

首条索引：Application #2729

[Application #2729] B-裸参数未渲染（见明细）
  baseline: EventID=8193 Provider=System Restore
    Date=2026-08-29T05:31:50.9320000Z
    Msg=Failed to create restore point (Process = C:\WINDOWS\system32\srtasks.exe ExecuteScheduledSPPCreation Description = 计划的检查点 Error = 0x80042308).
  new-en:   EventID=8193 Provider=System Restore
    Time=2026-08-29 05:31:50.932894
    Msg=C:\WINDOWS\system32\srtasks.exe ExecuteScheduledSPPCreation;;计划的检查点;;0x80042308;;;082304805F020000490200005702000022CE28677C6DDA79E28C1C000000000000000000;;

### 62. [B] Application / System Restore / EID=8211（共 2 条，裸参数未渲染）

首条索引：Application #2730

[Application #2730] B-裸参数未渲染（见明细）
  baseline: EventID=8211 Provider=System Restore
    Date=2026-08-29T05:31:50.9330000Z
    Msg=The scheduled restore point could not be created. Additional information: (0x80042308).
  new-en:   EventID=8211 Provider=System Restore
    Time=2026-08-29 05:31:50.933232
    Msg=0x80042308;;;00000000000000009F01000000000000574C29CE5843BE27E26C1C000000000000000000;;

### 63. [B] Application / Everything / EID=0（共 1 条，裸参数未渲染）

首条索引：Application #3343

[Application #3343] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=Everything
    Date=2026-09-01T05:32:39.3760000Z
    Msg=操作成功完成。
  new-en:   EventID=0 Provider=Everything
    Time=2026-09-01 05:32:39.376807
    Msg=.\src\mem.c(776): mem_virtual_alloc(): Fatal error: out of memory, size: 000000000000040f, error: 1455;;

### 64. [B] Application / ESENT / EID=300（共 2 条，裸参数未渲染）

首条索引：Application #3883

[Application #3883] B-裸参数未渲染（见明细）
  baseline: EventID=300 Provider=ESENT
    Date=2026-09-03T10:54:35.1270000Z
    Msg=Video.UI (25188,R,98,0) {CB186734-758D-4DA5-B036-80D295877D41}: The database engine is initiating recovery steps.
  new-en:   EventID=300 Provider=ESENT
    Time=2026-09-03 10:54:35.127548
    Msg=Video.UI;;25188,R,98,0;;{CB186734-758D-4DA5-B036-80D295877D41}: ;;

### 65. [D] Application / ESENT / EID=301（共 2 条，真渲染分歧待查）

首条索引：Application #3885

[Application #3885] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=301 Provider=ESENT
    Date=2026-09-03T10:54:35.1730000Z
    Msg=Video.UI (25188,R,98,0) {CB186734-758D-4DA5-B036-80D295877D41}: The database engine has finished replaying logfile C:\Users\kitsch\AppData\Local\Packages\Microsoft.ZuneVideo_8wekyb3d8bbwe\LocalState\Database\anonymous\edb.log. Processing Stats: [1] 0.044157 -0.001925 (5) WT +J(CM:0, PgRf:0, Rd:0/0, Dy:0/0, Lg:1058616/696) +M(C:0K, Fs:2815, WS:4540K # 4480K, PF:920K # 856K, P:920K). Log record of type 'AttachDB ' was seen most frequently (87 times)
  new-en:   EventID=301 Provider=ESENT
    Time=2026-09-03 10:54:35.173551
    Msg=Video.UI;;25188,R,98,0;;{CB186734-758D-4DA5-B036-80D295877D41}: ;;C:\Users\kitsch\AppData\Local\Packages\Microsoft.ZuneVideo_8wekyb3d8bbwe\LocalState\Database\anonymous\edb.log;; [1] 0.044157 -0.001925 (5) WT +J(CM:0, PgRf:0, Rd:0/0, Dy:0/0, Lg:1058616/696) +M(C:0K, Fs:2815, WS:4540K # 4480K, PF:920K # 856K, P:920K).;;AttachDB ;;87;;

### 66. [B] Application / ESENT / EID=302（共 2 条，裸参数未渲染）

首条索引：Application #3886

[Application #3886] B-裸参数未渲染（见明细）
  baseline: EventID=302 Provider=ESENT
    Date=2026-09-03T10:54:35.2080000Z
    Msg=Video.UI (25188,U,98,0) {CB186734-758D-4DA5-B036-80D295877D41}: The database engine has successfully completed recovery steps.
  new-en:   EventID=302 Provider=ESENT
    Time=2026-09-03 10:54:35.208140
    Msg=Video.UI;;25188,U,98,0;;{CB186734-758D-4DA5-B036-80D295877D41}: ;;

### 67. [D] Application / ESENT / EID=326（共 2 条，真渲染分歧待查）

首条索引：Application #3889

[Application #3889] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=326 Provider=ESENT
    Date=2026-09-03T10:54:35.2230000Z
    Msg=Video.UI (25188,D,50,0) {CB186734-758D-4DA5-B036-80D295877D41}: The database engine attached a database (1, C:\Users\kitsch\AppData\Local\Packages\Microsoft.ZuneVideo_8wekyb3d8bbwe\LocalState\Database\anonymous\EntClientDb.edb). (Time=0 seconds) Saved Cache: 0 0 Additional Data: lgposAttach = 00000002:0108:0268, dbv = 1568.20.0 (8920) Internal Timing Sequence: [1] 0.000003 +J(0) +M(C:0K, Fs:1, WS:4K # 0K, PF:0K # 0K, P:0K) [2] 0.000458 +J(0) +M(C:0K, Fs:18, WS:8K # 0K, PF:8K # 0K, P:8K) [3] 0.00
  new-en:   EventID=326 Provider=ESENT
    Time=2026-09-03 10:54:35.223185
    Msg=Video.UI;;25188,D,50,0;;{CB186734-758D-4DA5-B036-80D295877D41}: ;;1;;C:\Users\kitsch\AppData\Local\Packages\Microsoft.ZuneVideo_8wekyb3d8bbwe\LocalState\Database\anonymous\EntClientDb.edb;;0;; [1] 0.000003 +J(0) +M(C:0K, Fs:1, WS:4K # 0K, PF:0K # 0K, P:0K) [2] 0.000458 +J(0) +M(C:0K, Fs:18, WS:8K # 0K, PF:8K # 0K, P:8K) [3] 0.001794 -0.000655 (5) WT +J(CM:0, PgRf:0, Rd:0/0, Dy:0/0, Lg:3480/2) +M(C:0K, Fs:41, WS:152K # 0K, PF:36K # 0K, P:36K) [4] 0.000285 +J(0) +M(C:0K, Fs:4, WS:16K # 0K, PF:0K #

### 68. [B] System / EventLog / EID=6009（共 3 条，裸参数未渲染）

首条索引：System #0

[System #0] B-裸参数未渲染（见明细）
  baseline: EventID=6009 Provider=EventLog
    Date=2026-08-21T05:03:37.2050000Z
    Msg=Microsoft (R) Windows (R) 10.00. 26220 Multiprocessor Free.
  new-en:   EventID=6009 Provider=EventLog
    Time=2026-08-21 05:03:37.205170
    Msg=10.00.;;26220;;;;Multiprocessor Free;;0;;

### 69. [B] System / EventLog / EID=6005（共 3 条，裸参数未渲染）

首条索引：System #1

[System #1] B-裸参数未渲染（见明细）
  baseline: EventID=6005 Provider=EventLog
    Date=2026-08-21T05:03:37.2050000Z
    Msg=The Event log service was started.
  new-en:   EventID=6005 Provider=EventLog
    Time=2026-08-21 05:03:37.205251
    Msg=;EA07080004001400150003002500CC000000000000000000;;

### 70. [D] System / Microsoft-Windows-Kernel-General / EID=25（共 3 条，真渲染分歧待查）

首条索引：System #3

[System #3] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=25 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-21T05:02:10.1470000Z
    Msg=The system time was initialized to 2026-08-20T21:02:09.500000000Z. Loader time: 2026-08-20T21:02:08.000000000Z Internal boot flags: 0x2 HAL RTC error code: 0x0 RTC time is in UTC: false Soft boot: false Success: true Phase: 0
  new-en:   EventID=25 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-21 05:02:10.147334
    Msg=The system time was initialized to 2026-08-20T21:02:09.500000Z. Loader time: 2026-08-20T21:02:08.000000Z Internal boot flags: 2 HAL RTC error code: 0 RTC time is in UTC: Soft boot: Success: Phase: 0

### 71. [D] System / Microsoft-Windows-Kernel-Boot / EID=153（共 3 条，真渲染分歧待查）

首条索引：System #4

[System #4] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=153 Provider=Microsoft-Windows-Kernel-Boot
    Date=2026-08-21T05:02:10.1470000Z
    Msg=Virtualization-based security (policies: 0) is disabled.
  new-en:   EventID=153 Provider=Microsoft-Windows-Kernel-Boot
    Time=2026-08-21 05:02:10.147775
    Msg=Virtualization-based security (policies: 0) is Disabled.

### 72. [D] System / Microsoft-Windows-Kernel-Boot / EID=20（共 3 条，真渲染分歧待查）

首条索引：System #7

[System #7] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=20 Provider=Microsoft-Windows-Kernel-Boot
    Date=2026-08-21T05:02:10.1480000Z
    Msg=The last shutdown's success status was true. The last boot's success status was true.
  new-en:   EventID=20 Provider=Microsoft-Windows-Kernel-Boot
    Time=2026-08-21 05:02:10.148252
    Msg=The last shutdown's success status was . The last boot's success status was .

### 73. [D] System / Microsoft-Windows-Kernel-Boot / EID=25（共 3 条，真渲染分歧待查）

首条索引：System #9

[System #9] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=25 Provider=Microsoft-Windows-Kernel-Boot
    Date=2026-08-21T05:02:10.1480000Z
    Msg=The boot menu policy was 0x1.
  new-en:   EventID=25 Provider=Microsoft-Windows-Kernel-Boot
    Time=2026-08-21 05:02:10.148404
    Msg=The boot menu policy was 1.

### 74. [D] System / Microsoft-Windows-Kernel-Boot / EID=27（共 3 条，真渲染分歧待查）

首条索引：System #10

[System #10] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=27 Provider=Microsoft-Windows-Kernel-Boot
    Date=2026-08-21T05:02:10.1480000Z
    Msg=The boot type was 0x0.
  new-en:   EventID=27 Provider=Microsoft-Windows-Kernel-Boot
    Time=2026-08-21 05:02:10.148404
    Msg=The boot type was 0.

### 75. [D] System / Microsoft-Windows-Kernel-General / EID=20（共 3 条，真渲染分歧待查）

首条索引：System #12

[System #12] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=20 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-21T05:02:10.1560000Z
    Msg=The leap second configuration has been updated. Reason: Leap second data initialized from registry during boot Leap seconds enabled: true New leap second count: 0 Old leap second count: 0
  new-en:   EventID=20 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-21 05:02:10.156741
    Msg=The leap second configuration has been updated. Reason: 0 Leap seconds enabled: New leap second count: 0 Old leap second count: 0

### 76. [D] System / Microsoft-Windows-Ntfs / EID=98（共 28 条，真渲染分歧待查）

首条索引：System #17

[System #17] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=98 Provider=Microsoft-Windows-Ntfs
    Date=2026-08-21T05:02:12.2920000Z
    Msg=Volume C: (\Device\HarddiskVolume4) is healthy. No action is needed.
  new-en:   EventID=98 Provider=Microsoft-Windows-Ntfs
    Time=2026-08-21 05:02:12.292166
    Msg=Volume C: (\Device\HarddiskVolume4) Disabled

### 77. [D] System / Microsoft-Windows-Kernel-Power / EID=172（共 3 条，真渲染分歧待查）

首条索引：System #22

[System #22] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=172 Provider=Microsoft-Windows-Kernel-Power
    Date=2026-08-21T05:02:12.9070000Z
    Msg=Connectivity state in standby: Disconnected, Reason: NIC compliance
  new-en:   EventID=172 Provider=Microsoft-Windows-Kernel-Power
    Time=2026-08-21 05:02:12.907978
    Msg=Connectivity state in standby: 2, Reason: 6

### 78. [D] System / Microsoft-Windows-Kernel-Processor-Power / EID=55（共 36 条，真渲染分歧待查）

首条索引：System #23

[System #23] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=55 Provider=Microsoft-Windows-Kernel-Processor-Power
    Date=2026-08-21T05:02:13.0460000Z
    Msg=Processor 0 in group 0 exposes the following power management capabilities: Idle state type: ACPI Idle (C) States (3 state(s)) Performance state type: ACPI Collaborative Processor Performance Control Nominal Frequency (MHz): 3096 Maximum performance percentage: 145 Minimum performance percentage: 25 Minimum throttle percentage: 3
  new-en:   EventID=55 Provider=Microsoft-Windows-Kernel-Processor-Power
    Time=2026-08-21 05:02:13.046787
    Msg=Processor 0 in group 0 exposes the following power management capabilities: Idle state type: 1 (3 state(s)) Performance state type: 3 Nominal Frequency (MHz): 3096 Maximum performance percentage: 145 Minimum performance percentage: 25 Minimum throttle percentage: 3

### 79. [B] System / MEIx64 / EID=2（共 3 条，裸参数未渲染）

首条索引：System #35

[System #35] B-裸参数未渲染（见明细）
  baseline: EventID=2 Provider=MEIx64
    Date=2026-08-21T05:02:13.0950000Z
    Msg=Intel(R) Management Engine Interface driver has started successfully.
  new-en:   EventID=2 Provider=MEIx64
    Time=2026-08-21 05:02:13.095502
    Msg=;00000000010000000000000002000740000000000000000000000000000000000000000000000000;;

### 80. [D] System / Microsoft-Windows-Wininit / EID=19（共 3 条，真渲染分歧待查）

首条索引：System #42

[System #42] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=19 Provider=Microsoft-Windows-Wininit
    Date=2026-08-21T05:02:23.7110000Z
    Msg=Virtualization Based Security new timer creation status HRESULT: The operation completed successfully. New latch timer needed: false New latch timer waiting for system update completion: false Previous latch timer exists but disabled by registry: false Policy file exists: false
  new-en:   EventID=19 Provider=Microsoft-Windows-Wininit
    Time=2026-08-21 05:02:23.711957
    Msg=Virtualization Based Security new timer creation status HRESULT: 0 New latch timer needed: New latch timer waiting for system update completion: Previous latch timer exists but disabled by registry: Policy file exists:

### 81. [D] System / Microsoft-Windows-Wininit / EID=25（共 3 条，真渲染分歧待查）

首条索引：System #44

[System #44] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=25 Provider=Microsoft-Windows-Wininit
    Date=2026-08-21T05:02:23.7110000Z
    Msg=Boot App Anti-Rollback: Previous timer resumed with status: HRESULT: The operation completed successfully. Previous timer present: true Start time: 2026-07-01T18:07:18.524471700Z Grace period: 51840000000000 Attempted recovery enforcement succeeded: false
  new-en:   EventID=25 Provider=Microsoft-Windows-Wininit
    Time=2026-08-21 05:02:23.711995
    Msg=Boot App Anti-Rollback: Previous timer resumed with status: HRESULT: 0 Previous timer present: Start time: 2026-07-01T18:07:18.524471Z Grace period: 51840000000000 Attempted recovery enforcement succeeded:

### 82. [D] System / Microsoft-Windows-Wininit / EID=23（共 3 条，真渲染分歧待查）

首条索引：System #45

[System #45] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=23 Provider=Microsoft-Windows-Wininit
    Date=2026-08-21T05:02:23.7120000Z
    Msg=Boot App Anti-Rollback: Initialize Completed with status: HRESULT: The operation completed successfully. New timer needed: false New timer waiting for system update completion: false Previous latch timer exists but disabled by registry: false
  new-en:   EventID=23 Provider=Microsoft-Windows-Wininit
    Time=2026-08-21 05:02:23.712403
    Msg=Boot App Anti-Rollback: Initialize Completed with status: HRESULT: 0 New timer needed: New timer waiting for system update completion: Previous latch timer exists but disabled by registry:

### 83. [D] System / Microsoft-Windows-Wininit / EID=14（共 3 条，真渲染分歧待查）

首条索引：System #46

[System #46] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=14 Provider=Microsoft-Windows-Wininit
    Date=2026-08-21T05:02:23.8090000Z
    Msg=Credential Guard configuration: Registry Configuration: 0x2 Test Configuration: 0 Auto Enablement: 1
  new-en:   EventID=14 Provider=Microsoft-Windows-Wininit
    Time=2026-08-21 05:02:23.809352
    Msg=Credential Guard configuration: Registry Configuration: 2 Test Configuration: Disabled Auto Enablement: Enabled

### 84. [D] System / Microsoft-Windows-Wininit / EID=15（共 3 条，真渲染分歧待查）

首条索引：System #47

[System #47] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=15 Provider=Microsoft-Windows-Wininit
    Date=2026-08-21T05:02:23.8090000Z
    Msg=Credential Guard and/or VBS Key Isolation are configured but the secure kernel is not running continuing without them.
  new-en:   EventID=15 Provider=Microsoft-Windows-Wininit
    Time=2026-08-21 05:02:23.809365
    Msg=Credential Guard and/or VBS Key Isolation are configured but the secure kernel is not running; continuing without them.

### 85. [D] System / LsaSrv / EID=6156（共 3 条，真渲染分歧待查）

首条索引：System #49

[System #49] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=6156 Provider=LsaSrv
    Date=2026-08-21T05:02:23.8610000Z
    Msg=Credential Guard auto enablement status. Hardware Requirements for Virtualization Based Security: 1 Domain Joined: 0 Azure AD Joined: 0 Licensed for Credential Guard: 1 Domain Controller: 0
  new-en:   EventID=6156 Provider=LsaSrv
    Time=2026-08-21 05:02:23.861054
    Msg=Credential Guard auto enablement status. Hardware Requirements for Virtualization Based Security: 1 Domain Joined: 0 Azure AD Joined: 0 Licensed for Credential Guard: Enabled Domain Controller: 0

### 86. [D] System / Microsoft-Windows-Directory-Services-SAM / EID=16962（共 3 条，真渲染分歧待查）

首条索引：System #51

[System #51] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=16962 Provider=Microsoft-Windows-Directory-Services-SAM
    Date=2026-08-21T05:02:24.0240000Z
    Msg=Remote calls to the SAM database are being restricted using the default security descriptor: O:SYG:SYD:(A RC BA). For more information please see http://go.microsoft.com/fwlink/LinkId=787651.
  new-en:   EventID=16962 Provider=Microsoft-Windows-Directory-Services-SAM
    Time=2026-08-21 05:02:24.024560
    Msg=Remote calls to the SAM database are being restricted using the default security descriptor: . For more information please see http://go.microsoft.com/fwlink/?LinkId=787651.

### 87. [D] System / Microsoft-Windows-Directory-Services-SAM / EID=16977（共 3 条，疑似compare-?清洗artefact）

首条索引：System #52

[System #52] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=16977 Provider=Microsoft-Windows-Directory-Services-SAM
    Date=2026-08-21T05:02:24.0320000Z
    Msg=The domain is configured with the following minimum password length-related settings. MinimumPasswordLength: 0 RelaxMinimumPasswordLengthLimits: 0 MinimumPasswordLengthAudit: -1 For more information see https://go.microsoft.com/fwlink/LinkId=2097191.
  new-en:   EventID=16977 Provider=Microsoft-Windows-Directory-Services-SAM
    Time=2026-08-21 05:02:24.032788
    Msg=The domain is configured with the following minimum password length-related settings. MinimumPasswordLength: 0 RelaxMinimumPasswordLengthLimits: 0 MinimumPasswordLengthAudit: -1 For more information see https://go.microsoft.com/fwlink/?LinkId=2097191.

### 88. [D] System / Microsoft-Windows-Directory-Services-SAM / EID=16983（共 3 条，疑似compare-?清洗artefact）

首条索引：System #53

[System #53] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=16983 Provider=Microsoft-Windows-Directory-Services-SAM
    Date=2026-08-21T05:02:24.0320000Z
    Msg=The security account manager is now logging periodic summary events for remote clients that call legacy password change or set RPC methods. For more information please see https://go.microsoft.com/fwlink/linkid=2150956.
  new-en:   EventID=16983 Provider=Microsoft-Windows-Directory-Services-SAM
    Time=2026-08-21 05:02:24.032791
    Msg=The security account manager is now logging periodic summary events for remote clients that call legacy password change or set RPC methods. For more information please see https://go.microsoft.com/fwlink/?linkid=2150956.

### 89. [D] System / Service Control Manager / EID=7045（共 53 条，疑似compare-尾冒号artefact）

首条索引：System #54

[System #54] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=7045 Provider=Service Control Manager
    Date=2026-08-21T05:02:25.0620000Z
    Msg=A service was installed in the system. Service Name: Capsa Packet Driver (CAPSADRV) (Wi-Fi) Service File Name: \SystemRoot\system32\DRIVERS\capsadrv.sys Service Type: 内核模式驱动程序 Service Start Type: 系统启动 Service Account
  new-en:   EventID=7045 Provider=Service Control Manager
    Time=2026-08-21 05:02:25.062827
    Msg=A service was installed in the system. Service Name: Capsa Packet Driver (CAPSADRV) (Wi-Fi) Service File Name: \SystemRoot\system32\DRIVERS\capsadrv.sys Service Type: 内核模式驱动程序 Service Start Type: 系统启动 Service Account:

### 90. [D] System / Service Control Manager / EID=7000（共 4 条，真渲染分歧待查）

首条索引：System #59

[System #59] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=7000 Provider=Service Control Manager
    Date=2026-08-21T05:02:29.8230000Z
    Msg=The Capsa Packet Driver (CAPSADRV) service failed to start due to the following error: The system cannot find the file specified.
  new-en:   EventID=7000 Provider=Service Control Manager
    Time=2026-08-21 05:02:29.823162
    Msg=The Capsa Packet Driver (CAPSADRV) service failed to start due to the following error: Elevation Service Events

### 91. [D] System / Microsoft-Windows-Kernel-General / EID=16（共 275 条，疑似compare-?清洗artefact）

首条索引：System #62

[System #62] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=16 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-21T05:03:10.1410000Z
    Msg=The access history in hive \\C:\ProgramData\Microsoft\Windows\AppRepository\Packages\c5e2524a-ea46-4f67-841f-6a9465d9d515_10.0.26100.6_neutral_neutral_cw5n1h2txyewy\ActivationStore.dat was cleared updating 0 keys and creating 0 modified pages.
  new-en:   EventID=16 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-21 05:03:10.141160
    Msg=The access history in hive \??\C:\ProgramData\Microsoft\Windows\AppRepository\Packages\c5e2524a-ea46-4f67-841f-6a9465d9d515_10.0.26100.6_neutral_neutral_cw5n1h2txyewy\ActivationStore.dat was cleared updating 0 keys and creating 0 modified pages.

### 92. [D] System / Microsoft-Windows-HttpService / EID=112（共 51 条，真渲染分歧待查）

首条索引：System #107

[System #107] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=112 Provider=Microsoft-Windows-HttpService
    Date=2026-08-21T05:03:37.3930000Z
    Msg=Attempted to reserve URL http://*:5357/. Status 0x0. Process Id 0x4 Executable path , User S-1-5-18
  new-en:   EventID=112 Provider=Microsoft-Windows-HttpService
    Time=2026-08-21 05:03:37.393786
    Msg=Attempted to reserve URL http://*:5357/. Status 0. Process Id 4 Executable path , User S-1-5-18

### 93. [D] System / Service Control Manager / EID=7023（共 6 条，真渲染分歧待查）

首条索引：System #126

[System #126] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=7023 Provider=Service Control Manager
    Date=2026-08-21T05:03:37.7570000Z
    Msg=The Network List Service service terminated with the following error: The device is not ready.
  new-en:   EventID=7023 Provider=Service Control Manager
    Time=2026-08-21 05:03:37.757803
    Msg=The Network List Service service terminated with the following error: %21

### 94. [B] System / VMnetAdapter / EID=36（共 3 条，裸参数未渲染）

首条索引：System #132

[System #132] B-裸参数未渲染（见明细）
  baseline: EventID=36 Provider=VMnetAdapter
    Date=2026-08-21T05:03:38.7900000Z
    Msg=() Starting up the Adapter Driver for VMware Virtual Networks.
  new-en:   EventID=36 Provider=VMnetAdapter
    Time=2026-08-21 05:03:38.790802
    Msg=;00000000010000000000000024000840240008400000000000000000000000000000000000000000;;

### 95. [B] System / e1dexpress / EID=32（共 3 条，裸参数未渲染）

首条索引：System #159

[System #159] B-裸参数未渲染（见明细）
  baseline: EventID=32 Provider=e1dexpress
    Date=2026-08-21T05:03:47.9620000Z
    Msg=Intel(R) Ethernet Connection (12) I219-V Network link has been established at 1Gbps full duplex.
  new-en:   EventID=32 Provider=e1dexpress
    Time=2026-08-21 05:03:47.962030
    Msg=;;Intel(R) Ethernet Connection (12) I219-V;;;0000040002003000000000002000046000000000000000000000000000000000000000000000000020000460;;

### 96. [D] System / User32 / EID=1074（共 1 条，疑似compare-尾冒号artefact）

首条索引：System #228

[System #228] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1074 Provider=User32
    Date=2026-08-21T05:05:09.3810000Z
    Msg=The process C:\WINDOWS\system32\winlogon.exe (DESKTOP-7T8SD61) has initiated the 重启 of computer DESKTOP-7T8SD61 on behalf of user NT AUTHORITY\SYSTEM for the following reason: 操作系统: 升级(计划内) Reason Code: 0x80020003 Shutdown Type: 重启 Comment
  new-en:   EventID=1074 Provider=User32
    Time=2026-08-21 05:05:09.381176
    Msg=The process C:\WINDOWS\system32\winlogon.exe (DESKTOP-7T8SD61) has initiated the 重启 of computer DESKTOP-7T8SD61 on behalf of user NT AUTHORITY\SYSTEM for the following reason: 操作系统: 升级(计划内) Reason Code: 0x80020003 Shutdown Type: 重启 Comment:

### 97. [B] System / EventLog / EID=6006（共 1 条，裸参数未渲染）

首条索引：System #229

[System #229] B-裸参数未渲染（见明细）
  baseline: EventID=6006 Provider=EventLog
    Date=2026-08-21T05:05:25.3710000Z
    Msg=The Event log service was stopped.
  new-en:   EventID=6006 Provider=EventLog
    Time=2026-08-21 05:05:25.371515
    Msg=;0000000000000000;;

### 98. [D] System / EventLog / EID=6013（共 16 条，真渲染分歧待查）

首条索引：System #238

[System #238] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=6013 Provider=EventLog
    Date=2026-08-21T05:06:12.6910000Z
    Msg=The system uptime is 23 seconds.
  new-en:   EventID=6013 Provider=EventLog
    Time=2026-08-21 05:06:12.691588
    Msg=;;;;;;;;23;;60;;-480 中国标准时间;;;31002E003100000030000000570069006E0064006F0077007300200031003000200045006400750063006100740069006F006E000000310030002E0030002E003200360032003200300020004200750069006C0064002000320036003200320030002000200000004D0075006C0074006900700072006F0063006500730073006F007200200046007200650065000000320036003100300030002E00670065005F00720065006C0065006100730065005F0066006C0074002E003200360030003700310036002D00310037003000300000003600610038003700360063003000350000004E006F00740020

### 99. [D] System / Microsoft-Windows-Kernel-Power / EID=109（共 1 条，真渲染分歧待查）

首条索引：System #240

[System #240] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=109 Provider=Microsoft-Windows-Kernel-Power
    Date=2026-08-21T05:05:25.8730000Z
    Msg=The kernel power manager has initiated a shutdown transition. Action: Power Action Reboot Event Code: 0x0 Reason: Kernel API
  new-en:   EventID=109 Provider=Microsoft-Windows-Kernel-Power
    Time=2026-08-21 05:05:25.873336
    Msg=The kernel power manager has initiated a shutdown transition. Action: 5 Event Code: 0 Reason: 5

### 100. [D] System / Microsoft-Windows-Kernel-Power / EID=577（共 1 条，真渲染分歧待查）

首条索引：System #241

[System #241] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=577 Provider=Microsoft-Windows-Kernel-Power
    Date=2026-08-21T05:05:26.4880000Z
    Msg=The system has prepared for a system initiated reboot from Active.
  new-en:   EventID=577 Provider=Microsoft-Windows-Kernel-Power
    Time=2026-08-21 05:05:26.488348
    Msg=The system has prepared for a system initiated reboot from Disabled.

### 101. [D] System / Microsoft-Windows-Kernel-PnP / EID=219（共 8 条，真渲染分歧待查）

首条索引：System #266

[System #266] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=219 Provider=Microsoft-Windows-Kernel-PnP
    Date=2026-08-21T05:05:54.8530000Z
    Msg=The driver \Driver\WUDFRd failed to load. Device: ROOT\DISPLAY\0000 Status: 0xC0000365
  new-en:   EventID=219 Provider=Microsoft-Windows-Kernel-PnP
    Time=2026-08-21 05:05:54.853966
    Msg=The driver \Driver\WUDFRd failed to load. Device: ROOT\DISPLAY\0000 Status: 0xc0000365

### 102. [B] System / VMnetuserif / EID=1（共 2 条，裸参数未渲染）

首条索引：System #322

[System #322] B-裸参数未渲染（见明细）
  baseline: EventID=1 Provider=VMnetuserif
    Date=2026-08-21T05:06:12.7970000Z
    Msg=() Starting up the User Interface Driver for VMware Virtual Networks.
  new-en:   EventID=1 Provider=VMnetuserif
    Time=2026-08-21 05:06:12.797823
    Msg=;00000000010000000000000001000640010006400000000001000000000000000000000000000000;;

### 103. [B] System / vmx86 / EID=34（共 2 条，裸参数未渲染）

首条索引：System #340

[System #340] B-裸参数未渲染（见明细）
  baseline: EventID=34 Provider=vmx86
    Date=2026-08-21T05:06:12.9710000Z
    Msg=N/A
  new-en:   EventID=34 Provider=vmx86
    Time=2026-08-21 05:06:12.971189
    Msg=;;VMX86.SYS: begin DriverEntry;;;00000000020028000000000022000840220008400000000000000000000000000000000000000000;;

### 104. [B] System / hcmon / EID=0（共 89 条，裸参数未渲染）

首条索引：System #341

[System #341] B-裸参数未渲染（见明细）
  baseline: EventID=0 Provider=hcmon
    Date=2026-08-21T05:06:13.1380000Z
    Msg=Detected unrecognized USB driver (\Driver\hrdevmon).
  new-en:   EventID=0 Provider=hcmon
    Time=2026-08-21 05:06:13.138788
    Msg=\Device\hcmon;;\Driver\hrdevmon;;;00000000020028000000000000000080000000000000000000000000000000000000000000000000;;

### 105. [D] System / Microsoft-Windows-Kernel-Power / EID=566（共 190 条，真渲染分歧待查）

首条索引：System #420

[System #420] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=566 Provider=Microsoft-Windows-Kernel-Power
    Date=2026-08-21T05:10:40.9210000Z
    Msg=The system session has transitioned from 0 to 1. Reason SessionUnlock BootId: 2
  new-en:   EventID=566 Provider=Microsoft-Windows-Kernel-Power
    Time=2026-08-21 05:10:40.921847
    Msg=The system session has transitioned from 0 to 1. Reason 12 BootId: 2

### 106. [D] System / Microsoft-Windows-TPM-WMI / EID=1808（共 2 条，真渲染分歧待查）

首条索引：System #424

[System #424] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1808 Provider=Microsoft-Windows-TPM-WMI
    Date=2026-08-21T05:14:02.3060000Z
    Msg=This device has updated Secure Boot CA/keys. This device signature information is included here. DeviceAttributes: BaseBoardManufacturer:ASUSTeK COMPUTER INC. FirmwareManufacturer:American Megatrends Inc. FirmwareVersion:0306 OEMModelBaseBoard:TUF GAMING B460M-PLUS OEMManufacturerName:ASUS OSArchitecture:amd64 BucketId: f98b09469af9d08a2d88124f8993b9f2d0354b45d39e1de62b437d183b9739dc BucketConfidenceLevel: High Confidence UpdateType: Windows UEFI CA 2023 (DB), Option ROM CA 2023 (DB), 3P UEFI CA
  new-en:   EventID=1808 Provider=Microsoft-Windows-TPM-WMI
    Time=2026-08-21 05:14:02.306132
    Msg=This device has updated Secure Boot CA/keys. This device signature information is included here. DeviceAttributes: BaseBoardManufacturer:ASUSTeK COMPUTER INC.;FirmwareManufacturer:American Megatrends Inc.;FirmwareVersion:0306;OEMModelBaseBoard:TUF GAMING B460M-PLUS;OEMManufacturerName:ASUS;OSArchitecture:amd64; BucketId: f98b09469af9d08a2d88124f8993b9f2d0354b45d39e1de62b437d183b9739dc BucketConfidenceLevel: High Confidence UpdateType: Windows UEFI CA 2023 (DB), Option ROM CA 2023 (DB), 3P UEFI C

### 107. [B] System / Win32k / EID=267（共 14 条，裸参数未渲染）

首条索引：System #468

[System #468] B-裸参数未渲染（见明细）
  baseline: EventID=267 Provider=Win32k
    Date=2026-08-21T09:18:59.3260000Z
    Msg=Touch/Touchpad Hardware Quality Assurance verification succeeded.
  new-en:   EventID=267 Provider=Win32k
    Time=2026-08-21 09:18:59.326113
    Msg=;0000000001000000000000000B010040000000000000000000000000000000000000000000000000;;

### 108. [B] System / EventLog / EID=6008（共 1 条，裸参数未渲染）

首条索引：System #482

[System #482] B-裸参数未渲染（见明细）
  baseline: EventID=6008 Provider=EventLog
    Date=2026-08-21T09:31:08.4640000Z
    Msg=The previous system shutdown at 9:16:12 on 2026/8/21 was unexpected.
  new-en:   EventID=6008 Provider=EventLog
    Time=2026-08-21 09:31:08.464621
    Msg=9:16:12;;‎2026/‎8/‎21;;;;;;15023;;;;;;;EA07080005001500090010000C00FF02EA07080005001500010010000C00FF02DC0500003C00000001000000DC050000010000003C0000000000000000000000;;

### 109. [D] System / Microsoft-Windows-Kernel-Boot / EID=18（共 1 条，真渲染分歧待查）

首条索引：System #487

[System #487] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=18 Provider=Microsoft-Windows-Kernel-Boot
    Date=2026-08-21T09:30:45.0290000Z
    Msg=There are 0x2 boot options on this system.
  new-en:   EventID=18 Provider=Microsoft-Windows-Kernel-Boot
    Time=2026-08-21 09:30:45.029437
    Msg=There are 2 boot options on this system.

### 110. [D] System / Microsoft-Windows-WindowsUpdateClient / EID=20（共 22 条，真渲染分歧待查）

首条索引：System #624

[System #624] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=20 Provider=Microsoft-Windows-WindowsUpdateClient
    Date=2026-08-21T10:10:28.3010000Z
    Msg=Installation Failure: Windows failed to install the following update with error 0x8024200B: PowerShell v7.3.1 (x64).
  new-en:   EventID=20 Provider=Microsoft-Windows-WindowsUpdateClient
    Time=2026-08-21 10:10:28.301182
    Msg=Installation Failure: Windows failed to install the following update with error 0x8024200b: PowerShell v7.3.1 (x64).

### 111. [D] System / Microsoft-Windows-WPDClassInstaller / EID=24577（共 2 条，真渲染分歧待查）

首条索引：System #638

[System #638] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=24577 Provider=Microsoft-Windows-WPDClassInstaller
    Date=2026-08-21T14:00:42.0870000Z
    Msg=Media player and imaging program compatibility layers were successfully registered for device %1. Layer bits %2 were requested, layer bits %3 were registered.
  new-en:   EventID=24577 Provider=Microsoft-Windows-WPDClassInstaller
    Time=2026-08-21 14:00:42.087302
    Msg=Media player and imaging program compatibility layers were successfully registered for device . Layer bits were requested, layer bits were registered.

### 112. [D] System / Microsoft-Windows-WPDClassInstaller / EID=24579（共 2 条，真渲染分歧待查）

首条索引：System #639

[System #639] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=24579 Provider=Microsoft-Windows-WPDClassInstaller
    Date=2026-08-21T14:00:42.0880000Z
    Msg=Autoplay registration was skipped for device %1.
  new-en:   EventID=24579 Provider=Microsoft-Windows-WPDClassInstaller
    Time=2026-08-21 14:00:42.088354
    Msg=Autoplay registration was skipped for device .

### 113. [D] System / Microsoft-Windows-DriverFrameworks-UserMode / EID=10000（共 2 条，疑似compare-?清洗artefact）

首条索引：System #640

[System #640] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=10000 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Date=2026-08-21T14:00:41.8740000Z
    Msg=A driver package which uses user-mode driver framework version 2.35.0 is being installed on device SWD\WPDBUSENUM\__USBSTOR#DISK&VEN_HIFIMAN&PROD_SUPERMINI_SD&REV_1.00#USBV1.00&0#{53F56307-B6BF-11D0-94F2-00A0C91EFB8B}.
  new-en:   EventID=10000 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Time=2026-08-21 14:00:41.874178
    Msg=A driver package which uses user-mode driver framework version 2.35.0 is being installed on device SWD\WPDBUSENUM\_??_USBSTOR#DISK&VEN_HIFIMAN&PROD_SUPERMINI_SD&REV_1.00#USBV1.00&0#{53F56307-B6BF-11D0-94F2-00A0C91EFB8B}.

### 114. [D] System / Microsoft-Windows-DriverFrameworks-UserMode / EID=10001（共 1 条，真渲染分歧待查）

首条索引：System #641

[System #641] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=10001 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Date=2026-08-21T14:00:41.8810000Z
    Msg=The UMDF service WpdFs (CLSID {112de495-ac4c-46f8-b663-6a4266c53313}) was installed. It requires framework version 2.35.0 or higher.
  new-en:   EventID=10001 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Time=2026-08-21 14:00:41.881816
    Msg=The UMDF service WpdFs (CLSID 112DE495-AC4C-46F8-B663-6A4266C53313) was installed. It requires framework version 2.35.0 or higher.

### 115. [D] System / Microsoft-Windows-UserPnp / EID=20003（共 2 条，疑似compare-?清洗artefact）

首条索引：System #644

[System #644] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=20003 Provider=Microsoft-Windows-UserPnp
    Date=2026-08-21T14:00:41.9110000Z
    Msg=Driver Management has concluded the process to add Service WUDFWpdFs for Device Instance ID SWD\WPDBUSENUM\__USBSTOR#DISK&VEN_HIFIMAN&PROD_SUPERMINI_SD&REV_1.00#USBV1.00&0#{53F56307-B6BF-11D0-94F2-00A0C91EFB8B} with the following status: 0.
  new-en:   EventID=20003 Provider=Microsoft-Windows-UserPnp
    Time=2026-08-21 14:00:41.911236
    Msg=Driver Management has concluded the process to add Service WUDFWpdFs for Device Instance ID SWD\WPDBUSENUM\_??_USBSTOR#DISK&VEN_HIFIMAN&PROD_SUPERMINI_SD&REV_1.00#USBV1.00&0#{53F56307-B6BF-11D0-94F2-00A0C91EFB8B} with the following status: 0.

### 116. [C] System / Microsoft-Windows-Configuration-Change-Monitor / EID=1（共 40 条，无模板fallback）

首条索引：System #649

[System #649] C-无模板fallback（见明细）
  baseline: EventID=1 Provider=Microsoft-Windows-Configuration-Change-Monitor
    Date=2026-08-21T15:10:05.5620000Z
    Msg=N/A
  new-en:   EventID=1 Provider=Microsoft-Windows-Configuration-Change-Monitor
    Time=2026-08-21 15:10:05.562933
    Msg=The description for Event ID 1 from source Microsoft-Windows-Configuration-Change-Monitor cannot be found. The component that raises this event is not installed on your local computer or the installation is corrupted. You can install or repair the component on the local computer. If the event originated on another computer, the display information had to be saved with the event. The following information was included with the event: S-1-5-21-4275410414-1035109868-2709043917-1001 "C:\WINDOWS\syst

### 117. [D] System / Microsoft-Windows-Time-Service / EID=35（共 7 条，真渲染分歧待查）

首条索引：System #712

[System #712] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=35 Provider=Microsoft-Windows-Time-Service
    Date=2026-08-22T05:31:25.5320000Z
    Msg=The time service is now synchronizing the system time with the time source time.windows.com,0x9 (ntp.m|0x9|0.0.0.0:123->52.231.114.183:123) with reference id 3077760820. Current local stratum number is 5.
  new-en:   EventID=35 Provider=Microsoft-Windows-Time-Service
    Time=2026-08-22 05:31:25.532021
    Msg=The time service is now synchronizing the system time with the time source time.windows.com,0x9 (ntp.m|0x9|0.0.0.0:123->52.231.114.183:123) with reference id 0xb772e734. Current local stratum number is 5.

### 118. [D] System / Microsoft-Windows-Kernel-General / EID=1（共 26 条，真渲染分歧待查）

首条索引：System #714

[System #714] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=1 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-22T05:31:25.5330000Z
    Msg=The system time has changed to 2026-08-21T21:31:25.532381200Z from 2026-08-21T21:31:25.531995100Z. Time Delta: 0 ms Change Reason: An application or system component changed the time. Process: '\Device\HarddiskVolume4\Windows\System32\svchost.exe' (PID 31464). RTC time: 2026-08-22T05:31:25.532381200Z Current time zone bias: -480 RTC time is in UTC: false System time was based on RTC time: false
  new-en:   EventID=1 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-22 05:31:25.533742
    Msg=The system time has changed to 2026-08-21T21:31:25.532381Z from 2026-08-21T21:31:25.531995Z. Time Delta: 0 ms Change Reason: 1. Process: '\Device\HarddiskVolume4\Windows\System32\svchost.exe' (PID 31464). RTC time: 2026-08-22T05:31:25.532381Z Current time zone bias: -480 RTC time is in UTC: System time was based on RTC time:

### 119. [B] System / Virtual Disk Service / EID=3（共 1 条，裸参数未渲染）

首条索引：System #789

[System #789] B-裸参数未渲染（见明细）
  baseline: EventID=3 Provider=Virtual Disk Service
    Date=2026-08-23T18:42:09.0050000Z
    Msg=Service started.
  new-en:   EventID=3 Provider=Virtual Disk Service
    Time=2026-08-23 18:42:09.005012
    Msg=@2010005;;

### 120. [B] System / Virtual Disk Service / EID=4（共 1 条，裸参数未渲染）

首条索引：System #797

[System #797] B-裸参数未渲染（见明细）
  baseline: EventID=4 Provider=Virtual Disk Service
    Date=2026-08-23T18:56:54.6500000Z
    Msg=Service stopped.
  new-en:   EventID=4 Provider=Virtual Disk Service
    Time=2026-08-23 18:56:54.650059
    Msg=@2010001;;

### 121. [B] System / NetBT / EID=4311（共 4 条，裸参数未渲染）

首条索引：System #842

[System #842] B-裸参数未渲染（见明细）
  baseline: EventID=4311 Provider=NetBT
    Date=2026-08-24T09:33:03.8110000Z
    Msg=Initialization failed because the driver device could not be created. Use the string "000000000100320000000000D710004013010000250200C000000000000000000000000000000000" to identify the interface for which initialization failed. It represents the MAC address of the failed interface or the Globally Unique Interface Identifier (GUID) if NetBT was unable to map from GUID to MAC address. If neither the MAC address nor the GUID were available, the string represents a cluster device name.
  new-en:   EventID=4311 Provider=NetBT
    Time=2026-08-24 09:33:03.811085
    Msg=;000000000100320000000000D710004013010000250200C000000000000000000000000000000000;;

### 122. [D] System / Microsoft-Windows-Kernel-General / EID=11（共 9 条，真渲染分歧待查）

首条索引：System #860

[System #860] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=11 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-24T12:27:27.5310000Z
    Msg=TxR init phase for hive \\GLOBALROOT\Device\HarddiskVolumeShadowCopy3\WINDOWS\System32\config\SYSTEM (TM: {ea01ca89-9ef4-11f1-a4ef-244bfe8eb88e}, RM: {ea01ca88-9ef4-11f1-a4ef-244bfe8eb88e}) finished with result=0xC00000A2 (Internal code=7).
  new-en:   EventID=11 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-24 12:27:27.531382
    Msg=TxR init phase for hive \??\GLOBALROOT\Device\HarddiskVolumeShadowCopy3\WINDOWS\System32\config\SYSTEM (TM: EA01CA89-9EF4-11F1-A4EF-244BFE8EB88E, RM: EA01CA88-9EF4-11F1-A4EF-244BFE8EB88E) finished with result=0xC00000A2 (Internal code=7).

### 123. [D] System / Microsoft-Windows-DriverFrameworks-UserMode / EID=10002（共 1 条，真渲染分歧待查）

首条索引：System #898

[System #898] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=10002 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Date=2026-08-24T16:35:37.7430000Z
    Msg=The UMDF service WpdFs (CLSID {112de495-ac4c-46f8-b663-6a4266c53313}) was upgraded. It requires framework version 2.35.0 or higher.
  new-en:   EventID=10002 Provider=Microsoft-Windows-DriverFrameworks-UserMode
    Time=2026-08-24 16:35:37.743316
    Msg=The UMDF service WpdFs (CLSID 112DE495-AC4C-46F8-B663-6A4266C53313) was upgraded. It requires framework version 2.35.0 or higher.

### 124. [D] System / disk / EID=51（共 1 条，真渲染分歧待查）

首条索引：System #907

[System #907] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=51 Provider=disk
    Date=2026-08-24T16:37:30.5090000Z
    Msg=An error was detected on device \Device\Harddisk4\DR5 during a paging operation.
  new-en:   EventID=51 Provider=disk
    Time=2026-08-24 16:37:30.509192
    Msg=\Device\Harddisk4\DR5;;;030080000100000000000000330004802D010000130000C0000000000000000000784100000000004422160100000000FFFFFFFF01000000580000C40200000000200A124002204000000000140000000000000000000000E85E83A885C3FFFF00000000000000001080A05785C3FFFF1010839385C3FFFFBC200000000000002800000020BC00000800000000000000700002000000000A000000003A0000000000000000000000;;

### 125. [D] System / Microsoft-Windows-Kernel-General / EID=15（共 2 条，疑似compare-?清洗artefact）

首条索引：System #919

[System #919] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=15 Provider=Microsoft-Windows-Kernel-General
    Date=2026-08-24T18:25:08.0090000Z
    Msg=Hive \\C:\WINDOWS\AppCompat\Programs\Amcache.hve was reorganized with a starting size of 11051008 bytes and an ending size of 9228288 bytes.
  new-en:   EventID=15 Provider=Microsoft-Windows-Kernel-General
    Time=2026-08-24 18:25:08.009245
    Msg=Hive \??\C:\WINDOWS\AppCompat\Programs\Amcache.hve was reorganized with a starting size of 11051008 bytes and an ending size of 9228288 bytes.

### 126. [D] System / LsaSrv / EID=6041（共 1 条，疑似compare-?清洗artefact）

首条索引：System #1020

[System #1020] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=6041 Provider=LsaSrv
    Date=2026-08-25T17:26:58.4110000Z
    Msg=A CredSSP authentication to TERMSRV/192.168.81.75 failed to negotiate a common protocol version. The remote host offered version 2 which is not permitted by Encryption Oracle Remediation. See https://go.microsoft.com/fwlink/linkid=866660 for more information.
  new-en:   EventID=6041 Provider=LsaSrv
    Time=2026-08-25 17:26:58.411483
    Msg=A CredSSP authentication to TERMSRV/192.168.81.75 failed to negotiate a common protocol version. The remote host offered version 2 which is not permitted by Encryption Oracle Remediation. See https://go.microsoft.com/fwlink/?linkid=866660 for more information.

### 127. [B] System / disk / EID=157（共 1 条，裸参数未渲染）

首条索引：System #1095

[System #1095] B-裸参数未渲染（见明细）
  baseline: EventID=157 Provider=disk
    Date=2026-08-26T10:12:00.3230000Z
    Msg=Disk 3 has been surprise removed.
  new-en:   EventID=157 Provider=disk
    Time=2026-08-26 10:12:00.323306
    Msg=\Device\Harddisk3\DR3;;3;;;0000000002003000000000009D000480000000000000000000000000000000000000000000000000;;

### 128. [D] System / Microsoft-Windows-Ntfs / EID=140（共 1 条，真渲染分歧待查）

首条索引：System #1502

[System #1502] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=140 Provider=Microsoft-Windows-Ntfs
    Date=2026-08-29T05:31:50.7470000Z
    Msg=The system failed to flush data to the transaction log. Corruption may occur in VolumeId: 系统, DeviceName: \Device\HarddiskVolumeShadowCopy9. Failure status: A device which does not exist was specified. Device GUID: {00000000-0000-0000-0000-000000000000} Device manufacturer: Device model: Device revision: Device serial number: Bus type: <unknown> Adapter serial number
  new-en:   EventID=140 Provider=Microsoft-Windows-Ntfs
    Time=2026-08-29 05:31:50.747352
    Msg=The system failed to flush data to the transaction log. Corruption may occur in VolumeId: 系统, DeviceName: \Device\HarddiskVolumeShadowCopy9. Failure status: 0xc000000e Device GUID: {00000000-0000-0000-0000-000000000000} Device manufacturer: Device model: Device revision: Device serial number: Bus type: 0 Adapter serial number:

### 129. [B] System / Ntfs / EID=50（共 515 条，裸参数未渲染）

首条索引：System #1503

[System #1503] B-裸参数未渲染（见明细）
  baseline: EventID=50 Provider=Ntfs
    Date=2026-08-29T05:31:50.7470000Z
    Msg={Delayed Write Failed} Windows was unable to save all the data for the file \Device\HarddiskVolumeShadowCopy9\$Mft. The data has been lost. This error may be caused by a failure of your computer hardware or network connection. Please try to save this file elsewhere.
  new-en:   EventID=50 Provider=Ntfs
    Time=2026-08-29 05:31:50.747478
    Msg=;;\Device\HarddiskVolumeShadowCopy9\$Mft;;;04000400020030000000000032000480000000000E0000C0000000000000000000000000000000000E0000C0;;

### 130. [D] System / Microsoft-Windows-Wininit / EID=26（共 1 条，真渲染分歧待查）

首条索引：System #2244

[System #2244] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=26 Provider=Microsoft-Windows-Wininit
    Date=2026-08-31T02:07:18.5650000Z
    Msg=Boot App Anti-Rollback: Boot.stl Enforcement completed with status: HRESULT: The operation completed successfully. Boot Stl Enforced Successfully: true WNF Published with result: STATUS_SUCCESS
  new-en:   EventID=26 Provider=Microsoft-Windows-Wininit
    Time=2026-08-31 02:07:18.565269
    Msg=Boot App Anti-Rollback: Boot.stl Enforcement completed with status: HRESULT: 0 Boot Stl Enforced Successfully: WNF Published with result: 0x0

### 131. [D] System / Service Control Manager / EID=7038（共 4 条，真渲染分歧待查）

首条索引：System #2328

[System #2328] D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）（见明细）
  baseline: EventID=7038 Provider=Service Control Manager
    Date=2026-09-01T05:32:49.5540000Z
    Msg=The WerSvc service was unable to log on as NT AUTHORITY\SYSTEM with the currently configured password due to the following error: Not enough memory resources are available to process this command. To ensure that the service is configured properly, use the Services snap-in in Microsoft Management Console (MMC).
  new-en:   EventID=7038 Provider=Service Control Manager
    Time=2026-09-01 05:32:49.554953
    Msg=The WerSvc service was unable to log on as NT AUTHORITY\SYSTEM with the currently configured password due to the following error: %8 To ensure that the service is configured properly, use the Services snap-in in Microsoft Management Console (MMC).
