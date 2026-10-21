# NimbleMap for Linux

A portable tarball, best effort (docs/lm/spec.md; the details are in docs/lm/linux.md). These
files are NimbleMap's, not the old RustGIS packages in `packaging/linux` and `packaging/package.sh`.

| File | What it is |
|---|---|
| `build-tarball.sh` | Builds `dist/lm/NimbleMap-<version>-<build>-linux-<arch>.tar.gz` (`NimbleMap-Pro-…` with `LM_TIER=pro`) |
| `nimblemap.desktop` | The application menu entry, copied into the tarball |
| `nimblemap-mime.xml` | The `.lmap` type (`application/x-layeredmap-project`), copied into the tarball |

## Building

On Ubuntu or Debian (other distributions have the same libraries under their own names):

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxcb1-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libegl1-mesa-dev libgl1-mesa-dev libvulkan-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # Rust, if not installed

packaging/lm/linux/build-tarball.sh                     # NimbleMap (Free)
LM_TIER=pro packaging/lm/linux/build-tarball.sh         # NimbleMap Pro
LM_BUILD_NUMBER=12 packaging/lm/linux/build-tarball.sh /tmp/out
```

Environment variables, as on macOS and Windows: `LM_TIER` (`free` or `pro`), `LM_BUILD_NUMBER`,
`CARGO_TARGET_DIR`, and for Pro `LM_REFERENCE_DATA` (the built-in reference layers; there is no
default folder on Linux) and `LM_ACCOUNT_CONFIG` (the sign-in service's config). The assistant
key that macOS test builds take from the keychain isn't built into Linux tarballs.

The script stages the package in `dist/lm/stage-linux-<tier>/` and prints the tarball's sha256.
Nothing is signed: publish the sha256 beside the download.

## Running

```sh
tar -xzf NimbleMap-0.7.0-12-linux-x64.tar.gz
NimbleMap-0.7.0-12-linux-x64/layeredmap
```

The executable finds its examples, help and tier beside it in `Resources/`, so the folder can be
unpacked anywhere but must stay together. `README-linux.txt` in the tarball lists the run-time
libraries and the commands that add NimbleMap to the application menu and register `.lmap`.

From a checkout, `cargo run -p layeredmap` works too: without a `Resources` folder beside the
executable the app uses the checkout's examples.
