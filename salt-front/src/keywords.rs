use syn::custom_keyword;

custom_keyword!(fn_);
custom_keyword!(struct_);
custom_keyword!(global);
custom_keyword!(package);
custom_keyword!(import);
custom_keyword!(requires);
custom_keyword!(ensures);
custom_keyword!(concept);
custom_keyword!(invariant);
custom_keyword!(forall);
custom_keyword!(exists);
custom_keyword!(owned);
custom_keyword!(window);
custom_keyword!(move_); // 'move' is a keyword in Rust, so we map it to move_ or verify usage
custom_keyword!(map_window);
custom_keyword!(reinterpret_cast);
custom_keyword!(shader);
custom_keyword!(with);
custom_keyword!(region);
custom_keyword!(salt_return);
custom_keyword!(var);
custom_keyword!(else_);  // For let-else parsing (else is reserved)
