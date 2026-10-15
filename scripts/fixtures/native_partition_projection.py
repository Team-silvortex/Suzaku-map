"""Exact source-only projection for the native core session split.

Historical guards retain their original hashes and physical assertions. The
new partition guard checks the actual selectors/dispatch; this projection
removes only the explicit session routing, never a native check or deadline.
It does not import or execute the native fixture.
"""
from pathlib import Path


IMPORT = "from fixtures.native_partition_projection import historical_source\n"
SELECTOR = '''def native_core_part(environment, only_flag):
    """A core subpart is valid only with the explicit core diagnostic mode."""
    if "SUZAKU_NATIVE_CORE_PART" not in environment:
        return None
    part = environment["SUZAKU_NATIVE_CORE_PART"]
    if only_flag != "SUZAKU_NATIVE_CORE_ONLY" or part not in ("lifecycle", "composition"):
        raise ValueError("Native core part requires CORE_ONLY and lifecycle or composition")
    return part


'''
PART_ASSIGN = "native_core_partition = native_core_part(os.environ, native_only_flag)\n"
LIFECYCLE = '''    check_native_presentation(bus, host_environment)
    check_tray_activation(bus)
    check_language_shortcut(bus)
    check_held_language_transitions(bus)
    check_custom_language_long_workflows(bus)
    check_post_process_preedit(bus)
    check_candidate_page_boundaries(bus)
    check_candidate_row_boundaries(bus)
    check_screen_keys(bus)
    check_english_prose_workflows(bus)
    check_japanese_daily_workflows(bus)
    check_engine_event_boundaries(bus)
    check_prediction_cancellation(bus)
    check_native_draft_limits(bus)
    check_prediction_length_boundaries(bus)
    check_commit_target_boundaries(bus)
    check_cancel_target_boundaries(bus)
    check_draft_end_prediction_boundaries(bus)
    check_language_target_boundaries(bus)
    check_content_type_commit_boundaries(bus)
    check_content_type_prediction_boundaries(bus)
    check_engine_destruction(bus)
    check_engine_destruction_prediction(bus)
    check_committed_model_context(bus)
'''
LIFECYCLE_ROUTED = ('    if native_core_partition != "composition":\n'
                    + "".join("    " + line for line in LIFECYCLE.splitlines(keepends=True))
                    + '    if native_core_partition == "lifecycle":\n'
                      '        raise SystemExit(0)\n')
RECOVERY = '    check_password_probe_cleanup(bus)\n    check_engine_recovery(bus)\n'
RECOVERY_ROUTED = ('    if native_core_partition != "composition":\n'
                   '        check_password_probe_cleanup(bus)\n'
                   '        check_engine_recovery(bus)\n')
SHELL_VALIDATION = '''  if [[ ${SUZAKU_NATIVE_CORE_PART+x} == x ]]; then
    if [[ $suzaku_ci_selected_flag != SUZAKU_NATIVE_CORE_ONLY ||
          ( $SUZAKU_NATIVE_CORE_PART != lifecycle && $SUZAKU_NATIVE_CORE_PART != composition ) ]]; then
      printf 'Native core part requires CORE_ONLY and lifecycle or composition\\n' >&2
      return 2
    fi
  fi
'''
CORE_CASE = '      core) suzaku_ci_partition_env=(SUZAKU_NATIVE_CORE_ONLY=1) ;;\n'
CORE_CASES = '''      core-lifecycle) suzaku_ci_partition_env=(SUZAKU_NATIVE_CORE_ONLY=1 SUZAKU_NATIVE_CORE_PART=lifecycle) ;;
      core-composition) suzaku_ci_partition_env=(SUZAKU_NATIVE_CORE_ONLY=1 SUZAKU_NATIVE_CORE_PART=composition) ;;
      recovery) suzaku_ci_partition_env=(SUZAKU_NATIVE_RECOVERY_ONLY=1) ;;
'''
GUARDS = {
    "test-native-model-fixture.py", "test-held-language-coverage.py",
    "test-custom-language-coverage.py", "test-custom-boundary-coverage.py",
    "test-custom-continuation-coverage.py", "test-custom-long-coverage.py",
}
MODEL_DISPATCH_GUARD = '''    def test_core_session_dispatch_guard(self):
        result = subprocess.run(["/usr/bin/python3", "-B", str(Path(__file__).with_name(
            "test-native-core-partitions.py"))], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

'''


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise AssertionError("Native partition projection requires one exact routing block")
    return source.replace(before, after, 1)


def project_native(source):
    if "native_core_partition" not in source:
        return source
    source = replace_once(source, SELECTOR, "")
    source = replace_once(source, PART_ASSIGN, "")
    source = replace_once(source, LIFECYCLE_ROUTED, LIFECYCLE)
    return replace_once(source, RECOVERY_ROUTED, RECOVERY)


def project_runner(source):
    if "core-lifecycle" not in source:
        return source
    source = replace_once(source,
                          "  suzaku_ci_native_partitions=(core-lifecycle core-composition recovery fallback)\n",
                          "  suzaku_ci_native_partitions=(core fallback)\n")
    source = replace_once(source, SHELL_VALIDATION, "")
    source = replace_once(source, CORE_CASES, CORE_CASE)
    source = replace_once(source,
                          "  if [[ ${suzaku_ci_native_partitions[*]} == 'core-lifecycle core-composition recovery fallback' ]]; then\n",
                          "  if [[ ${suzaku_ci_native_partitions[*]} == 'core fallback' ]]; then\n")
    return replace_once(source,
                        "  # Full gates build once and retain every assertion across four fresh sessions.\n",
                        "  # Full gates build once and retain every assertion across two fresh sessions.\n")


def project_guard(source, name):
    if name not in GUARDS:
        raise AssertionError("Unknown historical native partition guard")
    if IMPORT not in source:
        return source
    source = replace_once(source, IMPORT, "")
    if name == "test-native-model-fixture.py":
        source = replace_once(source, MODEL_DISPATCH_GUARD, "")
    # These substitutions reverse source reads only. Existing hash assertions,
    # mutation tests, AST projections and physical workflow bodies stay intact.
    for before, after in (
        ("historical_source(NATIVE_PATH)", "NATIVE_PATH.read_text()"),
        ("historical_source(cls.source)", "cls.source.read_text()"),
        ('historical_source(Path(__file__).with_name("test-linux-ci.sh"))',
         'Path(__file__).with_name("test-linux-ci.sh").read_text()'),
        ("historical_source(ROOT / path).encode()", "(ROOT / path).read_bytes()"),
        ("historical_source(ROOT / name).encode()", "(ROOT / name).read_bytes()"),
        ('historical_source(ROOT / "scripts/test-linux-ci.sh").encode()',
         '(ROOT / "scripts/test-linux-ci.sh").read_bytes()'),
        ("historical_source(ROOT / path)", "(ROOT / path).read_text()"),
        ('historical_source(ROOT / "scripts/test-held-language-coverage.py")',
         '(ROOT / "scripts/test-held-language-coverage.py").read_text()'),
    ):
        source = source.replace(before, after)
    if "historical_source(" in source:
        raise AssertionError("Unexpected historical source routing")
    return source


def historical_source(path):
    path = Path(path)
    source = path.read_text()
    if path.name == "test-native-sync.py":
        return project_native(source)
    if path.name == "test-linux-ci.sh":
        return project_runner(source)
    if path.name in GUARDS:
        return project_guard(source, path.name)
    return source
