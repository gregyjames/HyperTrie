#![doc = "stumpalo"]
#![cfg_attr(not(feature = "std"), no_std)]

// INVARIANT: `layout.size()` is a multiple of `layout.align()`, in
// all layouts passed to the internal allocation routines.
//
// Layouts are either constructed from `Layout::new::<T>()` or
// `Layout::array::<T>(len)`, which uphold this invariant, or are
// caller-provided layouts normalized with `Layout::pad_to_align()`.
//
// The Rust language guarantees `size_of::<T>() % align_of::<T>() == 0`
// for all types (<https://doc.rust-lang.org/reference/type-layout.html>).

#[doc(hidden)]
extern crate alloc as core_alloc;

mod limits;

use core::{
    alloc::Layout,
    array,
    cell::Cell,
    marker::PhantomData,
    mem,
    ptr::{self, NonNull},
};

#[cold]
#[inline(never)]
fn cold_path() {}

trait NonNullExt<T: ?Sized> {
    unsafe fn as_ref_unchecked<'a>(&self) -> &'a T;
    unsafe fn as_mut_unchecked<'a>(&self) -> &'a mut T;
}

impl<T: ?Sized> NonNullExt<T> for NonNull<T> {
    #[inline]
    unsafe fn as_ref_unchecked<'a>(&self) -> &'a T {
        unsafe { self.as_ref() }
    }

    #[inline]
    unsafe fn as_mut_unchecked<'a>(&self) -> &'a mut T {
        unsafe { &mut *(self.as_ptr()) }
    }
}

trait RawPtrExt<T: ?Sized> {
    unsafe fn as_ref_unchecked<'a>(self) -> &'a T;
    unsafe fn as_mut_unchecked<'a>(self) -> &'a mut T;
}

impl<T: ?Sized> RawPtrExt<T> for *mut T {
    #[inline]
    unsafe fn as_ref_unchecked<'a>(self) -> &'a T {
        unsafe { &*self }
    }

    #[inline]
    unsafe fn as_mut_unchecked<'a>(self) -> &'a mut T {
        unsafe { &mut *self }
    }
}

impl<T: ?Sized> RawPtrExt<T> for *const T {
    #[inline]
    unsafe fn as_ref_unchecked<'a>(self) -> &'a T {
        unsafe { &*self }
    }

    #[inline]
    unsafe fn as_mut_unchecked<'a>(self) -> &'a mut T {
        panic!("cannot cast *const T to &mut T");
    }
}
use core_alloc::alloc::{alloc, dealloc};

use crate::limits::SAFE_SIZE;

const HEADER_SIZE: usize = mem::size_of::<ChunkHeader>();
pub const INITIAL_CHUNK_CAPACITY: usize = 256 - HEADER_SIZE;
const CHUNK_ALIGN: usize = 16;

// The absolute minimum address that the header can live at
// is at 0x10, as 0x00 is guaranteed to not be valid, and
// we allocate chunks that are 16-byte aligned.
// This means that the absolute minimum address that `top`
// can start at is 0x20.
const MIN_TOP: usize = HEADER_SIZE * 2;

struct ChunkHeader {
    // When this chunk is in use, this points to the previously
    // used chunk.
    // When this chunk is free, this points to the next free chunk.
    next: *mut ChunkHeader,
    capacity: usize,
}

/// A bump allocator whose fast path compiles to as few as six instructions.
///
/// Create one with [`Arena::new()`], then allocate via [`alloc`](Arena::alloc),
/// [`alloc_slice_copy`](Arena::alloc_slice_copy), etc.  Chunks are allocated lazily
/// and freed on drop.  All allocation methods take `&self` (interior mutability
/// via [`Cell`]).
///
// # Layout compatibility with `ArenaRef`
//
// `Arena` and `ArenaRef` have the same `repr(C)` field layout so that
// [`as_arena_ref`](Arena::as_arena_ref) can reinterpret `&Arena` as
// `&ArenaRef<'_>` without undefined behaviour.
#[repr(C)]
pub struct Arena {
    top: Cell<*mut u8>,
    bottom: Cell<*mut u8>,
    fresh_chunks: Cell<*mut ChunkHeader>,
    // Padding: matches the size of ArenaRef's `arena_ptr` field so that
    // `as_arena_ref` can cast &Arena -> &ArenaRef without reading past the
    // allocation. The reinterpreted `arena_ptr` is never dereferenced.
    // It would aparently be undefined behaviour to have this, even though
    // the field is never read (according to miri, anyway).
    _padding: *const (),
}

// Arena owns all of its state.
// Calls to the global alloc/dealloc are fine to call from any thread.
unsafe impl Send for Arena {}

impl Arena {
    /// Create an empty arena.  No chunk is allocated until the first `alloc`.
    pub fn new() -> Self {
        let header = (&FIRST_HEADER) as *const ChunkHeader as *mut ChunkHeader;
        let bottom = unsafe { header.add(1) as *mut u8 };
        Self {
            top: Cell::new(bottom),
            bottom: Cell::new(bottom),
            fresh_chunks: Cell::new(ptr::null_mut()),
            _padding: ptr::null(),
        }
    }

    /// Create a new arena with the default chunk size.
    /// and allocate a chunk of that size.
    /// See [with_chunk](Arena::with_chunk) for a reason why you might want to
    /// do this.
    pub fn with_default_chunk() -> Self {
        Self::with_chunk(INITIAL_CHUNK_CAPACITY)
    }

    /// Create an arena, allocating the first chunk upfront.
    /// This can avoid reinforcing the wrong conditional branch
    /// on the first call to `alloc`.
    pub fn with_chunk(default_chunk_size: usize) -> Self {
        let header = ArenaRef::chunk_new(default_chunk_size, ptr::null_mut());
        let bottom = unsafe { header.add(1) as *mut u8 };
        let top = unsafe { bottom.byte_add((*header).capacity) as *mut u8 };
        Self {
            top: Cell::new(top),
            bottom: Cell::new(bottom),
            fresh_chunks: Cell::new(ptr::null_mut()),
            _padding: ptr::null(),
        }
    }

