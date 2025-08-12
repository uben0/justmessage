// use pest::Parser;
// use pest_derive::Parser;

// pub enum Token {
//     Enter,
//     Leave,
//     Year(i32),
//     Month(u32),
//     Day(u32),
//     WeekDay(u32),
//     MonthDay(u32, u32),
//     YearMonthDay(i32, u32, u32),
//     HourMinute(u32, u32),
//     HourMinuteSecond(u32, u32, u32),
//     Number(u32),
// }
// pub fn next_token(input: &mut &str) -> Result<Token, ()> {
//     input.prefix(char::is_whitespace);
//     let word = input.prefix(char::is_alphabetic);
//     if !word.is_empty() {
//         return match word {
//             "enter" | "Enter" | "entra" | "Entra" => Ok(Token::Enter),
//             "leave" | "Leave" | "sale" | "Sale" => Ok(Token::Leave),
//             "monday" | "Monday" | "mon" => Ok(Token::WeekDay(1)),
//             "tuesday" | "Tuesday" | "martes" => Ok(Token::WeekDay(2)),
//             _ => Err(()),
//         };
//     }
//     let number = input.prefix(|c| c.is_digit(10));
//     if !number.is_empty() {
//         return Ok(Token::Number(number.parse().unwrap()));
//     }
//     todo!()
// }

// // pub fn prefix_while(input: &str, mut p: impl FnMut(char) -> bool) -> &str {
// //     match input.split_once(|c| !p(c)) {
// //         Some((prefix, _)) => prefix,
// //         None => input,
// //     }
// // }

// trait ParseExt<'a>: Sized {
//     fn prefix(&mut self, p: impl FnMut(char) -> bool) -> Self;
//     fn take_n<const N: usize>(&mut self) -> Option<[char; N]>;
//     // fn filter(self, p: impl FnMut(&'a str) -> bool) -> Option<&'a str>;
// }

// impl<'a> ParseExt<'a> for &'a str {
//     fn prefix(&mut self, mut p: impl FnMut(char) -> bool) -> Self {
//         match self.split_once(|c| !p(c)) {
//             Some((prefix, suffix)) => {
//                 *self = suffix;
//                 prefix
//             }
//             None => self,
//         }
//     }

//     fn take_n<const N: usize>(&mut self) -> Option<[char; N]> {
//         let mut chars = self.chars();
//         let array = [(); N].map(|()| chars.next());
//         for c in array {
//             if c.is_none() {
//                 return None;
//             }
//         }
//         *self = chars.as_str();
//         Some(array.map(|o| o.unwrap()))
//     }

//     // fn filter(self, mut p: impl FnMut(&'a str) -> bool) -> Option<Self> {
//     //     if p(self) { Some(self) } else { None }
//     // }
// }
// // impl<'a> ParseExt<'a> for Option<&'a str> {
// //     fn prefix(self, p: impl FnMut(char) -> bool) -> Self {
// //         self.map(|s| s.prefix(p))
// //     }

// //     // fn filter(self, mut p: impl FnMut(&'a str) -> bool) -> Option<&'a str> {
// //     //     self.filter(|s| p(*s))
// //     // }
// // }
