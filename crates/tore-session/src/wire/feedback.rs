//! Protocol 67 feedback. Only the host fills identity on the accepted reply.
use super::{
    WireError, WireResult,
    bits::{read_long_str, write_long_str},
};
use tore_codec::{BitReader, BitWriter};
pub use tore_replay::feedback::Feedback;
pub fn write(w: &mut BitWriter, f: &Feedback) -> WireResult<()> {
    if !Feedback::valid_text(&f.text) || f.player.len() > 128 || f.aircraft.len() > 64 {
        return Err(WireError::Invalid("feedback bounds"));
    }
    for n in [
        f.tick,
        f.submit_tick,
        f.opened_at,
        f.submitted_at,
        u64::from(f.seat),
        u64::from(f.plane),
    ] {
        w.write_varint(n);
    }
    for text in [&f.player, &f.aircraft, &f.text] {
        write_long_str(w, text);
    }
    Ok(())
}
pub fn read(r: &mut BitReader<'_>) -> WireResult<Feedback> {
    let f = Feedback {
        tick: r.read_varint()?,
        submit_tick: r.read_varint()?,
        opened_at: r.read_varint()?,
        submitted_at: r.read_varint()?,
        seat: u8::try_from(r.read_varint()?).map_err(|_| WireError::Invalid("feedback seat"))?,
        plane: u32::try_from(r.read_varint()?).map_err(|_| WireError::Invalid("feedback plane"))?,
        player: read_long_str(r, 128, "feedback player")?,
        aircraft: read_long_str(r, 64, "feedback aircraft")?,
        text: read_long_str(r, tore_replay::feedback::MAX_BYTES, "feedback text")?,
    };
    if !Feedback::valid_text(&f.text) {
        return Err(WireError::Invalid("feedback text"));
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::messages::Message;
    #[test]
    fn feedback_wire_round_trip_and_bounds() {
        let f = Feedback {
            text: "é🛩".repeat(500),
            tick: 12,
            ..Default::default()
        };
        let mut w = BitWriter::new();
        write(&mut w, &f).unwrap();
        let mut r = BitReader::new(w.as_bytes());
        assert_eq!(read(&mut r).unwrap(), f);
        let mut bad = f;
        bad.text.push('a');
        assert!(write(&mut BitWriter::new(), &bad).is_err());
        assert_ne!(
            Message::FeedbackSend(bad.clone()).kind(),
            Message::FeedbackAccepted(bad).kind()
        );
    }
}
