#!/bin/sh
# Forward container stop to the existing console so listings, players and
# router mappings are closed by the server, even without an attached terminal.
set -eu
pipe_dir=$(mktemp -d)
mkfifo "$pipe_dir/console"
exec 3<>"$pipe_dir/console"
rm -r "$pipe_dir"
interrupted=0
trap 'interrupted=1; printf "quit\n" >&3' TERM INT
# A non-interactive POSIX shell gives a background command /dev/null as its
# stdin, and dash does so even with `<&0`. Keep the console on its own
# descriptor first, so the background cat reads the real terminal or pipe.
exec 4<&0
cat <&4 >&3 &
exec 4<&-
input_pid=$!
nice -n 5 tore-server "$@" <&3 &
server_pid=$!
status=0
while :; do
    interrupted=0
    wait "$server_pid" && status=0 || status=$?
    # A signal interrupts wait. Reap again even if the server already exited
    # after consuming quit, so that its status replaces the interrupted wait's.
    [ "$interrupted" -eq 1 ] || break
done
kill "$input_pid" 2>/dev/null || true
wait "$input_pid" 2>/dev/null || true
exit "$status"
