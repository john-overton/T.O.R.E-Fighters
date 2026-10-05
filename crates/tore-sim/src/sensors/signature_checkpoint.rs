//! The coders of an aircraft's signature profile, a copied record
//! (docs/formats/checkpoint.md): the radar and infrared signatures and the
//! aspect, deployment and reference weights. It never changes after the
//! import; a handoff copies it into state, so it is coded by value.
//! `Configuration` (the deployed fractions) is not state held anywhere
//! between ticks: the host supplies it to every step, so it has no coder.

use super::{Aspect, Deployment, Reference, SignatureProfile};

crate::checkpoint_struct!(Aspect {
    forward,
    side,
    vertical,
});

crate::checkpoint_struct!(Deployment { gear, flaps, bay });

crate::checkpoint_struct!(Reference {
    nominal_nmi,
    signature,
});

crate::checkpoint_struct!(SignatureProfile {
    radar,
    infrared,
    aspect,
    deployment,
    reference,
});