    // --- Allocation forwarding methods ---
    // These forward to ArenaRef by reinterpreting &self as &ArenaRef via
    // transmute. Arena and ArenaRef share the same repr(C) layout for their
    // first three fields (top, bottom, fresh_chunks), so treating an Arena
    // as an ArenaRef is sound.  The arena pointer field in the reinterpreted
    // ArenaRef is never read during allocation (only in ArenaRef::Drop,
    // which does not run on references), so dangling bytes there are harmless.

    #[inline(always)]
    pub fn as_arena_ref(&self) -> &ArenaRef<'_> {
        // SAFETY: Arena and ArenaRef have identical #[repr(C)] layouts for
        // the first three fields (top, bottom, fresh_chunks). The arena
        // field is only accessed in ArenaRef::Drop, which does not run on
        // the returned reference.
        unsafe { &*(self as *const Arena as *const ArenaRef<'_>) }
    }

    #[inline(always)]
    pub fn as_arena_ref_mut(&mut self) -> &mut ArenaRef<'_> {
        // SAFETY: Arena and ArenaRef have identical #[repr(C)] layouts for
        // the first three fields (top, bottom, fresh_chunks). The arena
        // field is only accessed in ArenaRef::Drop, which does not run on
        // the returned reference.
        unsafe { &mut *(self as *mut Arena as *mut ArenaRef<'_>) }
    }

    /// Allocate a `T` into the arena. Returns a mutable reference to the copy.
    /// Note that `T`'s destructor will *not* be called on the copy.
    #[inline]
    pub fn alloc<T>(&self, data: T) -> &mut T {
        self.as_arena_ref().alloc(data)
    }

    /// This is a version of [alloc](Arena::alloc), marked as `#[inline(never)]`.
    #[inline(never)]
    pub fn alloc_no_inline<T>(&self, data: T) -> &mut T {
        self.alloc(data)
    }

    /// Allocate a `T` into the arena, as constructed by the closure.
    /// Returns a mutable reference to the allocated value.
    #[inline]
    pub fn alloc_with<F, T>(&self, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        self.as_arena_ref().alloc_with(f)
    }

    /// Allocate space for an object with the given [`Layout`].
    ///
    /// The returned pointer points at uninitialized memory.
    ///
    /// # Panics
    ///
    /// Panics if reserving space matching `layout` fails.
    #[inline]
    pub fn alloc_layout(&self, layout: Layout) -> NonNull<[u8]> {
        self.as_arena_ref().alloc_layout(layout)
    }

    /// Copy a slice into the arena. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_slice_copy<T: Copy>(&self, data: &[T]) -> &mut [T] {
        self.as_arena_ref().alloc_slice_copy(data)
    }

    /// Copy a slice literal into the arena. Returns a mutable reference to the copy.
    /// This is faster than [alloc_slice_copy](Arena::alloc_slice_copy), as it assumes the size is
    /// available at compile-time.
    /// If you use this on a non-literal slice, (eg. one produced by [leak](Box::leak)),
    /// then this will have slightly worse performance than [alloc_slice_copy](Arena::alloc_slice_copy).
    #[inline]
    pub fn alloc_slice_lit_copy<T: Copy>(&self, data: &'static [T]) -> &mut [T] {
        self.as_arena_ref().alloc_slice_lit_copy(data)
    }

    /// Copy a slice into the arena via cloning. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_slice_clone<T: Clone>(&self, data: &[T]) -> &mut [T] {
        self.as_arena_ref().alloc_slice_clone(data)
    }

    /// Allocate an array of known size, populated by a function.
    #[inline]
    pub fn alloc_sized_slice_with<T, const N: usize, F>(&self, f: F) -> &mut [T; N]
    where
        F: FnOnce() -> [T; N],
    {
        self.as_arena_ref().alloc_sized_slice_with(f)
    }

    /// Allocate an array of known size, populated element-by-element.
    #[inline]
    pub fn alloc_sized_slice_fill_with<T, const N: usize, F>(&self, f: F) -> &mut [T; N]
    where
        F: FnMut(usize) -> T,
    {
        self.as_arena_ref().alloc_sized_slice_fill_with(f)
    }

    /// Allocate a slice, populating all slots uniformly by a function.
    #[inline]
    pub fn alloc_slice_fill_with<T, F>(&self, len: usize, f: F) -> &mut [T]
    where
        F: FnMut(usize) -> T,
    {
        self.as_arena_ref().alloc_slice_fill_with(len, f)
    }

    /// Allocate a slice, populating all slots uniformly by a fallible function.
    #[inline]
    pub fn alloc_slice_try_fill_with<T, E, F>(&self, len: usize, f: F) -> Result<&mut [T], E>
    where
        F: FnMut(usize) -> Result<T, E>,
    {
        self.as_arena_ref().alloc_slice_try_fill_with(len, f)
    }

    /// Copy a fixed-size array into the arena. Returns a mutable reference.
    #[inline]
    pub fn alloc_sized_slice_copy<T: Copy, const N: usize>(&self, data: &[T; N]) -> &mut [T; N] {
        self.as_arena_ref().alloc_sized_slice_copy(data)
    }

    /// Copy a fixed-size array into the arena via cloning.
    #[inline]
    pub fn alloc_sized_slice_clone<T: Clone, const N: usize>(&self, data: &[T; N]) -> &mut [T; N] {
        self.as_arena_ref().alloc_sized_slice_clone(data)
    }

    /// Allocate a fixed-size array filled with [Default] values.
    #[inline]
    pub fn alloc_sized_slice_fill_default<T: Default, const N: usize>(&self) -> &mut [T; N] {
        self.as_arena_ref().alloc_sized_slice_fill_default()
    }

