//! Personal testing mod that requests the host-capped attack reach override.

use mod_api::bindings::{
    Guest,
    cinnabar::extension::{gameplay, hud},
};

struct ReachTest;
const TEST_REACH_BLOCKS: f32 = 3.2;

impl Guest for ReachTest {
    fn init() {
        let _ = hud::set_label("Reach test: 3.2 blocks");
    }

    fn frame() {
        let _ = gameplay::set_attack_reach(Some(TEST_REACH_BLOCKS));
    }
}

mod_api::bindings::export!(ReachTest with_types_in mod_api::bindings);
