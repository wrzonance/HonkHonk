"""Exercise manifest installation across separate build-command sandboxes."""

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
APP_ID = "io.github.wrzonance.HonkHonk"


class FlatpakInstallTest(unittest.TestCase):
    def test_assets_install_when_each_command_has_a_fresh_tmp(self):
        manifest = ROOT / "packaging/flatpak" / f"{APP_ID}.yml"
        commands = yaml.safe_load(manifest.read_text())["modules"][0]["build-commands"]
        with tempfile.TemporaryDirectory() as directory:
            build = Path(directory) / "build"
            app = Path(directory) / "app"
            app.mkdir()
            shutil.copytree(ROOT / "assets", build / "assets")
            shutil.copytree(ROOT / "packaging/flatpak", build / "packaging/flatpak")
            # This test exercises installation; compilation has its own CI job.
            binary = build / "target/x86_64-unknown-linux-gnu/release/honkhonk"
            binary.parent.mkdir(parents=True)
            binary.write_text("fixture executable\n")
            original = (build / "assets/honkhonk.desktop").read_bytes()

            for command in commands:
                if command.startswith("cargo "):
                    continue
                result = subprocess.run(
                    [
                        "bwrap",
                        "--ro-bind", "/usr", "/usr", "--ro-bind", "/bin", "/bin",
                        "--ro-bind", "/lib", "/lib", "--ro-bind-try", "/lib64", "/lib64",
                        "--bind", str(build), "/build", "--bind", str(app), "/app",
                        "--tmpfs", "/tmp", "--chdir", "/build",
                        "/bin/sh", "-ec", command,
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 0, f"{command}\n{result.stderr}")

            desktop = app / "share/applications" / f"{APP_ID}.desktop"
            self.assertIn(f"Icon={APP_ID}\n", desktop.read_text())
            self.assertEqual((build / "assets/honkhonk.desktop").read_bytes(), original)
            self.assertTrue((app / f"share/icons/hicolor/64x64/apps/{APP_ID}.png").is_file())
            self.assertTrue((app / f"share/metainfo/{APP_ID}.metainfo.xml").is_file())


if __name__ == "__main__":
    unittest.main()
