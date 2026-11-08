#!/usr/bin/env python3
"""Create a release bundle zip without depending on zip(1).

Usage: zip-bundle.py STAGE_DIR OUTPUT.zip

Staged entries are flat files, which is all a release bundle contains. Unix
modes are recorded explicitly (0755 for anything already executable, 0644
otherwise) because the fallback path exists precisely for runners where zip(1)
is unavailable -- and losing the executable bit on `aliasc` or
`alias-install.sh` would make the bundle useless after extraction.

Timestamps are pinned to 1980-01-01 so the archive is reproducible: rebuilding
the same inputs produces the same bytes.
"""

import os
import sys
import zipfile

FIXED_DATE_TIME = (1980, 1, 1, 0, 0, 0)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[2], file=sys.stderr)
        return 2
    stage, output = sys.argv[1], sys.argv[2]
    if not os.path.isdir(stage):
        print(f"zip-bundle: not a directory: {stage}", file=sys.stderr)
        return 1

    entries = sorted(os.listdir(stage))
    if not entries:
        print(f"zip-bundle: refusing to write an empty bundle: {stage}", file=sys.stderr)
        return 1

    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for entry in entries:
            path = os.path.join(stage, entry)
            if not os.path.isfile(path):
                continue
            mode = 0o755 if os.access(path, os.X_OK) else 0o644
            info = zipfile.ZipInfo(entry, date_time=FIXED_DATE_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = mode << 16
            # Mark regular files so extractors do not try to create directories.
            info.external_attr |= 0o100000
            with open(path, "rb") as fh:
                zf.writestr(info, fh.read())
    return 0


if __name__ == "__main__":
    sys.exit(main())
