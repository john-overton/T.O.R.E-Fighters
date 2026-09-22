F/A-XX concept: experimental separate Fighters Anthology aircraft

Built from the retail F-22N (the carrier Raptor), not the F-22A. John confirmed
successful original FA flight and the floating-decal fix on 2026-09-18 for the
earlier F-22A-donor revision; this F-22N revision has not yet been flown in the
original game. Detailed controls/damage acceptance and Kapset 3.0 compatibility
remain unverified. It contains donor-derived SH files. Do not confuse it with the
retail-free developer/source kit. No rights to the original game's assets are
granted by this package.

Contents
  FAXX.PT: separate F/A-XX identity, retaining the donor F-22N settings and
    equipment, including its stock hook and carrier capability flags.
  FAXX.SH: finless intact shape, split flap leaves and the stock F-22N hook.
  FAXX_A.SH and FAXX_C.SH: finless damaged bodies.
  FAXX_B.SH, FAXX_D.SH and FAXX_S.SH: unchanged donor fragment/shadow aliases.
  FAXX.LIB: the same seven resources packed together.
  export-report.json and validation.json: file hashes and static checks.

Independent identity
  The package contains no F22- or F22N-named resource entries and does not
  overwrite either Raptor. FAXX.PT carries the displayed name F/A-XX Concept and
  refers to FAXX.SH and FAXX_S.SH. FA's type catalog enumerates PT files. The
  shadow filename also provides the base for the damaged/fragment family, so its
  FAXX name is necessary even though the shadow geometry is unchanged.
  Cockpit, HUD, textures, equipment and numeric aerodynamic settings remain the
  stock F-22N references/settings and require the recipient's own retail files.
  We do not claim FA handles like T.O.R.E's flight adapters.

Windows test installation
  1. Close Fighters Anthology and extract this ZIP outside the game directory.
  2. Copy only FAXX.LIB beside FA.EXE. Keep its filename. Do not also copy the
     loose PT/SH files. If FAXX.LIB already exists, stop and check that conflict.
  3. Start FA normally, with its installation folder as the working directory.
     In a Windows shortcut this is the "Start in" folder.
  4. Open Create Quick Mission and look for F/A-XX or F/A-XX Concept in the
     aircraft selector. It inherits the donor's availability/filter settings.
  5. Test selection, cockpit/flight startup, external appearance, rudder in
     both directions and hook deployment. Rudder poses are discrete endpoints;
     the hook is the donor's own, so it behaves as the F-22N's hook does.
  To uninstall: close FA and remove the FAXX.LIB file you added.

  Static inspection confirms that LibStartUp scans *.* and recognizes .LIB
  names, while the aircraft catalog enumerates *.PT entries. A special FA_5
  filename or a library merge is not needed for this unique resource family.
  This verifies discovery, not successful original-game loading or flight.
  The stock F-22A and F-22N remain separate aircraft. Kapset compatibility and
  existing FAXX resource collisions remain untested; no Kapset files were
  available.

What the export implements
  Nearest-detail fin masks on the intact and two damaged bodies, plus removal
  of both separate intact fin decals.
  Rudder +1 opens right leaves, -1 opens left, 0 closes both.
  Full leaf opening is +/-0.6 radians around the source flap hinge.
  Flap state -1 adds the authored 0.4-radian midpoint.
  PT capability flags are the donor's own $d3 and are left unchanged; the hook
  bit 0x02 is already set on the F-22N, so nothing is patched.
  The tail hook is the donor's stock F-22N hook: its two native faces draw at
  hook state 1 and nothing draws at 0. No hook geometry is authored here.
  The deployed hook bottom reaches source z=-23, the wheel-bottom plane.

Fitted export differences and limits
  Rudder and flap animation uses discrete endpoint states. Continuous rudder
  opening is not exported, and the hook keeps the donor's own two-state draw
  rather than T.O.R.E's three-second hook motion.
  SH integer vertices round coordinates. One source unit is about four inches.
  Original flap-state skin switches are held neutral. Only the seven authored
  inboard flap faces move; the donor's alternative flap/brake skins are bypassed.
  Lower-detail LOD jumps are bypassed to keep the reviewed finless near model
  visible at every distance. Original shadow and detached fragment shapes are
  unchanged. There is no new drag, finless stability, or arrestment simulation.
  Live appearance, rudder sign, drawing order and hook clearance on the F-22N
  donor remain to be checked in the original game. Use FA's hook control in
  flight; the capability flag alone does not make the hook visible.

Validation
  The patched OpenFA exporter disables its emulated-x86 shape analysis.
  Our bounded data reader checked 24 gear/flap/rudder/hook combinations against
  the F-22N donor projected at the same hook state, with no unexpected faces in
  any pose, and confirmed retained neutral geometry and UVs, fin and decal
  removal, and both damaged bodies. The hook was checked separately: hook 1
  minus hook 0 is exactly the donor's native quad. The LIB was unpacked and
  every payload compared byte-for-byte. No original game was run by the agent.
  These checks establish file construction and decoded geometry, not playability.

Rebuild
  Use tools/openfa_tools.py, tools/export_faxx.py and
  tools/validate_faxx_export.py from the accompanying developer source kit.
  See docs/fa-xx-developer-kit.md and docs/spec/fa-xx-export.md.
