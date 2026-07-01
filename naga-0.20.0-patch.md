# naga 0.20.0 SPIR-V Backend Patch

## Problem

When compiling WGSL compute shaders to SPIR-V for the Vulkan backend, naga 0.20.0's SPIR-V writer crashes with:

```
Expression [113] is not cached!
```

## Root Cause

The `Statement::Call` handler in `src/back/spv/block.rs` assumes all function call arguments are pre-cached by the pre-pass in `writer.rs`. However, the pre-pass only caches expressions that `needs_pre_emit()` or are `is_const`. Function call arguments (typically `Load`, `AccessIndex`, etc.) may not qualify, leaving them uncached.

## Patch (applied locally)

### `src/back/spv/mod.rs` — Add `is_cached` method

```rust
impl CachedExpressions {
    fn is_cached(&self, h: Handle<crate::Expression>) -> bool {
        self.ids[h.index()] != 0
    }
}
```

### `src/back/spv/block.rs` — On-demand caching in Call handler

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
    self.temp_list.push(self.cached[argument]);
}
```

## Status

This is a local patch to the vendored naga dependency in the cargo registry. A full upstream fix should be submitted to the naga project.
