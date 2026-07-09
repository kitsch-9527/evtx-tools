import csv
from collections import Counter, defaultdict

no_msg_files = []
partial_files = []
with open('hayabusa_regression.csv', 'r') as f:
    reader = csv.DictReader(f)
    for row in reader:
        ev = int(row['events'])
        mp = float(row['msg_pct']) if row['msg_pct'] else 0
        fn = row['file']
        if ev > 0 and mp == 0:
            no_msg_files.append(fn)
        elif ev > 0 and 0 < mp < 50:
            partial_files.append((fn, mp))

print(f"=== Files with 0% message: {len(no_msg_files)} ===")
dirs = Counter()
for p in no_msg_files:
    p_norm = p.replace('\\', '/')
    parts = p_norm.split('/')
    if len(parts) >= 2:
        top = parts[0]
        dirs[top] += 1
print("By source directory:")
for d, c in dirs.most_common(20):
    print(f"  {d}: {c}")

print(f"\n=== Partial message files (< 50%) ===")
for fn, mp in sorted(partial_files, key=lambda x: x[1]):
    print(f"  {mp}% - {fn[:80]}")

# Deeper analysis: check which providers have messages in the full DB
import sqlite3
db = sqlite3.connect(r"E:\temp\evtx_data\test\output\msg_logon_en.db")

# Let's also categorize by file size to see if small files are the ones without messages
print("\n=== 0% msg files - file sizes ===")
sizes = Counter()
for fn in no_msg_files:
    # count by first 2 path segments
    p = fn.replace('\\', '/')
    segs = p.split('/')
    if len(segs) >= 3:
        key = '/'.join(segs[:2])
    elif len(segs) >= 2:
        key = segs[0]
    else:
        key = 'root'
    sizes[key] += 1
for k, v in sizes.most_common(20):
    print(f"  {k}: {v}")
