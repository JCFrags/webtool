#!/usr/bin/env python3
"""Build or assemble private native Linux candidates. Never install or publish."""
import argparse
import hashlib
import io
import json
import os
import pathlib
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE_PATHS = [
    'Cargo.toml', 'Cargo.lock', 'LICENSE', 'COPYING', 'README.md',
    'config.example.toml', 'crates', 'migrations', 'Dockerfile', 'compose.yaml',
    '.dockerignore', 'scripts/install-local.sh', 'scripts/package-local.sh',
    'scripts/package.py', 'scripts/smoke_cli.py', 'docs/ALPHA.md',
    'docs/THIRD-PARTY.md', 'docs/SOURCE-ACCESS.md', 'docs/LINUX.md',
    'docs/DOCKER.md', 'docs/RELEASING.md', 'docs/CRAWL.md', 'docs/SPRINTS.md', 'packaging/licenses',
]


def run(*args, **kwargs):
    return subprocess.check_output(args, cwd=ROOT, text=True, **kwargs).strip()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def source_files(git_sha=None):
    if git_sha:
        result = {}
        for record in run('git', 'ls-tree', '-r', git_sha, '--', *SOURCE_PATHS).splitlines():
            info, name = record.split('\t', 1)
            mode, kind, object_id = info.split()
            if kind != 'blob' or mode not in ('100644', '100755'):
                raise SystemExit('Unexpected source object: ' + name)
            result[name] = (subprocess.check_output(['git', 'cat-file', 'blob', object_id], cwd=ROOT), int(mode[-3:], 8))
        return result
    result = {}
    for name in SOURCE_PATHS:
        path = ROOT / name
        paths = sorted(path.rglob('*')) if path.is_dir() else [path]
        for item in paths:
            if item.is_symlink():
                raise SystemExit('Symlink in source input: ' + str(item))
            if item.is_file():
                result[item.relative_to(ROOT).as_posix()] = (item.read_bytes(), 0o755 if os.access(item, os.X_OK) else 0o644)
    return result


def source_hashes(files):
    return {name: hashlib.sha256(data).hexdigest() for name, (data, _) in files.items()}


