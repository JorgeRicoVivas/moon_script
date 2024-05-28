pub struct OnErrorsIter<Iter, Item, Err, OnError> where Iter: Iterator<Item=Result<Item, Err>>, OnError: FnMut(Err) {
    iter: Iter,
    on_error: OnError,
}

impl<Iter, Item, Err, OnError> Iterator for OnErrorsIter<Iter, Item, Err, OnError> where Iter: Iterator<Item=Result<Item, Err>>, OnError: FnMut(Err) {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                None => return None,
                Some(Ok(result)) => return Some(result),
                Some(Err(error)) => (self.on_error)(error),
            }
        }
    }
}

pub struct ErrorsIgnoredIter<Iter, Item, Err> where Iter: Iterator<Item=Result<Item, Err>> {
    iter: Iter,
}

impl<Iter, Item, Err> Iterator for ErrorsIgnoredIter<Iter, Item, Err> where Iter: Iterator<Item=Result<Item, Err>> {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                None => return None,
                Some(Ok(result)) => return Some(result),
                _ => {}
            }
        }
    }
}


pub trait IterOnError<Iter, Item, Err> where Iter: Iterator<Item=Result<Item, Err>> {
    fn on_errors<OnError: FnMut(Err)>(self, on_error: OnError) -> OnErrorsIter<Iter, Item, Err, OnError> where OnError: FnMut(Err);
    fn ignore_errors(self) -> ErrorsIgnoredIter<Iter, Item, Err>;
}

impl<Iter, Item, Err> IterOnError<Iter, Item, Err> for Iter where Iter: Iterator<Item=Result<Item, Err>> {
    fn on_errors<OnError: FnMut(Err)>(self, on_error: OnError) -> OnErrorsIter<Iter, Item, Err, OnError> where Iter: Iterator<Item=Result<Item, Err>>, OnError: FnMut(Err) {
        OnErrorsIter {
            iter: self,
            on_error,
        }
    }

    fn ignore_errors(self) -> ErrorsIgnoredIter<Iter, Item, Err> {
        ErrorsIgnoredIter { iter: self }
    }
}


