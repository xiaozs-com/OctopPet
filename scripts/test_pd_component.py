"""Windows acceptance against the real packaged executable, with isolated user data."""
from __future__ import annotations
import argparse
import ctypes
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
import shutil
import struct
import subprocess
import tempfile
import threading
import time
import unittest
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("packager", Path(__file__).with_name("package_pd_component.py"))
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)
OPTIONS = None


def read_exact(file, count):
    data = b""
    while len(data) < count:
        chunk = file.read(count - len(data))
        if not chunk:
            raise EOFError("pipe closed before the complete frame")
        data += chunk
    return data


class ComponentAcceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.name != "nt":
            raise unittest.SkipTest("Windows x64 runtime tests")
        cls.temporary = tempfile.TemporaryDirectory(prefix="octoppet-acceptance-")
        cls.root = Path(cls.temporary.name)
        cls.installation = cls.root / "组件 安装测试" / "versions" / "test-version"
        cls.user_data = cls.root / "独立 用户数据"
        cls.user_data.mkdir()
        cls.user_data.joinpath("retained.txt").write_text("retain-user-data", encoding="utf-8")
        cls.user_data.joinpath("config.json").write_text(json.dumps({
            "mascotId": "taiji-bot", "shortcutOpenPet": "", "shortcutOpenHome": "", "username": "",
        }), encoding="utf-8")
        cls.manifest = json.loads((OPTIONS.package_root / "latest-windows-x64.json").read_text(encoding="utf-8"))
        cls.catalog = json.loads((OPTIONS.package_root / "catalog-entry.json").read_text(encoding="utf-8"))
        cls.archive = OPTIONS.package_root / cls.manifest["version"] / f"paldee-pet-{cls.manifest['version']}-windows-x64.zip"
        with zipfile.ZipFile(cls.archive) as bundle:
            bundle.extractall(cls.installation)
        cls.exe = cls.installation / "paldee-pet.exe"
        cls.env = dict(os.environ, OCTOPPET_DATA_DIR=str(cls.user_data))

    def invoke(self, command, expected=0, extra=(), env=None):
        result = subprocess.run([str(self.exe), command, "--json", *extra], env=env or self.env,
                                capture_output=True, text=True, encoding="utf-8", timeout=22)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        value = json.loads(result.stdout)
        self.assertEqual(value["protocol"], "octoppet-component@1")
        self.assertEqual(value["id"], "octoppet")
        self.assertEqual(value["version"], self.manifest["version"])
        return value

    def setUp(self):
        self.invoke("stop")

    def tearDown(self):
        if self.exe.exists():
            self.invoke("stop")

    @classmethod
    def tearDownClass(cls):
        if cls.exe.exists():
            subprocess.run([str(cls.exe), "stop", "--json"], env=cls.env, capture_output=True, timeout=22)
        cls.temporary.cleanup()

    def test_archive_metadata_architecture_and_allowlist(self):
        result = packager.validate_archive(self.archive, self.manifest, self.catalog)
        self.assertEqual(result["entry"], "paldee-pet.exe")
        with zipfile.ZipFile(self.archive) as bundle:
            metadata = json.loads(bundle.read("component.json"))
            self.assertIs(metadata["launchable"], True)
            self.assertEqual(metadata["settings"][0]["key"], "start_with_helper")
            self.assertIs(metadata["settings"][0]["default"], False)
        with zipfile.ZipFile(self.archive) as bundle:
            for name in bundle.namelist():
                self.assertTrue(name in ["component.json", "LICENSE", "paldee-pet.exe", "pd-device-bridge.exe"] or
                                name.startswith("licenses/"), name)

    def test_chinese_space_path_and_repeat_launch(self):
        first = self.invoke("start")
        self.assertTrue(first["running"])
        self.assertTrue(os.path.samefile(first["data_dir"], self.user_data))
        self.assertEqual(self.invoke("start")["pid"], first["pid"])
        # Test the ordinary double-click path as well as the JSON command.
        repeated = subprocess.run([str(self.exe)], env=self.env, capture_output=True, timeout=22)
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(self.invoke("status")["pid"], first["pid"])
        self.assertEqual({p.name for p in self.installation.iterdir()},
                         {"component.json", "LICENSE", "paldee-pet.exe", "pd-device-bridge.exe", "licenses"})

    def test_concurrent_start_has_one_owner(self):
        # Repeat cold starts to cover both contenders winning the ownership race.
        for attempt in range(5):
            with self.subTest(attempt=attempt):
                self.invoke("stop")
                with ThreadPoolExecutor(max_workers=2) as pool:
                    values = list(pool.map(lambda _: self.invoke("start"), range(2)))
                self.assertEqual(values[0]["pid"], values[1]["pid"])
                self.assertEqual(self.invoke("status")["pid"], values[0]["pid"])

    def test_start_and_status_queries_do_not_end_the_host(self):
        first = self.invoke("start")
        for attempt in range(10):
            with self.subTest(attempt=attempt), ThreadPoolExecutor(max_workers=3) as pool:
                values = list(pool.map(lambda command: self.invoke(command), ["start", "status", "status"]))
                self.assertTrue(all(value["running"] for value in values))
                self.assertEqual({value["pid"] for value in values}, {first["pid"]})
        self.assertEqual(self.invoke("status")["pid"], first["pid"])

    def test_show_hide_and_normal_stop(self):
        pid = self.invoke("start")["pid"]
        self.assertFalse(self.invoke("hide")["visible"])
        self.assertFalse(self.invoke("start")["visible"])
        self.assertTrue(self.invoke("show")["visible"])
        stopped = self.invoke("stop")
        self.assertFalse(stopped["running"])
        self.assertIsNone(stopped["pid"])
        self.assertFalse(self.invoke("stop")["running"])
        self.assertFalse(self.invoke("status")["running"])
        self.assertIsInstance(pid, int)

    def test_abnormal_exit_releases_ownership(self):
        first = self.invoke("start")
        # Only kill the process started in this test's private scope, after verifying its image.
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.restype = ctypes.c_void_p
        kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel.QueryFullProcessImageNameW.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_uint32)]
        kernel.TerminateProcess.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel.OpenProcess(0x101001, 0, first["pid"])
        self.assertTrue(handle)
        try:
            buffer = ctypes.create_unicode_buffer(32768)
            length = ctypes.c_uint32(len(buffer))
            self.assertTrue(kernel.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(length)))
            self.assertEqual(Path(buffer.value).resolve(), self.exe.resolve())
            self.assertTrue(kernel.TerminateProcess(handle, 99))
            self.assertEqual(kernel.WaitForSingleObject(handle, 5000), 0)
        finally:
            kernel.CloseHandle(handle)
        deadline = time.monotonic() + 8
        while self.invoke("status")["running"] and time.monotonic() < deadline:
            time.sleep(0.1)
        self.assertFalse(self.invoke("status")["running"])
        self.assertTrue(self.invoke("start")["running"])

    def test_invalid_commands_and_not_running(self):
        self.assertEqual(self.invoke("shell", expected=2)["error"]["code"], "INVALID_ARGUMENT")
        self.assertEqual(self.invoke("show", expected=4)["error"]["code"], "NOT_RUNNING")
        self.assertEqual(self.invoke("hide", expected=4)["error"]["code"], "NOT_RUNNING")
        self.assertEqual(self.invoke("status", expected=2, extra=("--timeout-ms", "99999"))["error"]["code"], "INVALID_ARGUMENT")

    def test_protocol_negotiation_refuses_wrong_version(self):
        value = self.invoke("start")
        with open(value["control_pipe"], "r+b", buffering=0) as file:
            request = json.dumps({"protocol": "octoppet-component@99", "command": "hide"}).encode()
            file.write(struct.pack("<I", len(request)) + request)
            length = struct.unpack("<I", read_exact(file, 4))[0]
            result = json.loads(read_exact(file, length))
            file.write(b"\x01")
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "UNSUPPORTED_PROTOCOL")
        self.assertTrue(self.invoke("status")["visible"])

    def test_timeout_is_bounded_and_structured(self):
        name = self.invoke("status")["control_pipe"]
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateNamedPipeW.restype = ctypes.c_void_p
        kernel.CreateNamedPipeW.argtypes = [ctypes.c_wchar_p, *([ctypes.c_uint32] * 6), ctypes.c_void_p]
        kernel.ConnectNamedPipe.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        kernel.DisconnectNamedPipe.argtypes = [ctypes.c_void_p]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel.CreateNamedPipeW(name, 0x00080003, 8, 1, 8192, 8192, 1000, None)
        self.assertNotEqual(handle, ctypes.c_void_p(-1).value)
        def stalled_test_server():
            try:
                kernel.ConnectNamedPipe(handle, None)
                time.sleep(1)
                kernel.DisconnectNamedPipe(handle)
            finally:
                kernel.CloseHandle(handle)
        thread = threading.Thread(target=stalled_test_server, daemon=True)
        thread.start()
        try:
            start = time.monotonic()
            value = self.invoke("status", expected=5, extra=("--timeout-ms", "200"))
            self.assertEqual(value["error"]["code"], "TIMEOUT")
            self.assertLess(time.monotonic() - start, 5)
        finally:
            thread.join(timeout=3)
        self.assertFalse(thread.is_alive())

    def test_user_directory_inside_component_is_rejected(self):
        managed_root = self.installation.parent.parent
        forbidden = [self.installation, managed_root, managed_root / "user-data",
                     managed_root / "versions" / "0.1.0" / "user-data"]
        for path in forbidden:
            existed = path.exists()
            value = self.invoke("status", expected=8, env=dict(self.env, OCTOPPET_DATA_DIR=str(path)))
            self.assertEqual(value["error"]["code"], "IO_ERROR")
            self.assertIn("outside the component", value["error"]["message"])
            if not existed:
                self.assertFalse(path.exists())

    def test_bad_metadata_size_hash_and_x86_are_rejected(self):
        for manifest in [dict(self.manifest, size=self.manifest["size"] + 1),
                         dict(self.manifest, sha256="0" * 64)]:
            with self.assertRaises(ValueError):
                packager.validate_archive(self.archive, manifest, self.catalog)
        with self.assertRaises(ValueError):
            packager.validate_archive(self.archive, self.manifest, dict(self.catalog, id="other"))
        path = self.root / "x86-fixture.exe"
        data = bytearray(70)
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 64)
        data[64:68] = b"PE\0\0"
        struct.pack_into("<H", data, 68, 0x14C)
        path.write_bytes(data)
        self.assertEqual(packager.pe_machine(path), 0x14C)
        with self.assertRaisesRegex(ValueError, "Only Windows x64"):
            packager.package(argparse.Namespace(version=self.manifest["version"],
                base_url="https://www.xiaozs.com/sah/components/paldee-pet", pet=path,
                bridge=self.installation / "pd-device-bridge.exe"))

    def test_zz_uninstall_preserves_independent_user_data(self):
        self.invoke("start")
        self.invoke("stop")
        managed_root = self.installation.parent.parent
        self.assertTrue(managed_root.resolve().is_relative_to(self.root.resolve()))
        shutil.rmtree(managed_root)
        self.assertFalse(managed_root.exists())
        self.assertEqual(self.user_data.joinpath("retained.txt").read_text(encoding="utf-8"), "retain-user-data")
        self.assertTrue(self.user_data.joinpath("config.json").is_file())


def main():
    global OPTIONS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    OPTIONS = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ComponentAcceptance)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {"tests": result.testsRun, "failures": len(result.failures), "errors": len(result.errors),
              "skipped": len(result.skipped), "success": result.wasSuccessful(),
              "scope": "isolated Windows x64 package; Chinese/space installation path; no helper GUI integration"}
    OPTIONS.report.parent.mkdir(parents=True, exist_ok=True)
    OPTIONS.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    main()
