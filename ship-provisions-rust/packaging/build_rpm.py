#!/usr/bin/env python3
"""Build a binary RPM for the Ship Provisions Tracker (Linux x86_64) without
needing rpmbuild: writes lead + signature header + header + gzipped cpio payload.

Usage: python3 packaging/build_rpm.py [path-to-ShipProvisions-binary] [output.rpm]
"""

import gzip
import hashlib
import io
import os
import struct
import sys
import time

NAME = "ship-provisions"
VERSION = "1.0.0"
RELEASE = "1"
ARCH = "x86_64"

DESKTOP = """[Desktop Entry]
Name=Ship Provisions Tracker
Comment=Track provisions inventory on board (PDF invoices, stocktakes, cost per person)
Exec=/usr/bin/ShipProvisions
Icon=ship-provisions
Type=Application
Categories=Office;
Terminal=false
"""

# RPM header tags
TAG_NAME = 1000
TAG_VERSION = 1001
TAG_RELEASE = 1002
TAG_SUMMARY = 1004
TAG_DESCRIPTION = 1005
TAG_BUILDTIME = 1006
TAG_BUILDHOST = 1007
TAG_SIZE = 1009
TAG_LICENSE = 1014
TAG_PACKAGER = 1015
TAG_GROUP = 1016
TAG_URL = 1021
TAG_OS = 1022
TAG_ARCH = 1023
TAG_SOURCERPM = 1044
TAG_PROVIDENAME = 1047
TAG_REQUIREFLAGS = 1048
TAG_REQUIRENAME = 1049
TAG_REQUIREVERSION = 1050
TAG_DIRINDEXES = 1116
TAG_BASENAMES = 1117
TAG_DIRNAMES = 1118
TAG_FILESIZES = 1028
TAG_FILEMODES = 1030
TAG_FILEMTIMES = 1034
TAG_FILEDIGESTS = 1035
TAG_FILELINKTOS = 1036
TAG_FILEFLAGS = 1037
TAG_FILEUSERNAME = 1039
TAG_FILEGROUPNAME = 1040
TAG_PROVIDEFLAGS = 1112
TAG_PROVIDEVERSION = 1113
TAG_PAYLOADFORMAT = 1124
TAG_PAYLOADCOMPRESSOR = 1125
TAG_PAYLOADFLAGS = 1126

# RPM data types
TYPE_INT16 = 3
TYPE_INT32 = 4
TYPE_STRING = 6
TYPE_STRING_ARRAY = 8
TYPE_I18NSTRING = 9


def build_header(entries):
    """entries: list of (tag, type, count, data_bytes). Returns header blob."""
    entries = sorted(entries, key=lambda e: e[0])
    data = b""
    index = []
    for tag, typ, count, blob in entries:
        index.append((tag, typ, len(data), count))
        data += blob
    n = len(index)
    hsize = len(data)
    out = bytearray()
    out += b"\x8e\xad\xe8\x01" + b"\x00" * 4
    out += struct.pack(">II", n, hsize)
    for tag, typ, off, count in index:
        out += struct.pack(">IIII", tag, typ, off, count)
    out += data
    while len(out) % 8:
        out += b"\x00"
    return bytes(out)


def s_string(v):
    return v.encode() + b"\x00"


def s_string_array(vs):
    return b"".join(v.encode() + b"\x00" for v in vs)


def s_int32(vs):
    return b"".join(struct.pack(">i", v) for v in vs)


def s_int16(vs):
    return b"".join(struct.pack(">h", v) for v in vs)


def cpio_newc(entries):
    """entries: list of (path, mode, data bytes or None for dirs). Returns cpio blob."""
    out = io.BytesIO()
    ino = 1
    for path, mode, data in entries:
        name = path.encode() + b"\x00"
        is_dir = data is None
        filesize = 0 if is_dir else len(data)
        nlink = 2 if is_dir else 1
        hdr = b"070701" + b"".join(
            f"{x:08X}".encode()
            for x in (
                ino,
                mode,
                0,
                0,
                nlink,
                int(time.time()),
                filesize,
                0,
                0,
                0,
                0,
                len(name),
                0,
            )
        )
        out.write(hdr)
        out.write(name)
        while out.tell() % 4:
            out.write(b"\x00")
        if not is_dir:
            out.write(data)
            while out.tell() % 4:
                out.write(b"\x00")
        ino += 1
    # trailer
    name = b"TRAILER!!!\x00"
    hdr = b"070701" + b"".join(
        f"{x:08X}".encode()
        for x in (0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, len(name), 0)
    )
    out.write(hdr)
    out.write(name)
    while out.tell() % 512:
        out.write(b"\x00")
    return out.getvalue()


