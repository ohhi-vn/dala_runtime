//! Term representation - the fundamental data type of the BEAM VM.
//!
//! BEAM uses a tagged pointer representation for terms. This implementation
//! uses a 64-bit tagged word scheme inspired by the real BEAM VM.

use core::fmt;
use core::hash::{Hash, Hasher};

/// A BEAM term - the fundamental value type.
#[repr(transparent)]
#[derive(Copy, Clone, Eq)]
pub struct Term(u64);

/// Tag constants for term encoding
pub mod tags {
    pub const PRIMARY_TAG_MASK: u64 = 0b11;
    pub const PRIMARY_TAG_BOXED: u64 = 0b00;
    pub const PRIMARY_TAG_LIST: u64 = 0b01;
    pub const PRIMARY_TAG_HEADER: u64 = 0b10;
    pub const PRIMARY_TAG_IMMED1: u64 = 0b11;

    pub const IMMED1_TAG_MASK: u64 = 0b1111 << 28;
    pub const IMMED1_TAG_SHIFT: u64 = 28;
    pub const IMMED1_SMALL: u64 = 0b0000 << 28;
    pub const IMMED1_PID: u64 = 0b0001 << 28;
    pub const IMMED1_PORT: u64 = 0b0010 << 28;
    pub const IMMED1_IMMED2: u64 = 0b0011 << 28;
    pub const IMMED1_LOCAL_PID: u64 = 0b0100 << 28;
    pub const IMMED1_LOCAL_PORT: u64 = 0b0101 << 28;

    pub const IMMED2_TAG_MASK: u64 = 0b111 << 25;
    pub const IMMED2_TAG_SHIFT: u64 = 25;
    pub const IMMED2_ATOM: u64 = 0b000 << 25;
    pub const IMMED2_CATCH: u64 = 0b001 << 25;
    pub const IMMED2_XREG: u64 = 0b010 << 25;
    pub const IMMED2_YREG: u64 = 0b011 << 25;
    pub const IMMED2_SPECIAL: u64 = 0b100 << 25;

    pub const SPECIAL_NIL: u64 = (IMMED2_SPECIAL | 0x00) << 25;
    pub const SPECIAL_TRUE: u64 = (IMMED2_SPECIAL | 0x01) << 25;
    pub const SPECIAL_FALSE: u64 = (IMMED2_SPECIAL | 0x02) << 25;

    pub const HEADER_TAG_POS: u64 = 32;
    pub const HEADER_TAG_MASK: u64 = 0xFFFF << HEADER_TAG_POS;
    pub const HEADER_ARITY_MASK: u64 = 0x0000_FFFF_FFFF;

    pub const HEADER_TUPLE: u64 = 0b0000 << HEADER_TAG_POS;
    pub const HEADER_POS_BIG: u64 = 0b0001 << HEADER_TAG_POS;
    pub const HEADER_NEG_BIG: u64 = 0b0010 << HEADER_TAG_POS;
    pub const HEADER_FLOAT: u64 = 0b0011 << HEADER_TAG_POS;
    pub const HEADER_EXPORT: u64 = 0b0100 << HEADER_TAG_POS;
    pub const HEADER_FUN: u64 = 0b0101 << HEADER_TAG_POS;
    pub const HEADER_BIN_MATCHSTATE: u64 = 0b0110 << HEADER_TAG_POS;
    pub const HEADER_SUB_BIN: u64 = 0b0111 << HEADER_TAG_POS;
    pub const HEADER_MAP: u64 = 0b1000 << HEADER_TAG_POS;
    pub const HEADER_REFC_BIN: u64 = 0b1001 << HEADER_TAG_POS;
    pub const HEADER_HEAP_BIN: u64 = 0b1010 << HEADER_TAG_POS;
    pub const HEADER_SUB_BIN_FULL: u64 = 0b1011 << HEADER_TAG_POS;

    pub const BOXED_PTR_MASK: u64 = !0b11;
}

impl Term {
    #[inline]
    pub const fn from_raw(bits: u64) -> Self {
        Term(bits)
    }

