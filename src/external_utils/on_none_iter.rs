pub struct OnNoneIter<Iter, Item, OnNone> where Iter: Iterator<Item=Option<Item>>, OnNone: FnMut() {
    iter: Iter,
    on_none: OnNone,
}

impl<Iter, Item, OnNone> Iterator for OnNoneIter<Iter, Item, OnNone> where Iter: Iterator<Item=Option<Item>>, OnNone: FnMut() {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                None => return None,
                Some(Some(result)) => return Some(result),
                _ => (self.on_none)(),
            }
        }
    }
}

pub struct NoneIgnoredIter<Iter, Item> where Iter: Iterator<Item=Option<Item>> {
    iter: Iter,
}

impl<Iter, Item> Iterator for NoneIgnoredIter<Iter, Item> where Iter: Iterator<Item=Option<Item>> {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                None => return None,
                Some(Some(result)) => return Some(result),
                _ => {}
            }
        }
    }
}


pub trait IterOnNone<Iter, Item> where Iter: Iterator<Item=Option<Item>> {
    fn on_none<OnNone: FnMut()>(self, on_error: OnNone) -> OnNoneIter<Iter, Item, OnNone> where OnNone: FnMut();
    fn ignore_nones(self) -> NoneIgnoredIter<Iter, Item>;
}

impl<Iter, Item> IterOnNone<Iter, Item> for Iter where Iter: Iterator<Item=Option<Item>> {
    fn on_none<OnNone: FnMut()>(self, on_error: OnNone) -> OnNoneIter<Iter, Item, OnNone> where Iter: Iterator<Item=Option<Item>>, OnNone: FnMut() {
        OnNoneIter {
            iter: self,
            on_none: on_error,
        }
    }

    fn ignore_nones(self) -> NoneIgnoredIter<Iter, Item> {
        NoneIgnoredIter { iter: self }
    }
}