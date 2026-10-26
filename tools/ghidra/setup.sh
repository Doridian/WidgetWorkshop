#!/usr/bin/env bash
# (Re)create the headless Ghidra project in ghidra/ and analyze the Win95 build:
# WIDGET.EXE plus the DLLs that carry code (WWGADGET, WWSOUND, WWINTRO).
# Usage: tools/ghidra/setup.sh [--force]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GHIDRA="${GHIDRA_INSTALL_DIR:-/opt/ghidra}"
PROJ="$ROOT/ghidra"
SCRIPTS="$ROOT/tools/ghidra/scripts"
SRC="$ROOT/orig/WIN95/WIDGET"

if [[ ! -f "$SRC/WIDGET.EXE" ]]; then
	echo "$SRC/WIDGET.EXE missing; run tools/extract_cd.py first" >&2
	exit 1
fi
if [[ -e "$PROJ/WidgetWorkshop.gpr" ]]; then
	if [[ "${1:-}" != "--force" ]]; then
		echo "ghidra/WidgetWorkshop.gpr exists; pass --force to delete and re-create it" >&2
		exit 1
	fi
	rm -rf "$PROJ/WidgetWorkshop.gpr" "$PROJ/WidgetWorkshop.rep"
fi
mkdir -p "$PROJ/logs"
rm -f "$PROJ"/logs/setup*

exec 9>"$PROJ/.gh.lock"
flock 9
for f in WIDGET.EXE WWGADGET.DLL WWSOUND.DLL WWINTRO.DLL; do
	"$GHIDRA/support/analyzeHeadless" "$PROJ" WidgetWorkshop \
		-import "$SRC/$f" \
		-scriptPath "$SCRIPTS" \
		-postScript WidgetPost.java \
		-postScript GhCli.java "$PROJ/logs/setup-$f-apply.out" apply \
		-log "$PROJ/logs/setup-$f.log" -scriptlog "$PROJ/logs/setup-$f-script.log" \
		>"$PROJ/logs/setup-$f.stdout" 2>&1 || { tail -50 "$PROJ/logs/setup-$f.stdout"; exit 1; }
	grep -h "WidgetPost:" "$PROJ/logs/setup-$f-script.log" || true
	cat "$PROJ/logs/setup-$f-apply.out" 2>/dev/null || true
done
echo "done: $PROJ/WidgetWorkshop.gpr"
