# Vault Reference Map — Hardware IPC Surface for dc34-scanner

Reverse-engineered from two reference clones under `C:\Users\ksgra\dc43 badge\`:

- `dc34-vault/` — the stock DC34 badge app (this is the **real, customized firmware**:
  it depends on `dc34-api`, has `genemenu.rs`/`tourmenu.rs`/`fido2.rs`/`bitmaps/`,
  and pulls `ux-api`, `bao1x-hal`, `usb-bao1x`, `blitstr2`, etc. as **git dependencies**
  pinned to a specific `xous-core` rev — see §5).
- `xous-core/` — the OS. Hardware services live in `libs/bao1x-hal`,
  `services/bao1x-hal-service`, `services/bao-video`, `libs/ux-api`, `libs/blitstr2`,
  `services/usb-bao1x`, `libs/bao1x-api`.
- `xous-core/apps-baosec/vault2/` — an **in-tree twin** of `dc34-vault` (same file
  layout, same code in the shared modules, minus the DC34-specific gene/tour/fido
  content and the `dc34-api` dependency). It was invaluable for cross-checking exactly
  which files are live vs. dead in `dc34-vault`, and its `Cargo.toml` uses plain
  `path = "../../..."` deps (no `rev` pins), which makes the *real* crate/feature
  wiring easier to read than `dc34-vault/Cargo.toml`'s git deps.

**Important gotcha found while mapping this**: `dc34-vault/src/ux/framework.rs` and
`dc34-vault/src/ux/icontray.rs` (and the identical files in `apps-baosec/vault2/src/ux/`)
are **dead code** — leftovers from an earlier Precursor-era version of vault that used
the classic `gam::Gam` windowing system and a crate called `usb_device_xous`. Neither
`gam` nor `usb_device_xous` appear anywhere in `dc34-vault/Cargo.toml`'s dependency
list, and neither `main.rs` nor the top-level `src/ux.rs` declare `mod framework;` or
`mod icontray;`. **The live UI code is `dc34-vault/src/ux.rs`** (top-level file, not
the `ux/` subdirectory), which uses `ux_api::service::gfx::Gfx` directly (no GAM — the
baosec board runs a single full-screen app talking straight to the `bao-video` graphics
service) and `usb_bao1x::UsbHid` for USB. Don't crib code from `src/ux/framework.rs` —
it won't compile against the current dependency set.

---

## 1. Camera + QR decode

**QR decode is *not* vault-internal and *not* a separate reusable library — it is
baked into the `bao-video` service itself**, triggered by a single opcode. The vault
app (and any other baosec app) only ever calls one client method and blocks on the
result; all camera control, frame capture, and QR decoding happen server-side in
`bao-video`.

### Client call (what dc34-scanner should call)

`dc34-vault/src/actions.rs:1296-1310` (`ActionManager::acquire_qr`):
```rust
pub(crate) fn acquire_qr(&mut self) {
    let qr_data = match self.gfx.acquire_qr() {
        Ok(qr_data) => qr_data,
        Err(e) => {
            log::error!("QR acquisition failed: {:?}", e);
            return;
        }
    };
    if let Some(meta) = qr_data.meta { log::info!("QR code metadata: {}", meta); }
    if let Some(qr_uri) = qr_data.content { /* ... parse qr_uri ... */ }
}
```
`self.gfx` is a `ux_api::service::gfx::Gfx` (connected via `Gfx::new(&xns)`).

### The API itself

`xous-core/libs/ux-api/src/service/gfx.rs:746-753`:
```rust
#[cfg(feature = "board-baosec")]
pub fn acquire_qr(&self) -> Result<QrAcquisition, xous::Error> {
    let acquisition = QrAcquisition { content: None, meta: None };
    let mut buf = Buffer::into_buf(acquisition).unwrap();
    buf.lend_mut(self.conn, GfxOpcode::AcquireQr.to_u32().unwrap())?;
    let response: QrAcquisition = buf.to_original()?;
    Ok(response)
}
```
This is a **blocking IPC call** (`lend_mut`) to the `Gfx` server (`SERVER_NAME_GFX`,
registered by `bao-video`, see below). It blocks until the camera captures a frame,
finds a QR pattern, and decodes it — or until the user presses any button, which
aborts the scan and returns `content: None`.

Types (`xous-core/libs/ux-api/src/service/api.rs:152-163`):
```rust
pub struct QrAcquisition {
    pub content: Option<String>,
    pub meta: Option<String>,
}
pub struct QrRender {              // used by the separate encode/render path, see below
    pub width: usize,
    pub top_left: crate::minigfx::Point,
    pub modules: Vec<bool>,
}
```
Opcode enum entry: `GfxOpcode::AcquireQr` (`#[cfg(feature = "board-baosec")]`,
`api.rs:103-104`). In `hosted-baosec` (desktop UI dev) builds, `acquire_qr()` is
stubbed to instantly return a canned `otpauth://` URI (`gfx.rs:866-871`) — useful for
UI testing without hardware.

