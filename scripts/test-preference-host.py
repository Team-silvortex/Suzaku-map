#!/usr/bin/env python3
"""Default disk policy on a new private D-Bus/IBus session; no display or desktop keys.

Reuses only the existing inert fixture classes; their source/assertions remain
unchanged. --host and --tool must point to rebuilt binaries. The outer process
creates every path; the inner process validates isolation before GI or any write.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def require_owned(environment):
    root = Path(environment.get("XDG_RUNTIME_DIR", ""))
    assert environment.get("SUZAKU_PREFERENCE_NATIVE_QA") == "1"
    assert root.parent == Path("/tmp") and root.name.startswith("suzaku-preference-native.")
    assert root.is_dir() and not root.is_symlink()
    metadata = root.stat()
    assert metadata.st_uid == os.getuid() and stat.S_IMODE(metadata.st_mode) == 0o700
    assert not environment.get("DISPLAY") and not environment.get("WAYLAND_DISPLAY")
    assert environment.get("DBUS_SESSION_BUS_ADDRESS")
    assert environment.get("IBUS_ADDRESS") == f"unix:path={root}/ibus.sock"
    assert environment.get("SUZAKU_IME_CONFIG") == str(root / "ime.json")
    assert environment.get("SUZAKU_LINUX_IME_SOCKET") == str(root / "suzaku-ime/host.sock")
    for variable, directory in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                                ("XDG_CACHE_HOME", "cache"), ("XDG_STATE_HOME", "state")]:
        path = root / directory
        assert environment.get(variable) == str(path) and path.is_dir() and not path.is_symlink()
        assert path.stat().st_uid == os.getuid()
    assert environment.get("GSETTINGS_BACKEND") == "memory" and environment.get("GIO_USE_VFS") == "local"
    for variable in ["LD_PRELOAD", "SUZAKU_PREFERENCE_MODE", "SUZAKU_NATIVE_SYNC_QA", "SUZAKU_LEXICON_DIR"]:
        assert not environment.get(variable), f"ordinary production defaults required: {variable}"
    for name in ["ime.json", "ibus.sock", "suzaku-ime", "candidate-preferences"]:
        assert not (root / name).exists() and not (root / name).is_symlink()
    return root


def inner(host, tool):
    root = require_owned(os.environ)
    spec = importlib.util.spec_from_file_location("preference_owned_fixture", HERE / "test-custom-shortcuts-ibus.py")
    qa = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(qa)
    qa.require_private_compose(root, os.environ)
    import gi
    gi.require_version("IBus", "1.0")
    from gi.repository import IBus, GLib, Gio
    fixture = qa.Fixture(root, root / "ime.json", host, IBus, GLib, Gio)
    file = root / "candidate-preferences/preferences-v1.json"
    library = root / "preference-fsync.so"
    subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                    str(HERE / "fixtures/preference-fsync.c"), "-ldl", "-o", str(library)],
                   check=True, timeout=15)
    host_env = dict(os.environ, LD_PRELOAD=str(library))
    watch = peer = None
    host_logs = {}

    def status():
        result = json.loads(fixture.command("S"))
        assert result["ok"]
        return result["preferences"]

    def settled():
        fixture.wait(lambda: not status()["pending"], "preference writer completion", timeout=2)
        assert status()["error"] is None, status()

    def start_peer():
        nonlocal watch, peer
        watch, peer = qa.Watch(fixture), qa.EnginePeer(fixture)
        peer.event("FocusIn")
        fixture.wait(lambda: watch.latest is not None and watch.latest["focused"], "owned preference input focus")

    def restart(corrupt=False, normal_path=False, mode=None, release_save=False, shutdown_warning=None):
        nonlocal watch, peer
        if peer is not None:
            peer.close()
            watch.close()
            peer = watch = None
        old = next(process for process in fixture.processes if process.args[0] == str(host))
        old.terminate()
        if release_save:
            fixture.wait(lambda: not fixture.socket.exists(), "input loop stopped before held-save shutdown", timeout=0.5)
            assert old.poll() is None, "host exited instead of waiting for its pending save"
            release("save")
        assert old.wait(timeout=2) == 0, "SIGTERM did not return through graceful host shutdown"
        fixture.processes.remove(old)
        assert not fixture.socket.exists(), "old host retained its socket"
        if shutdown_warning is not None:
            log = host_logs[old].read_text()
            assert ("preference shutdown was not confirmed" in log) == shutdown_warning, log
        if corrupt:
            file.write_bytes(b"corrupt synthetic preference document")
            file.chmod(0o600)
        environment = dict(host_env)
        if normal_path:
            environment.pop("SUZAKU_IME_CONFIG")
        if mode is not None:
            environment["SUZAKU_PREFERENCE_MODE"] = mode
        log = root / f"host-{len(host_logs)}.log"
        with log.open("x") as stderr:
            process = subprocess.Popen([str(host)], stdout=subprocess.DEVNULL, stderr=stderr, env=environment)
        host_logs[process] = log
        fixture.processes.append(process)
        fixture.wait(fixture.socket.exists, "owned restarted preference host")
        start_peer()

    def choose(language, reading, text):
        fixture.control("L" + language)
        peer.reset(watch, reading)
        index = next(index for index, row in enumerate(watch.latest["candidates"]) if row["text"] == text)
        peer.select(watch, index)
        before = len(peer.commits)
        assert peer.key(IBus.KEY_Return, IBus.ModifierType.SHIFT_MASK)
        fixture.wait(lambda: watch.latest["seed"] == text, "preference deliberate editable adoption")
        assert len(peer.commits) == before
        assert peer.key(IBus.KEY_Return)
        fixture.wait(lambda: peer.commits[before:] == [text] and not watch.latest["seed"], "preference exact single commit")

    def clear():
        result = subprocess.run([str(tool), "preferences", "clear"], capture_output=True,
                                text=True, timeout=2, env=os.environ)
        assert result.returncode == 0, result.stdout + result.stderr
        cleared = json.loads(result.stdout)
        assert cleared["ok"] and cleared["enabled"] and not cleared["memory_only"]
        assert not file.exists()
        assert status()["clear"]["state"] == "complete"

    def arm(kind, fail=False):
        for extension in ["arm", "entered", "release", "finished", "fail"]:
            (root / f"preference-{kind}.{extension}").unlink(missing_ok=True)
        if fail:
            (root / f"preference-{kind}.fail").touch(mode=0o600)
        (root / f"preference-{kind}.arm").touch(mode=0o600)

    def release(kind):
        (root / f"preference-{kind}.release").touch(mode=0o600)
        fixture.wait(lambda: (root / f"preference-{kind}.finished").exists(), "actual preference fsync gate released")

    try:
        fixture.start()
        assert status()["enabled"] and not status()["memory_only"] and not file.exists()
        # Restart once with the isolated fsync library; production mode remains
        # unset, proving the actual default, not an explicit disk override.
        restart()
        before_epoch = status()["epoch"]
        for language, reading, text in [("en", "hel", "help"), ("zh-Hans", "shijian", "实践")]:
            for _ in range(8):
                choose(language, reading, text)
        settled()
        assert status()["entries"] == 2
        raw = file.read_text()
        document = json.loads(raw)
        assert len(document["entries"]) == 2 and len(document["fingerprint_key_hex"]) == 64
        assert len(raw.encode()) < 512 * 1024
        assert not any(text in raw for text in ["hel", "help", "shijian", "实践", "zh-Hans", "pending", "undo"])
        assert stat.S_IMODE(file.stat().st_mode) == 0o600
        assert stat.S_IMODE(file.parent.stat().st_mode) == 0o700
        config = fixture.config.read_bytes()
        restart()
        assert status()["epoch"] != before_epoch and status()["entries"] == 2
        for language, reading, text in [("en", "hel", "help"), ("zh-Hans", "shijian", "实践")]:
            fixture.control("L" + language)
            peer.reset(watch, reading)
            words = [row["text"] for row in watch.latest["candidates"] if row["kind"] == "word"]
            assert words[0] == text, (language, words)
            assert not peer.commits
        print("PASS: default native persistence, two-language confirmed choices, private format, real SIGTERM/restart and restored ranking", flush=True)

        # A global clear must preserve the current native draft/choice/undo.
        index = next(i for i, row in enumerate(watch.latest["candidates"]) if row["text"] == "实践")
        peer.select(watch, index)
        assert peer.key(IBus.KEY_Return, IBus.ModifierType.SHIFT_MASK)
        fixture.wait(lambda: watch.latest["seed"] == "实践", "live adoption before disk clear")
        before = fixture.fresh()
        config = fixture.config.read_bytes()
        clear()
        assert fixture.fresh() == before and fixture.config.read_bytes() == config
        assert peer.key(IBus.KEY_BackSpace)
        fixture.wait(lambda: watch.latest["seed"] == "shijian", "disk clear preserves exact adoption undo")
        assert status()["entries"] == 0
        restart()
        peer.reset(watch, "shijian")
        assert watch.latest["candidates"][0]["text"] == "时间"
        assert not file.exists() and status()["entries"] == 0
        print("PASS: CLI/tray clear receipt deletes RAM + file, preserves live native undo and does not revive on restart", flush=True)

        arm("save")
        choose("en", "hel", "help")
        fixture.wait(lambda: (root / "preference-save.entered").exists(), "actual staging fsync blocked")
        started = time.monotonic()
        peer.reset(watch, "still responsive")
        assert time.monotonic() - started < 0.5, "input waited on preference file sync"
        request = json.loads(fixture.command("F"))
        assert not request["ok"] and request["preference_clear_accepted"]
        assert status()["clear"]["state"] == "pending"
        release("save")
        fixture.wait(lambda: status()["clear"]["state"] == "complete", "clear after canceled old staged save")
        assert not file.exists() and status()["entries"] == 0
        assert not list(file.parent.glob(".suzaku-*.tmp"))
        assert watch.latest["seed"] == "still responsive"
        print("PASS: actual blocked save fsync leaves input responsive; clear cancels staged write, removes temporary file and prevents resurrection", flush=True)

        choose("en", "hel", "help")
        settled()
        arm("clear")
        operation = subprocess.Popen([str(tool), "preferences", "clear"], stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, env=os.environ)
        fixture.wait(lambda: (root / "preference-clear.entered").exists(), "actual parent directory sync blocked")
        peer.reset(watch, "live during clear")
        stdout, stderr = operation.communicate(timeout=2)
        assert operation.returncode != 0 and "未获确认" in stderr, stdout + stderr
        assert status()["clear"]["state"] == "pending", "unsynced deletion was reported as completed"
        release("clear")
        fixture.wait(lambda: status()["clear"]["state"] == "complete", "eventual durable explicit clear receipt")
        assert not file.exists() and watch.latest["seed"] == "live during clear"
        print("PASS: real clear sync stays pending; CLI keeps its original 700ms deadline and never reports optimistic success", flush=True)

        arm("save", fail=True)
        choose("en", "hel", "help")
        fixture.wait(lambda: (root / "preference-save.entered").exists(), "failed preference fsync entered")
        release("save")
        fixture.wait(lambda: not status()["pending"] and status()["error"] is not None, "safe persistence error status")
        assert not file.exists()
        assert not list(file.parent.glob(".suzaku-*.tmp"))
        peer.reset(watch, "typing still works")
        assert watch.latest["seed"] == "typing still works"
        clear()
        choose("en", "hel", "help")
        settled()
        before = file.read_bytes()
        peer.reset(watch)
        peer.content(IBus.InputPurpose.PASSWORD)
        assert not peer.key(IBus.KEY_h)
        fixture.wait(lambda: watch.latest["private"] and not watch.latest["seed"], "private context redacted")
        assert file.read_bytes() == before and status()["entries"] == 1
        clear()  # Explicit global removal is allowed even while private.
        peer.content(IBus.InputPurpose.FREE_FORM)
        peer.reset(watch, "hel")
        assert status()["entries"] == 0
        print("PASS: injected file failure preserves input; private input never changes the file and explicit private-context global clear works", flush=True)

        restart(corrupt=True)
        assert status()["memory_only"] and status()["error"] is not None
        peer.reset(watch, "hel")
        assert file.read_bytes() == b"corrupt synthetic preference document"
        clear()
        choose("en", "hel", "help")
        settled()
        assert json.loads(file.read_text())["entries"]
        print("PASS: corrupt startup falls back to working RAM input, preserves original bytes, and explicit clear safely enables new learning", flush=True)

        clear()
        arm("save")
        for _ in range(8):
            choose("en", "hel", "help")
        fixture.wait(lambda: (root / "preference-save.entered").exists(), "pending native save before graceful shutdown")
        restart(release_save=True, shutdown_warning=False)
        assert status()["entries"] == 1 and not list(file.parent.glob(".suzaku-*.tmp"))
        saved = json.loads(file.read_text())["entries"]
        assert len(saved) == 1 and 7.9 < saved[0]["frequency"] <= 8, "shutdown lost later coalesced choices"
        peer.reset(watch, "hel")
        assert next(row["text"] for row in watch.latest["candidates"] if row["kind"] == "word") == "help"
        baseline = file.read_bytes()
        arm("save")
        choose("zh-Hans", "shijian", "实践")
        fixture.wait(lambda: (root / "preference-save.entered").exists(), "pending native save beyond bounded shutdown")
        assert file.read_bytes() == baseline and list(file.parent.glob(".suzaku-*.tmp"))
        restart(shutdown_warning=True)
        assert status()["entries"] == 1 and file.read_bytes() == baseline
        leftovers = list(file.parent.glob(".suzaku-*.tmp"))
        print(f"OBSERVE: timed-out native shutdown preserved the committed cache; leftover staged files after restart={len(leftovers)}", flush=True)
        assert not leftovers, "timed-out preference save retained sensitive staged data after restart"
        clear()
        assert not list(file.parent.glob(".suzaku-*.tmp"))
        choose("en", "hel", "help")
        settled()
        print("PASS: pending SIGTERM flushes the latest coalesced choice within its bound; timed-out shutdown reports unconfirmed and restart removes only abandoned preference staging", flush=True)

        # No config override: the ordinary state-home path is isolated too, and
        # must not silently import or modify the earlier config-scoped cache.
        isolated_file, isolated_bytes = file, file.read_bytes()
        restart(normal_path=True)
        file = root / "state/suzaku-ime/preferences-v1.json"
        assert status()["enabled"] and not status()["memory_only"]
        assert status()["entries"] == 0 and not file.exists()
        choose("en", "hel", "help")
        settled()
        assert file.is_file() and isolated_file.read_bytes() == isolated_bytes
        assert stat.S_IMODE(file.stat().st_mode) == 0o600
        clear()
        assert isolated_file.read_bytes() == isolated_bytes
        print("PASS: ordinary XDG_STATE_HOME default and explicit-config namespace remain separate; clear affects only the host-owned namespace", flush=True)

        choose("en", "hel", "help")
        settled()
        disk_bytes = file.read_bytes()
        restart(normal_path=True, mode="memory")
        assert not status()["enabled"] and status()["memory_only"] and status()["entries"] == 0
        choose("en", "hel", "help")
        assert status()["entries"] == 1 and file.read_bytes() == disk_bytes
        result = subprocess.run([str(tool), "preferences", "clear"], capture_output=True,
                                text=True, timeout=2, env=os.environ)
        assert result.returncode == 0, result.stdout + result.stderr
        acknowledged = json.loads(result.stdout)
        assert acknowledged["memory_only"] and not acknowledged["enabled"] and acknowledged["entries"] == 0
        assert file.read_bytes() == disk_bytes
        print("PASS: explicit memory-only opt-out does not load, overwrite or erase existing disk preferences and preserves legacy clear semantics", flush=True)
    finally:
        # Always release only these owned barriers before graceful teardown.
        for kind in ["save", "clear"]:
            (root / f"preference-{kind}.release").touch(mode=0o600, exist_ok=True)
        fixture.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--tool", type=Path, required=True)
    parser.add_argument("--inside", action="store_true")
    arguments = parser.parse_args()
    host, tool = arguments.host.resolve(strict=True), arguments.tool.resolve(strict=True)
    assert host.is_file() and os.access(host, os.X_OK)
    assert tool.is_file() and os.access(tool, os.X_OK)
    if arguments.inside:
        inner(host, tool)
        return
    with tempfile.TemporaryDirectory(prefix="suzaku-preference-native.", dir="/tmp") as temporary:
        root = Path(temporary)
        root.chmod(0o700)
        environment = dict(os.environ)
        for variable in ["DISPLAY", "WAYLAND_DISPLAY", "LD_PRELOAD", "SUZAKU_PREFERENCE_MODE",
                         "SUZAKU_NATIVE_SYNC_QA", "SUZAKU_LEXICON_DIR"]:
            environment.pop(variable, None)
        for variable, directory in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                                    ("XDG_CACHE_HOME", "cache"), ("XDG_STATE_HOME", "state")]:
            (root / directory).mkdir(mode=0o700)
            environment[variable] = str(root / directory)
        shutil.copyfile(HERE / "fixtures/compose.XCompose", root / "compose.XCompose")
        (root / "compose.XCompose").chmod(0o600)
        environment.update(XDG_RUNTIME_DIR=str(root), SUZAKU_PREFERENCE_NATIVE_QA="1",
            SUZAKU_IME_CONFIG=str(root / "ime.json"), SUZAKU_LINUX_IME_SOCKET=str(root / "suzaku-ime/host.sock"),
            IBUS_ADDRESS=f"unix:path={root}/ibus.sock", XCOMPOSEFILE=str(root / "compose.XCompose"),
            XLOCALEDIR="/usr/share/X11/locale", GSETTINGS_BACKEND="memory", GIO_USE_VFS="local")
        subprocess.run(["timeout", "--kill-after=3s", "120s", "dbus-run-session", "--", "/usr/bin/python3",
                        "-B", "-u", str(Path(__file__).resolve()), "--inside", "--host", str(host), "--tool", str(tool)],
                       env=environment, check=True, timeout=125)


if __name__ == "__main__":
    main()
