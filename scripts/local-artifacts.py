#!/usr/bin/env python3
"""Explicit local artifact retention, never an automatic scanner or scheduler.

plan INVENTORY prints a deterministic JSON plan without touching artifacts.
apply PLAN --sha256 HEX --confirm-inactive rechecks every kept/deleted file.
The caller must first stop/check builds and processes using these files. The
confirmation only acknowledges that external check; this tool cannot prove
inactivity. Cargo locks serialize cooperating builds, not arbitrary writers.
No recursive deletion, installation, remote operation or configuration change.
"""
import argparse
from contextlib import ExitStack
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

REPO = Path(__file__).absolute().parent.parent
SYSTEM_OWNER = os.stat('/').st_uid  # Sandboxed hosts can map the system owner to nobody.
FIELDS = {'version', 'kind', 'target', 'profile', 'features', 'source_id', 'path',
          'size', 'sha256', 'dev', 'ino', 'mode', 'mtime_ns', 'nlink'}
KINDS = {'main', 'library', 'representative', 'test', 'metadata', 'auxiliary'}
NOTICE = 'Caller externally confirmed inactivity; not independently detected. Cargo locks cover cooperating builds only.'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def decode(raw):
    require(len(raw) <= 32 * 1024 * 1024, 'JSON input too large')
    def unique(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def absolute(value):
    require(isinstance(value, str) and value and '\x00' not in value, 'invalid path')
    path = Path(value)
    require(path.is_absolute() and '..' not in path.parts, 'absolute non-traversing path required')
    return path


def root_allowed(path):
    return path in (REPO/'target/debug/deps', REPO/'target/release/deps') or (
        path.parent == Path('/tmp') and re.fullmatch(r'suzaku-[A-Za-z0-9][A-Za-z0-9._-]{0,127}', path.name))


def normalized(inventory):
    require(isinstance(inventory, dict) and set(inventory) == {'format', 'allowed_roots', 'pinned_source_ids', 'entries'}
            and type(inventory['format']) is int and inventory['format'] == 1, 'inventory schema')
    roots = [absolute(p) for p in inventory['allowed_roots']]
    require(roots and len(roots) == len(set(roots)) and all(root_allowed(p) for p in roots), 'root not allowlisted')
    pins = inventory['pinned_source_ids']
    require(isinstance(pins, list) and all(isinstance(p, str) and p for p in pins) and len(pins) == len(set(pins)), 'pins schema')
    require(isinstance(inventory['entries'], list) and len(inventory['entries']) <= 100000, 'entries schema')
    entries = []
    for entry in inventory['entries']:
        require(isinstance(entry, dict) and FIELDS <= set(entry) <= FIELDS | {'role', 'mapping_proof', 'archive_format'}, 'entry schema')
        item = dict(entry); path = absolute(item['path'])
        require(any(path != root and root in path.parents for root in roots), 'file outside explicit roots')
        require(all(item[k] is None or isinstance(item[k], str) for k in ('version', 'kind', 'target', 'profile', 'source_id')), 'metadata strings')
        require(isinstance(item['features'], list) and all(isinstance(f, str) and f for f in item['features'])
                and len(set(item['features'])) == len(item['features']), 'features schema')
        require(all(type(item[k]) is int and item[k] >= 0 for k in ('size', 'dev', 'ino', 'mtime_ns', 'nlink'))
                and item['ino'] > 0 and item['nlink'] > 0, 'identity schema')
        mode = item['mode']
        if isinstance(mode, str):
            require(re.fullmatch(r'[0-7]{4}', mode), 'mode schema'); mode = int(mode, 8)
        require(type(mode) is int and 0 <= mode <= 0o7777, 'mode schema')
        require(isinstance(item['sha256'], str) and re.fullmatch(r'[0-9a-f]{64}', item['sha256']), 'SHA256 schema')
        require(item.get('role', 'copy') in ('canonical', 'copy'), 'role schema')
        item.update(path=str(path), features=sorted(item['features']), mode=f'{mode:04o}', role=item.get('role', 'copy'))
        entries.append(item)
    require(len(entries) == len({e['path'] for e in entries}), 'duplicate file path')
    return sorted(map(str, roots)), sorted(pins), sorted(entries, key=lambda e: e['path'])


def known(entry):
    return bool(isinstance(entry['version'], str) and re.fullmatch(r'\d+\.\d+\.\d+', entry['version'])
                and entry['kind'] in KINDS and all(entry[k] and entry[k] != 'unknown' for k in ('target', 'profile', 'source_id')))


def make_plan(inventory):
    roots, pins, entries = normalized(inventory)
    versions = sorted({e['version'] for e in entries if known(e)}, key=lambda v: tuple(map(int, v.split('.'))))
    for entry in entries:
        version = entry['version']
        if not known(entry): keep, reason = True, 'unknown-provenance'
        elif entry['nlink'] != 1: keep, reason = True, 'multiple-links-protected'
        elif entry['source_id'] in pins: keep, reason = True, 'pinned-source'
        elif versions and version == versions[-1]: keep, reason = True, 'latest-version'
        elif len(versions) > 1 and version == versions[-2] and entry['kind'] in {'main', 'library', 'representative'}:
            keep, reason = True, 'previous-representative'
        elif entry['kind'] == 'main': keep, reason = True, 'older-main'
        else: keep, reason = False, 'older-rebuildable'
        entry.update(action='keep' if keep else 'delete', reason=reason)
    groups = {}
    for entry in entries:
        if known(entry):
            key = tuple(entry[k] for k in ('kind', 'target', 'profile')) + (tuple(entry['features']),) + tuple(
                entry[k] for k in ('version', 'source_id', 'sha256'))
            groups.setdefault(key, []).append(entry)
    for group in groups.values():
        canonical = min(group, key=lambda e: (e['role'] != 'canonical', e['path']))
        require(all(e['size'] == canonical['size'] for e in group), 'same hash with inconsistent sizes')
        for entry in group:
            if entry is not canonical and entry['nlink'] == 1:
                entry.update(action='delete', reason='duplicate-of:' + canonical['path'])
    return dict(format=1, kind='explicit-local-artifact-plan', allowed_roots=roots,
                pinned_source_ids=pins, external_activity_check_required=True, entries=entries)


def safe_permissions(info, directory=False, system_tmp=False):
    mode = stat.S_IMODE(info.st_mode)
    require(info.st_uid in ((0, SYSTEM_OWNER, os.getuid()) if directory else (os.getuid(),)), 'untrusted file owner')
    if system_tmp:
        require(info.st_uid == SYSTEM_OWNER and mode & stat.S_ISVTX, 'unsafe /tmp')
        return
    require(not mode & (0o002 | stat.S_ISUID | stat.S_ISGID), 'dangerous write/set-id permissions')
    require(not mode & 0o020 or (info.st_uid == os.getuid() and info.st_gid in os.getgroups() + [os.getgid()]), 'untrusted group write')


def parent_fd(path):
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        current = Path('/')
        for name in path.parent.parts[1:]:
            child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd); fd = child; current /= name
            safe_permissions(os.fstat(fd), directory=True, system_tmp=current == Path('/tmp'))
        return fd
    except BaseException:
        os.close(fd); raise


