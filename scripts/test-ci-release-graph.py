#!/usr/bin/env python3
"""Source-only CI graph guards; never execute a build, fixture or package.

The legacy normalized YAML projections were captured before splitting the
40-minute Linux job. Passing proves structure preservation, not native input,
installation compatibility or acceptance of any downloadable artifact.
"""
import ast
import contextlib
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import unittest
from unittest import mock

import yaml

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW_PATH = ROOT / ".github/workflows/ci.yml"
# BaseLoader keeps GitHub's `on` key and scalar strings stable across PyYAML
# versions, and never constructs executable Python objects.
GLOBAL_HASH = "b9f43bed286d9a1ba455e699a24c8fbe995eb97734d76e777561a6aaffb4f90d"
OTHER_JOBS_HASH = "bd57dd42d070ea155e1434d70cd3df1a88bfd1cba8bab66b5735ee99e66056b0"
SETUP_HASH = "b45b9c8d207e668254573e6c3bc0acbd1e8e28f2ff263c0dff2aaea2d6a37650"
CORE_HASH = "e124e9c9cc2e48e85686fabb5ab501aac0d14ead49b11b6b8b05600a46b08a86"
APPS_HASH = "0cafc321218eb95c672819ad77fec4e889ce67ca7464661801b466ef6f18aa66"
PACKAGE_HASH = "01970cdb144b572fd4e9af7d14b9a1a28aad7e8dbc57afada46bc49fea39ed69"
BINDING_HASH = "7419b46f7e99a31cd6b678ad5a0ca0295b26532a2bc03944b00f1fd6fd4a0e41"
CACHE_ACTION = "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
UPLOAD_ACTION = "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
# Official actions/download-artifact Git refs API: refs/tags/v4.3.0, commit.
DOWNLOAD_ACTION = "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"
UNACCEPTED = "suzaku-linux-unaccepted-${{ runner.arch }}-${{ github.sha }}-${{ github.run_id }}"
ACCEPTED = "suzaku-linux-${{ runner.arch }}-${{ github.sha }}"
NEEDS = ["linux", "linux-apps", "linux-package", "secrets", "format", "desktop-check"]
LEGACY_CACHE_PREFIX = "${{ runner.os }}-${{ runner.arch }}-linux-ci-v1-${{ env.RUSTUP_TOOLCHAIN }}-${{ hashFiles('Cargo.lock') }}-"
PRODUCERS = {
    "linux": ("core", "Linux lint, tests and native input", 28),
    "linux-apps": ("apps", "Linux application workflows and recovery", 19),
    "linux-package": ("package", "Linux release packages and clean installation", 9),
}
RESULT_GATE = {
    "name": "Require every regression and package gate to succeed",
    "env": {"SUZAKU_REQUIRED_RESULTS": "${{ toJSON(needs) }}"},
    "run": """/usr/bin/python3 - <<'PY'
import json, os
needs = json.loads(os.environ["SUZAKU_REQUIRED_RESULTS"])
expected = {"linux", "linux-apps", "linux-package", "secrets", "format", "desktop-check"}
if set(needs) != expected:
    raise SystemExit("Missing or unexpected required gate results")
failures = {name: values.get("result") for name, values in needs.items() if values.get("result") != "success"}
if failures:
    raise SystemExit(f"Required gates did not succeed: {failures}")
print("PASS: every required regression and installation gate succeeded")
PY
""",
}
DOCUMENTATION_STEP = {
    "name": "Check documentation structure, history and package navigation",
    "run": "python3 -B scripts/check-documentation.py\n"
           "python3 -B scripts/test-documentation.py\n"
           "python3 -B scripts/test-package-docs.py\n",
}


class UniqueBaseLoader(yaml.BaseLoader):
    def construct_mapping(self, node, deep=False):
        mapping = {}
        for key_node, value_node in node.value:
            key = self.construct_object(key_node, deep=deep)
            if key in mapping:
                raise AssertionError(f"Duplicate YAML key: {key}")
            mapping[key] = self.construct_object(value_node, deep=deep)
        return mapping


def load(source):
    return yaml.load(source, Loader=UniqueBaseLoader)


