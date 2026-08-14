# Building & flashing dc34-scanner

This app is built **in-tree** against [`xous-core`](https://github.com/betrusted-io/xous-core):
the app and the Xous kernel are compiled together into one matched image
(`loader.uf2` + `xous.uf2` + `swap.uf2`). This is required — on the `baosec` board, apps
run from a **swap** partition, and a stock kernel will not boot a standalone app image.
(An "out-of-tree" `apps.uf2` works for the `dabao` board but **not** for the badge.)

Everything below was verified on Windows + WSL (kali), rustc 1.97.1.

---

## 1. Prerequisites

- **Rust** (rustup). A recent stable works; these notes used 1.97.1.
- **WSL or Linux.** The Xous toolchain targets Linux; on Windows, build inside WSL.
- **A clone of xous-core** as your build tree.

```bash
git clone https://github.com/betrusted-io/xous-core
cd xous-core
# xous-core checks version tags during image creation; if you shallow-cloned, fetch tags:
git fetch --tags
```

### The Xous `std` gotcha

The target `riscv32imac-unknown-xous-elf` is a tier-3 target whose `std` is **not**
available through `rustup`. xous-core's tooling normally installs it, but if you hit
`error[E0463]: can't find crate for core`, install the prebuilt std manually, matched to
your **exact** rustc version:

```bash
# betrusted publishes a Xous std per rustc version:
#   rustc 1.97.1  ->  release tag 1.97.1.1  ->  asset riscv32imac-unknown-xous_1.97.1.zip
URL=$(curl -sL https://api.github.com/repos/betrusted-io/rust/releases/tags/1.97.1.1 \
      | grep -oE 'https://[^"]*riscv32imac-unknown-xous_1.97.1.zip' | head -1)
curl -sL -o /tmp/xousstd.zip "$URL"
unzip -oq /tmp/xousstd.zip -d "$(rustc --print sysroot)"   # zip already has the lib/rustlib/... layout
cat "$(rustc --print sysroot)/lib/rustlib/riscv32imac-unknown-xous-elf/RUST_VERSION"  # must equal rustc
```

If you bump rustc later, re-install the matching std zip (only versions betrusted has
published exist).

## 2. Add the app to the xous-core tree

1. Copy this repo's `dc34-scanner/src/` into `xous-core/apps-baosec/dc34-scanner/src/`.

2. Create `xous-core/apps-baosec/dc34-scanner/Cargo.toml` using **path deps** to the
   local crates (mirror `apps-baosec/vault2/Cargo.toml`):

   ```toml
   [package]
   name = "dc34-scanner"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   log = "0.4.14"
   num-derive = { version = "0.4.2", default-features = false }
   num-traits = { version = "0.2.14", default-features = false }
   xous = "0.9.66"
   xous-ipc = "0.10.6"
   log-server = { package = "xous-api-log", version = "0.1.65" }
   xous-names = { package = "xous-api-names", version = "0.9.67" }
   ticktimer-server = { package = "xous-api-ticktimer", version = "0.9.65" }
   ux-api = { path = "../../libs/ux-api" }
   bao1x-api = { path = "../../libs/bao1x-api" }
   blitstr2 = { path = "../../libs/blitstr2" }
   usb-bao1x = { path = "../../services/usb-bao1x" }
   bao1x-hal-service = { path = "../../services/bao1x-hal-service", optional = true }
   xous-usb-hid = { git = "https://github.com/betrusted-io/xous-usb-hid.git", branch = "main" }

   [features]
   default = []
   board-baosec = [
       "bao1x-hal-service",
       "ux-api/board-baosec",
       "blitstr2/board-baosec",
       "usb-bao1x/board-baosec",
   ]
   ```

   (Don't copy this repo's out-of-tree `build.rs` or `Cargo.lock` into the in-tree crate.)

3. Register the crate in `xous-core/Cargo.toml`, in `[workspace] members`, next to
   `"apps-baosec/vault2"`:

   ```toml
   "apps-baosec/dc34-scanner",
   ```

## 3. Keyboard-layout patch (required for correct typing)

`xous-core/services/bao1x-hal-service/src/servers/keyboard.rs` hardcodes the keymap to
**Dvorak** in its `GetKeyMap` handler. `usb-bao1x`'s `send_str()` reads that to choose its
HID keycode table, so on a QWERTY host all autotyped text comes out garbled
(`https://…` becomes `jkkr;Z[[gh…`). The `SelectKeyMap` opcode is `unimplemented!()`, so
this cannot be fixed at runtime from the app — patch the service:

```rust
// in the GetKeyMap opcode handler, change:
//   xous::return_scalar(msg.sender, KeyMap::Dvorak.into())...
// to:
xous::return_scalar(msg.sender, KeyMap::Qwerty.into()).expect("can't retrieve keymap");
```

(Use whichever `KeyMap` matches your host's physical layout.)

## 4. Build

From `xous-core/`:

```bash
cargo xtask baosec-lite dc34-scanner
```

The first build compiles the whole OS and takes a while. Artifacts land in:

```
xous-core/target/riscv32imac-unknown-xous-elf/release/
  loader.uf2
  xous.uf2
  swap.uf2
```

## 5. Flash

1. **Enter update mode:** hold any button on the badge while pressing reset / power-cycling.
   The screen shows an update state and the badge mounts as a USB mass-storage volume
   labeled **BAOCHIP**.
2. **Copy all three** `.uf2` files onto the BAOCHIP volume. Let each copy finish (flush/sync).
3. **Press any button on the badge to commit** the write. Skipping this leaves the swap
   partition partially written.
4. The badge reboots into the scanner (`Scan a QR - press any key`).

> The first custom flash trips developer mode and erases the badge's provisioned secrets
> (one-way). See the warning in the [README](../README.md).

## 6. Restore the stock firmware

Download the official image and flash its three `.uf2` files the same way:

- <https://ci.betrusted.io/releases/latest/baochip/dc34-badge/latest.zip>

Unzip it (it contains `loader.uf2`, `xous.uf2`, `swap.uf2`), enter update mode, copy all
three onto BAOCHIP, and press a button to commit.

## 7. Run the logic tests (host)

The pure-logic modules build and test on your normal host, no badge required:

```bash
cd dc34-scanner && cargo test
```
