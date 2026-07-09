#!/usr/bin/env python3
"""Compare hotech-evtx vs python-evtx parsing results."""

import Evtx.Evtx as evtx
import Evtx.Views as e_views
import sys
import json
import sqlite3
import os
import io
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

def parse_with_python_evtx(evtx_path):
    """Extract event_id, provider, event_data from python-evtx."""
    results = []
    with evtx.Evtx(str(evtx_path)) as log:
        for record in log.records():
            try:
                xml = record.xml()
                eid = None
                provider = None
                # Simple extraction from XML
                import re
                m_eid = re.search(r'<EventID[^>]*>(\d+)</EventID>', xml)
                if m_eid:
                    eid = int(m_eid.group(1))
                m_prov = re.search(r'Provider\s+Name\s*=\s*["\']([^"\']+)["\']', xml)
                if m_prov:
                    provider = m_prov.group(1)
                if not m_prov:
                    m_prov2 = re.search(r'<Provider\s+Name\s*=\s*["\']([^"\']+)["\']', xml)
                    if m_prov2:
                        provider = m_prov2.group(1)
                results.append({
                    'record_id': record.record_num(),
                    'event_id': eid,
                    'provider': provider,
                    'xml_len': len(xml),
                })
            except Exception as e:
                results.append({
                    'record_id': record.record_num(),
                    'event_id': None,
                    'provider': None,
                    'error': str(e),
                })
    return results

def parse_with_hotech(evtx_path, msg_dir=None):
    """Parse with hotech-evtx and return event list."""
    import subprocess
    import tempfile

    exe = r"E:\Code\rust\hotech-analyzer-gui\crates\hotech-evtx\target\release\hotech-evtx.exe"
    tmp = tempfile.mktemp(suffix='.db')

    cmd = [exe, 'parse', str(evtx_path), '-o', tmp]
    if msg_dir:
        cmd.extend(['--msg-dir', str(msg_dir)])
    # Use en-US for clean comparison
    cmd.extend(['--lang', 'en-US'])

    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        return None, result.stderr

    # Query SQLite
    conn = sqlite3.connect(tmp)
    cursor = conn.cursor()
    cursor.execute("SELECT event_record_id, event_id, provider_name, message, level_name, keywords_name FROM events ORDER BY event_record_id")
    rows = cursor.fetchall()
    conn.close()
    Path(tmp).unlink(missing_ok=True)

    return [
        {'record_id': r[0], 'event_id': r[1], 'provider': r[2],
         'message': r[3], 'level_name': r[4], 'keywords_name': r[5]}
        for r in rows
    ], None

def compare(evtx_path, msg_dir=None):
    print(f"\n{'='*60}")
    print(f"File: {Path(evtx_path).name}")
    print(f"{'='*60}")

    # python-evtx
    py_results = parse_with_python_evtx(evtx_path)
    print(f"\npython-evtx: {len(py_results)} records")
    for r in py_results[:5]:
        print(f"  #{r['record_id']}: EventID={r['event_id']}, Provider={r['provider']}")

    # hotech-evtx (without MSG DB)
    ho_results, err = parse_with_hotech(evtx_path)
    if err:
        print(f"hotech-evtx error: {err[:100]}")
    else:
        print(f"\nhotech-evtx (no MSG): {len(ho_results)} records")
        for r in ho_results[:5]:
            msg_preview = (r['message'][:60] if r['message'] else 'NULL')
            print(f"  #{r['record_id']}: EventID={r['event_id']}, Prov={r['provider']}, Msg={msg_preview}")

    # hotech-evtx (with MSG DB)
    ho_msg_results, err = parse_with_hotech(evtx_path, msg_dir)
    if err:
        print(f"hotech-evtx (+MSG) error: {err[:100]}")
    else:
        print(f"\nhotech-evtx (+MSG DB): {len(ho_msg_results)} records")
        for r in ho_msg_results[:5]:
            msg_preview = (r['message'][:80] if r['message'] else 'NULL')
            print(f"  #{r['record_id']}: EventID={r['event_id']}, Prov={r['provider']}")
            print(f"    Msg: {msg_preview}")
            print(f"    level_name={r['level_name']}, keywords_name={r['keywords_name']}")

    # Verify: same number of records?
    if ho_results and len(py_results) != len(ho_results):
        print(f"\n⚠️  Record count mismatch! python-evtx={len(py_results)}, hotech={len(ho_results)}")
    else:
        print(f"\n✓ Record count matches: {len(py_results)}")

    # Verify event IDs match
    if ho_results and len(py_results) == len(ho_results):
        mismatches = sum(1 for a, b in zip(py_results, ho_results)
                        if a['event_id'] != b['event_id'])
        if mismatches:
            print(f"⚠️  EventID mismatches: {mismatches}")
        else:
            print(f"✓ All EventIDs match")

    return py_results, ho_results, ho_msg_results

if __name__ == '__main__':
    evtx_path = sys.argv[1] if len(sys.argv) > 1 else r"E:\temp\evtx_data\test\CA_4624_4625_LogonType2_LogonProc_chrome.evtx"
    msg_dir = sys.argv[2] if len(sys.argv) > 2 else r"E:\temp\evtx_data\msg_db"
    compare(evtx_path, msg_dir)
