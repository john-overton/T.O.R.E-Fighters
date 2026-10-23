//! The coders of an aircraft's signature profile, a copied record
//! (docs/formats/checkpoint.md): the radar and infrared signatures and the
//! aspect, deployment, external-store and reference weights. It never changes after the
//! import; a handoff copies it into state, so it is coded by value.
//! `Configuration` (the deployed fractions and external stores) is coded where
//! a target row holds it (`combat/live_checkpoint.rs`).

use super::{Aspect, Deployment, Reference, SignatureProfile, StoreWeights};

crate::checkpoint_struct!(Aspect {
    forward,
    side,
    vertical,
});

crate::checkpoint_struct!(Deployment { gear, flaps, bay });

crate::checkpoint_struct!(StoreWeights {
    per_store,
    per_1000_lb,
});

crate::checkpoint_struct!(Reference {
    nominal_nmi,
    signature,
});

crate::checkpoint_struct!(SignatureProfile {
    radar,
    infrared,
    aspect,
    deployment,
    stores,
    reference,
});
