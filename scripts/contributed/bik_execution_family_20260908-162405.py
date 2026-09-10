"""Probe the observed bik_execution_ three-digit sound-alias family.

The reach report shows 145 confirmed members with this prefix, while its measured
beginning is not carried.  Emit only missing members of the observed 000..999
numbered family; this is a bounded family-gap probe, not a general sweep.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import snapshot

PREFIX = "bik_execution_"
PATTERN = re.compile(r"^bik_execution_(\d{3})$")


def main():
    known = {
        name.strip().lower().replace("\\", "/")
        for name in snapshot.confirmed_names("sound_alias")
    }
    for number in range(1000):
        candidate = f"{PREFIX}{number:03d}"
        if candidate not in known:
            print(candidate)


if __name__ == "__main__":
    main()
