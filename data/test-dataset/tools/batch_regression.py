#!/usr/bin/env python3
"""Batch regression test: parse all hayabusa EVTX samples with hotech-evtx."""

import subprocess
import sqlite3
import sys
import json
import os
import tempfile
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

EXE = r"E:\Code\rust\hotech-analyzer-gui\crates\hotech-evtx\target\release\hotech-evtx.exe"
MSG_DIR = r"E:\temp\evtx_data\msg_db"
SRC_DIR = r"E:\temp\evtx_data\hayabusa-sample-evtx"

def parse_one(evtx_path):
    """Parse a single EVTX file. Returns (path, result_dict)."""
    rel = os.path.relpath(evtx_path, SRC_DIR)
    tmp = tempfile.mktemp(suffix='.db')

    start = time.time()
    try:
        result = subprocess.run(
            [EXE, 'parse', evtx_path, '-o', tmp, '--msg-dir', MSG_DIR, '--lang', 'en-US'],
            capture_output=True, text=True, timeout=120
        )
        elapsed = time.time() - start
    except subprocess.TimeoutExpired:
        return evtx_path, {'file': rel, 'status': 'TIMEOUT', 'events': 0, 'error': 'timeout > 120s', 'ms': int((time.time()-start)*1000)}
    except Exception as e:
        return evtx_path, {'file': rel, 'status': 'CRASH', 'events': 0, 'error': str(e), 'ms': int((time.time()-start)*1000)}

    # Check result
    if result.returncode != 0:
        stderr = result.stderr[:500] if result.stderr else ''
        # Clean up tmp
        try: Path(tmp).unlink(missing_ok=True)
        except: pass
        return evtx_path, {
            'file': rel, 'status': 'FAIL', 'events': 0,
            'error': stderr.replace('\n', ' | ').strip(),
            'ms': int(elapsed * 1000)
        }

    # Query event count
    try:
        conn = sqlite3.connect(tmp)
        cursor = conn.cursor()
        cursor.execute("SELECT count(*) FROM events")
        count = cursor.fetchone()[0]
        # Check message stats
        cursor.execute("SELECT count(*) FROM events WHERE message IS NOT NULL AND message != ''")
        msg_count = cursor.fetchone()[0]
        cursor.execute("SELECT count(*) FROM events WHERE level_name IS NOT NULL AND level_name != ''")
        level_count = cursor.fetchone()[0]
        conn.close()
    except Exception as e:
        try: Path(tmp).unlink(missing_ok=True)
        except: pass
        return evtx_path, {
            'file': rel, 'status': 'DB_ERR', 'events': 0,
            'error': str(e), 'ms': int(elapsed * 1000)
        }

    # Clean up
    try: Path(tmp).unlink(missing_ok=True)
    except: pass

    return evtx_path, {
        'file': rel, 'status': 'OK',
        'events': count,
        'msg_count': msg_count,
        'msg_pct': round(msg_count / count * 100, 1) if count > 0 else 0,
        'level_count': level_count,
        'level_pct': round(level_count / count * 100, 1) if count > 0 else 0,
        'ms': int(elapsed * 1000),
    }

