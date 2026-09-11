#!/usr/bin/env bash
# Native Linux candidates only. No installation, server startup or publication.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
exec python3 - "$@" <<'PY'
import argparse, hashlib, io, json, os, pathlib, platform, re, subprocess, tarfile, tempfile, tomllib
from datetime import datetime, timezone
root = pathlib.Path.cwd()
parser = argparse.ArgumentParser(prog='scripts/package-local.sh', description='Build and package a clean native Linux candidate; no installation.')
parser.add_argument('--out-dir', default='runtime/dist', help='output directory, relative to the checkout or absolute')
args = parser.parse_args()
def run(*args, **kwargs):
    return subprocess.check_output(args, cwd=root, text=True, **kwargs).strip()
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def clean():
    if run('git', 'status', '--porcelain', '--untracked-files=all'):
        raise SystemExit('Commit all source changes before packaging. Ignored runtime files are allowed.')
clean()
sha = run('git', 'rev-parse', 'HEAD')
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
rust = run('rustc', '-vV')
target = re.search(r'^host: (.+)$', rust, re.M).group(1)
if platform.system() != 'Linux' or '-linux-' not in target:
    raise SystemExit('This script supports only the native Linux Rust host target.')
if any(os.environ.get(k) for k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET')):
    raise SystemExit('Unset custom Rust flags/target before producing the default native candidate.')
dist = (root / args.out_dir).resolve()
dist.mkdir(parents=True, exist_ok=True)
stem = f'webtool-{version}-{target}'
names = [f'{stem}-client.tar.gz', f'{stem}-host.tar.gz', f'webtool-{version}-source.tar.gz', 'SHA256SUMS']
if any((dist / n).exists() or (dist / n).is_symlink() for n in names):
    raise SystemExit('Output filename already exists; preserve it and choose another --out-dir.')
lock = dist / '.packaging-lock'
lock.mkdir()  # A concurrent packager must refuse, not share output ownership.
try:
    with tempfile.TemporaryDirectory(prefix='alpha-stage-', dir=root / 'runtime') as temporary:
        stage = pathlib.Path(temporary)
        metadata = json.loads(run('cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target))
        locked = {(p['name'], p['version'], p.get('source')): p.get('checksum')
                  for p in tomllib.loads((root / 'Cargo.lock').read_text())['package']}
        # Check the published archive and extracted build inputs, not VCS dirtiness.
        for p in metadata['packages']:
            if p['source'] is None: continue
            base = pathlib.Path(p['manifest_path']).parent
            cache = base.parent.parent.parent / 'cache' / base.parent.name / (base.name + '.crate')
            if digest(cache) != locked[(p['name'], p['version'], p['source'])]:
                raise SystemExit('Locked source checksum mismatch: ' + base.name)
            with tarfile.open(cache) as source:
                for member in source.getmembers():
                    if member.isfile():
                        relative = pathlib.PurePosixPath(member.name).parts[1:]
                        if '..' in relative or not relative:
                            raise SystemExit('Unsafe dependency source path: ' + member.name)
                        if base.joinpath(*relative).read_bytes() != source.extractfile(member).read():
                            raise SystemExit('Extracted build input differs from published source: ' + member.name)
        cargo_home = pathlib.Path(os.environ.get('CARGO_HOME', pathlib.Path.home() / '.cargo')).resolve()
        env = dict(os.environ, RUSTFLAGS=f'--remap-path-prefix={root}=/webtool --remap-path-prefix={cargo_home}=/cargo')
        command = ['cargo', 'build', '--locked', '--offline', '--release', '--target', target,
                   '--target-dir', str(root / 'target'), '-p', 'webtool-cli', '-p', 'webtool-server',
                   '--message-format=json-render-diagnostics']
        with (stage / 'cargo.jsonl').open('w') as log:
            subprocess.run(command, cwd=root, env=env, stdout=log, check=True)
        clean()
        if run('git', 'rev-parse', 'HEAD') != sha:
            raise SystemExit('Source commit changed during build; refusing to package.')
        artifacts = [json.loads(line) for line in (stage / 'cargo.jsonl').read_text().splitlines() if line.startswith('{')]
        used = {a['package_id'] for a in artifacts if a.get('reason') == 'compiler-artifact'}
        packages = [p for p in metadata['packages'] if p['id'] in used and p['source'] is not None]
        common = {name: root / name for name in ('LICENSE', 'COPYING')}
        common['README.md'] = root / 'docs/ALPHA.md'
        common['THIRD-PARTY.md'] = root / 'docs/THIRD-PARTY.md'
        supplements = json.loads((root / 'packaging/licenses/index.json').read_text())['packages']
        concerns = json.loads((root / 'packaging/licenses/concerns.json').read_text())
        inventory, blockers = [], []
        for concern in concerns['records']:
            if not concern['resolved']:
                blockers.append(f"{concern['package']} {concern['version']}: {concern['missing_obligation']} Remedy: {concern['remedy']}")
        for p in sorted(packages, key=lambda p: (p['name'], p['version'])):
            base = pathlib.Path(p['manifest_path']).parent
            key = p['name'] + '-' + p['version']
            cache = base.parent.parent.parent / 'cache' / base.parent.name / (key + '.crate')
            checksum = locked[(p['name'], p['version'], p['source'])]
            if not cache.is_file() or digest(cache) != checksum:
                raise SystemExit('Missing or mismatched locked source archive: ' + key)
            notices = []
            for f in sorted(base.rglob('*')):
                if f.is_file() and not f.is_symlink() and (f.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')) or f.name.upper() == 'COPYRIGHT' or any(part.upper() in ('LICENSES', 'LICENCES') for part in f.relative_to(base).parts[:-1])):
                    name = 'licenses/' + key + '/' + f.relative_to(base).as_posix()
                    common[name] = f
                    notices.append(name)
            supplement = supplements.get(key)
            if supplement:
                for relative in supplement['files']:
                    name = 'licenses/supplements/' + relative
                    common[name] = root / 'packaging/licenses' / relative
                    notices.append(name)
            if not notices:
                blockers.append(key + ': no applicable license text or attribution material collected for this build dependency; recover the required material before distribution.')
            inventory.append({'name': p['name'], 'version': p['version'], 'license_expression': p['license'],
                              'source_url': f"https://crates.io/api/v1/crates/{p['name']}/{p['version']}/download", 'source_archive_sha256': checksum,
                              'upstream_repository': p.get('repository'), 'notice_files': notices,
                              'supplement': supplement,
                              'features': next(n['features'] for n in metadata['resolve']['nodes'] if n['id'] == p['id']),
                              'observed_target_kinds': sorted({kind for a in artifacts if a.get('reason') == 'compiler-artifact' and a['package_id'] == p['id'] for kind in a['target']['kind']})})
        rust_docs = pathlib.Path(run('rustc', '--print', 'sysroot')) / 'share/doc/rust'
        rust_copyright = rust_docs / 'COPYRIGHT-library.html'
        if rust_copyright.is_file():
            common['licenses/rust/COPYRIGHT-library.html'] = rust_copyright
            for f in sorted((rust_docs / 'licenses').glob('*')):
                if f.is_file(): common['licenses/rust/licenses/' + f.name] = f
        else:
            blockers.append('Rust standard-library copyright/license material missing from this toolchain installation.')
        inventory_file = stage / 'THIRD-PARTY.json'
        inventory_file.write_text(json.dumps({'scope': 'Packages observed in the native build, including build-only/proc-macro dependencies; not a per-binary link map. License expressions are manifest declarations, not overrides of file-specific terms; consult supplements and LICENSE-CONCERNS.json.', 'packages': inventory}, indent=2) + '\n')
        common[inventory_file.name] = inventory_file
        common['LICENSE-CONCERNS.json'] = root / 'packaging/licenses/concerns.json'
        locked_sources = stage / 'LOCKED-SOURCES.json'
        locked_sources.write_text(json.dumps([{'name': p['name'], 'version': p['version'], 'sha256': p['checksum'],
            'source_url': f"https://crates.io/api/v1/crates/{p['name']}/{p['version']}/download"}
            for p in tomllib.loads((root / 'Cargo.lock').read_text())['package'] if p.get('source', '').startswith('registry+')], indent=2) + '\n')
        common[locked_sources.name] = locked_sources
        access = (root / 'docs/SOURCE-ACCESS.md').read_text()
        mpl = '\n'.join(f"- {p['name']} {p['version']}: {p['source_url']}\n  SHA256: {p['source_archive_sha256']}" for p in inventory if 'MPL' in (p['license_expression'] or ''))
        for key, value in {'@SOURCE_SHA@': sha, '@SOURCE_ARCHIVE@': names[2], '@TARGET@': target, '@MPL_SOURCES@': mpl}.items():
            access = access.replace(key, value)
        access_file = stage / 'SOURCE-ACCESS.md'
        access_file.write_text(access)
        common[access_file.name] = access_file
        blocker_file = stage / 'PUBLICATION-BLOCKERS.txt'
        blocker_file.write_text('LOCAL CANDIDATES: no publication authorized.\nCheck THIRD-PARTY.md and resolve the following material gaps before distribution:\n' + '\n'.join(blockers) + '\n')
        common[blocker_file.name] = blocker_file
        binaries = {name: root / 'target' / target / 'release' / name for name in ('webtool', 'webtoold')}
        linkage = {}
        for name, binary in binaries.items():
            dynamic = run('readelf', '-d', str(binary))
            versions = run('readelf', '--version-info', str(binary))
            segments = run('readelf', '-l', str(binary))
            linkage[name] = {'file': run('file', '-b', str(binary)), 'sha256': digest(binary),
                             'needed_libraries': re.findall(r'\(NEEDED\).*?\[(.*?)\]', dynamic),
                             'symbol_versions': sorted(set(re.findall(r'Name: ((?:GLIBC|GCC|OPENSSL)_[\w.]+)', versions))),
                             'interpreter': re.findall(r'Requesting program interpreter: (.*?)\]', segments)}
        build_info = {'version': version, 'source_sha': sha, 'source_repository': 'https://github.com/JCFrags/webtool',
                      'built_at_utc': datetime.now(timezone.utc).isoformat(), 'rustc': rust, 'cargo': run('cargo', '-V'),
                      'target': target, 'platform': platform.freedesktop_os_release().get('PRETTY_NAME'),
                      'kernel': platform.release(), 'glibc_on_builder': run('getconf', 'GNU_LIBC_VERSION'),
                      'features': 'normal defaults: CLI; server documents; engine default HTML/search',
                      'flags': 'locked offline release; native target; source paths remapped to /webtool and /cargo',
                      'workspace_features': {a['target']['name']: a.get('features', []) for a in artifacts if a.get('reason') == 'compiler-artifact' and a['target']['name'] in ('webtool', 'webtoold', 'webtool_engine')},
                      'linkage': linkage, 'publication_material_gaps': len(blockers),
                      'portability': 'Only this build platform is checked. Dynamic linking; no static, broad Linux portability or reproducible-binary claim.'}
        info_file = stage / 'BUILD-INFO.json'
        info_file.write_text(json.dumps(build_info, indent=2) + '\n')
        common[info_file.name] = info_file
        # Source paths are an explicit Git allowlist, not a working-directory tar.
        allow = ['Cargo.toml', 'Cargo.lock', 'LICENSE', 'COPYING', 'config.example.toml', 'crates',
                 'scripts/install-local.sh', 'scripts/package-local.sh', 'docs/ALPHA.md', 'docs/THIRD-PARTY.md', 'docs/SOURCE-ACCESS.md', 'packaging/licenses']
        source_files = {}
        for record in run('git', 'ls-tree', '-r', sha, '--', *allow).splitlines():
            metadata_line, name = record.split('\t', 1)
            mode, kind, object_id = metadata_line.split()
            if kind != 'blob' or mode not in ('100644', '100755'):
                raise SystemExit('Unexpected source object: ' + name)
            source_files[name] = (subprocess.check_output(['git', 'cat-file', 'blob', object_id], cwd=root), int(mode[-3:], 8))
        def archive(filename, prefix, files, git_files=None):
            with (dist / filename).open('xb') as output, tarfile.open(fileobj=output, mode='w:gz') as tar:
                for name, path in sorted(files.items()):
                    if pathlib.PurePosixPath(name).is_absolute() or '..' in pathlib.PurePosixPath(name).parts:
                        raise SystemExit('Unsafe package path: ' + name)
                    data = path.read_bytes()
                    member = tarfile.TarInfo(prefix + '/' + name)
                    member.mode = 0o755 if name in binaries else 0o644
                    member.size = len(data)
                    tar.addfile(member, io.BytesIO(data))
                for name, (data, mode) in sorted((git_files or {}).items()):
                    member = tarfile.TarInfo(prefix + '/' + name)
                    member.mode, member.size = mode, len(data)
                    tar.addfile(member, io.BytesIO(data))
        archive(names[0], stem + '-client', dict(common, webtool=binaries['webtool']))
        archive(names[1], stem + '-host', dict(common, webtoold=binaries['webtoold'], **{'config.example.toml': root / 'config.example.toml'}))
        source_extra = {n: p for n, p in common.items() if n not in ('LICENSE', 'COPYING')}
        archive(names[2], f'webtool-{version}-source', source_extra, source_files)
        with (dist / 'SHA256SUMS').open('x') as sums:
            for name in names[:3]:
                path = dist / name
                checksum = digest(path)
                sums.write(f'{checksum}  {name}\n')
                print(f'{path}: {path.stat().st_size} bytes, SHA256 {checksum}')
        print(f'Build commit: {sha}; publication material gaps: {len(blockers)}. No installation or server startup performed.')
finally:
    lock.rmdir()
PY
