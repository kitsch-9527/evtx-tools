# 编排者提示词 — 夜间计划 2026-09-20（ZCode 入口）

> 用法：**2026-09-20 23:00** 在 ZCode 新会话中，从下方分隔线起**原样粘贴**到文件末尾。
> 该提示词自包含：新会话只需读它 + 它指向的两个文件即可驱动全流程。
> 心跳模式则把本档内容作为每次心跳的会话提示词（见 §心跳）。

---

# 任务：执行《夜间持续执行计划 2026-09-20 23:00 → 2026-09-21 09:00》（编排者角色）

你是**编排者**。职责是驱动 `docs/overnight-plan-2026-09-20.md` 定义的阶段
（N0–N9）依次完成并验收。你**不亲自执行阶段任务**——每阶段派发一个子代理（全新上下文），
你只做派发、核对、状态落盘。工作目录 `E:\Code\evet`。

## 第一步：启动核对（≤20 分钟，即 N0）

1. `date '+%F %T'` 记录启动时间；确认已在 `E:\Code\evet`。
2. **完成哨兵**：若 `docs/phase-reports/overnight-2026-09-20/FINAL.md` 已存在 → 本轮已完结，直接退出。
3. **并发锁**：`docs/phase-reports/overnight-2026-09-20/.lock` 存在且 mtime <90min → 另有会话在跑，退出；
   过期/残留 → 删旧领新。**锁文件不入库，每次 commit 前删除。**
4. **刷新 prev**：`cd evtx-parity-test; cp report.json report.prev.json`（当前 prev 是旧快照 799）。
5. **基线复核**：读 `evtx-parity-test/report.json`，确认
   compared=32801 / matched=32020 / diff=771 / Security=0 / Setup=0 / Application B74 C11 D2 / System B644 C40。
   不符 → 跑 `python scripts/06-compare.py` 复现；仍不符 → **停止并报告**，勿盲跑。
6. **通读计划**：`docs/overnight-plan-2026-09-20.md` 全文（操作手册：起点 recon、红线、
   命令与坑位、阶段总表、各阶段简报、心跳协议、报告模板、派发模板）。
7. **建目录**：`docs/phase-reports/overnight-2026-09-20/`；写 `N0-readiness.md`（1–6 实测值）。

## 第二步：阶段派发循环（按 N1→N2→N3→N4→N5→N6→N7→N8→N9 顺序）

对每个未完成阶段 X（判据：`docs/phase-reports/overnight-2026-09-20/night-N<X>-report.md` 不存在）：

1. **派发**：用 Agent 工具派发子代理，提示词 = 计划 `§N<X>` 全文 + 计划 §7 模板头部。
   若工具支持 `model` 参数则指定 `glm-5.3-flash`；否则在提示词首行声明模型并保持继承。
   提示词必须含：必读清单、禁读清单、时间盒、报告与 jj commit 要求、退出条件。
2. **核对**（子代理返回后）：
   - 报告存在且模板字段完整；
   - 阶段数字验收达成（计划 §4 总表"数字验收"列）；
   - `python scripts/acceptance-check.py --prev report.prev.json` 无 G1/G2 FAIL；
   - jj 有该阶段提交。
3. **处置**：任一不满足 → 按该阶段退出条件处理（回退 / 登记 / 重派一次）；重派仍失败 → 登记转后续，进入下一阶段。
   全部满足 → 下一阶段。

## 硬规则（违反即任务失败）

- **红线一票否决**：任何阶段后 Security/Setup diff>0、四通道 B/C 类新增、
  出现"缺失"类 diff → `jj undo` 回退该阶段提交 → 重派一次 → 仍失败登记转后续。
- **编排者禁读**：`report.diff` 全文、`evtx-parity-test/msgdb|new-parsed` 下 DB、
  handoff 全文。核对只看：阶段报告、`report.json` 的 diff/matched/by_channel/categories、
  `acceptance-check.py` 输出。
- **取证优先**：未探针实证不得改代码（阶段 A 的两个根因假设曾被实测证伪）。
- **TDD**：任何修复先加失败单测再改实现；改完 `cargo test --workspace` 与
  `go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1` 必须全绿。
- **时间盒**：阶段超时按退出条件收尾；总超时优先裁剪 N5/N6 的"落地"降级为"登记"。
- **每阶段必须产出**：阶段报告 + jj 提交 + 验收数字。三者缺一不算完成。
- **顺序依赖**：N3 的取证结论决定 N4 的修复走向；N7 必须在全部实现阶段之后；N8 必须在 N7 全绿之后。
- **EXEMPTIONS 双表同步**：`06-compare.py` 与 `scripts/acceptance-check.py` 的豁免表
  必须同一提交改动；登记表唯一真源 `docs/residual-zeroing-plan-2026-09-07.md` §4。

## 墙钟纪律

- ≥07:30 只允许 ≤30min 的收尾动作。
- ≥08:00 **不开新阶段**，只做 N8/N9。
- ≥09:00 一律停止，确保磁盘状态一致（报告 + 提交落盘）。

## 收尾（N8/N9）

1. 汇总各阶段报告 → 本轮最终执行报告：起止时间、各阶段前后数字、终态
   diff/matched/M1/M2/D 残留/豁免命中、红线复核结论、后续专项清单。
2. 确认最终 jj 提交包含：各阶段报告、skill 三镜像、STATUS、README 修正、登记表、FINAL.md。
3. 写 `docs/phase-reports/overnight-2026-09-20/FINAL.md`（完成哨兵）。
4. 回报用户：终态数字、完成/登记清单、需人工跟进的事项。

## 心跳模式（若配置 ZCode 定时自动化）

- **cron**：`*/20 23,0-8 * * *`（23:00–08:59 每 20 分钟）。
- 每次心跳：完成哨兵检查 → 并发锁检查（90min 过期）→ 领锁 → 取第一个缺报告阶段 → 派发**恰好一个**子代理
  → 核对 → 提交 + 报告落盘 → 删锁 → 退出。**一轮只做一个阶段。**
- 状态只认磁盘（阶段报告 + jj 提交），不依赖会话记忆；限流时等 5min 重试至多 3 次，仍失败删锁退出，下次心跳续跑。
- 用户侧前提：机器不休眠、ZCode 运行、工作区打开、工具权限自动放行、删除旧 `UIEOrchestrator` 任务。

## 状态锚点（2026-09-20 现场实测，续跑以磁盘报告与 report.json 为准）

- 起点：diff=771、matched=32020/32801、M1=97.6190%、M2=99.43%、exempted=6 项/10 条、M5=761。
- 簇分解：**B718**（Ntfs 50 ×515 为最大；hcmon 89；WXWorkUpgrader 27；asus 16；Win32k 267 14；
  igfxCUIService 9；PDFShellExt 9；其余 17 簇 39）、**C51**（CfgChangeMonitor 1 ×40；第三方服务源 11）、
  **D2**（WER 1001 ×2）。
- 根因 recon 已写入计划 §0（经典源消息表未落库 / N/A 精度缺口 / WER hex 化）——**子代理先读 §0，勿重复 recon**。
- 已知坑位：03 重建期间禁止并发 05；03 输出 GBK；report.diff 每条 Msg 截断 500 字；
  `report.prev.json` 必须由 N0 刷新；改簇前先跑 dry-run 探针。

（编排者提示词结束）
