#!/usr/bin/env python3
"""Dump Widget Workshop's puzzle file (WIDGET.PZL), read-only.

WIDGET.PZL is a classic Mac resource fork stored as a plain file (data
offset 0x100).  It holds three resource types:

  TMPL 128 "Done"   ResEdit template describing the 'Done' resource
  Done 1            u16 count, then one 2-byte Mac Boolean per puzzle
                    (the shipped defaults: all unsolved)
  Puzl 1..25        one saved widget each (FORM Gadf, exactly as a .WGT
                    file); the resource name is the puzzle's title

See docs/puzzles.md for the full description.

Usage:  python3 ghidra/pzl_dump.py [path/to/WIDGET.PZL] [-v] [-n N]
  -v     also list every gadget / wire / tray entry of each puzzle
  -n N   only puzzle N
"""

import struct
import sys

DEFAULT = "orig/WIN95/WIDGET/WIDGET.PZL"

# Stopper ("Win / Lose Checker") comparison radio group.
CMP = ["==", "!=", ">", "<", ">=", "<="]

TEXT_TAGS = {b"TxLb", b"TxLn", b"TxBx"}


def be16(b, o):
    return struct.unpack_from(">H", b, o)[0]


def be32(b, o):
    return struct.unpack_from(">I", b, o)[0]


def mac(b):
    return b.decode("mac_roman").replace("\r", "\\r")


def cstr(b):
    return b.split(b"\0", 1)[0]


def read_fork(d):
    """Return (header, {(type, id): (name, attrs, data)}, type order)."""
    data_off, map_off, data_len, map_len = struct.unpack_from(">4I", d, 0)
    m = d[map_off:map_off + map_len]
    map_attrs, tl_off, nl_off, ntypes_m1 = struct.unpack_from(">HHHH", m, 22)
    res = {}
    order = []
    for i in range(ntypes_m1 + 1):
        rtype, n_m1, ref_off = struct.unpack_from(">4sHH", m, tl_off + 2 + 8 * i)
        order.append((rtype, n_m1 + 1, ref_off))
        for j in range(n_m1 + 1):
            e = tl_off + ref_off + 12 * j
            rid, name_off, attrs = struct.unpack_from(">hHB", m, e)
            off = int.from_bytes(m[e + 5:e + 8], "big")
            name = None
            if name_off != 0xFFFF:
                ln = m[nl_off + name_off]
                name = m[nl_off + name_off + 1:nl_off + name_off + 1 + ln]
            size = be32(d, data_off + off)
            body = d[data_off + off + 4:data_off + off + 4 + size]
            res[(rtype, rid)] = (name, attrs, body, off)
    hdr = dict(data_off=data_off, map_off=map_off, data_len=data_len,
               map_len=map_len, map_attrs=map_attrs, tl_off=tl_off,
               nl_off=nl_off)
    return hdr, res, order


def chunks(form):
    """Split a FORM Gadf the way CWidgetDoc's loader (0x42503e) does."""
    assert form[:4] == b"FORM" and form[8:12] == b"Gadf"
    at = 12
    while at + 8 <= len(form):
        tag = form[at:at + 4]
        size = be32(form, at + 4)
        yield tag, form[at + 8:at + 8 + size]
        at += 8 + size


def gadget_info(tag, body):
    gid = be32(body, 0)
    v, h = struct.unpack_from(">hh", body, 4)
    blk = body[8:8 + 0x108]
    checks = be32(blk, 0)
    radios = list(blk[4:8])
    nums = [be32(blk, 8 + 4 * k) for k in range(4)]
    texts = [cstr(blk[0x18 + 60 * k:0x18 + 60 * (k + 1)]) for k in range(4)]
    locked = body[0x110]
    return dict(tag=tag, id=gid, pos=(v, h), checks=checks, radios=radios,
                nums=nums, texts=texts, locked=locked, size=len(body),
                bigtext=cstr(blk[0x18:0x18 + 240]))


def describe_stopper(g):
    r = g["radios"]
    if r[0] == 0:
        cond = "input is %s" % ("True" if r[1] == 0 else "False")
    else:
        cond = "input %s %s" % (CMP[r[2]] if r[2] < 6 else "?%d" % r[2],
                                mac(g["texts"][0]))
    kind = "WIN" if r[3] == 0 else "LOSE"
    return "%s when %s" % (kind, cond)


