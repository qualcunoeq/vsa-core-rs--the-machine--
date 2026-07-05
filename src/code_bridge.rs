//! Code perception bridge: Rust source files → role-filler frames.
//!
//! Parses Rust source files directly using the `syn` crate (no Python
//! dependency). Extracts four frame types from the AST:
//!
//! * **signature** – function/method definitions
//! * **call** – call sites within function bodies
//! * **type** – struct field declarations
//! * **impl** – impl block relationships
//!
//! All frames use the same `RoleDictionary` encoding as the NLP bridge,
//! so code frames and natural-language frames can coexist in the same
//! `PrimaryIndex` and participate in the same analogical inferences.

use std::path::Path;
use std::fs;
use std::collections::HashMap;

use syn::{
    File, Item, ItemFn, ItemImpl, ItemStruct, ItemTrait, ImplItem, FnArg,
    ReturnType, Type, Signature, visit::{self, Visit},
};

use crate::analogy::{
    AnalogicalIndex, EpistemicStatus, MetaIndex,
    ObservationProvenance, RoleDictionary,
    ROLE_AGENT, ROLE_ACTION, ROLE_PATIENT,
    ROLE_INSTRUMENT, ROLE_CAUSE, ROLE_ATTRIBUTE,
};
use crate::bridge::encode_phrase;
use crate::Hypervector;

// ── Result type ──────────────────────────────────────────────────────────────

/// Result of ingesting a single source file.
#[derive(Debug, Default)]
pub struct CodeBridgeResult {
    pub frames_signature: usize,
    pub frames_call:      usize,
    pub frames_type:      usize,
    pub frames_impl:      usize,
    pub frames_skipped:   usize,
    pub parse_errors:     usize,
}

impl CodeBridgeResult {
    pub fn total_inserted(&self) -> usize {
        self.frames_signature + self.frames_call + self.frames_type + self.frames_impl
    }
}

// ─── Fixed concept hypervectors ───────────────────────────────────────────────

fn hv_calls()    -> Hypervector { encode_phrase("calls_canonical") }
fn hv_contains() -> Hypervector { encode_phrase("contains_canonical") }
fn hv_impls()    -> Hypervector { encode_phrase("impls_canonical") }

// ─── Binding helpers ─────────────────────────────────────────────────────────

fn bind_quad(
    roles: &RoleDictionary,
    a: &Hypervector,
    b: &Hypervector,
    c: &Hypervector,
    d: &Hypervector,
) -> Hypervector {
    roles.bind_role_filler(ROLE_AGENT, a)
        .bitwise_xor(&roles.bind_role_filler(ROLE_ACTION, b))
        .bitwise_xor(&roles.bind_role_filler(ROLE_PATIENT, c))
        .bitwise_xor(&roles.bind_role_filler(ROLE_INSTRUMENT, d))
}

fn bundle_types(types: &[String]) -> Hypervector {
    if types.is_empty() {
        return Hypervector::new_zero();
    }
    types.iter()
        .map(|t| encode_phrase(t))
        .fold(Hypervector::new_zero(), |acc, hv| acc.bitwise_xor(&hv))
}

