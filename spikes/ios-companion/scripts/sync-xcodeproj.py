#!/usr/bin/env python3
"""Spike-only: rewrite the hand-made project.pbxproj file lists from the app folder.

Usage: sync-xcodeproj.py <spike>/ios/<App>.xcodeproj

Every *.swift in ios/<App>/ is compiled; the example data in assets/examples/ named in
RESOURCES is bundled straight from the repo (no copies). Object ids are derived from file
names, so reruns are stable. The Rust build phase always runs cargo (incremental), so a
changed C ABI is never linked from a stale library.
"""
import hashlib
import os
import re
import sys

RESOURCES = ["countries.geojson", "cities.geojson", "earthquakes.csv", "volcanoes.csv"]


def oid(kind: str, name: str) -> str:
    return hashlib.sha1(f"{kind}:{name}".encode()).hexdigest()[:24].upper()


def replace_section(text: str, section: str, body: str) -> str:
    pat = re.compile(rf"(/\* Begin {section} section \*/\n).*?(/\* End {section} section \*/)", re.S)
    if not pat.search(text):
        raise SystemExit(f"no {section} section")
    return pat.sub(lambda m: m.group(1) + body + m.group(2), text)


def replace_files_list(text: str, obj_comment: str, entries: list) -> str:
    pat = re.compile(rf"(/\* {re.escape(obj_comment)} \*/ = \{{\n\t\t\tisa = \w+;\n\t\t\tbuildActionMask = \d+;\n\t\t\tfiles = \(\n).*?(\t\t\t\);)", re.S)
    if not pat.search(text):
        raise SystemExit(f"no files list for {obj_comment}")
    return pat.sub(lambda m: m.group(1) + "".join(f"\t\t\t\t{e},\n" for e in entries) + m.group(2), text, count=1)


def main() -> None:
    proj = sys.argv[1].rstrip("/")
    ios = os.path.dirname(os.path.abspath(proj))
    app = os.path.basename(proj)[: -len(".xcodeproj")]
    src = os.path.join(ios, app)
    path = os.path.join(proj, "project.pbxproj")
    text = open(path).read()

    swift = sorted(f for f in os.listdir(src) if f.endswith(".swift") and not f.startswith("._"))
    build, refs, children, sources, resources = [], [], [], [], []
    for f in swift:
        r, b = oid("ref", f), oid("build", f)
        refs.append(f"\t\t{r} /* {f} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = {f}; sourceTree = \"<group>\"; }};\n")
        build.append(f"\t\t{b} /* {f} in Sources */ = {{isa = PBXBuildFile; fileRef = {r} /* {f} */; }};\n")
        children.append(f"{r} /* {f} */")
        sources.append(f"{b} /* {f} in Sources */")
    for f in RESOURCES:
        r, b = oid("res-ref", f), oid("res-build", f)
        ftype = "text" if f.endswith(".csv") else "text.json"
        refs.append(f"\t\t{r} /* {f} */ = {{isa = PBXFileReference; lastKnownFileType = {ftype}; name = {f}; path = ../../../assets/examples/{f}; sourceTree = SOURCE_ROOT; }};\n")
        build.append(f"\t\t{b} /* {f} in Resources */ = {{isa = PBXBuildFile; fileRef = {r} /* {f} */; }};\n")
        children.append(f"{r} /* {f} */")
        resources.append(f"{b} /* {f} in Resources */")

    # Keep the non-Swift references (headers, Info.plist, product, MapLibre framework build file).
    keep_refs = [l + "\n" for l in re.findall(r"^\t\t\w+ /\* [^*]+ \*/ = \{isa = PBXFileReference;.*$", text, re.M)
                 if ".swift" not in l and "assets/examples" not in l]
    keep_build = [l + "\n" for l in re.findall(r"^\t\t\w+ /\* [^*]+ \*/ = \{isa = PBXBuildFile;.*$", text, re.M)
                  if "in Frameworks" in l]
    text = replace_section(text, "PBXBuildFile", "".join(keep_build + build))
    text = replace_section(text, "PBXFileReference", "".join(keep_refs + refs))

    # App group children: Swift + resources + the kept header/plist entries.
    gpat = re.compile(rf"(/\* {app} \*/ = \{{\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = \(\n)(.*?)(\t\t\t\);\n\t\t\tpath = {app};)", re.S)
    m = gpat.search(text)
    if not m:
        raise SystemExit("no app group")
    kept = [l.strip().rstrip(",") for l in m.group(2).splitlines() if l.strip() and ".swift" not in l
            and not any(r in l for r in RESOURCES)]
    entries = children + kept
    text = text[: m.start(2)] + "".join(f"\t\t\t\t{e},\n" for e in entries) + text[m.end(2):]

    text = replace_files_list(text, "Sources", sources)
    text = replace_files_list(text, "Resources", resources)
    text = text.replace('if [[ ! -f \\"$LIB\\" ]]; then\\n', "if true; then\\n")
    open(path, "w").write(text)
    print(f"{app}: {len(swift)} Swift files, {len(RESOURCES)} resources")


if __name__ == "__main__":
    main()
