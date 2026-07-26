import hashlib
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import release_tools


class VersionTests(unittest.TestCase):
    def test_initial_feature_release_is_v0_1_0(self):
        self.assertEqual(release_tools.next_tag([], ["feat: initial product"]), "v0.1.0")

    def test_fix_increments_patch(self):
        self.assertEqual(release_tools.next_tag(["v1.2.3"], ["fix: repair output"]), "v1.2.4")

    def test_feature_increments_minor(self):
        self.assertEqual(release_tools.next_tag(["v1.2.3"], ["feat(tui): add search"]), "v1.3.0")

    def test_breaking_marker_increments_major(self):
        self.assertEqual(release_tools.next_tag(["v1.2.3"], ["feat(cli)!: change flags"]), "v2.0.0")

    def test_breaking_change_footer_increments_major(self):
        message = "refactor: reshape model\n\nBREAKING CHANGE: API removed"
        self.assertEqual(release_tools.next_tag(["v1.2.3"], [message]), "v2.0.0")

    def test_no_commits_means_no_release(self):
        self.assertIsNone(release_tools.next_tag(["v1.2.3"], []))


class FileTests(unittest.TestCase):
    def test_sync_version_updates_manifest_and_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text(
                '[package]\nname = "markdown-tasks"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            (root / "Cargo.lock").write_text(
                '[[package]]\nname = "markdown-tasks"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            release_tools.sync_version("v2.3.4", root)
            self.assertIn('version = "2.3.4"', (root / "Cargo.toml").read_text())
            self.assertIn('version = "2.3.4"', (root / "Cargo.lock").read_text())

    def test_packages_archives_and_matching_checksums(self):
        for archive_format in ("tar.gz", "zip"):
            with self.subTest(archive_format=archive_format), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                binary = root / ("mdt.exe" if archive_format == "zip" else "mdt")
                binary.write_bytes(b"binary")
                (root / "README.md").write_text("readme", encoding="utf-8")
                (root / "LICENSE").write_text("license", encoding="utf-8")
                archive, checksum = release_tools.package_binary(
                    "v1.2.3",
                    "test-target",
                    binary,
                    archive_format,
                    root / "dist",
                    root,
                )
                expected = hashlib.sha256(archive.read_bytes()).hexdigest()
                self.assertEqual(checksum.read_text().split()[0], expected)
                if archive_format == "zip":
                    with zipfile.ZipFile(archive) as bundle:
                        names = bundle.namelist()
                else:
                    with tarfile.open(archive) as bundle:
                        names = bundle.getnames()
                        binary_info = next(
                            member for member in bundle.getmembers() if member.name.endswith("/mdt")
                        )
                        self.assertNotEqual(binary_info.mode & 0o111, 0)
                self.assertTrue(any(name.endswith("/README.md") for name in names))
                self.assertTrue(any(name.endswith("/LICENSE") for name in names))
                self.assertTrue(any(name.endswith(("/mdt", "/mdt.exe")) for name in names))


if __name__ == "__main__":
    unittest.main()
