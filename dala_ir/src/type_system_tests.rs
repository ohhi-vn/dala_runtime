//! Comprehensive edge-case tests for the set-theoretic type system.
//!
//! Tests lattice laws, subtyping properties, compound type behavior,
//! and edge cases not covered by the basic unit tests.

#![cfg(test)]

use crate::type_system::*;

// ═══════════════════════════════════════════════════════════════════════════
// Helper constructors — all use fully qualified TypeKind:: and ConstantValue::
// ═══════════════════════════════════════════════════════════════════════════

fn t_smallint() -> IRType { IRType::new(TypeKind::SmallInt) }
fn t_nonneg() -> IRType { IRType::new(TypeKind::NonNegInt) }
fn t_int64() -> IRType { IRType::new(TypeKind::Int64) }
fn t_float() -> IRType { IRType::new(TypeKind::Float) }
fn t_atom() -> IRType { IRType::new(TypeKind::Atom) }
fn t_bool() -> IRType { IRType::new(TypeKind::Boolean) }
fn t_nil() -> IRType { IRType::new(TypeKind::Nil) }
fn t_cons() -> IRType { IRType::new(TypeKind::Cons) }
fn t_list() -> IRType { IRType::new(TypeKind::List) }
fn t_any() -> IRType { IRType::new(TypeKind::Any) }
fn t_bottom() -> IRType { IRType::new(TypeKind::Bottom) }
fn t_dynamic() -> IRType { IRType::new(TypeKind::Dynamic) }
fn t_map() -> IRType { IRType::new(TypeKind::Map) }
fn t_bin() -> IRType { IRType::new(TypeKind::Binary) }
fn t_tuple(a: u32) -> IRType { IRType::new(TypeKind::Tuple{arity:a}) }
fn t_fun(a: u32) -> IRType { IRType::new(TypeKind::Fun{arity:a}) }
fn t_pid() -> IRType { IRType::new(TypeKind::Pid) }
fn t_port() -> IRType { IRType::new(TypeKind::Port) }
fn t_ref() -> IRType { IRType::new(TypeKind::Reference) }

fn t_const_int(i: i64) -> IRType { IRType::new(TypeKind::Constant(ConstantValue::Int(i))) }
fn t_const_atom(a: u32) -> IRType { IRType::new(TypeKind::Constant(ConstantValue::Atom(a))) }
fn t_const_true() -> IRType { IRType::new(TypeKind::Constant(ConstantValue::True)) }
fn t_const_false() -> IRType { IRType::new(TypeKind::Constant(ConstantValue::False)) }
fn t_const_nil() -> IRType { IRType::new(TypeKind::Constant(ConstantValue::Nil)) }
fn t_const_float(f: f64) -> IRType { IRType::new(TypeKind::Constant(ConstantValue::Float(f.to_bits()))) }

fn t_union(a: IRType, b: IRType) -> IRType { IRType::new(TypeKind::Union(Box::new(a), Box::new(b))) }
fn t_inter(a: IRType, b: IRType) -> IRType { IRType::new(TypeKind::Intersection(Box::new(a), Box::new(b))) }
fn t_diff(a: IRType, b: IRType) -> IRType { IRType::new(TypeKind::Difference(Box::new(a), Box::new(b))) }
fn t_stable(elems: Vec<IRType>, imm: bool) -> IRType { IRType::new(TypeKind::StableTuple{element_types:elems,immutable:imm}) }
fn t_mapshape(keys: Vec<u32>, vals: Vec<IRType>) -> IRType { IRType::new(TypeKind::MapShape{keys,values:vals}) }
fn t_msg(payload: IRType, pri: MessagePriority) -> IRType { IRType::new(TypeKind::Message{payload:Box::new(payload),priority:pri}) }
fn t_actor(accepts: Vec<IRType>, lc: ActorLifecycle) -> IRType { IRType::new(TypeKind::Actor{accepts,lifecycle:lc}) }
fn t_tensor(dt: TensorDtype, shape: Vec<Option<u64>>) -> IRType { IRType::new(TypeKind::Tensor{dtype:dt,shape}) }
fn t_cap(r: NativeResourceKind, o: bool, s: bool) -> IRType { IRType::new(TypeKind::Capability{resource:r,owned:o,shareable:s}) }
fn t_recvar(id: u32, bound: Option<IRType>) -> IRType { IRType::new(TypeKind::RecursiveVar{id,bound:bound.map(Box::new)}) }
fn t_spec(assumed: IRType, actual: IRType, guard: SpeculativeGuard) -> IRType { IRType::new(TypeKind::Speculative{assumed:Box::new(assumed),actual:Box::new(actual),guard}) }

