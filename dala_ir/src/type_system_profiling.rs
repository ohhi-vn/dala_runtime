//! Profiling and tracing tools for the set-theoretic type system.
//!
//! This module provides tools for:
//! - **Profiling**: Measuring performance of type operations (join, meet, contains)
//! - **Tracing**: Recording type transformations through optimization passes
//! - **Complexity analysis**: Computing type depth, width, and simplification potential
//! - **Benchmarking**: Statistical summaries of operation performance

use crate::type_system::*;
use std::fmt;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════
// TypeProfiler — Performance measurement for type operations
// ═══════════════════════════════════════════════════════════════════════════

/// Tracks performance statistics for type operations.
#[derive(Debug, Default, Clone)]
pub struct TypeProfiler {
    /// Number of join operations performed
    pub join_count: u64,
    /// Number of meet operations performed
    pub meet_count: u64,
    /// Number of contains operations performed
    pub contains_count: u64,
    /// Number of normalize operations performed
    pub normalize_count: u64,
    /// Total time spent in join operations
    pub join_time: Duration,
    /// Total time spent in meet operations
    pub meet_time: Duration,
    /// Total time spent in contains operations
    pub contains_time: Duration,
    /// Total time spent in normalize operations
    pub normalize_time: Duration,
    /// Per-operation timing samples (for percentile computation)
    join_samples: Vec<Duration>,
    meet_samples: Vec<Duration>,
    contains_samples: Vec<Duration>,
    /// Maximum number of samples to keep
    max_samples: usize,
}

impl TypeProfiler {
    /// Create a new profiler with default sample limit.
    pub fn new() -> Self {
        Self {
            max_samples: 10_000,
            ..Default::default()
        }
    }

    /// Create a profiler with a custom sample limit.
    pub fn with_sample_limit(max_samples: usize) -> Self {
        Self {
            max_samples,
            ..Default::default()
        }
    }

    /// Record a join operation with its duration.
    pub fn record_join(&mut self, duration: Duration) {
        self.join_count += 1;
        self.join_time += duration;
        if self.join_samples.len() < self.max_samples {
            self.join_samples.push(duration);
        }
    }

    /// Record a meet operation with its duration.
    pub fn record_meet(&mut self, duration: Duration) {
        self.meet_count += 1;
        self.meet_time += duration;
        if self.meet_samples.len() < self.max_samples {
            self.meet_samples.push(duration);
        }
    }

    /// Record a contains operation with its duration.
    pub fn record_contains(&mut self, duration: Duration) {
        self.contains_count += 1;
        self.contains_time += duration;
        if self.contains_samples.len() < self.max_samples {
            self.contains_samples.push(duration);
        }
    }

    /// Record a normalize operation with its duration.
    pub fn record_normalize(&mut self, duration: Duration) {
        self.normalize_count += 1;
        self.normalize_time += duration;
    }

    /// Get the average join time.
    pub fn avg_join_time(&self) -> Duration {
        if self.join_count == 0 {
            Duration::ZERO
        } else {
            self.join_time / self.join_count as u32
        }
    }

    /// Get the average meet time.
    pub fn avg_meet_time(&self) -> Duration {
        if self.meet_count == 0 {
            Duration::ZERO
        } else {
            self.meet_time / self.meet_count as u32
        }
    }

    /// Get the average contains time.
    pub fn avg_contains_time(&self) -> Duration {
        if self.contains_count == 0 {
            Duration::ZERO
        } else {
            self.contains_time / self.contains_count as u32
        }
    }

    /// Get the p95 join time (95th percentile).
    pub fn p95_join_time(&self) -> Duration {
        Self::percentile(&self.join_samples, 0.95)
    }

    /// Get the p99 join time (99th percentile).
    pub fn p99_join_time(&self) -> Duration {
        Self::percentile(&self.join_samples, 0.99)
    }

    /// Get the p95 meet time.
    pub fn p95_meet_time(&self) -> Duration {
        Self::percentile(&self.meet_samples, 0.95)
    }

