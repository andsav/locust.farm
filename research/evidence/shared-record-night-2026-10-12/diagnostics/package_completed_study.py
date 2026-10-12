#!/usr/bin/env python3
"""Package completed original shared-record evidence without running evaluations.

Invoke from a repository containing the exact frozen source files. Calls only
analyze.package, protocol.verify_seal, and analyze._load_hidden from the harness.
No generation, scoring, or analysis-writing entry point is called. The study
must already contain hidden.json, hidden-seal.json, and analysis.json.

Usage: python3 package_completed_study.py STUDY EVIDENCE [--outside-seal FILE ...]
"""
import argparse
import hashlib
import json
import lzma
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile


def file_info(path):
    digest = hashlib.sha256()
    size = 0
    with Path(path).open('rb') as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    return dict(sha256=digest.hexdigest(), bytes=size)


def safe_member(root, name):
    path = (root / name).resolve()
    if Path(name).is_absolute() or not path.is_relative_to(root.resolve()) or not path.is_file():
        raise ValueError('invalid evidence member: ' + name)
    if (root / name).is_symlink():
        raise ValueError('symlink evidence member: ' + name)
    return path


class HashWriter:
    def __init__(self, stream):
        self.stream = stream
        self.digest = hashlib.sha256()
        self.size = 0

    def write(self, value):
        self.digest.update(value)
        self.size += len(value)
        return self.stream.write(value)


def write_archive(source, names, archive, prefix):
    """Stream sorted deterministic USTAR bytes through a deterministic XZ writer."""
    with Path(archive).open('xb') as raw:
        with lzma.LZMAFile(raw, mode='w', format=lzma.FORMAT_XZ,
                           check=lzma.CHECK_CRC64, preset=6) as compressed:
            writer = HashWriter(compressed)
            with tarfile.open(fileobj=writer, mode='w|', format=tarfile.USTAR_FORMAT) as tar:
                for name in sorted(names):
                    path = safe_member(source, name)
                    info = tarfile.TarInfo(prefix + '/' + name)
                    info.size = path.stat().st_size
                    info.mode = 0o644
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ''
                    with path.open('rb') as handle:
                        tar.addfile(info, handle)
    return dict(sha256=writer.digest.hexdigest(), bytes=writer.size)


def restore_archive(archive, destination, prefix, inventory):
    seen = []
    with tarfile.open(archive, mode='r:xz') as tar:
        for member in tar:
            expected_prefix = prefix + '/'
            if not member.name.startswith(expected_prefix):
                raise ValueError('unexpected archive prefix')
            name = member.name[len(expected_prefix):]
            if name not in inventory or name in seen:
                raise ValueError('unexpected or repeated archive member: ' + name)
            if not member.isfile() or member.mode != 0o644 or any((member.uid, member.gid, member.mtime)):
                raise ValueError('noncanonical archive member metadata: ' + name)
            if member.uname or member.gname:
                raise ValueError('noncanonical archive owner names: ' + name)
            target = (destination / prefix / name).resolve()
            if not target.is_relative_to(destination.resolve()):
                raise ValueError('archive path escapes output')
            target.parent.mkdir(parents=True, exist_ok=True)
            digest = hashlib.sha256()
            size = 0
            with tar.extractfile(member) as source, target.open('xb') as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
                    digest.update(chunk)
                    size += len(chunk)
            if dict(sha256=digest.hexdigest(), bytes=size) != inventory[name]:
                raise ValueError('extracted bytes differ: ' + name)
            seen.append(name)
    if seen != sorted(inventory):
        raise ValueError('archive ordering or inventory differs')
    return destination / prefix


def frozen_source_proof(repo, manifest):
    runtime = manifest['runtime']
    committed, current = {}, {}
    for name, expected in runtime['source_sha256'].items():
        current[name] = file_info(safe_member(repo, name))['sha256']
        original = subprocess.run(['git', 'show', runtime['source_commit'] + ':' + name],
                                  cwd=repo, check=True, capture_output=True).stdout
        committed[name] = hashlib.sha256(original).hexdigest()
        if current[name] != expected or committed[name] != expected:
            raise ValueError('frozen source identity mismatch: ' + name)
    return dict(commit=runtime['source_commit'], file_count=len(current),
                current_files_match=True, committed_files_match=True, source_sha256=current)


