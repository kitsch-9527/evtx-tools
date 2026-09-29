# probe-r12b-registered-no-resources.py — R12b 前置取证（dry-run 模型）
# 复用 06-compare.py 的 parse_baseline/load_new_csv/cmp_pair，不改其行为。
# 模型（待验证）：已注册（eventlog_sources 有行）但 provider_message_variants
# 无任何行 的 provider，wevtapi EvtFormatMessage 无消息资源可解析 → 基线渲染 N/A。
# 验证口径：四通道比对中出现的全部 provider，凡满足「已注册 ∧ variants=0」，
# 其基线行 message 必须全为 N/A；若存在非 N/A 基线行 → 规则须收窄，不得硬上。
import glob
import importlib.util
import sqlite3
import sys
from collections import defaultdict
from pathlib import Path

TEST_ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "cmp", Path(__file__).resolve().parent / "06-compare.py"
)
cmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cmp)

CHANNELS = ["Application", "Setup", "Security", "System"]
DB = glob.glob(str(TEST_ROOT / "msgdb" / "hotech-msg-*.db"))[0]

con = sqlite3.connect(DB)
cur = con.cursor()
registered = {r[0] for r in cur.execute("SELECT source_name FROM eventlog_sources")}
with_variants = {
    r[0] for r in cur.execute("SELECT DISTINCT provider_name FROM provider_message_variants")
}
print(f"DB={DB}")
print(f"registered sources={len(registered)}  providers_with_variants={len(with_variants)}")

primary_lang = cmp.detect_baseline_lang(TEST_ROOT / "baseline-parsed" / "System.evtx.txt")
print(f"baseline_lang={primary_lang}")

# 逐通道收集比对中出现的 provider 及其基线/新解析行
appearing = defaultdict(lambda: {"baseline": [], "new": []})
for ch in CHANNELS:
    base = cmp.parse_baseline(TEST_ROOT / "baseline-parsed" / f"{ch}.evtx.txt")
    zh = cmp.load_new_csv(TEST_ROOT / "new-parsed" / "zh" / f"{ch}.csv")
    en = cmp.load_new_csv(TEST_ROOT / "new-parsed" / "en" / f"{ch}.csv")
    primary, _cov = (zh, en) if primary_lang == "zh-CN" else (en, zh)
    for b in base:
        appearing[b["provider"]]["baseline"].append((ch, b))
    for z in primary:
        appearing[z["provider"]]["new"].append((ch, z))

target = sorted(p for p in appearing if p in registered and p not in with_variants)
print(f"\n=== 比对中出现 ∧ 已注册 ∧ variants=0 的 provider（共 {len(target)}）===")
all_ok = True
for p in target:
    na, non_na, eid0 = 0, 0, 0
    non_na_samples = []
    for ch, b in appearing[p]["baseline"]:
        if b["event_id"] == "0":
            eid0 += 1
        if b["message"] == "N/A":
            na += 1
        else:
            non_na += 1
            if len(non_na_samples) < 3:
                non_na_samples.append((ch, b["event_id"], b["message"][:120]))
    new_cnt = len(appearing[p]["new"])
    print(
        f"- {p}: baseline_rows={na + non_na} (eid0={eid0}) N/A={na} 非_N/A={non_na}"
        f" new_primary_rows={new_cnt}"
    )
    for ch, eid, msg in non_na_samples:
        all_ok = False
        print(f"    非 N/A 样例 [{ch} eid={eid}]: {msg}")

print()
print("VERDICT:", "OK — 目标 provider 基线全 N/A，规则可硬上" if all_ok else "NARROW — 存在非 N/A 基线，规则须收窄")

# 附：未注册 ∧ variants=0 的 provider（既有 R12 规则覆盖面，应已全 N/A）
unreg_zero = sorted(p for p in appearing if p not in registered and p not in with_variants)
bad = []
for p in unreg_zero:
    for ch, b in appearing[p]["baseline"]:
        if b["event_id"] != "0" and b["message"] != "N/A":
            bad.append((p, ch, b["event_id"], b["message"][:80]))
print(f"\n对照组（未注册 ∧ variants=0）provider 数={len(unreg_zero)}，eid!=0 且基线非 N/A 行数={len(bad)}")
for row in bad[:5]:
    print("   ", row)
