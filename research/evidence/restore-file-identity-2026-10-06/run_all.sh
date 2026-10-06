#!/bin/bash
# run_all.sh — runs both halves of the G1 file-identity measurements and saves
# their output next to this script:
#   results-macos.txt   measure_macos.sh on this Mac
#   results-linux.txt   measure_linux_container.sh inside a Docker container
#                         (only when Docker is installed and running)
# Usage: run_all.sh [OTHER_VOLUME]

set -euo pipefail
cd "$(dirname "$0")"

chmod +x measure_macos.sh measure_linux_container.sh
./measure_macos.sh "$@" > results-macos.txt
echo "wrote results-macos.txt"

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  IMAGE="${IMAGE:-ubuntu:22.04}"
  docker run --rm -i --tmpfs /tmpfs "$IMAGE" bash -s < measure_linux_container.sh > results-linux.txt
  echo "wrote results-linux.txt"
else
  echo "Docker not installed or not running; Linux half skipped" | tee results-linux.txt
fi
