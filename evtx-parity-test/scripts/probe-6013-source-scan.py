# -*- coding: utf-8 -*-
"""Phase C probe: locate the source DLL of EventLog/System 6013
("The system uptime is N seconds.").

Strategy:
  1. Read HKLM\\SYSTEM\\CurrentControlSet\\Services\\EventLog\\System sources'
     EventMessageFile values (classic provider registry).
  2. For every referenced PE, plus its en-US / zh-CN MUI satellite, scan the raw
     bytes for the needle "uptime" case-insensitively in both ASCII and UTF-16LE.
  3. Report every hit.

Usage:
  python probe-6013-source-scan.py                # registry-driven scan
  python probe-6013-source-scan.py --dirs ...     # whole-directory scan
"""
import ctypes
import os
import re
import sys
import winreg

NEEDLE = b"uptime"

def needle_variants():
    out = [NEEDLE]
    wide = NEEDLE.decode().encode("utf-16le")
    out.append(wide)
    return out

def count_hits(data, pat):
    return len(pat.findall(data))

def scan_file(path, pats):
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError:
        return None
    hits = {name: count_hits(data, p) for name, p in pats}
    total = sum(hits.values())
    if total:
        return hits
    return None

def mui_variants(path):
    """For C:\\...\\foo.dll yield C:\\...\\en-US\\foo.dll.mui and zh-CN variant."""
    d, name = os.path.split(path)
    for lang in ("en-US", "zh-CN"):
        yield os.path.join(d, lang, name + ".mui")

def enumerate_registry_files():
    files = set()
    key_path = r"SYSTEM\CurrentControlSet\Services\EventLog\System"
    with winreg.ConnectRegistry(None, winreg.HKEY_LOCAL_MACHINE) as rk:
        with winreg.OpenKey(rk, key_path) as base:
            nsub, _, _ = winreg.QueryInfoKey(base)
            for i in range(nsub):
                try:
                    sub = winreg.EnumKey(base, i)
                except OSError:
                    continue
                try:
                    with winreg.OpenKey(base, sub) as sk:
                        v, _ = winreg.QueryValueEx(sk, "EventMessageFile")
                except OSError:
                    continue
                for part in v.split(";"):
                    part = part.strip()
                    if not part:
                        continue
                    part = os.path.expandvars(part)
                    files.add(part.lower())
    return sorted(files)

def main():
    pats = [
        ("ascii", re.compile(NEEDLE, re.IGNORECASE)),
        ("utf16le", re.compile(NEEDLE.decode().encode("utf-16le"), re.IGNORECASE)),
    ]
    targets = set()
    if "--dirs" in sys.argv:
        for d in sys.argv[sys.argv.index("--dirs") + 1:]:
            for root, _, names in os.walk(d):
                for n in names:
                    targets.add(os.path.join(root, n))
    else:
        for f in enumerate_registry_files():
            targets.add(f)
            # also register MUI satellites as scan targets (independent files)
            d, name = os.path.split(f)
            for lang in ("en-us", "zh-cn"):
                targets.add(os.path.join(d, lang, name + ".mui"))

    scanned = hit = miss = 0
    for t in sorted(targets):
        if not os.path.isfile(t):
            continue
        scanned += 1
        r = scan_file(t, pats)
        if r:
            hit += 1
            print("HIT", t, r, flush=True)
        else:
            miss += 1
    print(f"scanned={scanned} hit={hit} miss={miss}")

if __name__ == "__main__":
    main()