### Server side — camera capture + decode (`xous-core/services/bao-video/src/main.rs`)

- Camera driver: `bao1x_hal::gc2145::Gc2145` (`libs/bao1x-hal/src/gc2145/gc2145.rs`),
  instantiated with `unsafe { Gc2145::new() }`. Init call:
  `cam.init(&mut i2c, bao1x_api::camera::Resolution::Res320x240)` (line 535); capture is
  kicked off asynchronously with `cam.capture_async()` (line 549) and the frame buffer
  is read back via `cam.rx_buf::<T>() -> &[T]` (line 649, `Gc2145::rx_buf`,
  `gc2145.rs:256`) once a `GfxOpcode::CamIrq` interrupt fires. Frame dimensions are
  `IMAGE_WIDTH x IMAGE_HEIGHT` (256x240 after slicing off a border from the raw
  320x240 sensor capture — see `cam.set_slicing(...)`, line 540); the Y channel is
  extracted from the raw sensor words into a plain `[u8; IMAGE_WIDTH*IMAGE_HEIGHT]`
  greyscale buffer called `frame` (lines 648-665).
- `bao1x_api::camera::Resolution` enum (`libs/bao1x-api/src/lib.rs:85-93`):
  `Res480x272 | Res640x480 | Res320x240 | Res160x120 | Res256x256`.
- QR decode library: **`rqrr`**, pinned via git in `bao-video/Cargo.toml`:
  ```toml
  rqrr = { git = "https://github.com/betrusted-io/rqrr.git", branch = "rv32-opt" }
  ```
  Decode call (`main.rs:718-724`):
  ```rust
  let mut img = rqrr::PreparedImage::prepare_from_greyscale(IMAGE_WIDTH, IMAGE_HEIGHT, |x, y| {
      frame[y * IMAGE_WIDTH + x]
  });
  let grids = img.detect_grids();
  if grids.len() == 1 {
      match grids[0].decode() {
          Ok((meta, content)) => { /* content: String, meta: rqrr::MetaData */ }
          Err(e) => { /* decode failed */ }
      }
  }
  ```
  `rqrr::PreparedImage::prepare_from_greyscale` input is `(width: usize, height: usize,
  pixel_fn: impl Fn(usize, usize) -> u8)`; output of `.decode()` is
  `Result<(rqrr::MetaData, String), rqrr::DeQRError>`.
- There is also a decoder "warm-up" pass on every `AcquireQr` call
  (`main.rs:454-479`) that decodes a baked-in test bitmap first, purely to page the
  swapped-out decoder code back into RAM before the real scan, improving perceived
  latency.
