F/A-XX concept: experimental Fighters Anthology F-22 appearance replacement

This is a generated local mod candidate, not verified in the original game or
Kapset 3.0. It contains donor-derived SH files. Do not confuse it with the
retail-free developer/source kit. No rights to the original game's assets are
granted by this package.

Contents
  F22.SH: finless intact donor, split inboard flap leaves, added hidden hook.
  F22_A.SH and F22_C.SH: corresponding finless damaged bodies.
  F22.PT: donor definition with hook capability enabled.
  FAXX.LIB: these four modified files packed together, not another mod.
  export-report.json and validation.json: file hashes and static checks.

Target
  An F-22 replacement using the reviewed stock FA_2.LIB shapes. The donor
  aircraft definition with hook capability enabled, cockpit, textures, stores, flight model, fragments and
  shadow are supplied by the recipient's installation. There is no FAXX.PT or
  new aircraft registration. Select the existing F-22 after integration.
  Original F-22 handling is retained through the unchanged aerodynamic settings.
  We do not claim original FA handles like T.O.R.E's flight adapters.

Integration for the recipient's FA library maintainer
  Back up the existing installation/library first. Use your established FA
  library editor or override workflow to replace the four identically named
  SH/PT entries, keeping other resources intact. The loose files and FAXX.LIB are
  alternatives. Do not install both or rename this archive over a stock LIB.
  FAXX.LIB is an interchange archive; automatic loading of that filename is not
  established. Installation load order and Kapset resource compatibility need
  checking on your setup. Restore the backed-up four entries to undo the edit.
  Existing stock _F22 texture references are retained. A different Kapset skin
  or donor layout may need reconciliation. No Kapset files were available.

What the export implements
  Nearest-detail fin masks on the intact and two damaged bodies.
  Rudder +1 opens right leaves, -1 opens left, 0 closes both.
  Full leaf opening is +/-0.6 radians around the source flap hinge.
  Flap state -1 adds the authored 0.4-radian midpoint.
  Hook state 1 draws a rigid deployed hook; 0 draws no hook.
  The deployed hook bottom reaches source z=-23, the wheel-bottom plane.

Fitted export differences and limits
  Rudder, flap and hook animation uses discrete endpoint states. Continuous
  rudder opening and the T.O.R.E three-second hook motion are not exported.
  SH integer vertices round coordinates; hook half-width is at least one source
  unit so the thin shank does not collapse. The shoe is no longer wider than the
  shank at this precision. One source unit is about four inches.
  Original flap-state skin switches are held neutral. Only the seven authored
  inboard flap faces move; the donor's alternative flap/brake skins are bypassed.
  Lower-detail LOD jumps are bypassed to keep the reviewed finless near model
  visible at every distance. Original shadow and detached fragment shapes are
  unchanged. There is no new drag, finless stability, or arrestment simulation.
  H availability, rudder sign in live FA, palette appearance, drawing order and
  hook clearance in live FA remain unverified. Hook geometry is conditional on
  the original game actually supplying the hook state for this aircraft.

Validation
  The patched OpenFA exporter disables its emulated-x86 shape analysis.
  Our bounded data reader checked 24 gear/flap/rudder/hook combinations, retained
  neutral geometry and UVs, fin removal, and both damaged bodies. The LIB was
  unpacked and every payload compared byte-for-byte. No original game was run.
  These checks establish file construction and decoded geometry, not playability.

Rebuild
  Use tools/openfa_tools.py, tools/export_faxx.py and
  tools/validate_faxx_export.py from the accompanying developer source kit.
  See docs/fa-xx-developer-kit.md and docs/spec/fa-xx-export.md.
