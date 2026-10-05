#!/usr/bin/env bash
# Run a command whose window should not appear on the user's active workspace.
#
#   tools/agent-run.sh cargo run --locked -p tore-app -- --smoke-test
#
# On Hyprland this launches the command on its own spare workspace, silently
# (no focus change, no workspace switch), floating so the window keeps the size
# the game asks for. Each concurrent run claims a different workspace from
# TORE_AGENT_WS_FIRST..TORE_AGENT_WS_LAST (default 91..99). Output, exit status
# and cwd behave as if the command ran directly, and stopping the wrapper with
# INT or TERM stops the command. Outside Hyprland the command
# just runs in place.
set -u

# An agent's hosted game never asks a real router to forward its port.
export TORE_NO_PORT_MAPPING=1

if [ "$#" -eq 0 ]; then
    echo "usage: tools/agent-run.sh COMMAND [ARGS...]" >&2
    exit 2
fi

if [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] || ! command -v hyprctl >/dev/null 2>&1; then
    exec "$@"
fi

first=${TORE_AGENT_WS_FIRST:-91}
last=${TORE_AGENT_WS_LAST:-99}
lock_root=${XDG_RUNTIME_DIR:-/tmp}/tore-agent-workspaces
work=$(mktemp -d "${TMPDIR:-/tmp}/tore-agent-run.XXXXXX")
mkdir -p "$lock_root"

workspace=
# Signal a process and everything under it, children first.
kill_tree() {
    local child
    for child in $(pgrep -P "$1" 2>/dev/null); do
        kill_tree "$child"
    done
    kill -TERM "$1" 2>/dev/null
}

cleanup() {
    # Hyprland owns the launched command, so if this wrapper is stopped before
    # the command finishes, stop the command too.
    if [ -s "$work/pid" ] && [ ! -s "$work/rc" ]; then
        kill_tree "$(cat "$work/pid")"
    fi
    [ -n "$workspace" ] && rm -rf "${lock_root:?}/$workspace"
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# Claim a workspace with mkdir (atomic). A lock left by a dead run is reclaimed
# when its owner pid is gone.
for ((n = first; n <= last; n++)); do
    if mkdir "$lock_root/$n" 2>/dev/null; then
        echo $$ >"$lock_root/$n/pid"
        workspace=$n
        break
    fi
    owner=$(cat "$lock_root/$n/pid" 2>/dev/null || true)
    if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
        rm -rf "$lock_root/$n"
        if mkdir "$lock_root/$n" 2>/dev/null; then
            echo $$ >"$lock_root/$n/pid"
            workspace=$n
            break
        fi
    fi
done
if [ -z "$workspace" ]; then
    echo "agent-run: no free workspace in $first..$last, running in place" >&2
    rm -rf "$work"
    exec "$@"
fi
# hyprctl exec starts the command from Hyprland's own environment and cwd, so
# hand both over through a small script and collect the result through files.
{
    export -p
    printf 'echo $$ >%q\n' "$work/pid"
    printf 'cd %q || exit 1\n' "$PWD"
    printf 'exec >%q 2>&1\n' "$work/out"
    printf '%q ' "$@"
    printf '\n'
    printf 'echo $? >%q\n' "$work/rc"
} >"$work/run.sh"
: >"$work/out"

# Hyprland 0.55 and later take Lua in hyprctl dispatch; older ones take the
# "[rules] command" form. Try Lua first.
launch="bash $work/run.sh"
reply=$(hyprctl dispatch "hl.dsp.exec_cmd(\"$launch\", {workspace = \"$workspace silent\", float = true})" 2>&1)
if [ "$reply" != ok ]; then
    reply=$(hyprctl dispatch exec "[workspace $workspace silent; float] $launch" 2>&1)
fi
if [ "$reply" != ok ]; then
    echo "agent-run: hyprctl exec failed: $reply" >&2
    exit 1
fi

tail -n +1 -f "$work/out" &
tailer=$!
while [ ! -s "$work/rc" ]; do sleep 0.2; done
sleep 0.3
kill "$tailer" 2>/dev/null
wait "$tailer" 2>/dev/null
exit "$(cat "$work/rc")"
