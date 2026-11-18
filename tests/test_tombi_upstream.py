from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest


updater = runpy.run_path(str(Path(__file__).resolve().parents[1] / "scripts/tombi-upstream.py"))
select = updater["select"]
manifest_branch = updater["manifest_branch"]
update_manifest_branch = updater["update_manifest_branch"]
require_locked_revision = updater["require_locked_revision"]
fork = updater["FORK"]
packages = updater["ENGINE_PACKAGES"]
current = "dprint/v1.11.2-headless"
newer = "dprint/v1.11.10-headless"
older = "dprint/v1.10.0-headless"
first = "1" * 40
second = "2" * 40


def ref(branch, commit):
    return f"{commit}\trefs/heads/{branch}\n"


def lock_entry(name, branch, commit, repository=fork):
    return f'[[package]]\nname = "{name}"\nsource = "git+{repository}?branch={branch.replace("/", "%2F")}#{commit}"\n'


def locked(branch, commit):
    return "".join(lock_entry(name, branch, commit) for name in packages)


class TombiUpdaterTest(unittest.TestCase):
    def test_old_remote_fails_before_cargo_can_refresh(self):
        with self.assertRaisesRegex(ValueError, "older than"):
            select(current, ref(older, first))

    def test_same_version_can_refresh_to_a_new_commit(self):
        self.assertEqual(select(current, ref(current, second)), (current, second))

    def test_selects_latest_version_numerically(self):
        self.assertEqual(select(current, ref(current, first) + ref(newer, second)), (newer, second))

    def test_rejects_mixed_manifest_branches(self):
        manifest = "[dependencies]\n" + "".join(
            f'{name} = {{ git = "{fork}", branch = "{current}" }}\n' for name in packages
        )
        self.assertEqual(manifest_branch(manifest), current)
        with self.assertRaisesRegex(ValueError, "one fork branch"):
            manifest_branch(manifest.replace(current, newer, 1))

    def test_rejects_mixed_or_moved_lock_sources(self):
        require_locked_revision(locked(current, first), current, first)
        for source in (locked(current, second), locked(current, first) + locked(newer, first)):
            with self.assertRaisesRegex(ValueError, "must resolve every Tombi crate"):
                require_locked_revision(source, current, first)

    def test_rejects_a_redirected_package_alongside_valid_fork_entries(self):
        source = "".join(
            lock_entry(name, current, first) for name in packages if name != "tombi-formatter"
        )
        source += lock_entry("tombi-formatter", current, first, "https://github.com/example/tombi")
        with self.assertRaisesRegex(ValueError, "tombi-formatter uses"):
            require_locked_revision(source, current, first)

    def test_updates_mixed_quotes_and_leaves_other_branch_values_unchanged(self):
        def dependencies(branch):
            lines = (
                f"{name} = {{ git = \"{fork}\", branch  =  {quote}{branch}{quote} }}"
                for name, quote in zip(packages, ('"', "'") * 3)
            )
            return "[dependencies]\r\n" + "\r\n".join(lines) + "\r\n"

        unrelated = (
            f"# branch = '{current}'\r\n"
            f"other = {{ git = 'https://github.com/example/other', branch = '{current}' }}\r\n"
        )
        updated = update_manifest_branch(dependencies(current) + unrelated, current, newer)
        self.assertEqual(updated, dependencies(newer) + unrelated)
        self.assertEqual(manifest_branch(updated), newer)

    def test_requires_the_matching_upstream_tag_as_an_ancestor(self):
        with tempfile.TemporaryDirectory() as repository:
            def git(*args, data=""):
                return subprocess.check_output(
                    ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com", *args],
                    cwd=repository, input=data, text=True,
                ).strip()

            git("init", "--bare", "--quiet")
            git("config", "uploadpack.allowFilter", "true")
            tree = git("mktree")
            base = git("commit-tree", tree, "--no-gpg-sign", "-m", "upstream")
            descendant = git("commit-tree", tree, "--no-gpg-sign", "-p", base, "-m", "headless")
            unrelated = git("commit-tree", tree, "--no-gpg-sign", "-m", "unrelated")
            git("update-ref", "refs/tags/v1.11.2", base)
            git("update-ref", "refs/heads/headless", descendant)
            git("update-ref", "refs/heads/unrelated", unrelated)
            upstream_base = updater["upstream_base"]
            self.assertEqual(upstream_base(current, descendant, fork=repository, upstream=repository), base)
            with self.assertRaisesRegex(ValueError, "does not descend from upstream"):
                upstream_base(current, unrelated, fork=repository, upstream=repository)
