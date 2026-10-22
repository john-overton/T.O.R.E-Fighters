#!/bin/sh
# Forward container stop to the existing console so listings, players and
# router mappings are closed by the server, even without an attached terminal.
set -eu
pipe_dir=$(mktemp -d)
mkfifo "$pipe_dir/console"
exec 3<>"$pipe_dir/console"
rm -r "$pipe_dir"
trap 'printf "quit\n" >&3' TERM INT
# Explicit stdin redirection keeps a background cat from receiving /dev/null.
cat <&0 >&3 &
input_pid=$!
nice -n 5 tore-server "$@" <&3 &
server_pid=$!
status=0
while :; do
    wait "$server_pid" && status=0 || status=$?
    # A signal interrupts wait before the server consumes quit.
    kill -0 "$server_pid" 2>/dev/null || break
done
kill "$input_pid" 2>/dev/null || true
wait "$input_pid" 2>/dev/null || true
exit "$status"
