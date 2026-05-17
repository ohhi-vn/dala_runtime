//! Native Layout Specialization Pass.
//!
//! Converts stable tuples and shape-specialized maps into compact
//! native layouts for SIR (Stable Immutable Region) promotion.
//!
//! # Stable Tuple → Native Struct
//!
//! A `StableTuple { [SmallInt, SmallInt], immutable: true }` can be
//! represented as a packed struct of two `i64` values — no tags,
//! no pointers, no boxing overhead.
//!
//! Before (boxed BEAM):
//! ```text
//!   [tag | ptr0 | ptr1]  →  [tag | int0]  [tag | int1]
//!   24 bytes + 2 × 16 bytes = 56 bytes
//! ```
//!
//! After (native layout):
//! ```text
//!   [int0 | int1]
//!   16 bytes
//! ```
//!
//! # Map Shape → Hidden Class
//!
//! A `MapShape { keys: [id, name], values: [SmallInt, Binary] }` can
//! be represented as a hidden-class struct:
//!
//! ```text
//!   struct Map_IdName {
//!       int64_t id;      // Known offset, no hash lookup
//!       Binary* name;    // Known offset
//!   };
//! ```
//!
//! This is the same optimization V8, PyPy, and JavaScriptCore use.

use crate::function::{BasicBlock, IRFunction};
use crate::instruction::{IRInst, IRInstKind, SideEffects};
use crate::type_system::{IRType, NativeField, NativeFieldKind, NativeLayout, TypeKind};
use crate::value::IRValueId;

/// Result of native layout analysis for a single value.
#[derive(Debug, Clone)]
pub struct NativeSpec {
    /// The value being specialized.
    pub value: IRValueId,
    /// The native layout to use.
    pub layout: NativeLayout,
    /// Whether this value should be promoted to SIR.
    pub promote_to_sir: bool,
    /// Estimated memory savings in bytes.
    pub savings: u32,
}

/// Run native layout specialization on a function.
///
/// Analyzes all stable tuples and map shapes, computing optimal
/// native layouts for SIR promotion.
///
/// Returns true if any specializations were applied.
pub fn specialize(func: &mut IRFunction) -> bool {
    let mut changed = false;
    let specs = compute_specializations(func);

    for spec in &specs {
        if spec.promote_to_sir {
            log::debug!(
                "SIR promotion for value {:?}: save {} bytes",
                spec.value,
                spec.savings
            );
            changed = true;
        }
    }

    // Apply the specializations: rewrite allocation instructions
    // to use native layout instead of boxed layout.
    for block in &mut func.blocks {
        if !block.reachable {
            continue;
        }
        for inst in &mut block.instructions {
            if let Some(result) = inst.result {
                if let Some(spec) = specs.iter().find(|s| s.value == result) {
                    changed |= apply_native_layout(inst, spec);
                }
            }
        }
    }

    changed
}

/// Compute native layout specializations for all values in a function.
fn compute_specializations(func: &IRFunction) -> Vec<NativeSpec> {
    let mut specs = Vec::new();

    for block in &func.blocks {
        if !block.reachable {
            continue;
        }
        for inst in &block.instructions {
            if let Some(result) = inst.result {
                if let Some(spec) = analyze_value(inst, result) {
                    specs.push(spec);
                }
            }
        }
    }

    specs
}

/// Analyze a single instruction for native layout opportunities.
fn analyze_value(inst: &IRInst, result: IRValueId) -> Option<NativeSpec> {
    match &inst.kind {
        IRInstKind::AllocStable { type_desc, words } => {
            // Stable allocation — compute native layout from type descriptor
            let layout = NativeLayout {
                fields: vec![], // Would be filled from type descriptor
                size: *words * 8,
            };
            Some(NativeSpec {
                value: result,
                layout,
                promote_to_sir: true,
                savings: *words * 4, // Conservative estimate
            })
        }
        _ => {
            // Check if the instruction produces a stable tuple or map shape
            // by examining the result type
            None
        }
    }
}

/// Apply a native layout specialization to an instruction.
fn apply_native_layout(inst: &mut IRInst, spec: &NativeSpec) -> bool {
    match &inst.kind {
        IRInstKind::AllocStable { .. } => {
            // The allocation is already in SIR — mark it with the
            // computed native layout. In a full implementation,
            // this would attach the NativeLayout to the instruction.
            true
        }
        _ => false,
    }
}

