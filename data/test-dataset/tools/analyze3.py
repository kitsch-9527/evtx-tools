"""Quick analysis of 0% msg files from the regression test."""
# Parse the regression output directly from the terminal output we saw
# The issue is many files have 0% message resolution - let's categorize them

# From the regression test, we know:
# 0% msg files = 311 files (where no messages resolved)
# These are primarily from: DeepBlueCLI (PowerShell), Sysmon, 3rd-party providers

# Let's check by looking at the raw CSV more carefully
import csv
from collections import Counter

with open('hayabusa_regression.csv', 'r', encoding='utf-8') as f:
    content = f.read()

# Split by lines - check if there are multiple lines
lines = content.strip().split('\n')
print(f"Total lines: {len(lines)}")

if len(lines) == 1:
    print("All data is on one line - need to parse differently")
    header = lines[0].split(',')
    print(f"Header: {header}")
    ncol = len(header)
    # All values
    vals = [v.strip() for v in content.strip().split(',')]
    data_vals = vals[ncol:]  # skip header
    print(f"Data values: {len(data_vals)}, Cols: {ncol}, Expected records: {len(data_vals)//ncol}")

    records = []
    for i in range(0, len(data_vals), ncol):
        if i + ncol <= len(data_vals):
            rec = dict(zip(header, data_vals[i:i+ncol]))
            records.append(rec)
else:
    reader = csv.DictReader(lines)
    records = list(reader)

print(f"Records: {len(records)}")

# Categorize
no_msg_files = []
no_msg_by_top = Counter()
no_msg_by_prov = Counter()  # rough: first segment of filename

for r in records:
    if r['status'] != 'OK':
        continue
    ev = int(r['events'])
    mp = float(r['msg_pct']) if r['msg_pct'] else 0.0
    if ev > 0 and mp == 0:
        no_msg_files.append(r)
        fn = r['file'].replace('\\', '/')
        top = fn.split('/')[0]
        no_msg_by_top[top] += 1

print(f"\n=== 0% msg files: {len(no_msg_files)} ===")
print("By top directory:")
for d, c in no_msg_by_top.most_common(20):
    print(f"  {d}: {c}")

# Show some 0% msg file paths
print("\nSample 0% msg files:")
for r in no_msg_files[:20]:
    print(f"  {r['file'][:90]}")