    /// Get the p95 contains time.
    pub fn p95_contains_time(&self) -> Duration {
        Self::percentile(&self.contains_samples, 0.95)
    }

    /// Compute a percentile from a sorted list of samples.
    fn percentile(samples: &[Duration], p: f64) -> Duration {
        if samples.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted = samples.to_vec();
        sorted.sort();
        let idx = ((sorted.len() as f64) * p) as usize;
        let idx = idx.min(sorted.len() - 1);
        sorted[idx]
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        *self = Self {
            max_samples: self.max_samples,
            ..Default::default()
        };
    }

    /// Get total operations count.
    pub fn total_ops(&self) -> u64 {
        self.join_count + self.meet_count + self.contains_count + self.normalize_count
    }

    /// Get total time across all operations.
    pub fn total_time(&self) -> Duration {
        self.join_time + self.meet_time + self.contains_time + self.normalize_time
    }
}

impl fmt::Display for TypeProfiler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Type System Profiler ===")?;
        writeln!(f, "Total operations: {}", self.total_ops())?;
        writeln!(f, "Total time: {:?}", self.total_time())?;
        writeln!(f)?;
        writeln!(
            f,
            "  Join:     count={:>8}  avg={:?}  p95={:?}  p99={:?}",
            self.join_count,
            self.avg_join_time(),
            self.p95_join_time(),
            self.p99_join_time()
        )?;
        writeln!(
            f,
            "  Meet:     count={:>8}  avg={:?}  p95={:?}",
            self.meet_count,
            self.avg_meet_time(),
            self.p95_meet_time()
        )?;
        writeln!(
            f,
            "  Contains: count={:>8}  avg={:?}  p95={:?}",
            self.contains_count,
            self.avg_contains_time(),
            self.p95_contains_time()
        )?;
        write!(
            f,
            "  Normalize: count={:>7}  avg={:?}",
            self.normalize_count,
            if self.normalize_count == 0 {
                Duration::ZERO
            } else {
                self.normalize_time / self.normalize_count as u32
            }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TypeTracer — Records type transformations
// ═══════════════════════════════════════════════════════════════════════════

/// A snapshot of a type at a point in time.
#[derive(Debug, Clone)]
pub struct TypeSnapshot {
    pub ty: IRType,
    pub description: String,
    pub sequence: u64,
}

/// Traces type transformations through optimization passes.
#[derive(Debug, Default, Clone)]
pub struct TypeTracer {
    pub snapshots: Vec<TypeSnapshot>,
    sequence: u64,
    enabled: bool,
}

impl TypeTracer {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
    pub fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    pub fn record(&mut self, ty: &IRType, description: &str) {
        if !self.enabled {
            return;
        }
        self.sequence += 1;
        self.snapshots.push(TypeSnapshot {
            ty: ty.clone(),
            description: description.to_string(),
            sequence: self.sequence,
        });
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.sequence = 0;
    }
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn diff(&self, from_idx: usize, to_idx: usize) -> Option<TypeDiff> {
        let from = self.snapshots.get(from_idx)?;
        let to = self.snapshots.get(to_idx)?;
        Some(TypeDiff {
            from: from.clone(),
            to: to.clone(),
            widened: to.ty.contains(&from.ty) && !from.ty.contains(&to.ty),
            narrowed: from.ty.contains(&to.ty) && !to.ty.contains(&from.ty),
            unchanged: from.ty == to.ty,
        })
    }

    pub fn format_trace(&self) -> String {
        let mut result = String::from("=== Type Trace ===\n");
        for snap in &self.snapshots {
            result.push_str(&format!(
                "  [{:>4}] {}: {}\n",
                snap.sequence, snap.description, snap.ty
            ));
        }
        result
    }
}

/// Represents the difference between two type snapshots.
#[derive(Debug, Clone)]
pub struct TypeDiff {
    pub from: TypeSnapshot,
    pub to: TypeSnapshot,
    pub widened: bool,
    pub narrowed: bool,
    pub unchanged: bool,
}

impl fmt::Display for TypeDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.unchanged {
            write!(f, "  [UNCHANGED] {}", self.from.ty)
        } else if self.widened {
            write!(f, "  [WIDENED]   {} → {}", self.from.ty, self.to.ty)
        } else if self.narrowed {
            write!(f, "  [NARROWED]  {} → {}", self.from.ty, self.to.ty)
        } else {
            write!(f, "  [CHANGED]   {} → {}", self.from.ty, self.to.ty)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Complexity Analysis
// ═══════════════════════════════════════════════════════════════════════════

/// Complexity metrics for a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComplexityInfo {
    pub depth: u32,
    pub width: u32,
    pub constructors: u32,
    pub has_compound: bool,
    pub can_simplify: bool,
}

impl fmt::Display for ComplexityInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Complexity {{ depth: {}, width: {}, constructors: {}, compound: {}, simplifiable: {} }}",
            self.depth, self.width, self.constructors, self.has_compound, self.can_simplify
        )
    }
}