- QR **encoding** (rendering a QR code on-screen, e.g. to show a gene/token) is a
  separate, simpler path: the client builds a `qrcode::QrCode` (crate `qrcode = "0.12"`,
  used directly in `dc34-vault`, e.g. `main.rs:655-659`) and calls
  `Gfx::render_qr(&self, qr_stream: &Vec<bool>, qr_width: usize, top_left: Point)`
  (`gfx.rs:732-744`), which blits the modules to the screen server-side.

### Dependencies to reuse

```toml
# in bao-video's Cargo.toml, mirrors what dc34-scanner needs if it wants to
# decode QR itself instead of going through Gfx::acquire_qr()
rqrr = { git = "https://github.com/betrusted-io/rqrr.git", branch = "rv32-opt" }
nalgebra = { version = "0.33", default-features = false, features = ["libm"] }
libm = "0.2.8"
```
But in practice **dc34-scanner should not vendor `rqrr` or touch `Gc2145` directly** —
just depend on `ux-api` and call `Gfx::acquire_qr()`, exactly like vault does. The
camera/UDMA/I2C wiring in `bao-video` is intricate (retry logic, IRQ claiming, power
sequencing) and is explicitly owned by that one service — `bao1x-hal-service`
manages the shared I2C/UDMA underneath it, and `bao-video` is the sole client
permitted to drive the camera peripheral.

---

## 2. USB HID keyboard send

### The type

`dc34-vault/src/ux.rs:512` / `dc34-vault/src/actions.rs:90` (struct fields):
```rust
usb_dev: usb_bao1x::UsbHid,
```
Constructed with `usb_bao1x::UsbHid::new()` (`ux.rs:582`, `actions.rs:127`).

Defined in `xous-core/services/usb-bao1x/src/lib.rs:16-19`:
```rust
#[derive(Debug)]
pub struct UsbHid {
    conn: CID,
}
impl UsbHid {
    pub fn new() -> Self {
        let xns = xous_names::XousNames::new().expect("couldn't connect to XousNames");
        let conn = xns.request_connection_blocking(api::SERVER_NAME_USB_DEVICE)
            .expect("Can't connect to USB device server");
        UsbHid { conn }
    }
    ...
}
```

### Sending text (what vault actually calls)

`dc34-vault/src/ux.rs:484` (autotype) and `actions.rs:1353/1379/1410` (password
autotype from the item list):
```rust
self.usb_dev.send_str(to_type).ok(); // ignore USB errors
```
`send_str` (`usb-bao1x/src/lib.rs:138-148`):
```rust
pub fn send_str(&self, s: &str) -> Result<usize, xous::Error> {
    let serializer = UsbString { s: String::from(s), sent: None };
    let mut buf = Buffer::into_buf(serializer).or(Err(xous::Error::InternalError))?;
    buf.lend_mut(self.conn, Opcode::SendString.to_u32().unwrap())
        .or(Err(xous::Error::InternalError))?;
    let returned = buf.to_original::<UsbString, _>().or(Err(xous::Error::InternalError))?;
    match returned.sent {
        Some(sent) => Ok(sent as usize),
        None => Err(xous::Error::UseBeforeInit), // USB probably not connected
    }
}
```
It types a whole string via internal keycode-per-character translation
(`usb-bao1x/src/mappings.rs`). There is also a lower-level per-keystroke API:
```rust
/// Sends up to three keyboard codes at once (USB HID usage tables).
pub fn send_keycode(&self, code: Vec<UsbKeyCode>, auto_keyup: bool) -> Result<(), xous::Error>
```
(`usb-bao1x/src/lib.rs:108-121`), where `UsbKeyCode = xous_usb_hid::page::Keyboard`.
Autotype pacing is controlled with `set_autotype_delay_ms(rate: usize)` (defaults to
30ms, must be re-set every boot).

### Is a keyboard class device always enumerated?

**Yes — the keyboard is always present, it is not requested/switched on demand.**
The composite HID class is built once at USB init time with *both* a boot keyboard
and the FIDO raw-HID interface, and there is no runtime API to remove the keyboard
interface on baosec:

