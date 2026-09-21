"""Python reimplementation of `src/bin/name_field_probe.rs`, for a machine with no Rust toolchain.

Where an asset's name actually lives in its header, measured against the live Cordycep loader.
Cordycep maps a game's fast files into its OWN process without running the game at all, so this
reads Cordycep's memory (read-only: OpenProcess + ReadProcessMemory, nothing written, no game
process involved) the same way Saluki/Greyhound already do to browse assets.

For every one of the loader's 512 pools: sample up to 128 assets, and for each 8-byte-aligned word
in the first 0x80 bytes of the asset's header, check two things -- does the word equal the asset's
own id (the id sitting at a second offset, already known from `snapshot`, rather than a name at
all), or is the word a plausible pointer to a null-terminated string whose FNV-1a hash *equals* the
id? The second case is a name, hash-verified by construction: no lookup table, no guess, the
loader is holding a name and the id it goes with is the game's own proof it belongs there.

Whatever wins the sample is then checked over the WHOLE pool, so the coverage figure is exact.

    python contrib/name_field_probe.py            sample 128 assets/pool (default)
    python contrib/name_field_probe.py all         confirm every asset in the scan phase too

Needs Cordycep (windowed or `.CLI`) running with a game loaded. Writes `logs/names_from_headers_
<game>.csv` as `hash,name` -- confirm_list's own input shape.
"""
import ctypes
import ctypes.wintypes as wintypes
import os
import struct
import subprocess
import sys

BASIS = 0xCBF29CE484222325
PRIME = 0x100000001B3
U64 = 0xFFFFFFFFFFFFFFFF
ID_MASK = 0x7FFFFFFFFFFFFFFF

POOL_COUNT = 512
POOL_STRIDE = 0x28
ASSET_SIZE = 0x60
SCAN_BYTES = 0x80
SCAN_SAMPLE = 128
PLAUSIBLE_POINTER = 0x0000_8000_0000_0000
MAX_STRING_LENGTH = 256

PROCESS_NAMES = ["Cordycep", "Cordycep.CLI"]

PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010

kernel32 = ctypes.windll.kernel32


def fnv1a(text):
    h = BASIS
    for byte in text.strip().lower().replace("\\", "/").encode("utf-8", "replace"):
        h = ((h ^ byte) * PRIME) & U64
    return h


def id_of(text):
    return fnv1a(text) & ID_MASK


def find_loader():
    """(pid, directory, is_cli) for the first running, preferred Cordycep build."""
    out = subprocess.run(
        ["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, check=False
    ).stdout
    found = {}
    for line in out.splitlines():
        parts = [p.strip('"') for p in line.strip().split('","')]
        if len(parts) < 2:
            continue
        name = parts[0]
        stem = name[:-4] if name.lower().endswith(".exe") else name
        for rank, candidate in enumerate(PROCESS_NAMES):
            if stem.lower() == candidate.lower():
                pid = int(parts[1])
                found.setdefault(rank, (pid, candidate))
    if not found:
        return None
    rank = min(found)
    pid, name = found[rank]
    directory = process_directory(pid)
    return pid, directory, name == "Cordycep.CLI"


def process_directory(pid):
    handle = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION, False, pid)
    if not handle:
        return None
    try:
        buf = ctypes.create_unicode_buffer(32768)
        size = wintypes.DWORD(len(buf))
        ok = ctypes.windll.kernel32.QueryFullProcessImageNameW(
            handle, 0, buf, ctypes.byref(size)
        )
        if not ok:
            return None
        return os.path.dirname(buf.value)
    finally:
        kernel32.CloseHandle(handle)


def read_handler_state(directory, is_cli):
    data_dir = os.path.join(directory, "Data")
    if is_cli:
        path = os.path.join(data_dir, "CurrentHandler.csi")
        with open(path, "rb") as fh:
            raw = fh.read()
        if len(raw) < 24:
            raise RuntimeError("Cordycep's state file is truncated -- still loading?")
        game_id = raw[:8].decode("ascii", "replace")
        pools_addr, strings_addr = struct.unpack_from("<qq", raw, 8)
        return game_id, pools_addr, strings_addr
    path = os.path.join(data_dir, "CurrentHandler.json")
    import json
    with open(path, encoding="utf-8") as fh:
        state = json.load(fh)
    packed = state["game_id"]
    game_id = struct.pack("<q", packed if packed < (1 << 63) else packed - (1 << 64)).decode(
        "ascii", "replace"
    )
    return game_id, state["pools_addr"], state["strings_addr"]


class ProcessReader:
    def __init__(self, pid):
        self.handle = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
        if not self.handle:
            raise RuntimeError(f"could not open process {pid} for reading (admin needed?)")

    def close(self):
        if self.handle:
            kernel32.CloseHandle(self.handle)
            self.handle = None

    def try_read(self, address, size):
        if address <= 0 or size <= 0:
            return b""
        buf = ctypes.create_string_buffer(size)
        read = ctypes.c_size_t(0)
        ok = kernel32.ReadProcessMemory(self.handle, ctypes.c_void_p(address), buf, size, ctypes.byref(read))
        if not ok:
            return b""
        return buf.raw[: read.value]

    def read_bytes(self, address, size):
        data = self.try_read(address, size)
        if len(data) != size:
            return None
        return data

    def try_read_u64(self, address):
        data = self.try_read(address, 8)
        if len(data) != 8:
            return 0
        return struct.unpack("<Q", data)[0]

    def read_string(self, address):
        if address <= 0:
            return ""
        out = bytearray()
        addr = address
        while len(out) < MAX_STRING_LENGTH:
            chunk = self.try_read(addr, 128)
            if not chunk:
                chunk = self.try_read(addr, 16)
                if not chunk:
                    break
            terminated = False
            for byte in chunk:
                if byte == 0:
                    terminated = True
                    break
                out.append(byte)
            if terminated:
                break
            addr += len(chunk)
        try:
            return out.decode("ascii")
        except UnicodeDecodeError:
            return ""


