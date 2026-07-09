import csv, sqlite3
from collections import Counter

# Read CSV - note: all data is on one line, each record is comma-separated
# CSV format: file,status,events,msg_count,msg_pct,level_count,level_pct,ms,error
with open('hayabusa_regression.csv', 'r') as f:
    raw = f.read()

lines = raw.strip().split('\n')
header = lines[0].split(',')

# Number of columns
ncol = len(header)
print(f"Header ({ncol} cols): {header}")

# Parse all values
all_vals = [v.strip() for v in raw.strip().split(',')]
# Remove header
data_vals = all_vals[ncol:]
print(f"Total values (after header): {len(data_vals)}")

# Group into records
records = []
for i in range(0, len(data_vals), ncol):
    if i + ncol <= len(data_vals):
        rec = dict(zip(header, data_vals[i:i+ncol]))
        records.append(rec)

print(f"Records parsed: {len(records)}")

no_msg = []
partial = []
all_ok = []
for r in records:
    ev = int(r['events'])
    mp = float(r['msg_pct']) if r['msg_pct'] else 0.0
    fn = r['file']
    status = r['status']
    if status != 'OK':
        print(f"  NON-OK: {status} - {fn[:60]}")
    all_ok.append(r)
    if ev > 0 and mp == 0:
        no_msg.append(r)
    elif ev > 0 and 0 < mp < 50:
        partial.append(r)

print(f"\n{'='*60}")
print(f"OK files:     {len(all_ok)}")
print(f"0% msg:       {len(no_msg)}")
print(f"Partial msg:  {len(partial)}")
print(f"{'='*60}")

# Categorize 0% msg by source directory
src_dirs = Counter()
for r in no_msg:
    fn = r['file'].replace('\\', '/')
    parts = fn.split('/')
    src_dirs[parts[0]] += 1

print(f"\n=== 0% msg by source directory ===")
for d, c in src_dirs.most_common(30):
    print(f"  {d}: {c}")

# Categorize 0% msg files by provider pattern (from filename)
print(f"\n=== 0% msg sample filenames (first 30) ===")
for r in no_msg[:30]:
    fn = r['file'].replace('\\', '/')
    # Show last 2 path segments
    parts = fn.split('/')
    print(f"  {r['events']:>4}evt - {parts[-2] if len(parts)>1 else ''}/{parts[-1]}")

# Also show partial ones
if partial:
    print(f"\n=== Partial msg files (< 50%) ===")
    for r in partial:
        print(f"  {r['msg_pct']}% - {r['events']}evt - {r['file'][:70]}")
