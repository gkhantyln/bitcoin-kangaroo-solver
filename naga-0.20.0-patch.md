# naga 0.20.0 SPIR-V Backend Patch

## Problem

When compiling WGSL compute shaders to SPIR-V for the Vulkan backend, naga 0.20.0's SPIR-V writer crashes with:

```
Expression [113] is not cached!
```

## Root Cause

The `Statement::Call` handler in `src/back/spv/block.rs` unconditionally indexes `self.cached[argument]`, assuming all function call arguments are pre-cached. The `CachedExpressions::Index` trait implementation calls `unreachable!()` when `*id == 0`, so any uncached expression causes a crash.

The pre-pass in `writer.rs` only caches expressions that `needs_pre_emit()` or are `is_const`. Function call arguments (typically `Load`, `Access`, `AccessIndex`, etc.) may not qualify, leaving them uncached.

Additionally, `cache_expression_value()` returns 0 for intermediate `Access`/`AccessIndex` expressions whose base is also intermediate — it assigns 0 to `self.cached[expr_handle]`, making the expression permanently uncached.

## Patch (applied locally to vendored naga in `.cargo/registry`)

Two files are patched:

### `src/back/spv/mod.rs` — Add `is_cached` safe-check method

```rust
impl CachedExpressions {
    fn is_cached(&self, h: Handle<crate::Expression>) -> bool {
        self.ids[h.index()] != 0
    }
}
```

### `src/back/spv/block.rs` — On-demand pointer-based caching in `Statement::Call`

```rust
// BEFORE (line 2244):
for &argument in arguments {
    self.temp_list.push(self.cached[argument]);
}

// AFTER:
for &argument in arguments {
    if !self.cached.is_cached(argument) {
        self.cache_expression_value(argument, &mut block)?;
    }
    if self.cached.is_cached(argument) {
        self.temp_list.push(self.cached[argument]);
    } else {
        // write_expression_pointer works where cache_expression_value returns 0
        let pointer_id = match self.write_expression_pointer(
            argument, &mut block, None,
        )? {
            ExpressionPointer::Ready { pointer_id } => pointer_id,
            ExpressionPointer::Conditional { .. } => {
                return Err(Error::FeatureNotImplemented(
                    "conditional expression pointer in function call",
                ));
            }
        };
        self.cached[argument] = pointer_id;
        self.temp_list.push(pointer_id);
    }
}
```

## Why `write_expression_pointer` Instead of Just Fixing `cache_expression_value`

`cache_expression_value` returns 0 for `Access`/`AccessIndex` where `is_intermediate(base)` is true. This is by design — these expressions compute pointers, not values. The correct fix is to use `write_expression_pointer` for such cases, which is the same pattern already used by `Statement::Store` (line 2182).

## Status

**✅ Applied and verified.** The patch is applied locally to the vendored naga dependency in `.cargo/registry`. It survives `cargo build --release --features gpu-wgpu` and the GPU solver initializes and compiles the WGSL shader to SPIR-V without crashing.

## Warning

`cargo update` may overwrite these patches. If the naga dependency is updated, these changes must be re-applied.

A proper upstream fix should be submitted to the naga project to handle uncached expressions in function call arguments gracefully.
