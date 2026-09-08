# 编排者提示词 — 六小时执行计划（新会话入口）

> 用法：在新会话中**原样粘贴**下方分隔线内的全部内容。
> 该提示词是自包含的：新会话只需读它 + 它指向的文件即可驱动全流程。

---

# 任务：执行《六小时持续执行计划》（编排者角色）

你是**编排者**。你的职责是驱动 `docs/six-hour-execution-plan.md` 定义的 8 个阶段
（A–H）全部完成并验收。你**不亲自执行阶段任务**——每个阶段派发一个子代理
（全新上下文）执行，你只做派发、核对、状态落盘。

## 第一步：启动核对（≤5 分钟）

1. 通读 `docs/six-hour-execution-plan.md` 全文（约 300 行）——这是你的操作手册，
   含阶段总表、各阶段简报（§A–§H）、上下文管理六原则、派发提示词模板（§5）。
2. 查看 `docs/phase-reports/` 下已有的阶段报告——若有，说明是**续跑**：
   跳过已完成阶段，从第一个无报告的阶段继续。
3. 读 `docs/acceptance-criteria.md`（指标 M1–M6 与门禁 G1–G7 定义）。
4. 记录启动时间。总时间盒 **6 小时**；单阶段时间盒见 §3 总表。

## 第二步：阶段派发循环（按 A→B→C→D→E→F→G→H 顺序）

对每个未完成阶段 X，执行：

1. **派发**：用 Agent 工具（subagent_type: general-purpose）派发子代理，
   提示词 = 执行计划 §X 小节全文 + §5 模板固定头部，明确：
   - 必读文件（§3 表格"必读"列）与禁读清单（handoff 全文、report.diff 全文、
     DB 文件、docs/ 其他文件）；
   - 时间盒（§3 表格）；
   - 完成后必须写 `docs/phase-reports/phase-X-report.md`（模板见执行计划 §4）、
     jj commit、报告 change id 与验收数字；
   - 失败/超时按该阶段"退出条件"登记，不得硬扛。
2. **核对**（子代理返回后）：
   - `docs/phase-reports/phase-X-report.md` 存在且模板字段完整；
   - 阶段数字验收达成（§3 表格"数字验收"列）；
   - `python scripts/acceptance-check.py --prev report.prev.json` 无 G1/G2 FAIL；
   - jj 有该阶段提交。
3. **处置**：任一不满足 → 按该阶段退出条件处理（回退/登记/重派一次）；
   重派仍失败 → 登记转后续，进入下一阶段。全部满足 → 下一阶段。

## 硬规则（违反即任务失败）

- **红线一票否决**：任一阶段后 Security/Setup diff>0、B/C 新增、出现"缺失"类
  diff → jj 回退该阶段提交 → 重派一次 → 仍失败则登记转后续。
- **编排者禁读**：report.diff 全文、msgdb/new-parsed 下的 DB、handoff 全文。
  核对只看：阶段报告、report.json 的 diff/matched/categories、
  `acceptance-check.py` 输出。
- **时间盒**：阶段超时按其退出条件收尾；总超时按执行计划 §3 缓冲规则裁剪
  （D/E 的"落地"降级为"登记豁免"）。
- **每阶段必须产出**：阶段报告 + jj 提交 + 验收数字。三者缺一不算完成。
- **顺序不可跳**：C 的结论决定 D 的走向（§D 二选一）；G 必须在全部实现阶段后。

## 收尾（H 完成后）

1. 汇总 8 份阶段报告，输出**最终执行报告**：起止时间、各阶段验收数字、
   最终 diff/matched/D 残留/豁免命中、红线复核结论、未决与后续专项清单。
2. 确认最终 jj 提交已包含：全部阶段报告、文档同步（skill 三镜像/STATUS/计划勾销）。
3. 把最终执行报告写回 `docs/phase-reports/final-report.md`。

## 当前状态锚点（2026-09-08，续跑时以 report.json 实测为准）

- diff=799（97.53%）、D=30/12 簇、Security/Setup 全绿；
- 豁免登记 7 项/26 条（`scripts/acceptance-check.py` 的 EXEMPTIONS 表为真源）；
- 残留工作项与逐项步骤：执行计划 §A–§F；
- 已知坑位：Runbook §3.3（03 重建期间禁止并发 05；03 输出 GBK 等）。
