mod text;
mod literals;
mod calls;
mod returns;
mod oracles;
mod probe_shapes;

pub(crate) use text::{slice_text, slice_macro_call_text, text_size_to_usize, is_predicate_operator, has_effect_text, is_effect_call_name};
