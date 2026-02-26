use quote::{ToTokens, TokenStreamExt};
use crate::grammar::{SynType, SynPath, SynPathSegment, SynTuple, TensorDim};

impl ToTokens for SynType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            // [SOVEREIGN V2.0]: Emit the first-class Pointer primitive
            SynType::Pointer(inner) => {
                let p = quote::format_ident!("Ptr");
                tokens.append(p);
                tokens.append(proc_macro2::Punct::new('<', proc_macro2::Spacing::Alone));
                inner.to_tokens(tokens);
                tokens.append(proc_macro2::Punct::new('>', proc_macro2::Spacing::Alone));
            }
            // [SOVEREIGN V2.0]: Emit the Safe Reference
            SynType::Reference(inner, is_mut) => {
                tokens.append(proc_macro2::Punct::new('&', proc_macro2::Spacing::Joint));
                if *is_mut {
                    tokens.append(quote::format_ident!("mut"));
                }
                inner.to_tokens(tokens);
            }
            // [SOVEREIGN PHASE 3]: Emit ShapedTensor as Tensor<T, {Rank, D1...}>
            SynType::ShapedTensor { element, rank, dims } => {
                let t = quote::format_ident!("Tensor");
                tokens.append(t);
                tokens.append(proc_macro2::Punct::new('<', proc_macro2::Spacing::Alone));
                element.to_tokens(tokens);
                tokens.append(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone));
                
                // Emit dimension block {Rank, D1, D2...}
                let mut dim_tokens = proc_macro2::TokenStream::new();
                dim_tokens.extend(quote::quote!(#rank));
                for dim in dims {
                    dim_tokens.append(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone));
                    match dim {
                        TensorDim::Static(n) => dim_tokens.extend(quote::quote!(#n)),
                        TensorDim::Dynamic => dim_tokens.append(proc_macro2::Punct::new('?', proc_macro2::Spacing::Alone)),
                        TensorDim::Symbolic(s) => {
                            let id = quote::format_ident!("{}", s);
                            dim_tokens.append(id);
                        }
                    }
                }
                tokens.append(proc_macro2::Group::new(proc_macro2::Delimiter::Brace, dim_tokens));
                
                tokens.append(proc_macro2::Punct::new('>', proc_macro2::Spacing::Alone));
            }
            SynType::Path(p) => p.to_tokens(tokens),
            SynType::Array(inner, len) => {
                // Emit [T; N] form
                let mut inner_tokens = proc_macro2::TokenStream::new();
                inner.to_tokens(&mut inner_tokens);
                inner_tokens.append(proc_macro2::Punct::new(';', proc_macro2::Spacing::Alone));
                len.to_tokens(&mut inner_tokens);
                
                tokens.append(proc_macro2::Group::new(
                    proc_macro2::Delimiter::Bracket, 
                    inner_tokens
                ));
            }
            SynType::Tuple(t) => t.to_tokens(tokens),
            SynType::FnPtr(args, ret) => {
                // Emit: fn(T1, T2) -> R
                tokens.append(quote::format_ident!("fn"));
                let mut inner = proc_macro2::TokenStream::new();
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        inner.append(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone));
                    }
                    arg.to_tokens(&mut inner);
                }
                tokens.append(proc_macro2::Group::new(proc_macro2::Delimiter::Parenthesis, inner));
                if let Some(ret_ty) = ret {
                    tokens.append(proc_macro2::Punct::new('-', proc_macro2::Spacing::Joint));
                    tokens.append(proc_macro2::Punct::new('>', proc_macro2::Spacing::Alone));
                    ret_ty.to_tokens(tokens);
                }
            }
            SynType::Other(s) => {
                 if let Ok(ts) = s.parse::<proc_macro2::TokenStream>() {
                     tokens.extend(ts);
                 } else {
                     let id = quote::format_ident!("{}", s);
                     tokens.append(id);
                 }
            }
        }
    }
}

impl ToTokens for SynPath {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 {
                // Emit double-colon path separator
                tokens.append(proc_macro2::Punct::new(':', proc_macro2::Spacing::Joint));
                tokens.append(proc_macro2::Punct::new(':', proc_macro2::Spacing::Alone));
            }
            segment.to_tokens(tokens);
        }
    }
}

impl ToTokens for SynPathSegment {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.ident.to_tokens(tokens);
        if !self.args.is_empty() {
            tokens.append(proc_macro2::Punct::new('<', proc_macro2::Spacing::Alone));
            for (i, arg) in self.args.iter().enumerate() {
                if i > 0 {
                    tokens.append(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone));
                }
                arg.to_tokens(tokens);
            }
            tokens.append(proc_macro2::Punct::new('>', proc_macro2::Spacing::Alone));
        }
    }
}

impl ToTokens for SynTuple {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut inner = proc_macro2::TokenStream::new();
        for (i, elem) in self.elems.iter().enumerate() {
            if i > 0 {
                inner.append(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone));
            }
            elem.to_tokens(&mut inner);
        }
        tokens.append(proc_macro2::Group::new(proc_macro2::Delimiter::Parenthesis, inner));
    }
}