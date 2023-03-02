#![warn(clippy::pedantic)]

/// This is a circular array data structure that is optimized for a few use cases.
/// Its length is always a power of 2, meaning it's very cheap to write, but new writes will always
/// overwrite the oldest data. Also, it starts out zeroed, not empty, unlike a vector.
/// As a result, it's much faster to immutably iterate over than a vecdeque (~8x faster in my
/// benchmarks), as well as being more concise and only allocating on initialization.
/// NOTE:
/// this module makes heavy use of the fact that if `k` is a power of two, then
/// `x % k` is equal to `x & (k - 1)`. That is, `2^n - 1` is a bitmask of the lower `n` bits
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct DyadicRingBuffer<T: Copy + Default> {
    n: usize,
    len: usize,
    posn: usize,
    data: Vec<T>,
}

impl<T: Copy + Default> DyadicRingBuffer<T> {
    #[must_use]
    pub fn new(n: usize) -> Option<Self> {
        // semi-arbitrary maximum size -- at the moment, we limit log length to about 1M entries.
        // One could expand this, but you may run into issues trying to allocate a very large
        // array. You probably shouldn't be storing that many values in a basic array, though?
        if n > 20 {
            return None;
        };
        let mut buff = Vec::<T>::with_capacity(1 << n);
        for _ in 0..(1 << n) {
            buff.push(Default::default());
        }
        Some(DyadicRingBuffer {
            data: buff,
            n,
            len: (1 << n),
            posn: (1 << n) - 1,
        })
    }

    #[must_use]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn exponent(&self) -> usize {
        self.n
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.posn & (self.len - 1)
    }

    pub fn push(&mut self, val: T) {
        self.posn = self.posn.wrapping_add(1);
        self.data[self.posn & (self.len - 1)] = val;
    }
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, new_vals: I) {
        for i in new_vals {
            self.push(i);
        }
    }

    #[must_use]
    pub fn iter(&self) -> Iter<T> {
        Iter {
            parent: self,
            posn: 0,
        }
    }
    #[must_use]
    pub fn last_n(&self, num: usize) -> Iter<T> {
        Iter {
            parent: self,
            posn: self.len - num,
        }
    }

    #[must_use]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        let (second, first) = self.data.split_at_mut(self.posn & (self.len - 1));
        IterMut {
            parent_first: Some(first),
            parent_second: Some(second),
        }
    }
}

pub struct Iter<'a, T: Default + Copy> {
    parent: &'a DyadicRingBuffer<T>,
    posn: usize,
}
impl<'a, T: Default + Copy> Iterator for Iter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.posn += 1;
        if self.posn > self.parent.len {
            None
        } else {
            Some(self.parent.data[(self.posn + self.parent.posn) & (self.parent.len - 1)])
        }
    }
}
impl<'a, T: Default + Copy> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.parent.len - self.posn
    }
}

#[derive(Debug)]
pub struct IterMut<'a, T: Default + Copy> {
    // The ``back half'' of the parent array-- everything AFTER the current position in the
    // underlying vector.
    parent_first: Option<&'a mut [T]>,
    parent_second: Option<&'a mut [T]>,
}
impl<'a, T: Default + Copy> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        self.parent_first
            .take() //take contents of parent_first
            .and_then(|x| {
                // if parent_first is Some
                // and the contents are a non-empty slice
                if let Some((next_elem, remainder_first)) = x.split_first_mut() {
                    self.parent_first = Some(remainder_first);
                    Some(next_elem)
                // otherwise parent_first was empty, so we null it out
                } else {
                    self.parent_first = None;
                    None
                }
            })
            // if we've made it to here without anything to give back, we should try the
            // same things on parent_second
            .or_else(|| {
                self.parent_second.take().and_then(|y| {
                    let (next_elem, remainder_second) = y.split_first_mut()?;
                    self.parent_second = Some(remainder_second);
                    Some(next_elem)
                })
            })
    }
}
impl<'a, T: Default + Copy> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.parent_first.as_ref().map_or(0, |x| x.len())
            + self.parent_second.as_ref().map_or(0, |x| x.len())
    }
}

impl<'a, T: Default + Copy> IntoIterator for &'a DyadicRingBuffer<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        let buff = DyadicRingBuffer::<usize>::new(8).expect("should allocate");
        assert_eq!(buff.n, 8);
        assert_eq!(buff.len, buff.data.len());
        assert_eq!(buff.len, 2_usize.pow(8));

        let buff = DyadicRingBuffer::<usize>::new(0).expect("should allocate");
        assert_eq!(buff.n, 0);
        assert_eq!(buff.len, buff.data.len());
        assert_eq!(buff.len, 2_usize.pow(0));

        assert!(DyadicRingBuffer::<usize>::new(24).is_none());
    }

    #[test]
    fn iter() {
        let mut buff = DyadicRingBuffer::new(3).expect("should allocate");
        for i in 0..10 {
            buff.push(i);
        }
        // println!("buff {:?}", buff);
        let vecb: Vec<i32> = buff.iter().collect();
        let vec_ref: Vec<i32> = (2..10).collect();
        assert_eq!(vecb, vec_ref);
    }

    #[test]
    fn last_n() {
        let mut buff = DyadicRingBuffer::new(16).expect("should allocate");
        for i in 0..buff.len() {
            buff.push(i);
        }
        let vecb: Vec<usize> = buff.last_n(4).collect();
        let vec_ref: Vec<usize> = ((buff.len() - 4)..(buff.len())).collect();
        assert_eq!(vecb, vec_ref);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn iter_mut() {
        let mut buff = DyadicRingBuffer::new(10).expect("should allocate");
        for i in 0..1024 {
            buff.push(i as f32);
        }
        for i in buff.iter_mut() {
            *i *= 2.0;
        }
        let vecb: Vec<f32> = buff.last_n(3).collect();
        let vec_ref: Vec<f32> = vec![2042.0, 2044.0, 2046.0];
        assert_eq!(vecb, vec_ref);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn size_hint() {
        let mut buff = DyadicRingBuffer::new(10).expect("should allocate");
        for i in 0..1024 {
            buff.push(i as f32);
        }
        {
            let mut i = buff.iter();
            i.next();
            i.next();
            i.next();
            assert_eq!(i.len(), 1021);
        }
        assert_eq!(buff.last_n(23).len(), 23);
        {
            let mut i_mut = buff.iter_mut();
            for _ in 0..73 {
                i_mut.next();
            }
            assert_eq!(i_mut.len(), 1024 - 73);
        }
    }
}
