# dc34-scanner

A QR-code-to-keyboard scanner for the **DEF CON 34 badge** (Baochip-1x / Xous OS).

Point the badge's camera at a QR code — it decodes the contents, shows them on the
badge screen for confirmation, and then **types them into whatever has focus on your
computer** over USB, emulating a standard keyboard. No companion software, no drivers:
the badge just looks like a keyboard.

Handy for getting a URL, Wi-Fi string, or any text off a phone screen and into a
desktop app without retyping it.

---

## What it does

The app is a small four-state loop:

1. **Idle** — shows `Scan a QR - press any key`. Tap any button to start.
2. **Scanning** — the camera comes on and looks for a QR code. Point it at one.
3. **Preview** — the decoded text is shown on screen with a character count. Anything
   that can't be typed (non-ASCII) is flagged and skipped.
4. **Typing** — press the **middle button** (or push the jog-dial in) and the badge
   types the text into your focused window. Press **left** to cancel back to Idle.

Characters are sent as a US-QWERTY USB HID keyboard. Non-printable/non-ASCII characters
are skipped so a stray emoji in a QR can't derail the rest of the payload.

## Requirements

- A DEF CON 34 badge (Baochip-1x, `baosec` / `oem-baosec-lite` board).
- A build host with **WSL** (or Linux) and Rust — see [docs/BUILDING.md](docs/BUILDING.md).
- Willingness to run the badge in **developer mode** (see the warning below).

> [!WARNING]
> **Flashing custom firmware trips developer mode and permanently erases the badge's
> provisioned secrets** (light-exchange keys, and any vault/token data). This is a
> one-way change. Only flash this onto a badge you're happy to dedicate as a dev unit.
> You can always restore the stock firmware afterward, but the erased secrets do not
> come back.

## Install (quick version)

This is an **in-tree** app: it's built together with the Xous kernel so the two stay in
sync (a stock kernel will not boot a lone app image on this board). The short version:

1. Clone [`xous-core`](https://github.com/betrusted-io/xous-core) and set up the Xous
   Rust toolchain.
2. Copy `dc34-scanner/` into `xous-core/apps-baosec/dc34-scanner/` (with the in-tree
   `Cargo.toml`), register it in the workspace, and apply the one-line keyboard-layout
   patch.
3. Build the matched image:
   ```bash
   cargo xtask baosec-lite dc34-scanner
   ```
4. Put the badge in update mode (hold a button while resetting), copy the three
   generated `.uf2` files onto the `BAOCHIP` drive, and press a button to commit.

**Full step-by-step instructions, including the toolchain gotchas, are in
[docs/BUILDING.md](docs/BUILDING.md).**

## Repository layout

```
dc34-scanner/          The application (this is the deliverable)
  src/
    keymap.rs          US-QWERTY ASCII -> HID keycode table + typeable-char check
    payload.rs         Preview formatting (counts, truncation, untypeable flagging)
    state.rs           The four-state machine (pure, unit-tested)
    scanner.rs         QR capture via the bao-video Gfx::acquire_qr() service
    hid.rs             Typing via UsbHid::send_str()
    ui.rs              Screen rendering + button reading
    lib.rs             run_on_hardware(): wires the state machine to the badge
docs/
  BUILDING.md          Full build + flash + restore guide
  hardware-notes.md    Reverse-engineered map of the badge's IPC/hardware APIs
```

The pure-logic modules (`keymap`, `payload`, `state`) have no hardware dependencies and
are unit-tested on the host:

```bash
cd dc34-scanner && cargo test
```

## How it works (brief)

The badge already provides high-level services, so the app is mostly glue:

- **QR scanning** is a single blocking call, `Gfx::acquire_qr()`, served by the
  `bao-video` service (which owns the camera and an `rqrr`-based decoder).
- **Typing** is `UsbHid::send_str()`, served by `usb-bao1x` over the badge's
  always-present USB HID keyboard interface.
- **Buttons** arrive as `char`s from the `Keyboard` service; **display** is drawn with
  `Gfx` + `TextView`.

See [docs/hardware-notes.md](docs/hardware-notes.md) for the detailed API map.

## Credits

Built on the open-source work of the DEF CON 34 badge team and the Baochip / Precursor
ecosystem:

- [xous-core](https://github.com/betrusted-io/xous-core) — the Xous microkernel OS
- [dc34-vault](https://github.com/bunnie/dc34-vault) — the stock badge app (reference)
- [Baochip-1x](https://github.com/baochip/baochip-1x) — the SoC
- [DEF CON 34 badge page](https://defcon.org/34b/)

## License

MIT — see [LICENSE](LICENSE).
