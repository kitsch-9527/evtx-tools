# night-P3 报告：hex 格式族（Binary→%n+1）—— diff 82→44

日期：2026-09-30 凌晨。时间盒 60 分钟，实际约 55 分钟（取证约 30 分 + TDD/闭环约 25 分）。

## 结论

| 指标 | 前 | 后 |
|---|---|---|
| diff 总数 | 82（System 68 + App 14） | **44（System 30 + App 14）**，只减不增（-38） |
| NetBT 4311 | 38 | **0（清零）** |
| matched | 44790/44890 | 44828/44890（M1=99.8619%） |
| M2 D 闭环率 | 76.44% | **87.36%**（硬门 80%，转达标） |

红线逐项（acceptance-check --prev 全 PASS）：
- [G0] baseline_lang zh-CN→zh-CN ✓
- [G1] M1 99.8619% ≥ 97.53% 下限 ✓；M2 87.36% ≥ 80% 硬门 ✓
- [G2] Security diff=0 ✓；Setup diff=0 ✓；四通道 B/C=0（0→0）✓；无缺失类 ✓
- [G3] 豁免命中 5 项/18 条与登记表一致 ✓（DSAM 16962×8、WPD 24577/24579×8、SPP 1004×1、CAPI2 4111×1）
- 单测：Rust 238 绿（233+5 新增，先红 3 后绿 5）；Go 135 绿（-count=1 实测）

硬性验收「≥1 个目标簇清零」：NetBT 4311 38→0 达成。

## 取证（证据先行）

1. 对齐 diff 样本（importlib 复用 06-compare.py 的 parse_baseline/load_new_csv/cmp_pair）：
   - NetBT 4311×38：基线 `Use the string "0000000001003200…0000" to identify…`，我方 `Use the string "" …`（首分歧=引号内参数为空）。
   - e1dexpress 32×8：基线 `I219-V;; 网络链接已以 1Gbps 全双工建立。`，我方 `I219-V Se ha establecido el enlace de red en 1 Gbps…`（**西语**）。
   - VMnetAdapter 36×8：基线 `() Starting up the Adapter Driver…`，我方 `一个用户已达到卷 上的配额阈值。`（**配额文案=另一条消息**）。
2. raw XML（wevtutil /f:xml 对 baseline-evtx/System.evtx）：
   - NetBT 4311：`<EventData><Data></Data><Binary>0000000001003200…0000</Binary></EventData>`（VMnetAdapter 36 同构；e1dexpress 32 为 `<Data/>+<Data>NIC名</Data>+<Binary/>`）。
   - wevtutil /f:text 实测渲染：NetBT %1=""（空 Data），%2=Binary 大写 hex（无 0x、无分隔、原样长度）。
3. msgdb 模板（SQL 单点探针）：
   - NetBT 4311：`…Use the string "%2" to identify…`（仅 %2）→ Binary hex 必须落 %2 ⇒ **规则：Data 值占 %1..%n，Binary hex 追加为 %n+1**。e1dexpress 32 模板 `%2%n\r\nSe ha…`（%3=Binary 未引用）同构佐证。
   - e1dexpress 32：msgdb **zh-CN 与 en-US 行均为西语文案** ⇒ 萃取侧错拿语言 stringTable，非渲染格式问题。
   - VMnetAdapter 36：msgdb 模板=`A user hit their quota threshold on volume %2.`（我方渲染其实"忠于"该模板），而 wevtapi 实际渲染 `(%1) Starting up…` ⇒ 萃取侧错库（VMware 驱动族消息表混串），非渲染格式问题。
   - 简报"剩余差异在参数格式"假设：对 NetBT 成立；对 e1dexpress（错语言）/VMnetAdapter（错模板）**证伪**——取证阶段排除，未改代码。
4. 回归面排查（防伤已修簇）：全 corpus Binary 事件 System 1017 + App 580 条，按 (provider,eid) 分 50 簇，逐一对照 msgdb 模板 max_param 与 Data 个数：**仅 NetBT 4311 与 VMnetAdapter 36 模板引用 %n+1 槽位**，其余 48 簇 max_param≤data（追加参数对未引用模板无输出差；substitute() 对越界参数空替换）。TS35/WER 等 manifest 事件走扁平对象路径，本次改动仅限 classic Data 分支，构造性隔离。
5. format_for_display 无害性：80 位 hex blob 无 0x 前缀、无分隔；首段 24 位数字 u64 解析溢出不转换、末段 32 个 0 值<0x80000000 不转换、中段两侧邻 hex 字符进 GUID 上下文不转换——原样透传。

## 完成（改动文件）

- `evtx-tools/evtx-tools-cli/src/cmd_parse.rs`：
  - `extract_subst_params` 拆为 wrapper（gate off，语义不变）+ `extract_subst_params_ext(…, include_binary: bool)`；
  - Strategy 1.5（Data.#text）与 Strategy 1.75（Data 数组定位分支）在 include_binary 且 Binary 非空字符串时把 hex 追加为末位参数（%n+1）；
  - 模板命中路径调用点传 `true`；miss fallback 路径（evtx2csv ;;-join 兼容）保持 `false` 不动。
  - 新增 5 个单测（append on/off、Data 数组形态、null/空 Binary、未引用槽位不挤占）；红→绿过程实测（先 3 失败，实现后 5 通过）。
- 无 DB/萃取端改动。

## 登记-转出

| 簇 | 条数 | 判定 | 转出理由与下一步 |
|---|---|---|---|
| e1dexpress 32 | 8 | 萃取侧错语言（msgdb zh-CN/en-US 行均存西语 `Se ha establecido…`；wevtapi 实际渲染中文 `网络链接已以 1Gbps 全双工建立。`） | 萃取端按注册表 Provider MessageFile 重新定位 e1dexpress 的 MUI/DriverStore 载体，修 zh stringTable 选择；渲染层无可修点 |
| VMnetAdapter 36 | 8 | 萃取侧错库（msgdb 该行是配额文案 `%2`，wevtapi 实际渲染 `(%1) Starting up the Adapter Driver…`） | VMware 驱动族（vmnetadapter/vmx86/VMnetuserif…）消息表归属需按注册表 EventMessageFile 逐 provider 重验；本次修复后该簇内容变化（%2 落 hex）但条数不变（8→8） |

## 未决与建议

- System 30 残留：ResExhaustion 2004×9（P4）、DSAM 16962×8/WPD×8（豁免表内）、EventLog 6008×2、SCM 7023×2、Wininit 23×1（P6 小簇）；App 14（WER×2、Chrome/Edge×10、Codex×2）＝P5/P6。
- EventLog 6008 与 SCM 7023 亦含 Binary 但模板 max_param≤data，与本修复无关，归 P6 时再取证。
- VMnetAdapter 修复若日后萃取纠正，需复查 %n+1 追加规则下 hex 落位是否仍与 wevtapi 一致。
- 未提交（按简报由编排者统一 jj 提交）。
