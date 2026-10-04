#!/usr/bin/env python3
"""Exercise actual CLI/server onboarding in isolated storage with a finite lifetime."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def identity(pid: int) -> dict:
    base = Path('/proc') / str(pid)
    stat = (base / 'stat').read_text().rsplit(')', 1)[1].split()
    executable = (base / 'exe').resolve(strict=True)
    return {'pid': pid, 'start_ticks': stat[19], 'executable': str(executable), 'sha256': sha(executable)}


def same_process(saved: dict) -> bool:
    try:
        return identity(saved['pid']) == saved
    except (FileNotFoundError, ProcessLookupError):
        return False


def listening(port: int) -> bool:
    with socket.socket() as sock:
        sock.settimeout(0.3)
        return sock.connect_ex(('127.0.0.1', port)) == 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', type=Path, default=ROOT / 'target/debug')
    parser.add_argument('--out-dir', type=Path, help='new private receipt directory; default is a disposable temporary directory')
    parser.add_argument('--data-dir', type=Path, help='new private native data directory')
    parser.add_argument('--server-config', type=Path, help='private native config; default is a copied example')
    parser.add_argument('--lifetime', type=float, default=120)
    parser.add_argument('--close-reserve', type=float, default=20)
    parser.add_argument('--container-engine', choices=['docker', 'podman'])
    parser.add_argument('--container-image')
    args = parser.parse_args()
    if not 0 < args.close_reserve < args.lifetime:
        parser.error('Require 0 < close reserve < lifetime.')
    if bool(args.container_engine) != bool(args.container_image):
        parser.error('Specify both --container-engine and --container-image.')
    if args.container_engine and (args.data_dir or args.server_config):
        parser.error('Container checks own a new named volume and use the image config.')
    client = (args.bin_dir / 'webtool').resolve()
    server = (args.bin_dir / 'webtoold').resolve()
    if not client.is_file() or (not args.container_engine and not server.is_file()):
        parser.exit(2, 'Build or install the required real binaries first.\n')
    temporary = tempfile.TemporaryDirectory(prefix='webtool-onboarding-') if not args.out_dir else None
    directory = Path(temporary.name) if temporary else args.out_dir.resolve()
    if not temporary:
        directory.mkdir(parents=True, exist_ok=False)
    home = directory / 'home'
    home.mkdir()
    env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / '.config'))
    env.pop('WEBTOOL_SERVER', None)
    data = args.data_dir.resolve() if args.data_dir else directory / 'data'
    config = args.server_config.resolve() if args.server_config else directory / 'server.toml'
    if not args.container_engine:
        if data.exists() and any(data.iterdir()):
            parser.error('The diagnostic data directory must be new or empty.')
        data.mkdir(parents=True, exist_ok=True)
        if not args.server_config:
            shutil.copyfile(ROOT / 'config.example.toml', config)
    fixture = directory / 'source.md'
    fixture.write_bytes(b'# Retained source\n\nExact code and zero values stay in the original.\n\n```rust\nlet count = 0;\n```\n')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    base = f'http://127.0.0.1:{port}'
    started_wall, started = time.time(), time.monotonic()
    work_deadline = started + args.lifetime - args.close_reserve
    close_deadline = started + args.lifetime
    token = 'webtool-smoke-' + uuid.uuid4().hex
    volume = token + '-data'
    state = {'process': None, 'identity': None, 'container': None, 'log': None}
    receipt = {
        'started_at_utc': datetime.fromtimestamp(started_wall, timezone.utc).isoformat(),
        'work_deadline_utc': datetime.fromtimestamp(started_wall + args.lifetime - args.close_reserve, timezone.utc).isoformat(),
        'close_deadline_utc': datetime.fromtimestamp(started_wall + args.lifetime, timezone.utc).isoformat(),
        'lifetime_seconds': args.lifetime, 'close_reserve_seconds': args.close_reserve,
        'endpoint': base, 'client': {'path': str(client), 'sha256': sha(client)},
        'engine': args.container_engine, 'image': args.container_image,
        'starts': [], 'commands': [], 'closures': [], 'passed': False,
    }

    def budget(closing=False, maximum=10):
        deadline = close_deadline if closing else work_deadline
        remaining = deadline - time.monotonic()
        if remaining <= 0.25:
            raise TimeoutError('Diagnostic deadline exhausted. No replacement process will start.')
        return min(maximum, remaining)

    def record(command):
        receipt['commands'].append({'at_utc': datetime.now(timezone.utc).isoformat(), 'command': [str(item) for item in command]})
        (directory / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')

    def command(arguments, closing=False, maximum=10):
        budget(closing, maximum)
        if state['identity'] and state['process'].poll() is None and not same_process(state['identity']):
            raise RuntimeError('Owned process identity changed.')
        record(arguments)
        execution_env = os.environ.copy() if args.container_engine and arguments[0] == args.container_engine else env
        return subprocess.run(arguments, cwd=directory, env=execution_env, capture_output=True, text=True, timeout=budget(closing, maximum), check=True)

    def inspect_container(closing=False):
        value = json.loads(command([args.container_engine, 'inspect', token], closing, 3).stdout)[0]
        return {key: value[key] for key in ('Id', 'Image', 'Created')}

    def stop():
        process = state['process']
        if process is None:
            return
        closed = {'process': state['identity'], 'container': state['container']}
        if state['container']:
            if inspect_container(True) != state['container']:
                raise RuntimeError('Container ownership changed before stop.')
            command([args.container_engine, 'stop', '--time', str(max(1, int(budget(True, 8) - 1))), token], True, 10)
        elif process.poll() is None:
            if not same_process(state['identity']):
                raise RuntimeError('Server ownership changed before stop.')
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=budget(True, 5))
        except subprocess.TimeoutExpired:
            if same_process(state['identity']):
                os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=budget(True, 3))
        if state['container']:
            command([args.container_engine, 'rm', token], True, 3)
        while listening(port) and time.monotonic() < close_deadline - 0.5:
            time.sleep(0.05)
        closed.update(returncode=process.returncode, reaped=process.poll() is not None, listener_closed=not listening(port), within_close_deadline=time.monotonic() <= close_deadline)
        receipt['closures'].append(closed)
        if state['log']:
            state['log'].close()
        state.update(process=None, identity=None, container=None, log=None)
        if not closed['listener_closed']:
            raise RuntimeError('Owned listener did not close.')

    def hard_close():
        # Independent close deadline. Do not signal a reused PID or foreign container.
        process, saved = state['process'], state['identity']
        if args.container_engine and state['container']:
            try:
                record([args.container_engine, 'inspect', token])
                current = json.loads(subprocess.check_output([args.container_engine, 'inspect', token], text=True, timeout=2))[0]
                if {key: current[key] for key in ('Id', 'Image', 'Created')} == state['container']:
                    record([args.container_engine, 'kill', token])
                    subprocess.run([args.container_engine, 'kill', token], capture_output=True, timeout=2)
            except (OSError, subprocess.SubprocessError):
                pass
        if process and saved and process.poll() is None and same_process(saved):
            os.killpg(process.pid, signal.SIGKILL)

    timer = threading.Timer(args.lifetime, hard_close)
    timer.daemon = True
    timer.start()
    old_handler = signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError('Work deadline reached.')))
    signal.setitimer(signal.ITIMER_REAL, args.lifetime - args.close_reserve)

    def start(index):
        budget()
        if listening(port):
            raise RuntimeError('Selected diagnostic port became occupied.')
        log = (directory / f'server-{index}.log').open('wb')
        if args.container_engine:
            arguments = [args.container_engine, 'run', '--name', token, '--read-only', '--cap-drop', 'all', '--security-opt', 'no-new-privileges', '--tmpfs', '/tmp:rw,nosuid,nodev,size=256m,mode=1777', '-p', f'127.0.0.1:{port}:8420', '-v', f'{volume}:/data']
            if args.container_engine == 'podman':
                arguments += ['--timeout', str(max(1, int(close_deadline - time.monotonic())))]
            arguments += [args.container_image]
        else:
            arguments = [str(server), '--config', str(config), '--bind', f'127.0.0.1:{port}', '--data-dir', str(data)]
        record(arguments)
        execution_env = os.environ.copy() if args.container_engine else env
        process = subprocess.Popen(arguments, cwd=directory, env=execution_env, stdout=log, stderr=log, start_new_session=True)
        state.update(process=process, log=log)
        # Popen may still be between fork and exec; identify the selected executable.
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError((directory / f'server-{index}.log').read_text())
            saved = identity(process.pid)
            if saved['executable'] == str(Path(shutil.which(args.container_engine)).resolve() if args.container_engine else server):
                break
            time.sleep(0.01)
            budget()
        else:
            raise RuntimeError('Could not verify launched executable identity.')
        state['identity'] = saved
        if args.container_engine:
            for _ in range(100):
                try:
                    state['container'] = inspect_container()
                    break
                except subprocess.CalledProcessError:
                    time.sleep(0.05)
                    budget()
            if not state['container']:
                raise RuntimeError('Container identity unavailable.')
        receipt['starts'].append({'command': arguments, 'identity': saved, 'container': state['container']})
        while True:
            budget()
            if process.poll() is not None or not same_process(saved):
                raise RuntimeError('Server/engine process exited or changed identity before readiness.')
            try:
                with urllib.request.urlopen(base + '/v1/health', timeout=budget(maximum=1)) as response:
                    if response.status == 200:
                        break
            except OSError:
                time.sleep(0.05)

    def cli(*arguments):
        if state['container'] and inspect_container() != state['container']:
            raise RuntimeError('Container ownership changed before CLI operation.')
        result = command([str(client), '--timeout', str(max(1, int(budget(maximum=5)))), '--format', 'json', *arguments], maximum=7)
        receipt['commands'][-1].update(stdout=result.stdout, stderr=result.stderr)
        return json.loads(result.stdout) if result.stdout.strip() else None

    try:
        if args.container_engine:
            command([args.container_engine, 'volume', 'create', '--label', 'webtool.smoke=' + token, volume])
            receipt['volume'] = volume
        start(1)
        cli('connect', base)
        selected = cli('config', 'show')
        receipt['client_config'] = selected
        receipt['doctor'] = cli('doctor')
        cli('library', 'create', 'shared')
        document = cli('ingest', str(fixture), '--library', 'shared', '--actor', 'Reader')
        ident = document['id']
        assert cli('read', ident)['id'] == ident
        assert cli('find', ident, 'Exact code')['matches']
        assert cli('extract', ident, 'code')['data']
        assert cli('search', 'Exact code', '--library', 'shared')['results']
        cli('note', ident, '--actor', 'Reviewer', '--text', 'Reviewed')
        assert cli('notes', ident)[0]['actor'] == 'Reviewer'
        output = directory / 'source-original.md'
        cli('export', ident, '--kind', 'original', '--output', str(output))
        assert output.read_bytes() == fixture.read_bytes()
        receipt.update(document_id=ident, original_sha256=sha(output))
        stop()
        if not args.container_engine:
            backup = directory / 'backup-data'
            shutil.copytree(data, backup)
            with sqlite3.connect(backup / 'webtool.sqlite3') as database:
                check = database.execute('PRAGMA quick_check').fetchone()[0]
                schema = database.execute('PRAGMA user_version').fetchone()[0]
            assert check == 'ok'
            receipt['stopped_backup'] = {'quick_check': check, 'schema': schema, 'objects': {str(p.relative_to(backup)): sha(p) for p in (backup / 'objects').rglob('*') if p.is_file()}}
        start(2) # Planned persistence restart within the same absolute lifetime.
        assert cli('read', ident)['id'] == ident
        items = cli('library', 'items', 'shared')
        assert ident in json.dumps(items)
        receipt['restart_doctor'] = cli('doctor')
        receipt['passed'] = True
        print('PASS: isolated CLI/server connect, upload, shared library, saved read, find, extraction, search, notes, exact original and persistent restart.')
        return 0
    except Exception as error:
        receipt['error'] = str(error)
        raise
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, old_handler)
        try:
            stop()
            if args.container_engine and receipt.get('volume'):
                ownership = json.loads(command([args.container_engine, 'volume', 'inspect', volume], True, 3).stdout)[0]
                if ownership.get('Labels', {}).get('webtool.smoke') != token:
                    raise RuntimeError('Volume ownership changed; refusing removal.')
                command([args.container_engine, 'volume', 'rm', volume], True, 3)
                receipt['volume_removed'] = True
        finally:
            timer.cancel()
            receipt['closed_at_utc'] = datetime.now(timezone.utc).isoformat()
            receipt['within_close_deadline'] = time.monotonic() <= close_deadline
            (directory / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
            if temporary:
                temporary.cleanup()


if __name__ == '__main__':
    raise SystemExit(main())
