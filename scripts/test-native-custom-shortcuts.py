#!/usr/bin/env python3
"""Test the real C shortcut adapter with stubs, never a desktop or model.

--structural-only does not compile. The default additionally needs cc, pkg-config,
GLib/GIO/xkbcommon development files, IBus headers discovered as in build.rs and
the libibus-1.0.so.5 runtime. It compiles only the extracted adapter functions.
Rust settings and private IBus end-to-end acceptance are separate gates.
"""
import argparse
import hashlib
import os
from pathlib import Path
import runpy
import shlex
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "src/linux/ibus_engine_bridge.c"
LANGUAGE = ROOT / "src/linux/ibus_language.inc.c"
IPC = ROOT / "src/linux/ibus_ipc.inc.c"


def locate_ibus_include(environment):
    """Match build.rs, including the header override and locally staged headers."""
    candidates = []
    if environment.get("SUZAKU_IBUS_INCLUDE_DIR"):
        candidates.append(Path(environment["SUZAKU_IBUS_INCLUDE_DIR"]))
    candidates.extend([Path("/usr/include/ibus-1.0"), ROOT / "target/ibus-dev/usr/include/ibus-1.0"])
    for candidate in candidates:
        if (candidate / "ibus.h").is_file():
            return candidate
    raise RuntimeError("IBus headers not found; use the project's existing SUZAKU_IBUS_INCLUDE_DIR override")


def function(source, signature):
    if source.count(signature) != 1:
        raise AssertionError(f"expected exactly one {signature!r}")
    start = source.index(signature)
    end = source.index("\n}\n", start) + len("\n}\n")
    return source[start:end]