    /// Allocate a slice filled with [Default] values.
    #[inline]
    pub fn alloc_slice_fill_default<T: Default>(&self, len: usize) -> &mut [T] {
        self.as_arena_ref().alloc_slice_fill_default(len)
    }

    /// Allocate a slice from an iterator with known size.
    /// Returns a mutable reference to the allocated slice.
    /// Panics if the iterator yields less elements than its `.len()`.
    #[inline]
    pub fn alloc_slice_fill_iter<T, I>(&self, iter: I) -> &mut [T]
    where
        I: IntoIterator<Item = T>,
        <I as IntoIterator>::IntoIter: ExactSizeIterator,
    {
        self.as_arena_ref().alloc_slice_fill_iter(iter)
    }

    /// Copy a string into the arena. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_str(&self, s: &str) -> &mut str {
        self.as_arena_ref().alloc_str(s)
    }

    /// Copy a string literal into the arena. Returns a mutable reference to the copy.
    /// This is faster than [alloc_str](Arena::alloc_str), as it assumes the size is
    /// available at compile-time.
    /// If you use this on a non-literal string, (eg. one produced by [leak](String::leak)),
    /// then this will have slightly worse performance than [alloc_str](Arena::alloc_str).
    #[inline]
    pub fn alloc_str_lit(&self, s: &'static str) -> &mut str {
        self.as_arena_ref().alloc_str_lit(s)
    }

    /// Counts the size of all chunks in this arena, whether those chunks are
    /// in use or not. Doesn't include chunk metadata.
    pub fn allocated_bytes(&self) -> usize {
        self.as_arena_ref().allocated_bytes()
    }

    /// Counts the size of all chunks in this arena, whether those chunks are
    /// in use or not. Includes chunk metadata.
    pub fn allocated_bytes_including_metadata(&self) -> usize {
        self.as_arena_ref().allocated_bytes_including_metadata()
    }

    /// Returns the capacity of the current chunk.
    pub fn chunk_capacity(&self) -> usize {
        self.as_arena_ref().chunk_capacity()
    }

    /// Returns whether the arena has any fresh chunks allocated.
    pub fn has_fresh_chunks(&self) -> bool {
        self.as_arena_ref().has_fresh_chunks()
    }

    fn header(&self) -> *mut ChunkHeader {
        let ptr = self.bottom.get() as *mut ChunkHeader;
        unsafe { ptr.sub(1) }
    }

    /// Clear the arena, invalidating all references to arena-allocated data
    /// at compile time. Chunks that have already been allocated are reused
    /// in subsequent allocations.
    /// If you want to actually free those chunks, just recreate the arena.
    pub fn clear(&mut self) {
        let current = self.header();
        let capacity = unsafe { (*current).capacity };
        // Guard against the FIRST_HEADER sentinel (capacity 0), which
        // lives in read-only static memory and must not be mutated.
        if capacity == 0 {
            return;
        }

        // Reset top to end of current chunk
        self.top
            .set(unsafe { (current.add(1) as *mut u8).byte_add(capacity) });
        // Move used chain to fresh_chunks
        let used_chain = unsafe { (*current).next };
        unsafe { (*current).next = ptr::null_mut() };
        self.fresh_chunks.set(ArenaRef::join_chunk_chains(
            self.fresh_chunks.get(),
            used_chain,
        ));
    }

    /// Free all chunks in the 'fresh chunk' list.
    /// Fresh chunks only occur if you're using scoped arenas (see [with_scope](Self::with_scope)),
    /// or if you've called [clear](Self::clear).
    pub fn free_fresh_chunks(&mut self) {
        unsafe { ArenaRef::free_chunk_chain(self.fresh_chunks.get()) };
    }

    /// Alias for [clear](Arena::clear).
    pub fn reset(&mut self) {
        self.clear()
    }

    /// Run a closure with a sub-scoped arena.  References returned by the
    /// closure are bounded by the closure's scope (not the parent arena's
    /// lifetime), and the arena state is reverted on return (chunks are
    /// reused, not freed).
    ///
    /// Takes `&mut self`, making references from `alloc` are invalidated.
    /// This is intentional. If you need to use outer allocations after
    /// a scope closes, use an [ArenaRef], by calling
    /// [as_arena_ref_mut](Arena::as_arena_ref_mut).
    pub fn with_scope<'brand, F, R>(&'brand mut self, f: F) -> R
    where
        F: for<'scope> FnOnce(&'scope mut ArenaRef<'scope>) -> R,
    {
        unsafe { self.as_arena_ref_mut().with_scope_unchecked(f) }
    }
}

/// An arena allocator reference, tied to the lifetime of its parent
/// [`Arena`] or [`ArenaRef`].
/// The only time you should need to use this is when using [with_scope](Arena::with_scope).
/// It provides mostly the same API as `Arena`.
#[repr(C)]
pub struct ArenaRef<'a> {
    // pointer to the first unfree byte
    top: Cell<*mut u8>,
    // pointer to the current chunk (after the header)
    bottom: Cell<*mut u8>,
    // pointer to the next reusable chunk
    fresh_chunks: Cell<*mut ChunkHeader>,
    // Raw pointer to the parent Arena.  Only dereferenced in Drop (and only
    // by real ArenaRef values, not by the reinterpreted view from
    // Arena::as_arena_ref).
    arena_ptr: *const Arena,
    // Lifetime marker for the borrow-checker.
    _marker: PhantomData<&'a ()>,
}

const FIRST_HEADER: ChunkHeader = ChunkHeader {
    next: ptr::null::<ChunkHeader>() as *mut ChunkHeader,
    capacity: 0,
};

// --- Public API ---

