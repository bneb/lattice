use crate::types::Type;
use crate::codegen::context::CodegenContext;

pub struct GenericContextGuard<'b, 'a> {
    pub ctx: &'b CodegenContext<'a>,
    pub old_args: std::collections::BTreeMap<String, Type>,
    pub old_self: Option<Type>,
    pub old_ordered_args: Vec<Type>,
}

impl<'b, 'a> GenericContextGuard<'b, 'a> {
    pub fn new(ctx: &'b CodegenContext<'a>, new_args: std::collections::BTreeMap<String, Type>, self_ty: Type, ordered_args: Vec<Type>) -> Self {
        let old_args = std::mem::replace(&mut *ctx.current_type_map_mut(), new_args);
        let old_self = (*ctx.current_self_ty_mut()).replace(self_ty);
        let old_ordered_args = std::mem::replace(&mut *ctx.current_generic_args_mut(), ordered_args);
        Self { ctx, old_args, old_self, old_ordered_args }
    }
}

impl<'b, 'a> Drop for GenericContextGuard<'b, 'a> {
    fn drop(&mut self) {
        *self.ctx.current_type_map_mut() = self.old_args.clone();
        *self.ctx.current_self_ty_mut() = self.old_self.clone();
        *self.ctx.current_generic_args_mut() = self.old_ordered_args.clone();
    }
}

pub struct ImportContextGuard<'b, 'a> {
    pub ctx: &'b CodegenContext<'a>,
    pub old_imports: Vec<crate::grammar::ImportDecl>,
}

impl<'b, 'a> ImportContextGuard<'b, 'a> {
    pub fn new(ctx: &'b CodegenContext<'a>, new_imports: Vec<crate::grammar::ImportDecl>) -> Self {
        let old_imports = std::mem::replace(&mut *ctx.imports_mut(), new_imports);
        Self { ctx, old_imports }
    }
}

impl<'b, 'a> Drop for ImportContextGuard<'b, 'a> {
    fn drop(&mut self) {
        *self.ctx.imports_mut() = self.old_imports.clone();
    }
}
