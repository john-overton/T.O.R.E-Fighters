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
/// The scroll bar's track, kept from `FA_1.LIB` with the menu art but not
/// required: the lobby pass added them after imports had been made, and the
/// scroll bar draws a flat track when a pack lacks them, so nobody has to
/// re-import for it. They are the Sound Prefs slider's `SLIDETOP` (34 by 9),
/// `SLIDEMID` (34 by 8) and `SLIDEBOT` (34 by 15). The knob, `SLIDERV`, is in
/// [`MENU_ART`].
pub const SLIDER_ART: &[&str] = &["SLIDETOP.PIC", "SLIDEMID.PIC", "SLIDEBOT.PIC"];
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

/// Pictures the multiplayer screens draw (Direct Connection, the lobby, the
/// host's setting dialogs, chat), kept from `FA_1.LIB`. Slice EF1;
/// `docs/formats/menu.md` ("Multiplayer connection screens") says which screen
/// draws each piece and with which palette. The game's pack check
/// (`tore-app/src/assets.rs`) requires them, the dedicated server does not.
///
/// Left out on purpose: `SERIAL3` and the modem and serial dialog panels
/// (`MODEM`, `MODEMCOM`, `MODEMSTS`, `SERIAL`, `COM`, `MODEM2`, `SERIAL2`),
/// `NETIPX`, `NETIPX2` and `NETDIR` (earlier-game leftovers and IPX), the
/// horizontal rocker `ROCKERH0` to `4` and `CHECK320` (the 320 by 200 mode):
/// no screen in the design shows them.
pub const MULTIPLAYER_ART: &[&str] = &[
    // Backgrounds. MODEM3 has its panel baked in; NETIPX3 has none (the panel
    // below is drawn on it).
    "MODEM3.PIC",
    "NETIPX3.PIC",
    // The panel kit: fill, four corners, two edge strips.
    "PANEL.PIC",
    "EDGETL.PIC",
    "EDGETR.PIC",
    "EDGEBL.PIC",
    "EDGEBR.PIC",
    "EDGELR.PIC",
    "EDGETB.PIC",
    // Lists, entry fields, page counter box and check boxes.
    "LISTLFT.PIC",
    "LISTMID.PIC",
    "LISTRT.PIC",
    "LISTHI.PIC",
    "EDITL.PIC",
    "EDITM.PIC",
    "EDITR.PIC",
    "PAGEBOX.PIC",
    "CHECK00.PIC",
    "CHECK01.PIC",
    "CHECK02.PIC",
    "CHECK03.PIC",
    "CHECK04.PIC",
    "CHECK05.PIC",
    "CHECK06.PIC",
    // Fonts: the dim panel font, the default button's pair, the status window
    // font and the ten pixel monospaced font typed into the entry fields.
    "PANELFND.PIC",
    "FONTDFT.PIC",
    "FONTDFD.PIC",
    "MPFONT.PIC",
    "WHEELFNT.PIC",
    // The disabled default button (Start, Call) and its cap.
    "ACTDFD0L.PIC",
    "ACTDFD0M.PIC",
    "ACTDFD0R.PIC",
    "ACTDFLD.PIC",
    // The menus' connected-state status window.
    "MPSTATUS.PIC",
    // The picture of the host's `MC_DLG` dialog.
    "MC.PIC",
];
/// Dialog layouts, menus and the quick-message source for the multiplayer
/// screens, kept from `FA_2.LIB`. Slice EF1.
///
/// Left out on purpose: `NETIPX2`, `NETIPX` and `NETDIR` (IPX and an earlier
/// game's), `SERIAL`, `MODEMCOM`, `MODEMSTS`, `MODLIST`, `COMLIST` (modem and
/// serial) and `FORTAIRB` (Airbase Assault's dialog, phase 2 at the earliest).
pub const MULTIPLAYER_DATA: &[&str] = &[
    // The network dialogs: Direct Connection, the lobby's players lists, the
    // options panel, the message prompt and the callsign dialogs.
    "NEWNET.DLG",
    "NETNEW.DLG",
    "NETJOIN.DLG",
    "NETTCP.DLG",
    "NETCEDT.DLG",
    "NETEDT.DLG",
    "NETBEDT.DLG",
    "CALLSIGN.DLG",
    "EDITSIGN.DLG",
    "MODEM.DLG",
    // The host's mission-setting dialogs (the King's settings, phase 2).
    "MC_DELAY.DLG",
    "MC_DIST.DLG",
    "MC_DLG.DLG",
    "MC_KILLS.DLG",
    "MC_KILLT.DLG",
    "MC_LIVES.DLG",
    "MC_NAME.DLG",
    "MC_NAT2.DLG",
    "MC_NAT.DLG",
    "MC_NATF.DLG",
    "MC_SCR.DLG",
    "MC_TIME.DLG",
    "MC_WETH.DLG",
    // Choose Activity's menu bar and the connection screens' "?" bar.
    "CHOOSEM.MNU",
    "MULTI.MNU",
    // The mission creator's menu bar; its Multiplayer menu (Time limit,
    // Number of kills, End scenario conditions, Number of revives, Revive time
    // delay, Revive distance) is what opens the `MC_*` dialogs above.
    "MC_MENU.MNU",
];
/// The pack's name for the retail `CHAT.TXT` (a loose file, in no archive): its
/// bytes as read, parsed by `tore_formats::chat`. Absent when the media has no
/// readable copy, in which case chat has no quick messages.
pub const CHAT_RESOURCE: &str = "TORE_CHAT_V1";
/// The loose file's name on the media.
pub const CHAT_FILE: &str = "CHAT.TXT";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_list_holds_unique_names_a_pack_can_store() {
        let lists = [
            MENU_ART,
            MENU_DATA,
            DEBRIEF_ART,
            DEBRIEF_DATA,
            MULTIPLAYER_ART,
            MULTIPLAYER_DATA,
            SLIDER_ART,
        ];
        for list in lists {
            let unique: BTreeSet<_> = list.iter().collect();
            assert_eq!(unique.len(), list.len(), "duplicate in a list");
            assert!(list.iter().all(|name| (1..=32).contains(&name.len())));
        }
        assert!(MULTIPLAYER_ART.iter().all(|name| name.ends_with(".PIC")));
        assert!(
            MULTIPLAYER_DATA
                .iter()
                .all(|name| name.ends_with(".DLG") || name.ends_with(".MNU"))
        );
    }

    #[test]
    fn the_multiplayer_lists_hold_every_piece_the_spec_names() {
        for name in [
            "MODEM3.PIC",
            "NETIPX3.PIC",
            "PANEL.PIC",
            "EDGETL.PIC",
            "EDGETR.PIC",
            "EDGEBL.PIC",
            "EDGEBR.PIC",
            "EDGELR.PIC",
            "EDGETB.PIC",
            "LISTLFT.PIC",
            "LISTMID.PIC",
            "LISTRT.PIC",
            "LISTHI.PIC",
            "EDITL.PIC",
            "EDITM.PIC",
            "EDITR.PIC",
            "PAGEBOX.PIC",
            "CHECK00.PIC",
            "CHECK06.PIC",
            "PANELFND.PIC",
            "FONTDFT.PIC",
            "FONTDFD.PIC",
            "MPFONT.PIC",
            "WHEELFNT.PIC",
            "ACTDFD0L.PIC",
            "ACTDFD0M.PIC",
            "ACTDFD0R.PIC",
            "ACTDFLD.PIC",
            "MPSTATUS.PIC",
        ] {
            assert!(MULTIPLAYER_ART.contains(&name), "{name}");
        }
        for name in [
            "NEWNET.DLG",
            "NETNEW.DLG",
            "NETJOIN.DLG",
            "NETTCP.DLG",
            "NETCEDT.DLG",
            "NETEDT.DLG",
            "NETBEDT.DLG",
            "CALLSIGN.DLG",
            "EDITSIGN.DLG",
            "MODEM.DLG",
            "MC_DLG.DLG",
            "CHOOSEM.MNU",
            "MULTI.MNU",
            "MC_MENU.MNU",
        ] {
            assert!(MULTIPLAYER_DATA.contains(&name), "{name}");
        }
        // The horizontal rocker is not used by any screen of ours.
        assert!(
            !MULTIPLAYER_ART
                .iter()
                .any(|name| name.starts_with("ROCKERH"))
        );
    }
}