def package(study, evidence, repo, outside_seal=()):
    study, evidence, repo = Path(study).resolve(), Path(evidence).resolve(), Path(repo).resolve()
    if evidence.is_relative_to(study):
        raise ValueError('evidence must be outside the study')
    prefix = study.name
    archive = evidence / (prefix + '-sealed.tar.xz')
    receipt_path = evidence / (prefix + '-sealed-receipt.json')
    if archive.exists() or receipt_path.exists():
        raise ValueError('archive or receipt already exists')
    for name in ('manifest.json', 'seal.json', 'hidden.json', 'hidden-seal.json', 'analysis.json'):
        safe_member(study, name)
    manifest = json.loads((study / 'manifest.json').read_bytes())
    proof = frozen_source_proof(repo, manifest)
    if file_info(study / 'bin/locust')['sha256'] != manifest['runtime']['binary_sha256']:
        raise ValueError('frozen binary identity mismatch')
    sys.path.insert(0, str(repo / 'research/experiments/shared_record'))
    import analyze
    import protocol
    seal = json.loads((study / 'seal.json').read_bytes())
    names = set(seal['files']) | {'manifest.json', 'seal.json', 'hidden.json', 'hidden-seal.json', 'analysis.json'}
    names |= {str(path.relative_to(study)) for path in (study / 'hidden-journal').glob('*.json')}
    before = {name: file_info(safe_member(study, name)) for name in sorted(names)}
    external = {name: file_info(safe_member(evidence, name)) for name in outside_seal}
    evidence.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix='shared-record-completed-package-', dir=repo / 'output'))
    staged = scratch / prefix
    analyze.package(study, staged)
    actual = {str(path.relative_to(staged)) for path in staged.rglob('*') if path.is_file()}
    if actual != names:
        raise ValueError('analyze.package inventory differs')
    for name in names:
        if file_info(staged / name) != before[name]:
            raise ValueError('packaged bytes differ: ' + name)
    staged_manifest = protocol.verify_seal(staged)
    analyze._load_hidden(staged, staged_manifest)
    first = scratch / 'archive-first.tar.xz'
    second = scratch / 'archive-second.tar.xz'
    tar_info = write_archive(staged, names, first, prefix)
    second_tar_info = write_archive(staged, names, second, prefix)
    archive_info = file_info(first)
    if tar_info != second_tar_info or archive_info != file_info(second):
        raise ValueError('archive regeneration is not deterministic')
    restored = restore_archive(first, scratch / 'roundtrip', prefix, before)
    restored_manifest = protocol.verify_seal(restored)
    analyze._load_hidden(restored, restored_manifest)
    for name in names:
        if file_info(study / name) != before[name]:
            raise ValueError('source bytes changed during packaging: ' + name)
    for name in external:
        if file_info(evidence / name) != external[name]:
            raise ValueError('outside-seal artifact changed: ' + name)
    with first.open('rb') as source, archive.open('xb') as target:
        while chunk := source.read(1024 * 1024):
            target.write(chunk)
    if file_info(archive) != archive_info:
        raise ValueError('saved archive differs')
    receipt = dict(schema='shared-record-evidence-archive.v2', study_id=manifest['study_id'],
                   archive=archive.name, archive_sha256=archive_info['sha256'], archive_bytes=archive_info['bytes'],
                   uncompressed_tar_sha256=tar_info['sha256'], uncompressed_tar_bytes=tar_info['bytes'],
                   payload_bytes=sum(info['bytes'] for info in before.values()), member_count=len(names),
                   format=dict(archive='ustar', compression='xz', compression_preset=6, integrity_check='CRC64',
                               mtime=0, uid=0, gid=0, uname='', gname='', file_mode='0644', order='relative path sorted'),
                   packager=dict(call='analyze.package(source, disposable_output_staging)',
                                 source='research/experiments/shared_record/analyze.py',
                                 sha256=file_info(repo / 'research/experiments/shared_record/analyze.py')['sha256'],
                                 wrapper_sha256=file_info(Path(__file__))['sha256']),
                   frozen_source_commit=manifest['runtime']['source_commit'], frozen_source_proof=proof,
                   frozen_binary_sha256=manifest['runtime']['binary_sha256'],
                   manifest_sha256=before['manifest.json']['sha256'], seal_sha256=before['seal.json']['sha256'],
                   hidden_sha256=before['hidden.json']['sha256'], hidden_seal_sha256=before['hidden-seal.json']['sha256'],
                   analysis_sha256=before['analysis.json']['sha256'], outside_seal=external,
                   verification=dict(source_bytes_unchanged=True, package_bytes_equal_source=True,
                                     archive_roundtrip_bytes_equal_source=True, deterministic_regeneration=True,
                                     sealed_files_verified=len(seal['files']),
                                     hidden_journal_files_verified=sum(name.startswith('hidden-journal/') for name in names),
                                     binary_omitted=True, private_daemon_state_omitted=True, model_calls=0, evaluator_runs=0),
                   members=before)
    with receipt_path.open('x') as target:
        target.write(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    return receipt, scratch


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--repo', type=Path, default=Path.cwd())
    parser.add_argument('--outside-seal', action='append', default=[])
    args = parser.parse_args()
    receipt, scratch = package(args.study, args.evidence, args.repo, args.outside_seal)
    print(json.dumps({key: receipt[key] for key in ('study_id', 'archive_sha256', 'archive_bytes',
                                                   'uncompressed_tar_bytes', 'payload_bytes', 'member_count',
                                                   'verification')}, indent=2))
    print('Scratch:', scratch)


if __name__ == '__main__':
    main()