`xous-core/services/usb-bao1x/src/hw.rs:72-75`:
```rust
let class = UsbHidClassBuilder::new()
    .add_device(NKROBootKeyboardConfig::default())
    .add_device(RawFidoConfig::default())
    .build(usb_alloc);
```
And the client-side "core switch" methods are stubbed as always-FidoKbd on this board
(`usb-bao1x/src/lib.rs:73-79`):
```rust
pub fn switch_to_core(&self, _core: UsbDeviceType) -> Result<(), xous::Error> { Ok(()) }
pub fn ensure_core(&self, _core: UsbDeviceType) -> Result<(), xous::Error> { Ok(()) }
pub fn get_current_core(&self) -> Result<UsbDeviceType, xous::Error> { Ok(UsbDeviceType::FidoKbd) }
```
`UsbDeviceType` (`usb-bao1x/src/api.rs:141-148`): `Debug=0, FidoKbd=1, Fido=2,
MassStorage=3, Serial=4, HIDv2=5` — these are leftover from the Precursor version
where the USB core really was switchable; on baosec it's fixed at composite
`FidoKbd` + a CDC serial port (see `SerialPort::new_with_store(...)` a few lines
below the class builder in `hw.rs`).

### FIDO2 CTAP HID (separate from keyboard autotype)

`dc34-vault/src/ctap/main_hid.rs` implements the CTAP2 HID protocol (`MainHid`
struct, `process_hid_packet`) which rides over the same composite device's raw FIDO
HID interface (`RawFidoConfig`/`RawFido` in `hw.rs`) — this is unrelated to the
keyboard-report path used for autotype; it's Google OpenSK's CTAP-over-HID framing,
unchanged from upstream OpenSK. Not needed for a scanner app unless it also needs to
act as a FIDO authenticator.

---

## 3. Buttons

Physical buttons are delivered as **push/event messages**, not blocking polling, in
the vault app's main loop — but a **blocking** primitive also exists and is simpler
for a single-purpose app like dc34-scanner.

### Button identifiers

`xous-core/libs/bao1x-hal/src/board/baosec.rs:395-407`:
```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyPress {
    Up,
    Select,
    Down,
    Left,
    Center,
    Right,
    #[cfg(feature = "accel-kbd")]
    Accel,
    Invalid,
    None,
}
```
`scan_keyboard(iox, rows, cols) -> [KeyPress; 4]` in the same file does the raw GPIO
matrix scan.

### How it becomes a `char` on the wire

`xous-core/services/bao1x-hal-service/src/servers/keyboard.rs:151-191`
(`KeyMapper::map_keypress`), respecting screen-flip orientation:
```rust
pub fn map_keypress(&self, kp: KeyPress) -> char {
    match kp {
        KeyPress::Down   => if self.orientation_flipped { '↑' } else { '↓' },
        KeyPress::Up     => if self.orientation_flipped { '↓' } else { '↑' },
        KeyPress::Left   => if self.orientation_flipped { '→' } else { '←' },
        KeyPress::Right  => if self.orientation_flipped { '←' } else { '→' },
        KeyPress::Select => '∴',
        KeyPress::Center => '🔥',   // "fire"/action button — deliberately not '\r'
        #[cfg(feature = "accel-kbd")]
        KeyPress::Accel  => '⏯',
        _ => '\u{0000}',
    }
}
```
This service registers as `bao1x_api::SERVER_NAME_KBD` (`"_Matrix keyboard driver_"`,
`bao1x-api/src/lib.rs:50`), implemented in
`bao1x-hal-service/src/servers/keyboard.rs:275`.

### Two consumption patterns

**A. Blocking (simplest — recommended for dc34-scanner):**

