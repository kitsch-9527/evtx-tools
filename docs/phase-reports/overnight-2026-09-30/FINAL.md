# FINAL — 夜间计划 2026-09-30 完成哨兵

> 状态：**完成**。全部阶段 P0–P8 收口，未触发红线回退，无未决 FAIL。
> 时间戳：2026-09-30 03:40 前后（P8 定稿）。详细版见同目录 `final-report.md`。

## 终态锚点（P7 验收 + P8 复核，report.json）

- compared=44890、**matched=44852**、**diff=21**（App 2 + System 19，全部 D 类已定性转出）
- **M1=99.9153%**、M2=93.97%、M5≈4、baseline_lang=zh-CN
- exempted=17 条/4 项（CAPI2 4111×1、SAM 16962×8、WPD 24577×4/24579×4）；
  SPP 1004（转绿）、disk 51（脱命中）已同步摘除豁免（P8，双表+登记表三处同步）
- `acceptance-check.py --prev` 总体 **PASS exit=0**（P8 豁免同步后复核仍 PASS）
- G6 关豁免 diff=38=21+17；确定性门两次全链路全等；单测 Go 134（四包，全仓 135）+ Rust 245 全绿
- **B/C 类全场 0**；Security/Setup 全绿（一票否决红线全程未破）

## 逐阶段一行结论

| 阶段 | 结论 |
|---|---|
| P0 就绪 | 锚点复核一致，锁+计划档落盘 |
| P1（60min） | **diff 237→157（−80）**：R12b registered-but-no-resources N/A 精度，CfgChangeMonitor×80 清零 |
| P2（90min） | **157→82（−75）**：value_maps schema v9 双语化（lang 入 PK + per-lang overlay + 渲染双层），Ntfs 98×74 清零 |
| P3（60min） | **82→44（−38）**：classic Binary 大写 hex 追加 %n+1（仅模板命中路径），NetBT 4311×38 清零 |
| P4（45min） | **44→35（−9）**：UserData 嵌套展平（形态门控），Resource-Exhaustion 2004×9 清零 |
| P5（60min） | **35→33（−2）**：WER GUID-span hex 保护收窄，WER 1001×2 清零；SPP 1004 连带转绿（exempted 18→17） |
| P6（60min） | **33→21（−12）**：`!S!` specifier 消费 + `%%` verbatim，Chrome/Edge×10、SCM 7023×2 清零；残留 21 条逐簇定性完毕 |
| P7（60min） | 全量验收 PASS：常规门 exit=0、G6 关豁免 38=21+17、确定性门两次全等、单测双端全绿 |
| P8（60min） | 定稿：skill 事实 #21–#23 + 基线表（三镜像 md5 一致）、STATUS 全量更新、EXEMPTIONS 摘 SPP/disk（验证 PASS）、final-report 落盘 |

## 转出（残留 21 条，逐条定性，无一"未知"）

EventLog 6008×2（BASE-ARTEFACT，建议白名单）、CodexSandboxService×2（UNREG-FALLBACK）、
e1dexpress×8（MUI-WRONGLANG es-ES）、VMnetAdapter×8（EID-QUALIFIER）、Wininit 23×1（TDH-SYMBOL）。
登记真源：`docs/residual-zeroing-plan-2026-09-07.md` §4 + skill"已知上游缺口" #4–#8。

## 文档产物

- `docs/phase-reports/overnight-2026-09-30/final-report.md`（详细终稿）
- `skills/parity-test-skill.md`（+`.agents`/`.dsh` 镜像，md5=7cad81a23ffe5442db929451dfc36aec）
- `STATUS.md`（§5/§6/§7/§8.1 终态口径）
- `docs/residual-zeroing-plan-2026-09-07.md` §4（2 项关闭 + 5 簇转出登记）
- `night-P0..P8-report.md`（逐阶段）
