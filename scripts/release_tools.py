#!/usr/bin/env python3
"""Small, dependency-free helpers used by the release workflow."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEMVER = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")
CONVENTIONAL = re.compile(r"^[a-zA-Z][\w-]*(?:\([^)]*\))?(!)?:")
BREAKING = re.compile(r"(^|\n)BREAKING(?: |-)?CHANGE:", re.IGNORECASE)
DEFAULT_MODEL = "deepseek/deepseek-v4-pro"
OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions"
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


def run_git(*args: str, cwd: Path = ROOT) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout.strip()


def parse_version(tag: str) -> tuple[int, int, int]:
    match = SEMVER.fullmatch(tag.strip())
    if match is None:
        raise ValueError(f"invalid semantic version: {tag}")
    return tuple(int(part) for part in match.groups())  # type: ignore[return-value]


def release_level(messages: list[str]) -> str | None:
    if not messages:
        return None
    if any(BREAKING.search(message) or (CONVENTIONAL.match(message) or [None, None])[1] for message in messages):
        return "major"
    if any(re.match(r"^feat(?:\([^)]*\))?!?:", message, re.IGNORECASE) for message in messages):
        return "minor"
    return "patch"


def bump_version(version: tuple[int, int, int], level: str) -> tuple[int, int, int]:
    major, minor, patch = version
    if level == "major":
        return major + 1, 0, 0
    if level == "minor":
        return major, minor + 1, 0
    if level == "patch":
        return major, minor, patch + 1
    raise ValueError(f"unknown release level: {level}")


def next_tag(tags: list[str], messages: list[str]) -> str | None:
    level = release_level(messages)
    if level is None:
        return None
    base = parse_version(tags[0]) if tags else (0, 0, 0)
    version = bump_version(base, level)
    return f"v{version[0]}.{version[1]}.{version[2]}"


def latest_tag(cwd: Path = ROOT) -> str | None:
    tags = run_git("tag", "--list", "v[0-9]*", "--sort=-version:refname", cwd=cwd)
    return tags.splitlines()[0] if tags else None


def commit_messages(since: str | None, target: str = "HEAD", cwd: Path = ROOT) -> list[str]:
    revision = f"{since}..{target}" if since else target
    output = run_git("log", "--format=%B%x00", revision, cwd=cwd)
    return [message.strip() for message in output.split("\x00") if message.strip()]


def calculate_next_tag(cwd: Path = ROOT) -> str | None:
    previous = latest_tag(cwd)
    tags = [previous] if previous else []
    return next_tag(tags, commit_messages(previous, cwd=cwd))


def replace_package_version(text: str, package: str, version: str) -> str:
    package_pattern = re.compile(
        rf'(\[\[package\]\]\s+name = "{re.escape(package)}"\s+version = ")[^"]+(")',
        re.MULTILINE,
    )
    updated, count = package_pattern.subn(rf"\g<1>{version}\2", text, count=1)
    if count != 1:
        raise ValueError(f"could not find {package} in Cargo.lock")
    return updated


def sync_version(tag: str, root: Path = ROOT) -> None:
    version = ".".join(str(part) for part in parse_version(tag))
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    manifest = manifest_path.read_text(encoding="utf-8")
    updated_manifest, count = re.subn(
        r'(?ms)(^\[package\]\s+.*?^version = ")[^"]+(")',
        rf"\g<1>{version}\2",
        manifest,
        count=1,
    )
    if count != 1:
        raise ValueError("could not find package version in Cargo.toml")
    manifest_path.write_text(updated_manifest, encoding="utf-8")
    lock_path.write_text(
        replace_package_version(lock_path.read_text(encoding="utf-8"), "markdown-tasks", version),
        encoding="utf-8",
    )


def release_context(tag: str, cwd: Path = ROOT) -> tuple[str | None, str]:
    tags = run_git("tag", "--list", "v[0-9]*", "--sort=-version:refname", cwd=cwd).splitlines()
    previous = next((candidate for candidate in tags if candidate != tag), None)
    revision = f"{previous}..{tag}" if previous else tag
    commits = run_git("log", "--reverse", "--format=- %h %s%n%b", revision, cwd=cwd)
    if previous:
        changed = run_git("diff", "--name-status", f"{previous}..{tag}", cwd=cwd)
        stats = run_git("diff", "--stat", f"{previous}..{tag}", cwd=cwd)
    else:
        changed = run_git("diff", "--name-status", EMPTY_TREE, tag, cwd=cwd)
        stats = run_git("diff", "--stat", EMPTY_TREE, tag, cwd=cwd)
    context = (
        f"Release tag: {tag}\n"
        f"Previous release: {previous or 'none (initial release)'}\n"
        f"Exact revision range: {revision}\n\n"
        f"COMMITS\n{commits}\n\n"
        f"CHANGED FILES\n{changed}\n\n"
        f"DIFF STAT\n{stats}\n"
    )
    return previous, context


def generate_notes(tag: str, root: Path = ROOT) -> Path:
    token = os.environ.get("OPENROUTER_API_KEY")
    if not token:
        raise RuntimeError("OPENROUTER_API_KEY is required")
    prompt = (root / ".github/prompts/github-create-release.md").read_text(encoding="utf-8")
    _, context = release_context(tag, root)
    payload = {
        "model": os.environ.get("RELEASE_NOTES_MODEL", DEFAULT_MODEL),
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": context[:120_000]},
        ],
    }
    request = urllib.request.Request(
        OPENROUTER_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://github.com/markwylde/markdown-tasks",
            "X-Title": "markdown-tasks release notes",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=180) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"OpenRouter returned HTTP {error.code}: {detail}") from error
    try:
        notes = result["choices"][0]["message"]["content"].strip()
    except (KeyError, IndexError, TypeError, AttributeError) as error:
        raise RuntimeError(f"OpenRouter returned an unexpected response: {result}") from error
    if notes.startswith("```") and notes.endswith("```"):
        notes = re.sub(r"^```(?:markdown)?\s*", "", notes)
        notes = re.sub(r"\s*```$", "", notes)
    if not notes:
        raise RuntimeError("OpenRouter returned empty release notes")
    output = root / "RELEASE.md"
    output.write_text(f"{notes.rstrip()}\n", encoding="utf-8")
    return output


def package_binary(
    tag: str,
    target: str,
    binary: Path,
    archive_format: str,
    output_dir: Path,
    root: Path = ROOT,
) -> tuple[Path, Path]:
    parse_version(tag)
    if not binary.is_file():
        raise ValueError(f"binary does not exist: {binary}")
    output_dir.mkdir(parents=True, exist_ok=True)
    suffix = "zip" if archive_format == "zip" else "tar.gz"
    archive = output_dir / f"mdt-{tag}-{target}.{suffix}"
    executable_name = "mdt.exe" if binary.suffix == ".exe" else "mdt"
    with tempfile.TemporaryDirectory() as temporary:
        bundle = Path(temporary) / f"mdt-{tag}-{target}"
        bundle.mkdir()
        (bundle / executable_name).write_bytes(binary.read_bytes())
        for name in ("README.md", "LICENSE"):
            (bundle / name).write_bytes((root / name).read_bytes())
        if archive_format == "zip":
            with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle_zip:
                for path in sorted(bundle.iterdir()):
                    bundle_zip.write(path, f"{bundle.name}/{path.name}")
        elif archive_format == "tar.gz":
            with tarfile.open(archive, "w:gz") as bundle_tar:
                bundle_tar.add(bundle, arcname=bundle.name)
        else:
            raise ValueError(f"unsupported archive format: {archive_format}")
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum_path = archive.with_name(f"{archive.name}.sha256")
    checksum_path.write_text(f"{checksum}  {archive.name}\n", encoding="utf-8")
    return archive, checksum_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("next-tag")
    sync = commands.add_parser("sync-version")
    sync.add_argument("tag")
    notes = commands.add_parser("generate-notes")
    notes.add_argument("tag")
    package = commands.add_parser("package")
    package.add_argument("--tag", required=True)
    package.add_argument("--target", required=True)
    package.add_argument("--binary", required=True, type=Path)
    package.add_argument("--format", required=True, choices=("tar.gz", "zip"))
    package.add_argument("--output-dir", default=Path("dist"), type=Path)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    if args.command == "next-tag":
        print(calculate_next_tag() or "")
    elif args.command == "sync-version":
        sync_version(args.tag)
    elif args.command == "generate-notes":
        print(generate_notes(args.tag))
    elif args.command == "package":
        archive, checksum = package_binary(
            args.tag,
            args.target,
            args.binary,
            args.format,
            args.output_dir,
        )
        print(archive)
        print(checksum)


if __name__ == "__main__":
    main()
