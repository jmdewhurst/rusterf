use core::cmp::min;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct CircleBuffer2n<T: Copy + Default> {
    data: Vec<T>,
    n: usize,
    len: usize,
    posn: usize,
}

impl<T: Copy + Default> CircleBuffer2n<T> {
    pub fn new(n: usize) -> Self {
        let mut buff = Vec::<T>::with_capacity(1 << n);
        for _ in 0..(1 << n) {
            buff.push(Default::default());
        }
        CircleBuffer2n {
            data: buff,
            n,
            len: (1 << n),
            posn: (1 << n) - 1,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn exponent(&self) -> usize {
        self.n
    }

    pub fn index(&self) -> usize {
        self.posn % self.len
    }

    pub fn append(&mut self, val: T) {
        self.posn = self.posn.wrapping_add(1);
        self.data[self.posn % self.len] = val;
    }
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, new_vals: I) {
        for i in new_vals {
            self.append(i);
        }
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            parent: self,
            posn: 0,
        }
    }
    pub fn iter_last_n(&self, num: usize) -> Iter<T> {
        Iter {
            parent: self,
            posn: self.len - min(num, self.len),
        }
    }
}

#[derive(Debug)]
pub struct Iter<'a, T: Default + Copy> {
    parent: &'a CircleBuffer2n<T>,
    posn: usize,
}

impl<'a, T: Default + Copy> Iterator for Iter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.posn += 1;
        if self.posn <= self.parent.len() {
            Some(self.parent.data[(self.parent.posn.wrapping_add(self.posn)) % self.parent.len])
        } else {
            None
        }
    }
}

impl<'a, T: Default + Copy> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.parent.len() - self.posn
    }
}

impl<'a, T: Default + Copy> IntoIterator for &'a CircleBuffer2n<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}
