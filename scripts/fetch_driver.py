#!/usr/bin/env python3
"""Fetch the Cua Driver build a gui-gate release archive carries, verify it, and lay it out for packing.

    python3 scripts/fetch_driver.py PLATFORM OUTDIR [--require-signature]

PLATFORM is one of the gui-gate archive names in scripts/cua-driver.lock (linux-x86_64, linux-aarch64,
macos-universal, windows-x86_64). Checks, in order: the sha256 pinned in the lock file, the same line in the Cua
release's SHA256SUMS, and (with cosign on PATH, mandatory with --require-signature) the asset's keyless Sigstore
bundle issued to trycua/cua's release workflow at that tag. Then writes into OUTDIR:
    cua-driver/            cua-driver[.exe], cua-cursor-theme[.exe], (Linux) wayland-helper/, (Windows)
                           cua-driver-uia.exe, LICENSE, THIRD_PARTY_NOTICES.md, VERSION
    CuaDriver.app/         macOS only: Cua's signed app bundle, unmodified (extracted with the system tar)
"""
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = "trycua/cua"


def lock():
    d = {}
    for line in open(os.path.join(HERE, "cua-driver.lock")):
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            d[k.strip()] = v.strip()
    return d


def get(url, path):
    for attempt in range(4):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "gui-gate-release"})
            with urllib.request.urlopen(req, timeout=120) as r, open(path, "wb") as f:
                shutil.copyfileobj(r, f)
            return
        except Exception as e:  # noqa: BLE001
            if attempt == 3:
                raise SystemExit(f"download failed: {url}: {e}")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    require_sig = "--require-signature" in sys.argv
    if len(args) != 2:
        raise SystemExit(__doc__)
    plat, out = args
    lk = lock()
    if plat not in lk:
        raise SystemExit(f"no Cua Driver pinned for {plat}")
    asset, want = lk[plat].split()
    tag, version = lk["tag"], lk["version"]
    base = f"https://github.com/{REPO}/releases/download/{tag}"
    tmp = tempfile.mkdtemp(prefix="cua-fetch-")
    try:
        a = os.path.join(tmp, asset)
        get(f"{base}/{asset}", a)
        got = sha256(a)
        if got != want:
            raise SystemExit(f"{asset}: sha256 {got} != pinned {want}")
        sums = os.path.join(tmp, "SHA256SUMS")
        get(f"{base}/SHA256SUMS", sums)
        listed = {ln.split()[1].lstrip("*"): ln.split()[0] for ln in open(sums) if len(ln.split()) == 2}
        if listed.get(asset) != want:
            raise SystemExit(f"{asset}: not listed with the pinned hash in the release's SHA256SUMS")
        cosign = shutil.which("cosign")
        if cosign:
            b = a + ".sigstore.json"
            get(f"{base}/{asset}.sigstore.json", b)
            ident = f"https://github.com/{REPO}/.github/workflows/cd-rust-cua-driver.yml@refs/tags/{tag}"
            r = subprocess.run([cosign, "verify-blob", "--bundle", b, "--certificate-identity", ident,
                                "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com", a],
                               capture_output=True, text=True)
            if r.returncode != 0:
                raise SystemExit(f"{asset}: Sigstore verification failed:\n{r.stdout}{r.stderr}")
            print(f"{asset}: Sigstore bundle verified ({ident})")
        elif require_sig:
            raise SystemExit("--require-signature: cosign is not on PATH")
        print(f"{asset}: sha256 verified ({want})")

        x = os.path.join(tmp, "x")
        os.makedirs(x)
        if asset.endswith(".zip"):
            zipfile.ZipFile(a).extractall(x)
        else:
            subprocess.run(["tar", "-xzf", a, "-C", x], check=True)
        roots = [os.path.join(x, n) for n in os.listdir(x)]
        src = roots[0] if len(roots) == 1 and os.path.isdir(roots[0]) else x
        win = asset.endswith(".zip")
        mac = "darwin" in asset
        exe = ".exe" if win else ""
        dst = os.path.join(out, "cua-driver")
        os.makedirs(dst, exist_ok=True)
        files = ["LICENSE", "THIRD_PARTY_NOTICES.md"]
        if not mac:
            files += [f"cua-driver{exe}", f"cua-cursor-theme{exe}"]
        if win:
            files += ["cua-driver-uia.exe"]
        for f in files:
            p = os.path.join(src, f)
            if not os.path.exists(p):
                if f == "cua-driver-uia.exe":
                    continue
                raise SystemExit(f"{asset} has no {f}")
            shutil.copy2(p, os.path.join(dst, f))
        if os.path.isdir(os.path.join(src, "wayland-helper")) and not win and not mac:
            shutil.copytree(os.path.join(src, "wayland-helper"), os.path.join(dst, "wayland-helper"), symlinks=True)
        if mac:
            app = os.path.join(src, "CuaDriver.app")
            if not os.path.isdir(app):
                raise SystemExit(f"{asset} has no CuaDriver.app")
            # keep the bundle byte-for-byte (its code signature): move it with the system tools, no re-signing
            subprocess.run(["ditto" if shutil.which("ditto") else "cp", *([] if shutil.which("ditto") else ["-a"]),
                            app, os.path.join(out, "CuaDriver.app")], check=True)
        with open(os.path.join(dst, "VERSION"), "w") as f:
            f.write(version + "\n")
        print(f"Cua Driver {version} for {plat} laid out in {out}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
