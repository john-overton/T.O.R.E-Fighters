# OpenFA export support

Implementation tooling for the F/A-XX handoff. Start with
[the developer guide](../../docs/fa-xx-developer-kit.md) and
[export contract](../../docs/spec/fa-xx-export.md).

`upstream/sh.rs` and `upstream/lib_ext.rs` are unchanged copies of the relevant
OpenFA conversion and archive command source. They are reference snapshots,
not standalone scripts. Their original GPL notices and full license are retained;
`upstream/provenance.json` identifies paths, revision and SHA-256 hashes.
They are not compiled into T.O.R.E.

`static-export.patch` applies to OpenFA revision
`7507fef5bbb126302a59cb413e80cadf5c547f9d`. It adds a feature that bypasses
`ShAnalysis::analyze`, because that upstream pass runs an x86 interpreter on
imported code. Our export build only disassembles, reads and writes shape data.
It also makes the CLI report failures without modal dialogs, and adds a version
probe that verifies the static feature is enabled. Gameplay and the upstream
interactive application are not supported by this patched export build.

`../openfa_tools.py setup` fetches the pinned OpenFA and Nitrogen revisions,
applies the patch, builds with the static feature, and records binary, patch
and Cargo.lock hashes in the ignored source checkout. It requires Git, Rust
1.91.1 and a host C/C++ build toolchain. The first dependency resolution needs
network access. Later builds use the generated lockfile with `--locked`.

Do not use an ordinary upstream `ofa-tools` build for this workflow. Wrappers
check the static-build marker before passing it an SH file. Upstream does not
endorse this patch or the F/A-XX export.

LIB writing uses `../fa_lib.py`, not upstream LibWriter. The captured upstream
writer omitted the terminal directory sentinel, which its own reader tolerated.
Our stored-entry writer emits it; `check_lib.rs` independently checks the full
archive and payloads before the exporter makes a distributable ZIP.
