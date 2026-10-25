#!/usr/bin/env python3
"""Synthetic temporary artifacts only; never apply against real build directories."""
from contextlib import redirect_stderr, redirect_stdout
import copy
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('local_artifacts', Path(__file__).with_name('local-artifacts.py'))
TOOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOL)


class RetentionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='suzaku-artifacts-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo_patch = patch.object(TOOL, 'REPO', self.root)
        self.repo_patch.start(); self.addCleanup(self.repo_patch.stop)
        self.deps = self.root/'target/debug/deps'
        self.deps.mkdir(parents=True)
        (self.deps.parent/'.cargo-lock').touch(mode=0o600)
        self.release = self.root/'target/release/deps'
        self.release.mkdir(parents=True)
        (self.release.parent/'.cargo-lock').touch(mode=0o600)

    def entry(self, name, version='3.0.0', kind='test', data=None, **changes):
        path = self.deps/name
        path.write_bytes(name.encode() if data is None else data)
        info = path.stat()
        item = dict(version=version, kind=kind, target='test-target', profile='debug',
                    features=['gpu'], source_id='source:' + str(version), path=str(path),
                    size=info.st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                    dev=info.st_dev, ino=info.st_ino, mode=f'{stat.S_IMODE(info.st_mode):04o}',
                    mtime_ns=info.st_mtime_ns, nlink=info.st_nlink)
        return dict(item, **changes)

    def inventory(self, entries, pins=None):
        return dict(format=1, allowed_roots=[str(self.deps)], pinned_source_ids=pins or [], entries=entries)

    def test_tiers_use_semver_and_preserve_unknown_and_pinned_sources(self):
        entries = [self.entry('latest', '0.10.0'), self.entry('previous-main', '0.9.9', 'main'),
                   self.entry('previous-lib', '0.9.9', 'library'), self.entry('previous-rep', '0.9.9', 'representative'),
                   self.entry('previous-test', '0.9.9'), self.entry('old-main', '0.8.0', 'main'),
                   self.entry('old-lib', '0.8.0', 'library'), self.entry('old-pin', '0.7.0'),
                   self.entry('unknown', None), self.entry('unknown-kind', '0.6.0', 'unknown')]
        plan = TOOL.make_plan(self.inventory(entries, ['source:0.7.0']))
        actions = {Path(e['path']).name: e['action'] for e in plan['entries']}
        self.assertEqual({k for k, v in actions.items() if v == 'delete'}, {'previous-test', 'old-lib'})

    def test_dedup_uses_all_identity_dimensions_role_then_path_not_mtime(self):
        a = self.entry('a', data=b'same'); b = self.entry('b', data=b'same', role='canonical')
        c = self.entry('c', data=b'same', source_id='other-source')
        d = self.entry('d', data=b'same', features=['other'])
        a['mtime_ns'] = 999999999999999999  # Policy must not examine filesystem or age.
        plan = TOOL.make_plan(self.inventory([a, b, c, d]))
        self.assertEqual([Path(e['path']).name for e in plan['entries'] if e['action'] == 'delete'], ['a'])
        self.assertEqual(plan, TOOL.make_plan(self.inventory([d, c, b, a])))

    def test_unknown_duplicates_and_shared_inodes_are_all_protected(self):
        a = self.entry('a', version=None, data=b'same'); b = self.entry('b', version=None, data=b'same')
        shared = self.entry('shared', version='1.0.0')
        os.link(shared['path'], self.deps/'other-link'); shared['nlink'] = 2
        latest = self.entry('latest')
        plan = TOOL.make_plan(self.inventory([a, b, shared, latest]))
        self.assertTrue(all(e['action'] == 'keep' for e in plan['entries']))
        self.assertEqual(TOOL.apply_plan(plan, True)['status'], 'APPLIED')

    def test_audit_fields_and_exact_integer_timestamps_survive_plan(self):
        item = self.entry('a', mapping_proof={'kind': 'direct', 'line': 3}, archive_format='elf')
        item['mtime_ns'] = 1792833220123456789
        plan = TOOL.make_plan(self.inventory([item]))
        row = plan['entries'][0]
        self.assertEqual(row['mapping_proof'], item['mapping_proof'])
        self.assertEqual(row['archive_format'], 'elf')
        self.assertEqual(TOOL.decode(json.dumps(plan).encode())['entries'][0]['mtime_ns'], item['mtime_ns'])

    def test_canonical_shared_inode_can_discard_only_its_single_link_copy(self):
        shared = self.entry('canonical', data=b'same', role='canonical')
        os.link(shared['path'], self.deps/'outside-manifest-link'); shared['nlink'] = 2
        duplicate = self.entry('copy', data=b'same')
        plan = TOOL.make_plan(self.inventory([shared, duplicate]))
        self.assertEqual([Path(e['path']).name for e in plan['entries'] if e['action'] == 'delete'], ['copy'])
        self.assertEqual(TOOL.apply_plan(plan, True)['status'], 'APPLIED')
        self.assertTrue(Path(shared['path']).exists()); self.assertFalse(Path(duplicate['path']).exists())

    def test_plan_rejects_invalid_roots_paths_duplicates_modes_and_json_keys(self):
        item = self.entry('a')
        bad = [dict(self.inventory([item]), allowed_roots=['/']),
               dict(self.inventory([item]), allowed_roots=['/tmp']), self.inventory([item, item])]
        for field, value in [('path', str(self.deps/'../escape')), ('size', True), ('sha256', 'x'), ('mode', '9999')]:
            bad.append(self.inventory([dict(item, **{field: value})]))
        for inventory in bad:
            with self.subTest(inventory=inventory), self.assertRaises(ValueError): TOOL.make_plan(inventory)
        with self.assertRaises(ValueError): TOOL.decode(b'{"format":1,"format":1}')

    def removal_plan(self):
        old = self.entry('old', '1.0.0'); kept = self.entry('kept', '2.0.0')
        return TOOL.make_plan(self.inventory([old, kept])), Path(old['path']), Path(kept['path'])

    def test_apply_is_hash_checked_single_file_and_idempotent(self):
        plan, old, kept = self.removal_plan()
        result = TOOL.apply_plan(plan, True)
        self.assertEqual(result['status'], 'APPLIED'); self.assertFalse(old.exists()); self.assertTrue(kept.exists())
        again = TOOL.apply_plan(plan, True)
        self.assertIn(dict(path=str(old), result='already_absent'), again['outcomes'])

    def test_retained_hash_and_all_delete_identities_checked_before_first_unlink(self):
        for mutate in ('keep', 'delete', 'replace', 'link'):
            with self.subTest(mutate=mutate):
                plan, old, kept = self.removal_plan()
                if mutate == 'keep': kept.write_bytes(b'changed')
                elif mutate == 'delete': old.write_bytes(b'changed')
                elif mutate == 'replace': old.unlink(); old.write_bytes(b'old')
                else: os.link(old, self.deps/'extra-link')
                with self.assertRaises(ValueError): TOOL.apply_plan(plan, True)
                self.assertTrue(old.exists())
                extra = self.deps/'extra-link'
                if extra.exists(): extra.unlink()

    def test_absent_retained_file_is_not_idempotent_success(self):
        plan, old, kept = self.removal_plan(); kept.unlink()
        with self.assertRaises(ValueError): TOOL.apply_plan(plan, True)
        self.assertTrue(old.exists())

    def test_active_cargo_lock_prevents_any_deletion(self):
        plan, old, _ = self.removal_plan()
        with (self.deps.parent/'.cargo-lock').open('r+b') as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError): TOOL.apply_plan(plan, True)
        self.assertTrue(old.exists())

    def test_no_confirmation_and_tampered_actions_are_refused(self):
        plan, old, _ = self.removal_plan()
        with self.assertRaises(ValueError): TOOL.apply_plan(plan)
        next(e for e in plan['entries'] if e['action'] == 'delete')['action'] = 'keep'
        with self.assertRaises(ValueError): TOOL.apply_plan(plan, True)
        self.assertTrue(old.exists())

    def test_leaf_symlink_fifo_and_dangerous_modes_fail_before_deletion(self):
        for kind in ('symlink', 'fifo', 'world-write', 'setid'):
            with self.subTest(kind=kind):
                plan, old, kept = self.removal_plan()
                if kind in ('symlink', 'fifo'):
                    old.unlink()
                    if kind == 'symlink': old.symlink_to(kept)
                    else: os.mkfifo(old)
                else: old.chmod(0o666 if kind == 'world-write' else 0o4755)
                with self.assertRaises((OSError, ValueError)): TOOL.apply_plan(plan, True)
                self.assertTrue(kept.exists()); old.unlink()

    def test_parent_symlink_cannot_escape_to_another_directory(self):
        plan, old, _ = self.removal_plan(); moved = self.deps.with_name('moved')
        self.deps.rename(moved); self.deps.symlink_to(moved, target_is_directory=True)
        with self.assertRaises(OSError): TOOL.apply_plan(plan, True)
        self.assertTrue((moved/'old').exists())

    def test_partial_failure_is_not_reported_as_success(self):
        entries = [self.entry('a', '1.0.0'), self.entry('b', '1.0.0'), self.entry('latest')]
        plan = TOOL.make_plan(self.inventory(entries)); real_unlink = os.unlink
        def fail_second(path, **kwargs):
            if path == 'b': raise PermissionError('synthetic refusal')
            return real_unlink(path, **kwargs)
        with patch.object(TOOL.os, 'unlink', side_effect=fail_second): result = TOOL.apply_plan(plan, True)
        self.assertEqual(result['status'], 'PARTIAL_FAILURE')
        self.assertFalse((self.deps/'a').exists()); self.assertTrue((self.deps/'b').exists())

    def test_cli_output_is_exclusive_and_keeps_reports_out_of_inventory(self):
        inventory = self.inventory([self.entry('a')]); source = self.root/'inventory.json'
        source.write_text(json.dumps(inventory)); output = self.root/'plan.json'
        with redirect_stdout(io.StringIO()) as text:
            self.assertEqual(TOOL.main(['plan', str(source), '--output', str(output)]), 0)
        self.assertEqual(json.loads(text.getvalue())['report_sha256'], hashlib.sha256(output.read_bytes()).hexdigest())
        with redirect_stdout(io.StringIO()): self.assertEqual(TOOL.main(['plan', str(source), '--output', str(output)]), 2)
        with self.assertRaises(ValueError): TOOL.output_file(Path(inventory['entries'][0]['path']), inventory['entries'])
        with self.assertRaises(ValueError): TOOL.output_file(self.root/'.cargo-lock', [])

    def test_cli_requires_exact_plan_sha_and_writes_final_apply_report(self):
        plan, old, _ = self.removal_plan(); source = self.root/'plan.json'
        source.write_text(json.dumps(plan)); digest = hashlib.sha256(source.read_bytes()).hexdigest()
        with redirect_stdout(io.StringIO()): self.assertEqual(TOOL.main(['apply', str(source), '--sha256', '0'*64, '--confirm-inactive']), 2)
        self.assertTrue(old.exists()); report = self.root/'applied.json'
        with redirect_stdout(io.StringIO()) as text:
            self.assertEqual(TOOL.main(['apply', str(source), '--sha256', digest, '--confirm-inactive', '--output', str(report)]), 0)
        self.assertEqual(json.loads(report.read_text())['status'], 'APPLIED')
        self.assertEqual(json.loads(text.getvalue())['report_sha256'], hashlib.sha256(report.read_bytes()).hexdigest())

    def test_report_failure_after_apply_preserves_deleted_outcomes_and_never_claims_refusal(self):
        plan, old, _ = self.removal_plan(); source = self.root/'plan.json'
        source.write_text(json.dumps(plan)); digest = hashlib.sha256(source.read_bytes()).hexdigest()
        arguments = ['apply', str(source), '--sha256', digest, '--confirm-inactive']
        real_emit = TOOL.emit; recovered = []
        def fail_once(result, fd):
            recovered.append(result)
            if len(recovered) == 1: raise OSError('synthetic final-report write failure')
            return real_emit(result, fd)
        with patch.object(TOOL, 'emit', side_effect=fail_once), redirect_stdout(io.StringIO()) as text:
            self.assertEqual(TOOL.main(arguments), 1)
        self.assertFalse(old.exists())
        failure = json.loads(text.getvalue())
        self.assertEqual(failure['status'], 'REPORT_FAILURE_AFTER_APPLY')
        self.assertTrue(failure['receipt_uncertain'])
        self.assertEqual(failure['apply_status'], 'APPLIED')
        self.assertEqual(failure['apply_result'], recovered[0])
        self.assertIn(dict(path=str(old), result='deleted'), failure['apply_result']['outcomes'])
        self.assertNotIn('REFUSED', [row['status'] for row in recovered])
        plan, old, _ = self.removal_plan(); source.write_text(json.dumps(plan))
        arguments[arguments.index('--sha256') + 1] = hashlib.sha256(source.read_bytes()).hexdigest()
        with patch.object(TOOL, 'emit', side_effect=OSError('persistent report failure')) as emit, \
                redirect_stderr(io.StringIO()) as error:
            self.assertEqual(TOOL.main(arguments), 1)
        self.assertFalse(old.exists()); self.assertEqual(emit.call_count, 2)
        self.assertEqual(emit.call_args_list[1].args[0]['status'], 'REPORT_FAILURE_AFTER_APPLY')
        self.assertIn(dict(path=str(old), result='deleted'), emit.call_args_list[1].args[0]['apply_result']['outcomes'])
        self.assertIn('deletion may already have occurred', error.getvalue())


if __name__ == '__main__':
    unittest.main()
