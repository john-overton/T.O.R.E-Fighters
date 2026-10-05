//! The coder of the radio call memory (the radio section): when each shooter
//! last had an unguided hit announced and each aircraft last called a gun
//! hit. Stage H0's worked example of a section coder.

use super::Radio;

// `made` and `heard` are diagnostic counters that only tests read; they cost
// a few bits, so they are coded and a restored world compares equal.
tore_sim::checkpoint_struct!(Radio {
    unguided_hits,
    bullet_hits,
    made,
    heard,
});
