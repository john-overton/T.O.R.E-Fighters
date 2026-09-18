"""Pinned static OpenFA toolchain setup and non-destructive SH/LIB workflows."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from fa_lib import pack_stored

ROOT = Path(__file__).resolve().parents[1]
REVISION = '7507fef5bbb126302a59cb413e80cadf5c547f9d'
NITROGEN = '0691b37c66f0c8668a2c197a9c49b8c75f753c21'
DEFAULT_SOURCE = ROOT / '.local/tools/openfa'
DEFAULT_TOOL = DEFAULT_SOURCE / 'target/debug' / ('ofa-tools.exe' if __import__('os').name == 'nt' else 'ofa-tools')
PATCH = ROOT / 'tools/openfa/static-export.patch'


def run(args, cwd=None):
    result = subprocess.run([str(a) for a in args], cwd=cwd, capture_output=True, text=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout


def require_static(tool):
    if run([tool, '--tore-static-export-version']).strip() != 'tore-static-export-v1':
        raise ValueError('requires the patched static OpenFA build; run openfa_tools.py setup')


def patch_contents(text):
    # Git accepts an empty context line with or without its optional space marker.
    return '\n'.join('' if line == ' ' else line for line in text.splitlines()).strip()


def setup(source):
    if not source.exists():
        source.parent.mkdir(parents=True, exist_ok=True)
        run(['git', 'clone', '--no-checkout', 'https://gitlab.com/openfa/openfa.git', source])
        run(['git', 'checkout', REVISION], source)
    if run(['git', 'rev-parse', 'HEAD'], source).strip() != REVISION:
        raise ValueError('existing checkout is not the pinned OpenFA revision')
    run(['git', 'submodule', 'update', '--init', 'nitrogen'], source)
    if run(['git', 'rev-parse', 'HEAD'], source/'nitrogen').strip() != NITROGEN:
        raise ValueError('unexpected Nitrogen revision')
    link = source/'crates/nitrogen'
    if not link.exists():
        try:
            link.symlink_to('../nitrogen/crates', target_is_directory=True)
        except OSError:
            shutil.copytree(source/'nitrogen/crates', link)
    reverse = subprocess.run(['git', 'apply', '--reverse', '--check', str(PATCH)], cwd=source,
                             capture_output=True, check=False)
    if reverse.returncode:
        run(['git', 'apply', '--check', PATCH], source)
        run(['git', 'apply', PATCH], source)
    if patch_contents(run(['git', 'diff', '--no-color', '--no-ext-diff', '--src-prefix=a/', '--dst-prefix=b/'], source)) != patch_contents(PATCH.read_text()):
        raise ValueError('checkout contains changes beyond the static-export patch')
    if not (source/'Cargo.lock').exists():
        run(['cargo', '+1.91.1', 'generate-lockfile'], source)
    run(['cargo', '+1.91.1', 'build', '--locked', '-p', 'ofa-tools', '--features', 'sh/static-export', '--target-dir', source/'target'], source)
    tool = source/'target/debug'/DEFAULT_TOOL.name
    require_static(tool)
    stamp = {'openfa': REVISION, 'nitrogen': NITROGEN, 'static_export': 1,
             'binary_sha256': hashlib.sha256(tool.read_bytes()).hexdigest(),
             'patch_sha256': hashlib.sha256(PATCH.read_bytes()).hexdigest(),
             'lock_sha256': hashlib.sha256((source/'Cargo.lock').read_bytes()).hexdigest()}
    (source/'tore-export-toolchain.json').write_text(json.dumps(stamp, indent=2)+'\n')
    print(tool)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tool', type=Path, default=DEFAULT_TOOL)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('setup')
    p.add_argument('--source', type=Path, default=DEFAULT_SOURCE)
    for name in ['decode', 'encode', 'pack', 'unpack']:
        p = sub.add_parser(name)
        p.add_argument('--out', type=Path, required=True)
        p.add_argument('inputs', type=Path, nargs='+')
    args = parser.parse_args()
    if args.command == 'setup':
        setup(args.source.resolve())
        return
    tool = args.tool.resolve()
    if args.command != 'pack':
        require_static(tool)
    sources = [p.resolve() for p in args.inputs]
    if any(not p.is_file() for p in sources):
        parser.error('inputs must be files')
    if len({p.name.upper() for p in sources}) != len(sources):
        parser.error('duplicate input basenames')
    out = args.out.resolve()
    if out.exists():
        parser.error('output must not already exist')
    if args.command == 'pack':
        pack_stored(sources, out)
    else:
        out.mkdir(parents=True)
        if args.command == 'unpack':
            run([tool, 'lib', 'unpack', '-o', out, *sources])
        else:
            suffix = '.sh' if args.command == 'decode' else '.sh.yaml'
            for source in sources:
                if not source.name.lower().endswith(suffix):
                    parser.error(f'expected {suffix} input')
                scratch = out/source.name
                shutil.copyfile(source, scratch)
                run([tool, scratch], out)
    print(out)


if __name__ == '__main__':
    main()