fn type_to_string(ty: &Type) -> String {
    // Pretty-print a syn Type to a string
    quote::quote!(#ty).to_string()
}

// ─── AST visitor ──────────────────────────────────────────────────────────────

/// Holds extracted raw data before frame encoding
struct RawSignature {
    struct_name: String,
    fn_name:     String,
    param_types: Vec<String>,
    return_type: String,
    confidence:  f64,
}

struct RawCall {
    caller:        String,
    caller_struct: String,
    callee:        String,
    confidence:    f64,
}

struct RawField {
    struct_name: String,
    field_name:  String,
    field_type:  String,
    confidence:  f64,
}

struct RawImpl {
    struct_name: String,
    trait_name:  String,
    confidence:  f64,
}

/// AST visitor that extracts code structure frames.
struct CodeVisitor {
    current_struct: String,
    signatures: Vec<RawSignature>,
    calls: Vec<RawCall>,
    fields: Vec<RawField>,
    impls: Vec<RawImpl>,
}

impl CodeVisitor {
    fn new() -> Self {
        CodeVisitor {
            current_struct: String::new(),
            signatures: Vec::new(),
            calls: Vec::new(),
            fields: Vec::new(),
            impls: Vec::new(),
        }
    }

    /// Visit a function signature and extract call sites from its body.
    fn visit_fn(&mut self, item_fn: &ItemFn, struct_name: &str) {
        let fn_name = item_fn.sig.ident.to_string();

        // Extract parameter types
        let param_types: Vec<String> = item_fn.sig.inputs.iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(pat_type) => Some(type_to_string(&pat_type.ty)),
                FnArg::Receiver(_) => None, // skip self
            })
            .collect();

        // Extract return type
        let return_type = match &item_fn.sig.output {
            ReturnType::Default => "()".to_string(),
            ReturnType::Type(_, ty) => type_to_string(ty),
        };

        self.signatures.push(RawSignature {
            struct_name: struct_name.to_string(),
            fn_name: fn_name.clone(),
            param_types,
            return_type,
            confidence: 1.0,
        });

        // Extract call sites from the function body
        self.extract_calls_from_block(&item_fn.block, &fn_name, struct_name);
    }

    /// Walk a block's statements and extract call expressions.
    fn extract_calls_from_block(&mut self, block: &syn::Block, caller: &str, struct_name: &str) {
        for stmt in &block.stmts {
            self.extract_calls_from_stmt(stmt, caller, struct_name);
        }
    }

    fn extract_calls_from_stmt(&mut self, stmt: &syn::Stmt, caller: &str, struct_name: &str) {
        match stmt {
            syn::Stmt::Expr(expr, _) => {
                self.extract_calls_from_expr(expr, caller, struct_name);
            }
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.extract_calls_from_expr(&init.expr, caller, struct_name);
                }
            }
            _ => {}
        }
    }

    fn extract_calls_from_expr(&mut self, expr: &syn::Expr, caller: &str, struct_name: &str) {
        match expr {
            syn::Expr::Call(call_expr) => {
                // Extract callee name
                let callee = self.expr_name(&call_expr.func);
                if !callee.is_empty() && callee != caller {
                    self.calls.push(RawCall {
                        caller: caller.to_string(),
                        caller_struct: struct_name.to_string(),
                        callee,
                        confidence: 0.95,
                    });
                }
                // Recurse into arguments
                for arg in &call_expr.args {
                    self.extract_calls_from_expr(arg, caller, struct_name);
                }
            }
            syn::Expr::MethodCall(method_call) => {
                self.calls.push(RawCall {
                    caller: caller.to_string(),
                    caller_struct: struct_name.to_string(),
                    callee: method_call.method.to_string(),
                    confidence: 0.95,
                });
                // Recurse into receiver and args
                self.extract_calls_from_expr(&method_call.receiver, caller, struct_name);
                for arg in &method_call.args {
                    self.extract_calls_from_expr(arg, caller, struct_name);
                }
            }
            syn::Expr::Binary(bin) => {
                self.extract_calls_from_expr(&bin.left, caller, struct_name);
                self.extract_calls_from_expr(&bin.right, caller, struct_name);
            }
            syn::Expr::Block(block) => {
                self.extract_calls_from_block(&block.block, caller, struct_name);
            }
            syn::Expr::If(if_expr) => {
                self.extract_calls_from_expr(&if_expr.cond, caller, struct_name);
                self.extract_calls_from_block(&if_expr.then_branch, caller, struct_name);
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.extract_calls_from_expr(else_expr, caller, struct_name);
                }
            }
            syn::Expr::Unary(un) => {
                self.extract_calls_from_expr(&un.expr, caller, struct_name);
            }
            syn::Expr::Paren(p) => {
                self.extract_calls_from_expr(&p.expr, caller, struct_name);
            }
            syn::Expr::Tuple(t) => {
                for e in &t.elems {
                    self.extract_calls_from_expr(e, caller, struct_name);
                }
            }
            syn::Expr::ForLoop(for_loop) => {
                self.extract_calls_from_block(&for_loop.body, caller, struct_name);
            }
            syn::Expr::While(while_loop) => {
                self.extract_calls_from_block(&while_loop.body, caller, struct_name);
            }
            syn::Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
                    self.extract_calls_from_expr(&arm.body, caller, struct_name);
                }
            }
            syn::Expr::Closure(closure) => {
                self.extract_calls_from_expr(&closure.body, caller, struct_name);
            }
            syn::Expr::Return(ret) => {
                if let Some(expr) = &ret.expr {
                    self.extract_calls_from_expr(expr, caller, struct_name);
                }
            }
            syn::Expr::Assign(assign) => {
                self.extract_calls_from_expr(&assign.left, caller, struct_name);
                self.extract_calls_from_expr(&assign.right, caller, struct_name);
            }
            syn::Expr::Macro(mac) => {
                // Skip macro calls (too complex for v1)
            }
            syn::Expr::Unsafe(unsafe_block) => {
                for stmt in &unsafe_block.block.stmts {
                    self.extract_calls_from_stmt(stmt, caller, struct_name);
                }
            }
            _ => {} // skip other expression types
        }
    }

    /// Extract a simple name from an expression (for callee identification).
    fn expr_name(&self, expr: &syn::Expr) -> String {
        match expr {
            syn::Expr::Path(path) => {
                path.path.segments.last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }
}

// ─── Main ingestion ──────────────────────────────────────────────────────────

/// Parse a Rust source file and insert frames into the PrimaryIndex.
///
/// Uses `syn` directly — no Python subprocess required.
/// The AST walker extracts function signatures, call sites, struct fields,
/// and impl block relationships.
pub fn ingest_source_file(
    path: &Path,
    primary: &mut AnalogicalIndex,
    meta:    &mut MetaIndex,
    novel_threshold: f64,
    frame_counter:   &mut usize,
) -> CodeBridgeResult {
    let mut result = CodeBridgeResult::default();
    let roles = RoleDictionary::new();

    // Read the source file
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[code_bridge] failed to read {:?}: {e}", path);
            result.parse_errors += 1;
            return result;
        }
    };

    // Parse the AST
    let syntax: File = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[code_bridge] syn parse failed for {:?}: {e}", path);
            result.parse_errors += 1;
            return result;
        }
    };

    // Walk the AST
    let mut visitor = CodeVisitor::new();

    for item in &syntax.items {
        match item {
            Item::Impl(item_impl) => {
                // Extract impl block information
                let struct_name = type_to_string(&item_impl.self_ty);
                let trait_name = item_impl.trait_.as_ref()
                    .map(|(_, path, _)| path_to_string(path))
                    .unwrap_or_default();

                visitor.impls.push(RawImpl {
                    struct_name: struct_name.clone(),
                    trait_name,
                    confidence: 1.0,
                });

                // Visit all items in the impl block
                for impl_item in &item_impl.items {
                    match impl_item {
                        ImplItem::Fn(method) => {
                            // ImplItemFn has sig and block like ItemFn but is a different type
                            let fn_name = method.sig.ident.to_string();
                            let param_types: Vec<String> = method.sig.inputs.iter()
                                .filter_map(|arg| match arg {
                                    FnArg::Typed(pat_type) => Some(type_to_string(&pat_type.ty)),
                                    FnArg::Receiver(_) => None,
                                })
                                .collect();
                            let return_type = match &method.sig.output {
                                ReturnType::Default => "()".to_string(),
                                ReturnType::Type(_, ty) => type_to_string(ty),
                            };
                            visitor.signatures.push(RawSignature {
                                struct_name: struct_name.clone(),
                                fn_name: fn_name.clone(),
                                param_types,
                                return_type,
                                confidence: 1.0,
                            });
                            visitor.extract_calls_from_block(&method.block, &fn_name, &struct_name);
                        }
                        _ => {}
                    }
                }
            }
            Item::Fn(item_fn) => {
                // Free function (not in an impl block)
                visitor.visit_fn(item_fn, "");
            }
            Item::Struct(item_struct) => {
                let struct_name = item_struct.ident.to_string();
                // Extract fields
                if let syn::Fields::Named(fields_named) = &item_struct.fields {
                    for field in &fields_named.named {
                        let field_name = field.ident.as_ref()
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        let field_type = type_to_string(&field.ty);
                        visitor.fields.push(RawField {
                            struct_name: struct_name.clone(),
                            field_name,
                            field_type,
                            confidence: 1.0,
                        });
                    }
                }
            }
            Item::Trait(item_trait) => {
                let trait_name = item_trait.ident.to_string();
                // Treat trait methods as signatures with empty struct_name
                for trait_item in &item_trait.items {
                    if let syn::TraitItem::Fn(trait_method) = trait_item {
                        let fn_name = trait_method.sig.ident.to_string();
                        let param_types: Vec<String> = trait_method.sig.inputs.iter()
                            .filter_map(|arg| match arg {
                                FnArg::Typed(pat_type) => Some(type_to_string(&pat_type.ty)),
                                FnArg::Receiver(_) => None,
                            })
                            .collect();
                        let return_type = match &trait_method.sig.output {
                            ReturnType::Default => "()".to_string(),
                            ReturnType::Type(_, ty) => type_to_string(ty),
                        };
                        visitor.signatures.push(RawSignature {
                            struct_name: format!("trait {}", trait_name),
                            fn_name,
                            param_types,
                            return_type,
                            confidence: 1.0,
                        });
                    }
                }
            }
            _ => {} // Skip other items (mod, use, const, etc.)
        }
    }

    // ── Insert all frames ───────────────────────────────────────────

    for sig in &visitor.signatures {
        let agent      = encode_phrase(&sig.struct_name);
        let action     = encode_phrase(&sig.fn_name);
        let patient    = encode_phrase(&sig.return_type);
        let instrument = bundle_types(&sig.param_types);
        let bound      = bind_quad(&roles, &agent, &action, &patient, &instrument);
        let fillers = vec![
            (ROLE_AGENT,      agent,      sig.struct_name.clone()),
            (ROLE_ACTION,     action,     sig.fn_name.clone()),
            (ROLE_PATIENT,    patient,    sig.return_type.clone()),
            (ROLE_INSTRUMENT, instrument, sig.param_types.join(", ")),
        ];
        let is_novel = primary.frames().iter().all(|f| {
            f.bound_vector.normalized_hamming_distance(&bound) > novel_threshold
        });
        if is_novel {
            let label = format!("code_{:05}", frame_counter);
            *frame_counter += 1;
            primary.insert_with_provenance(&label, bound, fillers, ObservationProvenance::Ambient);
            meta.on_insert(&label, &bound, EpistemicStatus::Observed, 400.0, ObservationProvenance::Ambient);
            result.frames_signature += 1;
        } else {
            result.frames_skipped += 1;
        }
    }

    for call in &visitor.calls {
        let agent   = encode_phrase(&call.caller);
        let action  = hv_calls();
        let patient = encode_phrase(&call.callee);
        let cause   = encode_phrase(&call.caller_struct);
        let bound   = roles.bind_triple(&agent, &action, &patient);
        let fillers = vec![
            (ROLE_AGENT,   agent,   call.caller.clone()),
            (ROLE_ACTION,  action,  "calls".to_string()),
            (ROLE_PATIENT, patient, call.callee.clone()),
            (ROLE_CAUSE,   cause,   call.caller_struct.clone()),
        ];
        let is_novel = primary.frames().iter().all(|f| {
            f.bound_vector.normalized_hamming_distance(&bound) > novel_threshold
        });
        if is_novel {
            let label = format!("code_{:05}", frame_counter);
            *frame_counter += 1;
            primary.insert_with_provenance(&label, bound, fillers, ObservationProvenance::Ambient);
            let weight = (call.confidence * 380.0).clamp(0.0, 500.0);
            meta.on_insert(&label, &bound, EpistemicStatus::Observed, weight, ObservationProvenance::Ambient);
            result.frames_call += 1;
        } else {
            result.frames_skipped += 1;
        }
    }

    for field in &visitor.fields {
        let agent     = encode_phrase(&field.struct_name);
        let action    = hv_contains();
        let patient   = encode_phrase(&field.field_name);
        let attribute = encode_phrase(&field.field_type);
        let bound     = roles.bind_triple(&agent, &action, &patient);
        let fillers   = vec![
            (ROLE_AGENT,     agent,     field.struct_name.clone()),
            (ROLE_ACTION,    action,    "contains".to_string()),
            (ROLE_PATIENT,   patient,   field.field_name.clone()),
            (ROLE_ATTRIBUTE, attribute, field.field_type.clone()),
        ];
        let is_novel = primary.frames().iter().all(|f| {
            f.bound_vector.normalized_hamming_distance(&bound) > novel_threshold
        });
        if is_novel {
            let label = format!("code_{:05}", frame_counter);
            *frame_counter += 1;
            primary.insert_with_provenance(&label, bound, fillers, ObservationProvenance::Ambient);
            meta.on_insert(&label, &bound, EpistemicStatus::Observed, 400.0, ObservationProvenance::Ambient);
            result.frames_type += 1;
        } else {
            result.frames_skipped += 1;
        }
    }

    for imp in &visitor.impls {
        let agent   = encode_phrase(&imp.struct_name);
        let action  = hv_impls();
        let patient = if imp.trait_name.is_empty() {
            encode_phrase("inherent")
        } else {
            encode_phrase(&imp.trait_name)
        };
        let bound   = roles.bind_triple(&agent, &action, &patient);
        let fillers = vec![
            (ROLE_AGENT,   agent,   imp.struct_name.clone()),
            (ROLE_ACTION,  action,  "impls".to_string()),
            (ROLE_PATIENT, patient, imp.trait_name.clone()),
        ];
        let is_novel = primary.frames().iter().all(|f| {
            f.bound_vector.normalized_hamming_distance(&bound) > novel_threshold
        });
        if is_novel {
            let label = format!("code_{:05}", frame_counter);
            *frame_counter += 1;
            primary.insert_with_provenance(&label, bound, fillers, ObservationProvenance::Ambient);
            meta.on_insert(&label, &bound, EpistemicStatus::Observed, 400.0, ObservationProvenance::Ambient);
            result.frames_impl += 1;
        } else {
            result.frames_skipped += 1;
        }
    }

    result
}

