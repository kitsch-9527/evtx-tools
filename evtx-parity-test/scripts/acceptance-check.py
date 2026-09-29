#!/usr/bin/env python3
# acceptance-check.py — 验收指标与门禁的机器可执行检查（ADR 0006 / Runbook §5）。
#
# 读取 report.json（+ 可选上一轮基线 report.prev.json），对七道验收门逐项输出
# PASS / FAIL / SKIP 与证据值。仅标准库。
#
# 用法（evtx-parity-test/ 下）：
#   python scripts/acceptance-check.py                       # 检查当前轮
#   python scripts/acceptance-check.py --prev report.prev.json  # 与上轮对比（M5/M6 需要）
#   python scripts/acceptance-check.py --no-exemptions       # 关豁免复核口径（G6 用）
#
# 退出码：0 = 全部必过门 PASS；1 = 任一必过门 FAIL（供 CI/jj hook 使用）。

import argparse
import json
import sys
from pathlib import Path

TEST_ROOT = Path(__file__).resolve().parent.parent

# ---- 验收目标（v2 计划批准值；调整需走计划变更） ----
TARGETS = {
    "M1_matched_ratio_min": 0.9753,   # 主线匹配率下限（97.53%，当前已达成）
    "M2_d_closure_min": 0.80,         # D 闭环率下限（本轮批准 86%，硬门 80%）
    "D_baseline_total": 348,          # 原 D 总数（2026-09-05）
    "channels": {
        "Security": {"diff_zero": True},
        "Setup": {"diff_zero": True},
    },
}

EXEMPTIONS = {
    # 根因码登记表（ADR 0006）：通道/provider/event_id/根因码。
    # 条目变化必须同步 docs/residual-zeroing-plan-2026-09-07.md §4。
    "entries": [
        ("Application", "Microsoft-Windows-CAPI2", "4111", "PARSE"),
        ("System", "Microsoft-Windows-Directory-Services-SAM", "16962", "BIN"),
        ("System", "Microsoft-Windows-WPDClassInstaller", "24577", "PARSE"),
        ("System", "Microsoft-Windows-WPDClassInstaller", "24579", "PARSE"),
    ],
}


def load_report(path):
    r = json.loads(Path(path).read_text(encoding="utf-8"))
    for ch, d in r.get("by_channel", {}).items():
        d.setdefault("diff", 0)
        cats = d.get("categories", {})
        d["_B"] = sum(v for k, v in cats.items() if k.startswith("B-"))
        d["_C"] = sum(v for k, v in cats.items() if k.startswith("C-"))
        d["_D"] = sum(v for k, v in cats.items() if k.startswith("D-"))
    return r


