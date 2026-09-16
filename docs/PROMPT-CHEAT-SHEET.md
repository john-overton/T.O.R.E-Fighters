# Prompting cheat sheet

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

How to ask for work on this project without sending it off after the wrong
target. Written after "replicate only native functionality" was read as the
executable's code path rather than the game's behaviour, and eighteen plan
revisions were built on it.

## The one line that matters

> Match what a player experiences, not what the original code did.

If a request is ever ambiguous between those two, the answer is always the first
one. Say so out loud and it cannot go wrong.

## Words that get misread

| Word you might reach for | What it means inside this repo | Say this instead |
| --- | --- | --- |
| native | a code path rebuilt from the original executable's control flow. A label describing origin, nothing more | "the original game's behaviour", "what the player sees" |
| functionality | ambiguous: the behaviour, or the code that produced it? | "behaviour", "what it does on screen" |
| replicate, reproduce | invites byte for byte reconstruction | "match", "make it behave like" |
| the original | the game, or the executable? | "the original game", or "the original executable" |
| recover, recovery | here it means static code recovery from the disassembly | "research what the game does" |
| source, source backed | reads as "taken from the disassembly" | "from a spec", "from evidence" |
| complete, full, exact | invites exhaustive reconstruction of a routine | "enough that a player cannot tell" |
| parity, 1:1 | fine, but it has been read both ways before | "parity by expression of feature" |

## Phrases that keep it on track

- "Would a player notice if this were different?"
- "Write the spec first, then implement it idiomatically."
- "Do not translate the original's control flow, caches or RNG order."
- "If the spec is missing a number, pick one, label it fitted, and tell me."
- "Provenance is a label, not a gate."
- "This is research mode." / "This is implementation mode."

## Openers you can paste

**Research a behaviour**

> Research mode. Work out what the game does when [X] and write it up as a prose
> spec in `docs/spec/` with the numbers a player would notice. Stop when you can
> write the spec. I do not need the routine fully decoded.

**Build something**

> Implementation mode. Read `docs/spec/[x].md` and build it idiomatically in the
> existing crates. Test against the spec's numbers. Do not translate the
> original's control flow, caches or RNG ordering.

**When you genuinely do want the disassembly traced**

> This one really is about the original executable: trace [routine] and record
> what it does. That is evidence for a spec, not a specification to implement.

**Ambiguity guard, safe to append to anything**

> If any of this is ambiguous between the game's behaviour and the original
> code, assume behaviour and say so in your report.

## Warning signs in the reply

If an agent's summary starts using this vocabulary, the old frame is creeping
back in:

- "gate", "does not close the gate", "remains open before acceptance"
- "whole tick", "runtime connected", "retail compared"
- "cannot be accepted until the native path is connected"
- "fitted substitute" used as if it were a defect

The correction is one sentence:

> Provenance is a label, not a gate. What would a player notice?

## What is already true, so you do not have to repeat it

These are in [AGENTS.md](../AGENTS.md) and apply to every task without being
asked for:

- Parity is by expression of feature. Native is a label, never a requirement.
- `spec-derived` is the default for gameplay code. Fitted and opinionated
  components are acceptable as shipped.
- No AI or autonomous behaviour work unless you ask for it.
- `F18.PT` is the F/A-18D and `RAFALE.PT` is the Rafale C, with no substitutions
  to make a test pass.
- Retail comparison is unavailable and never blocks progress.
- Default adapters and compatibility modes are not changed silently.
- Agent decisions are recorded as agent decisions, never attributed to you.
