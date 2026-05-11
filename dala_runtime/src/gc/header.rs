// dala_runtime/src/gc/header.rs
//
// Object header layout (64-bit word):
//
//  63       48 47    40 39  38 37  36 35       8 7          0
//  ┌──────────┬────────┬─────┬─────┬───────────┬────────────┐
//  │ type_idx │  size  │imut │color│  reserved │survival_ct │
//  └──────────┴────────┴─────┴─────┴───────────┴────────────┘
//
// color   : 2 bits  — GcColor (White/Gray/Black/StableBlack)
// imut    : 1 bit   — immutability proven by compiler
// size    : 8 bits  — object word count (0–255); large objects use LargeHeader
// type_idx: 16 bits — index into the TypeDescriptor table
// survival: 8 bits  — survived GC cycle counter; drives tier promotion
//
// A forwarding pointer replaces the entire header word during copying GC.
// The lowest 2 bits of a valid header are always 0b00 (aligned); a
// forwarding pointer sets bit 0 to distinguish it.

use std::sync::atomic::{AtomicU64, Ordering};

// ── Bit positions ──────────────────────────────────────────────────────────

const SURVIVAL_SHIFT: u64 = 0;
const SURVIVAL_MASK: u64 = 0xFF;

const COLOR_SHIFT: u64 = 36;
const COLOR_MASK: u64 = 0x3;

const IMMUTABLE_SHIFT: u64 = 38;
const IMMUTABLE_MASK: u64 = 0x1;

const SIZE_SHIFT: u64 = 40;
const SIZE_MASK: u64 = 0xFF;

const TYPE_IDX_SHIFT: u64 = 48;
const TYPE_IDX_MASK: u64 = 0xFFFF;

const FORWARD_BIT: u64 = 0x1; // bit 0 set → forwarding pointer

// ── GC tri-color + stable extension ───────────────────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcColor {
    /// Not yet visited in the current GC cycle.
    White = 0b00,
    /// Discovered; children not yet scanned.
    Gray = 0b01,
    /// Fully scanned this cycle.
    Black = 0b10,
    /// Promoted to Stable Immutable Region — never rescanned.
    StableBlack = 0b11,
}

impl GcColor {
    #[inline]
    fn from_bits(bits: u64) -> Self {
        match bits & COLOR_MASK {
            0b00 => Self::White,
            0b01 => Self::Gray,
            0b10 => Self::Black,
            0b11 => Self::StableBlack,
            _ => unreachable!(),
        }
    }
}

// ── Promotion thresholds ───────────────────────────────────────────────────

/// Young heap → Old heap after this many survived minor GCs.
pub const YOUNG_PROMOTION_THRESHOLD: u8 = 3;

/// Old heap → Stable Immutable Region after this many survived major GCs,
/// provided `immutable == true`.
pub const STABLE_PROMOTION_THRESHOLD: u8 = 5;

// ── ObjectHeader ──────────────────────────────────────────────────────────

/// Placed at the start of every heap-allocated object.
/// Internally an atomic u64 so the concurrent GC marker can update
/// color bits without locking the mutator.
#[repr(C)]
pub struct ObjectHeader(AtomicU64);

impl ObjectHeader {
    // ── Construction ───────────────────────────────────────────────────

    /// Create a fresh header for a newly allocated object.
    ///
    /// # Arguments
    /// * `type_idx` — index into the runtime `TypeDescriptor` table
    /// * `size_words` — number of payload words following the header
    /// * `immutable` — compiler-proven structural immutability
    pub fn new(type_idx: u16, size_words: u8, immutable: bool) -> Self {
        let mut raw: u64 = 0;
        raw |= 0u64 << SURVIVAL_SHIFT; // survival = 0
        raw |= (GcColor::White as u64) << COLOR_SHIFT; // color = White
        raw |= (immutable as u64) << IMMUTABLE_SHIFT;
        raw |= (size_words as u64) << SIZE_SHIFT;
        raw |= (type_idx as u64) << TYPE_IDX_SHIFT;
        Self(AtomicU64::new(raw))
    }

    // ── Raw word access ────────────────────────────────────────────────

