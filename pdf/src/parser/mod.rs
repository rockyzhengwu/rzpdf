pub mod character;
pub mod crypto;
pub mod diagnostic;
pub mod options;
pub mod parse_utility;
#[expect(
    clippy::module_inception,
    reason = "preserve the public parser::parser API path"
)]
pub mod parser;
pub mod security;
pub mod syntax;
