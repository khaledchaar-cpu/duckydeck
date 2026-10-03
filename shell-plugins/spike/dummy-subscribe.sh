#!/usr/bin/env bash
# Stand-in for `duckydeck subscribe`: emits JSON lines, then exits to test restarts.
for i in 1 2 3 4 5; do
  printf '{"v":1,"event":"tick","seq":%d,"pid":%d}\n' "$i" "$$"
  sleep 1
done
echo 'not json'
exit 3