def main():
    args = sys.argv[1:]
    verbose = "-v" in args
    only = None
    if "-n" in args:
        only = int(args[args.index("-n") + 1])
        del args[args.index("-n"):args.index("-n") + 2]
    args = [a for a in args if a != "-v"]
    path = args[0] if args else DEFAULT
    d = open(path, "rb").read()
    hdr, res, order = read_fork(d)

    if only is None:
        print("file %s: %d bytes" % (path, len(d)))
        print("header: data_off=%#x map_off=%#x data_len=%#x map_len=%#x"
              % (hdr["data_off"], hdr["map_off"], hdr["data_len"], hdr["map_len"]))
        print("map: attrs=%#x typelist_off=%#x namelist_off=%#x"
              % (hdr["map_attrs"], hdr["tl_off"], hdr["nl_off"]))
        for rtype, n, ref_off in order:
            print("type %s: %d resource(s), refs at typelist+%#x"
                  % (rtype.decode(), n, ref_off))
        name, attrs, body, off = res[(b"TMPL", 128)]
        print("TMPL 128 %r (%d bytes): %s" % (mac(name), len(body), body.hex()))
        name, attrs, body, off = res[(b"Done", 1)]
        cnt = be16(body, 0)
        flags = [be16(body, 2 + 2 * k) for k in range((len(body) - 2) // 2)]
        print("Done 1 (%d bytes): count word=%d, %d flags: %s"
              % (len(body), cnt, len(flags), "".join("1" if f else "0" for f in flags)))
        print()

    ids = sorted(rid for (t, rid) in res if t == b"Puzl")
    for rid in ids:
        if only is not None and rid != only:
            continue
        name, attrs, form, off = res[(b"Puzl", rid)]
        gadgets, wires, tray, back, fver = [], [], [], None, None
        for tag, body in chunks(form):
            if tag == b"fver":
                fver = be16(body, 0)
            elif tag == b"back":
                back = be16(body, 0)
            elif tag == b"conn":
                fr, to, fk, tk = struct.unpack_from(">IIHH", body, 0)
                lock = body[12] if len(body) > 12 else 0
                npts = be16(body, 14) if len(body) > 15 else 0
                wires.append((fr, fk, to, tk, lock, npts))
            elif tag == b"numG":
                n = be16(body, 0)
                for k in range(n):
                    t = body[2 + 6 * k:6 + 6 * k]
                    c = struct.unpack_from(">h", body, 6 + 6 * k)[0]
                    tray.append((t.decode("latin-1"), c))
            else:
                gadgets.append(gadget_info(tag.decode("latin-1"), body))
        texts = []
        for g in gadgets:
            if g["tag"] in ("TxLb", "TxLn"):
                texts.append(mac(g["texts"][0]))
            elif g["tag"] == "TxBx":
                texts.append(mac(g["bigtext"]))
        stoppers = [describe_stopper(g) for g in gadgets if g["tag"] == "Stop"]
        locked = sum(1 for g in gadgets if g["locked"])
        lockedw = sum(1 for w in wires if w[4])
        avail = [(t, c) for t, c in tray if c != 0]
        print("Puzzle %2d  %-24s FORM %5d bytes, fver %s, back %s"
              % (rid, '"%s"' % mac(name), len(form), fver, back))
        print("  gadgets %d (%d locked), wires %d (%d locked), numG entries %d"
              % (len(gadgets), locked, len(wires), lockedw, len(tray)))
        print("  checker: %s" % ("; ".join(stoppers) if stoppers else "none"))
        loose = [g["tag"] for g in gadgets if not g["locked"]]
        print("  unlocked parts: %s" % (" ".join(loose) if loose else "-"))
        print("  tray (non-zero): %s" % (" ".join("%s=%s" % (t, "inf" if c == -1 else c)
                                                  for t, c in avail) if avail else "-"))
        print("  text: %s" % " | ".join(t for t in texts if t))
        if verbose:
            for g in gadgets:
                print("    %s id=%08x pos=%s lock=%d checks=%08x radios=%s nums=%s texts=%s"
                      % (g["tag"], g["id"], g["pos"], g["locked"], g["checks"],
                         g["radios"], g["nums"], [mac(t) for t in g["texts"]]))
            for fr, fk, to, tk, lock, npts in wires:
                print("    conn %08x.%d -> %08x.%d lock=%d bends=%d"
                      % (fr, fk, to, tk, lock, npts))
            print("    numG: %s" % " ".join("%s=%d" % tc for tc in tray))
        print()


if __name__ == "__main__":
    main()