`xous-core/libs/bao1x-api/src/keyboard.rs:177-198`:
```rust
/// Blocks until a key is hit. Does not block the keyboard server, just the caller.
pub fn get_keys_blocking(&self) -> Vec<char> {
    match send_message(
        self.conn,
        Message::new_blocking_scalar(KeyboardOpcode::BlockingKeyListener.to_usize().unwrap(), 0, 0, 0, 0),
    ) {
        Ok(xous::Result::Scalar2(k1, k2)) => { /* decode up to 2 simultaneous keys into chars */ }
        Ok(_) | Err(_) => panic!("internal error: Incorrect return type"),
    }
}
```
Minimal "wait for next button" snippet:
```rust
let xns = xous_names::XousNames::new().unwrap();
let kbd = bao1x_api::keyboard::Keyboard::new(&xns).unwrap();
loop {
    for k in kbd.get_keys_blocking() {
        match k {
            '∴' => { /* select */ }
            '↑' => { /* up */ }
            '↓' => { /* down */ }
            '←' => { /* left */ }
            '→' => { /* right */ }
            '🔥' => { /* center/"fire" action button */ }
            _ => {}
        }
    }
}
```

**B. Event-driven listener (what vault actually uses, because it's already inside an
event loop serving other message types):**

`dc34-vault/src/main.rs` registers with the `Gfx` service instead of the keyboard
service directly, because `bao-video` (the `Gfx` server) itself is registered as a
keyboard listener and rebroadcasts key events to its own listeners alongside camera
UI concerns:
```rust
let gfx = Gfx::new(&xns).unwrap();
gfx.register_listener(SERVER_NAME_VAULT2, VaultOp::KeyPress.to_u32().unwrap() as usize);
```
Then in the main loop (`main.rs`, `VaultOp::KeyPress` arm):
```rust
Some(VaultOp::KeyPress) => xous::msg_scalar_unpack!(msg, k1, _k2, _k3, _k4, {
    let k = char::from_u32(k1 as u32).unwrap_or('\u{0000}');
    match k {
        '∴' => { /* open menu */ }
        '↓' => { vault_ui.nav(NavDir::Down); vault_ui.redraw(); }
        '↑' => { vault_ui.nav(NavDir::Up); vault_ui.redraw(); }
        '←' => { vault_ui.nav(NavDir::Autotype); vault_ui.redraw(); }
        '→' => { /* switch mode (Password <-> Totp) */ }
        _ => {}
    }
}),
```
(`bao-video` itself does `kbd.register_listener(SERVER_NAME_GFX, GfxOpcode::KeyPress...)`
in `services/bao-video/src/main.rs:259`, chaining Keyboard service → Gfx service →
vault app.)

For dc34-scanner, pattern **A** (`get_keys_blocking`) is much simpler and doesn't
require also depending on `ux-api`/`Gfx` just to get button events — use it unless
the app also needs to share a screen/event loop with QR scanning (`Gfx::acquire_qr()`
already handles button-abort internally, see §1).

---

## 4. Display drawing

Baosec apps do **not** use GAM (`gam`/window-manager) — that's a Precursor-only
concept. Baosec is single full-app-per-screen, and the app talks straight to the
`Gfx` client type (`ux_api::service::gfx::Gfx`), which is an IPC connection to the
`bao-video` service (`SERVER_NAME_GFX`, registered in
`services/bao-video/src/main.rs:217`). `modals` (used for simple notification/prompt
popups, e.g. `modals.show_notification(...)`) also runs with its `no-gam` feature on
this board.

### Canvas / screen type

`Gfx` (`xous-core/libs/ux-api/src/service/gfx.rs:11-13`):
```rust
pub struct Gfx { conn: xous::CID }
impl Gfx {
    pub fn new(xns: &xous_names::XousNames) -> Result<Self, xous::Error> { ... }
}
```
Construction, as used by vault (`dc34-vault/src/main.rs:108`):
```rust
let gfx = Gfx::new(&xns).unwrap();
```

### Minimal "draw a string at a position"

