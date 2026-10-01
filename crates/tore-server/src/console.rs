//! The console: commands typed on standard input, read on their own thread and
//! handed to the run loop. Commands are in docs/DEDICATED-SERVER.md, "Console,
//! status and logs".

use std::{
    io::BufRead,
    sync::mpsc::{Receiver, Sender, channel},
};

/// One console command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Status,
    Players,
    Kick(u8),
    /// Removes a player by its lobby id (as `players` shows it), with the
    /// reason it is told, which may be empty.
    KickPlayer(u8, String),
    End,
    Restart,
    Quit,
    Help,
    /// Something typed that is not a command, with what to say back.
    Invalid(String),
}

/// The help text for `help`.
pub const HELP: &str =
    "Commands: status, players, kick SEAT, kick-player ID [REASON], end, restart, quit";

/// Reads one line. Blank lines are nothing.
pub fn parse(line: &str) -> Option<Command> {
    let mut words = line.split_whitespace();
    let word = words.next()?;
    let rest: Vec<&str> = words.collect();
    let no_arguments = |command: Command| {
        if rest.is_empty() {
            command
        } else {
            Command::Invalid(format!("`{word}` takes no arguments"))
        }
    };
    Some(match word {
        "status" => no_arguments(Command::Status),
        "players" => no_arguments(Command::Players),
        "end" => no_arguments(Command::End),
        "restart" => no_arguments(Command::Restart),
        "quit" => no_arguments(Command::Quit),
        "help" | "?" => Command::Help,
        "kick" => match rest.as_slice() {
            [seat] => match seat.parse::<u8>() {
                Ok(seat) => Command::Kick(seat),
                Err(_) => Command::Invalid(format!("`{seat}` is not a seat number")),
            },
            _ => Command::Invalid("usage: kick SEAT".into()),
        },
        "kick-player" => match rest.split_first() {
            Some((id, reason)) => match id.parse::<u8>() {
                Ok(id) => Command::KickPlayer(id, reason.join(" ")),
                Err(_) => Command::Invalid(format!("`{id}` is not a player id")),
            },
            None => Command::Invalid("usage: kick-player ID [REASON]".into()),
        },
        other => Command::Invalid(format!("unknown command `{other}`. {HELP}")),
    })
}

/// Reads commands from `input` until it ends, sending each to the channel.
/// When the input ends (a service with no terminal) the server keeps
/// running; only `quit` or a signal stops it.
pub fn read_commands(input: impl BufRead, sender: &Sender<Command>) {
    for line in input.lines() {
        let Ok(line) = line else { return };
        if let Some(command) = parse(&line)
            && sender.send(command).is_err()
        {
            return;
        }
    }
}

/// Starts the thread that reads standard input.
pub fn spawn_stdin() -> Receiver<Command> {
    let (sender, receiver) = channel();
    let spawned = std::thread::Builder::new()
        .name("console".into())
        .spawn(move || read_commands(std::io::stdin().lock(), &sender));
    if let Err(error) = spawned {
        eprintln!("The console could not start: {error}");
    }
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_of_the_guide_parses() {
        assert_eq!(parse("status"), Some(Command::Status));
        assert_eq!(parse("  players  "), Some(Command::Players));
        assert_eq!(parse("kick 3"), Some(Command::Kick(3)));
        assert_eq!(
            parse("kick-player 4"),
            Some(Command::KickPlayer(4, String::new()))
        );
        assert_eq!(
            parse("kick-player 4 no  callsigns like that"),
            Some(Command::KickPlayer(4, "no callsigns like that".into()))
        );
        assert!(matches!(parse("kick-player"), Some(Command::Invalid(m)) if m.contains("usage")));
        assert!(
            matches!(parse("kick-player x"), Some(Command::Invalid(m)) if m.contains("player id"))
        );
        assert_eq!(parse("end"), Some(Command::End));
        assert_eq!(parse("restart"), Some(Command::Restart));
        assert_eq!(parse("quit"), Some(Command::Quit));
        assert_eq!(parse("help"), Some(Command::Help));
    }

    #[test]
    fn blank_lines_are_nothing_and_mistakes_are_answered() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   \t"), None);
        assert!(matches!(parse("kick"), Some(Command::Invalid(m)) if m.contains("usage")));
        assert!(matches!(parse("kick x"), Some(Command::Invalid(m)) if m.contains("seat number")));
        assert!(matches!(parse("kick 300"), Some(Command::Invalid(_))));
        assert!(matches!(parse("kick 1 2"), Some(Command::Invalid(_))));
        assert!(
            matches!(parse("quit now"), Some(Command::Invalid(m)) if m.contains("no arguments"))
        );
        assert!(matches!(parse("fly"), Some(Command::Invalid(m)) if m.contains("unknown command")));
    }

    #[test]
    fn the_reader_sends_commands_and_stops_at_the_end_of_input() {
        let (sender, receiver) = channel();
        read_commands("status\n\nkick 2\nquit\n".as_bytes(), &sender);
        let all: Vec<_> = receiver.try_iter().collect();
        assert_eq!(all, vec![Command::Status, Command::Kick(2), Command::Quit]);
    }
}