def identity(info):
    return dict(dev=info.st_dev, ino=info.st_ino, mode=f'{stat.S_IMODE(info.st_mode):04o}',
                size=info.st_size, mtime_ns=info.st_mtime_ns, nlink=info.st_nlink)


def check_file(entry):
    path = Path(entry['path']); directory = parent_fd(path)
    try:
        try: fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
        except FileNotFoundError:
            require(entry['action'] == 'delete', 'retained file missing: ' + str(path)); return False
        try:
            before = os.fstat(fd); require(stat.S_ISREG(before.st_mode), 'not a regular file')
            safe_permissions(before)
            require(identity(before) == {k: entry[k] for k in identity(before)}, 'artifact identity changed: ' + str(path))
            require(entry['action'] != 'delete' or before.st_nlink == 1, 'refuse linked deletion')
            digest = hashlib.sha256()
            while chunk := os.read(fd, 1024 * 1024): digest.update(chunk)
            require(identity(os.fstat(fd)) == identity(before) and digest.hexdigest() == entry['sha256'], 'artifact content changed: ' + str(path))
            require(identity(os.stat(path.name, dir_fd=directory, follow_symlinks=False)) == identity(before), 'artifact path replaced')
            return True
        finally: os.close(fd)
    finally: os.close(directory)


def cargo_locks(entries):
    profiles = set()
    for entry in entries:
        path = Path(entry['path'])
        for profile in ('debug', 'release'):
            if REPO/'target'/profile/'deps' in path.parents: profiles.add(profile)
        if path.parts[:2] == ('/', 'tmp'):
            if entry['profile'] in ('dev', 'debug', 'test'): profiles.add('debug')
            elif entry['profile'] in ('release', 'release-package'): profiles.add('release')
    return [REPO/'target'/p/'.cargo-lock' for p in sorted(profiles)]


def validate_plan(plan):
    require(isinstance(plan, dict) and set(plan) == {'format', 'kind', 'allowed_roots', 'pinned_source_ids', 'external_activity_check_required', 'entries'}, 'plan schema')
    inventory = dict(format=plan['format'], allowed_roots=plan['allowed_roots'], pinned_source_ids=plan['pinned_source_ids'],
                     entries=[{k: v for k, v in e.items() if k not in ('action', 'reason')} for e in plan['entries']])
    require(make_plan(inventory) == plan, 'plan policy/actions changed')