`TextView` (`xous-core/libs/ux-api/src/minigfx/textview.rs:83-151`) is the drawable
text object; `Gid` (`ux_api::service::api::Gid`) is a canvas handle — for a
full-screen no-GAM app you can use `Gid::dummy()`. From
`dc34-vault/src/ux.rs` (TOTP code rendering, ~line 289-313):
```rust
use core::fmt::Write as TextViewWrite;
use ux_api::minigfx::*;
use ux_api::service::api::Gid;
use ux_api::service::gfx::Gfx;

let mut tv = TextView::new(
    Gid::dummy(),
    TextBounds::CenteredTop(some_rectangle),   // or TextBounds::BoundingBox(rect), etc.
);
tv.style = GlyphStyle::Regular;                 // font, from blitstr2::GlyphStyle
tv.draw_border = false;
write!(tv, "{}", "hello world").ok();
gfx.draw_textview(&mut tv).expect("couldn't draw text");
gfx.flush().ok();                               // commit to the physical OLED
```
`Gfx::draw_textview` (`gfx.rs:305`):
```rust
pub fn draw_textview(&self, tv: &mut TextView) -> Result<(), xous::Error>
```
sends the `TextView` (which owns the `String` to render plus font/bounds/style) over
IPC to `bao-video`, which rasterizes it using `blitstr2` fonts and blits to the
SH1107 OLED framebuffer. Nothing is visible on the physical screen until
`gfx.flush()` (or `.redraw()`/`.draw()`, depending on API surface used elsewhere) is
called — drawing calls stage into an internal buffer.

Other primitives on `Gfx` worth knowing about: `draw_line`, `draw_rounded_rectangle`,
`draw_object_list` (batched rects/lines via `ObjectList`/`ClipObjectType`, faster than
individual calls), `clear()`, `bitmap(&[u32], top_left, bounding_box)` /
`bitmap_diffusion(...)` (raw 1bpp bitmap blit, used for the boot logo and icons —
`main.rs:111`: `gfx.bitmap(&bitmaps::dc_logo::BITMAP, None, None).ok();`),
`screen_size()`, `glyph_height_hint(style)`.

For simple modal popups (no manual layout), `modals::Modals::show_notification(msg,
qrtext)` is much less code (`services/modals/src/lib.rs:238-249`):
```rust
pub fn show_notification(&self, notification: &str, qrtext: Option<&str>) -> Result<(), xous::Error>
```

---

## 5. App entry + IPC

### Process entry point

`dc34-vault/src/main.rs` — standard Xous process `fn main() -> ! { ... }` pattern:
```rust
fn main() -> ! {
    log_server::init_wait().unwrap();
    log::set_max_level(log::LevelFilter::Info);
    log::info!("dc34-vault PID is {}", xous::process::id());

    let xns = xous_names::XousNames::new().unwrap();
    let sid = xns.register_name(SERVER_NAME_VAULT2, None).expect("can't register server");
    let conn = xous::connect(sid).unwrap();

    let gfx = Gfx::new(&xns).unwrap();
    gfx.clear().ok();
    gfx.bitmap(&bitmaps::dc_logo::BITMAP, None, None).ok();   // boot splash
    gfx.flush().ok();
    let tt = ticktimer_server::Ticktimer::new().unwrap();
    // ... spawn actions_sid, menu_sid, etc. via xous::create_server() + xous::connect()
    // ... loop { xous::receive_message(sid)... match opcode { ... } }
}
```
`SERVER_NAME_VAULT2` is defined in `dc34-api` (`"_Vault2_"`) — shared between the app
and anything that needs to talk to it. The main server loop (`sid`) handles
`VaultOp` opcodes (`vault_api.rs`) like `Redraw`, `KeyPress`, `HandleQr`, `AbortQr`,
etc.; a separate `actions_sid` server (spawned on its own thread) handles
long-running/blocking work (`ActionOp` opcodes: `AcquireQr`, `MenuAddnew`,
`ReloadDb`, ...) so it doesn't stall the main UI message loop. This
"one lightweight dispatcher thread + one or more worker-thread servers, each with its
own `xous::create_server()`/`SID`, wired together with `xous::connect()`/
`xous::send_message()`" is the standard Xous app shape and is a reasonable template
for dc34-scanner.

