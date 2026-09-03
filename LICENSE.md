# TexelBox End User License Agreement (EULA)

> **Legal notice:** This is a developer-drafted summary EULA for the TexelBox
> application. It is not legal advice and makes no guarantees. If you need
> formal legal coverage, consult a lawyer. The full, authoritative license for
> the *source code* is the MIT License in `LICENSE-MIT.md`; the *application
> binaries and assets* (the `texelbox-vx.x.x.exe` distributable, icons, and
> packaged textures) are governed by this EULA.

## 1. Ownership & License Grant

Copyright (c) 2026 Imad Eddine Aris. All rights reserved.

TexelBox is proprietary software. "TexelBox" means the application binary,
its bundled libraries, icons, default assets, and the officially distributed
executable named `texelbox-vx.x.x.exe`. The source code in this repository
(remains covered by the separate MIT License — see `LICENSE-MIT.md`).

Subject to the terms below, you are granted a license to use TexelBox under one
of the following tiers:

* **Free Tier:** Granted permanently at no cost. You may use all Free-plan
  features for personal and commercial use, subject to the feature limits
  shown inside the application. The Free Tier is fully functional but lacks
  Pro-only features (high-resolution outputs, rotation packing, DDS export,
  and others marked as Pro in the UI).
* **Pro Tier:** Requires a valid, paid, non-transferable license key issued
  by the licensor and activated through the in-application license manager.
  A Pro license grants you the extra features described in the application's
  "Build plan: Pro" documentation. A Pro license is not a resale
  authorization and does not entitle you to redistribute license keys.
* **Time-Limited Trial:** Does not require any paid license key. TexelBox
  auto-activates a 24-hour Trial that temporarily unlocks the full Pro
  feature set so you can evaluate it. The Trial cannot be extended,
  transferred, or exchanged for a Free/Pro entitlement, and it must be
  activated at least once while online so the licensor can record the
  device and start time.

The license granted here is **non-exclusive, non-transferable, and
revocable**. The licensor may revoke or suspend a license key or Trial if
abuse, sharing, or tampering is detected.

## 2. Restrictions

You may not:

* Reverse engineer, decompile, disassemble, or otherwise attempt to derive
  the source code of the *binary* (the source code itself is MIT-licensed;
  see `LICENSE-MIT.md`).
* Bypass, alter, or circumvent any license verification, Trial period
  controls, plan enforcement, or other technical restrictions embedded in
  TexelBox (including editing the `license-cache.json` file, faking system
  clocks, or tampering with the signed license cache). The license cache is
  cryptographically signed per device and time-checked; tampering renders it
  unusable and may disable all non-Free features.
* Modify, adapt, translate, distribute, or create derivative works of the
  binary, assets, icons, or packaged textures.
* Rent, lease, sell, sublicense, or commercially redistribute the
  application binary or any Pro license key without explicit written
  permission.
* Resell, share, or otherwise make your Pro license key or Trial access
  available to other parties.

## 3. Redistributable Builds

You may freely redistribute the **original, unmodified** executable
(`texelbox-vx.x.x.exe`) provided that:

* no fee is charged for the software itself (you may charge only
  reasonable media/carrier costs),
* it is distributed with all original copyright notices, this EULA, and the
  separate source-code MIT License intact, and
* it is clearly identified as the TexelBox **Free** Tier. Redistributing a
  Pro-unlocked or Trial-patched binary is not permitted.

Distributing modified versions of the binary, hacked license caches, or
cracked Pro unlocks is a material breach of this agreement and may
constitute a criminal offense under anti-circumvention law.

## 4. Updates, Online Features & Data

* **Update checks:** TexelBox periodically checks `https://texelbox-license.imadedar98.workers.dev/app/version`
  for new releases and stores a download URL in memory. No personal data is
  sent during this check beyond the running version string. No automatic
  download or install occurs.
* **License / Trial sync:** Activating or refreshing a license requires an
  online round-trip to the licensor's license server. TexelBox caches a
  cryptographically signed license/trial token locally so it can continue to
  operate in the active tier offline for up to 2 days while a token is still
  valid. After that grace window, an online sync is required to re-evaluate
  expiry or revocation.
* The locally cached `license-cache.json` only ever stores a signed token,
  a session handle, and (for Trials) a start/expiry timestamp. It is used
  locally to apply your plan and is not transmitted in full to any third
  party.

## 5. Third-Party Source

This repository publishes **source code** (MIT License). The MIT-licensed
source code can be freely forked, modified, and built, but **binaries built
from modified source may not be redistributed under the "TexelBox" name or
brand**, nor may the license-verification logic be stripped from any
distribution — redistributions of the compiled application must remain
gated by the same Free/Pro/Trial controls described here.

## 6. Disclaimer of Warranty / Limitation of Liability

TEXELBOX IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, AND NONINFRINGEMENT. THE LICENSEE
ASSUMES THE ENTIRE RISK AS TO THE QUALITY, PERFORMANCE, AND ACCURACY OF
THE SOFTWARE. IN NO EVENT SHALL THE AUTHOR OR COPYRIGHT HOLDER BE LIABLE
FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF
CONTRACT, TORT, OR OTHERWISE, ARISING FROM, OUT OF, OR IN CONNECTION WITH
THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## 7. Termination

This license is effective until terminated. It terminates automatically if
you fail to comply with any term above. On termination, you must destroy all
copies of the application binary and assets that you have distributed or
retained. Your obligation to maintain confidentiality of the source code's
MIT terms and the application's copyright notices survives termination.

---

Copyright (c) 2026 Imad Eddine Aris. All rights reserved.
