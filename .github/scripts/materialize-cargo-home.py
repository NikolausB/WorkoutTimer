#!/usr/bin/env python3
"""Materialize an offline CARGO_HOME from cargo-sources.json.

Replicates what flatpak-builder does with the manifest's cargo sources:
archive entries are downloaded, checksum-verified and extracted into
their dest (single top-level dir stripped, like flatpak-builder), inline
entries are written verbatim. Used by CI to run `cargo test` offline
against exactly the crate set the Flatpak build uses.

Usage: materialize-cargo-home.py [cargo-sources.json] [output-dir]
"""
import concurrent.futures
import hashlib
import json
import os
import sys
import tarfile
import tempfile
import urllib.request

sources_file = sys.argv[1] if len(sys.argv) > 1 else "cargo-sources.json"
root = os.path.abspath(sys.argv[2] if len(sys.argv) > 2 else ".ci-cargo")
sources = json.load(open(sources_file))


def stripped(name):
    parts = name.split("/", 1)
    return parts[1] if len(parts) > 1 else None


def fetch(entry):
    # Manifest dests look like "cargo/vendor/<crate>" - relative to the
    # module source dir in flatpak-builder, where CARGO_HOME is the
    # "cargo" dir. Here the CARGO_HOME *is* the root, so strip that
    # leading component.
    rel = entry["dest"]
    prefix = "cargo/"
    if rel.startswith(prefix):
        rel = rel[len(prefix):]
    dest = os.path.join(root, rel)
    os.makedirs(dest, exist_ok=True)
    if entry["type"] == "inline":
        with open(os.path.join(dest, entry["dest-filename"]), "w") as f:
            f.write(entry["contents"])
        return
    if entry["type"] != "archive":
        raise SystemExit(f"unsupported source type: {entry['type']}")
    data = urllib.request.urlopen(entry["url"]).read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != entry["sha256"]:
        raise SystemExit(f"checksum mismatch for {entry['url']}")
    with tempfile.NamedTemporaryFile(suffix=".crate") as tmp:
        tmp.write(data)
        tmp.flush()
        with tarfile.open(tmp.name) as tar:
            members = [m for m in tar.getmembers() if stripped(m.name)]
            for m in members:
                m.name = stripped(m.name)
            try:
                tar.extractall(dest, members=members, filter="data")
            except TypeError:
                tar.extractall(dest, members=members)


with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
    for _ in pool.map(fetch, sources):
        pass

# Cargo config read from $CARGO_HOME: replaces crates-io with the vendor
# layout extracted above. The path must be absolute - cargo resolves
# relative paths in $CARGO_HOME/config against an unrelated base.
with open(os.path.join(root, "config"), "w") as f:
    f.write(
        '[source.crates-io]\n'
        'replace-with = "vendored-sources"\n'
        '\n'
        '[source.vendored-sources]\n'
        f'directory = "{os.path.join(root, "vendor")}"\n'
    )
print(f"materialized {len(sources)} sources into {root}")
