use crate::types::Type;
use crate::codegen::context::CodegenContext;

impl<'a> CodegenContext<'a> {
    /// Bridge: specialize_template (migrated to LoweringContext in type_bridge.rs)
    pub fn specialize_template(&self, base_name: &str, concrete_tys: &[Type], is_enum: bool) -> Result<crate::types::TypeKey, String> {
        self.with_lowering_ctx(|lctx| lctx.specialize_template(base_name, concrete_tys, is_enum))
    }

    /// Bridge: scan_function_for_calls (migrated to LoweringContext in seeker.rs)
    pub fn scan_function_for_calls(&self, func: &crate::grammar::SaltFn) -> Result<Vec<crate::codegen::collector::MonomorphizationTask>, String> {
        self.with_lowering_ctx(|lctx| lctx.scan_function_for_calls(func))
    }

    /// Bridge: to_mlir_type via LoweringContext
    pub fn resolve_mlir_type(&self, ty: &Type) -> Result<String, String> {
        self.with_lowering_ctx(|lctx| ty.to_mlir_type(lctx))
    }

    /// Bridge: to_mlir_storage_type via LoweringContext
    pub fn resolve_mlir_storage_type(&self, ty: &Type) -> Result<String, String> {
        self.with_lowering_ctx(|lctx| ty.to_mlir_storage_type(lctx))
    }

    /// Bridge: resolve_type via LoweringContext (type_bridge)
    pub fn bridge_resolve_type(&self, ty: &crate::grammar::SynType) -> Type {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::resolve_type(lctx, ty))
    }

    /// Bridge: resolve_codegen_type via LoweringContext (type_bridge)
    pub fn bridge_resolve_codegen_type(&self, ty: &Type) -> Type {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::resolve_codegen_type(lctx, ty))
    }

    /// Bridge: emit_global_def via LoweringContext (type_bridge)
    pub fn bridge_emit_global_def(&self, out: &mut String, g: &crate::grammar::GlobalDef) -> Result<(), String> {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::emit_global_def(lctx, out, g))
    }

    /// Bridge: emit_const via LoweringContext (type_bridge)
    pub fn bridge_emit_const(&self, out: &mut String, c: &crate::grammar::ConstDef) -> Result<(), String> {
        self.with_lowering_ctx(|lctx| crate::codegen::type_bridge::emit_const(lctx, out, c))
    }

    /// Bridge: resolve_package_prefix_ctx via LoweringContext (expr/utils)
    pub fn bridge_resolve_package_prefix(&self, segments: &[String]) -> Option<(String, String)> {
        self.with_lowering_ctx(|lctx| crate::codegen::expr::utils::resolve_package_prefix_ctx(lctx, segments))
    }

    /// Bridge: request_specialization via LoweringContext (type_bridge)
    pub fn request_specialization(&self, func_name: &str, concrete_tys: Vec<Type>, self_ty: Option<Type>) -> String {
        self.with_lowering_ctx(|lctx| lctx.request_specialization(func_name, concrete_tys, self_ty))
    }
}