    #[inline]
    fn load(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    // ── Forwarding pointer (copying GC) ────────────────────────────────

    /// Returns `true` if the header has been replaced by a forwarding pointer.
    #[inline]
    pub fn is_forwarded(&self) -> bool {
        self.load() & FORWARD_BIT != 0
    }

    /// Install a forwarding pointer. Called by the copying collector after
    /// moving an object to the to-space.
    ///
    /// # Safety
    /// `ptr` must be word-aligned (bit 0 free for the forward tag).
    #[inline]
    pub unsafe fn set_forward(&self, ptr: *mut u8) {
        debug_assert!(ptr as usize & 1 == 0, "forwarding ptr must be aligned");
        self.0.store(ptr as u64 | FORWARD_BIT, Ordering::Release);
    }

    /// Read the forwarding address. Only valid when `is_forwarded()` is true.
    #[inline]
    pub fn forward_ptr(&self) -> *mut u8 {
        (self.load() & !FORWARD_BIT) as *mut u8
    }

    // ── Field accessors ────────────────────────────────────────────────

    #[inline]
    pub fn survival_count(&self) -> u8 {
        ((self.load() >> SURVIVAL_SHIFT) & SURVIVAL_MASK) as u8
    }

    #[inline]
    pub fn gc_color(&self) -> GcColor {
        GcColor::from_bits(self.load() >> COLOR_SHIFT)
    }

    #[inline]
    pub fn is_immutable(&self) -> bool {
        (self.load() >> IMMUTABLE_SHIFT) & IMMUTABLE_MASK != 0
    }

    #[inline]
    pub fn size_words(&self) -> u8 {
        ((self.load() >> SIZE_SHIFT) & SIZE_MASK) as u8
    }

    #[inline]
    pub fn type_index(&self) -> u16 {
        ((self.load() >> TYPE_IDX_SHIFT) & TYPE_IDX_MASK) as u16
    }

    // ── Mutable operations ─────────────────────────────────────────────

    /// Atomically transition the GC color.
    /// Returns the previous color.
    pub fn set_color(&self, color: GcColor) -> GcColor {
        let shift = COLOR_SHIFT;
        let mask = COLOR_MASK << shift;
        let bits = (color as u64) << shift;
        // fetch_update: clear the color field, OR in the new value
        let prev = self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                Some((old & !mask) | bits)
            });
        GcColor::from_bits(prev.unwrap_or(0) >> shift)
    }

    /// Increment the survival counter, saturating at 255.
    /// Call this once per survived GC cycle.
    pub fn increment_survival(&self) {
        // TODO: replace with a proper fetch_update to handle saturation atomically
        let raw = self.load();
        let count = ((raw >> SURVIVAL_SHIFT) & SURVIVAL_MASK) as u8;
        if count < u8::MAX {
            let new_raw =
                (raw & !(SURVIVAL_MASK << SURVIVAL_SHIFT)) | ((count as u64 + 1) << SURVIVAL_SHIFT);
            self.0.store(new_raw, Ordering::Release);
        }
    }

    // ── Promotion helpers ──────────────────────────────────────────────

    /// True when the object should be promoted from young → old heap.
    #[inline]
    pub fn should_promote_to_old(&self) -> bool {
        self.survival_count() >= YOUNG_PROMOTION_THRESHOLD
    }

    /// True when the object is a candidate for the Stable Immutable Region.
    /// Requires both longevity AND compiler-proven immutability.
    #[inline]
    pub fn should_promote_to_stable(&self) -> bool {
        self.is_immutable() && self.survival_count() >= STABLE_PROMOTION_THRESHOLD
    }
}

// ── TypeDescriptor ─────────────────────────────────────────────────────────

/// Emitted by the compiler for every heap-allocated type.
/// Stored in a process-global table indexed by `ObjectHeader::type_index()`.
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Total allocation size in bytes (header included).
    pub alloc_size: u32,

    /// Bitmap: bit N is set if word N of the payload is a GC-traced pointer.
    /// Allows the GC to skip non-pointer fields entirely.
    pub pointer_map: u64,

    /// Compiler-proven structural immutability.
    /// When `true`, and survival threshold met, object can enter the SIR.
    pub immutable: bool,

    /// Optional compact native layout for SIR promotion.
    /// `None` means use the standard tagged layout.
    pub native_layout: Option<NativeLayout>,

    /// Human-readable name (debug builds only).
    #[cfg(debug_assertions)]
    pub name: &'static str,
}

