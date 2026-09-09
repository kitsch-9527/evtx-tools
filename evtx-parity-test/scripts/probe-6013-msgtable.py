# -*- coding: utf-8 -*-
"""Phase C probe: dump classic MESSAGE_TABLE (RT_MESSAGETABLE, resource type 11)
from a PE and grep entries.

Usage:
  python probe-6013-msgtable.py <pe-path> [<pe-path> ...] [--grep NEEDLE]
"""
import re
import struct
import sys


def utf16z(data, off):
    out = []
    while off + 1 < len(data):
        c = data[off] | (data[off + 1] << 8)
        if c == 0:
            break
        out.append(c)
        off += 2
    return "".join(chr(c) for c in out)


def ansi_z(data, off):
    out = []
    while off < len(data) and data[off] != 0:
        out.append(data[off])
        off += 1
    return bytes(out).decode("latin-1")


def enum_resources(data, want_types):
    """Yield (type_id, name_id, lang_id, blob_offset, blob_size) for resource types in want_types."""
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[e_lfanew:e_lfanew + 4] == b"PE\0\0", "not PE"
    coff = e_lfanew + 4
    num_sections = struct.unpack_from("<H", data, coff + 2)[0]
    opt_size = struct.unpack_from("<H", data, coff + 16)[0]
    opt = coff + 20
    magic = struct.unpack_from("<H", data, opt)[0]
    ddir = opt + (112 if magic == 0x20B else 96)
    rva_rsrc, _size = struct.unpack_from("<II", data, ddir + 2 * 8)
    if not rva_rsrc:
        return
    sec_tab = opt + opt_size
    sections = []
    for i in range(num_sections):
        so = sec_tab + i * 40
        vsize, vaddr, rsize, raddr = struct.unpack_from("<IIII", data, so + 8)
        sections.append((vaddr, vsize, raddr, rsize))

    def rva2off(rva):
        if rva is None:
            return None
        for vaddr, vsize, raddr, rsize in sections:
            if vaddr <= rva < vaddr + max(vsize, rsize):
                return raddr + (rva - vaddr)
        return None

    base = rva2off(rva_rsrc)
    out = []

    def walk(off, level, type_id=0, name_id=0):
        nnamed, nids = struct.unpack_from("<HH", data, off + 12)
        for i in range(nnamed + nids):
            nm, ofs = struct.unpack_from("<II", data, off + 16 + i * 8)
            if ofs & 0x80000000:
                sub_off = base + (ofs & 0x7FFFFFFF)
                ident = nm & 0x7FFFFFFF if nm & 0x80000000 else nm
                if level == 1:
                    walk(sub_off, 2, ident, 0)
                elif level == 2:
                    walk(sub_off, 3, type_id, ident)
            else:
                de = base + ofs
                dva, dsz, _cp, _rv = struct.unpack_from("<IIII", data, de)
                out.append((type_id, name_id, 0, rva2off(dva), dsz))

    walk(base, 1)
    yield from out


def dump_msgtable(data, off):
    nblocks = struct.unpack_from("<I", data, off)[0]
    entries = []
    for i in range(nblocks):
        low, high, ent_off = struct.unpack_from("<III", data, off + 4 + i * 12)
        p = off + ent_off
        for mid in range(low, high + 1):
            length, flags = struct.unpack_from("<HH", data, p)
            if flags == 1:
                txt = utf16z(data, p + 4)
            elif flags == 0:
                txt = ansi_z(data, p + 4)
            else:
                txt = f"<bad flags {flags}>"
            entries.append((mid, flags, txt))
            p += length
    return entries


def main():
    args = [a for a in sys.argv[1:]]
    needle = None
    if "--grep" in args:
        i = args.index("--grep")
        needle = args[i + 1].lower()
        args = args[:i] + args[i + 2:]
    for path in args:
        data = open(path, "rb").read()
        print("=" * 20, path)
        found = False
        for type_id, name_id, _lang, boff, bsize in enum_resources(data, {11}):
            if type_id != 11 or boff is None:
                continue
            found = True
            ents = dump_msgtable(data, boff)
            print(f"  msgtable name={name_id} entries={len(ents)}")
            for mid, flags, txt in ents:
                if needle is None or needle in txt.lower():
                    print(f"    0x{mid:08X} flags={flags} {txt!r}")
        if not found:
            print("  (no RT_MESSAGETABLE)")


if __name__ == "__main__":
    main()
