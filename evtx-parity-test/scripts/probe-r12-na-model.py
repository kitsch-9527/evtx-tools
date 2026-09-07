# probe-r12-na-model.py — R12 前置取证：全量验证「miss 事件 × 注册状态 → 基线 N/A」模型
# 复用 06-compare.py 的 parse_baseline/load_new_csv，不改其行为。
# 模型（待验证）：
#   miss = DB 无该 (provider_name, event_id) variant
#   注册状态 = 经典 EventLog\<log>\<name> 存在 → 'classic'
#            | WINEVT Publishers（按 GUID→Name 或 name 直配）→ 'winevt'
#            | 否则 → 'none'
#   预测基线渲染：none & EID!=0 → 'N/A'；其余 → 非 N/A
import csv
import io
import sqlite3
import sys
import winreg
from pathlib import Path

TEST_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
import importlib.util

spec = importlib.util.spec_from_file_location(
    "cmp", Path(__file__).resolve().parent / "06-compare.py"
)
cmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cmp)

CHANNELS = ["Application", "Setup", "Security", "System"]

# --- registry ---
def classic_sources():
    out = set()
    for ch in CHANNELS:
        try:
            k = winreg.OpenKey(
                winreg.HKEY_LOCAL_MACHINE,
                rf"SYSTEM\CurrentControlSet\Services\EventLog\{ch}",
            )
        except OSError:
            continue
        i = 0
        while True:
            try:
                s = winreg.EnumKey(k, i)
                i += 1
                out.add(s)
            except OSError:
                break
    return out

def publisher_names():
    names = set()
    k = winreg.OpenKey(
        winreg.HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers",
    )
    i = 0
    while True:
        try:
            g = winreg.EnumKey(k, i)
            i += 1
        except OSError:
            break
        try:
            sk = winreg.OpenKey(k, g)
            nm, _ = winreg.QueryValueEx(sk, None)  # default value = provider name
            if nm:
                names.add(nm)
        except OSError:
            pass
    return names

CLASSIC = classic_sources()
PUBS = publisher_names()
print(f"classic sources={len(CLASIC := CLASSIC)}, publisher names={len(PUBS)}")

def source_type(provider: str) -> str:
    if provider in CLASSIC:
        return "classic"
    if provider in PUBS:
        return "winevt"
    return "none"

# --- DB variant cohorts ---
db = sqlite3.connect(TEST_ROOT / "msgdb" / "hotech-msg-Windows_10_Education-25H2-26220.9223.db")
have = set()
for pn, eid in db.execute("SELECT DISTINCT provider_name, event_id FROM provider_message_variants"):
    have.add((pn, eid))

# --- walk events ---
from collections import Counter

pred_ok = Counter()
pred_bad = []
miss_by_type = Counter()
for ch in CHANNELS:
    base = cmp.parse_baseline(TEST_ROOT / "baseline-parsed" / f"{ch}.evtx.txt")
    en = cmp.load_new_csv(TEST_ROOT / "new-parsed" / "en" / f"{ch}.csv")
    n = min(len(base), len(en))
    for i in range(n):
        b, z = base[i], en[i]
        pn, eid = z["provider"], z["event_id"]
        try:
            eid_i = int(eid)
        except (TypeError, ValueError):
            continue
        if (pn, eid_i) in have:
            continue  # hit — 不在本模型范围
        st = source_type(pn)
        miss_by_type[st] += 1
        bmsg = (b["message"] or "").strip()
        is_na = bmsg == "N/A"
        pred_na = st == "none" and eid_i != 0
        if pred_na == is_na:
            pred_ok[(ch, st, "NA" if is_na else "msg")] += 1
        else:
            pred_bad.append((ch, i, pn, eid_i, st, bmsg[:60]))

print("\nmiss 事件按注册状态:", dict(miss_by_type))
print("\n预测命中分布:", dict(pred_ok))
print(f"\n预测失败 {len(pred_bad)} 条（前 30）:")
for row in pred_bad[:30]:
    print("  ", row)