def digest(value):
    source = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(source.encode()).hexdigest()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def cache_step(role):
    prefix = "${{ runner.os }}-${{ runner.arch }}-linux-ci-v2-" + role
    prefix += "-${{ env.RUSTUP_TOOLCHAIN }}-${{ hashFiles('Cargo.lock') }}-"
    return {
        "name": "Cache Cargo downloads and build outputs",
        "uses": CACHE_ACTION,
        "with": {
            "path": "~/.cargo/registry/index/\n~/.cargo/registry/cache/\n~/.cargo/git/db/\ntarget/\n",
            "key": prefix + "${{ github.sha }}",
            "restore-keys": prefix + "\n" + LEGACY_CACHE_PREFIX + "\n",
        },
    }


def upload_step(accepted):
    return {
        "name": "Upload accepted Linux packages and checksums" if accepted else "Stage unaccepted Linux packages and checksums",
        "uses": UPLOAD_ACTION,
        "with": {
            "name": ACCEPTED if accepted else UNACCEPTED,
            "path": "dist/*",
            "if-no-files-found": "error",
            "retention-days": "14" if accepted else "1",
        },
    }


def python_block(run):
    require(run.startswith("/usr/bin/python3 - <<'PY'\n") and run.endswith("PY\n"), "Owned Python gate block required")
    return ast.parse(run.split("\n", 1)[1].rsplit("PY\n", 1)[0])


def validate(workflow):
    require(digest({key: value for key, value in workflow.items() if key != "jobs"}) == GLOBAL_HASH,
            "Workflow triggers, permissions, defaults, concurrency and global environment changed")
    jobs = workflow["jobs"]
    require(set(jobs) == set(NEEDS + ["linux-artifacts"]), "Unexpected, duplicate or missing jobs")
    # This is the only addition to these frozen jobs. Validate every key and
    # command before removing exactly this final step from the legacy projection;
    # the original hash still protects all prior commands, fields and ordering.
    format_steps = jobs["format"]["steps"]
    require(bool(format_steps) and format_steps[-1] == DOCUMENTATION_STEP,
            "Exact, unconditional documentation checks must finish the format job")
    legacy_other_jobs = copy.deepcopy({name: jobs[name] for name in ["secrets", "format", "desktop-check"]})
    legacy_other_jobs["format"]["steps"].pop()
    require(digest(legacy_other_jobs) == OTHER_JOBS_HASH,
            "Secret, formatting or desktop checks changed")
    for name, (role, display_name, count) in PRODUCERS.items():
        job = jobs[name]
        require(set(job) == {"name", "runs-on", "timeout-minutes", "env", "steps"},
                f"{name}: independent default-success job fields required")
        require(job["name"] == display_name and job["runs-on"] == "ubuntu-24.04", name)
        require(job["timeout-minutes"] == "40", f"{name}: unchanged 40-minute deadline required")
        require(job["env"] == {"LIBGL_ALWAYS_SOFTWARE": "1"}, f"{name}: original environment required")
        steps = job["steps"]
        require(len(steps) == count, f"{name}: complete fixed step count required")
        require(digest(steps[:3]) == SETUP_HASH, f"{name}: unchanged real checkout, dependencies and compiler required")
        require(steps[3] == cache_step(role), f"{name}: isolated cache writer with legacy read fallback required")
    core = jobs["linux"]["steps"]
    require(core[4] == {"name": "Install source-only workflow guard dependency",
                        "run": "sudo apt-get install --no-install-recommends -y python3-yaml"},
            "PyYAML must be explicitly installed for the system interpreter")
    require(core[5] == {"name": "Test parallel release graph and unchanged regression gates",
                        "run": "/usr/bin/python3 -B scripts/test-ci-release-graph.py"},
            "The graph guard must actually run before legacy gates")
    require(digest(core[6:]) == CORE_HASH, "All 22 original core gates must remain exact and ordered")
    require(digest(jobs["linux-apps"]["steps"][4:]) == APPS_HASH,
            "All 15 original application gates must remain exact and ordered")
    package = jobs["linux-package"]["steps"]
    require(digest(package[4:-1]) == PACKAGE_HASH, "All four original release and installation gates must remain exact")
    require(package[-1] == upload_step(False), "Only run/SHA-bound unaccepted staging may follow package checks")

    final = jobs["linux-artifacts"]
    require(set(final) == {"name", "runs-on", "timeout-minutes", "needs", "if", "steps"},
            "Only the required-check result aggregator may have a job-level condition")
    require(final["if"] == "${{ always() }}", "The required check must report upstream failures instead of skipping")
    required_name = "Linux lint, tests, native input and release build"
    require(final["name"] == required_name and sum(job["name"] == required_name for job in jobs.values()) == 1,
            "Keep the existing required-check name uniquely on the all-gates join")
    require(final["runs-on"] == "ubuntu-24.04" and final["timeout-minutes"] == "10", "Bounded Ubuntu artifact join required")
    require(final["needs"] == NEEDS, "Every regression, installation, secret, format and desktop gate must succeed")
    steps = final["steps"]
    require(len(steps) == 5 and steps[0] == RESULT_GATE, "Mandatory exact result check must be first, without bypasses")
    require(steps[1] == core[0], "Fresh trusted checkout must follow a successful result check")
    require(steps[2] == {
        "name": "Download this run's unaccepted Linux candidate",
        "uses": DOWNLOAD_ACTION,
        "with": {"name": UNACCEPTED, "path": "dist"},
    }, "Pinned current-run download must exactly match staged architecture, revision and run")
    require(digest(steps[3]) == BINDING_HASH, "Exact four assets, checksum and clean tar/deb source-binding assertions required")
    # Parse, never execute, the embedded binding code as an additional syntax check.
    python_block(steps[0]["run"])
    python_block(steps[3]["run"])
    require(steps[4] == upload_step(True), "Accepted artifacts may upload only after the complete successful join")