### Shared IPC/data types (`dc34-api` crate, `dc34-vault/../dc34-api/src/lib.rs`)

```rust
pub const SERVER_NAME_VAULT2: &str = "_Vault2_";
pub const POWER_MANAGER_SERVER: &'static str = "_dc34_pwr_mgr_";
pub const LED_SERVER: &'static str = "_oem_led_";

pub const DC34_DICT: &str = "dc34";           // PDDB dictionary name used by all badge apps
pub const DC34_SECRET: &str = "k0";
pub const DC34_BADGE: &str = "badge";
pub const DC34_GENE: &str = "gene";
pub const DC34_IMAGE: &str = "image";
// ...

pub enum BadgeType { Uber, Other, Community, Village, CtfContest, Human, Goon, None }
pub enum PowerManagerOp { Enable, Poll, MotionIrq, VbusIrq, KeyPress, Boot, ForceWfi,
                           ForceDeepSleep, PowerOff, ScreenOffRequest, FeedWdt, ... }
pub enum LedManagerOp { Autogamy, Force, GeneTest, SetGene, Syngamy, JackEyes, ... }

#[repr(C)]
pub struct Haploid { pub cd_period: u8, pub cd_rate: u8, ... }  // Pod/Zeroable, bytemuck-serializable
pub struct Diploid(pub [Haploid; 2]);   // the "light gene" exchanged via QR in the light-exchange protocol
impl Diploid {
    pub fn send(&self, conn: xous::CID, opcode: usize); // xous_ipc::Buffer::into_buf(...).lend(...)
    pub fn receive(mem: &xous::MemoryMessage) -> Self;
}

pub fn init_light_gene(badge_type: BadgeType);
pub fn get_light_gene() -> Option<Diploid>;   // reads from PDDB dict DC34_DICT / key DC34_GENE
pub fn save_light_gene(gene: Diploid);
pub fn save_k0(k0: &[u8; 32]);
pub fn get_k0() -> Option<[u8; 32]>;
```
If dc34-scanner needs to interoperate with vault's light-exchange protocol (reading
gene data, badge type, etc.) or needs the same `PowerManagerOp`/`LedManagerOp`
IPC opcodes, it should depend on `dc34-api` directly (`path = "../dc34-api"`) rather
than re-declaring these constants/types.

### Pinned xous-core rev

`dc34-vault/Cargo.toml:27` (and repeated on every `git = "https://github.com/betrusted-io/xous-core"` dependency line):
```
rev = "616bf65f6e379165464f50b1e79ec42aff77a683"
```
All of `persistent_store`, `locales`, `pddb`, `modals`, `userprefs`, `ux-api`,
`bao1x-api`, `bao1x-hal`, `bao1x-hal-service`, `bao1x-emu`, `blitstr2`, `usb-bao1x`,
`keystore`, `susres` (`xous-api-susres`), and the patched `aes` crate are pinned to
this exact rev. Note `dc34-vault/Cargo.toml` also has a **local override** section
(`[patch."https://github.com/betrusted-io/xous-core"]`, lines 104-115) that redirects
several of these to `../xous-core` (the local clone) for day-to-day development —
so in this checkout, the *actual* code compiled is whatever is in
`C:\Users\ksgra\dc43 badge\xous-core`, not necessarily what's at that pinned rev on
GitHub. Worth `git -C xous-core log -1` / diffing against the rev before assuming
parity, since it wasn't possible to check that here (no git commands were run per
task constraints).

`xous-usb-hid` is pinned separately, not to the xous-core rev:
```toml
xous-usb-hid = { git = "https://github.com/betrusted-io/xous-usb-hid.git", branch = "main" }
```
and `rqrr` (inside `bao-video`, not a direct vault dependency) similarly:
```toml
rqrr = { git = "https://github.com/betrusted-io/rqrr.git", branch = "rv32-opt" }
```

