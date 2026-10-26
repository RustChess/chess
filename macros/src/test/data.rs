use crate::{Data, data};

#[data]
struct Datum {
    value: u8,
}

#[test]
fn derives_data() {
    fn assert_data<T: Data>() {}

    assert_data::<Datum>();
}
