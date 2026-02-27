use syn::{
    parse::{Parse, ParseStream},
    Ident, Token, parenthesized,
    punctuated::Punctuated,
};


#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub int_arg: Option<u32>,       // For @pulse(4096)
    pub string_arg: Option<String>, // [V4.0] For @string_prefix("f")
}

impl Attribute {
    pub fn parse_inner(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        
        let mut args = Vec::new();
        let mut int_arg = None;
        let mut string_arg = None;

        if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            
            if content.peek(syn::LitInt) {
                let lit: syn::LitInt = content.parse()?;
                int_arg = Some(lit.base10_parse::<u32>()?);
            } else if content.peek(syn::LitStr) {
                // [V4.0] Parse string literal for @string_prefix("...")
                let lit: syn::LitStr = content.parse()?;
                string_arg = Some(lit.value());
            } else {
                let parsed: Punctuated<Ident, Token![,]> = content.parse_terminated(Ident::parse, Token![,])?;
                args = parsed.into_iter().collect();
            }
        }

        Ok(Attribute { name, args, int_arg, string_arg })
    }
}

impl Parse for Attribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![@]>()?;
        Self::parse_inner(input)
    }
}


pub fn parse_attributes(input: ParseStream) -> syn::Result<Vec<Attribute>> {
    let mut attrs = Vec::new();
    while input.peek(Token![@]) {
        attrs.push(input.parse()?);
    }
    Ok(attrs)
}

/// Check if any attribute matches a given name
pub fn has_attribute(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// [SOVEREIGN V4.0] Check if function is marked @pure
/// Pure functions can be used in requires/ensures clauses and are modeled
/// as Z3 uninterpreted functions for verification.
pub fn is_pure(attrs: &[Attribute]) -> bool {
    has_attribute(attrs, "pure")
}

/// [V8] Check if function is marked @fast_math
/// Enables reassoc + contract fast-math flags on ALL floating-point ops
/// in the function body, not just reduction patterns. This allows LLVM to
/// vectorize and reassociate FP arithmetic for maximum throughput.
pub fn is_fast_math(attrs: &[Attribute]) -> bool {
    has_attribute(attrs, "fast_math")
}

/// Extract pulse value from @yielding attribute
/// Returns Some(pulse) if @yielding is present, None otherwise
/// Default pulse is 1024 when @yielding has no argument
pub fn extract_yielding_pulse(attrs: &[Attribute]) -> Option<u32> {
    attrs.iter()
        .find(|a| a.name == "yielding")
        .map(|a| a.int_arg.unwrap_or(1024))
}

/// [SOVEREIGN V2.0] Extract pulse frequency from @pulse attribute
/// Returns Some(frequency_hz) if @pulse is present, None otherwise
/// Examples: @pulse(60) -> Some(60), @pulse(1000) -> Some(1000)
pub fn extract_pulse_hz(attrs: &[Attribute]) -> Option<u32> {
    attrs.iter()
        .find(|a| a.name == "pulse")
        .map(|a| a.int_arg.unwrap_or(60)) // Default 60Hz if no arg
}

/// [SOVEREIGN V2.0] Determine priority tier from pulse frequency
/// Tier 0 (Real-Time): >= 500Hz
/// Tier 1 (Interactive): >= 30Hz
/// Tier 2 (Background): < 30Hz
pub fn pulse_to_tier(frequency_hz: u32) -> u8 {
    if frequency_hz >= 500 {
        0 // Real-Time
    } else if frequency_hz >= 30 {
        1 // Interactive
    } else {
        2 // Background
    }
}

/// [FACET L1] Extract shader kind from @shader attribute
/// Returns Some("compute"), Some("vertex"), or Some("fragment") if present
/// Default is "compute" when @shader has no arguments
pub fn extract_shader_kind(attrs: &[Attribute]) -> Option<String> {
    attrs.iter()
        .find(|a| a.name == "shader")
        .map(|a| {
            a.args.first()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "compute".to_string())
        })
}

/// [FACET L1] Extract workgroup size from @shader attribute
/// Returns the int_arg if present, defaulting to 64
/// Examples: @shader(compute, 256) -> 256, @shader(compute) -> 64
pub fn extract_workgroup_size(attrs: &[Attribute]) -> u32 {
    attrs.iter()
        .find(|a| a.name == "shader")
        .and_then(|a| a.int_arg)
        .unwrap_or(64)
}

/// [SPRINT 2] Extract explicit cycle budget from @pulse_budget attribute
/// Returns Some(cycles) if @pulse_budget is present, None otherwise
/// Examples: @pulse_budget(1000) -> Some(1000), @pulse_budget(50000) -> Some(50000)
pub fn extract_pulse_budget(attrs: &[Attribute]) -> Option<u32> {
    attrs.iter()
        .find(|a| a.name == "pulse_budget")
        .and_then(|a| a.int_arg)
}

/// [SOVEREIGN V0.9.1] Extract memory alignment from @align attribute
/// Returns Some(alignment) if @align(N) is present, None otherwise
/// Examples: @align(64) -> Some(64)
pub fn extract_align(attrs: &[Attribute]) -> Option<u32> {
    attrs.iter()
        .find(|a| a.name == "align")
        .and_then(|a| a.int_arg)
}