class ShortcutStructureTests(unittest.TestCase):
    def test_private_ibus_fixture_preflight_never_starts_processes(self):
        checks = runpy.run_path(str(ROOT / "scripts/test-custom-shortcuts-ibus.py"))
        require = checks["require_fixture"]
        with tempfile.TemporaryDirectory(prefix="suzaku-custom-shortcuts.", dir="/tmp") as directory:
            root = Path(directory)
            for name in ["config", "data", "cache", "bin"]:
                (root / name).mkdir(mode=0o700)
            (root / "compose.XCompose").write_text("# owned guard fixture\n")
            host = root / "bin/linux_ime_host"
            host.write_text("# not executed by a preflight\n")
            host.chmod(0o700)
            environment = {
                "SUZAKU_CUSTOM_SHORTCUT_QA": "1", "XDG_RUNTIME_DIR": str(root),
                "IBUS_ADDRESS": f"unix:path={root}/ibus.sock",
                "DBUS_SESSION_BUS_ADDRESS": "unix:path=/tmp/private-unit-test-only",
                "SUZAKU_LINUX_IME_SOCKET": str(root / "suzaku-ime/host.sock"),
                "SUZAKU_IME_CONFIG": str(root / "ime.json"),
                "XDG_CONFIG_HOME": str(root / "config"), "XDG_DATA_HOME": str(root / "data"),
                "XDG_CACHE_HOME": str(root / "cache"), "XCOMPOSEFILE": str(root / "compose.XCompose"),
                "GSETTINGS_BACKEND": "memory", "GIO_USE_VFS": "local",
                "SUZAKU_NATIVE_QA_BIN_DIR": str(root / "bin"),
            }
            self.assertEqual(require(environment), (root, root / "ime.json", host))
            self.assertFalse((root / "ime.json").exists())
            self.assertFalse((root / "ibus.sock").exists())
            for key, value in [
                ("SUZAKU_CUSTOM_SHORTCUT_QA", ""), ("DBUS_SESSION_BUS_ADDRESS", ""),
                ("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-0"),
                ("IBUS_ADDRESS", "unix:path=/tmp/shared-ibus.sock"),
                ("SUZAKU_IME_CONFIG", "/tmp/shared-ime.json"),
                ("SUZAKU_LINUX_IME_SOCKET", "/tmp/shared-host.sock"),
                ("XDG_CONFIG_HOME", "/tmp"), ("GSETTINGS_BACKEND", "dconf"),
                ("GIO_USE_VFS", "gvfs"), ("LD_PRELOAD", "/tmp/injected.so"),
                ("SUZAKU_IBUS_INLINE_PREEDIT", "1"), ("SUZAKU_LEXICON_DIR", "/tmp/personal"),
                ("SUZAKU_NATIVE_QA_BIN_DIR", ""), ("XCOMPOSEFILE", "/tmp/shared-Compose"),
            ]:
                with self.subTest(key=key), self.assertRaises(ValueError):
                    require(dict(environment, **{key: value}))
            (root / "ime.json").write_text("{}")
            with self.assertRaises(ValueError):
                require(environment)

    def test_header_override_matches_the_project_without_ibus_pkgconfig(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-shortcut-headers.", dir="/tmp") as directory:
            include = Path(directory)
            (include / "ibus.h").write_text("/* test header discovery only */\n")
            self.assertEqual(locate_ibus_include({"SUZAKU_IBUS_INCLUDE_DIR": str(include)}), include)

    def test_only_u_has_the_extended_bounded_control_payload(self):
        source = SOURCE.read_text()
        body = function(source, "static gboolean suzaku_ibus_is_control_request(")
        self.assertIn("bytes_read < 128 || (request[0] == 'U' && bytes_read <= 1024)", body)
        self.assertIn("g_utf8_validate(request, (gssize)bytes_read, NULL)", body)
        dispatch = function(source, "static void suzaku_ibus_ipc_dispatch(")
        self.assertEqual(dispatch.count("suzaku_ibus_is_control_request(request, bytes_read)"), 1)
        self.assertIn("#define SUZAKU_IPC_MAX_REQUEST 65535", IPC.read_text())
        self.assertIn("client->input->len + (gsize)received > SUZAKU_IPC_MAX_REQUEST", IPC.read_text())

    def test_unified_interface_replaces_fixed_language_and_alt_paths(self):
        source = SOURCE.read_text()
        body = function(source, "static gboolean suzaku_ibus_engine_process_key_event(")
        self.assertIn("suzaku_host_ime_shortcut(uint32_t key, uint32_t modifiers)", source)
        self.assertEqual(body.count("suzaku_ibus_shortcut_action(keyval, state)"), 1)
        self.assertNotIn("suzaku_host_ime_alt_shortcut", source)
        self.assertLess(body.index("suzaku_ibus_shortcut_action(keyval, state)"),
                        body.index("if ((state & SUZAKU_SYSTEM_MODIFIERS) != 0)"))
        self.assertIn("keyval == IBUS_KEY_Alt_L || keyval == IBUS_KEY_Alt_R", body)
        self.assertIn("if (!self->private_input)", body)
        self.assertIn("shortcut != 0 && suzaku_ibus_engine_is_composing(self)", body)
        self.assertIn("suzaku_host_ime_selected_index() < suzaku_host_ime_candidate_count()", body)

    def test_async_language_guards_and_deadlines_are_not_changed(self):
        source = LANGUAGE.read_text()
        self.assertNotIn("Ctrl+Shift+Space", source)
        self.assertIn("已切换输入语言，草稿保留", source)
        normalized = source.replace("已切换输入语言，草稿保留", "已切换输入语言，草稿保留 · Ctrl+Shift+Space")
        # Freeze the complete pre-change asynchronous owner/deadline/write code.
        self.assertEqual(hashlib.sha256(normalized.encode()).hexdigest(),
                         "8b4e3ce31d22dcd64f0baf82942d4ca6170bfac7658ab5877bed610e75204f42")


STUBS = r'''
#include <ibus.h>
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define SUZAKU_LOOKUP_PAGE_SIZE 6
#define SUZAKU_SYSTEM_MODIFIERS \
 (IBUS_CONTROL_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK | IBUS_HYPER_MASK | IBUS_META_MASK)
#define CHECK(condition) do { if (!(condition)) { \
 fprintf(stderr, "shortcut contract failed at line %d: %s\n", __LINE__, #condition); return 1; } } while (0)
typedef struct {
 GString *input;
 gchar *completion_undo;
 GHashTable *held_adoption_keys;
 gchar *language_feedback;
 gpointer language_job;
 gboolean bypass_input, private_input, focused, composing;
} SuzakuIBusEngine;
static guint configured_key, configured_modifiers, configured_action;
static guint ffi_calls, last_key, last_modifiers, last_action, actions, commits;
static guint full_renders, presentation_renders;
static size_t count = 12, selected = 2;
static guint language = 2;
static gboolean home_row;
static unsigned int suzaku_host_ime_shortcut(uint32_t key, uint32_t modifiers) {
 ffi_calls++; last_key = key; last_modifiers = modifiers;
 if (configured_action && g_unichar_tolower(key) == configured_key && modifiers == configured_modifiers)
   return configured_action;
 if (!configured_action && key == 32 && modifiers == 5) return 6;
 if (home_row && modifiers == 2) {
   switch (g_unichar_tolower(key)) {
   case 'k': return 1; case 'j': return 2; case 'h': return 3; case 'l': return 4; case ';': return 5;
   }
 }
 return 0;
}
static gboolean suzaku_ibus_engine_is_focused(IBusEngine *engine) { return ((SuzakuIBusEngine *)engine)->focused; }
static gboolean suzaku_ibus_engine_is_composing(SuzakuIBusEngine *self) { return self->composing; }
static void G_GNUC_UNUSED suzaku_ibus_engine_render(SuzakuIBusEngine *self) { (void)self; full_renders++; }
static void suzaku_ibus_engine_render_presentation(SuzakuIBusEngine *self) { (void)self; presentation_renders++; }
static void suzaku_ibus_engine_cancel_compose(SuzakuIBusEngine *self) { self->composing = FALSE; }
static void suzaku_ibus_engine_sync_input(SuzakuIBusEngine *self) { g_clear_pointer(&self->completion_undo, g_free); }
static void suzaku_ibus_engine_delete_word(SuzakuIBusEngine *self) { g_string_truncate(self->input, 0); }
static void suzaku_ibus_engine_move_selection(SuzakuIBusEngine *self, ptrdiff_t delta) {
 (void)self; last_action = delta < 0 ? 1 : 2; actions++;
}
static void suzaku_ibus_engine_change_page(SuzakuIBusEngine *self, gboolean forward) {
 (void)self; last_action = forward ? 4 : 3; actions++;
}
static gboolean suzaku_ibus_engine_complete(SuzakuIBusEngine *self, gboolean append_space) {
 if (append_space) { g_string_append_c(self->input, ' '); return TRUE; }
 last_action = 5; actions++;
 g_clear_pointer(&self->completion_undo, g_free);
 self->completion_undo = g_strdup(self->input->str);
 g_string_assign(self->input, "hello"); return TRUE;
}
static void suzaku_ibus_start_language_change(SuzakuIBusEngine *self) { (void)self; last_action = 6; actions++; }
static void suzaku_ibus_cancel_language_change(SuzakuIBusEngine *self) { (void)self; }
static size_t suzaku_host_ime_candidate_count(void) { return count; }
static size_t suzaku_host_ime_selected_index(void) { return selected; }
static size_t suzaku_ibus_candidate_page_start(void) { return selected / 6 * 6; }
static void suzaku_host_ime_select_candidate(size_t index) { selected = index; }
static unsigned int suzaku_host_ime_language_kind(void) { return language; }
static void suzaku_ibus_engine_append_character(SuzakuIBusEngine *self, gunichar key) { g_string_append_unichar(self->input, key); }
static gboolean suzaku_ibus_engine_process_compose(SuzakuIBusEngine *self, guint key) { (void)key; return self->composing; }
static void suzaku_ibus_engine_end_input(SuzakuIBusEngine *self) { g_string_truncate(self->input, 0); }
static gboolean suzaku_ibus_engine_commit(SuzakuIBusEngine *self, gboolean companion) {
 (void)companion; commits++; g_string_truncate(self->input, 0); return TRUE;
}
'''


MAIN = r'''
static void reset_fixture(SuzakuIBusEngine *self) {
 g_string_assign(self->input, "hel"); g_clear_pointer(&self->completion_undo, g_free);
 g_clear_pointer(&self->language_feedback, g_free); g_hash_table_remove_all(self->held_adoption_keys);
 self->language_job = NULL;
 self->focused = TRUE; self->composing = self->private_input = self->bypass_input = FALSE;
 configured_key = 'q'; configured_modifiers = 1; configured_action = 2;
 ffi_calls = last_key = last_modifiers = last_action = actions = commits = 0;
 full_renders = presentation_renders = 0;
 count = 12; selected = 2; language = 2; home_row = FALSE;
}
static gboolean event(SuzakuIBusEngine *self, guint key, guint code, guint state) {
 return suzaku_ibus_engine_process_key_event((IBusEngine *)self, key, code, state);
}
int main(int argc, char **argv) {
 CHECK(argc == 2);
 SuzakuIBusEngine value = {0}, *self = &value;
 self->input = g_string_new("hel"); self->held_adoption_keys = g_hash_table_new(g_direct_hash, g_direct_equal);
 if (strcmp(argv[1], "actions") == 0) {
   for (guint action = 1; action <= 6; action++) {
     reset_fixture(self); configured_action = action;
     CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK));
     CHECK(last_action == action && actions == 1 && ffi_calls == 1 && commits == 0);
     CHECK(last_key == 'q' && last_modifiers == 1);
   }
 } else if (strcmp(argv[1], "modifiers") == 0) {
   guint states[] = {IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK,
                     IBUS_MOD1_MASK | IBUS_SHIFT_MASK};
   guint logical[] = {1, 2, 5, 6};
   for (size_t i = 0; i < G_N_ELEMENTS(states); i++) {
     reset_fixture(self); configured_modifiers = logical[i];
     CHECK(event(self, IBUS_KEY_Q, 24, states[i] | IBUS_LOCK_MASK | IBUS_MOD2_MASK));
     CHECK(last_key == 'Q' && last_modifiers == logical[i] && ffi_calls == 1 && actions == 1);
   }
   guint forbidden[] = {IBUS_MOD3_MASK, IBUS_MOD4_MASK, IBUS_MOD5_MASK, IBUS_SUPER_MASK,
                         IBUS_HYPER_MASK, IBUS_META_MASK, IBUS_BUTTON1_MASK, 1u << 27};
   for (size_t i = 0; i < G_N_ELEMENTS(forbidden); i++) {
     reset_fixture(self);
     CHECK(!event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK | forbidden[i]));
     CHECK(ffi_calls == 0 && actions == 0 && commits == 0);
   }
   reset_fixture(self);
   CHECK(!event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK | IBUS_MOD1_MASK));
   CHECK(ffi_calls == 0 && actions == 0);
 } else if (strcmp(argv[1], "guards") == 0) {
   for (guint guard = 0; guard < 6; guard++) {
     reset_fixture(self);
     switch (guard) {
     case 0: self->focused = FALSE; break; case 1: self->bypass_input = TRUE; break;
     case 2: self->private_input = TRUE; break; case 3: self->composing = TRUE; break;
     case 4: g_string_truncate(self->input, 0); break; case 5: count = 0; break;
     }
     CHECK(!event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK));
     CHECK(actions == 0 && commits == 0);
     CHECK(ffi_calls == (guard < 3 ? 0 : 1));
     if (guard == 3) { CHECK(self->composing); }
   }
   reset_fixture(self); selected = count;
   CHECK(!event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(actions == 0);
   reset_fixture(self); g_string_truncate(self->input, 0); configured_action = 6;
   CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(last_action == 6 && commits == 0);
 } else if (strcmp(argv[1], "held") == 0) {
   for (guint action = 5; action <= 6; action++) {
     reset_fixture(self); configured_action = action;
     CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(actions == 1);
     CHECK(event(self, IBUS_KEY_q, 24, 0)); CHECK(actions == 1 && ffi_calls == 1 && commits == 0);
     CHECK(!event(self, IBUS_KEY_q, 24, IBUS_RELEASE_MASK));
     CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(actions == 2);
     reset_fixture(self); configured_action = action;
     CHECK(event(self, IBUS_KEY_q, 0, IBUS_CONTROL_MASK));
     CHECK(event(self, IBUS_KEY_q, 0, IBUS_CONTROL_MASK)); CHECK(actions == 2);
   }
   reset_fixture(self);
   CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK));
   CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(actions == 2);
 } else if (strcmp(argv[1], "compose-modifiers") == 0) {
   guint keys[] = {IBUS_KEY_Control_L, IBUS_KEY_Control_R, IBUS_KEY_Shift_L,
                   IBUS_KEY_Shift_R, IBUS_KEY_Alt_L, IBUS_KEY_Alt_R};
   for (size_t i = 0; i < G_N_ELEMENTS(keys); i++) {
     reset_fixture(self); self->composing = TRUE;
     self->completion_undo = g_strdup("raw spelling");
     CHECK(!event(self, keys[i], 37 + i, IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_SHIFT_MASK));
     CHECK(self->composing && actions == 0 && ffi_calls == 0 && commits == 0);
     CHECK(strcmp(self->input->str, "hel") == 0 && strcmp(self->completion_undo, "raw spelling") == 0);
   }
 } else if (strcmp(argv[1], "legacy") == 0) {
   reset_fixture(self); configured_action = 0;
   CHECK(event(self, IBUS_KEY_space, 65, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK)); CHECK(last_action == 6);
   guint keys[] = {IBUS_KEY_k, IBUS_KEY_j, IBUS_KEY_h, IBUS_KEY_l, IBUS_KEY_semicolon};
   for (size_t i = 0; i < G_N_ELEMENTS(keys); i++) {
     reset_fixture(self); configured_action = 0; home_row = TRUE;
     CHECK(event(self, keys[i], 30 + i, IBUS_MOD1_MASK)); CHECK(last_action == i + 1 && commits == 0);
   }
   reset_fixture(self); configured_action = 6;
   CHECK(!event(self, IBUS_KEY_space, 65, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK)); CHECK(actions == 0);
   CHECK(event(self, IBUS_KEY_q, 24, IBUS_CONTROL_MASK)); CHECK(last_action == 6);
   reset_fixture(self); configured_action = 0; self->composing = TRUE;
   CHECK(!event(self, IBUS_KEY_space, 65, IBUS_CONTROL_MASK | IBUS_SHIFT_MASK));
   CHECK(self->composing && actions == 0 && ffi_calls == 1);
 } else if (strcmp(argv[1], "language-feedback") == 0) {
   reset_fixture(self);
   self->language_job = self; /* Presence only: no worker is dereferenced here. */
   self->language_feedback = g_strdup("pending");
   CHECK(event(self, IBUS_KEY_x, 53, 0));
   CHECK(strcmp(self->input->str, "helx") == 0);
   CHECK(self->language_feedback != NULL && strcmp(self->language_feedback, "pending") == 0);
   CHECK(!event(self, IBUS_KEY_x, 53, IBUS_RELEASE_MASK));
   CHECK(self->language_feedback != NULL && actions == 0 && commits == 0);
   CHECK(full_renders == 0 && presentation_renders == 0);
   self->language_job = NULL;
   g_clear_pointer(&self->language_feedback, g_free);
   self->language_feedback = g_strdup("completed");
   CHECK(event(self, IBUS_KEY_y, 29, 0));
   CHECK(self->language_feedback == NULL && strcmp(self->input->str, "helxy") == 0);
   CHECK(full_renders == 0 && presentation_renders == 1);
   reset_fixture(self);
   self->language_feedback = g_strdup("failed");
   CHECK(!event(self, IBUS_KEY_F1, 67, 0));
   CHECK(self->language_feedback == NULL && strcmp(self->input->str, "hel") == 0);
   CHECK(actions == 0 && commits == 0);
   CHECK(full_renders == 0 && presentation_renders == 1);
 } else if (strcmp(argv[1], "control-bounds") == 0) {
   gchar payload[1026]; memset(payload, ' ', sizeof(payload)); payload[0] = 'U';
   CHECK(suzaku_ibus_is_control_request(payload, 127));
   CHECK(suzaku_ibus_is_control_request(payload, 128));
   CHECK(suzaku_ibus_is_control_request(payload, 1024));
   CHECK(!suzaku_ibus_is_control_request(payload, 1025));
   const gchar *legacy = "SLPR";
   for (size_t i = 0; i < strlen(legacy); i++) {
     payload[0] = legacy[i];
     CHECK(suzaku_ibus_is_control_request(payload, 127));
     CHECK(!suzaku_ibus_is_control_request(payload, 128));
     CHECK(!suzaku_ibus_is_control_request(payload, 1024));
   }
   CHECK(!suzaku_ibus_is_control_request("", 0));
   payload[0] = 'Z'; CHECK(!suzaku_ibus_is_control_request(payload, 1));
   payload[0] = 'U'; payload[1] = (gchar)0xff;
   CHECK(!suzaku_ibus_is_control_request(payload, 2));
   payload[1] = 0; CHECK(!suzaku_ibus_is_control_request(payload, 2));
 } else { CHECK(FALSE); }
 g_clear_pointer(&self->completion_undo, g_free); g_clear_pointer(&self->language_feedback, g_free);
 g_hash_table_unref(self->held_adoption_keys); g_string_free(self->input, TRUE);
 printf("PASS: C shortcut adapter %s\n", argv[1]); return 0;
}
'''


class NativeShortcutAdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = SOURCE.read_text()
        functions = "\n".join(function(source, signature) for signature in [
            "static void suzaku_ibus_engine_hold_adoption(",
            "static guint suzaku_ibus_shortcut_action(",
            "static gboolean suzaku_ibus_engine_process_key_event(",
            "static gboolean suzaku_ibus_is_control_request(",
        ])
        cls.temporary = tempfile.TemporaryDirectory(prefix="suzaku-shortcut-c.", dir="/tmp")
        cls.addClassCleanup(cls.temporary.cleanup)
        path = Path(cls.temporary.name)
        harness = path / "adapter.c"
        harness.write_text(STUBS + functions + MAIN)
        cls.binary = path / "adapter"
        include = locate_ibus_include(os.environ)
        flags = subprocess.run(["pkg-config", "--cflags", "--libs", "glib-2.0", "gio-2.0", "xkbcommon"],
                               capture_output=True, text=True, check=True, timeout=5)
        subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror", str(harness),
                        "-I", str(include), "-o", str(cls.binary), *shlex.split(flags.stdout),
                        "-Wl,-l:libibus-1.0.so.5"], check=True, timeout=15)

    def test_actions_modifiers_contexts_held_compose_and_legacy(self):
        for name in ["actions", "modifiers", "guards", "held", "compose-modifiers", "legacy", "control-bounds"]:
            with self.subTest(name=name):
                result = subprocess.run([str(self.binary), name], capture_output=True, text=True, timeout=2)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn(f"PASS: C shortcut adapter {name}", result.stdout)

    def test_language_feedback_lifetime_matches_the_extracted_adapter(self):
        result = subprocess.run([str(self.binary), "language-feedback"],
                                capture_output=True, text=True, timeout=2)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PASS: C shortcut adapter language-feedback", result.stdout)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--structural-only", action="store_true", help="inspect source without compiling")
    options = parser.parse_args()
    classes = [ShortcutStructureTests] if options.structural_only else [ShortcutStructureTests, NativeShortcutAdapterTests]
    suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(cls) for cls in classes)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
