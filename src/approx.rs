macro_rules! assert_approx {
    ($x:expr, $y:expr, $delta:expr) => {
        let difference = ($x - $y).abs();
        if difference > $delta {
            panic!("{} differs from {} by {}", $x, $y, difference);
        }
    };
}

pub(crate) use assert_approx;