def main():
    # Collect all EVTX files
    all_files = sorted(Path(SRC_DIR).rglob("*.evtx"))
    print(f"Total EVTX files: {len(all_files)}")
    print(f"MSG DB: {MSG_DIR}")
    print(f"{'='*60}\n")

    results = {'OK': [], 'FAIL': [], 'TIMEOUT': [], 'CRASH': [], 'DB_ERR': []}
    total_events = 0
    total_msg = 0
    total_level = 0

    # Multi-threaded parsing
    with ThreadPoolExecutor(max_workers=8) as pool:
        fut_to_file = {pool.submit(parse_one, str(f)): f for f in all_files}
        done = 0
        for fut in as_completed(fut_to_file):
            done += 1
            f, r = fut.result()
            results.setdefault(r['status'], []).append(r)
            total_events += r['events']
            total_msg += r.get('msg_count', 0)
            total_level += r.get('level_count', 0)

            if done % 50 == 0:
                print(f"[progress] {done}/{len(all_files)} files parsed...", file=sys.stderr)

    # Summary
    print(f"\n{'='*60}")
    print(f"REGRESSION TEST SUMMARY")
    print(f"{'='*60}")
    print(f"Total files:     {len(all_files)}")
    print(f"OK:              {len(results['OK'])} ({round(len(results['OK'])/len(all_files)*100,1)}%)")
    if results['FAIL']: print(f"FAIL:            {len(results['FAIL'])} ❌")
    if results['TIMEOUT']: print(f"TIMEOUT:         {len(results['TIMEOUT'])} ❌")
    if results['CRASH']: print(f"CRASH:           {len(results['CRASH'])} ❌")
    if results['DB_ERR']: print(f"DB_ERR:          {len(results['DB_ERR'])} ❌")
    print(f"")
    print(f"Total events:    {total_events}")
    print(f"With messages:   {total_msg} ({round(total_msg/total_events*100,1)}%)" if total_events > 0 else "N/A")
    print(f"With level_name: {total_level} ({round(total_level/total_events*100,1)}%)" if total_events > 0 else "N/A")
    print(f"")
    print(f"Avg parse time:  {round(sum(r['ms'] for r in results['OK'] + results['FAIL'])/len(results.get('OK',[]) + results.get('FAIL',[])), 1)}ms" if (results.get('OK') or results.get('FAIL')) else "N/A")

    # Event size distribution
    if results['OK']:
        event_buckets = {'1-10':0, '11-100':0, '101-1000':0, '1001-10000':0, '10000+':0}
        for r in results['OK']:
            e = r['events']
            if e <= 10: event_buckets['1-10'] += 1
            elif e <= 100: event_buckets['11-100'] += 1
            elif e <= 1000: event_buckets['101-1000'] += 1
            elif e <= 10000: event_buckets['1001-10000'] += 1
            else: event_buckets['10000+'] += 1
        print(f"\nEvent count distribution (OK files):")
        for k, v in event_buckets.items():
            print(f"  {k}: {v} files")

    # Top 10 by event count
    if results['OK']:
        sorted_ok = sorted(results['OK'], key=lambda x: x['events'], reverse=True)
        print(f"\nTop 10 largest files:")
        for r in sorted_ok[:10]:
            print(f"  {r['events']:>6} events  {r['file'][:80]}")

    # Message resolution stats
    if results['OK']:
        print(f"\nMessage resolution (MSG rate) distribution:")
        msg_buckets = {'0% (no msg)':0, '1-25%':0, '26-50%':0, '51-75%':0, '76-99%':0, '100%':0}
        for r in results['OK']:
            pct = r.get('msg_pct', 0)
            if pct == 0: msg_buckets['0% (no msg)'] += 1
            elif pct <= 25: msg_buckets['1-25%'] += 1
            elif pct <= 50: msg_buckets['26-50%'] += 1
            elif pct <= 75: msg_buckets['51-75%'] += 1
            elif pct < 100: msg_buckets['76-99%'] += 1
            else: msg_buckets['100%'] += 1
        for k, v in msg_buckets.items():
            print(f"  {k}: {v} files")

    # FAIL details
    if results['FAIL']:
        print(f"\n=== FAIL details ===")
        for r in results['FAIL'][:20]:
            print(f"  ❌ {r['file'][:80]}")
            print(f"     Error: {r['error'][:120]}")

    if results['TIMEOUT']:
        print(f"\n=== TIMEOUT details ===")
        for r in results['TIMEOUT'][:10]:
            print(f"  ⏰ {r['file'][:60]} ({r['ms']}ms)")

    # Summary JSON
    summary = {
        'total': len(all_files),
        'ok': len(results['OK']),
        'fail': len(results['FAIL']),
        'timeout': len(results['TIMEOUT']),
        'total_events': total_events,
        'msg_rate': round(total_msg / total_events * 100, 1) if total_events > 0 else 0,
        'level_rate': round(total_level / total_events * 100, 1) if total_events > 0 else 0,
    }
    print(f"\n{'='*60}")
    print(f"JSON summary: {json.dumps(summary, indent=2)}")

    # Write detailed CSV
    csv_path = os.path.join(os.path.dirname(__file__) or '.', 'hayabusa_regression.csv')
    with open(csv_path, 'w', encoding='utf-8') as f:
        f.write("file,status,events,msg_count,msg_pct,level_count,level_pct,ms,error\n")
        for r in results.get('OK', []) + results.get('FAIL', []) + results.get('TIMEOUT', []) + results.get('CRASH', []) + results.get('DB_ERR', []):
            f.write(f"{r['file']},{r['status']},{r['events']},{r.get('msg_count',0)},{r.get('msg_pct','')},{r.get('level_count',0)},{r.get('level_pct','')},{r['ms']},{r.get('error','')}\n".replace('\n',' '))
    print(f"\nDetail CSV: {csv_path}")

if __name__ == '__main__':
    main()
