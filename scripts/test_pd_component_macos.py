"""macOS x86_64 acceptance against the real packaged executable, with isolated user data.

Mirrors `scripts/test_pd_component.py` (Windows) so both platforms exercise the same
component contract: archive metadata, executable architecture, Chinese/space install
paths, concurrent and repeat launches, normal and abnormal stop, bounded timeouts,
protocol-version rejection, user-data independence and uninstall retention.

Only the transport and process primitives differ:

  named pipe (`control_pipe`)        -> Unix domain socket (`control_pipe`)
  PE machine word / import scan      -> Mach-O CPU type / `otool -L` dependency scan
  `TerminateProcess` after image check -> `SIGKILL` after `proc_pidpath` image check
  stalled `CreateNamedPipeW` server  -> stalled `AF_UNIX` listener that accepts and holds

The test runs the packaged binary directly with `OCTOPPET_DATA_DIR` pointed at a
private directory, so it never touches the user's real data or a running helper GUI.
"""
from __future__ import annotations

import argparse
import ctypes
import importlib.util
import json
import os
import platform
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import threading
import time
import unittest
import zipfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# CPU_TYPE_ARM64 from <mach/machine.h>; any value != CPU_TYPE_X86_64 exercises the
# architecture rejection, and this one is a real, recognizable Mach-O CPU type.
CPU_TYPE_ARM64 = 0x0100000C

spec = importlib.util.spec_from_file_location(
    "packager", Path(__file__).with_name("package_pd_component_macos.py")
)
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)
OPTIONS = None


def recv_exact(sock: socket.socket, count: int) -> bytes:
    """Read exactly `count` bytes from a socket or raise once it closes early."""
    data = b""
    while len(data) < count:
        chunk = sock.recv(count - len(data))
        if not chunk:
            raise EOFError("socket closed before the complete frame")
        data += chunk
    return data


class ComponentAcceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if platform.system() != "Darwin" or platform.machine() != "x86_64":
            raise unittest.SkipTest("macOS x86_64 runtime tests")
        cls.temporary = tempfile.TemporaryDirectory(prefix="paldee-pet-acceptance-")
        cls.root = Path(cls.temporary.name)
        cls.installation = cls.root / "组件 安装测试" / "versions" / "test-version"
        cls.user_data = cls.root / "独立 用户数据"
        cls.user_data.mkdir()
        cls.user_data.joinpath("retained.txt").write_text("retain-user-data", encoding="utf-8")
        cls.user_data.joinpath("config.json").write_text(
            json.dumps(
                {
                    "mascotId": "taiji-bot",
                    "shortcutOpenPet": "",
                    "shortcutOpenHome": "",
                    "username": "",
                }
            ),
            encoding="utf-8",
        )
        cls.manifest = json.loads(
            (OPTIONS.package_root / "latest-macos-x86_64.json").read_text(encoding="utf-8")
        )
        cls.catalog = json.loads(
            (OPTIONS.package_root / "catalog-entry-macos-x86_64.json").read_text(encoding="utf-8")
        )
        cls.archive = OPTIONS.package_root / cls.manifest["version"] / (
            f"paldee-pet-{cls.manifest['version']}-macos-x86_64.zip"
        )
        with zipfile.ZipFile(cls.archive) as bundle:
            bundle.extractall(cls.installation)
        # Archive permissions are a hint; the caller restores the executable bit.
        for name in (packager.ENTRY, packager.BRIDGE):
            (cls.installation / name).chmod(0o755)
        cls.exe = cls.installation / packager.ENTRY
        cls.env = dict(os.environ, OCTOPPET_DATA_DIR=str(cls.user_data))

    def invoke(self, command, expected=0, extra=(), env=None):
        result = subprocess.run(
            [str(self.exe), command, "--json", *extra],
            env=env or self.env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=22,
        )
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        value = json.loads(result.stdout)
        self.assertEqual(value["protocol"], packager.PROTOCOL)
        self.assertEqual(value["id"], packager.ID)
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
            subprocess.run(
                [str(cls.exe), "stop", "--json"],
                env=cls.env,
                capture_output=True,
                timeout=22,
            )
        cls.temporary.cleanup()

    def test_archive_metadata_architecture_and_allowlist(self):
        result = packager.validate_archive(self.archive, self.manifest, self.catalog)
        self.assertEqual(result["entry"], packager.ENTRY)
        with zipfile.ZipFile(self.archive) as bundle:
            metadata = json.loads(bundle.read("component.json"))
            self.assertIs(metadata["launchable"], True)
            self.assertEqual(metadata["settings"][0]["key"], "start_with_helper")
            self.assertIs(metadata["settings"][0]["default"], False)
            for name in bundle.namelist():
                self.assertTrue(
                    name
                    in [
                        "component.json",
                        "LICENSE",
                        packager.ENTRY,
                        packager.BRIDGE,
                    ]
                    or name.startswith("licenses/"),
                    name,
                )

    def test_chinese_space_path_and_repeat_launch(self):
        first = self.invoke("start")
        self.assertTrue(first["running"])
        self.assertTrue(os.path.samefile(first["data_dir"], self.user_data))
        self.assertEqual(self.invoke("start")["pid"], first["pid"])
        # The ordinary double-click path (no JSON args) is a no-op when already running.
        repeated = subprocess.run([str(self.exe)], env=self.env, capture_output=True, timeout=22)
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(self.invoke("status")["pid"], first["pid"])
        self.assertEqual(
            {p.name for p in self.installation.iterdir()},
            {"component.json", "LICENSE", packager.ENTRY, packager.BRIDGE, "licenses"},
        )

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
        pid = first["pid"]
        # Verify the image first so a recycled pid can never terminate the wrong process.
        libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
        libproc.proc_pidpath.restype = ctypes.c_int32
        libproc.proc_pidpath.argtypes = [ctypes.c_int32, ctypes.c_void_p, ctypes.c_uint32]
        buffer = ctypes.create_string_buffer(4096)
        length = libproc.proc_pidpath(pid, buffer, len(buffer))
        self.assertGreater(length, 0, "proc_pidpath resolved no image for the owned pid")
        self.assertEqual(
            Path(buffer.value.decode("utf-8", "replace")).resolve(),
            self.exe.resolve(),
        )
        os.kill(pid, signal.SIGKILL)
        deadline = time.monotonic() + 8
        while self.invoke("status")["running"] and time.monotonic() < deadline:
            time.sleep(0.1)
        self.assertFalse(self.invoke("status")["running"])
        self.assertTrue(self.invoke("start")["running"])

    def test_invalid_commands_and_not_running(self):
        self.assertEqual(self.invoke("shell", expected=2)["error"]["code"], "INVALID_ARGUMENT")
        self.assertEqual(self.invoke("show", expected=4)["error"]["code"], "NOT_RUNNING")
        self.assertEqual(self.invoke("hide", expected=4)["error"]["code"], "NOT_RUNNING")
        self.assertEqual(
            self.invoke("status", expected=2, extra=("--timeout-ms", "99999"))["error"]["code"],
            "INVALID_ARGUMENT",
        )

    def test_protocol_negotiation_refuses_wrong_version(self):
        value = self.invoke("start")
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            client.connect(value["control_pipe"])
            request = json.dumps(
                {"protocol": "octoppet-component@99", "command": "hide"}
            ).encode()
            client.sendall(struct.pack("<I", len(request)) + request)
            length = struct.unpack("<I", recv_exact(client, 4))[0]
            result = json.loads(recv_exact(client, length))
            client.sendall(b"\x01")  # ack the reply so the server completes the exchange
        finally:
            client.close()
        self.assertFalse(result["ok"])
        self.assertEqual(result["error"]["code"], "UNSUPPORTED_PROTOCOL")
        self.assertTrue(self.invoke("status")["visible"])

    def test_timeout_is_bounded_and_structured(self):
        name = self.invoke("status")["control_pipe"]
        try:
            os.unlink(name)
        except FileNotFoundError:
            pass
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(name)
        os.chmod(name, 0o600)
        server.listen(1)
        accepted = {"client": None}

        def stalled_test_server():
            try:
                client, _ = server.accept()
                accepted["client"] = client
                # Hold the connection open without replying; the client's bounded read
                # times out and surfaces a structured TIMEOUT well below the test budget.
                time.sleep(4)
            finally:
                if accepted["client"] is not None:
                    accepted["client"].close()
                server.close()
                try:
                    os.unlink(name)
                except FileNotFoundError:
                    pass

        thread = threading.Thread(target=stalled_test_server, daemon=True)
        thread.start()
        try:
            start = time.monotonic()
            value = self.invoke("status", expected=5, extra=("--timeout-ms", "200"))
            self.assertEqual(value["error"]["code"], "TIMEOUT")
            self.assertLess(time.monotonic() - start, 5)
        finally:
            thread.join(timeout=6)
        self.assertFalse(thread.is_alive())

    def test_user_directory_inside_component_is_rejected(self):
        managed_root = self.installation.parent.parent
        forbidden = [
            self.installation,
            managed_root,
            managed_root / "user-data",
            managed_root / "versions" / "0.1.0" / "user-data",
        ]
        for path in forbidden:
            existed = path.exists()
            value = self.invoke("status", expected=8, env=dict(self.env, OCTOPPET_DATA_DIR=str(path)))
            self.assertEqual(value["error"]["code"], "IO_ERROR")
            self.assertIn("outside the component", value["error"]["message"])
            if not existed:
                self.assertFalse(path.exists())

    def test_bad_metadata_size_hash_and_arch_are_rejected(self):
        for manifest in [
            dict(self.manifest, size=self.manifest["size"] + 1),
            dict(self.manifest, sha256="0" * 64),
        ]:
            with self.assertRaises(ValueError):
                packager.validate_archive(self.archive, manifest, self.catalog)
        with self.assertRaises(ValueError):
            packager.validate_archive(self.archive, self.manifest, dict(self.catalog, id="other"))
        # A thin Mach-O with a non-x86_64 CPU type must be rejected at packaging time.
        path = self.root / "arm64-fixture"
        data = bytearray(8)
        data[:4] = b"\xcf\xfa\xed\xfe"  # MH_MAGIC_64, little-endian
        struct.pack_into("<I", data, 4, CPU_TYPE_ARM64)
        path.write_bytes(data)
        self.assertEqual(packager.macho_cpu_type(path), CPU_TYPE_ARM64)
        with self.assertRaisesRegex(ValueError, "Only macOS x86_64 is supported"):
            packager.package(
                argparse.Namespace(
                    version=self.manifest["version"],
                    base_url="https://www.xiaozs.com/sah/components/paldee-pet",
                    pet=path,
                    bridge=self.installation / packager.BRIDGE,
                    bridge_root=self.installation,
                )
            )

    def test_zz_uninstall_preserves_independent_user_data(self):
        self.invoke("start")
        self.invoke("stop")
        managed_root = self.installation.parent.parent
        self.assertTrue(managed_root.resolve().is_relative_to(self.root.resolve()))
        shutil.rmtree(managed_root)
        self.assertFalse(managed_root.exists())
        self.assertEqual(
            self.user_data.joinpath("retained.txt").read_text(encoding="utf-8"),
            "retain-user-data",
        )
        self.assertTrue(self.user_data.joinpath("config.json").is_file())


def main():
    global OPTIONS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    OPTIONS = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ComponentAcceptance)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {
        "tests": result.testsRun,
        "failures": len(result.failures),
        "errors": len(result.errors),
        "skipped": len(result.skipped),
        "success": result.wasSuccessful(),
        "scope": "isolated macOS x86_64 package; Chinese/space installation path; no helper GUI integration",
    }
    OPTIONS.report.parent.mkdir(parents=True, exist_ok=True)
    OPTIONS.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    main()
