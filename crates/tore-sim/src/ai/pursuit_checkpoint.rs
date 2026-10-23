//! The coder of the AI's pursuit offset (docs/formats/checkpoint.md): the
//! steering offset the controller keeps around its pursued target. The rest of
//! the module is stateless (positions and attitudes passed in every tick).

use super::PursuitOffset;

crate::checkpoint_struct!(PursuitOffset {
    longitudinal_feet,
    lateral_feet,
    vertical_feet,
});