def read_pool(reader, pools_address, index):
    raw = reader.read_bytes(pools_address + index * POOL_STRIDE, 40)
    if raw is None:
        return None
    root, end, lookup, hmem, amem = struct.unpack("<5q", raw)
    return root


def iter_assets(reader, root, limit=None):
    """Yields (header, id, owner) for header!=0 and temp==0 entries, like Assets::next."""
    nxt = root
    count = 0
    while nxt != 0:
        raw = reader.read_bytes(nxt, ASSET_SIZE)
        if raw is None:
            return
        fields = struct.unpack("<12q", raw)
        header, temp, nxt_, previous, aid, kind, hsize, edpo, eds, fchild, lchild, owner = fields
        nxt = nxt_
        if header != 0 and temp == 0:
            yield header, aid & 0xFFFFFFFFFFFFFFFF, owner
            count += 1
            if limit is not None and count >= limit:
                return


def main():
    sample = SCAN_SAMPLE
    if len(sys.argv) > 1:
        sample = None if sys.argv[1] == "all" else int(sys.argv[1])

    loader = find_loader()
    if not loader:
        print("Cordycep is not running. Start Cordycep and load a game first.", file=sys.stderr)
        raise SystemExit(1)
    pid, directory, is_cli = loader
    if not directory:
        print("could not locate Cordycep's directory", file=sys.stderr)
        raise SystemExit(1)

    game_id, pools_address, strings_address = read_handler_state(directory, is_cli)
    print(f"the loader has {game_id} open")
    print(f"scanning {SCAN_BYTES:#x} bytes of each asset header, "
          f"{'all' if sample is None else sample} assets per pool\n")

    reader = ProcessReader(pid)

    root = os.path.dirname(os.path.abspath(__file__))
    while not os.path.isfile(os.path.join(root, "scripts", "snapshot.py")) and os.path.dirname(root) != root:
        root = os.path.dirname(root)
    logs_dir = os.path.join(root, "logs")
    os.makedirs(logs_dir, exist_ok=True)
    out_path = os.path.join(logs_dir, f"names_from_headers_{game_id.lower()}.csv")

    recovered_total = 0
    rows = []

    with open(out_path, "w", encoding="utf-8") as out:
        for pool in range(POOL_COUNT):
            root_addr = read_pool(reader, pools_address, pool)
            if not root_addr:
                continue

            id_hits = {}
            text_hits = {}
            scanned = 0

            for header, aid, owner in iter_assets(reader, root_addr, limit=sample):
                header_bytes = reader.read_bytes(header, SCAN_BYTES)
                if header_bytes is None:
                    continue
                scanned += 1

                for offset in range(0, SCAN_BYTES, 8):
                    word = struct.unpack_from("<Q", header_bytes, offset)[0]

                    if (word & ID_MASK) == aid:
                        id_hits[offset] = id_hits.get(offset, 0) + 1
                        continue

                    if word != 0 and word < PLAUSIBLE_POINTER:
                        text = reader.read_string(word)
                        if text and id_of(text) == aid:
                            text_hits[offset] = text_hits.get(offset, 0) + 1

            if scanned == 0:
                continue

            def best(hits):
                if not hits:
                    return None
                top = max(hits.values())
                if top * 2 < scanned:
                    return None
                for off in sorted(hits):
                    if hits[off] == top:
                        return off
                return None

            id_off = best(id_hits)
            text_off = best(text_hits)

            if id_off == 0:
                kind, offset = "id", 0
            elif id_off is not None and text_off is None:
                kind, offset = "id", id_off
            elif id_off is not None and text_off is not None and id_off <= text_off:
                kind, offset = "id", id_off
            elif text_off is not None:
                kind, offset = "name as text", text_off
            else:
                kind, offset = "not in the header", 0

            total = 0
            covered = 0

            if kind == "name as text":
                for header, aid, owner in iter_assets(reader, root_addr):
                    total += 1
                    word = reader.try_read_u64(header + offset)
                    if word and word < PLAUSIBLE_POINTER:
                        text = reader.read_string(word)
                        if text and id_of(text) == aid:
                            covered += 1
                            recovered_total += 1
                            out.write(f"{aid:016x},{text}\n")
            elif kind == "id":
                for header, aid, owner in iter_assets(reader, root_addr):
                    total += 1
                    word = reader.try_read_u64(header + offset)
                    if (word & ID_MASK) == aid:
                        covered += 1
            else:
                for header, aid, owner in iter_assets(reader, root_addr):
                    total += 1

            rows.append((pool, total, kind, offset, covered))
            pct = 0.0 if total == 0 else covered * 100.0 / total
            print(f"pool {pool:3}  {total:7} assets  {kind:17} at +0x{offset:02x}  "
                  f"covers {covered:7} ({pct:5.1f}%)")

    reader.close()

    print("\n---")
    at_zero = sum(1 for r in rows if r[2] == "id" and r[3] == 0)
    elsewhere = sum(1 for r in rows if r[2] == "id" and r[3] != 0)
    as_text = sum(1 for r in rows if r[2] == "name as text")
    nowhere = sum(1 for r in rows if r[2] == "not in the header")

    print(f"pools examined:                      {len(rows)}")
    print(f"id at header+0x00:                   {at_zero}")
    print(f"id at some other offset:             {elsewhere}")
    print(f"name readable as text, hash checked: {as_text}")
    print(f"name not in the header at all:       {nowhere}")
    print(f"\nnames recovered and hash-verified:   {recovered_total}  -> {out_path}")


if __name__ == "__main__":
    main()