/// Analyzes type complexity.
pub fn analyze_complexity(ty: &IRType) -> ComplexityInfo {
    let mut info = ComplexityInfo {
        depth: 0,
        width: 0,
        constructors: 0,
        has_compound: false,
        can_simplify: false,
    };
    walk_complexity(&ty.kind, 1, &mut info);
    let normalized = ty.normalize();
    info.can_simplify = normalized != *ty;
    info
}

fn walk_complexity(kind: &TypeKind, depth: u32, info: &mut ComplexityInfo) {
    if depth > info.depth {
        info.depth = depth;
    }
    info.constructors += 1;

    match kind {
        TypeKind::Union(a, b) | TypeKind::Intersection(a, b) | TypeKind::Difference(a, b) => {
            info.has_compound = true;
            walk_complexity(&a.kind, depth + 1, info);
            walk_complexity(&b.kind, depth + 1, info);
        }
        TypeKind::Message { payload, .. } => {
            walk_complexity(&payload.kind, depth + 1, info);
        }
        TypeKind::StableTuple { element_types, .. } => {
            for elem in element_types {
                walk_complexity(&elem.kind, depth + 1, info);
            }
        }
        TypeKind::MapShape { values, .. } => {
            for v in values {
                walk_complexity(&v.kind, depth + 1, info);
            }
        }
        TypeKind::Actor { accepts, .. } => {
            for a in accepts {
                walk_complexity(&a.kind, depth + 1, info);
            }
        }
        TypeKind::RecursiveVar { bound, .. } => {
            if let Some(b) = bound {
                walk_complexity(&b.kind, depth + 1, info);
            }
        }
        TypeKind::Speculative {
            assumed, actual, ..
        } => {
            walk_complexity(&assumed.kind, depth + 1, info);
            walk_complexity(&actual.kind, depth + 1, info);
        }
        TypeKind::Tuple { .. }
        | TypeKind::Fun { .. }
        | TypeKind::Cons
        | TypeKind::List
        | TypeKind::Nil
        | TypeKind::Atom
        | TypeKind::Boolean
        | TypeKind::SmallInt
        | TypeKind::NonNegInt
        | TypeKind::Int64
        | TypeKind::Float
        | TypeKind::Map
        | TypeKind::Binary
        | TypeKind::Pid
        | TypeKind::Port
        | TypeKind::Reference
        | TypeKind::Capability { .. }
        | TypeKind::Any
        | TypeKind::Bottom
        | TypeKind::Dynamic
        | TypeKind::Constant(_)
        | TypeKind::Tensor { .. } => {
            info.width += 1;
        }
    }
}