// ═══════════════════════════════════════════════════════════════════════════
// 1. Lattice Laws
// ═══════════════════════════════════════════════════════════════════════════

mod lattice_laws {
    use super::*;

    #[test] fn join_commut_basic() { let a=t_smallint(); let b=t_float(); assert_eq!(a.join(&b),b.join(&a)); }
    #[test] fn join_commut_compound() { let a=t_union(t_smallint(),t_atom()); let b=t_inter(t_float(),t_nonneg()); assert_eq!(a.join(&b),b.join(&a)); }
    #[test] fn join_commut_any() { let a=t_smallint(); assert_eq!(a.join(&t_any()),t_any()); }
    #[test] fn join_commut_bottom() { let a=t_smallint(); assert_eq!(a.join(&t_bottom()),a); }
    #[test] fn meet_commut_basic() { let a=t_smallint(); let b=t_nonneg(); assert_eq!(a.meet(&b),b.meet(&a)); }
    #[test] fn meet_commut_any() { let a=t_smallint(); assert_eq!(a.meet(&t_any()),a); }
    #[test] fn meet_commut_bottom() { let a=t_smallint(); assert!(a.meet(&t_bottom()).is_empty()); }
    #[test] fn join_assoc() { let a=t_smallint(); let b=t_float(); let c=t_atom(); assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c))); }
    #[test] fn meet_assoc() { let a=t_smallint(); let b=t_nonneg(); let c=t_int64(); assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c))); }
    #[test] fn join_idem() { let a=t_smallint(); assert_eq!(a.join(&a),a); }
    #[test] fn meet_idem() { let a=t_smallint(); assert_eq!(a.meet(&a),a); }
    #[test] fn join_idem_compound() { let a=t_union(t_smallint(),t_float()); assert_eq!(a.join(&a),a); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Absorption Laws
// ═══════════════════════════════════════════════════════════════════════════

mod absorption {
    use super::*;
    #[test] fn join_meet_absorb() { let a=t_smallint(); let b=t_nonneg(); assert_eq!(a.join(&a.meet(&b)),a); }
    #[test] fn meet_join_absorb() { let a=t_smallint(); let b=t_float(); assert_eq!(a.meet(&a.join(&b)),a); }
    #[test] fn absorb_const() { let a=t_const_int(5); let b=t_smallint(); assert_eq!(a.join(&a.meet(&b)),a); }
    #[test] fn absorb_compound() { let a=t_union(t_smallint(),t_atom()); let b=t_inter(t_smallint(),t_nonneg()); assert_eq!(a.join(&a.meet(&b)),a); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Subtyping Transitivity
// ═══════════════════════════════════════════════════════════════════════════

mod transitivity {
    use super::*;
    #[test] fn int_hierarchy() { assert!(t_smallint().contains(&t_nonneg())); assert!(t_int64().contains(&t_smallint())); assert!(t_int64().contains(&t_nonneg())); }
    #[test] fn list_hierarchy() { assert!(t_list().contains(&t_nil())); assert!(t_list().contains(&t_cons())); }
    #[test] fn const_chain() { let c=t_const_int(42); assert!(t_nonneg().contains(&c)); assert!(t_smallint().contains(&c)); assert!(t_int64().contains(&c)); }
    #[test] fn union_subtype() { let a=t_const_int(1); let b=t_const_int(2); let u=t_union(a.clone(),b.clone()); assert!(t_smallint().contains(&u)); }
    #[test] fn inter_supertype() { let a=t_nonneg(); let b=t_smallint(); let c=t_int64(); assert!(t_inter(b,c).contains(&a)); }
    #[test] fn stable_tuple_sub() { let st=t_stable(vec![t_smallint(),t_atom()],true); assert!(t_tuple(2).contains(&st)); assert!(t_any().contains(&st)); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Join/Meet Consistency
// ═══════════════════════════════════════════════════════════════════════════

mod consistency {
    use super::*;
    #[test] fn join_upper() { let j=t_smallint().join(&t_float()); assert!(j.contains(&t_smallint())); assert!(j.contains(&t_float())); }
    #[test] fn meet_lower() { let m=t_smallint().meet(&t_nonneg()); assert!(t_smallint().contains(&m)); assert!(t_nonneg().contains(&m)); }
    #[test] fn join_least_upper() { let j=t_nonneg().join(&t_const_int(5)); let c=t_smallint(); assert!(c.contains(&j)); }
    #[test] fn meet_greatest_lower() { let m=t_smallint().meet(&t_int64()); let c=t_nonneg(); assert!(m.contains(&c)); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Constant Propagation
// ═══════════════════════════════════════════════════════════════════════════

mod constants {
    use super::*;
    #[test] fn const_join_same() { let a=t_const_int(42); assert_eq!(a.join(&a),a); }
    #[test] fn const_join_diff() { assert_eq!(t_const_int(1).join(&t_const_int(2)),t_smallint()); }
    #[test] fn const_join_general() { assert_eq!(t_const_int(42).join(&t_smallint()),t_smallint()); }
    #[test] fn const_meet_same() { let a=t_const_int(42); assert_eq!(a.meet(&a),a); }
    #[test] fn const_meet_diff() { assert!(t_const_int(1).meet(&t_const_int(2)).is_empty()); }
    #[test] fn const_meet_super() { assert_eq!(t_const_int(42).meet(&t_smallint()),t_const_int(42)); }
    #[test] fn const_meet_incompat() { assert!(t_const_int(42).meet(&t_float()).is_empty()); }
    #[test] fn const_true_false() { assert_eq!(t_const_true().join(&t_const_false()),t_bool()); }
    #[test] fn const_nil_cons() { assert_eq!(t_const_nil().join(&t_cons()),t_list()); }
    #[test] fn const_contains() { assert!(t_smallint().contains(&t_const_int(42))); assert!(t_nonneg().contains(&t_const_int(0))); assert!(!t_nonneg().contains(&t_const_int(-1))); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Nested Compound Types
// ═══════════════════════════════════════════════════════════════════════════

mod nested {
    use super::*;
    #[test] fn union_of_inters() { let a=t_inter(t_smallint(),t_nonneg()); let b=t_inter(t_float(),t_int64()); assert_eq!(a.join(&b),t_union(t_nonneg(),t_float())); }
    #[test] fn inter_of_unions() { let a=t_union(t_smallint(),t_float()); let b=t_union(t_nonneg(),t_float()); let r=a.meet(&b); assert!(t_float().contains(&r)||r==t_float()); }
    #[test] fn diff_of_union() { let a=t_union(t_smallint(),t_float()); let r=a.subtract(&t_smallint()); assert!(!r.contains(&t_smallint())||r==t_float()); }
    #[test] fn deep_norm() { let i=t_union(t_smallint(),t_float()); let o=t_union(i,t_atom()); assert_eq!(o.normalize().union_arity(),3); }
    #[test] fn inter_norm() { let i=t_inter(t_smallint(),t_nonneg()); let o=t_inter(i,t_int64()); assert_eq!(o.normalize(),t_nonneg()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Exhaustiveness
// ═══════════════════════════════════════════════════════════════════════════

mod exhaustiveness {
    use super::*;
    #[test] fn empty_not_exhaustive() { assert!(!t_smallint().is_exhaustive(&[])); }
    #[test] fn empty_exhaustive_bottom() { assert!(t_bottom().is_exhaustive(&[])); }
    #[test] fn any_exhaustive() { assert!(t_any().is_exhaustive(&[t_any()])); }
    #[test] fn overlapping() { assert!(t_smallint().is_exhaustive(&[t_smallint(),t_nonneg()])); }
    #[test] fn bool_exhaustive() { assert!(t_bool().is_exhaustive(&[t_const_true(),t_const_false()])); }
    #[test] fn bool_not_exhaustive() { assert!(!t_bool().is_exhaustive(&[t_const_true()])); }
    #[test] fn list_exhaustive() { assert!(t_list().is_exhaustive(&[t_nil(),t_cons()])); }
    #[test] fn int_not_exhaustive() { assert!(!t_smallint().is_exhaustive(&[t_const_int(0)])); }
    #[test] fn uncovered_bottom() { assert!(t_list().uncovered_by(&[t_nil(),t_cons()]).is_empty()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Normalization
// ═══════════════════════════════════════════════════════════════════════════

mod normalization {
    use super::*;
    #[test] fn norm_idem_simple() { let a=t_smallint(); assert_eq!(a.normalize(),a); }
    #[test] fn norm_idem_union() { let a=t_union(t_smallint(),t_float()); assert_eq!(a.normalize().normalize(),a.normalize()); }
    #[test] fn norm_absorb() { assert_eq!(t_union(t_smallint(),t_int64()).normalize(),t_int64()); }
    #[test] fn norm_absorb_rev() { assert_eq!(t_union(t_int64(),t_smallint()).normalize(),t_int64()); }
    #[test] fn norm_nonneg() { assert_eq!(t_union(t_nonneg(),t_smallint()).normalize(),t_smallint()); }
    #[test] fn norm_flatten() { let i=t_union(t_smallint(),t_float()); let o=t_union(i,t_atom()); assert_eq!(o.normalize().union_arity(),3); assert_eq!(o.normalize().normalize(),o.normalize()); }
    #[test] fn norm_inter() { assert_eq!(t_inter(t_smallint(),t_nonneg()).normalize(),t_nonneg()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Message Types
// ═══════════════════════════════════════════════════════════════════════════

mod message_types {
    use super::*;
    #[test] fn msg_covariant_payload() { let a=t_msg(t_smallint(),MessagePriority::High); let b=t_msg(t_nonneg(),MessagePriority::High); assert!(!b.contains(&a)); }
    #[test] fn msg_contra_priority() { let hi=t_msg(t_atom(),MessagePriority::High); let lo=t_msg(t_atom(),MessagePriority::Normal); assert!(lo.contains(&hi)); assert!(!hi.contains(&lo)); }
    #[test] fn msg_join_priority() { assert_eq!(t_msg(t_atom(),MessagePriority::Low).join(&t_msg(t_atom(),MessagePriority::Critical)).message_priority(),Some(MessagePriority::Critical)); }
    #[test] fn msg_meet_priority() { assert_eq!(t_msg(t_atom(),MessagePriority::High).meet(&t_msg(t_atom(),MessagePriority::Low)).message_priority(),Some(MessagePriority::Low)); }
    #[test] fn msg_join_payload() { let j=t_msg(t_smallint(),MessagePriority::Normal).join(&t_msg(t_float(),MessagePriority::Normal)); assert!(j.contains(&t_msg(t_smallint(),MessagePriority::Normal))); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Actor Types
// ═══════════════════════════════════════════════════════════════════════════

mod actor_types {
    use super::*;
    #[test] fn actor_sub_same_lc() { let a=t_actor(vec![t_smallint()],ActorLifecycle::Permanent); let b=t_actor(vec![t_smallint(),t_float()],ActorLifecycle::Permanent); assert!(b.contains(&a)); }
    #[test] fn actor_sub_diff_lc() { let a=t_actor(vec![t_smallint()],ActorLifecycle::Permanent); let b=t_actor(vec![t_smallint()],ActorLifecycle::Transient); assert!(!a.contains(&b)); assert!(!b.contains(&a)); }
    #[test] fn actor_join() { let j=t_actor(vec![t_smallint()],ActorLifecycle::Permanent).join(&t_actor(vec![t_float()],ActorLifecycle::Permanent)); assert!(j.contains(&t_actor(vec![t_smallint()],ActorLifecycle::Permanent))); }
    #[test] fn actor_meet() { let m=t_actor(vec![t_smallint(),t_float()],ActorLifecycle::Permanent).meet(&t_actor(vec![t_float(),t_atom()],ActorLifecycle::Permanent)); assert!(m.contains(&t_actor(vec![t_float()],ActorLifecycle::Permanent))); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. Tensor Types
// ═══════════════════════════════════════════════════════════════════════════

mod tensor_types {
    use super::*;
    #[test] fn tensor_sub_same() { let a=t_tensor(TensorDtype::F32,vec![Some(1),Some(224)]); assert!(a.contains(&t_tensor(TensorDtype::F32,vec![Some(1),Some(224)]))); }
    #[test] fn tensor_sub_dtype() { assert!(!t_tensor(TensorDtype::F32,vec![Some(1)]).contains(&t_tensor(TensorDtype::F64,vec![Some(1)]))); }
    #[test] fn tensor_sub_dyn_dim() { assert!(t_tensor(TensorDtype::F32,vec![None,Some(224)]).contains(&t_tensor(TensorDtype::F32,vec![Some(1),Some(224)]))); }
    #[test] fn tensor_sub_rank() { assert!(!t_tensor(TensorDtype::F32,vec![Some(1)]).contains(&t_tensor(TensorDtype::F32,vec![Some(1),Some(2)]))); }
    #[test] fn tensor_join() { let j=t_tensor(TensorDtype::F32,vec![Some(1)]).join(&t_tensor(TensorDtype::F32,vec![Some(2)])); assert!(j.contains(&t_tensor(TensorDtype::F32,vec![Some(1)]))); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. MapShape Types
// ═══════════════════════════════════════════════════════════════════════════

mod map_shape_types {
    use super::*;
    #[test] fn mshape_sub_same_keys() { let a=t_mapshape(vec![1,2],vec![t_smallint(),t_atom()]); let b=t_mapshape(vec![1,2],vec![t_nonneg(),t_atom()]); assert!(!a.contains(&b)); assert!(b.contains(&a)); }
    #[test] fn mshape_sub_diff_keys() { assert!(!t_mapshape(vec![1],vec![t_smallint()]).contains(&t_mapshape(vec![2],vec![t_smallint()]))); }
    #[test] fn map_contains_shape() { assert!(t_map().contains(&t_mapshape(vec![1],vec![t_smallint()]))); }
    #[test] fn mshape_join_match() { let j=t_mapshape(vec![1],vec![t_smallint()]).join(&t_mapshape(vec![1],vec![t_nonneg()])); assert!(j.is_map_shape()); assert_eq!(j.map_shape().unwrap().1[0],t_smallint()); }
    #[test] fn mshape_join_mismatch() { assert_eq!(t_mapshape(vec![1],vec![t_smallint()]).join(&t_mapshape(vec![2],vec![t_float()])),t_map()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. StableTuple Types
// ═══════════════════════════════════════════════════════════════════════════

mod stable_tuple_types {
    use super::*;
    #[test] fn st_sub_element() { let a=t_stable(vec![t_smallint(),t_atom()],true); let b=t_stable(vec![t_nonneg(),t_atom()],true); assert!(!a.contains(&b)); assert!(b.contains(&a)); }
    #[test] fn st_sub_arity() { assert!(!t_stable(vec![t_smallint()],true).contains(&t_stable(vec![t_smallint(),t_atom()],true))); }
    #[test] fn st_contains_tuple() { assert!(t_tuple(2).contains(&t_stable(vec![t_smallint(),t_atom()],true))); }
    #[test] fn st_join_arity() { assert!(matches!(t_stable(vec![t_smallint()],true).join(&t_stable(vec![t_smallint(),t_atom()],true)).kind,TypeKind::Tuple{arity:2})); }
    #[test] fn st_immutable_and() { let j=t_stable(vec![t_smallint()],true).join(&t_stable(vec![t_smallint()],false)); if let TypeKind::StableTuple{immutable,..}=&j.kind{assert!(!*immutable);} }
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. Dynamic Type
// ═══════════════════════════════════════════════════════════════════════════

mod dynamic_type {
    use super::*;
    #[test] fn dyn_join_any() { assert_eq!(t_dynamic().join(&t_smallint()),t_any()); assert_eq!(t_dynamic().join(&t_any()),t_any()); assert_eq!(t_smallint().join(&t_dynamic()),t_any()); }
    #[test] fn dyn_meet() { assert_eq!(t_dynamic().meet(&t_smallint()),t_smallint()); assert_eq!(t_smallint().meet(&t_dynamic()),t_smallint()); }
    #[test] fn dyn_contains_all() { assert!(t_dynamic().contains(&t_smallint())); assert!(t_dynamic().contains(&t_any())); assert!(t_dynamic().contains(&t_bottom())); }
    #[test] fn dyn_not_immutable() { assert!(!t_dynamic().is_immutable()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. RecursiveVar
// ═══════════════════════════════════════════════════════════════════════════

mod recursive_var {
    use super::*;
    #[test] fn rec_bounded_join() { assert_eq!(t_recvar(0,Some(t_smallint())).join(&t_nonneg()),t_smallint().join(&t_nonneg())); }
    #[test] fn rec_unbounded_join() { assert_eq!(t_recvar(0,None).join(&t_smallint()),t_any()); }
    #[test] fn rec_bounded_meet() { assert_eq!(t_recvar(0,Some(t_smallint())).meet(&t_nonneg()),t_smallint().meet(&t_nonneg())); }
    #[test] fn rec_unbounded_meet() { assert!(t_recvar(0,None).meet(&t_smallint()).is_empty()); }
    #[test] fn rec_bounded_contains() { assert!(t_recvar(0,Some(t_smallint())).contains(&t_nonneg())); assert!(!t_recvar(0,Some(t_smallint())).contains(&t_float())); }
    #[test] fn rec_unbounded_contains() { assert!(!t_recvar(0,None).contains(&t_smallint())); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. Speculative Type
// ═══════════════════════════════════════════════════════════════════════════

mod speculative_type {
    use super::*;
    #[test] fn spec_join_actual() { let s=t_spec(t_smallint(),t_float(),SpeculativeGuard::Trivial); assert_eq!(s.join(&t_atom()),t_float().join(&t_atom())); }
    #[test] fn spec_meet_assumed() { let s=t_spec(t_smallint(),t_float(),SpeculativeGuard::Trivial); assert_eq!(s.meet(&t_nonneg()),t_smallint().meet(&t_nonneg())); }
    #[test] fn spec_contains_assumed() { let s=t_spec(t_smallint(),t_float(),SpeculativeGuard::Trivial); assert!(s.contains(&t_nonneg())); assert!(!s.contains(&t_float())); }
    #[test] fn spec_display() { let s=t_spec(t_smallint(),t_float(),SpeculativeGuard::Trivial); let d=format!("{}",s); assert!(d.contains("spec")); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. Difference Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

mod difference_edge {
    use super::*;
    #[test] fn diff_self_bottom() { assert!(t_smallint().subtract(&t_smallint()).is_empty()); }
    #[test] fn diff_bottom_self() { assert_eq!(t_smallint().subtract(&t_bottom()),t_smallint()); }
    #[test] fn diff_any_bottom() { assert!(t_smallint().subtract(&t_any()).is_empty()); }
    #[test] fn diff_subtype() { let r=t_smallint().subtract(&t_nonneg()); assert!(!r.contains(&t_nonneg())); assert!(r.contains(&t_const_int(-1))); }
    #[test] fn diff_disjoint() { assert_eq!(t_smallint().subtract(&t_float()),t_smallint()); }
    #[test] fn diff_const() { let r=t_smallint().subtract(&t_const_int(0)); assert!(!r.contains(&t_const_int(0))); assert!(r.contains(&t_const_int(1))); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Intersection Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

mod intersection_edge {
    use super::*;
    #[test] fn inter_self() { let a=t_smallint(); assert_eq!(t_inter(a.clone(),a.clone()),a); }
    #[test] fn inter_bottom() { assert!(t_inter(t_smallint(),t_bottom()).is_empty()); }
    #[test] fn inter_any() { assert_eq!(t_inter(t_smallint(),t_any()),t_smallint()); }
    #[test] fn inter_narrows() { assert_eq!(t_inter(t_smallint(),t_nonneg()),t_nonneg()); }
    #[test] fn inter_disjoint() { assert!(t_inter(t_smallint(),t_float()).is_empty()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Union Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

mod union_edge {
    use super::*;
    #[test] fn union_self() { let a=t_smallint(); assert_eq!(t_union(a.clone(),a.clone()),a); }
    #[test] fn union_bottom() { assert_eq!(t_union(t_smallint(),t_bottom()),t_smallint()); }
    #[test] fn union_any() { assert_eq!(t_union(t_smallint(),t_any()),t_any()); }
    #[test] fn union_widen() { assert_eq!(t_union(t_smallint(),t_float()),t_any()); }
    #[test] fn union_subtype() { assert_eq!(t_union(t_smallint(),t_nonneg()),t_smallint()); }
    #[test] fn union_contains_both() { let u=t_union(t_smallint(),t_float()); assert!(u.contains(&t_smallint())); assert!(u.contains(&t_float())); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. Capability Types
// ═══════════════════════════════════════════════════════════════════════════

mod capability_edge {
    use super::*;
    #[test] fn cap_sub_owned() { let a=t_cap(NativeResourceKind::GpuContext,true,false); let b=t_cap(NativeResourceKind::GpuContext,false,false); assert!(b.contains(&a)); assert!(!a.contains(&b)); }
    #[test] fn cap_sub_resource() { assert!(!t_cap(NativeResourceKind::GpuContext,true,true).contains(&t_cap(NativeResourceKind::MlModel,true,true))); }
    #[test] fn cap_join() { let j=t_cap(NativeResourceKind::GpuContext,true,false).join(&t_cap(NativeResourceKind::GpuContext,false,true)); if let TypeKind::Capability{owned,shareable,..}=&j.kind{assert!(*owned);assert!(*shareable);} }
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. Beam Identity Types
// ═══════════════════════════════════════════════════════════════════════════

mod beam_identity {
    use super::*;
    #[test] fn pid_port_distinct() { assert!(!t_pid().contains(&t_port())); assert!(!t_port().contains(&t_pid())); assert!(t_any().contains(&t_pid())); }
    #[test] fn reference_distinct() { assert!(!t_ref().contains(&t_pid())); assert!(t_any().contains(&t_ref())); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 22. Fun Types
// ═══════════════════════════════════════════════════════════════════════════

mod fun_edge {
    use super::*;
    #[test] fn fun_same_arity() { assert!(t_fun(2).contains(&t_fun(2))); }
    #[test] fn fun_diff_arity() { assert!(!t_fun(2).contains(&t_fun(3))); }
    #[test] fn fun_join_same() { assert_eq!(t_fun(2).join(&t_fun(2)),t_fun(2)); }
    #[test] fn fun_join_diff() { assert_eq!(t_fun(2).join(&t_fun(3)),t_any()); }
}

// ═══════════════════════════════════════════════════════════════════════════
// 23. Complex Scenarios
// ═══════════════════════════════════════════════════════════════════════════

mod complex_scenarios {
    use super::*;
    #[test] fn pattern_narrowing() { let x=t_any(); assert_eq!(x.meet(&t_smallint()),t_smallint()); assert_eq!(x.meet(&t_float()),t_float()); }
    #[test] fn control_flow_merge() { assert_eq!(t_const_int(42).join(&t_float()),t_any()); }
    #[test] fn receive_types() { let j=t_msg(t_tuple(2),MessagePriority::High).join(&t_msg(t_atom(),MessagePriority::Normal)); assert!(j.is_message()); }
    #[test] fn actor_protocol() { let at=t_actor(vec![t_tuple(2),t_tuple(2)],ActorLifecycle::Permanent); assert!(at.is_actor()); }
    #[test] fn tensor_shape() { let i=t_tensor(TensorDtype::F32,vec![None,Some(128)]); let o=t_tensor(TensorDtype::F32,vec![None,Some(256)]); let g=t_tensor(TensorDtype::F32,vec![None,None]); assert!(g.contains(&i)); assert!(g.contains(&o)); }
    #[test] fn map_struct() { let s=t_mapshape(vec![1,2],vec![t_smallint(),t_bin()]); assert!(t_map().contains(&s)); assert!(s.contains(&t_mapshape(vec![1,2],vec![t_const_int(42),t_bin()]))); }
}