// Note that there is no clear() method, as that would not invalidate references allocated through
// the arena...
impl<'a> ArenaRef<'a> {
    // For T: u32, on x86_64, the entire fast path compiles down to:
    // ```asm
    // alloc_u32:
    //   mov     rcx, qword ptr [rdi]      # rcx = self.top (current bump pointer)
    //   and     rcx, -4                   # Align top down to 4-byte boundary (top & !(align-1))
    //   lea     rax, [rcx - 4]            # rax = aligned_top - sizeof(u32); allocated address
    //   cmp     rax, qword ptr [rdi + 8]  # if new_top < self.bottom.get() (out of room):
    //   jb      ArenaRef::alloc_more      #    take the slow path
    //   mov     qword ptr [rdi], rax      # self.top = new_top
    //   mov     dword ptr [rcx - 4], esi  # write the u32 to allocated slot
    //   ret                               # return the &mut u32 in rax
    // ```
    //
    // To get this output on compiler explorer, mark this function
    // as #[inline(never)], and compile with `-O -C relocation-model=static`.
    //
    // When there are multiple allocations, the fast path can go as low as six instructions
    // per allocation.
    //
    /// Copy a `T` into the arena. Returns a mutable reference to the copy.
    /// Note that `T`'s destructor will *not* be called on the copy.
    #[inline]
    pub fn alloc<T>(&self, data: T) -> &'a mut T {
        self.alloc_with(|| data)
    }

    /// This is a version of [alloc](ArenaRef::alloc), marked as `#[inline(never)]`.
    /// This can be useful if you want to optimize for code size, as it
    /// doesn't emit a new conditional branch, it may even be faster when you have
    /// lots of different allocations in your codepath, as it'll free up branch
    /// predictor cache.
    /// You should default to `alloc`.
    #[inline(never)]
    pub fn alloc_no_inline<T>(&self, data: T) -> &'a mut T {
        self.alloc(data)
    }

    /// Allocate a `T` into the arena, constructed by the closure.
    /// Returns a mutable reference to the allocated value.
    #[inline]
    pub fn alloc_with<F, T>(&self, f: F) -> &'a mut T
    where
        F: FnOnce() -> T,
    {
        let layout = Layout::new::<T>();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, true);

        // We can potentially create a jbe instruction directly to the
        // slow path, but to do so when this is inlined, we need to
        // put the parameter in a register, or on the stack.
        // This is slow for larger sizes, so we cap this behaviour at 16 bytes.
        let ptr: *mut T = if layout.size() <= 16 {
            if !take_slow_path {
                unsafe {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    let ptr: *mut T = new_top as *mut T;
                    ptr::write(ptr, f());
                    self.top.set(new_top);
                    ptr
                }
            } else {
                cold_path();
                self.alloc_slow_with(f)
            }
        } else {
            unsafe {
                let ptr: *mut T = if !take_slow_path {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    self.top.set(new_top);
                    new_top as *mut T
                } else {
                    cold_path();
                    self.alloc_slow_raw::<T>()
                };
                ptr::write(ptr, f());
                ptr
            }
        };
        unsafe { &mut *ptr }
    }

    /// Allocate space for an object with the given [`Layout`].
    ///
    /// The returned pointer points at uninitialized memory.
    ///
    /// # Panics
    ///
    /// Panics if reserving space matching `layout` fails.
    #[inline]
    pub fn alloc_layout(&self, layout: Layout) -> NonNull<[u8]> {
        let len = layout.size();
        let layout = layout.pad_to_align();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        // Pessimistically use checked arithmetic here, because we can't guarantee there's
        // any size/alignment information available at compile-time.
        // Using the standard need_slow_path function would result in extra checks, without
        // the compile-time info...
        // If you do know this information at compile-time. I'd love to hear your
        // use-case for this function, and would be happy to add an alternative.
        let take_slow_path = self.need_slow_path_checked(layout, top, bottom);

        let ptr = unsafe {
            if !take_slow_path {
                let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                self.top.set(new_top);
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                NonNull::new_unchecked(new_top)
            } else {
                cold_path();
                self.alloc_layout_slow(layout)
            }
        };

        NonNull::slice_from_raw_parts(ptr, len)
    }

    /// Copy a slice into the arena. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_slice_copy<T: Copy>(&self, data: &[T]) -> &'a mut [T] {
        let len = data.len();
        let layout = Layout::for_value(data);
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, false);

        unsafe {
            let ptr: *mut T = if !take_slow_path {
                let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                self.top.set(new_top);
                new_top as *mut T
            } else {
                cold_path();
                self.alloc_layout_slow(layout).cast().as_ptr()
            };
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, len);
            core::slice::from_raw_parts_mut(ptr, len)
        }
    }

    /// Copy a slice literal into the arena. Returns a mutable reference to the copy.
    /// This is faster than [alloc_slice_copy](ArenaRef::alloc_slice_copy), as it assumes the size is
    /// available at compile-time.
    /// If you use this on a non-literal slice, (eg. one produced by [leak](Box::leak)),
    /// then this will have slightly worse performance than [alloc_slice_copy](ArenaRef::alloc_slice_copy).
    #[inline]
    pub fn alloc_slice_lit_copy<T: Copy>(&self, data: &'static [T]) -> &'a mut [T] {
        let len = data.len();
        let layout = Layout::for_value(data);
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, true);

        unsafe {
            let ptr: *mut T = if !take_slow_path {
                let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                self.top.set(new_top);
                new_top as *mut T
            } else {
                cold_path();
                self.alloc_layout_slow(layout).cast().as_ptr()
            };
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, len);
            core::slice::from_raw_parts_mut(ptr, len)
        }
    }

    /// Clone a slice into the arena.  Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_slice_clone<T: Clone>(&self, data: &[T]) -> &'a mut [T] {
        self.alloc_slice_fill_with(data.len(), |n| data[n].clone())
    }

    /// Allocate an array of known size, populated by a function.
    #[inline]
    pub fn alloc_sized_slice_with<T, const N: usize, F>(&self, f: F) -> &'a mut [T; N]
    where
        F: FnOnce() -> [T; N],
    {
        let layout = Layout::new::<[T; N]>();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, true);

        // We can potentially create a jbe instruction directly to the
        // slow path, but to do so when this is inlined, we need to
        // put the parameter in a register, or on the stack.
        // This is slow for larger sizes, so we cap this behaviour at 16 bytes.
        let ptr: *mut [T; N] = if layout.size() <= 16 {
            unsafe {
                if !take_slow_path {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    let ptr = (new_top as *mut [T; N]).as_mut_unchecked();
                    let data = mem::ManuallyDrop::new(f());
                    core::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut T, data.len());
                    self.top.set(new_top);
                    new_top as *mut [T; N]
                } else {
                    cold_path();
                    self.alloc_slow_with::<_, [T; N]>(f)
                }
            }
        } else {
            unsafe {
                let ptr: *mut [T; N] = if !take_slow_path {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    self.top.set(new_top);
                    new_top as *mut [T; N]
                } else {
                    cold_path();
                    self.alloc_slow_raw::<[T; N]>()
                };
                let data = mem::ManuallyDrop::new(f());
                core::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut T, data.len());
                ptr
            }
        };
        unsafe { &mut *ptr }
    }

    /// Allocate an array of known size, populated element-by-element.
    #[inline]
    pub fn alloc_sized_slice_fill_with<T, const N: usize, F>(&self, mut f: F) -> &'a mut [T; N]
    where
        F: FnMut(usize) -> T,
    {
        let layout = Layout::new::<[T; N]>();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, true);

        // We can potentially create a jbe instruction directly to the
        // slow path, but to do so when this is inlined, we need to
        // put the parameter in a register, or on the stack.
        // This is slow for larger sizes, so we cap this behaviour at 16 bytes.
        let ptr: *mut [T; N] = if layout.size() <= 16 {
            unsafe {
                if !take_slow_path {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    let mut p = new_top as *mut T;
                    for i in 0..N {
                        core::ptr::write(p, f(i));
                        p = p.add(1);
                    }
                    self.top.set(new_top);
                    new_top as *mut [T; N]
                } else {
                    cold_path();
                    self.alloc_slow_with::<_, [T; N]>(|| array::from_fn(f))
                }
            }
        } else {
            unsafe {
                let ptr: *mut T = if !take_slow_path {
                    let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                    debug_assert_eq!(new_top as usize % layout.align(), 0);
                    debug_assert!(new_top as usize + layout.size() <= top as usize);
                    self.top.set(new_top);
                    new_top as *mut T
                } else {
                    cold_path();
                    self.alloc_layout_slow(layout).cast().as_ptr()
                };
                let mut p = ptr;
                for i in 0..N {
                    core::ptr::write(p, f(i));
                    p = p.add(1);
                }
                ptr as *mut [T; N]
            }
        };
        unsafe { &mut *ptr }
    }

    /// Allocate a slice, populating all slots uniformly by a function.
    #[inline]
    pub fn alloc_slice_fill_with<T, F>(&self, len: usize, mut f: F) -> &'a mut [T]
    where
        F: FnMut(usize) -> T,
    {
        let layout = Layout::array::<T>(len).unwrap();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, false);

        unsafe {
            let ptr: *mut T = if !take_slow_path {
                let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                self.top.set(new_top);
                new_top as *mut T
            } else {
                cold_path();
                self.alloc_layout_slow(layout).cast().as_ptr()
            };
            let mut p = ptr;
            for i in 0..len {
                core::ptr::write(p, f(i));
                p = p.add(1);
            }
            core::slice::from_raw_parts_mut(ptr, len)
        }
    }

    /// Allocate a slice, populating all slots uniformly by a fallible function.
    /// Returns `Err(e)` if `f` returns `Err`, with no partially-allocated state.
    #[inline]
    pub fn alloc_slice_try_fill_with<T, E, F>(&self, len: usize, mut f: F) -> Result<&'a mut [T], E>
    where
        F: FnMut(usize) -> Result<T, E>,
    {
        let layout = Layout::array::<T>(len).unwrap();
        let top = self.top.get();
        let bottom = self.bottom.get() as usize;

        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        let take_slow_path = self.need_slow_path(layout, top, bottom, false);

        unsafe {
            let ptr: *mut T = if !take_slow_path {
                let new_top = top.byte_sub(extra_bytes_needed).byte_sub(layout.size());
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                self.top.set(new_top);
                new_top as *mut T
            } else {
                cold_path();
                self.alloc_layout_slow(layout).cast().as_ptr()
            };
            let mut p = ptr;
            // Optimizes to a memcpy, when source is a contiguous array,
            // and is large, or of unknown size.
            for i in 0..len {
                match f(i) {
                    Ok(a) => {
                        core::ptr::write(p, a);
                        p = p.add(1);
                    }
                    Err(e) => {
                        cold_path();
                        if take_slow_path {
                            cold_path();
                            let header = self.header().as_ref_unchecked();
                            self.top.set(self.bottom.get().byte_add(header.capacity));
                        } else {
                            // Fast path: restore original top to undo the partial alloc.
                            self.top.set(top);
                        }
                        return Err(e);
                    }
                }
            }
            Ok(core::slice::from_raw_parts_mut(ptr, len))
        }
    }

    /// Copy a fixed-size array into the arena. Returns a mutable reference.
    #[inline]
    pub fn alloc_sized_slice_copy<T: Copy, const N: usize>(&self, data: &[T; N]) -> &'a mut [T; N] {
        self.alloc_sized_slice_with(|| *data)
    }

    /// Clone a fixed-size array into the arena. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_sized_slice_clone<T: Clone, const N: usize>(
        &self,
        data: &[T; N],
    ) -> &'a mut [T; N] {
        self.alloc_sized_slice_fill_with(|n| data[n].clone())
    }

    /// Allocate a fixed-size array filled with [`Default`] values.
    #[inline]
    pub fn alloc_sized_slice_fill_default<T: Default, const N: usize>(&self) -> &'a mut [T; N] {
        self.alloc_sized_slice_fill_with(|_| Default::default())
    }

    /// Allocate a slice filled with [`Default`] values.
    #[inline]
    pub fn alloc_slice_fill_default<T: Default>(&self, len: usize) -> &'a mut [T] {
        self.alloc_slice_fill_with(len, |_| Default::default())
    }

    /// Allocate a slice from an iterator with known size.
    /// Returns a mutable reference to the allocated slice.
    /// Panics if the iterator yields less elements than its `.len()`.
    #[inline]
    pub fn alloc_slice_fill_iter<T, I>(&self, iter: I) -> &'a mut [T]
    where
        I: IntoIterator<Item = T>,
        <I as IntoIterator>::IntoIter: ExactSizeIterator,
    {
        const ERR: &str = "Iterator didn't produce enough elements to match its `.len()`.";
        let mut iter = iter.into_iter();
        self.alloc_slice_fill_with(iter.len(), |_| iter.next().expect(ERR))
    }

    /// Copy a string into the arena. Returns a mutable reference to the copy.
    #[inline]
    pub fn alloc_str(&self, s: &str) -> &'a mut str {
        let slice = self.alloc_slice_copy(s.as_bytes());
        unsafe { core::str::from_utf8_unchecked_mut(slice) }
    }

    /// Counts the size of all chunks in this arena, whether those chunks are
    /// in use or not. Doesn't include chunk metadata.
    pub fn allocated_bytes(&self) -> usize {
        let header = unsafe { self.header().as_ref_unchecked() };
        let mut res = header.capacity;
        res += Self::count_bytes_allocated_in_chunk_seq(header.next);
        res += Self::count_bytes_allocated_in_chunk_seq(self.fresh_chunks.get());
        res
    }

    /// Counts the size of all chunks in this arena, whether those chunks are
    /// in use or not. Includes chunk metadata.
    pub fn allocated_bytes_including_metadata(&self) -> usize {
        let header = unsafe { self.header().as_ref_unchecked() };
        let mut res = header.capacity;
        res += Self::count_bytes_allocated_in_chunk_seq_with_meta(header.next);
        res += Self::count_bytes_allocated_in_chunk_seq_with_meta(self.fresh_chunks.get());
        res
    }

    /// Run a closure with a sub-scoped arena.  References returned by the
    /// closure are bounded by the closure's scope (not the parent arena's
    /// lifetime), and the arena state is reverted on return (chunks are
    /// reused, not freed).
    ///
    /// Takes `&mut self` — the borrow checker prevents using the outer
    /// `ArenaRef` directly inside the closure.
    pub fn with_scope<'brand, F, R>(&'brand mut self, f: F) -> R
    where
        F: for<'scope> FnOnce(&'scope mut ArenaRef<'scope>) -> R,
    {
        unsafe { self.with_scope_unchecked(f) }
    }

    /// Same as `with_scope` but takes `&mut self`.  Used by `Arena::with_scope`
    /// which has no lifetime parameter and thus cannot prevent holding alloc
    /// references across the call.
    unsafe fn with_scope_unchecked<'brand, F, R>(&'brand mut self, f: F) -> R
    where
        F: for<'scope> FnOnce(&'scope mut ArenaRef<'scope>) -> R,
    {
        // SAFETY: checkpoint saves Cell fields, restore rewinds — both safe
        // because Cells provide interior mutability through &self.
        let checkpoint = unsafe { self.checkpoint() };
        // Use a raw pointer to avoid tying the reborrow to 'a (ArenaRef is
        // invariant in 'a, so f(self) would borrow for 'a and prevent the
        // subsequent self.restore_checkpoint).
        //
        // Safety: 'scope is universally quantified (HRTB), so references
        // returned by the scoped arena are bounded by the closure's scope.
        let inner = self as *mut ArenaRef<'_>;
        let result = f(unsafe { &mut *inner });
        unsafe { self.restore_checkpoint(checkpoint) };
        result
    }

    /// Restores the arena to the state at which the given checkpoint was saved.
    unsafe fn restore_checkpoint(&self, checkpoint: Checkpoint) {
        let mut header = self.header();
        let mut fresh_head = self.fresh_chunks.get();

        // Walk the chunk chain from the current header back toward the
        // checkpoint.  Move each visited chunk to the fresh-chunks list
        // (they will be reused, not freed).
        //
        // The FIRST_HEADER sentinel (capacity 0) is never linked into the
        // chain, so we may hit null before finding the checkpoint header.
        // That's fine — the sentinel is the implicit root.
        while header != checkpoint.header && !header.is_null() {
            // Read/write chunk header fields through raw pointers to avoid
            // creating &mut references that would retag and attach stale
            // borrow tags to the stored pointers.
            let next_header = unsafe { (*header).next };
            unsafe { ptr::write(&raw mut (*header).next, fresh_head) };
            fresh_head = header;
            header = next_header;
        }

        self.top.set(checkpoint.top);
        let new_bottom = unsafe { checkpoint.header.add(1) as *mut u8 };
        self.bottom.set(new_bottom);
        self.fresh_chunks.set(fresh_head);
    }

    /// Returns a checkpoint with which the arena can be
    /// reverted to a previous state.
    /// Note that no chunks are actually freed, they're just put
    /// into the reuse list. To free these chunks, call
    /// [free_unused_chunks](Arena::free_unused_chunks).
    unsafe fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            top: self.top.get(),
            header: self.header(),
        }
    }

    /// Copy a string literal into the arena. Returns a mutable reference to the copy.
    /// This is faster than [alloc_str](ArenaRef::alloc_str), as it assumes the size is
    /// available at compile-time.
    /// If you use this on a non-literal string, (eg. one produced by [leak](String::leak)),
    /// then this will have slightly worse performance than [alloc_str](ArenaRef::alloc_str).
    #[inline]
    pub fn alloc_str_lit(&self, s: &'static str) -> &'a mut str {
        let slice = self.alloc_slice_lit_copy(s.as_bytes());
        unsafe { core::str::from_utf8_unchecked_mut(slice) }
    }
}

