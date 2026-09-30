//! Which resources the import keeps from `FA_1.LIB` and `FA_2.LIB` for the menus
//! and the debrief, besides everything the simulation needs.
//!
//! These lists are part of what the import writes, so they live with the
//! importer: a pack written by the game and a pack written by the dedicated
//! server from the same media must hold the same resources. The game's menu and
//! debrief screens read the same lists to check that the pack has what they
//! draw.

/// Menu art and dialogs kept from `FA_1.LIB`.
pub const MENU_ART: &[&str] = &[
    "QUIKMIS3.PIC",
    "ORD_AIR3.PIC",
    "ROCKER00.PIC",
    "ROCKER01.PIC",
    "ROCKER02.PIC",
    "ROCKER03.PIC",
    "ROCKER04.PIC",
    "DIAL00.PIC",
    "DIAL04.PIC",
    "DIAL11.PIC",
    "DIAL13.PIC",
    "LIGHTON.PIC",
    "LIGHTOFF.PIC",
    "PANELFNT.PIC",
    "CHOOSEV.PIC",
    "CHOOSEAC.PIC",
    "CHOOSE3.PIC",
    "CHOOSEU.PIC",
    "CHOOSEM.PIC",
    "ACTDFLT.PIC",
    "ACTDFT0L.PIC",
    "ACTDFT0M.PIC",
    "ACTDFT0R.PIC",
    "ACTION0L.PIC",
    "ACTION0M.PIC",
    "ACTION0R.PIC",
    "ACTIOD0L.PIC",
    "ACTIOD0M.PIC",
    "ACTIOD0R.PIC",
    "FONTACT.PIC",
    "FONTACD.PIC",
    "MENUFONT.PIC",
    "BODYFONT.PIC",
    "ARMFONT.PIC",
    "SMLFONT.PIC",
    // The Sound/Music Prefs dialog (sound_screen.rs).
    "SNDPREF.PIC",
    "SLIDERV.PIC",
    "TOGGLE00.PIC",
    "TOGGLE01.PIC",
    "TOGGLE02.PIC",
    "TOGGLE03.PIC",
    "TOGGLE04.PIC",
];
/// Menu data and sounds kept from `FA_2.LIB`.
pub const MENU_DATA: &[&str] = &[
    "CHOOSEAC.DLG",
    "MAINMENU.MNU",
    "FMENUD.MNU",
    "&CLICK.11K",
    "&BUTTON.11K",
    "&TOGGLE1.5K",
    "&SWITCH.11K",
    // RWR warning tones (rwr_tone.rs); &RWRMISS.5K is never played.
    "&RWRLOCK.5K",
    "&RWRDTCT.5K",
    "&RWRIR.5K",
];
/// Debrief resources beyond the shared menu art, by archive.
pub const DEBRIEF_ART: &[&str] = &[
    "DEBSCR.PIC",
    "DEBSC3.PIC",
    "DEBSCU.PIC",
    "DEBSCV.PIC",
    "PANLFNT2.PIC",
    "PANELFNT.PIC",
    "BODYFONT.PIC",
    "BOLDFONT.PIC",
    "HEADFONT.PIC",
];
pub const DEBRIEF_DATA: &[&str] = &["BRIEFSCR.DLG", "QUICK.MT", "&ROCKUP.11K", "&ROCKDN.11K"];
