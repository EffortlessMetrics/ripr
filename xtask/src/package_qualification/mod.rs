mod command;
mod gate;
mod identity;
mod model;
mod parse;
mod render;

pub(crate) use command::run;

#[cfg(test)]
mod tests;