impl TypeDescriptor {
    /// Returns an iterator over the byte offsets of pointer fields.
    pub fn pointer_offsets(&self) -> impl Iterator<Item = usize> + '_ {
        (0u32..64)
            .filter(move |&bit| self.pointer_map & (1 << bit) != 0)
            .map(|bit| bit as usize * std::mem::size_of::<usize>())
    }
}

/// Compact memory layout used after SIR promotion.
/// Replaces the tagged BEAM term layout with a flat, cache-friendly struct.
#[derive(Debug)]
pub struct NativeLayout {
    /// Field descriptors in declaration order.
    pub fields: &'static [NativeField],
    /// Total size of the compact representation in bytes.
    pub size: u32,
}

#[derive(Debug)]
pub struct NativeField {
    pub offset: u32,
    pub kind: NativeFieldKind,
}

#[derive(Debug)]
pub enum NativeFieldKind {
    I64,
    F64,
    Ptr, // traced GC pointer in compact layout
    Bytes { len: u32 },
}

// ── TypeDescriptor table ───────────────────────────────────────────────────

/// Global registry of type descriptors, indexed by `type_index`.
/// TODO: replace Vec with a lock-free concurrent structure when
///       supporting parallel module loading.
pub struct TypeTable {
    descriptors: Vec<TypeDescriptor>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }

    pub fn register(&mut self, desc: TypeDescriptor) -> u16 {
        let idx = self.descriptors.len();
        assert!(idx < u16::MAX as usize, "TypeTable overflow");
        self.descriptors.push(desc);
        idx as u16
    }

    pub fn get(&self, idx: u16) -> Option<&TypeDescriptor> {
        self.descriptors.get(idx as usize)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header() -> ObjectHeader {
        ObjectHeader::new(42, 4, true)
    }

    #[test]
    fn initial_state() {
        let h = make_header();
        assert_eq!(h.survival_count(), 0);
        assert_eq!(h.gc_color(), GcColor::White);
        assert!(h.is_immutable());
        assert_eq!(h.size_words(), 4);
        assert_eq!(h.type_index(), 42);
        assert!(!h.is_forwarded());
    }

    #[test]
    fn color_transition() {
        let h = make_header();
        let prev = h.set_color(GcColor::Gray);
        assert_eq!(prev, GcColor::White);
        assert_eq!(h.gc_color(), GcColor::Gray);

        h.set_color(GcColor::Black);
        assert_eq!(h.gc_color(), GcColor::Black);

        h.set_color(GcColor::StableBlack);
        assert_eq!(h.gc_color(), GcColor::StableBlack);
    }

    #[test]
    fn survival_increment_and_promotion() {
        let h = ObjectHeader::new(0, 1, true);
        for _ in 0..YOUNG_PROMOTION_THRESHOLD {
            assert!(!h.should_promote_to_old());
            h.increment_survival();
        }
        assert!(h.should_promote_to_old());
        // Keep going to stable threshold
        for _ in YOUNG_PROMOTION_THRESHOLD..STABLE_PROMOTION_THRESHOLD {
            h.increment_survival();
        }
        assert!(h.should_promote_to_stable());
    }

    #[test]
    fn non_immutable_never_stable() {
        let h = ObjectHeader::new(0, 1, false);
        for _ in 0..=STABLE_PROMOTION_THRESHOLD {
            h.increment_survival();
        }
        assert!(h.should_promote_to_old());
        assert!(
            !h.should_promote_to_stable(),
            "mutable objects must not enter SIR"
        );
    }

    #[test]
    fn forwarding_pointer_roundtrip() {
        let h = make_header();
        let mut target: u64 = 0xDEAD_BEEF_0000;
        let ptr = &mut target as *mut u64 as *mut u8;
        unsafe { h.set_forward(ptr) };
        assert!(h.is_forwarded());
        assert_eq!(h.forward_ptr(), ptr);
    }

    #[test]
    fn type_descriptor_pointer_offsets() {
        let desc = TypeDescriptor {
            alloc_size: 32,
            pointer_map: 0b0000_1010, // words 1 and 3 are pointers
            immutable: true,
            native_layout: None,
            #[cfg(debug_assertions)]
            name: "TestType",
        };
        let offsets: Vec<usize> = desc.pointer_offsets().collect();
        assert_eq!(offsets, vec![8, 24]); // word 1 = 8 bytes, word 3 = 24 bytes
    }
}
