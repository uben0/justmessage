use super::to_string;
use std::collections::HashMap;

#[test]
fn test_00() {
    assert_eq!(&to_string(&4).unwrap(), "+4");
    assert_eq!(&to_string(&(true, 'c')).unwrap(), "[true 'c']");
    assert_eq!(&to_string(&Some("hello")).unwrap(), "(some \"hello\")");
    assert_eq!(&to_string(&[3u8, 2, 0]).unwrap(), "[3 2 0]");
    assert_eq!(
        &to_string(&HashMap::from([('a', true), ('b', false)])).unwrap(),
        "(map ['a' true] ['b' false])"
    );
}