/// Estimate the "size" of a type (rough measure of how many values it could represent).
pub fn estimated_type_size(ty: &IRType) -> u32 {
    match &ty.kind {
        TypeKind::Bottom => 0,
        TypeKind::Nil | TypeKind::Constant(_) => 1,
        TypeKind::Boolean => 2,
        TypeKind::SmallInt | TypeKind::NonNegInt | TypeKind::Int64 | TypeKind::Float => 64,
        TypeKind::Atom => 32,
        TypeKind::Cons | TypeKind::List => 128,
        TypeKind::Tuple { arity } => 64 * (*arity).max(1),
        TypeKind::StableTuple { element_types, .. } => element_types
            .iter()
            .map(|e| estimated_type_size(e))
            .sum::<u32>()
            .max(1),
        TypeKind::Map => 256,
        TypeKind::MapShape { values, .. } => values
            .iter()
            .map(|v| estimated_type_size(v))
            .sum::<u32>()
            .max(1),
        TypeKind::Union(a, b) => estimated_type_size(a).saturating_add(estimated_type_size(b)),
        TypeKind::Intersection(a, b) => estimated_type_size(a).min(estimated_type_size(b)),
        TypeKind::Difference(a, b) => estimated_type_size(a).saturating_sub(estimated_type_size(b)),
        TypeKind::Any | TypeKind::Dynamic => 512,
        TypeKind::Message { payload, .. } => estimated_type_size(payload),
        TypeKind::Actor { accepts, .. } => accepts
            .iter()
            .map(|a| estimated_type_size(a))
            .sum::<u32>()
            .max(1),
        TypeKind::Tensor { shape, .. } => shape.iter().filter(|d| d.is_some()).count() as u32 * 32,
        TypeKind::Capability { .. } => 8,
        TypeKind::Pid | TypeKind::Port | TypeKind::Reference => 16,
        TypeKind::Fun { .. } => 64,
        TypeKind::Binary => 64,
        TypeKind::RecursiveVar { bound, .. } => {
            bound.as_ref().map_or(256, |b| estimated_type_size(b))
        }
        TypeKind::Speculative { actual, .. } => estimated_type_size(actual),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Benchmark result for a type operation.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub operation: String,
    pub iterations: u64,
    pub total_time: Duration,
    pub avg_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
    pub p50_time: Duration,
    pub p95_time: Duration,
    pub p99_time: Duration,
}

impl fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Benchmark: {} ===", self.operation)?;
        writeln!(f, "  Iterations: {}", self.iterations)?;
        writeln!(f, "  Total:      {:?}", self.total_time)?;
        writeln!(f, "  Avg:        {:?}", self.avg_time)?;
        writeln!(f, "  Min:        {:?}", self.min_time)?;
        writeln!(f, "  Max:        {:?}", self.max_time)?;
        writeln!(f, "  P50:        {:?}", self.p50_time)?;
        writeln!(f, "  P95:        {:?}", self.p95_time)?;
        write!(f, "  P99:        {:?}", self.p99_time)
    }
}

/// Benchmark the join operation between two types.
pub fn benchmark_join(a: &IRType, b: &IRType, iterations: u64) -> BenchmarkResult {
    benchmark_op("join", iterations, || {
        let _ = a.join(b);
    })
}

/// Benchmark the meet operation between two types.
pub fn benchmark_meet(a: &IRType, b: &IRType, iterations: u64) -> BenchmarkResult {
    benchmark_op("meet", iterations, || {
        let _ = a.meet(b);
    })
}

/// Benchmark the contains operation between two types.
pub fn benchmark_contains(a: &IRType, b: &IRType, iterations: u64) -> BenchmarkResult {
    benchmark_op("contains", iterations, || {
        let _ = a.contains(b);
    })
}

/// Benchmark the normalize operation on a type.
pub fn benchmark_normalize(a: &IRType, iterations: u64) -> BenchmarkResult {
    benchmark_op("normalize", iterations, || {
        let _ = a.normalize();
    })
}

