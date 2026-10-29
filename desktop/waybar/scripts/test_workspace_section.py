#!/usr/bin/env python3
"""Behavior tests; no compositor connection or real window closure."""

import importlib.util
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
section = load("workspace_section", HERE / "workspace-section.py")
closer = load("close_workspaces", HERE.parents[1] / "hypr/close-workspaces.py")


def client(workspace, address="0xabcd"):
    return {"workspace": {"id": workspace}, "address": address}


class VisibilityTests(unittest.TestCase):
    def test_empty_primary_workspaces_start_collapsed(self):
        state = section.Section()
        for active in range(1, 7):
            state.refresh(active, [])
            self.assertFalse(state.expanded)

    def test_keyboard_visit_opens_every_extra_workspace_including_future_ones(self):
        for active in [7, 8, 9, 10, 11, 12, 13, 50]:
            state = section.Section()
            state.refresh(active, [])
            self.assertTrue(state.expanded)

    def test_occupancy_keeps_open_after_leaving_then_closes_when_empty(self):
        state = section.Section()
        state.refresh(7, [client(12)])
        state.refresh(1, [client(12)])
        self.assertTrue(state.expanded)
        state.refresh(1, [])
        self.assertFalse(state.expanded)

    def test_last_window_closes_while_still_inside_extra(self):
        state = section.Section()
        state.refresh(7, [])
        self.assertTrue(state.expanded)
        state.refresh(6, [])
        self.assertFalse(state.expanded)

    def test_window_moved_into_extra_opens_section(self):
        state = section.Section()
        state.refresh(1, [client(2)])
        self.assertFalse(state.expanded)
        state.refresh(1, [client(7)])
        self.assertTrue(state.expanded)

    def test_manual_open_stays_pinned_despite_becoming_empty(self):
        state = section.Section()
        state.toggle()
        state.refresh(7, [client(7)])
        state.refresh(1, [])
        self.assertTrue(state.expanded)
        self.assertEqual(state.mode, "pinned")
        state.toggle()
        self.assertFalse(state.expanded)

    def test_manual_close_overrides_occupancy_without_altering_clients(self):
        clients = [client(7), client(12)]
        state = section.Section()
        state.refresh(1, clients)
        state.toggle()
        state.refresh(1, clients)
        self.assertFalse(state.expanded)
        self.assertEqual(clients, [client(7), client(12)])
        state.refresh(7, clients)
        self.assertTrue(state.expanded)

    def test_manual_close_while_inside_stays_closed_until_next_visit(self):
        state = section.Section()
        state.refresh(7, [client(7)])
        state.toggle()
        state.refresh(7, [client(7)])
        self.assertFalse(state.expanded)
        state.refresh(1, [client(7)])
        self.assertFalse(state.expanded)
        state.refresh(7, [client(7)])
        self.assertTrue(state.expanded)

    def test_hidden_resets_automatic_mode_when_empty_outside(self):
        state = section.Section("hidden", 1, True)
        state.refresh(1, [])
        self.assertEqual(state.mode, "auto")
        state.refresh(1, [client(20)])
        self.assertTrue(state.expanded)

    def test_second_manual_open_after_hide_pins(self):
        state = section.Section("hidden", 1, True)
        state.toggle()
        self.assertEqual(state.mode, "pinned")

    def test_special_workspace_does_not_hold_extra_section_open(self):
        state = section.Section()
        state.refresh(1, [client(-99)])
        self.assertFalse(state.expanded)

    def test_button_direction_and_tooltip(self):
        state = section.Section()
        closed = state.status()["text"]
        state.toggle()
        self.assertNotEqual(closed, state.status()["text"])
        self.assertIn("Fijados", state.status()["tooltip"])

    def test_unchanged_state_does_not_wake_animation_or_status(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            state = section.Section()
            section.publish(state, directory)
            path = directory / "state.json"
            mtime = path.stat().st_mtime_ns
            section.publish(state, directory)
            self.assertEqual(mtime, path.stat().st_mtime_ns)
            state.toggle()
            section.publish(state, directory)
            saved = json.loads(path.read_text())
            self.assertTrue(saved["expanded"])
            self.assertEqual(saved["mode"], "pinned")

    def test_status_stream_updates_without_one_second_polling(self):
        with tempfile.TemporaryDirectory() as directory:
            environment = dict(os.environ, XDG_RUNTIME_DIR=directory,
                               HYPRLAND_INSTANCE_SIGNATURE="waybar-animation-test")
            with patch.dict(os.environ, environment):
                runtime = section.runtime_dir()
            state = section.Section()
            section.publish(state, runtime)
            process = subprocess.Popen(
                [sys.executable, str(HERE / "workspace-section.py"), "watch"],
                env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            try:
                self.assertTrue(select.select([process.stdout], [], [], 3)[0], "stream did not start")
                self.assertEqual(json.loads(process.stdout.readline()), state.status())
                for _ in range(4):
                    state.toggle()
                    section.publish(state, runtime)
                    self.assertTrue(select.select([process.stdout], [], [], 0.5)[0], "status was delayed")
                    self.assertEqual(json.loads(process.stdout.readline()), state.status())
            finally:
                process.terminate()
                process.communicate(timeout=3)


class ClosureTests(unittest.TestCase):
    def setUp(self):
        self.clients = [client(i, f"0x{n:x}") for n, i in enumerate([1, 6, 7, 12, 25, -99], 1)]

    def test_main_excludes_extra_and_special(self):
        self.assertEqual([c["workspace"]["id"] for c in closer.select_clients(self.clients, "main")], [1, 6])

    def test_extra_includes_workspaces_beyond_zeta(self):
        self.assertEqual([c["workspace"]["id"] for c in closer.select_clients(self.clients, "extra")], [7, 12, 25])

    def test_all_includes_special_workspaces(self):
        self.assertEqual(closer.select_clients(self.clients, "all"), self.clients)

    def test_uses_normal_close_by_validated_address(self):
        self.assertEqual(closer.close_commands([client(7)]), ["dispatch closewindow address:0xabcd"])
        self.assertEqual(closer.select_clients([client(7, "0xab; dispatch exit")], "all"), [])

    def test_dry_run_never_dispatches_closure(self):
        response = type("Result", (), {"stdout": json.dumps(self.clients)})()
        with patch.object(sys, "argv", ["close-workspaces.py", "extra", "--dry-run"]), \
             patch.object(closer.subprocess, "run", return_value=response) as run, \
             patch("builtins.print"):
            self.assertEqual(closer.main(), 0)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.args[0], ["hyprctl", "-j", "clients"])


class WaybarConfigurationTests(unittest.TestCase):
    def test_persistent_workspaces_cannot_create_two_visible_lists(self):
        # Waybar 0.15 loads all persistent Hyprland rules into every instance,
        # including workspaces excluded by ignore-workspaces. One native list
        # must own every numeric workspace, in order, with no second instance.
        text = (HERE.parent / "config.jsonc").read_text()
        config = json.loads("\n".join(
            line for line in text.splitlines() if not line.lstrip().startswith("//")
        ))
        instances = [name for name in config["modules-left"]
                     if name.split("#")[0] == "hyprland/workspaces"]
        self.assertEqual(instances, ["hyprland/workspaces"])
        self.assertEqual(config["modules-left"], ["hyprland/workspaces", "custom/workspace-toggle", "cffi/workspace-animation"])
        workspaces = config[instances[0]]
        self.assertEqual(workspaces["sort-by"], "number")
        self.assertTrue(workspaces["all-outputs"])
        self.assertEqual(set(workspaces["persistent-workspaces"]), {str(i) for i in range(1, 13)})


if __name__ == "__main__":
    unittest.main()
