#!/usr/bin/env python3
"""Show actual message differences side by side."""
import csv, sqlite3, re
from pathlib import Path

REF = Path("E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/data/test-dataset/reference")
HOT = Path("E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/data/test-dataset/hotech-output")

name = "bits_openvpn"
with open(str(REF/f"{name}.csv"), encoding='utf-8-sig') as f:
    refs = sorted(list(csv.DictReader(f)), key=lambda x: int(x['record_id']))
conn = sqlite3.connect(str(HOT/f"{name}.db"))
conn.row_factory = sqlite3.Row
hots = [dict(r) for r in conn.execute("SELECT * FROM events ORDER BY event_record_id")]
conn.close()

n = 0
for i in range(min(len(refs), len(hots))):
    if n >= 5: break
    r, h = refs[i], hots[i]
    rm = (r.get('message','') or '').replace('¥', '\n').strip()
    hm = (h.get('message','') or '').strip()

    # Quick normalize for comparison
    rn = re.sub(r'\s+', ' ', rm)
    hn = re.sub(r'\s+', ' ', hm)
    if rn == hn:
        continue

    n += 1
    print(f"\n{'='*60}")
    print(f"Record #{r.get('record_id','')} (Event {r.get('event_id','')})")
    print(f"{'='*60}")
    print("=== POWER SHELL (REF) ===")
    print(rm[:200])
    print("...")
    print("=== HOTECH-EVTX ===")
    print(hm[:200])
    print("...")

    # Show char-by-char differences
    for idx in range(min(len(rn), len(hn))):
        if rn[idx] != hn[idx]:
            print(f"\nFirst diff at char {idx}:")
            print(f"  REF: '{rn[idx:idx+40]}'")
            print(f"  HOT: '{hn[idx:idx+40]}'")
            break
