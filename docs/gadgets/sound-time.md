# Time and sound parts

Covers cStopWatch `Wtch`, cDigitalClock `DClk`, cMetronome `Metr`,
cTuningFork `Fork`, cTapePlayer `Tape` (the Recorder), cSpeaker `Spkr`,
cAnimalSound `AnSd`, cMiscSound `MsSd` (their common base is cNoiseMaker),
cSoundTransformer `SdTr` and cSoundSwitch `SdSw`. It also covers the
time base and the sound system as these parts use it. Addresses are in
`WIDGET.EXE`. Offsets like `+0x197` are offsets into the gadget object.
Read `docs/gadget-model.md` first. Where this file disagrees with it,
see "Corrections to gadget-model.md" at the end.

Conventions used below:

- `input(k)` is `0x41e56e`, which reads input knot `k` as a double (see
  the corrections for what it returns from object sources).
- `changed(k)` is `0x41e713`: it reads the changed flag of knot `k`'s
  wire and clears it.
- `value_changed(k)` is `0x41e62a`: it compares `input(k)` with the
  knot's cached value at `+0x156+8k`. If the value differs, it stores
  the new value and returns `wire != null`. Otherwise it returns false.
- `out_changed(k)` is `0x41e6cf`, which sets the changed flag of knot
  `k`'s wire.
- `invalidate()` is vtable slot 0x0c.
- `draw_broken()` is slot 0x10: the "broken" picture (sheet 101,
  `DAT_004a2694`), centred.
- `now()` is `0x424a89(doc)`, the document's `+0x4b`. It counts the
  ticks since the run started (see Time).
- `elapsed(n)` is `0x41e762(g, n)`:
  `if (now() - g[+0x152]) > n (unsigned) { g[+0x152] = now(); return true } else false`.
- `ftol` is `0x480ec4`, which truncates toward zero (it sets the
  rounding control to chop, FISTP qword, and keeps the low 32 bits).
- `clamp_int(x)` is `0x439e3d`: if `x >= 2147483647.0` it returns
  `0x7ffffffe`, if `x <= -2147483646.0` it returns `0x80000002`, and
  otherwise `ftol(x)`.
- `draw_frame(sheet, f, pt)` is `0x422d32`. It draws frame `f`
  (numbered from 1) with its rect offset by point `pt`.
- `pos` is the gadget's position at `+0x14`. `pos.v` is `+0x14` and
  `pos.h` is `+0x16`. A sub-picture at table point `p` (type 1010, read
  by `0x41141f`, entries `(v, h)`) is drawn at `(pos.v + p.v, pos.h + p.h)`.

Common to every part here: the dialog id, the 1022 combo strings and the
Doc text all use the id at `+0x28`. The Doc text is `WWGADGET 1020/<+0x28>`,
and the "Construction Tip" is `1020/<+0x28 + 2000>`. `WWGADGET 1021/<+0x28>`
is four u16s `{+0x28, 501 (502 for Spkr), +0x28, +0x28+2000}`. I could not
tell what the second u16 means.

The settings block fields are named by object offset:

| object offset | settings-block offset | meaning |
|---|---|---|
| `+0x35` | 0x00 | check box bits; bit i is the i-th check box in control-id order |
| `+0x39..+0x3c` | 0x04 | radio group 0..3 choice (u8) |
| `+0x3d,+0x41,+0x45,+0x49` | 0x08 | **combo box** 0..3 selection index (u32, read as u16) |
| `+0x4d + 60*i` | 0x18 | edit (text) field i |

See the corrections for how the dialog maps controls to these fields.

---

## 1. Time base

```
mac_ticks() (0x4113a8) = ftol( (double)(u64)timeGetTime() * 0.06 )   // DAT_004113cc = 0.06 (f64)
```

This is 1/60 s ticks taken from the millisecond clock, and it truncates.

The document keeps two fields. `+0x4f` is the run's origin and `+0x4b`
is the time since the origin.

- `0x42498f` runs once per idle loop while running (the tick):
  `if doc+0x4f == 0 { doc+0x4f = mac_ticks(); doc+0x4b = 0 } else doc+0x4b = mac_ticks() - doc+0x4f`.
  It then calls slot 0x44 on every gadget in list order. It stops early
  if run mode ends (`DAT_00493dde == 1`).
- `0x424a12` starts the run. It sets `doc+0x4b = 0` and calls slots 0x20
  (start) and 0x0c on every gadget. It is called from `0x430e70` (Run),
  which first plays UI sound 142 (`0x8e`).
- `0x424a94` stops or resets the run. It calls slot 0x1c (reset) on every
  gadget, sets `doc+0x4f = 0` and redraws. `0x430ee7` (Stop) first calls
  `0x440823` (close and reopen the sound device, which stops every
  playing sound), then `0x424a94`, then plays UI sound 143 (`0x8f`).
- `0x424944(doc, paused)` handles pausing while the app is inactive.
  When `paused` it stores `DAT_004931cc = mac_ticks()`. On resume it
  adds `mac_ticks() - DAT_004931cc` to `doc+0x4f`, so `now()` does not
  advance while the app is in the background. `0x4359f7` (app
  activate/deactivate) calls it and also closes the sound device on
  deactivate (`0x44083d`). It reopens the device on activate only if
  sound is on (`0x440837`).

So `now()` is unsigned-ish i32 ticks (1/60 s) since the first tick of
the run, frozen while the app is inactive. The Stopwatch and Metronome
use it. The Digital Clock uses the real wall clock instead:
`0x484748` = `time()` (it calls `GetLocalTime` and converts that to a
time_t) and `0x484308` = `localtime()`, which returns a C `struct tm*`
with `[0]` tm_sec, `[1]` tm_min and `[2]` tm_hour.

---

## 2. The sound system

### 2.1 Layers

| layer | what |
|---|---|
| `HWaveOut` (static object, init `0x43f94e`, which calls `0x43edce`; dtor `0x43f163`) | Reads the INI and allocates the mixing blocks. It owns the waveOut device and an 8-voice software mixer at 11025 Hz, 8-bit unsigned mono. |
| `cSoundOut` (1-byte object at `DAT_00498e04`, created by the document ctor `0x423268`; ctor `0x440144`, dtor `0x44028e`) | If the INI or command line says `NoSoundPlease`, it sets `DAT_00498e08 = 1` and every wrapper below does nothing. Otherwise it opens the device (`0x43f1a5`). It also makes 25 calls to the no-op `0x4407eb`. |
| wrappers `0x4402cc`..`0x440871` | These are what the gadgets call. See 2.4. |
| gDefaultSoundInit (static object init `0x4111ed`; its name string is at `0x4112d6`; dtor `0x411381`) | Loads the default tone (2.6) and the "broken" picture sheet 101. |

Global switches:

- `DAT_00493ddd` is the Sound On/Off menu (command `0x5e0`, handled in
  `0x430d2c`; on is the default). While it is 0, every
  play wrapper does nothing. Turning sound off plays UI sound 145
  (`0x91`) with flag 4 (2.3). Turning it on reopens the device and plays
  UI sound 144.