/// Opaque snapshot of arena state, produced by [`ArenaRef::checkpoint`].
struct Checkpoint {
    top: *mut u8,
    header: *mut ChunkHeader,
}

// --- Test helpers (doc-hidden) ---

impl<'a> ArenaRef<'a> {
    #[inline(never)]
    fn count_bytes_allocated_in_chunk_seq(mut head: *mut ChunkHeader) -> usize {
        let mut res = 0;
        while !head.is_null() {
            unsafe {
                let node = head.as_ref_unchecked();
                res += node.capacity;
                head = node.next;
            }
        }
        res
    }

    #[inline(never)]
    fn count_bytes_allocated_in_chunk_seq_with_meta(mut head: *mut ChunkHeader) -> usize {
        let mut res = 0;
        while !head.is_null() {
            unsafe {
                let node = head.as_ref_unchecked();
                res += node.capacity + HEADER_SIZE;
                head = node.next;
            }
        }
        res
    }

    /// Returns the number of free bytes remaining in the current chunk.
    #[doc(hidden)]
    pub fn chunk_capacity(&self) -> usize {
        self.top.get() as usize - self.bottom.get() as usize
    }

    /// Returns whether the fresh-chunks list is non-empty.
    #[doc(hidden)]
    pub fn has_fresh_chunks(&self) -> bool {
        !self.fresh_chunks.get().is_null()
    }
}