---

## Reuse notes for dc34-scanner

`dc34-scanner/Cargo.toml` is currently an empty skeleton (`src/main.rs` just prints
"dc34-scanner boot"). For a hardware build targeting the same badge, mirror
`dc34-vault/Cargo.toml`'s pattern — git deps pinned to the same
`616bf65f6e379165464f50b1e79ec42aff77a683` rev (or point at the local
`../xous-core` checkout via `[patch]`, as `dc34-vault` already does), plus a
`board-baosec` / `hosted-baosec` feature pair mirroring vault's:

```toml
[dependencies]
xous = "0.9.66"
xous-ipc = "0.10.6"
xous-names = { package = "xous-api-names", version = "0.9.67" }
log = "0.4.14"
log-server = { package = "xous-api-log", version = "0.1.65" }
ticktimer-server = { package = "xous-api-ticktimer", version = "0.9.65" }
num-derive = { version = "0.4.2", default-features = false }
num-traits = { version = "0.2.14", default-features = false }

# --- hardware surface needed per this map ---
ux-api        = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683" }   # Gfx client: acquire_qr(), draw_textview(), bitmap(), screen_size()
bao1x-api     = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683" }   # Keyboard client + KeyPress-adjacent types, camera::Resolution
bao1x-hal-service = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683", optional = true }  # only if you need Adc/other shared HAL services directly
blitstr2      = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683", features = ["bao1x"] }  # GlyphStyle used by TextView
usb-bao1x     = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683" }   # UsbHid::send_str/send_keycode, if scanner needs to type results
xous-usb-hid  = { git = "https://github.com/betrusted-io/xous-usb-hid.git", branch = "main" }   # UsbKeyCode etc. (transitive via usb-bao1x, but pin explicitly if used directly)
pddb          = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683" }   # if scanner needs persistent storage
modals        = { git = "https://github.com/betrusted-io/xous-core", rev = "616bf65f6e379165464f50b1e79ec42aff77a683", features = ["no-gam"] }  # simple notification popups
dc34-api      = { path = "../dc34-api" }   # only if interoperating with vault's SERVER_NAME_VAULT2 / light-gene / badge-type protocol

[patch."https://github.com/betrusted-io/xous-core"]
bao1x-hal         = { path = "../xous-core/libs/bao1x-hal" }
bao1x-hal-service = { path = "../xous-core/services/bao1x-hal-service" }
bao1x-api         = { path = "../xous-core/libs/bao1x-api" }
ux-api            = { path = "../xous-core/libs/ux-api" }
blitstr2          = { path = "../xous-core/libs/blitstr2" }
usb-bao1x         = { path = "../xous-core/services/usb-bao1x" }
pddb              = { path = "../xous-core/services/pddb" }
modals            = { path = "../xous-core/services/modals" }

[features]
board-baosec  = ["bao1x-hal-service", "ux-api/board-baosec", "blitstr2/board-baosec", "usb-bao1x/board-baosec"]
hosted-baosec = ["ux-api/hosted-baosec", "usb-bao1x/hosted-baosec"]   # desktop UI-dev mode; acquire_qr() returns canned data
```

Do **not** add `rqrr`, `nalgebra`, or touch `bao1x_hal::gc2145::Gc2145` directly —
that's `bao-video`'s job; scanner should call `Gfx::acquire_qr()` like vault does.
Do **not** add `gam` — it's not used on baosec at all.

For buttons, `bao1x-api::keyboard::Keyboard::get_keys_blocking()` is the simplest
integration and avoids needing to spin up an event-loop server just to read input
(see §3); reach for `Gfx::register_listener` only if the scanner's main loop already
needs to be message-driven for other reasons (e.g. concurrent camera + button
handling during a scan, which `acquire_qr()` already handles internally via
button-press abort).