/// Compute the native layout for a stable tuple type.
///
/// This is the key function that converts a `StableTuple` type into
/// a `NativeLayout` with unboxed fields.
pub fn stable_tuple_layout(element_types: &[IRType]) -> NativeLayout {
    let mut fields = Vec::new();
    let mut offset = 0u32;

    for elem_ty in element_types {
        let (kind, size) = native_field_for_type(elem_ty);
        fields.push(NativeField {
            offset,
            kind: kind.clone(),
        });
        offset += size;
    }

    // Align to 8 bytes
    let aligned_size = (offset + 7) & !7;

    NativeLayout {
        fields,
        size: aligned_size,
    }
}

/// Compute the native layout for a map shape type.
///
/// Converts a `MapShape` into a hidden-class struct layout where
/// each field has a known offset (no hash lookup needed).
pub fn map_shape_layout(values: &[IRType]) -> NativeLayout {
    let mut fields = Vec::new();
    let mut offset = 0u32;

    // Hidden class pointer (for polymorphic maps)
    fields.push(NativeField {
        offset: 0,
        kind: NativeFieldKind::Ptr,
    });
    offset += 8;

    for val_ty in values {
        let (kind, size) = native_field_for_type(val_ty);
        // Align field
        let align = size;
        offset = (offset + align - 1) & !(align - 1);
        fields.push(NativeField {
            offset,
            kind: kind.clone(),
        });
        offset += size;
    }

    let aligned_size = (offset + 7) & !7;

    NativeLayout {
        fields,
        size: aligned_size,
    }
}

/// Get the native field kind and size for a given IR type.
fn native_field_for_type(ty: &IRType) -> (NativeFieldKind, u32) {
    match &ty.kind {
        TypeKind::SmallInt | TypeKind::NonNegInt | TypeKind::Int64 => (NativeFieldKind::I64, 8),
        TypeKind::Float => (NativeFieldKind::F64, 8),
        TypeKind::Atom | TypeKind::Boolean | TypeKind::Nil => (NativeFieldKind::I64, 8),
        TypeKind::Cons
        | TypeKind::List
        | TypeKind::Tuple { .. }
        | TypeKind::StableTuple { .. }
        | TypeKind::Map { .. }
        | TypeKind::MapShape { .. }
        | TypeKind::Binary
        | TypeKind::Fun { .. }
        | TypeKind::Pid
        | TypeKind::Port
        | TypeKind::Reference
        | TypeKind::Message { .. }
        | TypeKind::Actor { .. }
        | TypeKind::Tensor { .. }
        | TypeKind::Capability { .. } => (NativeFieldKind::Ptr, 8),
        TypeKind::Constant(cv) => match cv {
            crate::type_system::ConstantValue::Int(_)
            | crate::type_system::ConstantValue::Atom(_)
            | crate::type_system::ConstantValue::True
            | crate::type_system::ConstantValue::False
            | crate::type_system::ConstantValue::Nil => (NativeFieldKind::I64, 8),
            crate::type_system::ConstantValue::Float(_) => (NativeFieldKind::F64, 8),
        },
        TypeKind::Any | TypeKind::Bottom => (NativeFieldKind::Ptr, 8),
        TypeKind::Union(a, _) => native_field_for_type(a),
        TypeKind::Intersection(a, _) => native_field_for_type(a),
        TypeKind::Difference(a, _) => native_field_for_type(a),
        TypeKind::RecursiveVar { bound, .. } => {
            if let Some(b) = bound {
                native_field_for_type(b)
            } else {
                (NativeFieldKind::Ptr, 8)
            }
        }
        TypeKind::Dynamic => (NativeFieldKind::Ptr, 8),
        TypeKind::Speculative { assumed, .. } => native_field_for_type(assumed),
    }
}

/// Estimate the memory savings from native layout conversion.
///
/// Compares the boxed BEAM representation size with the compact
/// native layout size.
pub fn estimate_savings(ty: &IRType) -> u32 {
    let boxed_size = boxed_size(ty);
    let native_size = native_size(ty);
    if boxed_size > native_size {
        boxed_size - native_size
    } else {
        0
    }
}

/// Estimate the size of a boxed BEAM term.
fn boxed_size(ty: &IRType) -> u32 {
    match &ty.kind {
        TypeKind::SmallInt
        | TypeKind::NonNegInt
        | TypeKind::Atom
        | TypeKind::Boolean
        | TypeKind::Nil => 8,
        TypeKind::Int64 | TypeKind::Float => 16,
        TypeKind::Tuple { arity } => 8 + *arity * 8,
        TypeKind::StableTuple { element_types, .. } => 8 + element_types.len() as u32 * 8,
        TypeKind::Cons => 16,
        TypeKind::List => 16,
        TypeKind::Map => 24,
        TypeKind::MapShape { values, .. } => 16 + values.len() as u32 * 8,
        TypeKind::Binary => 24,
        TypeKind::Fun { .. } => 16,
        TypeKind::Pid | TypeKind::Port | TypeKind::Reference => 8,
        TypeKind::Message { .. } => 24,
        TypeKind::Actor { accepts, .. } => 16 + accepts.len() as u32 * 8,
        TypeKind::Tensor { .. } => 32,
        TypeKind::Capability { .. } => 16,
        TypeKind::Union(a, _) => boxed_size(a),
        TypeKind::Intersection(a, _) => boxed_size(a),
        TypeKind::Difference(a, _) => boxed_size(a),
        TypeKind::Constant(_) => 8,
        TypeKind::Any | TypeKind::Bottom => 8,
        TypeKind::RecursiveVar { bound, .. } => bound.as_ref().map_or(8, |b| boxed_size(b)),
        TypeKind::Dynamic => 8,
        TypeKind::Speculative { actual, .. } => boxed_size(actual),
    }
}

