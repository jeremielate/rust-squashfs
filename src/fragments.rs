use crate::{
    utils::get_set_field_tuple,
};

pub const FRAGMENT_ENTRY_SIZE: usize = 16;

#[derive(Debug)]
pub struct FragmentEntry([u8; FRAGMENT_ENTRY_SIZE]);

impl FragmentEntry {
    pub fn new(entry: [u8; FRAGMENT_ENTRY_SIZE]) -> Self {
        Self(entry)
    }

    get_set_field_tuple!(start_block, set_start_block, u64, 0, 8);
    get_set_field_tuple!(size, set_size, u32, 8, 4);
    get_set_field_tuple!(unused, set_unused, u32, 12, 4);
}