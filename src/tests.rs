
use crate::{superblock::Superblock, utils::get_set_field_tuple, SUPERBLOCK_SIZE};
use std::mem;

struct TestField([u8; 4]);

impl TestField {
    get_set_field_tuple!(test, set_test, u32, 0, 4);
}

#[test]
fn get_set_field() {
    let mut test_value = 43434331;
    let mut tf = TestField([0, 0, 0, 0]);
    tf.set_test(test_value);
    test_value = tf.test();
    assert_eq!(test_value, 43434331);
}

#[test]
fn superblock_size() {
    assert_eq!(mem::size_of::<Superblock>(), SUPERBLOCK_SIZE);
}