/// Estimate the size of a native layout representation.
fn native_size(ty: &IRType) -> u32 {
    match &ty.kind {
        TypeKind::SmallInt | TypeKind::NonNegInt | TypeKind::Int64 => 8,
        TypeKind::Float => 8,
        TypeKind::Atom | TypeKind::Boolean | TypeKind::Nil => 8,
        TypeKind::Tuple { arity } => *arity * 8,
        TypeKind::StableTuple { element_types, .. } => {
            let layout = stable_tuple_layout(element_types);
            layout.size
        }
        TypeKind::MapShape { values, .. } => {
            let layout = map_shape_layout(values);
            layout.size
        }
        TypeKind::Constant(_) => 8,
        _ => 8, // Pointer-sized for complex types
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_system::{IRType, NativeFieldKind, TypeKind};

    #[test]
    fn test_stable_tuple_layout_two_ints() {
        let types = vec![
            IRType::new(TypeKind::SmallInt),
            IRType::new(TypeKind::SmallInt),
        ];
        let layout = stable_tuple_layout(&types);
        assert_eq!(layout.fields.len(), 2);
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 8);
        assert_eq!(layout.size, 16);
        assert!(matches!(layout.fields[0].kind, NativeFieldKind::I64));
    }

    #[test]
    fn test_stable_tuple_layout_mixed() {
        let types = vec![
            IRType::new(TypeKind::SmallInt),
            IRType::new(TypeKind::Float),
        ];
        let layout = stable_tuple_layout(&types);
        assert_eq!(layout.fields.len(), 2);
        assert!(matches!(layout.fields[0].kind, NativeFieldKind::I64));
        assert!(matches!(layout.fields[1].kind, NativeFieldKind::F64));
    }

    #[test]
    fn test_map_shape_layout() {
        let values = vec![
            IRType::new(TypeKind::SmallInt),
            IRType::new(TypeKind::Binary),
        ];
        let layout = map_shape_layout(&values);
        // First field is hidden class pointer
        assert!(layout.fields.len() >= 3);
        assert!(matches!(layout.fields[0].kind, NativeFieldKind::Ptr));
    }

    #[test]
    fn test_native_field_for_type() {
        let (kind, size) = native_field_for_type(&IRType::new(TypeKind::SmallInt));
        assert!(matches!(kind, NativeFieldKind::I64));
        assert_eq!(size, 8);

        let (kind, size) = native_field_for_type(&IRType::new(TypeKind::Float));
        assert!(matches!(kind, NativeFieldKind::F64));
        assert_eq!(size, 8);

        let (kind, size) = native_field_for_type(&IRType::new(TypeKind::Cons));
        assert!(matches!(kind, NativeFieldKind::Ptr));
        assert_eq!(size, 8);
    }

    #[test]
    fn test_estimate_savings() {
        // A stable tuple of two ints should save significant memory
        let ty = IRType::new(TypeKind::StableTuple {
            element_types: vec![
                IRType::new(TypeKind::SmallInt),
                IRType::new(TypeKind::SmallInt),
            ],
            immutable: true,
        });
        let savings = estimate_savings(&ty);
        // Boxed: 8 + 2*8 = 24 bytes, Native: 16 bytes → save 8 bytes
        assert!(savings > 0, "expected positive savings, got {}", savings);
    }

    #[test]
    fn test_boxed_vs_native_size() {
        // Two integers: boxed needs tags, native doesn't
        let ty = IRType::new(TypeKind::StableTuple {
            element_types: vec![
                IRType::new(TypeKind::SmallInt),
                IRType::new(TypeKind::SmallInt),
            ],
            immutable: true,
        });
        let boxed = boxed_size(&ty);
        let native = native_size(&ty);
        assert!(
            boxed > native,
            "boxed ({}) should be larger than native ({})",
            boxed,
            native
        );
    }
}
