//! Term representation - the fundamental data type of the BEAM VM.
//!
//! BEAM uses a tagged pointer representation for terms. This implementation
//! uses a 64-bit tagged word scheme inspired by the real BEAM VM.

use core::fmt;
use core::hash::{Hash, Hasher};
use num_bigint::BigInt;

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
    pub fn is_big(self) -> bool {
        self.is_boxed()
            && (self.header_tag() == tags::HEADER_POS_BIG
                || self.header_tag() == tags::HEADER_NEG_BIG)
    }

    #[inline]
    pub fn is_positive_big(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_POS_BIG
    }

    #[inline]
    pub fn is_negative_big(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_NEG_BIG
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
    pub fn is_fun(self) -> bool {
        self.is_boxed() && self.header_tag() == tags::HEADER_FUN
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
        unsafe { (*self.get_boxed_ptr()).to_raw() }
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
        if b { Term::true_() } else { Term::false_() }
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

    // --- Bignum operations ---

    /// Create a bignum from a BigInt.
    /// Returns a boxed term pointing to the heap-allocated bignum.
    /// The bignum is stored as:
    /// - Header word (HEADER_POS_BIG or HEADER_NEG_BIG with arity = number of 64-bit words + 1)
    /// - Sign word (0 for positive, 1 for negative)
    /// - magnitude words (little-endian, as per BEAM convention)
    pub fn bigint(big: &BigInt, heap: &mut Vec<Term>) -> Option<Self> {
        let (sign, magnitude) = big.to_bytes_le();
        let word_count = (magnitude.len() + 7) / 8; // Round up to 64-bit words
        let arity = word_count + 1; // +1 for sign word

        // Create header
        let header = if big.sign() == num_bigint::Sign::Minus {
            tags::HEADER_NEG_BIG | (arity as u64)
        } else {
            tags::HEADER_POS_BIG | (arity as u64)
        };

        // Allocate on heap
        heap.push(Term(header));

        // Sign word
        let sign_word = match sign {
            num_bigint::Sign::Minus => 1u64,
            _ => 0u64,
        };
        heap.push(Term(sign_word));

        // Magnitude words (pad to word boundary)
        let mut mag_bytes = magnitude.to_vec();
        mag_bytes.resize(word_count * 8, 0u8);
        for chunk in mag_bytes.chunks_exact(8) {
            let word = u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
            heap.push(Term(word));
        }

        // Return boxed pointer to header
        let ptr = heap.as_ptr() as u64;
        Some(Term(ptr | tags::PRIMARY_TAG_BOXED))
    }

    /// Get the BigInt value from a bignum term.
    pub fn get_bigint(self) -> Option<BigInt> {
        if !self.is_big() {
            return None;
        }
        unsafe {
            let header_ptr = self.get_boxed_ptr() as *const u64;
            let header = *header_ptr;
            let arity = (header & tags::HEADER_ARITY_MASK) as usize;

            // Sign word is at header + 1
            let sign_word = *(header_ptr.add(1));

            // Magnitude starts at header + 2
            let mag_ptr = header_ptr.add(2) as *const u8;
            let mag_len = (arity - 1) * 8; // -1 for sign word
            let mag_bytes = std::slice::from_raw_parts(mag_ptr, mag_len);

            let sign = if sign_word == 1 {
                num_bigint::Sign::Minus
            } else {
                num_bigint::Sign::Plus
            };

            Some(BigInt::from_bytes_le(sign, mag_bytes))
        }
    }
}

impl Default for Term {
    fn default() -> Self {
        Self::nil()
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
            write!(
                f,
                "boxed(ptr={:?}, header={:#x})",
                self.get_boxed_ptr(),
                self.header()
            )
        } else if self.is_pid() {
            write!(f, "pid({:#x})", self.0)
        } else if self.is_port() {
            write!(f, "port({:#x})", self.0)
        } else if self.is_big() {
            if let Some(big) = self.get_bigint() {
                write!(f, "big({})", big)
            } else {
                write!(f, "big(corrupted)")
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    #[test]
    fn test_small_int() {
        let term = Term::small(42);
        assert!(term.is_small());
        assert_eq!(term.unwrap_small(), 42);
    }

    #[test]
    fn test_negative_small_int() {
        let term = Term::small(-1);
        assert!(term.is_small());
        assert_eq!(term.unwrap_small(), -1);
    }

    #[test]
    fn test_nil() {
        let term = Term::nil();
        assert!(term.is_nil());
    }

    #[test]
    fn test_bool() {
        assert!(Term::true_().is_true());
        assert!(Term::false_().is_false());
        assert_eq!(Term::bool(true), Term::true_());
        assert_eq!(Term::bool(false), Term::false_());
    }

    #[test]
    fn test_atom() {
        let term = Term::atom(42);
        assert!(term.is_atom());
        assert_eq!(term.get_atom_index(), Some(42));
    }

    #[test]
    fn test_bignum_positive() {
        let big = BigInt::from(123456789012345678901234567890i128);
        let mut heap = Vec::new();
        let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");

        assert!(term.is_big());
        assert!(term.is_positive_big());
        assert!(!term.is_negative_big());

        let recovered = term.get_bigint().expect("Failed to get bignum");
        assert_eq!(recovered, big);
    }

    #[test]
    fn test_bignum_negative() {
        let big = BigInt::from(-123456789012345678901234567890i128);
        let mut heap = Vec::new();
        let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");

        assert!(term.is_big());
        assert!(!term.is_positive_big());
        assert!(term.is_negative_big());

        let recovered = term.get_bigint().expect("Failed to get bignum");
        assert_eq!(recovered, big);
    }

    #[test]
    fn test_bignum_zero() {
        let big = BigInt::from(0);
        let mut heap = Vec::new();
        let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");

        assert!(term.is_big());
        let recovered = term.get_bigint().expect("Failed to get bignum");
        assert_eq!(recovered, big);
    }

    #[test]
    fn test_bignum_large() {
        // Test with a very large number that definitely exceeds 64 bits
        let big = BigInt::parse_bytes(b"1234567890123456789012345678901234567890", 10)
            .expect("Failed to parse big number");
        let mut heap = Vec::new();
        let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");

        let recovered = term.get_bigint().expect("Failed to get bignum");
        assert_eq!(recovered, big);
    }

    #[test]
    fn test_get_bigint_on_non_big() {
        let small = Term::small(42);
        assert!(small.get_bigint().is_none());

        let atom = Term::atom(0);
        assert!(atom.get_bigint().is_none());
    }

    #[test]
    fn test_register_file() {
        let rf = RegisterFile::new();
        assert_eq!(rf.x[0], Term::nil());
        assert_eq!(rf.y[0], Term::nil());
        assert_eq!(rf.f[0], 0.0f64);
    }
}
