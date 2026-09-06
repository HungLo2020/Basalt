import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, call, patch


SCRIPT_PATH = Path(__file__).parents[1] / "DevUtils/PublishMattOSPackage.py"
SPEC = importlib.util.spec_from_file_location("publish_mattos_package", SCRIPT_PATH)
assert SPEC and SPEC.loader
publish = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publish)


class PublishMattOSPackageTests(unittest.TestCase):
    def test_doctor_passes_explicit_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(publish, "repository_root", return_value=root),
                patch.object(publish, "download_latest_script") as download,
                patch.object(publish, "run", return_value=0) as run,
            ):
                self.assertEqual(publish.main(["doctor"]), 0)

            download.assert_called_once_with(root / publish.SCRIPT_RELATIVE_PATH)
            run.assert_called_once_with(
                [publish.sys.executable, str(root / publish.SCRIPT_RELATIVE_PATH),
                 "--repo", "mattpackages", "doctor"],
                root,
            )

    def test_upload_passes_explicit_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "builds/pkg.deb"
            artifact.parent.mkdir()
            artifact.touch()
            (root / publish.BUILD_METADATA_RELATIVE_PATH).write_text(
                "BUILD_ARTIFACT_TYPE=deb\nBUILD_ARTIFACT_PATH=builds/pkg.deb\n",
                encoding="utf-8",
            )
            with (
                patch.object(publish, "repository_root", return_value=root),
                patch.object(publish, "download_latest_script") as download,
                patch.object(publish, "run", return_value=0) as run,
            ):
                self.assertEqual(publish.main(["publish"]), 0)

            download.assert_called_once_with(root / publish.SCRIPT_RELATIVE_PATH)
            self.assertEqual(run.call_args_list, [
                call(["bash", str(root / "DevUtils/Build.sh")], root),
                call(
                    [publish.sys.executable, str(root / publish.SCRIPT_RELATIVE_PATH),
                     "--repo", "mattpackages", "upload", str(artifact.resolve())],
                    root,
                ),
            ])

    def test_parse_build_metadata(self):
        metadata = publish.parse_build_metadata(
            "# generated\nBUILD_ARTIFACT_TYPE=deb\nBUILD_ARTIFACT_PATH=builds/pkg.deb\n"
        )
        self.assertEqual(metadata["BUILD_ARTIFACT_TYPE"], "deb")
        self.assertEqual(metadata["BUILD_ARTIFACT_PATH"], "builds/pkg.deb")

    def test_rejects_non_deb_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "BUILD_ARTIFACT_TYPE=deb"):
                publish.resolve_deb_artifact(
                    {"BUILD_ARTIFACT_TYPE": "rpm", "BUILD_ARTIFACT_PATH": "pkg.rpm"}, root
                )

    def test_rejects_missing_package_path(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "package path does not exist"):
                publish.resolve_deb_artifact(
                    {"BUILD_ARTIFACT_TYPE": "deb", "BUILD_ARTIFACT_PATH": "missing.deb"},
                    Path(directory),
                )

    def test_validates_downloaded_script_content(self):
        self.assertIn("print", publish.validate_script_content(b"print('repository manager')\n"))
        with self.assertRaisesRegex(ValueError, "empty"):
            publish.validate_script_content(b" \n")
        with self.assertRaisesRegex(ValueError, "valid UTF-8 Python"):
            publish.validate_script_content(b"not valid python !!!")

    def test_failed_download_does_not_reuse_cached_script(self):
        class FailedResponse:
            def __enter__(self):
                raise OSError("network unavailable")

            def __exit__(self, *args):
                return False

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "ManageMattOSRepository.py"
            target.write_text("old cached script", encoding="utf-8")

            opener = Mock(return_value=FailedResponse())
            with self.assertRaises(OSError):
                publish.download_latest_script(target, opener=opener)

            self.assertEqual(target.read_text(encoding="utf-8"), "old cached script")
            self.assertEqual(list(target.parent.glob("*.tmp")), [])
            request = opener.call_args.args[0]
            self.assertEqual(request.get_header("Cache-control"), "no-cache")


if __name__ == "__main__":
    unittest.main()