WORKFLOW = load(WORKFLOW_PATH.read_text())


class ReleaseGraphTests(unittest.TestCase):
    def reject(self, edit):
        changed = copy.deepcopy(WORKFLOW)
        edit(changed)
        with self.assertRaises(AssertionError):
            validate(changed)

    def test_actual_source_preserves_all_old_gates_and_all_success_join(self):
        validate(WORKFLOW)

    def test_frozen_gate_commands_environment_and_order_cannot_weaken(self):
        for name, index in [("linux", 6), ("linux-apps", 4), ("linux-package", 4)]:
            for field, value in [("run", "true"), ("if", "false"), ("continue-on-error", "true"),
                                 ("env", {"SUZAKU_IBUS_INLINE_PREEDIT": "0"}), ("timeout-minutes", "60")]:
                with self.subTest(job=name, field=field):
                    self.reject(lambda w: w["jobs"][name]["steps"][index].__setitem__(field, value))
            with self.subTest(job=name, change="drop"):
                self.reject(lambda w: w["jobs"][name]["steps"].pop(index))
            with self.subTest(job=name, change="reorder"):
                self.reject(lambda w: w["jobs"][name]["steps"].reverse())

    def test_all_six_needs_are_mandatory(self):
        for need in NEEDS:
            with self.subTest(need=need):
                self.reject(lambda w: w["jobs"]["linux-artifacts"]["needs"].remove(need))

    def test_no_always_continue_error_or_conditional_producers(self):
        for name in PRODUCERS:
            for field, value in [("if", "always()"), ("if", "success()"), ("continue-on-error", "true")]:
                with self.subTest(job=name, field=field, value=value):
                    self.reject(lambda w: w["jobs"][name].__setitem__(field, value))

    def test_final_always_reports_but_mandatory_first_gate_blocks_every_publish_step(self):
        self.reject(lambda w: w["jobs"]["linux-artifacts"].pop("if"))
        self.reject(lambda w: w["jobs"]["linux-artifacts"].__setitem__("if", "${{ success() }}"))
        self.reject(lambda w: w["jobs"]["linux-artifacts"].__setitem__("continue-on-error", "true"))
        self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"].pop(0))
        self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"].reverse())
        for index in range(5):
            for field, value in [("if", "always()"), ("if", "success()"), ("continue-on-error", "true")]:
                with self.subTest(step=index, field=field):
                    self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][index].__setitem__(field, value))
        for field, value in [("env", {"SUZAKU_REQUIRED_RESULTS": "{}"}), ("run", "true")]:
            with self.subTest(field=field):
                self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][0].__setitem__(field, value))

    def result_gate(self, results):
        # Execute only the frozen, tiny JSON/result predicate, never a build,
        # package-binding block, action, subprocess or native fixture.
        step = WORKFLOW["jobs"]["linux-artifacts"]["steps"][0]
        self.assertEqual(step, RESULT_GATE)
        with mock.patch.dict(os.environ, {"SUZAKU_REQUIRED_RESULTS": json.dumps(results)}):
            with contextlib.redirect_stdout(io.StringIO()):
                exec(compile(python_block(step["run"]), "<required-results>", "exec"), {})

    def test_actual_result_predicate_accepts_only_all_six_successful_gates(self):
        self.result_gate({name: {"result": "success"} for name in NEEDS})
        for name in NEEDS:
            for result in ["failure", "cancelled", "skipped", "", None]:
                with self.subTest(need=name, result=result):
                    results = {need: {"result": "success"} for need in NEEDS}
                    results[name]["result"] = result
                    with self.assertRaises(SystemExit) as failure:
                        self.result_gate(results)
                    self.assertNotIn(failure.exception.code, (None, 0))
            for missing_result in [False, True]:
                with self.subTest(need=name, missing_result=missing_result):
                    results = {need: {"result": "success"} for need in NEEDS}
                    if missing_result:
                        results[name] = {}
                    else:
                        del results[name]
                    with self.assertRaises(SystemExit) as failure:
                        self.result_gate(results)
                    self.assertNotIn(failure.exception.code, (None, 0))
        results = {need: {"result": "success"} for need in NEEDS}
        results["unexpected"] = {"result": "success"}
        with self.assertRaises(SystemExit) as failure:
            self.result_gate(results)
        self.assertNotIn(failure.exception.code, (None, 0))

    def test_producers_remain_parallel_with_unchanged_deadlines_and_environment(self):
        for name in PRODUCERS:
            for field, value in [("timeout-minutes", "41"), ("env", {}), ("needs", ["format"]),
                                 ("runs-on", "ubuntu-latest")]:
                with self.subTest(job=name, field=field):
                    self.reject(lambda w: w["jobs"][name].__setitem__(field, value))

    def test_staging_is_unaccepted_short_lived_and_revision_and_run_bound(self):
        for field, value in [("name", ACCEPTED), ("name", UNACCEPTED.replace("${{ github.sha }}", "main")),
                             ("name", UNACCEPTED.replace("-${{ github.run_id }}", "")),
                             ("retention-days", "14"), ("if-no-files-found", "warn"), ("path", "target/*")]:
            with self.subTest(field=field, value=value):
                self.reject(lambda w: w["jobs"]["linux-package"]["steps"][-1]["with"].__setitem__(field, value))

    def test_download_cannot_read_another_run_repository_or_revision(self):
        for field, value in [("run-id", "123"), ("repository", "other/repo"), ("github-token", "token"),
                             ("name", ACCEPTED), ("path", "elsewhere"), ("merge-multiple", "true")]:
            with self.subTest(field=field):
                self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][2]["with"].__setitem__(field, value))
        self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][2].__setitem__("uses", "actions/download-artifact@v4"))

    def test_caches_have_distinct_writers_and_only_a_legacy_read_fallback(self):
        for name in PRODUCERS:
            for field, value in [("key", cache_step("core")["with"]["key"] + "-wrong"),
                                 ("key", LEGACY_CACHE_PREFIX + "${{ github.sha }}"),
                                 ("restore-keys", LEGACY_CACHE_PREFIX + "\n")]:
                with self.subTest(job=name, field=field):
                    self.reject(lambda w: w["jobs"][name]["steps"][3]["with"].__setitem__(field, value))
        self.reject(lambda w: w["jobs"]["linux-apps"]["steps"].__setitem__(3, cache_step("core")))

    def test_each_lane_still_checks_out_installs_and_builds(self):
        for name in PRODUCERS:
            for index in range(3):
                with self.subTest(job=name, setup=index):
                    self.reject(lambda w: w["jobs"][name]["steps"][index].__setitem__("if", "false"))
        # Cache restoration cannot become proof that the release build ran.
        self.reject(lambda w: w["jobs"]["linux-package"]["steps"][4].__setitem__("if", "steps.cache.outputs.cache-hit != 'true'"))

    def test_no_early_accepted_upload_and_no_final_upload_policy_weakening(self):
        for name in PRODUCERS:
            with self.subTest(job=name):
                self.reject(lambda w: w["jobs"][name]["steps"].append(upload_step(True)))
        for field, value in [("name", UNACCEPTED), ("retention-days", "1"), ("if-no-files-found", "warn")]:
            with self.subTest(field=field):
                self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][-1]["with"].__setitem__(field, value))

    def test_source_and_checksum_binding_checks_cannot_be_dropped(self):
        for sentinel in ["assert {path.name", "assert (assets /", "assert tar_manifest", "assert manifest[\"target\"]",
                         "assert manifest[\"source_dirty\"]", "assert manifest[\"source_date_epoch\"]"]:
            with self.subTest(assertion=sentinel):
                def remove(w):
                    step = w["jobs"]["linux-artifacts"]["steps"][3]
                    step["run"] = "\n".join(line for line in step["run"].split("\n") if sentinel not in line)
                self.reject(remove)
        self.reject(lambda w: w["jobs"]["linux-artifacts"]["steps"][3].__setitem__("env", {"SUZAKU_ACCEPTED_REVISION": "main"}))

    def test_guard_itself_and_explicit_dependency_cannot_be_skipped(self):
        for index in [4, 5]:
            with self.subTest(step=index):
                self.reject(lambda w: w["jobs"]["linux"]["steps"][index].__setitem__("run", "true"))

    def test_existing_required_context_is_uniquely_on_the_final_join(self):
        self.reject(lambda w: w["jobs"]["linux-artifacts"].__setitem__("name", "Different release check"))
        self.reject(lambda w: w["jobs"]["linux"].__setitem__("name", "Linux lint, tests, native input and release build"))

    def test_original_other_jobs_global_permissions_and_session_limits_are_frozen(self):
        self.reject(lambda w: w["permissions"].__setitem__("contents", "write"))
        self.reject(lambda w: w["env"].__setitem__("RUSTUP_TOOLCHAIN", "stable"))
        for name in ["secrets", "format", "desktop-check"]:
            with self.subTest(job=name):
                self.reject(lambda w: w["jobs"][name].__setitem__("continue-on-error", "true"))

    def test_documentation_addition_is_exact_and_only_it_is_removed_from_the_old_projection(self):
        self.assertEqual(WORKFLOW["jobs"]["format"]["steps"][-1], DOCUMENTATION_STEP)
        before = copy.deepcopy(WORKFLOW)
        validate(WORKFLOW)
        self.assertEqual(WORKFLOW, before, "validation must not mutate the source workflow")
        legacy = copy.deepcopy({name: WORKFLOW["jobs"][name]
                                for name in ["secrets", "format", "desktop-check"]})
        self.assertEqual(legacy["format"]["steps"].pop(), DOCUMENTATION_STEP)
        self.assertEqual(digest(legacy), OTHER_JOBS_HASH)

    def test_documentation_checks_cannot_be_missing_changed_reordered_or_bypassed(self):
        self.reject(lambda w: w["jobs"]["format"]["steps"].pop())
        self.reject(lambda w: w["jobs"]["format"]["steps"].append(copy.deepcopy(DOCUMENTATION_STEP)))
        self.reject(lambda w: w["jobs"]["format"]["steps"].insert(0, w["jobs"]["format"]["steps"].pop()))
        for field, value in [("name", "Different documentation checks"), ("run", "true"),
                             ("if", "false"), ("continue-on-error", "true"), ("timeout-minutes", "60"),
                             ("env", {"SKIP_CHECKS": "1"}), ("working-directory", "elsewhere")]:
            with self.subTest(field=field):
                self.reject(lambda w: w["jobs"]["format"]["steps"][-1].__setitem__(field, value))
        commands = DOCUMENTATION_STEP["run"].splitlines(keepends=True)
        for command in commands:
            with self.subTest(removed_command=command):
                self.reject(lambda w: w["jobs"]["format"]["steps"][-1].__setitem__(
                    "run", DOCUMENTATION_STEP["run"].replace(command, "")))
        for changed in ["".join(reversed(commands)), DOCUMENTATION_STEP["run"] + "true\n",
                        DOCUMENTATION_STEP["run"].replace("\n", " || true\n")]:
            with self.subTest(commands=changed):
                self.reject(lambda w: w["jobs"]["format"]["steps"][-1].__setitem__("run", changed))

    def test_duplicate_yaml_keys_cannot_hide_jobs_or_conditions(self):
        with self.assertRaisesRegex(AssertionError, "Duplicate YAML key"):
            load("jobs:\n  linux: first\n  linux: second\n")
        with self.assertRaisesRegex(AssertionError, "Duplicate YAML key"):
            load("jobs:\n  linux:\n    if: success()\n    if: always()\n")

    def test_base_loader_retains_github_on_and_normalized_scalar_contract(self):
        value = load("on:\n  push:\nenv:\n  ONE: 1\n  FALSE: false\n")
        self.assertEqual(value, {"on": {"push": ""}, "env": {"ONE": "1", "FALSE": "false"}})
        self.assertEqual(digest(load("a: 1\nb: 2\n")), digest(load("b: '2'\na: '1'\n")))


if __name__ == "__main__":
    unittest.main()
