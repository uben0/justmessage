const MASK_0: u8 = 0b0000_0000;
const MASK_1: u8 = 0b1000_0000;
const MASK_2: u8 = 0b1100_0000;
const MASK_3: u8 = 0b1110_0000;
const MASK_4: u8 = 0b1111_0000;
const MASK_5: u8 = 0b1111_1000;

pub fn try_next_code_point<I, E>(iter: &mut I, err: E) -> Result<Option<char>, E>
where
    I: Iterator<Item = Result<u8, E>>,
{
    let Some(head) = iter.next() else {
        return Ok(None);
    };
    let head = head?;
    if head & MASK_1 == MASK_0 {
        return Ok(Some(head as char));
    }
    let (head, tail) = match () {
        () if head & MASK_1 == MASK_0 => (head & !MASK_0, 0),
        () if head & MASK_3 == MASK_2 => (head & !MASK_3, 1),
        () if head & MASK_4 == MASK_3 => (head & !MASK_4, 2),
        () if head & MASK_5 == MASK_4 => (head & !MASK_5, 3),
        () => return Err(err),
    };
    let mut code = head as u32;
    for _ in 0..tail {
        let Some(tail) = iter.next() else {
            return Err(err);
        };
        let tail = tail?;
        if tail & MASK_2 != MASK_1 {
            return Err(err);
        }
        code <<= 6;
        code |= (tail & !MASK_2) as u32;
    }
    Ok(Some(code.try_into().ok().ok_or(err)?))
}

pub fn next_code_point<I, E>(iter: &mut I, err: E) -> Result<Option<char>, E>
where
    I: Iterator<Item = u8>,
{
    let Some(head) = iter.next() else {
        return Ok(None);
    };
    if head & MASK_1 == MASK_0 {
        return Ok(Some(head as char));
    }
    let (head, tail) = match () {
        () if head & MASK_1 == MASK_0 => (head & !MASK_0, 0),
        () if head & MASK_3 == MASK_2 => (head & !MASK_3, 1),
        () if head & MASK_4 == MASK_3 => (head & !MASK_4, 2),
        () if head & MASK_5 == MASK_4 => (head & !MASK_5, 3),
        () => return Err(err),
    };
    let mut code = head as u32;
    for _ in 0..tail {
        let Some(tail) = iter.next() else {
            return Err(err);
        };
        if tail & MASK_2 != MASK_1 {
            return Err(err);
        }
        code <<= 6;
        code |= (tail & !MASK_2) as u32;
    }
    Ok(Some(code.try_into().ok().ok_or(err)?))
}

#[test]
fn test() {
    let original = "𝀷 helléç";
    let mut bytes = original.bytes();
    let mut chars = String::new();
    while let Some(c) = next_code_point(&mut bytes, ()).unwrap() {
        chars.push(c);
    }
    assert_eq!(original, chars);
}
