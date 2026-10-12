#!/usr/bin/env python3
"""Offline archive tests using only disposable byte fixtures."""
import io
import json
import lzma
from pathlib import Path
import tarfile
import tempfile
import unittest

from package_completed_study import file_info, package, restore_archive, safe_member, write_archive


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.source = self.root / 'study'
        (self.source / 'hidden-journal').mkdir(parents=True)
        (self.source / 'manifest.json').write_text(json.dumps({'value': 'α\n'}))
        (self.source / 'hidden-journal/record.json').write_bytes(b'1234567890' * 12345)
        self.names = {'manifest.json', 'hidden-journal/record.json'}
        self.inventory = {name: file_info(self.source / name) for name in self.names}
        self.archive = self.root / 'study.tar.xz'

    def tearDown(self):
        self.temp.cleanup()

    def test_independent_regeneration_is_exact(self):
        first = write_archive(self.source, self.names, self.archive, 'study')
        other = self.root / 'second.tar.xz'
        second = write_archive(self.source, reversed(sorted(self.names)), other, 'study')
        self.assertEqual(first, second)
        self.assertEqual(self.archive.read_bytes(), other.read_bytes())
        self.assertEqual(first['bytes'], len(lzma.decompress(self.archive.read_bytes())))

    def test_roundtrip_is_exact_and_source_unchanged(self):
        write_archive(self.source, self.names, self.archive, 'study')
        restored = restore_archive(self.archive, self.root / 'restored', 'study', self.inventory)
        for name in self.names:
            self.assertEqual((restored / name).read_bytes(), (self.source / name).read_bytes())
            self.assertEqual(file_info(self.source / name), self.inventory[name])

    def test_archive_contains_only_canonical_sorted_regular_files(self):
        write_archive(self.source, self.names, self.archive, 'study')
        with tarfile.open(self.archive, 'r:xz') as archive:
            members = archive.getmembers()
        self.assertEqual([member.name for member in members], ['study/' + name for name in sorted(self.names)])
        for member in members:
            self.assertTrue(member.isfile())
            self.assertEqual((member.mode, member.uid, member.gid, member.mtime), (0o644, 0, 0, 0))
            self.assertEqual((member.uname, member.gname), ('', ''))

    def test_corrupt_expected_hash_is_rejected(self):
        write_archive(self.source, self.names, self.archive, 'study')
        inventory = dict(self.inventory)
        inventory['hidden-journal/record.json'] = dict(sha256='0' * 64, bytes=123450)
        with self.assertRaisesRegex(ValueError, 'extracted bytes differ'):
            restore_archive(self.archive, self.root / 'restored', 'study', inventory)

    def test_unexpected_member_is_rejected(self):
        write_archive(self.source, self.names, self.archive, 'study')
        with self.assertRaisesRegex(ValueError, 'unexpected or repeated archive member'):
            restore_archive(self.archive, self.root / 'restored', 'study', {'manifest.json': self.inventory['manifest.json']})

    def test_missing_member_is_rejected(self):
        write_archive(self.source, {'manifest.json'}, self.archive, 'study')
        with self.assertRaisesRegex(ValueError, 'ordering or inventory'):
            restore_archive(self.archive, self.root / 'restored', 'study', self.inventory)

    def test_existing_archive_is_never_overwritten(self):
        self.archive.write_bytes(b'keep existing bytes')
        with self.assertRaises(FileExistsError):
            write_archive(self.source, self.names, self.archive, 'study')
        self.assertEqual(self.archive.read_bytes(), b'keep existing bytes')

    def test_source_paths_cannot_escape_or_be_symlinks(self):
        (self.root / 'outside.json').write_text('{}')
        (self.source / 'link.json').symlink_to(self.root / 'outside.json')
        for name in ('../outside.json', str(self.root / 'outside.json'), 'link.json', 'missing.json'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                safe_member(self.source, name)

    def test_noncanonical_member_timestamp_is_rejected(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode='w', format=tarfile.USTAR_FORMAT) as archive:
            info = tarfile.TarInfo('study/manifest.json')
            raw = (self.source / 'manifest.json').read_bytes()
            info.size, info.mode, info.mtime = len(raw), 0o644, 1
            archive.addfile(info, io.BytesIO(raw))
        self.archive.write_bytes(lzma.compress(buffer.getvalue()))
        with self.assertRaisesRegex(ValueError, 'noncanonical archive member metadata'):
            restore_archive(self.archive, self.root / 'restored', 'study', self.inventory)

    def test_evidence_cannot_be_written_inside_study(self):
        with self.assertRaisesRegex(ValueError, 'outside the study'):
            package(self.source, self.source / 'archive', self.root)


if __name__ == '__main__':
    unittest.main()