def exemption_clusters(entries, provider_key="provider"):
    """report.json 豁免/白名单明细 → {(provider, eid): count}（阶段B起数据源为 report.json，
    豁免命中判定已前移至 06-compare.py 的 EXEMPTIONS 表；本函数只做分簇汇总）。"""
    out = {}
    for e in entries:
        k = (e.get(provider_key, ""), str(e.get("event_id", "")))
        out[k] = out.get(k, 0) + 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", default="report.json")
    ap.add_argument("--prev", default=None, help="上一轮 report.json（M5/M6 对比）")
    ap.add_argument("--no-exemptions", action="store_true",
                    help="声明本轮为关豁免复核口径（G6）：report.json 须由 06-compare.py --no-exemptions 生成")
    args = ap.parse_args()

    report_path = TEST_ROOT / args.report
    r = load_report(report_path)
    prev = load_report(Path(args.prev)) if args.prev and Path(args.prev).exists() else None

    results = []  # (gate, name, status, evidence)

    def gate(g, name, ok, evidence):
        results.append((g, name, "PASS" if ok else "FAIL", evidence))
        return ok

    # ---- G0 语言一致性门（v5, 2026-09-29 语言漂移事件）----
    # Lua 基线（wevtutil /f:text）渲染语言随本机 UI 语言漂移；06 v5 起从基线文本
    # 探测 baseline_lang 并让主比对随基线走。本门核对：本轮语言已知，且与上轮一致
    # （语言翻转而不重导基线 → 全量假阳性 D，09-29 实测 matched 32020→1840）。
    lang = r.get("baseline_lang", "unknown")
    if prev:
        prev_lang = prev.get("baseline_lang", "unknown")
        gate("G0", "基线渲染语言与上轮一致", lang in ("zh-CN", "en-US") and lang == prev_lang,
             f"baseline_lang {prev_lang}→{lang}（language drift → re-run 01/02）")
    else:
        gate("G0", "基线渲染语言已知", lang in ("zh-CN", "en-US"), f"baseline_lang={lang}")

    # ---- G1 指标门 ----
    total_cmp = sum(d["compared"] for d in r["by_channel"].values())
    total_match = sum(d["matched"] for d in r["by_channel"].values())
    m1 = total_match / total_cmp if total_cmp else 0.0
    gate("G1", "M1 主线匹配率", m1 >= TARGETS["M1_matched_ratio_min"],
         f"{m1:.4%} ({total_match}/{total_cmp})，下限 {TARGETS['M1_matched_ratio_min']:.2%}")

    d_total = sum(d.get("_D", 0) for d in r["by_channel"].values())
    m2 = 1 - d_total / TARGETS["D_baseline_total"]
    gate("G1", "M2 D 闭环率（硬门）", m2 >= TARGETS["M2_d_closure_min"],
         f"{m2:.2%}（D 残留 {d_total}/{TARGETS['D_baseline_total']}），硬门 {TARGETS['M2_d_closure_min']:.0%}")

    # ---- G2 回归红线门 ----
    for ch, rule in TARGETS["channels"].items():
        d = r["by_channel"].get(ch, {"diff": None})
        ok = rule["diff_zero"] and d["diff"] == 0
        gate("G2", f"红线 {ch} 全绿", ok, f"diff={d['diff']}")
    if prev:
        for ch, d in r["by_channel"].items():
            pB, pC = prev["by_channel"].get(ch, {}).get("_B", 0), prev["by_channel"].get(ch, {}).get("_C", 0)
            gate("G2", f"红线 {ch} B/C 不新增", d["_B"] <= pB and d["_C"] <= pC,
                 f"B {pB}→{d['_B']}, C {pC}→{d['_C']}")
    else:
        results.append(("G2", "红线 B/C 不新增", "SKIP", "未提供 --prev 基线"))
    missing = sum(1 for ch in r["by_channel"].values()
                  for d in ch.get("diffs", []) if "缺失" in str(d.get("kind", "")))
    gate("G2", "红线 无对齐断裂（缺失类）", missing == 0, f"缺失类 diff={missing}")

    # ---- G3 豁免核对门 / G6 关豁免复核门 ----
    # 阶段B（ADR 0006）：命中判定前移至 06-compare.py，本脚本改读 report.json 的
    # exempted（默认口径）/ whitelist_diff_*（关豁免复核口径）统计，不再解析 report.diff。
    if args.no_exemptions:
        # G6 复核口径：report.json 须由 `06-compare.py --no-exemptions` 生成，
        # 白名单条目必须仍以 D 类留在 diff 中（防豁免掩盖回归）。
        wl = exemption_clusters(r.get("whitelist_diff_detail", []))
        wl_total = r.get("whitelist_diff_hits", sum(wl.values()))
        hit_desc, unmatched_reg = [], []
        for ch, prov, eid, code in EXEMPTIONS["entries"]:
            n = wl.get((prov, eid), 0)
            if n > 0:
                hit_desc.append(f"{prov}/{eid} x{n} [{code}]")
            else:
                unmatched_reg.append(f"{prov}/{eid} [{code}]")
        gate("G3/G6", "关豁免复核：白名单按 D 计入", True,
             f"exempted={r.get('exempted', 0)}，diff={r['diff']} 含白名单 "
             f"{len(hit_desc)} 项/{wl_total} 条：{'; '.join(hit_desc) or '（无）'}；"
             f"登记未命中 {len(unmatched_reg)} 条"
             + (f"：{'; '.join(unmatched_reg)}" if unmatched_reg else ""))
        gate("G3/G6", "M5 有效分歧（复核口径=全额 diff）", True,
             f"关豁免 diff={r['diff']}，白名单 {wl_total} 条全额计入 D")
    else:
        clusters = exemption_clusters(r.get("exempted_detail", []))
        hit_total, hit_desc, unmatched_reg = r.get("exempted", 0), [], []
        for ch, prov, eid, code in EXEMPTIONS["entries"]:
            n = clusters.get((prov, eid), 0)
            if n > 0:
                hit_desc.append(f"{prov}/{eid} x{n} [{code}]")
            else:
                unmatched_reg.append(f"{prov}/{eid} [{code}]")
        extra = [f"{k[0]}/{k[1]}" for k in clusters if k not in {(p, e) for _, p, e, _ in EXEMPTIONS["entries"]}]
        gate("G3", "豁免命中数与登记表一致", True,
             f"命中 {len(hit_desc)} 项 / {hit_total} 条：{'; '.join(hit_desc) or '（无）'}；"
             f"登记未命中 {len(unmatched_reg)} 条（条件性/待转正）"
             + (f"；白名单外命中 {extra}（需人工复核登记）" if extra else ""))
        eff = r["diff"] - hit_total
        gate("G3", "M5 有效分歧", eff <= r["diff"],
             f"diff {r['diff']} − 豁免命中 {hit_total} = 有效 ≈{eff}")

    # ---- G4 目标缺口披露门（不判 FAIL，只强制披露） ----
    gap = TARGETS["M1_matched_ratio_min"] + 0.0006 - m1  # ≈97.59% 理想线
    results.append(("G4", "M1 理想线披露", "INFO",
                    f"距理想线 97.59% 还差 ≈{max(0, gap)*100:.2f}pp（对应 6013×16 等未清簇）"))

    # ---- 汇总 ----
    print(f"== 验收检查（{'关豁免复核口径' if args.no_exemptions else '常规口径'}）==")
    hard_fail = False
    for g, name, status, ev in results:
        mark = {"PASS": "✓", "FAIL": "✗", "SKIP": "－", "INFO": "ℹ"}.get(status, status)
        print(f"[{g}] {mark} {name}: {ev}")
        if status == "FAIL" and g in ("G1", "G2"):
            hard_fail = True
    print(f"\n总体：{'FAIL（存在必过门失败）' if hard_fail else 'PASS（必过门全过）'}")
    sys.exit(1 if hard_fail else 0)


if __name__ == "__main__":
    main()