def build_rpm(binary_path, out_path):
    with open(binary_path, "rb") as f:
        binary = f.read()
    binary_mode = 0o755

    files = [
        ("/usr/bin/ShipProvisions", binary_mode, binary),
        ("/usr/share/applications/ship-provisions.desktop", 0o644, DESKTOP.encode()),
    ]
    dirs = ["/usr", "/usr/bin", "/usr/share", "/usr/share/applications"]

    cpio_entries = [(d, 0o40755, None) for d in dirs] + files
    payload = cpio_newc(cpio_entries)
    payload_gz = gzip.compress(payload, compresslevel=9)

    basenames = [os.path.basename(p) for p, _, _ in files]
    dirnames = sorted({os.path.dirname(p) for p, _, _ in files})
    dirindexes = [dirnames.index(os.path.dirname(p)) for p, _, _ in files]
    now = int(time.time())

    main_header = build_header([
        (TAG_NAME, TYPE_STRING, 1, s_string(NAME)),
        (TAG_VERSION, TYPE_STRING, 1, s_string(VERSION)),
        (TAG_RELEASE, TYPE_STRING, 1, s_string(RELEASE)),
        (TAG_SUMMARY, TYPE_I18NSTRING, 1, s_string("Ship provisions inventory tracker")),
        (TAG_DESCRIPTION, TYPE_I18NSTRING, 1, s_string(
            "Desktop app to track provisions inventory on board a ship: import proforma "
            "invoice PDFs, crew inventory checklists/stocktakes, crew-on-board log, "
            "consumption and cost per person insights, Excel report export. Data stored "
            "locally in SQLite (~/.local/share/ShipProvisions/provisions.db).")),
        (TAG_BUILDTIME, TYPE_INT32, 1, s_int32([now])),
        (TAG_BUILDHOST, TYPE_STRING, 1, s_string("localhost")),
        (TAG_SIZE, TYPE_INT32, 1, s_int32([len(payload)])),
        (TAG_LICENSE, TYPE_STRING, 1, s_string("MIT")),
        (TAG_PACKAGER, TYPE_STRING, 1, s_string("Ship office")),
        (TAG_GROUP, TYPE_STRING, 1, s_string("Applications/Office")),
        (TAG_URL, TYPE_STRING, 1, s_string("https://github.com/Nelix983/ship-provisions")),
        (TAG_OS, TYPE_STRING, 1, s_string("linux")),
        (TAG_ARCH, TYPE_STRING, 1, s_string(ARCH)),
        (TAG_SOURCERPM, TYPE_STRING, 1, s_string("none")),
        (TAG_PROVIDENAME, TYPE_STRING_ARRAY, 1, s_string_array([NAME])),
        (TAG_PROVIDEFLAGS, TYPE_INT32, 1, s_int32([8])),  # RPMSENSE_EQUAL
        (TAG_PROVIDEVERSION, TYPE_STRING_ARRAY, 1, s_string_array([VERSION])),
        (TAG_REQUIREFLAGS, TYPE_INT32, 3, s_int32([0, 0, 0])),
        (TAG_REQUIRENAME, TYPE_STRING_ARRAY, 3, s_string_array([
            "libc.so.6()(64bit)", "libm.so.6()(64bit)", "libgcc_s.so.1()(64bit)"])),
        (TAG_REQUIREVERSION, TYPE_STRING_ARRAY, 3, s_string_array(["", "", ""])),
        (TAG_DIRINDEXES, TYPE_INT32, len(files), s_int32(dirindexes)),
        (TAG_BASENAMES, TYPE_STRING_ARRAY, len(files), s_string_array(basenames)),
        (TAG_DIRNAMES, TYPE_STRING_ARRAY, len(dirnames), s_string_array(dirnames)),
        (TAG_FILESIZES, TYPE_INT32, len(files), s_int32([len(d) for _, _, d in files])),
        (TAG_FILEMODES, TYPE_INT16, len(files), s_int16([m for _, m, _ in files])),
        (TAG_FILEMTIMES, TYPE_INT32, len(files), s_int32([now] * len(files))),
        (TAG_FILEDIGESTS, TYPE_STRING_ARRAY, len(files),
         s_string_array([hashlib.md5(d).hexdigest() for _, _, d in files])),
        (TAG_FILELINKTOS, TYPE_STRING_ARRAY, len(files), s_string_array([""] * len(files))),
        (TAG_FILEFLAGS, TYPE_INT32, len(files), s_int32([0] * len(files))),
        (TAG_FILEUSERNAME, TYPE_STRING_ARRAY, len(files), s_string_array(["root"] * len(files))),
        (TAG_FILEGROUPNAME, TYPE_STRING_ARRAY, len(files), s_string_array(["root"] * len(files))),
        (TAG_PAYLOADFORMAT, TYPE_STRING, 1, s_string("cpio")),
        (TAG_PAYLOADCOMPRESSOR, TYPE_STRING, 1, s_string("gzip")),
        (TAG_PAYLOADFLAGS, TYPE_STRING, 1, s_string("9")),
    ])

    sig_header = build_header([
        (1000, TYPE_INT32, 1, s_int32([len(main_header) + len(payload_gz)])),  # RPMSIGTAG_SIZE
    ])

    lead = bytearray(96)
    lead[0:4] = b"\xed\xab\xee\xdb"
    lead[4] = 3
    lead[5] = 0
    struct.pack_into(">H", lead, 6, 0)      # type: binary
    struct.pack_into(">H", lead, 8, 1)      # archnum
    name = f"{NAME}-{VERSION}".encode()
    lead[10:10 + len(name)] = name
    struct.pack_into(">H", lead, 10 + 66, 1)  # osnum: linux
    struct.pack_into(">H", lead, 10 + 68, 5)  # sigtype: headersig

    with open(out_path, "wb") as f:
        f.write(bytes(lead))
        f.write(sig_header)
        f.write(main_header)
        f.write(payload_gz)
    return len(payload_gz)


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.dirname(here)
    binary = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, "target", "release", "ShipProvisions")
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(root, "dist", f"{NAME}-{VERSION}-{RELEASE}.{ARCH}.rpm")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    size = build_rpm(binary, out)
    print(f"wrote {out} ({os.path.getsize(out):,} bytes, payload {size:,} bytes) from {binary}")