/// Helper: convert a syn Path to a string
fn path_to_string(path: &syn::Path) -> String {
    path.segments.iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingest_self() {
        let roles = RoleDictionary::new();
        let mut primary = AnalogicalIndex::new(&roles);
        let mut meta = MetaIndex::new(&primary, 64);
        let mut counter = 0usize;

        // Ingest the code_bridge itself (small file, fast parse)
        let result = ingest_source_file(
            Path::new("src/code_bridge.rs"),
            &mut primary, &mut meta, 0.05, &mut counter,
        );

        eprintln!(
            "  [code_bridge] self: sig={} call={} type={} impl={} total={}",
            result.frames_signature, result.frames_call,
            result.frames_type, result.frames_impl,
            result.total_inserted(),
        );

        assert!(
            result.total_inserted() >= 5,
            "Expected ≥5 frames from code_bridge.rs, got {}",
            result.total_inserted(),
        );
    }

    #[test]
    #[ignore = "integration benchmark: parses a larger source file and is not needed for default unit verification"]
    fn test_ingest_larger_file() {
        let roles = RoleDictionary::new();
        let mut primary = AnalogicalIndex::new(&roles);
        let mut meta = MetaIndex::new(&primary, 64);
        let mut counter = 0usize;

        // Use lib.rs (smaller than analogy.rs) for a reasonably-sized test
        let result = ingest_source_file(
            Path::new("src/lib.rs"),
            &mut primary, &mut meta, 0.05, &mut counter,
        );

        eprintln!(
            "  [code_bridge] lib.rs: sig={} call={} type={} impl={} total={}",
            result.frames_signature, result.frames_call,
            result.frames_type, result.frames_impl,
            result.total_inserted(),
        );

        assert!(
            result.total_inserted() >= 5,
            "Expected ≥5 frames from lib.rs, got {}",
            result.total_inserted(),
        );

        // Check predictions were generated
        eprintln!(
            "  [code_bridge] predictions from code frames: {}",
            primary.predictions().len(),
        );
    }

    #[test]
    fn test_call_frames_exist() {
        let roles = RoleDictionary::new();
        let mut primary = AnalogicalIndex::new(&roles);
        let mut meta = MetaIndex::new(&primary, 64);
        let mut counter = 0usize;

        let result = ingest_source_file(
            Path::new("src/code_bridge.rs"),
            &mut primary, &mut meta, 0.05, &mut counter,
        );

        // Verify some specific functions were found
        let has_insert_with_gate = primary.frames().iter().any(|f| {
            f.fillers.iter().any(|filler| filler.filler_str.contains("ingest_source_file"))
        });
        assert!(
            has_insert_with_gate,
            "Should find ingest_source_file function",
        );

        // Verify call frames exist
        assert!(
            result.frames_call > 0,
            "Expected at least some call frames, got 0",
        );
    }
}