fn benchmark_op<F: Fn()>(name: &str, iterations: u64, f: F) -> BenchmarkResult {
    // Warmup
    for _ in 0..100.min(iterations) {
        let _ = f();
    }

    let mut samples = Vec::with_capacity(iterations as usize);
    let total_start = Instant::now();

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = f();
        samples.push(start.elapsed());
    }

    let total_time = total_start.elapsed();
    samples.sort();

    let len = samples.len();
    BenchmarkResult {
        operation: name.to_string(),
        iterations,
        total_time,
        avg_time: total_time / iterations as u32,
        min_time: samples[0],
        max_time: samples[len - 1],
        p50_time: samples[len / 2],
        p95_time: {
            let i = ((len as f64) * 0.95) as usize;
            samples[i.min(len - 1)]
        },
        p99_time: {
            let i = ((len as f64) * 0.99) as usize;
            samples[i.min(len - 1)]
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_basic() {
        let mut profiler = TypeProfiler::new();
        profiler.record_join(Duration::from_nanos(100));
        profiler.record_join(Duration::from_nanos(200));
        profiler.record_meet(Duration::from_nanos(50));
        profiler.record_contains(Duration::from_nanos(75));

        assert_eq!(profiler.join_count, 2);
        assert_eq!(profiler.meet_count, 1);
        assert_eq!(profiler.contains_count, 1);
        assert_eq!(profiler.total_ops(), 4);
    }

    #[test]
    fn test_profiler_reset() {
        let mut profiler = TypeProfiler::new();
        profiler.record_join(Duration::from_nanos(100));
        profiler.reset();
        assert_eq!(profiler.total_ops(), 0);
    }

    #[test]
    fn test_tracer_basic() {
        let mut tracer = TypeTracer::new();
        let a = IRType::new(TypeKind::SmallInt);
        let b = IRType::new(TypeKind::Float);

        tracer.record(&a, "initial type");
        let joined = a.join(&b);
        tracer.record(&joined, "after join");

        assert_eq!(tracer.len(), 2);

        let diff = tracer.diff(0, 1).unwrap();
        assert!(diff.widened || diff.unchanged);
    }

    #[test]
    fn test_tracer_disabled() {
        let mut tracer = TypeTracer::with_enabled(false);
        tracer.record(&IRType::new(TypeKind::SmallInt), "test");
        assert!(tracer.is_empty());
    }

    #[test]
    fn test_complexity_simple() {
        let ty = IRType::new(TypeKind::SmallInt);
        let info = analyze_complexity(&ty);
        assert_eq!(info.depth, 1);
        assert!(!info.has_compound);
        assert!(!info.can_simplify);
    }

    #[test]
    fn test_complexity_compound() {
        let ty = IRType::new(TypeKind::Union(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::Float)),
        ));
        let info = analyze_complexity(&ty);
        assert!(info.depth >= 2);
        assert!(info.has_compound);
    }

    #[test]
    fn test_complexity_nested() {
        let inner = IRType::new(TypeKind::Intersection(
            Box::new(IRType::new(TypeKind::SmallInt)),
            Box::new(IRType::new(TypeKind::NonNegInt)),
        ));
        let outer = IRType::new(TypeKind::Union(
            Box::new(inner),
            Box::new(IRType::new(TypeKind::Atom)),
        ));
        let info = analyze_complexity(&outer);
        assert!(info.depth >= 3);
        assert!(info.has_compound);
    }

    #[test]
    fn test_estimated_size() {
        assert_eq!(estimated_type_size(&IRType::new(TypeKind::Bottom)), 0);
        assert_eq!(estimated_type_size(&IRType::new(TypeKind::Nil)), 1);
        assert!(estimated_type_size(&IRType::new(TypeKind::Any)) > 0);
    }

    #[test]
    fn test_benchmark_runs() {
        let a = IRType::new(TypeKind::SmallInt);
        let b = IRType::new(TypeKind::Float);
        let result = benchmark_join(&a, &b, 100);
        assert_eq!(result.iterations, 100);
        assert_eq!(result.operation, "join");
    }
}
