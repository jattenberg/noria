use super::{key_to_double, key_to_single, Key};
use core::{DataType, Record};
use evmap;
use fnv::FnvBuildHasher;

pub(super) enum Handle {
    Single(evmap::WriteHandle<DataType, Vec<DataType>, i64, FnvBuildHasher>),
    Double(evmap::WriteHandle<(DataType, DataType), Vec<DataType>, i64, FnvBuildHasher>),
    Many(evmap::WriteHandle<Vec<DataType>, Vec<DataType>, i64, FnvBuildHasher>),
}

impl Handle {
    pub fn clear(&mut self, k: Key) {
        match *self {
            Handle::Single(ref mut h) => h.clear(key_to_single(k).into_owned()),
            Handle::Double(ref mut h) => h.clear(key_to_double(k).into_owned()),
            Handle::Many(ref mut h) => h.clear(k.into_owned()),
        }
    }

    pub fn empty(&mut self, k: Key) {
        match *self {
            Handle::Single(ref mut h) => h.empty(key_to_single(k).into_owned()),
            Handle::Double(ref mut h) => h.empty(key_to_double(k).into_owned()),
            Handle::Many(ref mut h) => h.empty(k.into_owned()),
        }
    }

    pub fn refresh(&mut self) {
        match *self {
            Handle::Single(ref mut h) => h.refresh(),
            Handle::Double(ref mut h) => h.refresh(),
            Handle::Many(ref mut h) => h.refresh(),
        }
    }

    pub fn set_meta(&mut self, meta: i64) -> i64 {
        match *self {
            Handle::Single(ref mut h) => h.set_meta(meta),
            Handle::Double(ref mut h) => h.set_meta(meta),
            Handle::Many(ref mut h) => h.set_meta(meta),
        }
    }

    pub fn meta_get_and<F, T>(&self, key: Key, then: F) -> Option<(Option<T>, i64)>
    where
        F: FnOnce(&[Vec<DataType>]) -> T,
    {
        match *self {
            Handle::Single(ref h) => {
                assert_eq!(key.len(), 1);
                h.meta_get_and(&key[0], then)
            }
            Handle::Double(ref h) => {
                assert_eq!(key.len(), 2);
                // we want to transmute &[T; 2] to &(T, T), but that's not actually safe
                // we're not guaranteed that they have the same memory layout
                // we *could* just clone DataType, but that would mean dealing with string refcounts
                // so instead, we play a trick where we memcopy onto the stack and then forget!
                //
                // h/t https://gist.github.com/mitsuhiko/f6478a0dd1ef174b33c63d905babc89a
                use std::mem;
                use std::ptr;
                unsafe {
                    let mut stack_key: (DataType, DataType) = mem::uninitialized();
                    ptr::copy_nonoverlapping(
                        &key[0] as *const DataType,
                        &mut stack_key.0 as *mut DataType,
                        1,
                    );
                    ptr::copy_nonoverlapping(
                        &key[1] as *const DataType,
                        &mut stack_key.1 as *mut DataType,
                        1,
                    );
                    let v = h.meta_get_and(&stack_key, then);
                    mem::forget(stack_key);
                    v
                }
            }
            Handle::Many(ref h) => h.meta_get_and(&key[..], then),
        }
    }

    pub fn add<I>(&mut self, key: &[usize], cols: usize, rs: I) -> isize
    where
        I: IntoIterator<Item = Record>,
    {
        let mut memory_delta = 0;
        match *self {
            Handle::Single(ref mut h) => {
                assert_eq!(key.len(), 1);
                for r in rs {
                    debug_assert!(r.len() >= cols);
                    match r {
                        Record::Positive(r) => {
                            memory_delta += r.deep_size_of() as usize;
                            h.insert(r[key[0]].clone(), r);
                        }
                        Record::Negative(r) => {
                            // TODO: evmap will remove the empty vec for a key if we remove the
                            // last record. this means that future lookups will fail, and cause a
                            // replay, which will produce an empty result. this will work, but is
                            // somewhat inefficient.
                            memory_delta - r.deep_size_of() as usize;
                            h.remove(r[key[0]].clone(), r);
                        }
                        Record::BaseOperation(..) => unreachable!(),
                    }
                }
            }
            Handle::Double(ref mut h) => {
                assert_eq!(key.len(), 2);
                for r in rs {
                    debug_assert!(r.len() >= cols);
                    match r {
                        Record::Positive(r) => {
                            memory_delta += r.deep_size_of() as usize;
                            h.insert((r[key[0]].clone(), r[key[1]].clone()), r);
                        }
                        Record::Negative(r) => {
                            memory_delta - r.deep_size_of() as usize;
                            h.remove((r[key[0]].clone(), r[key[1]].clone()), r);
                        }
                        Record::BaseOperation(..) => unreachable!(),
                    }
                }
            }
            Handle::Many(ref mut h) => for r in rs {
                debug_assert!(r.len() >= cols);
                let key = key.iter().map(|&k| &r[k]).cloned().collect();
                match r {
                    Record::Positive(r) => {
                        memory_delta += r.deep_size_of() as usize;
                        h.insert(key, r);
                    }
                    Record::Negative(r) => {
                        memory_delta - r.deep_size_of() as usize;
                        h.remove(key, r);
                    }
                    Record::BaseOperation(..) => unreachable!(),
                }
            },
        }
        memory_delta
    }
}