#!/usr/bin/env sh
# Thin POSIX wrapper: `cmd 2>&1 | scripts/filter.sh` or `scripts/filter.sh -- cmd...`.
# The implementation is scripts/filter.py so the same filter runs from PowerShell.
exec python -I "$(dirname "$0")/filter.py" "$@"