- `DAT_00498e08` is `NoSoundPlease`: no sound at all.
- `DAT_00496c64` is "simple sound" mode (INI `SimpleSoundPlease`, or the
  device's INI section has `No Low Level=1`). Sounds then go through
  `sndPlaySoundA` instead of the mixer. **A port can ignore this mode.**

### 2.2 INI ("Widget Sound System")

`0x43edce` runs at program start. It reads `WIDGET.INI` (path at
`0x4a2450`) through `0x41011b(key, default, section)`, which is
`GetPrivateProfileString` plus `%i`.

- `[general] WaveOutDevice` (default -1, the mapper) and `ShowDevices`
  (default 0; when set, it shows message boxes titled "Widget Sound
  System").
- `[<device name>] not compatible=1` disables low-level sound
  (message "Incompatible sound driver").
- `[<device name>] No Low Level=1` or `SimpleSoundPlease` selects simple
  mode.
- The keys below are read through `0x43e578(key, default)`. It reads the
  device-name section first, then `[default]`. The value is then clamped
  with `0x43eda5(min, &v, max)`:

| key | default | clamp | use |
|---|---|---|---|
| `SamplesPerSec` | 11 | none | Read, then **overwritten with 1**. The rate is always `0x2b11 = 11025`. |
| `Remix` | 2 | 1..2 | 1 means a new sound is mixed straight into the blocks already queued (low latency). 2 means it starts with the next block filled. |
| `GoodWavePos` | 0 | 0..1 | 1 means the play position comes from `waveOutGetPosition`. 0 means it comes from `timeGetTime`. |
| `WaveBlocks` | 2 | 2..6 | number of output blocks |
| `WaveBlockLen` | 1600 (0x640) | 0x200..0x7d00 | Bytes per block. -1 runs the calibration `0x43ea58` (0x2000 if that fails). |

After clamping, while `WaveBlocks*WaveBlockLen > 0xff00` the code
decrements `WaveBlocks` if it is above 2, and otherwise halves
`WaveBlockLen`. The defaults give two 1600-byte blocks, about 145 ms
each.

### 2.3 The mixer (port this behaviour, not the waveOut plumbing)

There are 8 voices at `DAT_004a2eb8`, 20 bytes each:
`{u32 free (1 = free), u8* data, u32 len, u32 pos, u32 flags}`. The
flags are:

- 1: free `data` when the voice ends or is stopped.
- 2: loop.
- 4: only used on play, see below.
- 8: done (internal).

```
play(data, len, flags)   // 0x43f5b8
  if device not open: open it (0x43f1a5); if that fails, return
  pump(); reclaim_done_voices()                          // 0x43f799, 0x43e76c
  if a "close-after" deadline is pending (DAT_00496c60 != 0): return  // sound is being shut off
  if Remix == 1: mix as much as fits into the current and next block right away (0x43e530/0x43e5ba),
                 and if anything is left (or it loops) add a voice whose pos is past the part already mixed
  else:          add a voice with pos 0 (0x43f559)
  if flags & 4: DAT_00496c60 = timeGetTime() + len*1000/11025 → the device is closed when that time passes
add_voice: take the first free voice of the 8. If none is free the sound is DROPPED,
           and a flag-1 buffer leaks.

fill_block(out, n)   // 0x43e660, called for each block before waveOutWrite
  out[0..n] = 0x80
  for each of the 8 voices that is active and not done:
     if loop:  mix data cyclically from pos, wrapping at len (0x43e5ba); pos = new offset
     else:     m = min(n, len-pos); mix data[pos..pos+m]; pos += m; if that reached len: flags |= 8 (done)
mix(out, src, m)  // 0x43e530, a saturating add done once per voice:
  for i: out[i] = clamp(out[i] + (src[i] - 128), 0, 255)

stop(data)   // 0x43f7c3: every active voice whose data == `data` is freed (its buffer too if flag 1).
             // With Remix==1 it also clears the two queued blocks to silence.
stop_all     // 0x43f39f closes the device and frees every voice; 0x43f1a5 reopens.
```

- **At most 8 sounds play at once.** A ninth `play` is silently dropped.
- **There is no volume control.** Every voice adds at full scale, and
  each addition saturates. Note that the clamp is applied after each
  voice, not once at the end.
- Done voices are reclaimed only at the start of the next `play`.

`0x43e88c` refills block `(cur+2) % N` on `MM_WOM_DONE` (window class
`"hwaveout"`, wndproc `0x43e943`, message `0x3bd`) and on every `pump`
(`0x44081d` → `0x43f799`). The app calls `pump` often from its message
loop (around `0x42b...`).

### 2.4 The wrappers (what gadgets call)

All of them do nothing when `NoSoundPlease` is set. The ones marked (*)
also do nothing when sound is off (`DAT_00493ddd == 0`).

`resource_sound(id)` is `0x43e4a4(id, &len, keep_header=0)`. It returns
the resource `WWSOUND 1006/<id>` past its 0x2c-byte RIFF header, with
`len = SizeofResource - 0x2c`. The game assumes every sound is a plain
44-byte-header 8-bit mono 11025 Hz WAVE. **The length counts every byte
after offset 44, including any trailing bytes beyond the `data` chunk's
declared size** (most WWSOUND resources have 20 extra bytes). Resources
come from `WWSOUND.DLL`. When it is not loaded, `0x41082a` returns
`WIDGET.EXE 1006/1` for any id. A missing id inside `WWSOUND` brings up a
retry/abort box in a loop, so in practice "missing" does not happen.

| addr | proposed name | does |
|---|---|---|
| `0x4402cc(id, close_after)` (*) | `PlaySoundId` | `play(resource_sound(id), len, close_after ? 4 : 0)` at 11025, no loop |
| `0x44073f(id)` (*) | `LoopSoundId` | `play(…, 2)` loops at 11025 |
| `0x440352(id)` | `StopSoundId` | `stop(resource_sound(id))` |
| `0x440384(buf)` | `StopSoundBuf` | `stop(buf)` |
| `0x4403ea(id, rate)` (*) | `PlaySoundIdAtRate` | if `rate >= 172`: `PlayBuffer(resource, len, rate, loop=0)` |
| `0x440787(id, rate)` (*) | `LoopSoundIdAtRate` | if `rate >= 172`: returns `PlayBuffer(resource, len, rate, loop=1)` |
| `0x4404a8(data, len, rate, loop)` (*) | `PlayBuffer` | see below |
| `0x440823` | `SoundRestart` | if sound is on: `stop_all` then reopen (used by Stop) |
| `0x44083d` / `0x440837` | `SoundClose` / `SoundOpen` | |
| `0x44081d` | `SoundPump` | |

```
PlayBuffer(data, len, rate, loop)    // 0x4404a8
  if rate == 11025: play(data, len, loop ? 2 : 0); return data
  if rate < 172:    return null                       // too low: nothing plays
  n   = MulDiv(len, 11025, rate)                      // Win32 MulDiv: rounds to nearest
  buf = alloc(n)
  for i in 0..n: j = MulDiv(i, len, n); if j >= len: j = len-1; buf[i] = data[j]   // 0x44044d, nearest neighbour
  play(buf, n, (loop ? 2 : 0) | 1)                    // freed when done or stopped
  return buf                                          // the handle to stop it with
```

**Pitch shifting is done this way: a sound is played at a sample rate
other than 11025, by resampling it to 11025 with nearest-neighbour
picking.** There is no other pitch, filter or volume processing. The
rate is a u16 everywhere, so it can be at most 65535.

### 2.5 Sounds through wires: the sound "object"

A part with a sound output returns true from slot 0x34 and a pointer
from slot 0x38 (the default reads `+0x14e`). The pointer is null when
the part is silent. It points to a **10-byte sound descriptor**, packed,
inside the gadget (always at `+0x197` for these parts):

| off | type | what |
|---|---|---|
| +0 | u32 | length in samples (bytes) |
| +4 | u16 | sample rate (Hz) to play it at |
| +6 | u8* | sample data, 8-bit unsigned mono |

The consumer (the Speaker, or the Sound Transformer) gets the source's
descriptor through the wire (`0x43aef3`) and copies or plays it. A
generic reader (`input(k)`) gets **1.0 if the pointer is non-null and
0.0 if it is null**. That 1.0/0.0 is what a Light Bulb or AND gate sees
when wired to a sound.

### 2.6 The default tone (gDefaultSoundInit)

`0x410d8c` runs from the static init `0x4111ed`:

```
res  = WWSOUND 1006/205 (0xcd)         // a ~220 Hz tone
tone      = { len = SizeofResource(res) (10681), rate = 11025, data = LockResource(res) }   // DAT_004a2670..
                                         // NOTE: the 44-byte RIFF header is NOT skipped
tone_oct  = { len = tone.len >> 3, rate = 11025, data = alloc; data[i] = tone.data[i*8] } // DAT_004a267a..
```

`make_tone(freq, seconds, desc)` (`0x410ea3`) is used by the Tuning Fork:

```
maxlen = tone.len; desc.data = tone.data
r = (1/220) * freq * 11025                  // the constant at 0x410f7f is an f80 equal to 1/220
if r > 65535: r *= 0.125; maxlen = tone_oct.len; desc.data = tone_oct.data
if r <= 65535: desc.rate = (u16)clamp_int(r); if desc.rate < 10: desc.rate = 10
else:          desc.rate = 0xffff
desc.len = min(clamp_int(desc.rate * seconds), maxlen)
```

The tone therefore starts at the resource's first byte, so the 44
header bytes play as samples, which makes a click.

An unreferenced function, `0x43ae9b`, builds a 0.2 s tone into a wire
(`wire+0x1b`) from a number through two calls to `0x481228`. Nothing
calls it, so it can be ignored.

### 2.7 Other sound code that is not for gadgets

`0x43f9d4`..`0x4400f3` (with `0x43fb9d`'s 256-entry pitch table and a
567-entry event list at `0x497138`) is a note sequencer. It is started
and stopped by `0x440853`/`0x440862`/`0x440871` from the title/intro code
(`0x429...`, `0x42b...`). No gadget uses it.

### 2.8 Recording (waveIn)

- `0x43e170(buf, n, progress_cb)` records once, blocking. It closes the
  output if it is open, then `waveInOpen` with the format at `0x496a78`:
  PCM, 1 channel, 11025 Hz, 11025 B/s, align 1, 8 bits. It fills `buf`
  with 0x80 and records one buffer of `n` bytes. It busy-polls
  `waveInGetPosition(TIME_BYTES)` and calls
  `progress_cb(pos*105/n, pos*105%n)`. The callback returns 2 to go on
  (1 aborts and the function returns -1). The loop ends at `pos >= n`.
  The function then stops, closes, reopens the output and returns `pos`,
  or 0 if no device or any waveIn call failed.
- `0x43e2db(buf, n)` starts continuous listening (the Sound Switch). It
  uses the same format with a single buffer, fills it with 0x80 and does
  not wait. If listening is already on, it does `waveInStop` then
  `waveInStart` and returns 1. It returns 1 on success.
- `0x43e446()` returns the bytes recorded since start, or -1 on error.
- `0x43e411()` stops listening (stop, unprepare, close) and reopens the
  output.

---

## 3. cStopWatch `Wtch`

- Fresh ctor `0x458220`, load ctor `0x45838c`, shared init `0x45813c`.
  Vtable `0x49bcb4`. Object size 0x1a0.
- `+0x28` is 1004, so the dialog is 1004 "Stopwatch", the Doc texts are
  1020/1004 and 1020/3004, and there are no 1022 strings.
- Sheet `WWGADGET 1004/1004` (12 frames). Knots 1014/1004, points
  1010/1004. The bounds are frame 1's rect, 85x84.

Knots:

| k | (v,h) | dir | meaning |
|---|---|---|---|
| 0 | (28,4) | in | START (level) |
| 1 | (79,4) | in | STOP (level) |
| 2 | (4,43) | in | RESET, the "R" on top (level) |
| 3 | (54,78) | out | output |

State:

| off | type | what |
|---|---|---|
| `+0x142` | i32 | displayed elapsed ticks |
| `+0x146` | f64 | number output |
| `+0x13e` | u8 | bool output |
| `+0x197` | u32 | the `now()` at which it last stopped (0 after a reset) |
| `+0x19b` | u32 | start offset: elapsed = `now() - +0x19b` |
| `+0x19f` | u8 | state: 0 running, 1 stopped, 2 reset (the initial state) |

The init sets `+0x19f=2`, `+0x197=+0x19b=0` and `+0x142=0`.

Settings: radio group 0 (`+0x39`), from dialog ids 107/108/109:
0 = "Seconds" (default), 1 = "Minutes", 2 = "Timer On/Off".

Slots:

```
0x24 has_bool   (0x45880f) = (+0x39 == 2)
0x2c has_number (0x458821) = (+0x39 < 2)
0x1c reset (0x4587da): base_reset(); snapshot_inputs() (0x41e4c8); +0x19f = 2; +0x197 = 0; +0x19b = 0

0x44 tick (0x458833):
  old = +0x142
  if input(2) != 0:  vt.reset()                                // runs on every tick while RESET is high
  else switch +0x19f:
    0 (running): if input(1) == 0: +0x142 = now() - +0x19b
                 else: snapshot_inputs(); +0x19f = 1; +0x197 = now()
    1 (stopped): if input(0) != 0: snapshot_inputs(); +0x19f = 0; +0x19b += now() - +0x197; +0x197 = now()
    2 (reset):   same as 1
  if +0x142/60 != old/60: invalidate()                          // integer division (C, truncating)
  oldnum = +0x146; oldbool = +0x13e                             // read AFTER any reset above
  switch +0x39:
    0: +0x146 = (double)(+0x142 / 60);   if +0x146 != oldnum: out_changed(3)   // whole seconds
    1: +0x146 = (double)(+0x142 / 3600); if +0x146 != oldnum: out_changed(3)   // whole minutes
    2: +0x13e = (+0x19f == 0);           if +0x13e != oldbool: out_changed(3)  // on while running
```

Notes:

- The inputs are levels, so if START and STOP are both high the state
  flips every tick.
- `+0x142` is not refreshed on the tick that starts the watch, nor while
  it is stopped.
- Because the old values are read after the reset, **a reset never
  marks the output changed**.
- `snapshot_inputs` only refreshes the `+0x156` value cache, which the
  stopwatch does not use otherwise.

Draw (`0x4583d7`):

```
draw_frame(1004, 1, pos)
m = +0x142 / 3600; s = (+0x142 / 60) % 60
if m < 100: frames 3+digit (frames 3..12 are the digits 0..9) at points 0..3: m/10, m%10, s/10, s%10   (MM:SS)
else:       draw_broken()
```

It draws the same way in edit mode, where it shows 00:00.

Saved data: none beyond the 0x150 base (slot 0x3c is the default).
Verified on DOGYEAR, HIMOM, RIDDLE and SLOTMACH, which all have size
0x150 and radio 0.

---

## 4. cDigitalClock `DClk`

- Fresh ctor `0x444ac5` (it calls reset), load ctor `0x444c49` (also
  calls reset), init `0x444a08`. Vtable `0x4995f4`. Size 0x199.
- `+0x28` is 1009, so the dialog is 1009 "Digital Clock" and the Doc
  texts are 1020/1009 and 3009.
- Sheet 1004/1009 (14 frames): 1 is the body (45x129), 2 is 1x1, 3 is
  "AM", 4 is "PM", and 5..14 are the digits 0..9. Knots 1014/1009.
  Points 1010/1009:
  `[(7,9) AM/PM, (24,11) H tens, (24,23) H units, (24,38) M tens, (24,50) M units, (24,65) S tens, (24,77) S units]`.

Knot 0, at (25,123), is an output (number).

Settings:

- Radio group 0 (`+0x39`, ids 106/107): 0 = "12 Hour" (default),
  1 = "24 Hour".
- Radio group 1 (`+0x3a`, ids 109/110/111): 0 = "Seconds since the
  minute" (default), 1 = "Minutes since the hour", 2 = "Hours since
  midnight".

State: `+0x146` f64 output. `+0x197` i16 is the tm_sec of the last run
draw.

```
0x2c has_number (0x445381) = true
0x20 start (0x445388): base_start(); out_changed(0); +0x197 = -1
0x44 tick (0x4453af):
  tm = localtime(time())                 // real local wall-clock time
  old = +0x146
  switch +0x3a: 0: +0x146 = tm.sec; 1: +0x146 = tm.min; 2: +0x146 = tm.hour
  if (int)(i16)+0x197 != tm.sec || +0x146 != old: invalidate()
  if +0x146 != old: out_changed(0)
```

The tick keeps invalidating until a draw has stored the new second.

Draw (`0x444cac`):

```
draw_frame(1009, 1, pos)
if not running (DAT_00493dde != 0): return           // edit mode shows only the body (printed 00:00:00)
tm = localtime(time())
if +0x39 == 0 (12 h):
   if tm.hour < 13: draw_frame(1009, 3 /*AM*/, pt0); h = (tm.hour == 0 ? 12 : tm.hour)
   else:            draw_frame(1009, 4 /*PM*/, pt0); h = tm.hour - 12
   // quirk: 12:xx noon shows "AM"
else: h = tm.hour                                    // no AM/PM drawn
+0x197 = tm.sec
digits (frame 5+d) at pt1..pt6: h/10, h%10, tm.min/10, tm.min%10, tm.sec/10, tm.sec%10
```

The hour's tens digit is always drawn, so 9 o'clock shows "09".

Saved data: none beyond the base. Verified on CLOCK.WGT, which has four
DClk with radios `[0,1]`, `[0,0]`, `[0,2]`, `[0,2]`.

---

## 5. cMetronome `Metr`

- Fresh ctor `0x4495ac` (it sets combo `+0x3d = 4`), load ctor
  `0x449722`, init `0x4494ec`. Vtable `0x499d70`. Size 0x1a3.
- `+0x28` is 1011, so the dialog is 1011 "Metronome", the combo strings
  are 1022/1011 and the Doc texts are 1020/1011 and 3011.
- Sheet 1004/1011: frames 1..3 are 90x69 pendulum pictures (1 swung
  right, 2 centre, 3 swung left) and frame 4 is 1x1. The bounds use
  **frame 2's** rect. Knots 1014/1011.

Knot 0, at (76,63), is an output (bool).

Settings: combo 0 (`+0x3d`, id 107). The period P in ticks comes from
the table at `0x499d48`:

| idx | string | P (ticks) |
|---|---|---|
| 0 | `1   /  60 sec.` | 3600 |
| 1 | `2   /  30 sec.` | 1800 |
| 2 | `5   /  12 sec.` | 720 |
| 3 | `10  /   6 sec.` | 360 |
| 4 | `30  /   2 sec.` (default) | 120 |
| 5 | `60  /   1 sec.` | 60 |
| 6 | `120 / 0.5 sec.` | 30 |

State:

| off | what |
|---|---|
| `+0x13e` | bool output |
| `+0x141` | u8 swing phase 0..3 (base reset sets 0) |
| `+0x197` | u32 P/2, unused |
| `+0x19b` | u32 P |
| `+0x19f` | u32 `now()` of the last on-tick, -1 at start |

```
0x24 has_bool (0x449823) = true
0x20 start (0x4497de): base_start(); +0x19b = P[+0x3d]; +0x197 = P >> 1; +0x19f = 0xffffffff
0x44 tick (0x449945):                         // all arithmetic unsigned 32-bit
  u = now() % (2P)
  old = +0x13e
  +0x13e = (u % P) < 15                      // a 15-tick (1/4 s) pulse at each beat
  if !+0x13e:
     if now()/P != +0x19f/P: +0x13e = 1; +0x19f = now()   // a beat was skipped (slow ticks): force one pulse
  else: +0x19f = now()
  s = u < P ? (u < P/2 ? 0 : 1) : (u < P + P/2 ? 2 : 3)    // P/2 = P >> 1
  if s != +0x141:
     +0x141 = s
     if (s & 1) == 0: PlaySoundId(1011, 0)    // WWSOUND 1006/1011, the "tick", at each beat
     invalidate()
  if +0x13e == 1 && old == 0: out_changed(0)  // only the RISING edge is signalled
```

The metronome plays its own tick and needs no Speaker. It plays nothing
on the beat at t=0, because `+0x141` is already 0.

Draw (`0x44982a`): `draw_frame(1011, [3,2,1,2][+0x141], pos)` (the table
is at `0x499d14`), so the pendulum swings left, centre, right, centre.

Saved data: none. Verified on CLOCK (combo 4) and SHOTMOON (combo 2).

---

## 6. cTuningFork `Fork`

- Fresh ctor `0x45c79b` and load ctor `0x45cb34` both call reset. Init
  `0x45c710`, vtable `0x49c494`, size 0x1a1.
- `+0x28` is 1008, so the dialog is 1008 "Tuning Fork", the combo
  strings are 1022/1008 and the Doc texts are 1020/1008 and 3008.
- Sheet 1004/1008: frames 1..6 are 65x73 (1 at rest, 2..4 vibrating,
  5..6 unused) and frame 7 is 1x1. Knots 1014/1008.

| k | (v,h) | dir | meaning |
|---|---|---|---|
| 0 | (32,4) | in | on when non-zero |
| 1 | (32,67) | out | sound |

Settings:

- Radio group 0 (`+0x39`, ids 106/107): 0 = "Preset:" (default),
  1 = "Custom:".
- Combo 0 (`+0x3d`, id 109) selects the preset. The frequency table is
  at `0x49c3f0`, as f64:
  440, 415.304, 391.996, 369.995, 349.228, 329.628, 311.127, 293.665,
  277.183, 261.626, 246.942, 233.082, 220. The strings are A (440 Hz),
  G#, G, F#, F, E, D#, D, C#, C, B, A#, A (220 Hz).
- Edit 0 (`+0x4d`, id 110, template text `"5#."`: at most 5
  characters, a real number, no minus; see
  [`../dialogs.md`](../dialogs.md) §1.5) holds the custom Hz. It is
  parsed by `0x4110c3`: `sscanf("%lf")`, 0 when unparseable, with the
  magnitude clamped to about 1e±300.
- The dialog-fill override `0x45cca9` only links the combo to "Preset:"
  and the edit to "Custom:" (working one selects its radio button; it
  enables or disables nothing, [`../dialogs.md`](../dialogs.md) §3.1),
  then calls the base fill.

The tone is **generated from a sample**, the default tone (2.6), and
not synthesised.

```
0x2c has_number (0x45cbbb) = true   // returns +0x146, which stays 0 (input() reads the object first anyway)
0x34 has_object (0x45cbc2) = true
init: +0x19d (desc.data) = tone.data
0x20 start (0x45ca67): base_start(); f = (+0x39 == 0) ? preset[+0x3d] : parse(+0x4d)
                       make_tone(f, 0.2, &+0x197)          // 0.2-second burst
0x44 tick (0x45cbc9):
  if input(0) == 0: +0x14e = null
  else:
     if +0x14e == null: out_changed(1); +0x141 = 1; invalidate()
     +0x14e = &+0x197
  if +0x141 != 0 && elapsed(10):                          // every >10 ticks
     +0x141 += 1; if +0x141 == 4: +0x141 = 0
     invalidate()
  if changed(0): out_changed(1)
```

Draw (`0x45c952`): `draw_frame(1008, +0x141 + 1, pos)`. Each time the
input turns on, the fork shows frames 2, 3, 4 (each for about 11 ticks)
and then returns to 1.

What you hear (through a Speaker): the speaker calls
`PlayBuffer(desc.data, desc.len, desc.rate)` once. Because
`desc.len = desc.rate*0.2`, that is 0.2 s at pitch f. The pitch goes
silent for f below about 3.4 Hz (rate < 172). From f > 1307.7 Hz it
switches to the octave sample, and the pitch stops rising at about
10461 Hz (rate is capped at 0xffff). The speaker plays it again on
every change notification (see Speaker), so a held input gives one
burst and not a continuous tone. For a continuous tone the designers
used the Speaker's number input instead.

Saved data: none. No FUNWIDGE example contains a Fork.

---

## 7. cTapePlayer `Tape` (the Recorder)

- Fresh ctor `0x45a1a0`, load ctor `0x45a30d`, init `0x45a057`. Vtable
  `0x49c080`, size 0x1a1.
- `+0x28` is 2026, so the dialog is 2026 "Recorder" and the Doc texts
  are 1020/2026 and 4026.
- Sheet 1004/1037: frame 1 is idle (40x68), 2 is playing, 3 is 1x1 and
  4 is the reel overlay (23x41). Knots 1014/1037. Point 1010/1037 is
  `[(3,14)]`.

| k | (v,h) | dir | meaning |
|---|---|---|---|
| 0 | (22,4) | in | play when non-zero |
| 1 | (22,62) | out | sound |

State:

- `+0x197` is the descriptor `{len = 40000, rate = 11025, data = +0x19d}`.
  `+0x19d` is a 40000-byte buffer allocated by the init and filled with
  0x80. That is about 3.63 s ("up to three seconds" in the Doc).
- The init sets `+0x14e = &+0x197`, so the object is non-null until the
  first reset or tick. A failed allocation sets it to null.
- `+0x141` is the u8 animation counter. `+0x140` is the broken flag:
  record and import clear it, and a failed recording sets it (the catch
  handler `0x459f9f` of Record; [`../dialogs.md`](../dialogs.md) §4.2).
  Nothing else sets it.

```
0x34 has_object (0x45a628) = true
0x3c saved_size (0x45a713) = 0x150 + 40000
0x40 save (0x45a6cf): base_save(); copy the 40000 sample bytes to chunk offset 0x150
load ctor: base_load(); init; copy 40000 bytes from chunk+0x150 into +0x19d
0x44 tick (0x45a62f):
  if input(0) == 0:
     +0x14e = null; if +0x141 != 0: invalidate(); +0x141 = 0
  else:
     if +0x14e == null: out_changed(1)
     +0x14e = &+0x197
     if elapsed(30): +0x141 += 1; invalidate()        // the reel blinks every ~31 ticks
```

The tick does not forward `changed(0)`. Compare AnSd.

Draw (`0x45a377`):

```
if +0x141 == 0: draw_frame(1037, 1, pos)
else: draw_frame(1037, 2, pos); if +0x141 odd: draw_frame(1037, 4, pos + pt0)
if +0x140: draw_broken()
```

Settings dialog 2026 has no settings fields. It has push buttons; the
push-button index counts BS_PUSHBUTTON/DEFPUSHBUTTON controls in id
order: 0 = OK(1), 1 = Cancel(2), 2 = "Record a Sound"(106) and
3 = "Import .WAV file"(107). Owner-drawn control 160 is a progress bar.
Both actions write **straight into the gadget's buffer**, so they happen
even if the dialog is then cancelled.

- Record (`0x459db3`, attached by the init: `+0x2f`/`+0x33 = 2`):

  ```
  tmp = alloc(40000); +0x140 = 0
  try:
    if +0x19d == null or tmp == null: throw
    SoundClose()
    n = record_once(tmp, 40000, cb)       // 0x43e170. cb 0x459d98 sets the progress bar (0..105) and returns 2
    progress(100); SoundOpen()
    if n == 0: SetFocus(OK)              // 0x41c1d9 + 0x417c89, off the button about to be disabled
    elif n != 0xffff (u16 compare):  +0x19b = 11025; n = min(n, 40000)
         +0x19d[0..40000] = 0x80; copy tmp[0..n] → +0x19d
    free(tmp)
    if n == 0: throw
  catch (0x459f9f):
    disable push button 2 ("Record a Sound") for the rest of the dialog
    +0x140 = 1                            // the broken picture
    AfxMessageBox("There was an error while recording sound")   // 0x49c04a
  ```

  The length stays 40000; anything not recorded is silence. A failed
  recording (no input device, any `waveIn*` error) leaves the buffer as
  it was. Full details in [`../dialogs.md`](../dialogs.md) §4.2.

- Import (`0x459fe8`, attached by the dialog-fill override `0x45a72e`
  as button 3) calls `import_wav(+0x19d, 40000, …)` (`0x43deda`) and
  clears `+0x140` on success:

  ```
  file dialog: title "Import from Wave format file", filter "Wave files (*.wav)"
  f = fopen(name, "rb"); read 44 bytes h
  require h[0..4]="RIFF", h[8..12]="WAVE", h[12..16]="fmt "   else "Bad file format"
  require bits (h+0x22, u16) == 8                            else "Can't import non-8 bit sound"
  ch = h+0x16 (u16); rate = h+0x18 (u32)
  // data is assumed to start at offset 44; no chunk walking (trailing chunks are read as samples)
  for i in 0..40000:
     b = fgetc(); if ch == 2: b = fgetc()        // keeps the 2nd (right) channel byte
     if b == EOF: break
     repeat (rate>=22000) + (rate>=30000) + (rate>=44000) times: b = the next frame, read the same way (EOF → break out)
     buf[i] = b                                  // the LAST frame of each group is kept
  buf[i..40000] = 0x80
  ```

  So 22050 and 44100 Hz files come out at about 11025. Any other rate
  plays at the wrong speed. The descriptor rate is not changed.

Saved data, after the 0x150 base: **40000 bytes, the raw samples,
unsigned 8-bit, mono, 11025 Hz**, with no length or rate field. The
chunk size is always 0x9d90. No FUNWIDGE example contains a Tape (the
"Tape" bytes in SHOTMOON.WGT are help text).

---

## 8. cSpeaker `Spkr`

- Fresh ctor `0x4539dd` and load ctor `0x453d05` both call reset. Init
  `0x453960`, vtable `0x49b392`, size 0x197 (no own fields).
- `+0x28` is 2042, so the dialog is 2042 "Speaker" (OK only, no
  settings) and the Doc texts are 1020/2042 and 4042.
- Sheet 1004/1038: frame 1 is the body (57x79), frame 2 is 1x1. Knots
  1014/1038: knot 0 at (15,4), an input.

Global tone channels: `DAT_004a330c[4]` holds the owning gadget id
(`+0x08`), 0 = free, and `DAT_004a331c[4]` holds the playing buffer.
They are shared by all Speakers, so **at most 4 continuous tones**
play at once.

```
0x1c reset (0x453d8c): base_reset(); DAT_004a330c[0..4] = 0      // the buffers are not stopped here (Stop restarts the device)
0x44 tick (0x453dbf):
  c = changed(0); v = value_changed(0)
  if !(c || v): return
  w = knot0.wire                              // non-null here
  if src(w).has_object:                       // a sound part
     d = src(w).object(knot)
     if d: PlayBuffer(d.data, d.len, d.rate, loop=0)   // one shot. Overlapping shots are allowed; nothing ever stops them
  elif src(w).has_number:  play_tone(fabs(input(0)))
  elif src(w).has_bool:    play_tone(src_bool(w) ? 220 : 0)
                                              // src_bool = 0x43adec: the bool, or number != 0, or object non-null

play_tone(f) (0x45381c):
  ch = the index i in 0..3 with DAT_004a330c[i] == +0x08
  if none: ch = the first i in 0..2 with DAT_004a330c[i] == 0, else 3     // channel 3 gets stolen
  id = 220; r = f/220 * 11025                 // f80 constant 1/220 at 0x453948
  if r > 65535: r *= 0.125; id = 224
  rate = r <= 65535 ? (u16)clamp_int(r) : 0xffff
  StopSoundBuf(DAT_004a331c[ch])
  if rate < 200 || f > 5512.0: DAT_004a330c[ch] = 0; DAT_004a331c[ch] = null      // silence
  else: DAT_004a330c[ch] = +0x08; DAT_004a331c[ch] = LoopSoundIdAtRate(id, rate)
```

The tone samples are `WWSOUND 1006/220`, a 220 Hz tone, and
`1006/224`, the same at 1760 Hz. Each is 2225 bytes after its header,
and the loop includes the 20 trailing bytes, so it is not seamless.
Tones play between about 4 Hz and 5512 Hz; a change in the number
input retunes the tone. Because the trigger is `changed || value_changed`,
a sound source triggers a shot when its object turns non-null (its
value goes 0→1) and whenever the source marks its output changed while
on.

Draw (`0x453b94`): `draw_frame(1038, 1, pos)`.

Saved data: none. Verified on CLOCK (4) and DOGYEAR (1).

---

## 9. cNoiseMaker base, cAnimalSound `AnSd`, cMiscSound `MsSd`

The two classes are identical except for their table and sound base.
cNoiseMaker has no vtable of its own in the constructors; the
constructors set `+0x1a1..+0x1ac = 0` (a cached resource handle triple
`{HMODULE, HRSRC, HGLOBAL}`).

| | AnSd | MsSd |
|---|---|---|
| ctors (fresh / load) | `0x4414e1` / `0x441679` | `0x44a31d` / `0x44a4ab` |
| init | `0x441410` | `0x44a24c` |
| vtable | `0x499116` | `0x499eba` |
| size | 0x1ad | 0x1ad |
| `+0x28`, dialog, Doc | 2040 "Animal Sounds", 1020/2040, 4040 | 2041 "Assorted Sounds", 1020/2041, 4041 |
| sheet, knots | 1004/1042, 1014/1042 | 1004/1043, 1014/1043 |
| knots | 0 (28,4) in; 1 (28,68) out (sound) | 0 (29,4) in; 1 (29,67) out (sound) |
| sound id | `200 + combo` | `300 + combo` |
| default combo (fresh) | 1 (Cat) | 0 (Buzzer) |

Settings: combo 0 (`+0x3d`, id 107).

- AnSd strings (1022/2040): 0 Dog → 1006/200, 1 Cat → 201,
  2 Rooster → 202, 3 Cow → 203.
- MsSd strings (1022/2041): 0 Buzzer → 300, 1 Alarm → 301,
  2 Chime → 302, 3 Clap → 303, 4 Breath → 304, 5 Laugh → 305,
  6 Cuckoo → 306, 7 Weird → 307, 8 Spooky → 308, 9 Chord → 309.

All of these are 8-bit mono 11025 Hz. 1006/204 does not exist, and
1006/205 is the default tone.

State: `+0x197` is the descriptor and `+0x140` is the broken flag.

```
0x34 has_object (0x4419a8 / 0x44a7ce) = true
0x20 start (0x44187b / 0x44a6a1):
   base_start()
   if +0x1a5 == 0: +0x1a1.. = find_resource(1006, base + +0x3d)  // cached until reset
   if +0x1a5 == 0: +0x140 = 1          // missing (unreachable in practice, see 2.4)
   else: desc = { data = res + 0x2c, len = size - 0x2c, rate = 11025 }
0x1c reset (0x441991 / 0x44a7b7): base_reset(); forget the cached handle (0x441962 / 0x44a788)
0x44 tick (0x4419af / 0x44a7d5):
   if +0x140 == 0 && input(0) != 0:
      if +0x14e == null: out_changed(1)
      +0x14e = &+0x197
   else: +0x14e = null
   if changed(0): out_changed(1)       // any input change while on re-triggers the Speaker
```

The sound is resolved at run start. A Stop resets every gadget, so the
next run picks up a changed choice.

Draw: AnSd (`0x441744`) draws `draw_frame(1042, [2,1,4,3][+0x3d], pos)`
(table at `0x4990d6`; sheet frames 1 cat, 2 dog, 3 cow, 4 rooster).
MsSd (`0x44a576`) draws `draw_frame(1043, 1, pos)`. Both then call
`draw_broken()` if `+0x140` is set. The picture follows the setting
even in edit mode.

Saved data: none. Verified on DOGYEAR (AnSd combo 0) and CLOCK (MsSd
combos 0, 3, 5, 2).

---

## 10. cSoundTransformer `SdTr`

- Fresh ctor `0x45301e`, load ctor `0x45318b`, init `0x452f24`. Vtable
  `0x49b300`, size 0x1aa.
- `+0x28` is 2038, so the dialog is 2038 "Sound Transformer", the combo
  strings are 1022/2038 and the Doc texts are 1020/2038 and 4038.
- Sheet 1004/1044: frame 1 is the body (30x100), 2 is 1x1, and 3..9
  are level bars of rising height (only 3..8 are used). Knots 1014/1044.
  Points 1010/1044 are `[(4,21),(4,34),(4,47),(4,60),(4,73)]`.

| k | (v,h) | dir | meaning |
|---|---|---|---|
| 0 | (14,4) | in | sound |
| 1 | (14,94) | out | sound |

Settings:

- Radio group 0 (`+0x39`, ids 106/107): 0 = "Play with No Changes"
  (default), 1 = "Transform Sound:".
- Check bit 0 (`+0x35 & 1`, id 108) is "Change Pitch:". Bit 1
  (`& 2`, id 110) is "Play the Sound Backwards".
- Combo 0 (`+0x3d`, id 109) picks the factor. The table is f64 at
  `0x49b2a0`:

| idx | string | factor |
|---|---|---|
| 0 | Up a Third | 1.333 (really a fourth) |
| 1 | Up a Fifth | 1.5 |
| 2 | Up an Octave | 2.0 |
| 3 | Down a Third | 0.75 |
| 4 | Down a Fifth | 0.667 |
| 5 | Down an Octave | 0.5 |

The dialog-fill override `0x4536ea` only links controls (it enables or
disables nothing): focusing the pitch list ticks "Change Pitch:", and
either check box selects "Transform Sound:"
([`../dialogs.md`](../dialogs.md) §3.3).

State:

| off | what |
|---|---|
| `+0x197` | output descriptor (a copy of the input's) |
| `+0x1a1` | u8* 40000-byte reverse buffer, or null |
| `+0x1a5..+0x1a9` | 5 bar levels, i8 0..6 |
| `+0x140` | broken flag (set on a bad rate; not drawn by this part) |

```
transform = (+0x39 != 0)
0x34 has_object (0x453467) = true
0x20 start (0x45346e): base_start(); +0x140 = 0
     if transform && (+0x35 & 2): +0x1a1 = alloc(40000); if null: +0x140 = 1      // not freed first: leaks on a double start
0x1c reset (0x45342b): base_reset(); free(+0x1a1); +0x1a1 = null
0x44 tick (0x4534c2):
  v = value_changed(0); c = changed(0)
  if c || v:
     d = source object via knot0.wire (null if the source has none)
     +0x14e = d
     if d == null: return                        // NOTE: skips out_changed AND the bar animation this tick
     copy 10 bytes *d → +0x197; +0x14e = &+0x197
     if transform && (+0x35 & 2):                // backwards
        n = min(len, 40000)
        for j = n-1 down to 1: buf[j] = src[n-1-j]   // buf = +0x1a1, src = d.data
                                                  // buf[0] is never written; src[n-1] is dropped
        data = +0x1a1                             // len is NOT clamped: a source longer than 40000 reads past the buffer
     if transform && (+0x35 & 1):                // pitch
        r = (double)rate * factor[+0x3d]
        if r <= 1.0 || r >= 65535.0: +0x140 = 1  // the rate stays as it was
        else: rate = (u16)clamp_int(r)           // truncates
     if c: out_changed(1)
  if elapsed(20):                                 // the bars wander all the time while running
     for i in 0..5: lvl[i] += 1 - (abs(rand()) % 3); clamp lvl[i] to 0..6   // rand = 0x415008
     invalidate()
```

`rand` (`0x415008`) is Borland's `rand` (`0x481530`): a 64-bit LCG with
`seed = seed*0x15A4E35 + 1` that returns the high 32 bits & 0x7fffffff,
mod 0x7fffffff. It is shared with every other random part, so it is
cosmetic here.

The result plays through a Speaker at the new rate, so pitch and length
change together (tape-speed style, through `PlayBuffer`'s resampling).

Draw (`0x453269`): `draw_frame(1044, 1, pos)`, then for each `i` with
`lvl[i] != 0`, `draw_frame(1044, lvl[i] + 2, pos + pt[i])`.

Saved data: none. Verified on DOGYEAR: two SdTr with radio 1, bits 0x1,
and combos 4 and 1.

---

## 11. cSoundSwitch `SdSw`

- Fresh ctor `0x452889`, load ctor `0x452a06`, init `0x45279c`. Vtable
  `0x49b216`, size 0x197.
- `+0x28` is 2037, so the dialog is 2037 "Sound Switch" and the Doc
  texts are 1020/2037 and 4037.
- Sheet 1004/1045: frame 1 is the body with "ON" (45x71), 2 is 1x1 and
  3 is the "OFF" label (21x9). Knots 1014/1045: knot 0 at (23,65), an
  output (bool). Point 1010/1045 is `[(12,40)]`.

Settings: radio group 0 (`+0x39`, ids 107/108/109): 0 = "High"
(default), 1 = "Medium", 2 = "Low". The threshold is `[7, 20, 50]`
(u16 at `0x49b1e0`, compared through the low byte).

Globals, shared by all Sound Switches:

| addr | what |
|---|---|
| `DAT_004a32f8` | the 22000-byte listen buffer. Every init allocates a new one, leaking the old. |
| `DAT_0049b1d8` | "output device must be closed before listening", initially 1 |
| `DAT_0049b1dc` | "done": listening is over for this run |
| `DAT_004a32f4` | instance count copy; written, never read |

```
0x24 has_bool (0x452e87) = true
0x20 start (0x452c09): base_start(); DAT_004a32f4 = instance count; done = 0
     if DAT_0049b1d8: SoundClose(); DAT_0049b1d8 = 0; listen_start(buf, 22000)   // only the first switch does this
0x1c reset (0x452c52): base_reset(); DAT_0049b1d8 = 1; listen_stop(); if sound on: SoundOpen()
0x44 tick (0x452c87):
  if !done && buf:
     p = (i16)listen_pos()                         // bytes recorded
     if p < 0: err = 1, on = false                  // no device / error (-1)
     else:
        err = 0
        n = count of i in 0..22000 with |buf[i] - 128| > threshold[+0x39]   // scans the WHOLE buffer, unrecorded part included (0x80)
        if p > 19800 (0x4d58): listen_stop(); listen_start(buf, 22000)       // refills with 0x80 and restarts
        on = n > 100
     if on != +0x13e || err != +0x140:
        +0x13e = on; done = 1; listen_stop(); DAT_004a32f4 -= 1
        if sound on: SoundOpen(); DAT_0049b1d8 = 1
        if on: out_changed(0)
        +0x140 = err; invalidate()
  elif +0x13e != 1: +0x13e = 1; invalidate()       // no out_changed here
```

Consequences, which a port should reproduce:

- The switch is a **one-shot latch**. The first time any Sound Switch
  hears more than 100 samples (out of about 2 s at 11025 Hz) beyond its
  threshold, listening stops. From the next tick on, **every** Sound
  Switch is forced ON until reset, because `done` is global.
- On a recording error, the switch first shows broken and OFF. On the
  next tick the `else` branch turns it ON, without `out_changed`.
- While listening, the output device is closed, though any sound played
  meanwhile reopens it.

Draw (`0x452a51`): `draw_frame(1045, 1, pos)`. If `+0x13e == 0` it also
draws `draw_frame(1045, 3 /*OFF*/, pos + pt0)`. If `+0x140` is set it
calls `draw_broken()`.

Saved data: none. No FUNWIDGE example contains one ("SdSw" in DOGYEAR
is not a chunk).

---

## 12. Sounds played directly by these parts (no Speaker)

| part | sound | when |
|---|---|---|
| Metronome | WWSOUND 1006/1011 | every beat after the first |
| (run start / stop, not a part) | 1006/142, 1006/143 | `0x430e70` / `0x430ee7` |

Every other part here makes a sound object and needs a Speaker.

---

## 13. Proposed names

| address | proposed name |
|---|---|
| 0x4113a8 | `MacTicks` |
| 0x424944 | `CWidgetDoc::PauseClock` |
| 0x42498f | `CWidgetDoc::Tick` |
| 0x424a12 | `CWidgetDoc::StartRun` |
| 0x424a89 | `CWidgetDoc::RunTime` |
| 0x424a94 | `CWidgetDoc::ResetRun` |
| 0x41e4c8 | `cGadget::SnapshotInputs` |
| 0x41e50a | `cGadget::Start` |
| 0x41e56e | `cGadget::Input` |
| 0x41e62a | `cGadget::InputValueChanged` |
| 0x41e762 | `cGadget::IntervalElapsed` |
| 0x439e3d | `ClampToInt` |
| 0x480e80 | `fabs` |
| 0x480ec4 | `__ftol` |
| 0x484748 | `time` |
| 0x484308 | `localtime` |
| 0x481530 | `rand_raw` |
| 0x415008 | `Random` |
| 0x4110c3 | `ParseDouble` |
| 0x41082a | `FindResourceAny` |
| 0x4111ed | `gDefaultSoundInit::gDefaultSoundInit` |
| 0x410d8c | `LoadDefaultTone` |
| 0x410e70 | `FreeDefaultTone` |
| 0x410ea3 | `MakeToneDesc` |
| 0x43deda | `ImportWaveFile` |
| 0x43dd54 | `OpenFileDialog` |
| 0x43e170 | `RecordOnce` |
| 0x43e2db | `ListenStart` |
| 0x43e411 | `ListenStop` |
| 0x43e446 | `ListenPosition` |
| 0x43e4a4 | `GetSoundResource` |
| 0x43e530 | `MixSaturate` |
| 0x43e5ba | `MixLooped` |
| 0x43e660 | `HWaveOut::FillBlock` |
| 0x43e76c | `HWaveOut::ReclaimDoneVoices` |
| 0x43e7c1 | `HWaveOut::SetPlayPosBase` |
| 0x43e7f7 | `HWaveOut::PlayPos` |
| 0x43e88c | `HWaveOut::RefillBlock` |
| 0x43e943 | `HWaveOutWndProc` |
| 0x43e97c | `HWaveOut::CreateCallbackWindow` |
| 0x43ea58 | `HWaveOut::CalibrateBlockLen` |
| 0x43edce | `HWaveOut::HWaveOut` (INI) |
| 0x43eda5 | `ClampInt` |
| 0x43e578 | `SoundIniInt` |
| 0x43f1a5 | `HWaveOut::Open` |
| 0x43f39f | `HWaveOut::Close` |
| 0x43f48e | `HWaveOut::Requeue` |
| 0x43f559 | `HWaveOut::AddVoice` |
| 0x43f5b8 | `HWaveOut::Play` |
| 0x43f799 | `HWaveOut::Pump` |
| 0x43f7c3 | `HWaveOut::Stop` |
| 0x43f881 | `SoundEndTime` |
| 0x440144 | `cSoundOut::cSoundOut` |
| 0x44028e | `cSoundOut::~cSoundOut` |
| 0x4402cc | `PlaySoundId` |
| 0x440352 | `StopSoundId` |
| 0x440384 | `StopSoundBuf` |
| 0x4403ea | `PlaySoundIdAtRate` |
| 0x44044d | `ResampleNearest` |
| 0x4404a8 | `PlayBuffer` |
| 0x44073f | `LoopSoundId` |
| 0x440787 | `LoopSoundIdAtRate` |
| 0x44081d | `SoundPump` |
| 0x440823 | `SoundRestart` |
| 0x440837 | `SoundOpen` |
| 0x44083d | `SoundClose` |
| 0x43ae9b | `cConnection::MakeTone` (unreferenced) |
| 0x43aef3 | `cConnection::SourceObject` |
| 0x43adec | `cConnection::SourceBool` |
| 0x45813c / 0x458220 / 0x45838c | `cStopWatch::Init` / `ctor` / `ctorLoad` |
| 0x4583d7 / 0x458769 / 0x4587da | `cStopWatch::Draw` / `dtor` / `Reset` |
| 0x45880f / 0x458821 / 0x458833 | `cStopWatch::HasBool` / `HasNumber` / `Tick` |
| 0x444a08 / 0x444ac5 / 0x444c49 | `cDigitalClock::Init` / `ctor` / `ctorLoad` |
| 0x444cac / 0x445310 / 0x445381 / 0x445388 / 0x4453af | `cDigitalClock::Draw` / `dtor` / `HasNumber` / `Start` / `Tick` |
| 0x4494ec / 0x4495ac / 0x449722 | `cMetronome::Init` / `ctor` / `ctorLoad` |
| 0x44976d / 0x4497de / 0x449823 / 0x44982a / 0x449945 | `cMetronome::dtor` / `Start` / `HasBool` / `Draw` / `Tick` |
| 0x45c710 / 0x45c79b / 0x45cb34 | `cTuningFork::Init` / `ctor` / `ctorLoad` |
| 0x45c952 / 0x45ca67 / 0x45cac3 | `cTuningFork::Draw` / `Start` / `dtor` |
| 0x45cbbb / 0x45cbc2 / 0x45cbc9 / 0x45cca9 | `cTuningFork::HasNumber` / `HasObject` / `Tick` / `FillDialog` |
| 0x45a057 / 0x45a1a0 / 0x45a30d | `cTapePlayer::Init` / `ctor` / `ctorLoad` |
| 0x45a377 / 0x45a585 / 0x45a628 / 0x45a62f | `cTapePlayer::Draw` / `dtor` / `HasObject` / `Tick` |
| 0x45a6cf / 0x45a713 / 0x45a72e | `cTapePlayer::Save` / `SavedSize` / `FillDialog` |
| 0x459db3 / 0x459fe8 / 0x459d98 | `cTapePlayer::OnRecord` / `OnImport` / `RecordProgress` |
| 0x453960 / 0x4539dd / 0x453d05 | `cSpeaker::Init` / `ctor` / `ctorLoad` |
| 0x453b94 / 0x453c9f / 0x453d8c / 0x453dbf / 0x45381c | `cSpeaker::Draw` / `dtor` / `Reset` / `Tick` / `PlayTone` |
| 0x441410 / 0x4414e1 / 0x441679 | `cAnimalSound::Init` / `ctor` / `ctorLoad` |
| 0x4416d5 / 0x441744 / 0x44187b / 0x441962 | `cAnimalSound::dtor` / `Draw` / `Start` / `ReleaseSound` |
| 0x441991 / 0x4419a8 / 0x4419af | `cAnimalSound::Reset` / `HasObject` / `Tick` |
| 0x44a24c / 0x44a31d / 0x44a4ab | `cMiscSound::Init` / `ctor` / `ctorLoad` |
| 0x44a507 / 0x44a576 / 0x44a6a1 / 0x44a788 | `cMiscSound::dtor` / `Draw` / `Start` / `ReleaseSound` |
| 0x44a7b7 / 0x44a7ce / 0x44a7d5 | `cMiscSound::Reset` / `HasObject` / `Tick` |
| 0x452f24 / 0x45301e / 0x45318b | `cSoundTransformer::Init` / `ctor` / `ctorLoad` |
| 0x4531c6 / 0x453269 / 0x45342b / 0x453467 | `cSoundTransformer::dtor` / `Draw` / `Reset` / `HasObject` |
| 0x45346e / 0x4534c2 / 0x4536ea | `cSoundTransformer::Start` / `Tick` / `FillDialog` |
| 0x45279c / 0x452889 / 0x452a06 | `cSoundSwitch::Init` / `ctor` / `ctorLoad` |
| 0x452a51 / 0x452c09 / 0x452c52 / 0x452c87 / 0x452e0a / 0x452e87 | `cSoundSwitch::Draw` / `Start` / `Reset` / `Tick` / `dtor` / `HasBool` |
| 0x41acb9 | `cGadgetDialog::OnInitDialog` (control scan, see the corrections) |
| 0x41abc2 | `FillCombosFromResource` |
| 0x41ca72 | `cGadgetDialog::SetButtonHandler` |
| 0x41caca | `cGadgetDialog::SetProgress` |

---

## 14. Corrections to gadget-model.md

1. **The number output is `+0x146` (f64).** The default slot 0x30
   (`0x41db39`) does `FLD qword [this+0x146]`. `+0x142` is an i32 class
   state, and the f64 at `+0x146` overlaps nothing before `+0x14e` (the
   object pointer). The base reset `0x41cef4` clears `+0x13e`, `+0x142`,
   `+0x146`, `+0x14a` and `+0x14e`. It also sets `+0x141 = 0`,
   `+0x13f = 1` and `+0x196 = 0`, and if `+0x140` is set it clears it
   and invalidates.
2. **`+0x140` is the "broken" flag.** Draw routines call slot 0x10 when
   it is set.
3. **`+0x152` is a timer stamp** for `0x41e762`
   (`IntervalElapsed(n)`).
4. **The "4 number fields" in the settings block are combo boxes.** The
   dialog setup (`0x41acb9`) scans control ids 1..0xa9
   and classifies each control by window class and style:

   | control | list | settings field |
   |---|---|---|
   | Button with style 0/1 (push) | `+0x5a` | none; indexable for `0x41ca72` handlers |
   | Button with style 2/3 (check box) | `+0x50` | bits at block 0x00, in id order |
   | Button with style 4/9 (radio) | grouped into `+0x64` | block 0x04 + group |
   | ComboBox | `+0x6e` | block 0x08 + 4i, the current selection index (u16 stored as u32) |
   | Edit | `+0x78` | text field i |
   | Static | `+0x82` | none |

   A radio button's group is the **last character of its caption**
   (e.g. `"Seconds1"`), which is stripped when the dialog is shown.
   Groups are numbered in order of first appearance. Owner-drawn
   buttons with ids 102 ("Pic", the part's picture), 103 (Doc), 150 and
   151 are special. Combo lists come from `WIDGET.EXE` type 1022 with
   the dialog's id (`0x41abc2`; msg 0x403 is CB_ADDSTRING, 0x401 is
   LB_ADDSTRING). The dialog id is the gadget's `+0x28`. `+0x2f` is an
   optional push-button handler `fn(dialog, gadget)` for push button
   number `+0x33`, attached by the base fill `0x41ea4d`.
5. **What `input(k)` returns depends on the source kind** (`0x41e56e`):
   - unwired: 0.0
   - object source: **1.0 if the object pointer is non-null, else 0.0**
     (not the object)
   - number source: the number
   - bool source: 1.0/0.0
   - a source with none of the three: -1.0

   `0x41e62a(g,k)` is "input value changed": it compares with the cache
   at `+0x156+8k`, updates the cache and returns true only if the knot
   is wired.
6. **Base start `0x41e50a` depends on `+0x13d`.** If `+0x13d == 0`
   (every part in this file) it only snapshots the current input values
   into the `+0x156` cache (`0x41e4c8`). It neither marks them "none"
   nor clears the wires' changed flags. Only when `+0x13d != 0` does it
   do what the model says.
7. **Time:** `DAT_004113cc` is the f64 0.06, and `0x4113a8` returns
   `ftol(timeGetTime()*0.06)`, truncating. The document's `+0x4b` is
   frozen while the app is inactive (`0x424944`).
8. **Doc texts are `WWGADGET 1020/<+0x28>`, plus a "Construction Tip"
   at `1020/<+0x28+2000>`.** `1021/<+0x28>` is four u16s
   `{id, 501/502, id, id+2000}`. The points list read by `0x41141f` is
   **type 1010**, not 1021. The comment on `gadget_info` in
   `crates/formats/src/tables.rs` names `0x41141f`, which actually loads
   `0x3f2` = 1010.
9. In the class table, the size of `Spkr` (0x197) is correct. Note that
   cNoiseMaker's own vtable never appears: its constructor is inlined
   into AnSd's and MsSd's.

## 15. Open questions

- Second u16 of type 1021 (501 / 502).
- `0x481228`, used only by the unreferenced `0x43ae9b`, was not
  identified.
- The INI file name behind `0x4a2450` was not checked.
- Simple-sound mode (`sndPlaySoundA`, synchronous, one sound at a time)
  is described only briefly. It seems irrelevant for a port.
