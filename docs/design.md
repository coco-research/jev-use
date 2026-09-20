# Design

**Status:** draft
**Last updated:** 2026-09-19

Visual direction: **electric blue on white, modern iOS feel, real glass, light theme.**
Everything responsive, no fixed widths.

---

## 1. The one interaction

```
press Fn  ->  speak  ->  release  ->  it happens
```

There is no chat window to open and no mode to enter. The app is invisible until you
summon it, then it is entirely present.

**Push-to-talk, not wake-word.** No always-listening. Privacy and battery both demand it,
and "press to speak" is unambiguous about when the microphone is live.

## 2. The three states

| State | Looks like | Lasts |
| --- | --- | --- |
| **Idle** | Nothing. A menu-bar glyph. | until summoned |
| **Listening** | A glass pill, centred, waveform, live transcript | while the key is held |
| **Working** | The pill stays, showing the current step as text | 0.05-15 s |

The pill is the whole UI for v0.1. Everything else is settings.

## 3. The pill

```
+-----------------------------------------------+
|  (())  "what is the status on coco code"       |   <- live transcript, streaming
+-----------------------------------------------+
```

Then, on release, it becomes the answer:

```
+-----------------------------------------------+
|  cococode - main - 4 uncommitted files         |
|  last commit 20h ago: "feat(hooks): fail..."   |   <- the answer, not a summary of it
|  1 session "Brain" - grok-4.6 - 10383 turns    |
+-----------------------------------------------+
```

For a multi-step task it shows the step count instead:

```
+-----------------------------------------------+
|  step 2 of ? - CLICK "new tab"   0.96         |
+-----------------------------------------------+
```

Rules for the pill:
- **Glass**: `backdrop-filter: blur(24px) saturate(180%)` over `rgba(255,255,255,.72)`
- **Never more than 3 lines.** Long answers open a panel instead.
- **Always shows the confidence** when Jev decided. Hiding it makes a guess look certain.
- **Dismisses on Escape**, and on click outside.

## 4. Colour

| Token | Value | Use |
| --- | --- | --- |
| `--blue` | `#0A6CFF` | primary, focus, active |
| `--blue-deep` | `#0550C8` | pressed |
| `--soft` | `#E8F1FF` | fills, hover |
| `--ink` | `#0B0F1A` | text |
| `--mut` | `#5B6B85` | secondary |
| `--ok` | `#12A150` | healthy |
| `--warn` | `#B45309` | degraded |
| `--bad` | `#C2410C` | failed |
| `--glass` | `rgba(255,255,255,.72)` | surfaces |

Light theme only for v1. Background is a soft radial wash, not flat white, so the glass
has something to refract.

## 5. Type and space

- System font stack. `-apple-system, BlinkMacSystemFont, "SF Pro Text"`.
- Monospace only for paths, model ids, and commands.
- Radius 20-22px on surfaces. Anything sharper reads as a web form.
- Padding scales with `clamp()`, never a fixed pixel value.

## 6. Confidence is visible, always

Jev returns a number and the UI must show it. Below 0.70 is the act threshold, so:

| Confidence | Treatment |
| --- | --- |
| >= 0.90 | no badge, just act |
| 0.70 - 0.89 | small amber dot |
| < 0.70 | banner: "not sure - showing what I found instead of acting" |

**A sub-threshold decision is never executed silently.** This is a UI expression of R10.

## 7. Strictly avoided

- **No chatbot transcript.** A conversation log would imply memory and deliberation that
  are not there.
- **No spinner without a label.** It always says what it is doing.
- **No purple gradients, no glassmorphism-as-decoration.** The blur must sit over real
  content, or it is a flat panel pretending.
- **No emoji in the interface.**
- **No fixed width anything.** Every container responds to the display.

## 8. Accessibility

Full keyboard operation. `Escape` always dismisses. VoiceOver labels on the pill state.
Visible focus rings (`0 0 0 3px rgba(10,108,255,.14)`) - never `outline: none` without a
replacement.

## 9. The window itself

Transparent, borderless, rounded, always on top while active, and it does **not** steal
focus from the app you are working in unless the answer requires it.

Tauri on WKWebView. Real backdrop blur; the desktop shows through. See
`docs/architecture.md` section 6 for why not SwiftUI.
