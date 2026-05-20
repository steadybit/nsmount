#!/usr/bin/env bash
#
# Verify that the listed ELF binaries do not require a glibc symbol
# version greater than the given maximum.
#
# This binary is shipped as part of the Steadybit host / container
# extensions, which target enterprise distros like RHEL 8 (glibc 2.28).
# A toolchain or dependency change that silently pulls in a newer
# GLIBC_x.y symbol would break installations on those distros, so CI
# fails fast here.
#
# Usage: check-glibc.sh <max-glibc-version> <binary> [binary ...]

set -euo pipefail

if [ "$#" -lt 2 ]; then
	echo "Usage: $0 <max-glibc-version> <binary> [binary ...]" >&2
	exit 2
fi

MAX="$1"
shift

fail=0
for bin in "$@"; do
	echo "=== $bin ==="
	if [ ! -f "$bin" ]; then
		echo "  ERROR: not found"
		fail=1
		continue
	fi

	versions=$(objdump -T "$bin" 2>/dev/null \
		| grep -oE 'GLIBC_[0-9]+\.[0-9]+(\.[0-9]+)?' \
		| sed 's/^GLIBC_//' \
		| sort -uV)

	if [ -z "$versions" ]; then
		echo "  No GLIBC symbol versions required (statically linked or no libc deps)"
		continue
	fi

	max_found=$(echo "$versions" | tail -n1)
	echo "  Max GLIBC required: $max_found (allowed: $MAX)"

	if [ "$(printf '%s\n%s\n' "$max_found" "$MAX" | sort -V | tail -n1)" != "$MAX" ]; then
		echo "  FAIL: requires GLIBC $max_found, max allowed is $MAX"
		echo "  Symbols above GLIBC_$MAX:"
		objdump -T "$bin" | awk -v max="$MAX" '
			function vgt(a, b,    aa, bb, i) {
				split(a, aa, ".")
				split(b, bb, ".")
				for (i = 1; i <= 3; i++) {
					if ((aa[i]+0) > (bb[i]+0)) return 1
					if ((aa[i]+0) < (bb[i]+0)) return 0
				}
				return 0
			}
			match($0, /GLIBC_[0-9]+\.[0-9]+(\.[0-9]+)?/) {
				v = substr($0, RSTART + 6, RLENGTH - 6)
				if (vgt(v, max)) print "    " $0
			}'
		fail=1
	fi
done

exit "$fail"
