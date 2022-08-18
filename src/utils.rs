// TODO: remove inner and use tuple 0
macro_rules! get_set_field {
    ($get_name:ident, $set_name:ident, $typ:ident) => {
        pub fn $get_name(&self) -> $typ {
            $typ::from_le_bytes(self.$get_name)
        }

        pub fn $set_name(&mut self, value: $typ) {
            self.$get_name = value.to_le_bytes();
        }
    };
}

macro_rules! get_set_field_tuple {
    ($get_name:ident, $set_name:ident, $typ:ident, $start:expr, $size:expr) => {
        pub fn $get_name(&self) -> $typ {
            let mut buf: [u8; $size] = [0; $size];
            let start = $start;
            let end = start + $size;
            buf.clone_from_slice(&self.0[start..end]);
            $typ::from_le_bytes(buf)
        }

        pub fn $set_name(&mut self, value: $typ) {
            let value = value.to_le_bytes();
            let start = $start;
            let end = start + $size;
            for i in start..end {
                self.0[i] = value[i - start];
            }
        }
    };
}

pub(crate) use get_set_field;
pub(crate) use get_set_field_tuple;
