#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -eq 0 ]]; then
  echo 'Usage: tools/reel/render.sh /path/to/Fighters-Anthology [--stage all|record|capture|edit|verify]'
  exit 2
fi
for tool in cargo ffmpeg ffprobe python3; do command -v "$tool" >/dev/null; done
mkdir -p out/reel
if [[ ! -x out/reel/venv/bin/python && ! -x out/reel/venv/Scripts/python.exe ]]; then python3 -m venv out/reel/venv; fi
reel_python=out/reel/venv/bin/python
if [[ -x out/reel/venv/Scripts/python.exe ]]; then reel_python=out/reel/venv/Scripts/python.exe; fi
"$reel_python" -c 'import PIL, numpy' 2>/dev/null || "$reel_python" -m pip install -r tools/reel/requirements.txt
exec "$reel_python" tools/reel/render.py "$@"