// --- Private functions ---

impl<'a> ArenaRef<'a> {
    /// This wrapper is never inlined, which keeps the amount of code generated
    /// inline as the fast path minimal.
    /// It immediately calls out to the non-generic `alloc_layout_slow`, which
    /// keeps the overall emitted code size small.
    #[inline(never)]
    fn alloc_slow_with<F, T>(&self, f: F) -> *mut T
    where
        F: FnOnce() -> T,
    {
        let p = self.alloc_slow_raw::<T>();
        unsafe { ptr::write(p, f()) };
        p
    }

    // Doesn't write to the allocated memory
    fn alloc_slow_raw<T>(&self) -> *mut T {
        let layout = Layout::new::<T>();
        self.alloc_layout_slow(layout).cast().as_ptr() as *mut T
    }

    /// Detect whether to take the slow path.
    /// The `comptime_layout` parameter is a compile-time constant at all call
    /// sites, so the compiler eliminates one side of the outer guard after
    /// inlining.
    #[inline(always)]
    fn need_slow_path(
        &self,
        layout: Layout,
        top: *mut u8,
        bottom: usize,
        comptime_layout: bool,
    ) -> bool {
        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());

        // The most padding you'll ever need is the alignment - 1.
        let max_padding = layout.align() - 1;

        // The maximum space that an allocation can take up is its size
        // plus the maximum amount of padding added to align the top.
        let max_space: usize = layout.size() + max_padding;
        let top = top as usize;