    #[inline]
    pub const fn to_raw(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn is_small(self) -> bool {
        (self.0 & tags::IMMED1_TAG_MASK) == tags::IMMED1_SMALL
    }

    #[inline]
    pub fn is_atom(self) -> bool {
        self.0 & tags::IMMED1_TAG_MASK == tags::IMMED1_IMMED2
            && self.0 & tags::IMMED2_TAG_MASK == tags::IMMED2_ATOM
    }

    #[inline]
    pub fn is_list(self) -> bool {
        (self.0 & tags::PRIMARY_TAG_MASK) == tags::PRIMARY_TAG_LIST
    }

    #[inline]
    pub fn is_tuple(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_TUPLE
    }

    #[inline]
    pub fn is_map(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_MAP
    }

    #[inline]
    pub fn is_boxed(self) -> bool {
        (self.0 & tags::PRIMARY_TAG_MASK) == tags::PRIMARY_TAG_BOXED
    }

    #[inline]
    pub fn is_float(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_FLOAT
    }

    #[inline]
    pub fn is_binary(self) -> bool {
        self.is_boxed()
            && (self.header_tag() == tags::HEADER_REFC_BIN
                || self.header_tag() == tags::HEADER_HEAP_BIN)
    }

    #[inline]
    pub fn is_fun(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_FUN
    }

    #[inline]
    pub fn is_nil(self) -> bool {
        self.0 == tags::SPECIAL_NIL
    }

    #[inline]
    pub fn is_true(self) -> bool {
        self.0 == tags::SPECIAL_TRUE
    }

    #[inline]
    pub fn is_false(self) -> bool {
        self.0 == tags::SPECIAL_FALSE
    }

    #[inline]
    pub fn is_pid(self) -> bool {
        (self.0 & tags::IMMED1_TAG_MASK) == tags::IMMED1_PID
    }

    #[inline]
    pub fn is_port(self) -> bool {
        (self.0 & tags::IMMED1_TAG_MASK) == tags::IMMED1_PORT
    }

    #[inline]
    pub fn is_catch(self) -> bool {
        self.0 & tags::IMMED1_TAG_MASK == tags::IMMED1_IMMED2
            && self.0 & tags::IMMED2_TAG_MASK == tags::IMMED2_CATCH
    }

    // --- Small integer operations ---

    #[inline]
    pub fn small(val: i64) -> Self {
        Term(((val as u64) << 4) | 0x0F)
    }

    #[inline]
    pub fn get_small(self) -> Option<i64> {
        if self.is_small() {
            Some((self.0 as i64) >> 4)
        } else {
            None
        }
    }

    #[inline]
    pub fn unwrap_small(self) -> i64 {
        self.get_small().expect("term is not a small integer")
    }

    // --- Atom operations ---

    #[inline]
    pub fn atom(index: u32) -> Self {
        Term(tags::IMMED1_IMMED2 | tags::IMMED2_ATOM | (index as u64))
    }

    #[inline]
    pub fn get_atom_index(self) -> Option<u32> {
        if self.is_atom() {
            Some((self.0 & 0x00FF_FFFF) as u32)
        } else {
            None
        }
    }

    // --- List operations ---

    #[inline]
    pub fn get_list_ptr(self) -> *const Term {
        (self.0 & tags::BOXED_PTR_MASK) as *const Term
    }

    #[inline]
    pub fn get_list_ptr_mut(self) -> *mut Term {
        (self.0 & tags::BOXED_PTR_MASK) as *mut Term
    }

    // --- Boxed value operations ---

    #[inline]
    pub fn get_boxed_ptr(self) -> *const Term {
        (self.0 & tags::BOXED_PTR_MASK) as *const Term
    }

    #[inline]
    pub fn get_boxed_ptr_mut(self) -> *mut Term {
        (self.0 & tags::BOXED_PTR_MASK) as *mut Term
    }

    #[inline]
    pub fn header(self) -> u64 {
        unsafe { *self.get_boxed_ptr() }
    }

    #[inline]
    pub fn header_arity(header: u64) -> usize {
        (header & tags::HEADER_ARITY_MASK) as usize
    }

    #[inline]
    pub fn header_tag(self) -> u64 {
        let h = self.header();
        h & tags::HEADER_TAG_MASK
    }

    #[inline]
    pub fn tuple_get(self, i: usize) -> Term {
        debug_assert!(self.is_tuple());
        unsafe { *self.get_boxed_ptr().add(1 + i) }
    }

    #[inline]
    pub fn tuple_data_ptr(self) -> *const Term {
        unsafe { self.get_boxed_ptr().add(1) }
    }

    #[inline]
    pub fn tuple_data_ptr_mut(self) -> *mut Term {
        unsafe { self.get_boxed_ptr_mut().add(1) }
    }

    #[inline]
    pub const fn nil() -> Self {
        Term(tags::SPECIAL_NIL)
    }

    #[inline]
    pub const fn true_() -> Self {
        Term(tags::SPECIAL_TRUE)
    }

    #[inline]
    pub const fn false_() -> Self {
        Term(tags::SPECIAL_FALSE)
    }

    #[inline]
    pub fn bool(b: bool) -> Self {
        if b {
            Term::true_()
        } else {
            Term::false_()
        }
    }

    #[inline]
    pub fn get_float_ptr(self) -> *const f64 {
        debug_assert!(self.is_float());
        unsafe { (self.get_boxed_ptr() as *const u64).add(1) as *const f64 }
    }

    #[inline]
    pub fn get_float(self) -> Option<f64> {
        if self.is_float() {
            Some(unsafe { *self.get_float_ptr() })
        } else {
            None
        }
    }
}

impl fmt::Debug for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_small() {
            write!(f, "{}", self.unwrap_small())
        } else if self.is_nil() {
            write!(f, "[]")
        } else if self.is_true() {
            write!(f, "true")
        } else if self.is_false() {
            write!(f, "false")
        } else if self.is_atom() {
            write!(f, "atom({})", self.get_atom_index().unwrap())
        } else if self.is_tuple() {
            let arity = Term::header_arity(self.header());
            write!(f, "{{tuple,{}}}", arity)
        } else if self.is_list() {
            write!(f, "list(ptr={:?})", self.get_list_ptr())
        } else if self.is_float() {
            if let Some(val) = self.get_float() {
                write!(f, "{}", val)
            } else {
                write!(f, "float(corrupted)")
            }
        } else if self.is_boxed() {
            write!(f, "boxed(ptr={:?}, header={:#x})", self.get_boxed_ptr(), self.header())
        } else if self.is_pid() {
            write!(f, "pid({:#x})", self.0)
        } else if self.is_port() {
            write!(f, "port({:#x})", self.0)
        } else if self.is_fun() {
            write!(f, "fun(ptr={:?})", self.get_boxed_ptr())
        } else if self.is_map() {
            write!(f, "map(ptr={:?})", self.get_boxed_ptr())
        } else {
            write!(f, "term({:#018x})", self.0)
        }
    }
}

impl PartialEq for Term {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for Term {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// A register file for BEAM registers (x0-x255, y0-yN, f0-f255).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct RegisterFile {
    pub x: [Term; 256],
    pub y: [Term; 1024],
    pub f: [f64; 256],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            x: [Term::nil(); 256],
            y: [Term::nil(); 1024],
            f: [0.0f64; 256],
        }
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}