def apply_plan(plan, confirm_inactive=False):
    require(confirm_inactive, 'external inactivity check must be explicitly confirmed')
    validate_plan(plan)
    outcomes = []
    with ExitStack() as stack:
        for path in cargo_locks(plan['entries']):
            directory = parent_fd(path)
            try: fd = os.open(path.name, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
            finally: os.close(directory)
            stack.callback(os.close, fd); info = os.fstat(fd)
            require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, 'unsafe Cargo lock')
            safe_permissions(info); fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        for entry in plan['entries']: check_file(entry)  # Entire batch, including all kept hashes, before the first unlink.
        for entry in plan['entries']:
            if entry['action'] == 'keep':
                outcomes.append(dict(path=entry['path'], result='kept')); continue
            try:
                if not check_file(entry): result = 'already_absent'
                else:
                    path = Path(entry['path']); directory = parent_fd(path)
                    try:
                        info = os.stat(path.name, dir_fd=directory, follow_symlinks=False)
                        require(identity(info) == {k: entry[k] for k in identity(info)}, 'artifact changed before unlink')
                        os.unlink(path.name, dir_fd=directory); result = 'deleted'
                    except FileNotFoundError: result = 'already_absent'
                    finally: os.close(directory)
                outcomes.append(dict(path=entry['path'], result=result))
            except (OSError, ValueError, KeyboardInterrupt) as error:
                return dict(status='PARTIAL_FAILURE', outcomes=outcomes, uncertain_path=entry['path'], error=str(error), activity_check=NOTICE)
    return dict(status='APPLIED', outcomes=outcomes, activity_check=NOTICE,
                recovery='Deleted build artifacts are rebuildable; no trash or backup is created by this tool.')


def output_file(value, entries):
    path = absolute(str(value))
    require(len(path.parts) >= 4 and root_allowed(Path(*path.parts[:3]))
            and path.parts[:2] == ('/', 'tmp') and path.name != '.cargo-lock', 'output requires explicit /tmp/suzaku-* task directory')
    require(str(path) not in {e['path'] for e in entries}, 'output overlaps inventory')
    directory = parent_fd(path)
    try: return os.open(path.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=directory)
    finally: os.close(directory)


def emit(result, fd):
    raw = (json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()
    if fd is None:
        print(raw.decode(), end=''); return
    os.lseek(fd, 0, os.SEEK_SET); os.ftruncate(fd, 0)
    with os.fdopen(os.dup(fd), 'wb') as stream:
        stream.write(raw); stream.flush(); os.fsync(stream.fileno())
    counts = {}
    for row in result.get('entries', result.get('outcomes', [])):
        key = row.get('action', row.get('result')); counts[key] = counts.get(key, 0) + 1
    print(json.dumps(dict(status=result.get('status', 'PLAN'), report_sha256=hashlib.sha256(raw).hexdigest(), counts=counts), sort_keys=True))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    preview = commands.add_parser('plan'); preview.add_argument('inventory', type=Path)
    apply = commands.add_parser('apply'); apply.add_argument('plan', type=Path)
    apply.add_argument('--sha256', required=True); apply.add_argument('--confirm-inactive', action='store_true')
    for command in (preview, apply): command.add_argument('--output', type=Path, help='exclusive new /tmp/suzaku-* JSON report; stdout prints only SHA and counts')
    args = parser.parse_args(argv); fd = None; applied_result = None
    try:
        raw = (args.inventory if args.command == 'plan' else args.plan).read_bytes()
        if args.command == 'plan': result = make_plan(decode(raw))
        else:
            require(re.fullmatch(r'[0-9a-f]{64}', args.sha256) and hashlib.sha256(raw).hexdigest() == args.sha256, 'plan SHA256 mismatch')
            result = decode(raw); validate_plan(result)
        if args.output:
            fd = output_file(args.output, result['entries'])
        if args.command == 'apply':
            if fd is not None:
                initial = b'{"status":"IN_PROGRESS_NOT_SUCCESS","outcomes":[]}\n'
                os.write(fd, initial); os.fsync(fd)
            result = apply_plan(result, args.confirm_inactive)
            applied_result = result
        emit(result, fd)
        return 1 if result.get('status') == 'PARTIAL_FAILURE' else 0
    except (OSError, ValueError, TypeError, KeyError) as error:
        if applied_result is not None:
            failure = dict(status='REPORT_FAILURE_AFTER_APPLY', receipt_uncertain=True,
                           error=str(error), apply_status=applied_result['status'],
                           apply_result=applied_result)
            try:
                emit(failure, fd)
            except (OSError, ValueError, TypeError, KeyError):
                try:
                    print('REPORT_FAILURE_AFTER_APPLY: deletion may already have occurred; '
                          'the apply outcomes could not be delivered. Do not assume nothing was deleted.', file=sys.stderr)
                except OSError:
                    pass
            return 1
        emit(dict(status='REFUSED', error=str(error)), fd); return 2
    finally:
        if fd is not None: os.close(fd)


if __name__ == '__main__':
    sys.exit(main())