        // Both halves of this condition are compile-time constants:
        if comptime_layout && max_space < SAFE_SIZE {
            // No overflow possible.
            if max_space <= MIN_TOP {
                // Optimal path: subtract alignment + size from top, unified comparison.
                let top = top - extra_bytes_needed;
                let top = top - layout.size();
                bottom > top as usize
            } else if max_padding < MIN_TOP {
                let top = top - extra_bytes_needed;
                bottom + layout.size() > top as usize
            } else {
                bottom + layout.size() + extra_bytes_needed > top
            }
        } else if max_padding
            // Layout::size can never be greater than isize::MAX
            .checked_add(isize::MAX as usize)
            .is_some_and(|a| a < SAFE_SIZE)
        {
            // This codepath is one more instruction, but one less conditional
            // jump, and seems to be worth it.
            if max_padding < MIN_TOP {
                let top = top - extra_bytes_needed;
                bottom + layout.size() > top
            } else {
                bottom + extra_bytes_needed + layout.size() > top
            }
        } else {
            // Even when we don't have a size available at compile-time,
            // we should still have the alignment...
            if max_padding < MIN_TOP {
                let top = top - extra_bytes_needed;
                top.checked_sub(layout.size()).is_none_or(|t| t < bottom)
            } else {
                self.need_slow_path_checked(layout, top as *mut u8, bottom)
            }
        }
    }

    /// Detect whether to take the slow path.
    /// This is a slow version which always uses checked arithmetic.
    #[inline(always)]
    fn need_slow_path_checked(&self, layout: Layout, top: *mut u8, bottom: usize) -> bool {
        let extra_bytes_needed = ArenaRef::extra_bytes_needed(top, layout.align());
        (top as usize)
            .checked_sub(extra_bytes_needed)
            .and_then(|p| p.checked_sub(layout.size()))
            .is_none_or(|p| p < bottom)
    }

    // --- Private fns ---

    fn join_chunk_chains(a: *mut ChunkHeader, b: *mut ChunkHeader) -> *mut ChunkHeader {
        if a.is_null() {
            return b;
        }
        if b.is_null() {
            return a;
        }
        let mut curr = a;
        loop {
            let next = unsafe { (*curr).next };
            if next.is_null() {
                break;
            }
            curr = next;
        }
        unsafe { (*curr).next = b };
        a
    }

    #[inline(always)]
    fn extra_bytes_needed(ptr: *const u8, alignment: usize) -> usize {
        assert!(alignment.is_power_of_two());

        // ptr           = 11010
        // 8             = 01000
        // 8 - 1         = 00111
        // ptr & (8 - 1) = 00010

        // Also works if alignment > ptr

        // ptr           = 00010
        // 8             = 01000
        // 8 - 1         = 00111
        // ptr & (8 - 1) = 00010
        return ptr as usize & (alignment - 1);
    }

    fn header(&self) -> *mut ChunkHeader {
        let ptr = self.bottom.get() as *mut ChunkHeader;
        unsafe { ptr.sub(1) }
    }

    // In a fresh chunk, how many extra bytes do we need to allocate
    // as padding?
    fn extra_fresh_bytes_needed(alignment: usize) -> usize {
        let start: usize = if alignment > HEADER_SIZE {
            alignment - HEADER_SIZE
        } else {
            HEADER_SIZE
        };
        start & (alignment - 1)
    }

    // Returns a pointer to the chunk_header.
    // Panics if the requested size is too large to form a valid Layout,
    // or if the underlying allocator returns null (OOM).
    #[inline]
    fn chunk_new(size: usize, next: *mut ChunkHeader) -> *mut ChunkHeader {
        let total = size + HEADER_SIZE;
        let layout = match Layout::from_size_align(total, CHUNK_ALIGN) {
            Ok(l) => l,
            Err(_) => {
                panic!(
                    "OOM: requested chunk size {} too large (exceeds Layout limits)",
                    size,
                );
            }
        };
        let chunk: *mut ChunkHeader = unsafe { alloc(layout) } as *mut ChunkHeader;
        if chunk.is_null() {
            panic!(
                "OOM: failed to allocate chunk of {} bytes (alignment {})",
                total, CHUNK_ALIGN,
            );
        }
        let header = ChunkHeader {
            next,
            capacity: size,
        };
        unsafe {
            ptr::write(chunk, header);
        }
        chunk
    }

    // Slow path: allocate a new chunk (or reuse a fresh one) and return a
    // pointer to the reserved space.
    #[inline(never)]
    fn alloc_layout_slow(&self, layout: Layout) -> NonNull<u8> {
        let extra_fresh_bytes = Self::extra_fresh_bytes_needed(layout.align());
        let min_size = layout.size() + extra_fresh_bytes;

        // Scan the fresh-chunks list for one that's big enough for this allocation.
        // Pop and discard undersized chunks (they can't hold future allocations either,
        // so we deallocate them to avoid leaking).
        loop {
            let fresh = self.fresh_chunks.get();
            if fresh.is_null() {
                break;
            }
            let capacity = unsafe { (*fresh).capacity };
            if capacity >= min_size {
                // This fresh chunk is big enough. Reuse it.
                let next = self.header();
                unsafe {
                    self.fresh_chunks.set((*fresh).next);
                    (*fresh).next = next;
                }
                let new_bottom = unsafe { fresh.add(1) as *mut u8 };
                self.bottom.set(new_bottom);
                let top = unsafe { new_bottom.byte_add(capacity) };
                let extra = Self::extra_bytes_needed(top, layout.align());
                let new_top = unsafe { top.byte_sub(layout.size() + extra) };
                self.top.set(new_top);
                debug_assert_eq!(new_top as usize % layout.align(), 0);
                debug_assert!(new_top as usize + layout.size() <= top as usize);
                return unsafe { NonNull::new_unchecked(new_top) };
            }
            // Chunk too small — pop it and deallocate, then check the next one.
            unsafe {
                self.fresh_chunks.set((*fresh).next);
                let layout = Layout::from_size_align_unchecked(capacity + HEADER_SIZE, CHUNK_ALIGN);
                dealloc(fresh as *mut u8, layout);
            }
        }

        // No usable fresh chunk — allocate a new one.
        let next = self.header();
        // Don't link the FIRST_HEADER sentinel into the chain
        let next = if unsafe { (*next).capacity == 0 } {
            ptr::null_mut()
        } else {
            next
        };
        let size = self.next_chunk_size().max(min_size);
        let new_chunk = Self::chunk_new(size, next);
        let new_bottom = unsafe { new_chunk.add(1) as *mut u8 };
        self.bottom.set(new_bottom);
        let top = unsafe { new_bottom.byte_add(size) };
        let extra = Self::extra_bytes_needed(top, layout.align());
        let new_top = unsafe { top.byte_sub(layout.size() + extra) };
        self.top.set(new_top);
        debug_assert_eq!(new_top as usize % layout.align(), 0);
        debug_assert!(new_top as usize + layout.size() <= top as usize);
        unsafe { NonNull::new_unchecked(new_top) }
    }

    unsafe fn free_chunk_chain(mut current: *mut ChunkHeader) {
        while !current.is_null() {
            let prev = unsafe { (*current).next };
            let capacity = unsafe { (*current).capacity };
            if capacity > 0 {
                let layout = unsafe {
                    Layout::from_size_align_unchecked(capacity + HEADER_SIZE, CHUNK_ALIGN)
                };
                unsafe { dealloc(current as *mut u8, layout) };
            }
            current = prev;
        }
    }

    fn next_chunk_size(&self) -> usize {
        let header = self.header();
        let cap = unsafe { header.as_ref_unchecked().capacity };
        let res = (cap + HEADER_SIZE).saturating_mul(2) - HEADER_SIZE;
        res.max(INITIAL_CHUNK_CAPACITY)
    }
}

impl<'a> Drop for ArenaRef<'a> {
    fn drop(&mut self) {
        // Merge state down to the base Arena so forwarding methods see it.
        let arena = unsafe { &*self.arena_ptr };
        arena.top.set(self.top.get());
        arena.bottom.set(self.bottom.get());
        arena.fresh_chunks.set(self.fresh_chunks.get());
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe {
            let bottom = self.bottom.get();
            if !bottom.is_null() {
                let current = bottom.byte_sub(HEADER_SIZE) as *mut ChunkHeader;
                ArenaRef::free_chunk_chain(current);
            }
            ArenaRef::free_chunk_chain(self.fresh_chunks.get());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn size_of_header() {
        assert_eq!(crate::HEADER_SIZE, 16);
    }
}
