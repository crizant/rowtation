#!/usr/bin/env python3
"""Generate the deterministic 2M-row scroll fixture outside tracked source files."""
import hashlib
import sys
from pathlib import Path

path = Path(sys.argv[1])
path.parent.mkdir(parents=True, exist_ok=True)
digest = hashlib.sha256()
with path.open("wb") as output:
    header = b"id,city,category,amount,paid,note\n"
    output.write(header)
    digest.update(header)
    for start in range(0, 2_000_000, 10_000):
        data = "".join(
            f'{i:07d},City {i % 200},Category {i % 20},{i % 100000}.00,'
            f'{"true" if i % 2 else "false"},Synthetic row {i}\n'
            for i in range(start, start + 10_000)
        ).encode()
        output.write(data)
        digest.update(data)
print(f"{path}: {path.stat().st_size} bytes; SHA-256 {digest.hexdigest()}")