def collect_materials(stage, metadata, artifacts, target, sha, source_archive):
    locked = {(p['name'], p['version'], p.get('source')): p.get('checksum')
              for p in tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']}
    # The registry archives and extracted inputs, not upstream Git dirtiness, own source identity.
    for package in metadata['packages']:
        if package['source'] is None:
            continue
        base = pathlib.Path(package['manifest_path']).parent
        cache = base.parent.parent.parent / 'cache' / base.parent.name / (base.name + '.crate')
        if not cache.is_file() or digest(cache) != locked[(package['name'], package['version'], package['source'])]:
            raise SystemExit('Missing or mismatched locked source archive: ' + base.name)
        with tarfile.open(cache) as archive:
            for member in archive.getmembers():
                if member.isfile():
                    relative = pathlib.PurePosixPath(member.name).parts[1:]
                    if '..' in relative or not relative:
                        raise SystemExit('Unsafe dependency source path: ' + member.name)
                    if base.joinpath(*relative).read_bytes() != archive.extractfile(member).read():
                        raise SystemExit('Extracted build input differs from published source: ' + member.name)
    used = {a['package_id'] for a in artifacts if a.get('reason') == 'compiler-artifact'}
    packages = [p for p in metadata['packages'] if p['id'] in used and p['source'] is not None]
    common = {name: ROOT / name for name in ('LICENSE', 'COPYING')}
    common['README.md'] = ROOT / 'docs/ALPHA.md'
    common['THIRD-PARTY.md'] = ROOT / 'docs/THIRD-PARTY.md'
    common['LINUX.md'] = ROOT / 'docs/LINUX.md'
    common['DOCKER.md'] = ROOT / 'docs/DOCKER.md'
    common['RELEASING.md'] = ROOT / 'docs/RELEASING.md'
    common['CRAWL.md'] = ROOT / 'docs/CRAWL.md'
    if (ROOT / 'docs/SPRINTS.md').is_file():
        common['SPRINTS.md'] = ROOT / 'docs/SPRINTS.md'
    supplements = json.loads((ROOT / 'packaging/licenses/index.json').read_text())['packages']
    concerns = json.loads((ROOT / 'packaging/licenses/concerns.json').read_text())
    inventory, blockers = [], []
    for concern in concerns['records']:
        if not concern['resolved']:
            blockers.append(f"{concern['package']} {concern['version']}: {concern['missing_obligation']} Remedy: {concern['remedy']}")
    for package in sorted(packages, key=lambda p: (p['name'], p['version'])):
        base = pathlib.Path(package['manifest_path']).parent
        key = package['name'] + '-' + package['version']
        notices = []
        for file in sorted(base.rglob('*')):
            if file.is_file() and not file.is_symlink() and (
                file.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')) or
                file.name.upper() == 'COPYRIGHT' or
                any(part.upper() in ('LICENSES', 'LICENCES') for part in file.relative_to(base).parts[:-1])
            ):
                name = 'licenses/' + key + '/' + file.relative_to(base).as_posix()
                common[name] = file
                notices.append(name)
        supplement = supplements.get(key)
        if supplement:
            for relative in supplement['files']:
                name = 'licenses/supplements/' + relative
                common[name] = ROOT / 'packaging/licenses' / relative
                notices.append(name)
        if not notices:
            blockers.append(key + ': no applicable license or attribution material collected; recover it before distribution.')
        inventory.append({
            'name': package['name'], 'version': package['version'], 'license_expression': package['license'],
            'source_url': f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download",
            'source_archive_sha256': locked[(package['name'], package['version'], package['source'])],
            'upstream_repository': package.get('repository'), 'notice_files': notices, 'supplement': supplement,
            'features': next(n['features'] for n in metadata['resolve']['nodes'] if n['id'] == package['id']),
            'observed_target_kinds': sorted({kind for a in artifacts if a.get('reason') == 'compiler-artifact' and a['package_id'] == package['id'] for kind in a['target']['kind']}),
        })
    rust_docs = pathlib.Path(run('rustc', '--print', 'sysroot')) / 'share/doc/rust'
    if (rust_docs / 'COPYRIGHT-library.html').is_file():
        common['licenses/rust/COPYRIGHT-library.html'] = rust_docs / 'COPYRIGHT-library.html'
        for file in sorted((rust_docs / 'licenses').glob('*')):
            if file.is_file():
                common['licenses/rust/licenses/' + file.name] = file
    else:
        blockers.append('Rust standard-library copyright/license material missing from this toolchain installation.')
    generated = {
        'THIRD-PARTY.json': {'scope': 'Packages observed in this build, including build-only dependencies; not a per-binary link map. Manifest license expressions do not override file-specific terms. Consult supplements and LICENSE-CONCERNS.json.', 'packages': inventory},
        'LOCKED-SOURCES.json': [{'name': p['name'], 'version': p['version'], 'sha256': p['checksum'], 'source_url': f"https://crates.io/api/v1/crates/{p['name']}/{p['version']}/download"} for p in tomllib.loads((ROOT / 'Cargo.lock').read_text())['package'] if p.get('source', '').startswith('registry+')],
    }
    for name, value in generated.items():
        path = stage / name
        path.write_text(json.dumps(value, indent=2) + '\n')
        common[name] = path
    common['LICENSE-CONCERNS.json'] = ROOT / 'packaging/licenses/concerns.json'
    access = (ROOT / 'docs/SOURCE-ACCESS.md').read_text()
    mpl = '\n'.join(f"- {p['name']} {p['version']}: {p['source_url']}\n  SHA256: {p['source_archive_sha256']}" for p in inventory if 'MPL' in (p['license_expression'] or ''))
    for key, value in {'@SOURCE_SHA@': sha, '@SOURCE_ARCHIVE@': source_archive, '@TARGET@': target, '@MPL_SOURCES@': mpl}.items():
        access = access.replace(key, value)
    (stage / 'SOURCE-ACCESS.md').write_text(access)
    common['SOURCE-ACCESS.md'] = stage / 'SOURCE-ACCESS.md'
    (stage / 'PUBLICATION-BLOCKERS.txt').write_text('LOCAL CANDIDATES: no publication authorized.\nMaterial gaps to resolve before distribution:\n' + '\n'.join(blockers) + '\n')
    common['PUBLICATION-BLOCKERS.txt'] = stage / 'PUBLICATION-BLOCKERS.txt'
    return common, blockers


def linkage(binary):
    return {
        'file': run('file', '-b', str(binary)), 'sha256': digest(binary),
        'needed_libraries': re.findall(r'\(NEEDED\).*?\[(.*?)\]', run('readelf', '-d', str(binary))),
        'symbol_versions': sorted(set(re.findall(r'Name: ((?:GLIBC|GCC|OPENSSL)_[\w.]+)', run('readelf', '--version-info', str(binary))))),
        'interpreter': re.findall(r'Requesting program interpreter: (.*?)\]', run('readelf', '-l', str(binary))),
    }


def archive(path, prefix, files):
    with path.open('xb') as output, tarfile.open(fileobj=output, mode='w:gz') as tar:
        for name, (data, mode) in sorted(files.items()):
            if pathlib.PurePosixPath(name).is_absolute() or '..' in pathlib.PurePosixPath(name).parts:
                raise SystemExit('Unsafe package path: ' + name)
            member = tarfile.TarInfo(prefix + '/' + name)
            member.mode, member.size = mode, len(data)
            tar.addfile(member, io.BytesIO(data))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', default='runtime/dist')
    parser.add_argument('--reuse-build', type=pathlib.Path, help='assemble from a complete, checksum-verified private BUILD-EVIDENCE.json directory')
    parser.add_argument('--record-build', type=pathlib.Path, help='record a finished native release build with cargo.jsonl and metadata.json here')
    parser.add_argument('--source-revision', help='exact source revision for --record-build; unknown is allowed only for local images')
    args = parser.parse_args()
    if args.reuse_build and args.record_build:
        parser.error('Choose --reuse-build or --record-build, not both.')
    if args.source_revision and not args.record_build:
        parser.error('--source-revision is only for --record-build.')
    os.chdir(ROOT)
    if platform.system() != 'Linux':
        raise SystemExit('Only native Linux candidates are supported.')
    if args.record_build:
        sha = args.source_revision or 'unknown'
        if sha != 'unknown' and not re.fullmatch('[0-9a-f]{40}', sha):
            raise SystemExit('Source revision must be a full lower-case Git SHA or unknown.')
        source = source_files()
        try:
            git_provenance = run('git', 'rev-parse', 'HEAD') == sha and not run('git', 'status', '--porcelain', '--untracked-files=all')
        except (FileNotFoundError, subprocess.CalledProcessError):
            git_provenance = False
        if git_provenance:
            source = source_files(sha)
    else:
        if run('git', 'status', '--porcelain', '--untracked-files=all'):
            raise SystemExit('Commit all source changes before packaging. Ignored runtime files are allowed.')
        sha = run('git', 'rev-parse', 'HEAD')
        source = source_files(sha)
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    dist = (ROOT / args.out_dir).resolve()
    if not args.record_build:
        dist.mkdir(parents=True, exist_ok=True)
        for name in ('SHA256SUMS', 'BUILD-EVIDENCE.json'):
            if (dist / name).exists() or (dist / name).is_symlink():
                raise SystemExit('Output already exists; preserve it and choose another --out-dir.')
    with tempfile.TemporaryDirectory(prefix='webtool-package-') as temporary:
        stage = pathlib.Path(temporary)
        if args.reuse_build:
            evidence = args.reuse_build.resolve()
            receipt = json.loads((evidence / 'BUILD-EVIDENCE.json').read_text())
            if receipt['source_sha'] != sha or receipt['source_file_sha256'] != source_hashes(source):
                raise SystemExit('Build evidence does not match this exact clean source revision and files.')
            for name, checksum in receipt['files_sha256'].items():
                if pathlib.PurePosixPath(name).is_absolute() or '..' in pathlib.PurePosixPath(name).parts:
                    raise SystemExit('Unsafe build-evidence path: ' + name)
                path = evidence / name
                if not path.is_file() or path.is_symlink() or digest(path) != checksum:
                    raise SystemExit('Missing or changed build-evidence file: ' + name)
            info = json.loads((evidence / 'materials/BUILD-INFO.json').read_text())
            if info['source_sha'] != sha or info['version'] != version:
                raise SystemExit('Build information differs from the source or version.')
            target = info['target']
            binaries = {name: evidence / name for name in ('webtool', 'webtoold')}
            for name, binary in binaries.items():
                if digest(binary) != info['linkage'][name]['sha256']:
                    raise SystemExit('Binary differs from the recorded build: ' + name)
            common = {p.relative_to(evidence / 'materials').as_posix(): p for p in (evidence / 'materials').rglob('*') if p.is_file()}
            blockers = info['publication_material_gaps']
        else:
            rust = run('rustc', '-vV')
            target = re.search(r'^host: (.+)$', rust, re.M).group(1)
            if '-linux-' not in target:
                raise SystemExit('Only the native Linux Rust host target is supported.')
            if args.record_build:
                evidence = args.record_build.resolve()
                metadata = json.loads((evidence / 'metadata.json').read_text())
                artifacts = [json.loads(line) for line in (evidence / 'cargo.jsonl').read_text().splitlines() if line.startswith('{')]
            else:
                if any(os.environ.get(k) for k in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET')):
                    raise SystemExit('Unset custom Rust flags/target before the default candidate build.')
                evidence = dist
                metadata = json.loads(run('cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target))
                cargo_home = pathlib.Path(os.environ.get('CARGO_HOME', pathlib.Path.home() / '.cargo')).resolve()
                env = dict(os.environ, RUSTFLAGS=f'--remap-path-prefix={ROOT}=/webtool --remap-path-prefix={cargo_home}=/cargo')
                command = ['cargo', 'build', '--locked', '--offline', '--release', '--target-dir', str(ROOT / 'target'), '-p', 'webtool-cli', '-p', 'webtool-server', '--message-format=json-render-diagnostics']
                with (evidence / 'cargo.jsonl').open('x') as log:
                    subprocess.run(command, cwd=ROOT, env=env, stdout=log, check=True)
                artifacts = [json.loads(line) for line in (evidence / 'cargo.jsonl').read_text().splitlines() if line.startswith('{')]
                (evidence / 'metadata.json').write_text(json.dumps(metadata) + '\n')
                if run('git', 'status', '--porcelain', '--untracked-files=all') or run('git', 'rev-parse', 'HEAD') != sha:
                    raise SystemExit('Source changed during the build; refusing to package.')
            if not any(a.get('reason') == 'build-finished' and a.get('success') for a in artifacts):
                raise SystemExit('No successful build-finished record in build evidence.')
            binaries = {name: ROOT / 'target/release' / name for name in ('webtool', 'webtoold')}
            for name, binary in binaries.items():
                observed = [a for a in artifacts if a.get('reason') == 'compiler-artifact' and a['target']['name'] == name and 'bin' in a['target']['kind'] and a.get('executable')]
                if len(observed) != 1 or pathlib.Path(observed[0]['executable']).resolve() != binary.resolve():
                    raise SystemExit('Build evidence does not identify the expected native release executable: ' + name)
            common, missing = collect_materials(stage, metadata, artifacts, target, sha, f'webtool-{version}-source.tar.gz')
            blockers = len(missing)
            info = {
                'version': version, 'source_sha': sha, 'source_repository': 'https://github.com/JCFrags/webtool',
                'built_at_utc': datetime.now(timezone.utc).isoformat(), 'rustc': rust, 'cargo': run('cargo', '-V'),
                'target': target, 'platform': platform.freedesktop_os_release().get('PRETTY_NAME'),
                'kernel': platform.release(), 'glibc_on_builder': run('getconf', 'GNU_LIBC_VERSION'),
                'features': 'normal defaults: CLI; server documents; engine default HTML/search',
                'flags': 'locked release; native target; normal default features. See the retained compiler invocation for flags; binary prefix checks are recorded below.',
                'source_path_screen': {name: {'project_prefix_found': str(ROOT).encode() in binary.read_bytes(), 'cargo_prefix_found': str(pathlib.Path(os.environ.get('CARGO_HOME', pathlib.Path.home() / '.cargo')).resolve()).encode() in binary.read_bytes()} for name, binary in binaries.items()},
                'workspace_features': {a['target']['name']: a.get('features', []) for a in artifacts if a.get('reason') == 'compiler-artifact' and a['target']['name'] in ('webtool', 'webtoold', 'webtool_engine')},
                'linkage': {name: linkage(binary) for name, binary in binaries.items()},
                'publication_material_gaps': blockers,
                'portability': 'Only this build platform is checked. Dynamic linking; no other architecture, broad Linux portability or reproducible-binary claim.',
                'provenance': 'Clean Git source build.' if not args.record_build or git_provenance else 'Transferred context files verified by the source-file receipt; no Git compiler provenance is manufactured.',
            }
            (stage / 'BUILD-INFO.json').write_text(json.dumps(info, indent=2) + '\n')
            common['BUILD-INFO.json'] = stage / 'BUILD-INFO.json'
            materials = evidence / 'materials'
            materials.mkdir()
            for name, path in common.items():
                destination = materials / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, destination)
            common = {p.relative_to(materials).as_posix(): p for p in materials.rglob('*') if p.is_file()}
            for name, binary in binaries.items():
                shutil.copy2(binary, evidence / name)
            binaries = {name: evidence / name for name in binaries}
            receipt = {
                'source_sha': sha, 'source_file_sha256': source_hashes(source),
                'files_sha256': {p.relative_to(evidence).as_posix(): digest(p) for p in evidence.rglob('*') if p.is_file()},
            }
            (evidence / 'BUILD-EVIDENCE.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
        if args.record_build:
            source_extra = {name: (path.read_bytes(), 0o644) for name, path in common.items() if name not in ('LICENSE', 'COPYING', 'README.md')}
            archive(evidence / f'webtool-{version}-source.tar.gz', f'webtool-{version}-source', dict(source, **source_extra))
            print(f'Recorded {sha}; material gaps: {blockers}. No publication or installation.')
            return
        stem = f'webtool-{version}-{target}'
        names = [f'{stem}-client.tar.gz', f'{stem}-host.tar.gz', f'webtool-{version}-source.tar.gz']
        common_bytes = {name: (path.read_bytes(), 0o644) for name, path in common.items()}
        for role, name in [('client', 'webtool'), ('host', 'webtoold')]:
            files = dict(common_bytes)
            files[name] = (binaries[name].read_bytes(), 0o755)
            files['BINARY-SHA256SUMS'] = (f'{digest(binaries[name])}  {name}\n'.encode(), 0o644)
            files['install.sh'] = ((ROOT / 'scripts/install-local.sh').read_bytes(), 0o755)
            if role == 'host':
                files['config.example.toml'] = ((ROOT / 'config.example.toml').read_bytes(), 0o644)
            archive(dist / f'{stem}-{role}.tar.gz', f'{stem}-{role}', files)
        extra = {name: pair for name, pair in common_bytes.items() if name not in ('LICENSE', 'COPYING', 'README.md')}
        archive(dist / names[2], f'webtool-{version}-source', dict(source, **extra))
        with (dist / 'SHA256SUMS').open('x') as sums:
            for name in names:
                path = dist / name
                checksum = digest(path)
                sums.write(f'{checksum}  {name}\n')
                print(f'{name}: {path.stat().st_size} bytes, SHA256 {checksum}')
        print(f'Exact source: {sha}; publication material gaps: {blockers}. No publication, installation or server startup.')


if __name__ == '__main__':
    main()
