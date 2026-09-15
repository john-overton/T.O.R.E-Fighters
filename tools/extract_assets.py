"""Cross-platform EALIB extraction using this repository's native Rust readers."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=repo / 'gameassets/fighters-anthology',
                        help='An archive or directory; default: local Fighters Anthology media')
    parser.add_argument('--out', type=Path, default=repo / '.local/extracted', help='Output directory outside source media')
    parser.add_argument('--aircraft', action='append', choices=['f18', 'rafale'], help='Reviewed F/A-18D or Rafale C and its transitive aircraft, cockpit, sensor, store and audio dependencies')
    parser.add_argument('--validate-flight', action='store_true', help='After aircraft extraction, run the shared headless hybrid-flight acceptance suite')
    parser.add_argument('--native-flight', action='store_true', help='Static FA.EXE/FA.SMS research instead of archive extraction; no retail code execution')
    parser.add_argument('--native-weapons', action='store_true', help='Static FA weapon, sensor, loading and effect code research; no retail execution')
    parser.add_argument('--native-menus', action='store_true', help='Static FA creator and ordnance screen research; no retail execution')
    parser.add_argument('--music', action='store_true', help='Original PCM music and bounded FA situation scripts; no MIDI/synth')
    parser.add_argument('--wav-previews', action='store_true', help='With --music, also wrap recorded tracks as lossless local WAV previews')
    parser.add_argument('--creator', action='store_true', help='Creator aircraft metadata and original ordnance UI resource profile')
    parser.add_argument('--weapons', action='store_true', help='All projectile, sensor, ECM and tank definitions plus reviewed shared combat dependencies')
    parser.add_argument('--theater', help='Defined theater code (e.g. UKR, TVIET), or all; includes shared sky/weather dependencies')
    parser.add_argument('--exclude-archive', action='append', default=[], help='Skip source-relative archive path glob; repeatable (e.g. disc1/LHX/*)')
    parser.add_argument('--include', action='append', default=[], help='Case-insensitive resource glob (* and ?); repeatable')
    parser.add_argument('--list', action='store_true', help='List matches without writing extracted files')
    parser.add_argument('--dry-run', action='store_true', help='Validate archive directories and preview counts; no output writes')
    parser.add_argument('--overwrite', action='store_true', help='Replace differing previously extracted files')
    parser.add_argument('--max-entry-mib', type=int, default=256, help='Decoded resource size cap, 1..1024 MiB')
    args = parser.parse_args()
    if not 1 <= args.max_entry_mib <= 1024:
        parser.error('--max-entry-mib must be 1..1024')
    if args.validate_flight and (not args.aircraft or args.native_flight or args.native_weapons or args.native_menus or args.list or args.dry_run or args.include):
        parser.error('--validate-flight requires --aircraft and full extraction (no preview/include/native-flight)')
    if sum((args.native_flight, args.native_weapons, args.native_menus)) > 1:
        parser.error('select one native research domain')
    if args.native_flight or args.native_weapons or args.native_menus:
        if args.aircraft or args.weapons or args.creator or args.music or args.wav_previews or args.theater or args.include or args.exclude_archive:
            parser.error('native research is a separate executable-research pass; omit archive selection flags')
        from extract_native_flight import extract
        try:
            extract(args.source, args.out, overwrite=args.overwrite, preview=args.list or args.dry_run,
                    domain='menus' if args.native_menus else 'weapons' if args.native_weapons else 'flight')
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            parser.exit(1, f'{error}\n')
        return 0
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    rustup_cargo = cargo_home / 'bin' / ('cargo.exe' if os.name == 'nt' else 'cargo')
    cargo = str(rustup_cargo) if rustup_cargo.is_file() else shutil.which('cargo')
    if not cargo:
        parser.error('Cargo not found. Install Rust with rustup; see docs/DEVELOPMENT.md')
    source, output = args.source.resolve(), args.out.resolve()
    if source.is_dir() and output.is_relative_to(source):
        parser.error('Output must be outside the source media directory')
    command = [cargo, 'run', '--release', '--locked', '-p', 'tore-extract', '--',
               '--source', str(source), '--out', str(output), '--max-entry-mib', str(args.max_entry_mib)]
    for aircraft in args.aircraft or []:
        command.extend(['--aircraft', aircraft])
    if args.wav_previews and not args.music:
        parser.error('--wav-previews requires --music')
    if args.creator:
        command.append('--creator')
    if args.music:
        command.append('--music')
    if args.wav_previews:
        command.append('--wav-previews')
    if args.weapons:
        command.append('--weapons')
    if args.theater:
        command.extend(['--theater', args.theater])
    for pattern in args.exclude_archive:
        command.extend(['--exclude-archive', pattern])
    for pattern in args.include:
        command.extend(['--include', pattern])
    for flag in ('list', 'dry_run', 'overwrite'):
        if getattr(args, flag):
            command.append('--' + flag.replace('_', '-'))
    report_path = output / 'extraction-report.json'
    previous_time = report_path.stat().st_mtime_ns if report_path.exists() else None
    result = subprocess.run(command, cwd=repo, check=False)
    if not (args.list or args.dry_run) and report_path.exists() and report_path.stat().st_mtime_ns != previous_time:
        report = json.loads(report_path.read_text())
        archives = sorted({entry['archive'] for entry in report['entries']})
        report['archive_sha256'] = {path: sha256(Path(path)) for path in archives}
        for entry in report['entries']:
            if entry['status'] in ('written', 'replaced', 'unchanged'):
                entry['sha256'] = sha256(output / entry['output'])
                if entry.get('preview_output'):
                    entry['preview_sha256'] = sha256(output / entry['preview_output'])
        # Replace only the report created by this invocation, never media files.
        temporary = report_path.with_suffix(f'.{os.getpid()}.tmp')
        with temporary.open('x', encoding='utf-8') as stream:
            json.dump(report, stream, indent=2)
            stream.write('\n')
        temporary.replace(report_path)
        print(f'SHA-256 provenance added: {report_path}')
    if result.returncode == 0 and args.validate_flight:
        report = json.loads(report_path.read_text())
        identities = {{'f18': 'F18.PT', 'rafale': 'RAFALE.PT'}[aircraft] for aircraft in args.aircraft}
        profiles = []
        hashes = set()
        for entry in report['entries']:
            path = (output / entry['output']).resolve()
            if path.name.upper() not in identities or entry['status'] not in ('written', 'replaced', 'unchanged'):
                continue
            if not path.is_relative_to(output):
                parser.error('flight profile escapes extraction output')
            digest = sha256(path)
            if digest not in hashes:
                profiles.append(str(path))
                hashes.add(digest)
        if not profiles:
            parser.error('no extracted flight profile to validate')
        return subprocess.run([cargo, 'run', '--release', '--locked', '-p', 'tore-sim',
                               '--example', 'flight_suite', '--', *profiles], cwd=repo, check=False).returncode
    return result.returncode


if __name__ == '__main__':
    sys.exit(main())
