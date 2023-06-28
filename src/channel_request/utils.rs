
// Need to add more conditions

pub fn check_if_valid(value: u128, upper: u128, lower: u128) -> () {
    let range = lower..upper;
    assert!(range.contains(&value));
}

