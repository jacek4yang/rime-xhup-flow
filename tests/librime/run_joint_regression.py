#!/usr/bin/env python3
"""Isolated real-librime mixed-code, single-writer and restart regression."""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import shlex
import shutil
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--deployed', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--shared-data-dir', type=Path, required=True)
    parser.add_argument('--cli', type=Path, help='exercise the product snapshot validator before native restore')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).resolve().parent
    binary = output / 'joint'
    flags = shlex.split(subprocess.check_output(
        ['pkg-config', '--cflags', '--libs', 'rime'], text=True))
    command = [os.environ.get('CC', 'cc'), '-O2', '-Wall', '-Wextra', '-Werror',
               '-std=c11', str(source / 'runtime_joint.c'), *flags, '-o', str(binary)]
    (output / 'build-command.json').write_text(json.dumps(command, indent=2) + '\n')
    subprocess.run(command, check=True)

    def prepare(name: str, enabled: bool, quota: int = 65536) -> Path:
        user = output / name
        shutil.copytree(args.deployed, user,
                        ignore=shutil.ignore_patterns('*.userdb*', 'user.yaml', 'sync'))
        (user / 'xhup_flow.custom.yaml').write_text(
            f'patch:\n  flow/enable_user_dict: {str(enabled).lower()}\n'
            f'  flow/learning_max_updates: {quota}\n')
        (user / 'default.custom.yaml').write_text(
            'patch:\n  schema_list/+ :\n    - schema: xhup_flow\n  menu/page_size: 5\n')
        with (output / f'{name}-deploy.log').open('wb') as log:
            subprocess.run(['rime_deployer', '--compile', str(user / 'xhup_flow.schema.yaml'),
                            str(user), str(args.shared_data_dir)],
                           stdout=log, stderr=subprocess.STDOUT, check=True)
        return user

    def run(user: Path, label: str, mode: str | None = None) -> None:
        command = [str(binary), str(user), str(args.shared_data_dir)]
        if mode:
            command.append(mode)
        with (output / f'{label}.jsonl').open('wb') as stdout, \
                (output / f'{label}.stderr').open('wb') as stderr:
            subprocess.run(command, stdout=stdout, stderr=stderr, check=True)
        errors = (output / f'{label}.stderr').read_text()
        if 'LuaTranslation::Next error' in errors or 'LuaTranslation::Peek error' in errors:
            raise AssertionError(f'{label}: native Lua callback failed; see saved stderr')

    def export_state(user: Path, label: str) -> tuple[list[str], list[list[str]]]:
        path = output / f'{label}.tsv'
        subprocess.run(['rime_dict_manager', '-e', 'xhup_flow_user', str(path)],
                       cwd=user, check=True, stdout=subprocess.DEVNULL)
        lines = path.read_text().splitlines()
        records = [line.split('\t') for line in lines if line and not line.startswith('#')]
        return lines, records

    def exported(user: Path, label: str, expected: int, previous: Path | None = None) -> None:
        lines, records = export_state(user, label)
        wanted = [['就是拟好', 'jqui nihc', str(expected)]] if expected else []
        old_tick = 0
        if previous:
            old_lines = previous.read_text().splitlines()
            wanted += [line.split('\t') for line in old_lines if line and not line.startswith('#')]
            old_tick = next(int(line.split('\t')[1]) for line in old_lines if line.startswith('#@/tick\t'))
        assert sorted(records) == sorted(wanted), f'{label}: duplicate, missing or misencoded native learning: {records!r}'
        assert f'#@/tick\t{old_tick + expected}' in lines, f'{label}: native tick reset or duplicate learning'
        assert '#@/db_name\txhup_flow_user' in lines, f'{label}: persistent identity changed'

    def save_snapshot(user: Path) -> Path:
        command = ([str(args.cli.resolve()), 'learning', 'export', '--user-data-dir', str(user)]
                   if args.cli else ['rime_dict_manager', '-b', 'xhup_flow_user'])
        subprocess.run(command, cwd=user, check=True, stdout=subprocess.DEVNULL)
        return user / 'xhup_flow_user.userdb.txt'

    typing = prepare('typing', False)
    run(typing, 'typing')
    assert not list(typing.glob('*.userdb*')), 'learning-off typing created a user database'
    learning = prepare('learning', True)
    run(learning, 'first-choice', '--learn')
    exported(learning, 'first-choice', 1)
    run(learning, 'restarted-choice', '--learned')
    exported(learning, 'restarted-choice', 2)
    # Preserve the real native Sentence's learning semantics, not the incidental
    # single-Phrase record layout of an unconditionally promoted joint candidate.
    # The control actually selects the same input with joint search disabled.
    control = prepare('compound-native-control', True)
    compound = prepare('compound', True)
    for label in ('compound-choice', 'compound-restart'):
        run(control, label + '-native', '--native-compound')
        run(compound, label, '--learn-compound')
        expected_lines, expected_records = export_state(control, label + '-native')
        actual_lines, actual_records = export_state(compound, label)
        assert sorted(actual_records) == sorted(expected_records), f'{label}: native learning records changed'
        for prefix in ('#@/tick\t', '#@/db_name\t'):
            assert [line for line in actual_lines if line.startswith(prefix)] == [
                line for line in expected_lines if line.startswith(prefix)
            ], f'{label}: native update clock or persistent identity changed'
    quota = prepare('quota', True, 0)
    run(quota, 'quota', '--quota')
    exported(quota, 'quota', 0)
    migrated = prepare('migrated', True)
    previous = source / 'legacy-state/expected-export.tsv'
    snapshot = source / 'legacy-state/xhup_flow_user.userdb.txt'
    restore = ([str(args.cli.resolve()), 'learning', 'import', '--user-data-dir', str(migrated),
                '--snapshot', str(snapshot)] if args.cli else ['rime_dict_manager', '-r', str(snapshot)])
    (output / 'migration-command.json').write_text(json.dumps(restore, indent=2) + '\n')
    subprocess.run(restore, cwd=migrated, check=True, stdout=subprocess.DEVNULL)
    exported(migrated, 'migration-import', 0, previous)
    run(migrated, 'migration-first-choice', '--learn')
    exported(migrated, 'migration-first-choice', 1, previous)
    run(migrated, 'migration-restart', '--learned')
    exported(migrated, 'migration-restart', 2, previous)
    encoded = prepare('encoded', True)
    run(encoded, 'encoded-seed', '--seed-legacy')
    seed_export = output / 'encoded-seed.tsv'
    subprocess.run(['rime_dict_manager', '-e', 'xhup_flow_user', str(seed_export)],
                   cwd=encoded, check=True, stdout=subprocess.DEVNULL)
    seed_lines = seed_export.read_text().splitlines()
    assert '#@/tick\t6' in seed_lines, 'duplicate encoder paths consumed the learning quota'
    learned = [line.split('\t') for line in seed_lines if line.startswith('你好你好去\t')]
    assert len(learned) == 1 and learned[0][2] == '1', 'selected compound was not learned exactly once'
    run(encoded, 'encoded-legacy', '--recall-legacy')
    run(encoded, 'encoded-joint', '--recall-joint')
    backup = save_snapshot(encoded)
    restored = prepare('encoded-restored', True)
    restore_command = ([str(args.cli.resolve()), 'learning', 'import', '--user-data-dir', str(restored),
                        '--snapshot', str(backup)] if args.cli else ['rime_dict_manager', '-r', str(backup)])
    subprocess.run(restore_command, cwd=restored, check=True, stdout=subprocess.DEVNULL)
    restored_backup = save_snapshot(restored)
    def state(path: Path):
        lines = path.read_text().splitlines()
        tick = next(int(line.split('\t')[1]) for line in lines if line.startswith('#@/tick\t'))
        records = {}
        for line in lines:
            if not line or line.startswith('#'):
                continue
            code, text, values = line.split('\t')
            fields = dict(item.split('=') for item in values.split())
            # UserDbMerger normalizes d/t to the snapshot clock using formula_d.
            # Compare effective probability state, not its equivalent timestamp.
            records[code, text] = (int(fields['c']),
                                  float(fields['d']) * math.exp((int(fields['t']) - tick) / 200))
        return tick, records
    before_tick, before = state(backup)
    # Native encoded entries and ordinary recalled spellings have distinct keys.
    # Preserve both: seed once, then select twice, without duplicate updates.
    assert before_tick == 8 and sum(
        value[0] for (_, text), value in before.items() if text == '你好你好去'
    ) == 3, 'native annotation lost or duplicated actual selection learning'
    after_tick, after = state(restored_backup)
    assert before_tick == after_tick and before.keys() == after.keys(), 'restore changed native codes or quota'
    for key, (commits, probability) in before.items():
        assert after[key][0] == commits, f'restore changed commits: {key!r}'
        # Native UserDbValue::Pack uses six significant decimal digits.
        assert math.isclose(after[key][1], probability, rel_tol=6e-6), f'restore changed probability: {key!r}'
    run(restored, 'encoded-restored', '--recall-joint')
    final_tick, final = state(save_snapshot(restored))
    assert final_tick == before_tick + 1, 'restored annotated selection did not learn exactly once'
    assert sum(value[0] for (_, text), value in final.items() if text == '你好你好去') == 4, \
        'restored annotated candidate lost its native learning identity'
    print(f'PASS mixed-code typing, cursor edits, learning-off privacy, exactly-once learning, restart, quota and old records: {output}')


if __name__ == '__main__':
    main()
