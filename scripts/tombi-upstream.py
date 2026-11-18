"""Select and verify the Tombi fork revision used by the updater."""

import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
from urllib.parse import parse_qsl


FORK = "https://github.com/kjanat/tombi"
UPSTREAM = "https://github.com/tombi-toml/tombi"
BRANCH = r"dprint/v(\d+)\.(\d+)\.(\d+)-headless"
SHA = r"[0-9a-f]{40}"
ENGINE_PACKAGES = ("serde_tombi", "tombi-config", "tombi-formatter", "tombi-fs", "tombi-schema-store", "tombi-uri")


def git(*args, cwd=None):
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def manifest_branch(text):
    dependencies = tomllib.loads(text)["dependencies"]
    specs = [dependencies.get(name) for name in ENGINE_PACKAGES]
    if any(not isinstance(spec, dict) or spec.get("git") != FORK for spec in specs):
        raise ValueError("All six Tombi dependencies must use the Tombi fork")
    branches = {spec.get("branch") for spec in specs}
    if len(branches) != 1:
        raise ValueError("Tombi dependencies must use one fork branch")
    branch = branches.pop()
    if not isinstance(branch, str) or not re.fullmatch(BRANCH, branch):
        raise ValueError(f"Invalid Tombi fork branch: {branch!r}")
    return branch


def update_manifest_branch(text, current, branch):
    if current == branch:
        return text

    branch_value = re.compile(r'''(?:\bbranch|"branch"|'branch')\s*=\s*(['"])(?P<value>''' + re.escape(current) + r''')\1''')

    def update(match):
        start, end = match.span("value")
        candidate = text[:start] + branch + text[end:]
        # An identical assignment in a comment or another dependency stays intact.
        dependencies = tomllib.loads(candidate)["dependencies"]
        if any(dependencies[name]["branch"] == branch for name in ENGINE_PACKAGES):
            return match[0].replace(current, branch)
        return match[0]

    updated = branch_value.sub(update, text)
    if manifest_branch(updated) != branch:
        raise ValueError(f"Could not update all Tombi dependencies to {branch}")
    return updated


def version(branch):
    return tuple(map(int, re.fullmatch(BRANCH, branch).groups()))


def select(current, listing):
    choices = []
    for line in listing.splitlines():
        commit, _, ref = line.partition("\t")
        branch = ref.removeprefix("refs/heads/")
        if re.fullmatch(SHA, commit) and re.fullmatch(BRANCH, branch):
            choices.append((branch, commit))
    if not choices:
        raise ValueError("No headless Tombi branches found")
    branch, commit = max(choices, key=lambda choice: version(choice[0]))
    if version(branch) < version(current):
        raise ValueError(f"Newest Tombi fork branch {branch} is older than {current}")
    return branch, commit


def require_locked_revision(text, branch, commit):
    fork_source = f"git+{FORK}"
    locked = set()
    for package in tomllib.loads(text)["package"]:
        name, source = package["name"], package.get("source", "")
        location, _, revision = source.partition("#")
        repository, _, query = location.partition("?")
        if name == "tombi" or name.startswith(("tombi-", "serde_tombi")) or repository == fork_source:
            if (
                repository != fork_source
                or parse_qsl(query, keep_blank_values=True) != [("branch", branch)]
                or revision != commit
            ):
                raise ValueError(
                    f"Cargo.lock must resolve every Tombi crate to {branch} at {commit}; "
                    f"{name} uses {source or 'a local path'}"
                )
            locked.add(name)
    if missing := sorted(set(ENGINE_PACKAGES) - locked):
        raise ValueError(f"Cargo.lock is missing Tombi dependencies: {', '.join(missing)}")


def upstream_base(branch, commit, *, fork=FORK, upstream=UPSTREAM):
    tag = "v" + ".".join(map(str, version(branch)))
    with tempfile.TemporaryDirectory(prefix="tombi-provenance-") as repository:
        git("init", "--bare", "--quiet", cwd=repository)
        git("remote", "add", "upstream", upstream, cwd=repository)
        git("remote", "add", "fork", fork, cwd=repository)
        git("fetch", "--quiet", "--no-tags", "--filter=blob:none", "upstream", f"refs/tags/{tag}", cwd=repository)
        base = git("rev-parse", "FETCH_HEAD^{commit}", cwd=repository)
        git("fetch", "--quiet", "--no-tags", "--filter=blob:none", "fork", commit, cwd=repository)
        if subprocess.run(["git", "merge-base", "--is-ancestor", base, commit], cwd=repository).returncode != 0:
            raise ValueError(f"Tombi fork commit {commit} does not descend from upstream {tag} ({base})")
        return base


def write_outputs(**outputs):
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as file:
        for key, value in outputs.items():
            file.write(f"{key}={value}\n")


def main():
    manifest = Path("Cargo.toml")
    text = manifest.read_bytes().decode("utf-8")
    current = manifest_branch(text)
    match sys.argv[1:]:
        case ["select"]:
            branch, commit = select(current, git("ls-remote", "--heads", FORK, "dprint/v*-headless"))
            updated = update_manifest_branch(text, current, branch)
            if updated != text:
                manifest.write_bytes(updated.encode("utf-8"))
            write_outputs(branch=branch, commit=commit, version=".".join(map(str, version(branch))))
        case ["verify", branch, commit] if re.fullmatch(SHA, commit) and current == branch:
            require_locked_revision(Path("Cargo.lock").read_text(encoding="utf-8"), branch, commit)
            base = upstream_base(branch, commit)
            write_outputs(compare=f"{FORK}/compare/{base}...{commit}")
        case _:
            raise ValueError("Expected select or verify BRANCH SHA")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
