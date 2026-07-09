#!/usr/bin/env python3
"""Categorize message match differences between hotech-evtx and PowerShell reference."""
import csv, sqlite3, re, sys
from pathlib import Path

REF = Path("E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/data/test-dataset/reference")
HOT = Path("E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/data/test-dataset/hotech-output")

def norm(m):
    if not m: return ""
    # Undo newline encoding (¥=U+00A5, 楼=U+697C, ␤=U+2424 depending on PS encoding)
    # Then normalize all newline sequences to \n and collapse whitespace
    r = m.replace('¥', '\n').replace('楼', '\n').replace('␤', '\n').replace('\r\n','\n').replace('\r','\n')
    # GUID: strip {} and uppercase
    r = re.sub(r'\{([0-9a-fA-F-]+)\}', lambda m: m.group(1).upper(), r)
    # Collapse all whitespace runs to single space for content comparison
    r = re.sub(r'\s+', ' ', r)
    return r.strip()

# Analyze bits_openvpn
name = "bits_openvpn"
with open(str(REF/f"{name}.csv"), encoding='utf-8-sig') as f:
    refs = sorted(list(csv.DictReader(f)), key=lambda x: int(x['record_id']))

conn = sqlite3.connect(str(HOT/f"{name}.db"))
conn.row_factory = sqlite3.Row
c = conn.cursor()
c.execute("SELECT * FROM events ORDER BY event_record_id")
hots = [dict(r) for r in c.fetchall()]
conn.close()

cats = {'exact':0, 'tab':0, 'hex':0, 'guid':0, 'enum':0, 'ws':0, 'newline':0, 'other':0}
total_nonempty = 0

for i in range(min(len(refs), len(hots))):
    r, h = refs[i], hots[i]
    rm_raw = r.get('message','') or ''
    hm_raw = h.get('message','') or ''
    rm, hm = norm(rm_raw), norm(hm_raw)
    if not rm or not hm:
        continue
    total_nonempty += 1
    if rm == hm:
        cats['exact'] += 1
        continue
    # GUID only
    r1 = re.sub(r'\{([0-9a-fA-F-]+)\}', lambda m: m.group(1).upper(), rm_raw)
    h1 = re.sub(r'\{([0-9a-fA-F-]+)\}', lambda m: m.group(1).upper(), hm_raw)
    if norm(r1) == norm(h1):
        cats['guid'] += 1; continue
    # Tab only
    if norm(rm_raw.replace('\t',' ')) == norm(hm_raw.replace('\t',' ')):
        cats['tab'] += 1; continue
    # Hex case only
    r2 = re.sub(r'(0x[0-9a-fA-F]+)', lambda m: m.group(1).upper(), rm_raw)
    h2 = re.sub(r'(0x[0-9a-fA-F]+)', lambda m: m.group(1).upper(), hm_raw)
    if norm(r2) == norm(h2):
        cats['hex'] += 1; continue
    # Newline structure (¥=U+00A5 or 楼=U+697C in ref)
    r3 = rm_raw.replace('¥', '\n').replace('楼', '\n').replace('\r\n', '\n').replace('\r', '\n')
    h3 = hm_raw.replace('\r\n', '\n').replace('\r', '\n')
    r3s = re.sub(r'\s+', ' ', r3).strip()
    h3s = re.sub(r'\s+', ' ', h3).strip()
    if r3s == h3s:
        cats['newline'] += 1; continue
    # Pure whitespace
    if re.sub(r'\s+', ' ', rm_raw.strip()) == re.sub(r'\s+', ' ', hm_raw.strip()):
        cats['ws'] += 1; continue
    cats['other'] += 1

total_matched = sum(cats.values())
print(f"Message diff categories — {name} ({total_matched} non-empty msgs):")
for k, v in sorted(cats.items(), key=lambda x: -x[1]):
    pct = round(v/total_matched*100, 1) if total_matched else 0
    print(f"  {k+':':8s} {v:>4} ({pct}%)")

# Same analysis for all files
print(f"\n{'='*60}")
print(f"All 21 files aggregated:")
all_cats = {}
total_all = 0
for evtx in sorted(Path("E:/Code/rust/hotech-analyzer-gui/crates/hotech-evtx/data/test-dataset/evtx").glob("*.evtx")):
    name = evtx.stem
    ref = REF / f"{name}.csv"
    hot = HOT / f"{name}.db"
    if not ref.exists() or not hot.exists(): continue
    with open(str(ref), encoding='utf-8-sig') as f:
        refs = sorted(list(csv.DictReader(f)), key=lambda x: int(x['record_id']))
    conn = sqlite3.connect(str(hot))
    conn.row_factory = sqlite3.Row
    c = conn.cursor()
    c.execute("SELECT * FROM events ORDER BY event_record_id")
    hots = [dict(r) for r in c.fetchall()]
    conn.close()
    for i in range(min(len(refs), len(hots))):
        r, h = refs[i], hots[i]
        rm_raw = r.get('message','') or ''
        hm_raw = h.get('message','') or ''
        rm, hm = norm(rm_raw), norm(hm_raw)
        if not rm or not hm: continue
        total_all += 1
        if rm == hm:
            all_cats['exact'] = all_cats.get('exact',0) + 1
            continue
        r1 = re.sub(r'\{([0-9a-fA-F-]+)\}', lambda m: m.group(1).upper(), rm_raw)
        h1 = re.sub(r'\{([0-9a-fA-F-]+)\}', lambda m: m.group(1).upper(), hm_raw)
        if norm(r1) == norm(h1):
            all_cats['guid'] = all_cats.get('guid',0) + 1; continue
        if norm(rm_raw.replace('\t',' ')) == norm(hm_raw.replace('\t',' ')):
            all_cats['tab'] = all_cats.get('tab',0) + 1; continue
        r2 = re.sub(r'(0x[0-9a-fA-F]+)', lambda m: m.group(1).upper(), rm_raw)
        h2 = re.sub(r'(0x[0-9a-fA-F]+)', lambda m: m.group(1).upper(), hm_raw)
        if norm(r2) == norm(h2):
            all_cats['hex'] = all_cats.get('hex',0) + 1; continue
        r3 = rm_raw.replace('¥','\n').replace('\r\n','\n').replace('\r','\n')
        h3 = hm_raw.replace('\r\n','\n').replace('\r','\n')
        if re.sub(r'\s+',' ',r3).strip() == re.sub(r'\s+',' ',h3).strip():
            all_cats['newline'] = all_cats.get('newline',0) + 1; continue
        if re.sub(r'\s+',' ',rm_raw.strip()) == re.sub(r'\s+',' ',hm_raw.strip()):
            all_cats['ws'] = all_cats.get('ws',0) + 1; continue
        all_cats['other'] = all_cats.get('other',0) + 1

for k, v in sorted(all_cats.items(), key=lambda x: -x[1]):
    pct = round(v/total_all*100, 1) if total_all else 0
    print(f"  {k+':':8s} {v:>4} ({pct}%)")
print(f"  {'total':8s} {total_all:>4}")
