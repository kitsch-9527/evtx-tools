# 校验报告：本机真实数据重跑 01→06（2026-09-29）——语言漂移导致闭环口径断裂

> 任务：以本机当前真实数据为基准，校验 Lua 基线解析（evtx2text.lua，路径 A 真值）。
> 结论先行：**Lua 解析器本身完好无损**；崩塌根因是**本机显示语言在 09-15 之后被切为 zh-CN**，
> Lua 真值随 wevtutil 渲染语言翻成中文，而 06 主比对（基线 vs new-parsed/**en**）假设英文基线。

## 1. 重跑记录

| 步 | 结果 |
|---|---|
| 01 导出 | Application/Setup/System 非提权导出；Security 走 UAC 提权（新增 `scripts/_01-security-elevated.ps1`）。**坑：`wevtutil epl` 不覆盖已存在文件，重导前必须先删**（09-03 首导后此坑一直潜伏） |
| 02 Lua 解析 | 4/4 通道成功；每通道事件数与 B 路完全相等（10742/676/27366/6106），顺序对齐 44890/44890 无缺失 |
| 03 msgdb | 重建成功（本机漂移实证：eventlog_sources 1272→**1273**） |
| 04 evtx-tools | release 构建绿（源码自 phase H 未变） |
| 05 new-parse | en/zh 双路全量 |
| 06 比对 | compared=44890 matched=**1840** diff=42879 verdict=DIFF |

## 2. 崩塌数字与语言证据

report.json（vs en 主比对）：Application 1152/10742、Setup **676/676 全绿**、Security **0/27366**、System 12/6106。

- 基线消息现为中文（"已成功登录账户。"、"Level: 信息"）；diff 样本 new-en 侧为英文。
- `Get-WinUILanguageOverride` = **zh-Hans-CN**（用户在系统设置里显式选择中文）；MUI 同时装有 en-US/zh-CN。
- 09-15 全绿 ⇒ 当时基线为英文；旁证：overnight-plan §0 旧基线形态 Microsoft 组件为英文（"Delayed Write Failed"），
  仅中文 ISV（WXWork"操作成功完成。"）为中文。
- **Setup 全绿的机理自洽**：Setup 通道的 provider 无 zh 资源，EvtFormatMessage 回退 en——语言翻转前后都渲染英文，
  主比对不受影响。翻转只影响有 zh 资源的 provider（全部 Windows 内置通道）。

## 3. 假设口径探针：基线 vs new-parsed/**zh**

用 importlib 复用 06 解析函数实测（06 未改动）：

| 通道 | matched/aligned | 比率 |
|---|---|---|
| Application | 10429/10742 | 97.09% |
| Setup | 676/676 | 100% |
| Security | 17633/27366 | 64.43% |
| System | 4843/6106 | 79.32% |
| **全量** | **33581/44890** | **74.81%** |

Security 9733 条 D 失配为**单一族**：模板中文一致，但**枚举值映射保持英文**——
基线"虚拟账户: 否 / 提升的令牌: 是 / 模拟级别: 模拟" vs 我方 zh 侧 "No / Yes / Impersonation"。
即 msgdb 的 value_maps（0xDn 字符串表）只落了 en 语言，zh 渲染路径未本地化。
System 缺口为 DB 侧 zh 变体覆盖缺失（EventLog 6009 走裸参数 fallback）。

## 4. 可选路径（待定，属项目方向决策）

- **A. 主比对翻到 zh**（最忠实"本机运行时真值"）：改 06 主比对字段（比对层，§2.2 只重跑 06），
  随后专项修 B 路 zh 缺口（value_maps 按语言落库 + zh 变体覆盖）。历史 M1/M2 全部重基线。
- **B. 本机显示语言切回 en-US**：重跑 01/02/05/06 即恢复 09-15 口径可比性，零代码改动；但真值绑在机器设置上。
- **C. 把基线语言钉死 en**：需改 evtx2text.lua 或其渲染调用，违反"原脚本不动"，且 wevtutil 无 locale 参数——不推荐。

## 5. 本次顺手产出

- `scripts/_01-elevated-run.ps1` / `scripts/_01-security-elevated.ps1`：提权导出包装（纯 ASCII；
  中文注释无 BOM UTF-8 会被 PS5.1 按 GBK 读入吞换行——坑位 2 同源）。
- 现存 report.json/report.diff/report.md 为 09-29 语言崩塌态，留作证据；未做任何提交。
