# Phase C 报告（2026-09-14 23:10）

## 结论

- **定位到源**：EventLog 6013 的消息源为 **`%SystemRoot%\System32\netevent.dll` 的 MUI 卫星
  `C:\Windows\System32\en-US\netevent.dll.mui`**（zh-CN 卫星同构），资源类型 **RT_MESSAGETABLE（type 11）**，
  消息 ID **`0x8000177D`**（低 16 位 0x177D = 6013），文本
  `The system uptime is %5 seconds.\r\n`（zh-CN：`系统启动时间为 %5 秒。\r\n`）。
  与基线渲染逐字符一致；先前"netevent.dll MUI 已实证排除"的结论系**误排除**（见未决-坑位1）。
  关键旁证：live `Get-WinEvent` 地面真值渲染 = "系统启动时间为 353288 秒。"，事件共 **8 个参数槽
  （前 4 槽为空串，第 5 槽 = uptime 秒数）**，与 `%5` 精确对应。
- 豁免登记状态：EventLog/6013×16 维持登记（根因码 SRC，条件性豁免）；其关闭条件"定位到源"
  **本阶段已满足** → 按计划转 §D 接入，接入落地后摘除白名单。
- 门：G1 ✓（M1 97.5641% ≥ 97.53%；M2 98.85% ≥ 80%） G2 ✓（Security=0、Setup=0、四通道 B/C
  零新增、无缺失；acceptance-check --prev 全门 PASS，exit=0。本阶段零代码影响，全绿符合预期）

## 完成

- **步骤 1（注册表静态源扫描）——命中，停在此步**：复用上轮遗留的
  `evtx-parity-test/scripts/probe-6013-source-scan.py`（修复可用，仅修 msgtable 脚本一处 list.decode
  bug）。枚举 `HKLM\SYSTEM\CCS\Services\EventLog\System` 全部源的 EventMessageFile（498 个文件，
  含 en-US/zh-CN MUI 卫星），raw 扫描 "uptime"（ASCII+UTF-16LE，大小写不敏感）→ 20 个文件命中；
  其中 `en-US\netevent.dll.mui` 命中后用 `probe-6013-msgtable.py`（RT_MESSAGETABLE 解码）精确定位
  `0x8000177D`。注册表同源确认：`...\EventLog\System\EventLog` 的 EventMessageFile =
  `%SystemRoot%\System32\netevent.dll`。
- 交叉验证（runbook §4/§6.1 取证项）：
  1. `crim-probe-intype.exe DUMP_STRINGS=1` 跑 netevent.dll.mui → "uptime" **0 命中**，复现先前误排除；
  2. 主 `netevent.dll` 本体无 RT_MESSAGETABLE（消息全在 MUI 卫星，618 条）；
  3. 同表邻近 ID：6005=0x1775、6006=0x1776、6008=0x1778、6009=0x1779、**无 0x1777(6011)**；
  4. `probe-6013-live.ps1`（Get-WinEvent 地面真值）：6013 渲染 "系统启动时间为 353288 秒。"，
     Param1-4=空、Param5=353288、Param6=60、Param7="-480 中国标准时间"、Param8=Byte[]；
  5. zh-CN 卫星同表同 ID 中文文本确认。
- 步骤 2（全盘 message table 扫描）、步骤 3（WebSearch 在线对照）：**未执行**——步骤 1 命中即停，
  地面真值已闭环，无需外部对照。

## 登记/转出

- **6013×16 → 定位到源，转 §D 接入**（登记表条目保持不变，状态由"登记未命中(条件性)"改为
  "关闭条件已满足、待 §D 接入后摘除"）：
  - 源 DLL：`C:\Windows\System32\netevent.dll`（本体无消息表）→ 消息在
    `System32\en-US\netevent.dll.mui` / `System32\zh-CN\netevent.dll.mui` 的 RT_MESSAGETABLE（type 11）；
  - 消息表索引/ID：`0x8000177D`（EventID 6013），en-US `The system uptime is %5 seconds.`，
    zh-CN `系统启动时间为 %5 秒。`；
  - 建议接入方式：msgdb-dump 现有 `dump/builtin_messages.go` 的 6013 硬编码文本是错的
    （见未决-坑位2）。最小修复 = 将 en-US/zh-CN 6013 文本改为上表真实文本；同时渲染/萃取层
    需保证**空参数槽不丢弃**（%5 依赖前 4 个空槽占位；我方 raw 只见 4 参数
    `23/60/-480/hex` 即空槽被丢弃的形态）。若渲染层暂不支持空槽，勿以"%1 形式的 6013 文本"
    固化巧合（丢弃空槽后 %1=23 恰好渲染正确文本，但这是脆弱巧合）。

## 未决与建议

- **坑位 1（误排除根因）**：crim-probe 的 `DUMP_STRINGS` 模式只 dump 0xDn **字符串表（RT_STRING，
  type 6）**，不覆盖 **RT_MESSAGETABLE（type 11）**——6013 文本恰在消息表里，故先前用
  DUMP_STRINGS 检查 netevent.dll.mui 得出"无文本"的错误排除。给后续探针的教训：
  查 classic provider 消息必查 type 11，`probe-6013-msgtable.py` 已入库可复用。
- **坑位 2（错误硬编码）**：`msgdb-dump/dump/builtin_messages.go` 的 `wellKnownEventLogMessages()`
  硬编码了 6013 = "Windows has shut down or restarted. The system was last started on %1."（错误），
  且 6011 = "The system uptime is %1 seconds."（真实表中**不存在** 6011 条目，此文本是真实 6013
  的 %1 变体，系张冠李戴）；注释声称 "cannot be extracted from any message DLL"——已被本次定位
  证伪。6005/6006/6008 与真实表一致；6009 真实文本带句点（`%3 %4.`）内置缺句点（6009 当前
  matched，勿动）。§D 建议顺带以真实表核对 6011 条目的去留。
- **参数形态链**：live 事件 8 槽（4 空 + uptime@5 + 60 + 时区 + Byte[]）；我方 raw 4 参数
  `23/60/-480中国标准时间/hex` 与"丢弃空槽"推断自洽（23=uptime 秒）。§D 接入时先用
  baseline 6013 事件的原始 XML 再实证一次空槽形态，再决定修萃取层还是渲染层。
- 给 §D 的提示：接入后跑 03→05→06 全链（msgdb-dump 改动触发 03 重建）；摘除 6013 白名单前
  先跑 `--no-exemptions` 复核口径确认 diff 773→757。

## 变更文件

- `evtx-parity-test/scripts/probe-6013-msgtable.py`（复用上轮遗留；修复 `ansi_z` list.decode bug）
- `evtx-parity-test/scripts/probe-6013-source-scan.py`（复用上轮遗留，未改）
- `evtx-parity-test/scripts/probe-6013-live.ps1`（新增：Get-WinEvent 6013 地面真值探针）
- `docs/phase-reports/phase-C-report.md`（本报告）

（jj change id：zswxsmpn）
